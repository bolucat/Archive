package anytls

import (
	"bytes"
	"crypto/tls"
	"errors"
	"fmt"
	"io"
	"net"
	"strings"
	"time"

	"github.com/nadoo/glider/pkg/log"
	"github.com/nadoo/glider/pkg/socks"
	"github.com/nadoo/glider/proxy"
)

func init() {
	proxy.RegisterServer("anytls", NewAnyTLSServer)
	proxy.RegisterServer("anytlsc", NewClearTextServer)
}

func NewAnyTLSServer(s string, p proxy.Proxy) (proxy.Server, error) {
	a, err := NewAnyTLS(s, nil, p)
	if err != nil {
		return nil, fmt.Errorf("[anytls] create instance error: %s", err)
	}
	if a.certFile == "" || a.keyFile == "" {
		return nil, errors.New("[anytls] cert and key file path must be specified")
	}
	cert, err := tls.LoadX509KeyPair(a.certFile, a.keyFile)
	if err != nil {
		return nil, fmt.Errorf("[anytls] unable to load cert: %s, key %s, error: %s", a.certFile, a.keyFile, err)
	}
	a.tlsConfig = &tls.Config{Certificates: []tls.Certificate{cert}, MinVersion: tls.VersionTLS12}
	return a, nil
}

func NewClearTextServer(s string, p proxy.Proxy) (proxy.Server, error) {
	a, err := NewAnyTLS(s, nil, p)
	if err != nil {
		return nil, fmt.Errorf("[anytlsc] create instance error: %s", err)
	}
	a.withTLS = false
	return a, nil
}

func (s *AnyTLS) ListenAndServe() {
	l, err := net.Listen("tcp", s.addr)
	if err != nil {
		log.Fatalf("[anytls] failed to listen on %s: %v", s.addr, err)
		return
	}
	defer l.Close()

	log.F("[anytls] listening TCP on %s, with TLS: %v", s.addr, s.withTLS)
	for {
		c, err := l.Accept()
		if err != nil {
			log.F("[anytls] failed to accept: %v", err)
			continue
		}
		go s.Serve(c)
	}
}

func (s *AnyTLS) Serve(c net.Conn) {
	if s.withTLS {
		tlsConn := tls.Server(c, s.tlsConfig)
		if err := tlsConn.Handshake(); err != nil {
			_ = tlsConn.Close()
			log.F("[anytls] error in tls handshake: %s", err)
			return
		}
		c = tlsConn
	}
	headBuf := bytes.NewBuffer(nil)
	if err := readAuth(io.TeeReader(c, headBuf), s.password); err != nil {
		if s.fallback != "" {
			s.serveFallback(c, s.fallback, headBuf)
			return
		}
		_ = c.Close()
		log.F("[anytls] auth error from %s: %s", c.RemoteAddr(), err)
		return
	}

	ss := newSession(c)
	ss.start()
	for {
		st, err := ss.acceptStream()
		if err != nil {
			_ = ss.Close()
			return
		}
		go s.serveStream(ss, st)
	}
}

func (s *AnyTLS) serveFallback(c net.Conn, target string, headBuf *bytes.Buffer) {
	defer c.Close()

	dialer := s.proxy.NextDialer(target)
	rc, err := dialer.Dial("tcp", target)
	if err != nil {
		log.F("[anytls-fallback] %s <-> %s via %s, error in dial: %v", c.RemoteAddr(), target, dialer.Addr(), err)
		return
	}
	defer rc.Close()

	if _, err := rc.Write(headBuf.Bytes()); err != nil {
		log.F("[anytls-fallback] write to rc error: %v", err)
		return
	}

	log.F("[anytls-fallback] %s <-> %s via %s", c.RemoteAddr(), target, dialer.Addr())
	if err := proxy.Relay(c, rc); err != nil {
		log.F("[anytls-fallback] %s <-> %s via %s, relay error: %v", c.RemoteAddr(), target, dialer.Addr(), err)
	}
}

func (s *AnyTLS) serveStream(ss *session, st *stream) {
	defer st.Close()

	target, err := socks.ReadAddr(st)
	if err != nil {
		_ = ss.writeFrame(frame{command: cmdSYNACK, streamID: st.id, data: []byte(err.Error())})
		log.F("[anytls] read target error: %v", err)
		return
	}
	host, _, err := net.SplitHostPort(target.String())
	if err == nil && host == uotV2MagicHost {
		s.serveUoT(ss, st)
		return
	}

	rc, dialer, err := s.proxy.Dial("tcp", target.String())
	if err != nil {
		_ = ss.writeFrame(frame{command: cmdSYNACK, streamID: st.id, data: []byte(err.Error())})
		log.F("[anytls] %s <-> %s via %s, error in dial: %v", st.RemoteAddr(), target, dialer.Addr(), err)
		return
	}
	defer rc.Close()

	_ = ss.writeFrame(frame{command: cmdSYNACK, streamID: st.id})
	log.F("[anytls] %s <-> %s via %s", st.RemoteAddr(), target, dialer.Addr())
	if err = proxy.Relay(st, rc); err != nil {
		log.F("[anytls] %s <-> %s via %s, relay error: %v", st.RemoteAddr(), target, dialer.Addr(), err)
		if !strings.Contains(err.Error(), s.addr) {
			s.proxy.Record(dialer, false)
		}
	}
}

func (s *AnyTLS) serveUoT(ss *session, st *stream) {
	target, err := readUOTV2Request(st)
	if err != nil {
		_ = ss.writeFrame(frame{command: cmdSYNACK, streamID: st.id, data: []byte(err.Error())})
		log.F("[anytls] read udp-over-tcp request error: %v", err)
		return
	}

	dstPC, dialer, err := s.proxy.DialUDP("udp", target.String())
	if err != nil {
		_ = ss.writeFrame(frame{command: cmdSYNACK, streamID: st.id, data: []byte(err.Error())})
		log.F("[anytls] %s <-UoT-> %s via %s, error in dial: %v", st.RemoteAddr(), target, dialer.Addr(), err)
		return
	}
	defer dstPC.Close()

	_ = ss.writeFrame(frame{command: cmdSYNACK, streamID: st.id})
	pc := newUOTPacketConn(st, target)
	log.F("[anytls] %s <-UoT-> %s via %s", st.RemoteAddr(), target, dialer.Addr())

	go proxy.CopyUDP(dstPC, nil, pc, 2*time.Minute, 5*time.Second)
	if err := proxy.CopyUDP(pc, nil, dstPC, 2*time.Minute, 5*time.Second); err != nil {
		log.F("[anytls] %s <-UoT-> %s via %s, relay error: %v", st.RemoteAddr(), target, dialer.Addr(), err)
		if d, ok := dialer.(proxy.Dialer); ok && !strings.Contains(err.Error(), s.addr) {
			s.proxy.Record(d, false)
		}
	}
}
