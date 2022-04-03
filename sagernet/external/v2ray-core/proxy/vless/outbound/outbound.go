package outbound

//go:generate go run github.com/v2fly/v2ray-core/v5/common/errors/errorgen

import (
	"context"
	"syscall"
	"time"

	core "github.com/v2fly/v2ray-core/v5"
	"github.com/v2fly/v2ray-core/v5/common"
	"github.com/v2fly/v2ray-core/v5/common/buf"
	"github.com/v2fly/v2ray-core/v5/common/net"
	"github.com/v2fly/v2ray-core/v5/common/net/packetaddr"
	"github.com/v2fly/v2ray-core/v5/common/platform"
	"github.com/v2fly/v2ray-core/v5/common/protocol"
	"github.com/v2fly/v2ray-core/v5/common/retry"
	"github.com/v2fly/v2ray-core/v5/common/serial"
	"github.com/v2fly/v2ray-core/v5/common/session"
	"github.com/v2fly/v2ray-core/v5/common/signal"
	"github.com/v2fly/v2ray-core/v5/common/task"
	"github.com/v2fly/v2ray-core/v5/common/xudp"
	"github.com/v2fly/v2ray-core/v5/features/policy"
	"github.com/v2fly/v2ray-core/v5/features/stats"
	"github.com/v2fly/v2ray-core/v5/proxy/vless"
	"github.com/v2fly/v2ray-core/v5/proxy/vless/encoding"
	"github.com/v2fly/v2ray-core/v5/transport"
	"github.com/v2fly/v2ray-core/v5/transport/internet"
	"github.com/v2fly/v2ray-core/v5/transport/internet/xtls"
)

var xtls_show = false

func init() {
	common.Must(common.RegisterConfig((*Config)(nil), func(ctx context.Context, config interface{}) (interface{}, error) {
		return New(ctx, config.(*Config))
	}))

	xtlsShow := platform.NewEnvFlag("xray.vless.xtls.show").GetValue(func() string { return "NOT_DEFINED_AT_ALL" })
	if xtlsShow == "true" {
		xtls_show = true
	}

	common.Must(common.RegisterConfig((*SimplifiedConfig)(nil), func(ctx context.Context, config interface{}) (interface{}, error) {
		simplifiedClient := config.(*SimplifiedConfig)
		fullClient := &Config{
			Vnext: []*protocol.ServerEndpoint{
				{
					Address: simplifiedClient.Address,
					Port:    simplifiedClient.Port,
					User: []*protocol.User{
						{
							Account: serial.ToTypedMessage(&vless.Account{Id: simplifiedClient.Uuid, Encryption: "none"}),
						},
					},
				},
			},
			PacketEncoding: simplifiedClient.PacketEncoding,
		}
		return New(ctx, fullClient)
	}))
}

// Handler is an outbound connection handler for VLess protocol.
type Handler struct {
	serverList     *protocol.ServerList
	serverPicker   protocol.ServerPicker
	policyManager  policy.Manager
	packetEncoding packetaddr.PacketAddrType
}

// New creates a new VLess outbound handler.
func New(ctx context.Context, config *Config) (*Handler, error) {
	serverList := protocol.NewServerList()
	for _, rec := range config.Vnext {
		s, err := protocol.NewServerSpecFromPB(rec)
		if err != nil {
			return nil, newError("failed to parse server spec").Base(err).AtError()
		}
		serverList.AddServer(s)
	}

	v := core.MustFromContext(ctx)
	handler := &Handler{
		serverList:     serverList,
		serverPicker:   protocol.NewRoundRobinServerPicker(serverList),
		policyManager:  v.GetFeature(policy.ManagerType()).(policy.Manager),
		packetEncoding: config.PacketEncoding,
	}

	return handler, nil
}

