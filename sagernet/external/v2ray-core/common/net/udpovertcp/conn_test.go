package udpovertcp_test

import (
	"testing"

	"golang.org/x/net/dns/dnsmessage"

	"github.com/v2fly/v2ray-core/v5/common"
	"github.com/v2fly/v2ray-core/v5/common/buf"
	"github.com/v2fly/v2ray-core/v5/common/net"
	"github.com/v2fly/v2ray-core/v5/common/net/udpovertcp"
)

func TestServerConn(t *testing.T) {
	udpConn, err := net.ListenUDP("udp", nil)
	common.Must(err)
	serverConn := udpovertcp.NewServerConn(udpConn)
	defer serverConn.Close()

	writer := udpovertcp.NewWriter(serverConn, nil)

	message := new(dnsmessage.Message)
	message.Header.ID = 1
	message.Header.RecursionDesired = true
	message.Questions = append(message.Questions, dnsmessage.Question{
		Name:  dnsmessage.MustNewName("google.com."),
		Type:  dnsmessage.TypeA,
		Class: dnsmessage.ClassINET,
	})

	packet, err := message.Pack()
	common.Must(err)
	buffer := buf.FromBytes(packet)
	endpoint := net.Destination{
		Network: net.Network_UDP,
		Address: net.IPAddress([]byte{8, 8, 8, 8}),
		Port:    53,
	}
	buffer.Endpoint = &endpoint
	common.Must(writer.WriteMultiBuffer(buf.MultiBuffer{buffer}))
	reader := udpovertcp.NewBufferedReader(serverConn)
	mb, err := reader.ReadMultiBuffer()
	common.Must(err)
	buffer = buf.New()
	defer buffer.Release()
	for _, p := range mb {
		buffer.Write(p.Bytes())
		p.Release()
	}
	common.Must(message.Unpack(buffer.Bytes()))
	for _, answer := range message.Answers {
		t.Log("got answer :", answer.Body)
	}
}
