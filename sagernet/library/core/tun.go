package libcore

import (
	"container/list"
	"context"
	"io"
	"math"
	"net"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"sync/atomic"
	"time"

	"github.com/sirupsen/logrus"
	"github.com/v2fly/v2ray-core/v4"
	"github.com/v2fly/v2ray-core/v4/common"
	"github.com/v2fly/v2ray-core/v4/common/buf"
	v2rayNet "github.com/v2fly/v2ray-core/v4/common/net"
	"github.com/v2fly/v2ray-core/v4/common/session"
	"github.com/v2fly/v2ray-core/v4/common/task"
	"github.com/v2fly/v2ray-core/v4/features/dns/localdns"
	"github.com/v2fly/v2ray-core/v4/transport"
	"github.com/v2fly/v2ray-core/v4/transport/internet"
	"github.com/v2fly/v2ray-core/v4/transport/pipe"
	"libcore/constant"
	"libcore/gvisor"
	"libcore/nat"
	"libcore/tun"
)

var _ tun.Handler = (*Tun2ray)(nil)

type Tun2ray struct {
	dev                 tun.Tun
	router              string
	v2ray               *V2RayInstance
	sniffing            bool
	overrideDestination bool
	debug               bool

	dumpUid      bool
	trafficStats bool
	pcap         bool

	udpTable  sync.Map
	appStats  sync.Map
	lockTable sync.Map

	connectionsLock sync.Mutex
	connections     list.List
}

type TunConfig struct {
	FileDescriptor      int32
	Protect             bool
	Protector           Protector
	MTU                 int32
	V2Ray               *V2RayInstance
	VLAN4Router         string
	IPv6Mode            int32
	Implementation      int32
	Sniffing            bool
	OverrideDestination bool
	Debug               bool
	DumpUID             bool
	TrafficStats        bool
	PCap                bool
	ErrorHandler        ErrorHandler
	LocalResolver       LocalResolver
}

type ErrorHandler interface {
	HandleError(err string)
}

type LocalResolver interface {
	LookupIP(network string, domain string) (string, error)
}

func NewTun2ray(config *TunConfig) (*Tun2ray, error) {
	if config.Debug {
		logrus.SetLevel(logrus.DebugLevel)
	} else {
		logrus.SetLevel(logrus.WarnLevel)
	}
	t := &Tun2ray{
		router:              config.VLAN4Router,
		v2ray:               config.V2Ray,
		sniffing:            config.Sniffing,
		overrideDestination: config.OverrideDestination,
		debug:               config.Debug,
		dumpUid:             config.DumpUID,
		trafficStats:        config.TrafficStats,
	}

	var err error
	switch config.Implementation {
	case constant.TunImplementationGVisor:
		var pcapFile *os.File
		if config.PCap {
			path := time.Now().UTC().String()
			path = externalAssetsPath + "/pcap/" + path + ".pcap"
			err = os.MkdirAll(filepath.Dir(path), 0o755)
			if err != nil {
				return nil, newError("unable to create pcap dir").Base(err)
			}
			pcapFile, err = os.Create(path)
			if err != nil {
				return nil, newError("unable to create pcap file").Base(err)
			}
		}

		t.dev, err = gvisor.New(config.FileDescriptor, config.MTU, t, gvisor.DefaultNIC, config.PCap, pcapFile, math.MaxUint32, config.IPv6Mode)
	case constant.TunImplementationSystem:
		t.dev, err = nat.New(config.FileDescriptor, config.MTU, t, config.IPv6Mode, config.ErrorHandler.HandleError)
	}

	if err != nil {
		return nil, err
	}

	if !config.Protect {
		config.Protector = noopProtectorInstance
	}

	dc := config.V2Ray.dnsClient
	internet.UseAlternativeSystemDialer(&protectedDialer{
		protector: config.Protector,
		resolver: func(domain string) ([]net.IP, error) {
			return dc.LookupIP(domain)
		},
	})

	if !config.Protect {
		localdns.SetLookupFunc(nil)
	} else {
		localdns.SetLookupFunc(func(network, host string) ([]v2rayNet.IP, error) {
			response, err := config.LocalResolver.LookupIP(network, host)
			if err != nil {
				return nil, err
			}
			addrs := strings.Split(response, ",")
			ips := make([]v2rayNet.IP, len(addrs))
			for i, addr := range addrs {
				ips[i] = net.ParseIP(addr)
			}
			return ips, nil
		})
	}

	internet.UseAlternativeSystemDNSDialer(&protectedDialer{
		protector: config.Protector,
		resolver: func(domain string) ([]net.IP, error) {
			return localdns.Instance.LookupIP(domain)
		},
	})

	net.DefaultResolver.Dial = t.dialDNS
	return t, nil
}

func (t *Tun2ray) Close() {
	net.DefaultResolver.Dial = nil
	localdns.SetLookupFunc(nil)

	closeIgnore(t.dev)
	t.connectionsLock.Lock()
	for item := t.connections.Front(); item != nil; item = item.Next() {
		common.Close(item.Value)
	}
	t.connectionsLock.Unlock()
}

