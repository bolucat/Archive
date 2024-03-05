package stun

import (
	"errors"
	"fmt"
	"net"
	"time"

	"github.com/Dreamacro/clash/transport/socks5"
	"github.com/pion/stun"
	"github.com/sirupsen/logrus"
	"github.com/v2fly/v2ray-core/v5/common/buf"
)

//go:generate go run ../errorgen

type stunServerConn struct {
	conn        net.PacketConn
	LocalAddr   net.Addr
	RemoteAddr  *net.UDPAddr
	OtherAddr   *net.UDPAddr
	messageChan chan *stunResponse
}

type stunResponse struct {
	*stun.Message
	net.Addr
}

func (c *stunServerConn) Close() error {
	return c.conn.Close()
}

var timeout = 5

const (
	messageHeaderSize = 20
)

const (
	NoResult = iota
	EndpointIndependentNoNAT
	EndpointIndependent
	AddressDependent
	AddressAndPortDependent
)

var (
	errResponseMessage = errors.New("error reading from response message channel")
	errTimedOut        = errors.New("timed out waiting for response")
	errNoOtherAddress  = errors.New("no OTHER-ADDRESS in message")
)

func Test(addrStr string, socksPort int) (natMapping int, natFiltering int, err error) {
	if addrStr == "" {
		addrStr = "stun.syncthing.net:3478"
	}
	var mapTestConn *stunServerConn
	newConn := func() error {
		if err == nil {
			mapTestConn, err = connect(addrStr, socksPort)
			if err != nil {
				e := newError("error creating STUN connection").Base(err)
				logrus.Warn(e)
				return e
			}
		}
		return err
	}
	if newConn() == nil {
		natMapping, err = mappingTests(mapTestConn)
	}
	if newConn() == nil {
		natFiltering, err = filteringTests(mapTestConn)
	}
	return
}

// RFC5780: 4.3.  Determining NAT Mapping Behavior
func mappingTests(mapTestConn *stunServerConn) (int, error) {
	defer mapTestConn.Close()
	// Test I: Regular binding request
	logrus.Info(newError("mapping test I: regular binding request"))
	request := stun.MustBuild(stun.TransactionID, stun.BindingRequest)

	resp, err := mapTestConn.roundTrip(request, mapTestConn.RemoteAddr)
	if err != nil {
		return NoResult, err
	}

	// Parse response message for XOR-MAPPED-ADDRESS and make sure OTHER-ADDRESS valid
	resps := parse(resp.Message)
	if resps.xorAddr == nil || resps.otherAddr == nil {
		err := newError("NAT discovery feature not supported by this server").Base(errNoOtherAddress)
		logrus.Warn(err)
		return NoResult, err
	}
	addr, err := net.ResolveUDPAddr("udp4", resps.otherAddr.String())
	if err != nil {
		err := newError("failed resolving OTHER-ADDRESS: ", resps.otherAddr)
		logrus.Warn(err)
		return NoResult, err
	}
	mapTestConn.OtherAddr = addr
	logrus.Info(newError("received XOR-MAPPED-ADDRESS: ", resps.xorAddr))

	// Assert mapping behavior
	if resps.xorAddr.String() == mapTestConn.LocalAddr.String() {
		logrus.Info(newError("NAT mapping behavior: endpoint independent (no NAT)"))
		return EndpointIndependentNoNAT, err
	}

	// Test II: Send binding request to the other address but primary port
	logrus.Info(newError("mapping test II: Send binding request to the other address but primary port"))
	oaddr := *mapTestConn.OtherAddr
	oaddr.Port = mapTestConn.RemoteAddr.Port
	resp, err = mapTestConn.roundTrip(request, &oaddr)
	if err != nil {
		if !errors.Is(err, errTimedOut) {
			return NoResult, err
		}
	} else {
		// Assert mapping behavior
		resps2 := parse(resp.Message)

		if resps2.respOrigin.String() != oaddr.String() {
			return AddressAndPortDependent, nil
		}

		logrus.Info(newError("received XOR-MAPPED-ADDRESS: ", resps2.xorAddr))
		if resps2.xorAddr.String() == resps.xorAddr.String() {
			logrus.Info(newError("NAT mapping behavior: endpoint independent"))
			return EndpointIndependent, nil
		}

		resps = resps2
	}

	// Test III: Send binding request to the other address and port
	logrus.Info(newError("mapping test III: Send binding request to the other address and port"))
	resp, err = mapTestConn.roundTrip(request, mapTestConn.OtherAddr)
	if err != nil {
		if !errors.Is(err, errTimedOut) {
			return NoResult, err
		}
	} else {
		resps3 := parse(resp.Message)
		logrus.Info(newError("received XOR-MAPPED-ADDRESS: ", resps3.xorAddr))
		if resps3.xorAddr.String() == resps.xorAddr.String() {
			logrus.Info(newError("NAT mapping behavior: address dependent"))
			return AddressDependent, nil
		}
	}

	logrus.Info(newError("NAT mapping behavior: address and port dependent"))
	return AddressAndPortDependent, nil
}

