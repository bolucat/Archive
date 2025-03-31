package derp

import (
	"context"
	"net"
	"net/netip"
	"time"

	"github.com/sagernet/sing-box/adapter"
	boxService "github.com/sagernet/sing-box/adapter/service"
	"github.com/sagernet/sing-box/common/listener"
	C "github.com/sagernet/sing-box/constant"
	"github.com/sagernet/sing-box/log"
	"github.com/sagernet/sing-box/option"
	E "github.com/sagernet/sing/common/exceptions"
	"github.com/sagernet/sing/common/logger"
	N "github.com/sagernet/sing/common/network"
	"github.com/sagernet/tailscale/net/stun"
)

func RegisterSTUN(registry *boxService.Registry) {
	boxService.Register[option.DERPSTUNServiceOptions](registry, C.TypeDERPSTUN, NewSTUNService)
}

type STUNService struct {
	boxService.Adapter
	ctx      context.Context
	logger   logger.ContextLogger
	listener *listener.Listener
}

func NewSTUNService(ctx context.Context, logger log.ContextLogger, tag string, options option.DERPSTUNServiceOptions) (adapter.Service, error) {
	return &STUNService{
		Adapter: boxService.NewAdapter(C.TypeDERPSTUN, tag),
		ctx:     ctx,
		logger:  logger,
		listener: listener.New(listener.Options{
			Context: ctx,
			Logger:  logger,
			Network: []string{N.NetworkUDP},
			Listen:  options.ListenOptions,
		}),
	}, nil
}

func (d *STUNService) Start(stage adapter.StartStage) error {
	if stage != adapter.StartStateStart {
		return nil
	}
	packetConn, err := d.listener.ListenUDP()
	if err != nil {
		return err
	}
	go d.loopPacket(packetConn.(*net.UDPConn))
	return nil
}

func (d *STUNService) Close() error {
	return d.listener.Close()
}

func (d *STUNService) loopPacket(packetConn *net.UDPConn) {
	buffer := make([]byte, 65535)
	oob := make([]byte, 1024)
	var (
		n        int
		oobN     int
		addrPort netip.AddrPort
		err      error
	)
	for {
		n, oobN, _, addrPort, err = packetConn.ReadMsgUDPAddrPort(buffer, oob)
		if err != nil {
			if E.IsClosedOrCanceled(err) {
				return
			}
			time.Sleep(time.Second)
			continue
		}
		if !stun.Is(buffer[:n]) {
			continue
		}
		txid, err := stun.ParseBindingRequest(buffer[:n])
		if err != nil {
			continue
		}
		packetConn.WriteMsgUDPAddrPort(stun.Response(txid, addrPort), oob[:oobN], addrPort)
	}
}
