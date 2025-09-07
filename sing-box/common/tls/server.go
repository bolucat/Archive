package tls

import (
	"context"
	"net"
	"os"

	"github.com/sagernet/sing-box/common/badtls"
	"github.com/sagernet/sing-box/common/ktls"
	C "github.com/sagernet/sing-box/constant"
	"github.com/sagernet/sing-box/log"
	"github.com/sagernet/sing-box/option"
	E "github.com/sagernet/sing/common/exceptions"
	aTLS "github.com/sagernet/sing/common/tls"
)

func NewServer(ctx context.Context, logger log.Logger, options option.InboundTLSOptions) (ServerConfig, error) {
	if !options.Enabled {
		return nil, nil
	}
	if options.Reality != nil && options.Reality.Enabled {
		return NewRealityServer(ctx, logger, options)
	}
	return NewSTDServer(ctx, logger, options)
}

func ServerHandshake(ctx context.Context, conn net.Conn, config ServerConfig) (Conn, error) {
	ctx, cancel := context.WithTimeout(ctx, C.TCPTimeout)
	defer cancel()
	tlsConn, err := aTLS.ServerHandshake(ctx, conn, config)
	if err != nil {
		return nil, err
	}
	if kConfig, isKConfig := config.(KTLSCapableConfig); isKConfig && (kConfig.KernelTx() || kConfig.KernelRx()) {
		if !C.IsLinux {
			return nil, E.New("kTLS is only supported on Linux")
		}
		return ktls.NewConn(tlsConn, kConfig.KernelTx(), kConfig.KernelRx())
	}
	readWaitConn, err := badtls.NewReadWaitConn(tlsConn)
	if err == nil {
		return readWaitConn, nil
	} else if err != os.ErrInvalid {
		return nil, err
	}
	return tlsConn, nil
}