// RFC5780: 4.4.  Determining NAT Filtering Behavior
func filteringTests(mapTestConn *stunServerConn) (int, error) {
	defer mapTestConn.Close()
	// Test I: Regular binding request
	logrus.Info(newError("filtering test I: regular binding request"))
	request := stun.MustBuild(stun.TransactionID, stun.BindingRequest)

	resp, err := mapTestConn.roundTrip(request, mapTestConn.RemoteAddr)
	if err != nil {
		return NoResult, err
	}
	resps := parse(resp.Message)
	if resps.xorAddr == nil || resps.otherAddr == nil {
		err := newError("NAT discovery feature not supported by this server").Base(errNoOtherAddress)
		logrus.Warn(err)
		return NoResult, err
	}
	addr, err := net.ResolveUDPAddr("udp", resps.otherAddr.String())
	if err != nil {
		err := newError("failed resolving OTHER-ADDRESS: ", resps.otherAddr).Base(err)
		logrus.Warn(err)
		return NoResult, err
	}
	mapTestConn.OtherAddr = addr

	// Test II: Request to change both IP and port
	logrus.Info(newError("filtering test II: request to change both IP and port"))
	request = stun.MustBuild(stun.TransactionID, stun.BindingRequest)
	request.Add(stun.AttrChangeRequest, []byte{0x00, 0x00, 0x00, 0x06})

	resp, err = mapTestConn.roundTrip(request, mapTestConn.RemoteAddr)
	if err == nil {
		parse(resp.Message) // just to print out the resp
		if resp.Addr.String() != mapTestConn.RemoteAddr.String() {
			logrus.Info(newError("NAT filtering behavior: endpoint independent"))
			return EndpointIndependent, nil
		}
	} else if !errors.Is(err, errTimedOut) {
		return NoResult, err // something else went wrong
	}

	// Test III: Request to change port only
	logrus.Info(newError("filtering test III: request to change port only"))
	request = stun.MustBuild(stun.TransactionID, stun.BindingRequest)
	request.Add(stun.AttrChangeRequest, []byte{0x00, 0x00, 0x00, 0x02})

	resp, err = mapTestConn.roundTrip(request, mapTestConn.RemoteAddr)
	if err == nil {
		parse(resp.Message) // just to print out the resp
		if resp.Addr.String() != mapTestConn.RemoteAddr.String() {
			logrus.Info(newError("NAT filtering behavior: address dependent"))
			return AddressDependent, nil
		}
	} else if !errors.Is(err, errTimedOut) {
		return NoResult, err
	}
	logrus.Info(newError("NAT filtering behavior: address and port dependent"))

	return AddressAndPortDependent, nil
}

// Parse a STUN message
func parse(msg *stun.Message) (ret struct {
	xorAddr    *stun.XORMappedAddress
	otherAddr  *stun.OtherAddress
	respOrigin *stun.ResponseOrigin
	mappedAddr *stun.MappedAddress
	software   *stun.Software
},
) {
	ret.mappedAddr = &stun.MappedAddress{}
	ret.xorAddr = &stun.XORMappedAddress{}
	ret.respOrigin = &stun.ResponseOrigin{}
	ret.otherAddr = &stun.OtherAddress{}
	ret.software = &stun.Software{}
	if ret.xorAddr.GetFrom(msg) != nil {
		ret.xorAddr = nil
	}
	if ret.otherAddr.GetFrom(msg) != nil {
		ret.otherAddr = nil
	}
	if ret.respOrigin.GetFrom(msg) != nil {
		ret.respOrigin = nil
	}
	if ret.mappedAddr.GetFrom(msg) != nil {
		ret.mappedAddr = nil
	}
	if ret.software.GetFrom(msg) != nil {
		ret.software = nil
	}
	logrus.Debug(newError(msg))
	logrus.Debug(newError("MAPPED-ADDRESS:     ", ret.mappedAddr))
	logrus.Debug(newError("XOR-MAPPED-ADDRESS: ", ret.xorAddr))
	logrus.Debug(newError("RESPONSE-ORIGIN:    ", ret.respOrigin))
	logrus.Debug(newError("OTHER-ADDRESS:      ", ret.otherAddr))
	logrus.Debug(newError("SOFTWARE:           ", ret.software))
	for _, attr := range msg.Attributes {
		switch attr.Type {
		case
			stun.AttrXORMappedAddress,
			stun.AttrOtherAddress,
			stun.AttrResponseOrigin,
			stun.AttrMappedAddress,
			stun.AttrSoftware:
			break //nolint: staticcheck
		default:
			logrus.Debug(newErrorf("%v (l=%v)", attr, attr.Length))
		}
	}
	return ret
}

