package dispatcher

import (
	"context"

	"github.com/sagernet/sing/common/rw"
	"github.com/v2fly/v2ray-core/v5/common/buf"
	"github.com/v2fly/v2ray-core/v5/common/net"
	"github.com/v2fly/v2ray-core/v5/common/task"
	"github.com/v2fly/v2ray-core/v5/features/routing"
	"github.com/v2fly/v2ray-core/v5/transport"
	"github.com/v2fly/v2ray-core/v5/transport/internet"
)

var (
	_              routing.Dispatcher = (*SystemDispatcher)(nil)
	SystemInstance                    = &SystemDispatcher{}
)

type SystemDispatcher struct{}

func (s *SystemDispatcher) Type() interface{} {
	return routing.DispatcherType()
}

func (s *SystemDispatcher) Start() error {
	return nil
}

func (s *SystemDispatcher) Close() error {
	return nil
}

func (s *SystemDispatcher) Dispatch(ctx context.Context, dest net.Destination) (*transport.Link, error) {
	conn, err := internet.DialSystem(ctx, dest, nil)
	if err != nil {
		return nil, err
	}
	return &transport.Link{Reader: buf.NewReader(conn), Writer: buf.NewWriter(conn)}, nil
}

func (s *SystemDispatcher) DispatchLink(ctx context.Context, dest net.Destination, outbound *transport.Link) error {
	conn, err := internet.DialSystem(ctx, dest, nil)
	if err != nil {
		return err
	}
	return task.Run(ctx, func() error {
		return buf.Copy(buf.NewReader(conn), outbound.Writer)
	}, func() error {
		return buf.Copy(outbound.Reader, buf.NewWriter(conn))
	})
}

func (s *SystemDispatcher) DispatchConn(ctx context.Context, dest net.Destination, conn net.Conn, wait bool) error {
	destConn, err := internet.DialSystem(ctx, dest, nil)
	if err != nil {
		return err
	}
	if wait {
		return rw.CopyConn(ctx, conn, destConn)
	} else {
		go rw.CopyConn(ctx, conn, destConn)
		return nil
	}
}
