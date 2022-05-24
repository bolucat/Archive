package libcore

import "C"
import (
	"context"
	"fmt"
	"io"
	"math"
	"net"
	"os"
	"path/filepath"
	"sync"
	"sync/atomic"
	"time"

	"github.com/sirupsen/logrus"
	"github.com/v2fly/v2ray-core/v5"
	appOutbound "github.com/v2fly/v2ray-core/v5/app/proxyman/outbound"
	"github.com/v2fly/v2ray-core/v5/common/buf"
	v2rayNet "github.com/v2fly/v2ray-core/v5/common/net"
	"github.com/v2fly/v2ray-core/v5/common/net/pingproto"
	"github.com/v2fly/v2ray-core/v5/common/session"
	"github.com/v2fly/v2ray-core/v5/features/dns/localdns"
	"github.com/v2fly/v2ray-core/v5/features/outbound"
	routing_session "github.com/v2fly/v2ray-core/v5/features/routing/session"
	"github.com/v2fly/v2ray-core/v5/proxy/wireguard"
	"github.com/v2fly/v2ray-core/v5/transport/internet"
	"golang.org/x/sys/unix"
	"libcore/comm"
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

	defaultOutboundForPing outbound.Handler
}

type TunConfig struct {
	FileDescriptor      int32
	Protect             bool
	Protector           Protector
	MTU                 int32
	V2Ray               *V2RayInstance
	Gateway4            string
	Gateway6            string
	BindUpstream        Protector
	IPv6Mode            int32
	Implementation      int32
	Sniffing            bool
	OverrideDestination bool
	Debug               bool
	DumpUID             bool
	TrafficStats        bool
	PCap                bool
	ErrorHandler        ErrorHandler
}

type ErrorHandler interface {
	HandleError(err string)
}

func NewTun2ray(config *TunConfig) (*Tun2ray, error) {
	if config.Debug {
		logrus.SetLevel(logrus.DebugLevel)
	} else {
		logrus.SetLevel(logrus.WarnLevel)
	}
	t := &Tun2ray{
		router:              config.Gateway4,
		v2ray:               config.V2Ray,
		sniffing:            config.Sniffing,
		overrideDestination: config.OverrideDestination,
		debug:               config.Debug,
		dumpUid:             config.DumpUID,
		trafficStats:        config.TrafficStats,
	}

	var err error
	switch config.Implementation {
	case comm.TunImplementationGVisor:
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
	case comm.TunImplementationSystem:
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
		resolver: func(ctx context.Context, domain string) ([]net.IP, error) {
			ips, _, err := dc.LookupDefault(ctx, domain)
			return ips, err
		},
	})
	if config.BindUpstream != nil {
		pingproto.ControlFunc = func(fd uintptr) {
			config.BindUpstream.Protect(int32(fd))
		}
	} else {
		pingproto.ControlFunc = func(fd uintptr) {
			config.Protector.Protect(int32(fd))
			bindToUpstream(fd)
		}
	}
	if defaultOutbound, ok := t.v2ray.outboundManager.GetDefaultHandler().(*appOutbound.Handler); ok {
		if _, isWireGuard := defaultOutbound.GetOutbound().(*wireguard.Client); isWireGuard {
			t.defaultOutboundForPing = defaultOutbound
		}
	}

	internet.UseAlternativeSystemDNSDialer(&protectedDialer{
		protector: config.Protector,
		resolver: func(ctx context.Context, domain string) ([]net.IP, error) {
			ips, _, err := localdns.Client().LookupDefault(ctx, domain)
			return ips, err
		},
	})

	return t, nil
}

func (t *Tun2ray) Close() {
	pingproto.ControlFunc = nil
	internet.UseAlternativeSystemDialer(nil)
	internet.UseAlternativeSystemDNSDialer(nil)
	comm.CloseIgnore(t.dev)
}

func (t *Tun2ray) NewConnection(source v2rayNet.Destination, destination v2rayNet.Destination, conn net.Conn) {
	element := v2rayNet.AddConnection(conn)
	defer v2rayNet.RemoveConnection(element)

	inbound := &session.Inbound{
		Source:      source,
		Tag:         "tun",
		NetworkType: networkType,
		WifiSSID:    wifiSSID,
	}

	isDns := destination.Address.String() == t.router
	if isDns {
		inbound.Tag = "dns-in"
	}

	var uid uint16
	var self bool

	if t.dumpUid || t.trafficStats {
		u, err := dumpUid(source, destination)
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
		conn = NewStatsCounterConn(conn, &stats.uplink, &stats.downlink)
		stats.Lock()
		statsElement := stats.connections.PushBack(conn)
		stats.Unlock()
		defer func() {
			if atomic.AddInt32(&stats.tcpConn, -1)+atomic.LoadInt32(&stats.udpConn) == 0 {
				atomic.StoreInt64(&stats.deactivateAt, time.Now().Unix())
			}
			stats.Lock()
			stats.connections.Remove(statsElement)
			stats.Unlock()
		}()
	}
	inbound.Conn = conn

	_ = t.v2ray.dispatcher.DispatchConn(ctx, destination, conn, true)
}