// Given an address string, returns a StunServerConn
func connect(addrStr string, socksPort int) (*stunServerConn, error) {
	addr, err := net.ResolveUDPAddr("udp", addrStr)
	if err != nil {
		return nil, newError("failed to resolve server address ", addrStr).Base(err)
	}

	logrus.Info(newError("connecting to STUN server: ", addrStr))

	var mapTestConn net.PacketConn

	socksConn, err := net.Dial("tcp", fmt.Sprint("127.0.0.1:", socksPort))
	if err == nil {
		handshake, err := socks5.ClientHandshake(socksConn, socks5.ParseAddr(addrStr), socks5.CmdUDPAssociate, nil)
		if err != nil {
			logrus.Warn(newError("failed to do udp associate handshake").Base(err))
		}
		udpConn, err := net.DialUDP("udp", nil, handshake.UDPAddr())
		if err == nil {
			mapTestConn = &socksPacketConn{udpConn, socksConn}
		}
	}

	if mapTestConn == nil {
		mapTestConn, err = net.ListenUDP("udp", nil)
		if err != nil {
			return nil, newError("failed to listen udp").Base(err)
		}
	}

	logrus.Info(newError("local address: ", mapTestConn.LocalAddr()))
	logrus.Info(newError("remote address: ", addr))

	mChan := listen(mapTestConn)

	return &stunServerConn{
		conn:        mapTestConn,
		LocalAddr:   mapTestConn.LocalAddr(),
		RemoteAddr:  addr,
		messageChan: mChan,
	}, nil
}

// Send request and wait for response or timeout
func (c *stunServerConn) roundTrip(msg *stun.Message, addr net.Addr) (*stunResponse, error) {
	_ = msg.NewTransactionID()
	logrus.Debug(newErrorf("sending to %v: (%v bytes)", addr, msg.Length+messageHeaderSize))
	logrus.Debug(newError(msg).AtDebug())
	for _, attr := range msg.Attributes {
		logrus.Debug(newErrorf("%v (l=%v)", attr, attr.Length))
	}
	_, err := c.conn.WriteTo(msg.Raw, addr)
	if err != nil {
		logrus.Warn(newError("error sending request to ", addr))
		return nil, err
	}

	// Wait for response or timeout
	select {
	case r, ok := <-c.messageChan:
		if !ok {
			return nil, errResponseMessage
		}
		return r, nil
	case <-time.After(time.Duration(timeout) * time.Second):
		logrus.Info(newError("timed out waiting for response from server ", addr))
		return nil, errTimedOut
	}
}

// taken from https://github.com/pion/stun/blob/master/cmd/stun-traversal/main.go
func listen(conn net.PacketConn) (messages chan *stunResponse) {
	messages = make(chan *stunResponse)
	go func() {
		buffer := buf.New()
		defer buffer.Release()
		b := buffer.Extend(buf.Size)

		for {
			n, addr, err := conn.ReadFrom(b)
			if err != nil {
				close(messages)
				return
			}
			logrus.Info(newErrorf("response from %v: (%d bytes)", addr, n))
			b = b[:n]

			r := &stunResponse{
				Message: &stun.Message{
					Raw: b,
				},
				Addr: addr,
			}
			err = r.Message.Decode()
			if err != nil {
				logrus.Warn(newErrorf("error decoding message").Base(err))
				close(messages)
				return
			}

			messages <- r
		}
	}()
	return
}