func (t *Tun2ray) NewConnection(source v2rayNet.Destination, destination v2rayNet.Destination, conn net.Conn) {
	inbound := &session.Inbound{
		Source: source,
		Tag:    "socks",
	}

	isDns := destination.Address.String() == t.router
	if isDns {
		inbound.Tag = "dns-in"
	}

	var uid uint16
	var self bool

	if t.dumpUid || t.trafficStats {
		u, err := uidDumper.DumpUid(destination.Address.Family().IsIPv6(), false, source.Address.IP().String(), int32(source.Port), destination.Address.IP().String(), int32(destination.Port))
		if err == nil {
			uid = uint16(u)
			var info *UidInfo
			self = uid > 0 && int(uid) == os.Getuid()
			if t.debug && !self && uid >= 10000 {
				if err == nil {
					info, _ = uidDumper.GetUidInfo(int32(uid))
				}
				if info == nil {
					logrus.Infof("[TCP] %s ==> %s", source.NetAddr(), destination.NetAddr())
				} else {
					logrus.Infof("[TCP][%s (%d/%s)] %s ==> %s", info.Label, uid, info.PackageName, source.NetAddr(), destination.NetAddr())
				}
			}

			if uid < 10000 {
				uid = 1000
			}

			inbound.Uid = uint32(uid)
		}
	}

	ctx := core.WithContext(context.Background(), t.v2ray.core)
	ctx = session.ContextWithInbound(ctx, inbound)

	if !isDns && t.sniffing {
		req := session.SniffingRequest{
			Enabled:   true,
			RouteOnly: !t.overrideDestination,
		}
		if t.sniffing {
			req.OverrideDestinationForProtocol = append(req.OverrideDestinationForProtocol, "http", "tls")
		}
		ctx = session.ContextWithContent(ctx, &session.Content{
			SniffingRequest: req,
		})
	}

	var stats *appStats
	if t.trafficStats && !self && !isDns {
		if iStats, exists := t.appStats.Load(uid); exists {
			stats = iStats.(*appStats)
		} else {
			iCond, loaded := t.lockTable.LoadOrStore(uid, sync.NewCond(&sync.Mutex{}))
			cond := iCond.(*sync.Cond)
			if loaded {
				cond.L.Lock()
				cond.Wait()
				iStats, exists = t.appStats.Load(uid)
				if !exists {
					panic("unexpected sync read failed")
				}
				stats = iStats.(*appStats)
				cond.L.Unlock()
			} else {
				stats = &appStats{}
				t.appStats.Store(uid, stats)
				t.lockTable.Delete(uid)
				cond.Broadcast()
			}
		}
		atomic.AddInt32(&stats.tcpConn, 1)
		atomic.AddUint32(&stats.tcpConnTotal, 1)
		atomic.StoreInt64(&stats.deactivateAt, 0)
		defer func() {
			if atomic.AddInt32(&stats.tcpConn, -1)+atomic.LoadInt32(&stats.udpConn) == 0 {
				atomic.StoreInt64(&stats.deactivateAt, time.Now().Unix())
			}
		}()
		conn = &statsConn{conn, &stats.uplink, &stats.downlink}
	}

	t.connectionsLock.Lock()
	element := t.connections.PushBack(conn)
	t.connectionsLock.Unlock()

	reader, input := pipe.New()
	link := &transport.Link{Reader: reader, Writer: connWriter{conn, buf.NewWriter(conn)}}
	err := t.v2ray.dispatcher.DispatchLink(ctx, destination, link)
	if err != nil {
		newError("[TCP] dispatchLink failed: ", err).WriteToLog()
		return
	}

	if err = task.Run(ctx, func() error {
		return buf.Copy(buf.NewReader(conn), input)
	}); err != nil {
		closeIgnore(conn, link.Reader, link.Writer)
		newError("connection finished: ", err).AtDebug().WriteToLog()
	} else {
		closeIgnore(conn, link.Writer, link.Reader)
	}

	t.connectionsLock.Lock()
	t.connections.Remove(element)
	t.connectionsLock.Unlock()
}

type connWriter struct {
	net.Conn
	buf.Writer
}

