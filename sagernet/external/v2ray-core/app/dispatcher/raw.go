package dispatcher

import (
	"context"
	"time"

	B "github.com/sagernet/sing/common/buf"
	"github.com/sagernet/sing/common/bufio"
	E "github.com/sagernet/sing/common/exceptions"
	"github.com/v2fly/v2ray-core/v5/common"
	"github.com/v2fly/v2ray-core/v5/common/buf"
	"github.com/v2fly/v2ray-core/v5/common/log"
	"github.com/v2fly/v2ray-core/v5/common/net"
	"github.com/v2fly/v2ray-core/v5/common/session"
	"github.com/v2fly/v2ray-core/v5/features/outbound"
	routing_session "github.com/v2fly/v2ray-core/v5/features/routing/session"
	"github.com/v2fly/v2ray-core/v5/transport"
)

func (d *DefaultDispatcher) DispatchConn(ctx context.Context, destination net.Destination, conn net.Conn, wait bool) error {
	if !destination.IsValid() {
		return newError("Dispatcher: Invalid destination.")
	}
	newError("dispatch conn to ", destination).AtDebug().WriteToLog()
	ob := &session.Outbound{
		Target: destination,
	}
	ctx = session.ContextWithOutbound(ctx, ob)
	content := session.ContentFromContext(ctx)
	if content == nil {
		content = new(session.Content)
		ctx = session.ContextWithContent(ctx, content)
	}
	sniffingRequest := content.SniffingRequest

	if content.Protocol != "" || !sniffingRequest.Enabled && destination.Network != net.Network_UDP {
		d.routedDispatchConn(ctx, conn, destination, wait)
		return nil
	}
	sniffer := defaultSniffers
	if !sniffingRequest.Enabled {
		sniffer = udpOnlyDnsSniffers
	}

	err := conn.SetReadDeadline(time.Now().Add(100 * time.Millisecond))
	if err != nil {
		d.routedDispatchConn(ctx, conn, destination, wait)
		return nil
	}

	var header *B.Buffer

	/*if wait {
		_header := B.StackNew()
		header = C.Dup(_header)
	} else {*/
	header = B.New()
	//}

	_, err = header.ReadFrom(conn)
	if err != nil && !E.IsTimeout(err) {
		header.Release()
		return err
	}

	err = conn.SetReadDeadline(time.Time{})
	if err != nil {
		header.Release()
		return err
	}

	if header.IsEmpty() {
		header.Release()
		d.routedDispatchConn(ctx, conn, destination, wait)
		return nil
	}

	conn = bufio.NewCachedConn(conn, header)

	result, err := sniffer.Sniff(ctx, header.Bytes(), net.Network_TCP)
	if err != nil {
		d.routedDispatchConn(ctx, conn, destination, wait)
		return nil
	}

	content.Protocol = result.Protocol()
	if shouldOverride(result, sniffingRequest.OverrideDestinationForProtocol) {
		domain := result.Domain()
		newError("sniffed domain: ", domain).WriteToLog(session.ExportIDToError(ctx))
		destination.Address = net.ParseAddress(domain)
		if sniffingRequest.RouteOnly {
			ob.RouteTarget = destination
		} else {
			ob.Target = destination
		}
	}

	d.routedDispatchConn(ctx, conn, destination, wait)
	return nil
}

func (d *DefaultDispatcher) routedDispatchConn(ctx context.Context, conn net.Conn, destination net.Destination, wait bool) {
	if wait {
		d.routedDispatchConn0(ctx, conn, destination)
	} else {
		go d.routedDispatchConn0(ctx, conn, destination)
	}
}

func (d *DefaultDispatcher) routedDispatchConn0(ctx context.Context, conn net.Conn, destination net.Destination) {
	var handler outbound.Handler

	if forcedOutboundTag := session.GetForcedOutboundTagFromContext(ctx); forcedOutboundTag != "" {
		ctx = session.SetForcedOutboundTagToContext(ctx, "")
		if h := d.ohm.GetHandler(forcedOutboundTag); h != nil {
			newError("taking platform initialized detour [", forcedOutboundTag, "] for [", destination, "]").WriteToLog(session.ExportIDToError(ctx))
			handler = h
		} else {
			newError("non existing tag for platform initialized detour: ", forcedOutboundTag).AtError().WriteToLog(session.ExportIDToError(ctx))
			common.Close(conn)
			return
		}
	} else if d.router != nil {
		if route, err := d.router.PickRoute(routing_session.AsRoutingContext(ctx)); err == nil {
			tag := route.GetOutboundTag()
			if h := d.ohm.GetHandler(tag); h != nil {
				newError("taking detour [", tag, "] for [", destination, "]").WriteToLog(session.ExportIDToError(ctx))
				handler = h
			} else {
				newError("non existing tag: ", tag).AtWarning().WriteToLog(session.ExportIDToError(ctx))
			}
		} else {
			newError("default route for ", destination).AtWarning().WriteToLog(session.ExportIDToError(ctx))
		}
	}

	if handler == nil {
		handler = d.ohm.GetDefaultHandler()
	}

	if handler == nil {
		newError("default outbound handler not exist").WriteToLog(session.ExportIDToError(ctx))
		common.Close(conn)
		return
	}

	if accessMessage := log.AccessMessageFromContext(ctx); accessMessage != nil {
		if tag := handler.Tag(); tag != "" {
			accessMessage.Detour = tag
		}
		log.Record(accessMessage)
	}

	if connHandler, ok := handler.(outbound.ConnHandler); ok && connHandler.IsConnDispatcher() {
		connHandler.DispatchConn(ctx, conn)
		return
	}

	handler.Dispatch(ctx, &transport.Link{
		Reader: buf.NewReader(conn),
		Writer: buf.NewWriter(conn),
	})
}