func (t *Tun2ray) NewPacket(source v2rayNet.Destination, destination v2rayNet.Destination, data *buf.Buffer, writeBack func([]byte, *net.UDPAddr) (int, error), closer io.Closer) {
	natKey := source.NetAddr()

	sendTo := func() bool {
		iConn, ok := t.udpTable.Load(natKey)
		if !ok {
			return false
		}
		conn := iConn.(packetConn)
		err := conn.writeTo(data, &net.UDPAddr{
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
		comm.CloseIgnore(closer)
		return
	} else {
		iCond, loaded := t.lockTable.LoadOrStore(natKey, sync.NewCond(&sync.Mutex{}))
		cond = iCond.(*sync.Cond)
		if loaded {
			cond.L.Lock()
			cond.Wait()
			sendTo()
			cond.L.Unlock()

			comm.CloseIgnore(closer)
			return
		}
	}

	inbound := &session.Inbound{
		Source:      source,
		Tag:         "tun",
		NetworkType: networkType,
		WifiSSID:    wifiSSID,
	}
	isDns := destination.Address.String() == t.router

	if isDns {
		inbound.Tag = "dns-in"
	}

	var uid uint16
	var self bool

	if t.dumpUid || t.trafficStats {

		u, err := dumpUid(source, destination)
		if err == nil {
			if u > 19999 {
				logrus.Debug("bad connection owner ", u, ", reset to android.")
				u = 1000
			}

			uid = uint16(u)
			var info *UidInfo
			self = uid > 0 && int(uid) == os.Getuid()

			if t.debug && !self && uid >= 1000 {
				if err == nil {
					info, err = uidDumper.GetUidInfo(int32(uid))
					if err != nil {
						uid = 1000
						info, err = uidDumper.GetUidInfo(int32(uid))
					}
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

	ctx := core.WithContext(context.Background(), t.v2ray.core)
	ctx = session.ContextWithInbound(ctx, inbound)

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
	element := v2rayNet.AddConnection(conn)
	defer v2rayNet.RemoveConnection(element)

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
		conn = statsPacketConn{conn, &stats.uplink, &stats.downlink}
		stats.Lock()
		statsElement := stats.connections.PushBack(conn)
		stats.Unlock()
		defer func() {
			if atomic.AddInt32(&stats.udpConn, -1)+atomic.LoadInt32(&stats.tcpConn) == 0 {
				atomic.StoreInt64(&stats.deactivateAt, time.Now().Unix())
			}
			stats.Lock()
			stats.connections.Remove(statsElement)
			stats.Unlock()
		}()
	}

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
			_, err = writeBack(buffer.Bytes(), addr)
		} else {
			_, err = writeBack(buffer.Bytes(), nil)
		}
		buffer.Release()
		if err != nil {
			break
		}
	}
	// close
	comm.CloseIgnore(closer)
	t.udpTable.Delete(natKey)
}

func (t *Tun2ray) NewPingPacket(source v2rayNet.Destination, destination v2rayNet.Destination, message *buf.Buffer, writeBack func([]byte) error, closer io.Closer) bool {
	natKey := fmt.Sprint(source.Address, "-", destination.Address)

	sendTo := func() bool {
		iConn, ok := t.udpTable.Load(natKey)
		if !ok {
			return false
		}
		conn := iConn.(packetConn)
		err := conn.writeTo(message, &net.UDPAddr{
			IP:   destination.Address.IP(),
			Port: int(destination.Port),
		})
		if err != nil {
			_ = conn.Close()
			newError("failed to write ping request to ", destination.Address).Base(err).WriteToLog()
		}
		return true
	}

	var cond *sync.Cond

	if sendTo() {
		comm.CloseIgnore(closer)
		return true
	} else {
		iCond, loaded := t.lockTable.LoadOrStore(natKey, sync.NewCond(&sync.Mutex{}))
		cond = iCond.(*sync.Cond)
		if loaded {
			cond.L.Lock()
			cond.Wait()
			sendTo()
			cond.L.Unlock()

			comm.CloseIgnore(closer)
			return true
		}
	}

	defer func() {
		t.lockTable.Delete(natKey)
		cond.Broadcast()
	}()

	ctx := core.WithContext(context.Background(), t.v2ray.core)
	ctx = session.ContextWithInbound(ctx, &session.Inbound{
		Source:      source,
		Tag:         "tun",
		NetworkType: networkType,
		WifiSSID:    wifiSSID,
	})
	ctx = session.ContextWithOutbound(ctx, &session.Outbound{Target: destination})
	ctx = session.ContextWithContent(ctx, &session.Content{Protocol: "ping"})

	var handler outbound.Handler
	if route, err := t.v2ray.router.PickRoute(routing_session.AsRoutingContext(ctx)); err == nil {
		tag := route.GetOutboundTag()
		handler = t.v2ray.outboundManager.GetHandler(tag)
		if handler != nil {
			newError("taking detour [", tag, "] for [", destination.Address, "]").WriteToLog()
		} else {
			newError("non existing tag: ", tag).AtWarning().WriteToLog()
			return false
		}
	} else if t.defaultOutboundForPing != nil {
		handler = t.defaultOutboundForPing
		newError("default route for ", destination.Address).AtWarning().WriteToLog()

	} else {
		return false
	}

	conn := t.v2ray.handleUDP(ctx, handler, destination, time.Second*30)

	element := v2rayNet.AddConnection(conn)
	defer v2rayNet.RemoveConnection(element)

	t.udpTable.Store(natKey, conn)

	go sendTo()

	go func() {
		for {
			buffer, _, err := conn.readFrom()
			if err != nil {
				newError("failed to read ping response from ", destination.Address).Base(err).WriteToLog()
				break
			}
			err = writeBack(buffer.Bytes())
			buffer.Release()
			if err != nil {
				if err != unix.ENETUNREACH {
					newError("failed to write ping response back").Base(err).WriteToLog()
				}
				break
			}
		}
		// close
		comm.CloseIgnore(closer)
		t.udpTable.Delete(natKey)
	}()

	return true
}
