package stun

import (
	"context"
	"errors"
	"net"
	"time"

	"github.com/pion/stun"
	"github.com/v2fly/v2ray-core/v4/common/buf"
)

//go:generate go run ../errorgen

type stunServerConn struct {
	conn        net.PacketConn
	LocalAddr   net.Addr
	RemoteAddr  *net.UDPAddr
	OtherAddr   *net.UDPAddr
	messageChan chan *stun.Message
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

func Test(addrStr string) (natMapping int, natFiltering int, err error) {
	if addrStr == "" {
		addrStr = "stun.voip.blackberry.com:3478"
	}
	var mapTestConn *stunServerConn
	newConn := func() error {
		if err == nil {
			mapTestConn, err = connect(addrStr)
			if err != nil {
				e := newError("error creating STUN connection").Base(err).AtWarning()
				e.WriteToLog()
				err = e
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
	newError("mapping test I: regular binding request").AtInfo().WriteToLog()
	request := stun.MustBuild(stun.TransactionID, stun.BindingRequest)

	resp, err := mapTestConn.roundTrip(request, mapTestConn.RemoteAddr)
	if err != nil {
		return NoResult, err
	}

	// Parse response message for XOR-MAPPED-ADDRESS and make sure OTHER-ADDRESS valid
	resps1 := parse(resp)
	if resps1.xorAddr == nil || resps1.otherAddr == nil {
		err := newError("NAT discovery feature not supported by this server").Base(errNoOtherAddress)
		err.AtWarning().WriteToLog()
		return NoResult, err
	}
	addr, err := net.ResolveUDPAddr("udp4", resps1.otherAddr.String())
	if err != nil {
		err := newError("failed resolving OTHER-ADDRESS: ", resps1.otherAddr)
		err.AtWarning().WriteToLog()
		return NoResult, err
	}
	mapTestConn.OtherAddr = addr
	newError("received XOR-MAPPED-ADDRESS: ", resps1.xorAddr).AtInfo().WriteToLog()

	// Assert mapping behavior
	if resps1.xorAddr.String() == mapTestConn.LocalAddr.String() {
		newError("NAT mapping behavior: endpoint independent (no NAT)").AtInfo().WriteToLog()
		return EndpointIndependentNoNAT, err
	}

	// Test II: Send binding request to the other address but primary port
	newError("mapping test II: Send binding request to the other address but primary port").AtInfo().WriteToLog()
	oaddr := *mapTestConn.OtherAddr
	oaddr.Port = mapTestConn.RemoteAddr.Port
	resp, err = mapTestConn.roundTrip(request, &oaddr)
	if err != nil {
		return NoResult, err
	}

	// Assert mapping behavior
	resps2 := parse(resp)
	newError("received XOR-MAPPED-ADDRESS: ", resps2.xorAddr).AtInfo().WriteToLog()
	if resps2.xorAddr.String() == resps1.xorAddr.String() {
		newError("NAT mapping behavior: endpoint independent").AtInfo().WriteToLog()
		return EndpointIndependent, nil
	}

	// Test III: Send binding request to the other address and port
	newError("mapping test III: Send binding request to the other address and port").AtDebug().WriteToLog()
	resp, err = mapTestConn.roundTrip(request, mapTestConn.OtherAddr)
	if err != nil {
		return NoResult, err
	}

	// Assert mapping behavior
	resps3 := parse(resp)
	newError("received XOR-MAPPED-ADDRESS: ", resps3.xorAddr).AtInfo().WriteToLog()
	if resps3.xorAddr.String() == resps2.xorAddr.String() {
		newError("NAT mapping behavior: address dependent").AtInfo().WriteToLog()
		return AddressDependent, nil
	} else {
		newError("NAT mapping behavior: address and port dependent").AtInfo().WriteToLog()
		return AddressAndPortDependent, nil
	}
}

// RFC5780: 4.4.  Determining NAT Filtering Behavior
func filteringTests(mapTestConn *stunServerConn) (int, error) {
	defer mapTestConn.Close()
	// Test I: Regular binding request
	newError("filtering test I: regular binding request").AtInfo().WriteToLog()
	request := stun.MustBuild(stun.TransactionID, stun.BindingRequest)

	resp, err := mapTestConn.roundTrip(request, mapTestConn.RemoteAddr)
	if err != nil || errors.Is(err, errTimedOut) {
		return NoResult, err
	}
	resps := parse(resp)
	if resps.xorAddr == nil || resps.otherAddr == nil {
		err := newError("NAT discovery feature not supported by this server").Base(errNoOtherAddress).AtWarning()
		err.WriteToLog()
		return NoResult, err
	}
	addr, err := net.ResolveUDPAddr("udp4", resps.otherAddr.String())
	if err != nil {
		err := newError("failed resolving OTHER-ADDRESS: ", resps.otherAddr)
		err.AtWarning().WriteToLog()
		return NoResult, err
	}
	mapTestConn.OtherAddr = addr

	// Test II: Request to change both IP and port
	newError("filtering test II: request to change both IP and port").AtInfo().WriteToLog()
	request = stun.MustBuild(stun.TransactionID, stun.BindingRequest)
	request.Add(stun.AttrChangeRequest, []byte{0x00, 0x00, 0x00, 0x06})

	resp, err = mapTestConn.roundTrip(request, mapTestConn.RemoteAddr)
	if err == nil {
		parse(resp) // just to print out the resp
		newError("NAT filtering behavior: endpoint independent").AtInfo().WriteToLog()
		return EndpointIndependent, nil
	} else if !errors.Is(err, errTimedOut) {
		return NoResult, err // something else went wrong
	}

	// Test III: Request to change port only
	newError("filtering test III: request to change port only").AtInfo().WriteToLog()
	request = stun.MustBuild(stun.TransactionID, stun.BindingRequest)
	request.Add(stun.AttrChangeRequest, []byte{0x00, 0x00, 0x00, 0x02})

	resp, err = mapTestConn.roundTrip(request, mapTestConn.RemoteAddr)
	if err == nil {
		parse(resp) // just to print out the resp
		newError("NAT filtering behavior: address dependent").AtInfo().WriteToLog()
		return AddressDependent, nil
	} else if errors.Is(err, errTimedOut) {
		newError("NAT filtering behavior: address and port dependent").AtInfo().WriteToLog()
		return AddressAndPortDependent, nil
	}
	return NoResult, err
}

// Parse a STUN message
func parse(msg *stun.Message) (ret struct {
	xorAddr    *stun.XORMappedAddress
	otherAddr  *stun.OtherAddress
	respOrigin *stun.ResponseOrigin
	mappedAddr *stun.MappedAddress
	software   *stun.Software
}) {
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
	newError(msg).AtDebug().WriteToLog()
	newError("MAPPED-ADDRESS:     ", ret.mappedAddr).AtDebug().WriteToLog()
	newError("XOR-MAPPED-ADDRESS: ", ret.xorAddr).AtDebug().WriteToLog()
	newError("RESPONSE-ORIGIN:    ", ret.respOrigin).AtDebug().WriteToLog()
	newError("OTHER-ADDRESS:      ", ret.otherAddr).AtDebug().WriteToLog()
	newError("SOFTWARE:           ", ret.software).AtDebug().WriteToLog()
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
			newErrorf("%v (l=%v)", attr, attr.Length).AtDebug().WriteToLog()
		}
	}
	return ret
}

// Given an address string, returns a StunServerConn
func connect(addrStr string) (*stunServerConn, error) {
	newError("connecting to STUN server: ", addrStr).AtInfo().WriteToLog()
	host, port, err := net.SplitHostPort(addrStr)
	if err != nil {
		return nil, err
	}
	ctx, cancel := context.WithTimeout(context.Background(), time.Duration(5)*time.Second)
	addrs, err := net.DefaultResolver.LookupIPAddr(ctx, host)
	cancel()
	if err != nil {
		newError("error resolving address").Base(err).WriteToLog()
		return nil, err
	}
	addr, _ := net.ResolveUDPAddr("udp", net.JoinHostPort(addrs[0].IP.String(), port))
	c, err := net.ListenUDP("udp4", nil)
	if err != nil {
		return nil, err
	}
	newError("local address: ", c.LocalAddr()).AtInfo().WriteToLog()
	newError("remote address: ", addr).AtInfo().WriteToLog()

	mChan := listen(c)

	return &stunServerConn{
		conn:        c,
		LocalAddr:   c.LocalAddr(),
		RemoteAddr:  addr,
		messageChan: mChan,
	}, nil
}

// Send request and wait for response or timeout
func (c *stunServerConn) roundTrip(msg *stun.Message, addr net.Addr) (*stun.Message, error) {
	_ = msg.NewTransactionID()
	newErrorf("sending to %v: (%v bytes)", addr, msg.Length+messageHeaderSize).AtDebug().WriteToLog()
	newError(msg).AtDebug().WriteToLog()
	for _, attr := range msg.Attributes {
		newErrorf("%v (l=%v)", attr, attr.Length).AtDebug().WriteToLog()
	}
	_, err := c.conn.WriteTo(msg.Raw, addr)
	if err != nil {
		newError("error sending request to ", addr).AtWarning().WriteToLog()
		return nil, err
	}

	// Wait for response or timeout
	select {
	case m, ok := <-c.messageChan:
		if !ok {
			return nil, errResponseMessage
		}
		return m, nil
	case <-time.After(time.Duration(timeout) * time.Second):
		newError("timed out waiting for response from server ", addr).AtInfo().WriteToLog()
		return nil, errTimedOut
	}
}

// taken from https://github.com/pion/stun/blob/master/cmd/stun-traversal/main.go
func listen(conn *net.UDPConn) (messages chan *stun.Message) {
	messages = make(chan *stun.Message)
	go func() {
		buffer := buf.New()
		defer buffer.Release()
		b := buffer.Extend(buf.Size)

		for {
			n, addr, err := conn.ReadFromUDP(b)
			if err != nil {
				close(messages)
				return
			}
			newErrorf("response from %v: (%v bytes)", addr, n).AtInfo().WriteToLog()
			b = b[:n]

			m := &stun.Message{
				Raw: b,
			}
			err = m.Decode()
			if err != nil {
				newErrorf("error decoding message").Base(err).AtWarning().WriteToLog()
				close(messages)
				return
			}

			messages <- m
		}
	}()
	return
}