func (t *Tun2ray) NewPacket(source v2rayNet.Destination, destination v2rayNet.Destination, data []byte, writeBack func([]byte, *net.UDPAddr) (int, error), closer io.Closer) {
	natKey := source.NetAddr()

	sendTo := func() bool {
		iConn, ok := t.udpTable.Load(natKey)
		if !ok {
			return false
		}
		conn := iConn.(net.PacketConn)
		_, err := conn.WriteTo(data, &net.UDPAddr{
			IP:   destination.Address.IP(),
			Port: int(destination.Port),
		})
		if err != nil {
			_ = conn.Close()
		}
		return true
	}

	var cond *sync.Cond

	if sendTo() {
		closeIgnore(closer)
		return
	} else {
		iCond, loaded := t.lockTable.LoadOrStore(natKey, sync.NewCond(&sync.Mutex{}))
		cond = iCond.(*sync.Cond)
		if loaded {
			cond.L.Lock()
			cond.Wait()
			sendTo()
			cond.L.Unlock()

			closeIgnore(closer)
			return
		}
	}

	inbound := &session.Inbound{
		Source: source,
		Tag:    "socks",
	}
	isDns := destination.Address.String() == t.router

	if isDns {
		inbound.Tag = "dns-in"
	}

	var uid uint16
	var self bool

	if t.dumpUid || t.trafficStats {

		u, err := uidDumper.DumpUid(source.Address.Family().IsIPv6(), true, source.Address.String(), int32(source.Port), destination.Address.String(), int32(destination.Port))
		if err == nil {
			uid = uint16(u)
			var info *UidInfo
			self = uid > 0 && int(uid) == os.Getuid()

			if t.debug && !self && uid >= 1000 {
				if err == nil {
					info, _ = uidDumper.GetUidInfo(int32(uid))
				}
				var tag string
				if !isDns {
					tag = "UDP"
				} else {
					tag = "DNS"
				}

				if info == nil {
					logrus.Infof("[%s] %s ==> %s", tag, source.NetAddr(), destination.NetAddr())
				} else {
					logrus.Infof("[%s][%s (%d/%s)] %s ==> %s", tag, info.Label, uid, info.PackageName, source.NetAddr(), destination.NetAddr())
				}
			}

			if uid < 10000 {
				uid = 1000
			}

			inbound.Uid = uint32(uid)
		}

	}

	ctx := session.ContextWithInbound(context.Background(), inbound)

	if !isDns && t.sniffing {
		req := session.SniffingRequest{
			Enabled:   true,
			RouteOnly: !t.overrideDestination,
		}
		if t.sniffing {
			req.OverrideDestinationForProtocol = append(req.OverrideDestinationForProtocol, "quic")
		}
		ctx = session.ContextWithContent(ctx, &session.Content{
			SniffingRequest: req,
		})
	}

	conn, err := t.v2ray.dialUDP(ctx, destination, time.Minute*5)
	if err != nil {
		logrus.Errorf("[UDP] dial failed: %s", err.Error())
		return
	}

	var stats *appStats
	if t.trafficStats && !self && !isDns {
		if iStats, exists := t.appStats.Load(uid); exists {
			stats = iStats.(*appStats)
		} else {
			iCond, loaded := t.lockTable.LoadOrStore(uid, sync.NewCond(&sync.Mutex{}))
			cond := iCond.(*sync.Cond)
			if loaded {
				cond.L.Lock()
				cond.Wait()
				iStats, exists = t.appStats.Load(uid)
				if !exists {
					panic("unexpected sync read failed")
				}
				stats = iStats.(*appStats)
				cond.L.Unlock()
			} else {
				stats = &appStats{}
				t.appStats.Store(uid, stats)
				t.lockTable.Delete(uid)
				cond.Broadcast()
			}
		}
		atomic.AddInt32(&stats.udpConn, 1)
		atomic.AddUint32(&stats.udpConnTotal, 1)
		atomic.StoreInt64(&stats.deactivateAt, 0)
		defer func() {
			if atomic.AddInt32(&stats.udpConn, -1)+atomic.LoadInt32(&stats.tcpConn) == 0 {
				atomic.StoreInt64(&stats.deactivateAt, time.Now().Unix())
			}
		}()
		conn = &statsPacketConn{conn, &stats.uplink, &stats.downlink}
	}

	t.connectionsLock.Lock()
	element := t.connections.PushBack(conn)
	t.connectionsLock.Unlock()

	t.udpTable.Store(natKey, conn)

	go sendTo()

	t.lockTable.Delete(natKey)
	cond.Broadcast()

	for {
		buffer, addr, err := conn.readFrom()
		if err != nil {
			break
		}
		if isDns {
			addr = nil
		}
		if addr, ok := addr.(*net.UDPAddr); ok {
			_, err = writeBack(buffer, addr)
		} else {
			_, err = writeBack(buffer, nil)
		}
		if err != nil {
			break
		}
	}
	// close
	closeIgnore(conn, closer)
	t.udpTable.Delete(natKey)

	t.connectionsLock.Lock()
	t.connections.Remove(element)
	t.connectionsLock.Unlock()
}

func (t *Tun2ray) dialDNS(ctx context.Context, _, _ string) (conn net.Conn, err error) {
	conn, err = t.v2ray.dialContext(session.ContextWithInbound(ctx, &session.Inbound{
		Tag:         "dns-in",
		SkipFakeDNS: true,
	}), v2rayNet.Destination{
		Network: v2rayNet.Network_UDP,
		Address: v2rayNet.ParseAddress(t.router),
		Port:    53,
	})
	if err == nil {
		conn = &wrappedConn{conn}
	}
	return
}

type wrappedConn struct {
	net.Conn
}

func (c *wrappedConn) ReadFrom(p []byte) (n int, addr net.Addr, err error) {
	n, err = c.Conn.Read(p)
	if err == nil {
		addr = c.Conn.RemoteAddr()
	}
	return
}

func (c *wrappedConn) WriteTo(p []byte, _ net.Addr) (n int, err error) {
	return c.Conn.Write(p)
}
