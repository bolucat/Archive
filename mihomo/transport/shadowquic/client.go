package shadowquic

import (
	"context"
	"errors"
	"net"
	"sync"

	C "github.com/metacubex/mihomo/constant"

	"github.com/metacubex/jls-quic-go"
)

type DialFunc func(ctx context.Context) (*quic.Conn, error)

type ClientOption struct {
	Dial          DialFunc
	UDPOverStream bool
}

type Client struct {
	option *ClientOption

	mu     sync.Mutex
	conn   *connState
	closed bool
}

func NewClient(option *ClientOption) *Client {
	return &Client{option: option}
}

func (c *Client) getConn(ctx context.Context) (*connState, error) {
	c.mu.Lock()
	defer c.mu.Unlock()

	if c.closed {
		return nil, net.ErrClosed
	}
	if c.conn != nil && !c.conn.closed() {
		return c.conn, nil
	}
	if c.option == nil || c.option.Dial == nil {
		return nil, errors.New("shadowquic: dial function is nil")
	}
	quicConn, err := c.option.Dial(ctx)
	if err != nil {
		return nil, err
	}
	c.conn = newConnState(quicConn)
	return c.conn, nil
}

func (c *Client) DialContext(ctx context.Context, metadata *C.Metadata) (net.Conn, error) {
	state, err := c.getConn(ctx)
	if err != nil {
		return nil, err
	}
	target, err := MetadataAddr(metadata)
	if err != nil {
		return nil, err
	}
	stream, err := state.quicConn.OpenStreamSync(ctx)
	if err != nil {
		return nil, err
	}
	conn := NewQuicStreamConn(stream, state.quicConn.LocalAddr(), state.quicConn.RemoteAddr(), nil)
	if err = WriteRequest(conn, CommandConnect, target); err != nil {
		_ = conn.Close()
		return nil, err
	}
	return conn, nil
}

func (c *Client) ListenPacket(ctx context.Context, metadata *C.Metadata) (net.PacketConn, error) {
	state, err := c.getConn(ctx)
	if err != nil {
		return nil, err
	}
	stream, err := state.quicConn.OpenStreamSync(ctx)
	if err != nil {
		return nil, err
	}
	mode := udpModeDatagram
	command := CommandAssociateDatagram
	if c.option != nil && c.option.UDPOverStream {
		mode = udpModeStream
		command = CommandAssociateStream
	}
	if err = WriteRequest(stream, command, UnspecifiedAddr()); err != nil {
		_ = stream.Close()
		return nil, err
	}
	return newAssociation(state, stream, mode), nil
}

func (c *Client) Close() error {
	c.mu.Lock()
	defer c.mu.Unlock()

	c.closed = true
	if c.conn != nil {
		return c.conn.closeWithError(0, "client closed")
	}
	return nil
}