// Process implements proxy.Outbound.Process().
func (h *Handler) Process(ctx context.Context, link *transport.Link, dialer internet.Dialer) error {
	var rec *protocol.ServerSpec
	var conn internet.Connection

	if err := retry.ExponentialBackoff(5, 200).On(func() error {
		rec = h.serverPicker.PickServer()
		var err error
		conn, err = dialer.Dial(ctx, rec.Destination())
		if err != nil {
			return err
		}
		return nil
	}); err != nil {
		return newError("failed to find an available destination").Base(err).AtWarning()
	}

	connElem := net.AddConnection(conn)
	defer net.RemoveConnection(connElem)

	iConn := conn
	statConn, ok := iConn.(*internet.StatCouterConnection)
	if ok {
		iConn = statConn.Connection
	}

	outbound := session.OutboundFromContext(ctx)
	if outbound == nil || !outbound.Target.IsValid() {
		return newError("target not specified").AtError()
	}

	target := outbound.Target
	newError("tunneling request to ", target, " via ", rec.Destination()).AtInfo().WriteToLog(session.ExportIDToError(ctx))

	command := protocol.RequestCommandTCP
	if target.Network == net.Network_UDP {
		command = protocol.RequestCommandUDP
	}
	if target.Address.Family().IsDomain() && target.Address.Domain() == "v1.mux.cool" {
		command = protocol.RequestCommandMux
	}

	request := &protocol.RequestHeader{
		Version: encoding.Version,
		User:    rec.PickUser(),
		Command: command,
		Address: target.Address,
		Port:    target.Port,
	}

	account := request.User.Account.(*vless.MemoryAccount)

	requestAddons := &encoding.Addons{
		Flow: account.Flow,
	}

	var rawConn syscall.RawConn
	var sctx context.Context

	allowUDP443 := false
	switch requestAddons.Flow {
	case vless.XRO + "-udp443", vless.XRD + "-udp443", vless.XRS + "-udp443":
		allowUDP443 = true
		requestAddons.Flow = requestAddons.Flow[:16]
		fallthrough
	case vless.XRO, vless.XRD, vless.XRS:
		switch request.Command {
		case protocol.RequestCommandMux:
			return newError(requestAddons.Flow + " doesn't support Mux").AtWarning()
		case protocol.RequestCommandUDP:
			if !allowUDP443 && request.Port == 443 {
				return newError(requestAddons.Flow + " stopped UDP/443").AtInfo()
			}
			requestAddons.Flow = ""
		case protocol.RequestCommandTCP:
			if xtlsConn, ok := iConn.(*xtls.Conn); ok {
				xtlsConn.RPRX = true
				xtlsConn.SHOW = xtls_show
				xtlsConn.MARK = "XTLS"
				if requestAddons.Flow == vless.XRS {
					sctx = ctx
					requestAddons.Flow = vless.XRD
				}
				if requestAddons.Flow == vless.XRD {
					xtlsConn.DirectMode = true
					if sc, ok := xtlsConn.Connection.(syscall.Conn); ok {
						rawConn, _ = sc.SyscallConn()
					}
				}
			} else {
				return newError(`failed to use ` + requestAddons.Flow + `, maybe "security" is not "xtls"`).AtWarning()
			}
		}
	default:
		if _, ok := iConn.(*xtls.Conn); ok {
			panic(`To avoid misunderstanding, you must fill in VLESS "flow" when using XTLS.`)
		}
	}

	sessionPolicy := h.policyManager.ForLevel(request.User.Level)
	ctx, cancel := context.WithCancel(ctx)
	timer := signal.CancelAfterInactivity(ctx, cancel, sessionPolicy.Timeouts.ConnectionIdle)

	clientReader := link.Reader // .(*pipe.Reader)
	clientWriter := link.Writer // .(*pipe.Writer)

	packetEncoding := packetaddr.PacketAddrType_None
	if command == protocol.RequestCommandUDP && target.Port > 0 && request.Port != 53 && request.Port != 443 {
		packetEncoding = h.packetEncoding
		switch packetEncoding {
		case packetaddr.PacketAddrType_Packet:
			request.Address = net.DomainAddress(packetaddr.SeqPacketMagicAddress)
			request.Port = 0
		case packetaddr.PacketAddrType_XUDP:
			request.Command = protocol.RequestCommandMux
			request.Address = net.DomainAddress("v1.mux.cool")
			request.Port = 0
		}
	}

	postRequest := func() error {
		defer timer.SetTimeout(sessionPolicy.Timeouts.DownlinkOnly)

		bufferWriter := buf.NewBufferedWriter(buf.NewWriter(conn))
		if err := encoding.EncodeRequestHeader(bufferWriter, request, requestAddons); err != nil {
			return newError("failed to encode request header").Base(err).AtWarning()
		}

		// default: serverWriter := bufferWriter
		serverWriter := encoding.EncodeBodyAddons(bufferWriter, request, requestAddons)
		switch packetEncoding {
		case packetaddr.PacketAddrType_Packet:
			serverWriter = packetaddr.NewPacketWriter(serverWriter, target)
		case packetaddr.PacketAddrType_XUDP:
			serverWriter = xudp.NewPacketWriter(serverWriter, target)
		}

		if err := buf.CopyOnceTimeout(clientReader, serverWriter, time.Millisecond*100); err != nil && err != buf.ErrNotTimeoutReader && err != buf.ErrReadTimeout {
			return err // ...
		}

		// Flush; bufferWriter.WriteMultiBufer now is bufferWriter.writer.WriteMultiBuffer
		if err := bufferWriter.SetBuffered(false); err != nil {
			return newError("failed to write A request payload").Base(err).AtWarning()
		}

		// from clientReader.ReadMultiBuffer to serverWriter.WriteMultiBufer
		if err := buf.Copy(clientReader, serverWriter, buf.UpdateActivity(timer)); err != nil {
			return newError("failed to transfer request payload").Base(err).AtInfo()
		}

		// Indicates the end of request payload.
		switch requestAddons.Flow {
		default:
		}
		return nil
	}

	getResponse := func() error {
		defer timer.SetTimeout(sessionPolicy.Timeouts.UplinkOnly)

		responseAddons, err := encoding.DecodeResponseHeader(conn, request)
		if err != nil {
			return newError("failed to decode response header").Base(err).AtInfo()
		}

		// default: serverReader := buf.NewReader(conn)
		serverReader := encoding.DecodeBodyAddons(conn, request, responseAddons)
		switch packetEncoding {
		case packetaddr.PacketAddrType_Packet:
			serverReader = packetaddr.NewPacketReader(serverReader)
		case packetaddr.PacketAddrType_XUDP:
			serverReader = xudp.NewPacketReader(&buf.BufferedReader{Reader: serverReader})
		}

		if rawConn != nil {
			var counter stats.Counter
			if statConn != nil {
				counter = statConn.ReadCounter
			}
			err = encoding.ReadV(serverReader, clientWriter, timer, iConn.(*xtls.Conn), rawConn, counter, sctx)
		} else {
			// from serverReader.ReadMultiBuffer to clientWriter.WriteMultiBufer
			err = buf.Copy(serverReader, clientWriter, buf.UpdateActivity(timer))
		}

		if err != nil {
			return newError("failed to transfer response payload").Base(err).AtInfo()
		}

		return nil
	}

	if err := task.Run(ctx, postRequest, task.OnSuccess(getResponse, task.Close(clientWriter))); err != nil {
		return newError("connection ends").Base(err).AtInfo()
	}

	return nil
}
