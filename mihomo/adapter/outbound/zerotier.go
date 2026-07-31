//go:build with_gvisor && !no_zerotier

package outbound

import (
	"context"
	"crypto/sha256"
	"errors"
	"fmt"
	"io/fs"
	"net"
	"net/netip"
	"os"
	"path/filepath"
	"sync"

	"github.com/metacubex/mihomo/component/dialer"
	"github.com/metacubex/mihomo/component/iface"
	"github.com/metacubex/mihomo/component/resolver"
	C "github.com/metacubex/mihomo/constant"
	"github.com/metacubex/mihomo/dns"
	"github.com/metacubex/mihomo/log"
	wireguard "github.com/metacubex/sing-wireguard"
	M "github.com/metacubex/sing/common/metadata"
	ZT "github.com/metacubex/zerotier-go"
	ZTIP "github.com/metacubex/zerotier-go/iplink"
	ZTTransport "github.com/metacubex/zerotier-go/transport"
)

const (
	zeroTierDefaultStateDir = "zerotier"
	zeroTierFrameQueueSize  = 256
)

var errZeroTierClosed = errors.New("ZeroTier outbound closed")

var errZeroTierStaleConfig = errors.New("stale ZeroTier network configuration")

type ZeroTier struct {
	*Base
	option            ZeroTierOption
	networkID         uint64
	planet            *ZT.World
	orbits            []zeroTierOrbit
	remoteTraceTarget ZT.Address
	stateStore        ZT.StateStore
	ctx               context.Context
	cancel            context.CancelFunc

	lifecycleMu        sync.Mutex
	closed             bool
	backgroundStarted  bool
	identityRecovering bool
	configMu           sync.Mutex

	node       *ZT.Node
	nodeCancel context.CancelFunc
	ipLink     *ZTIP.Link
	wire       *ZTTransport.Transport
	frameCh    chan zeroTierInboundFrame
	configCh   chan struct{}

	linkMu            sync.RWMutex
	config            ZT.NetworkConfigData
	tunDevice         wireguard.Device
	resolver          resolver.Resolver
	dns               []dns.NameServer
	networkErr        error
	stateCh           chan struct{}
	latestConfig      ZT.NetworkConfigData
	haveLatestConfig  bool
	retryLatestConfig bool
	networkRetrying   bool
	authURL           string
	configGeneration  uint64
}

type ZeroTierOption struct {
	BasicOption
	Name              string                `proxy:"name"`
	Network           string                `proxy:"network"`
	StateDir          string                `proxy:"state-dir,omitempty"`
	Planet            string                `proxy:"planet,omitempty"`
	MTU               int                   `proxy:"mtu,omitempty"`
	PhysicalMTU       int                   `proxy:"physical-mtu,omitempty"`
	UDP               bool                  `proxy:"udp,omitempty"`
	RemoteDnsResolve  bool                  `proxy:"remote-dns-resolve,omitempty"`
	Dns               []string              `proxy:"dns,omitempty"`
	LowBandwidth      bool                  `proxy:"low-bandwidth,omitempty"`
	EncryptedHello    bool                  `proxy:"encrypted-hello,omitempty"`
	PrimaryPort       int                   `proxy:"primary-port,omitempty"`
	SecondaryPort     int                   `proxy:"secondary-port,omitempty"`
	TCPFallbackMode   string                `proxy:"tcp-fallback-mode,omitempty"`
	TCPFallbackRelay  string                `proxy:"tcp-fallback-relay,omitempty"`
	Orbit             []ZeroTierOrbitOption `proxy:"orbit,omitempty"`
	RemoteTraceTarget string                `proxy:"remote-trace-target,omitempty"`
	RemoteTraceLevel  uint64                `proxy:"remote-trace-level,omitempty"`
}

type ZeroTierOrbitOption struct {
	World string `proxy:"world"`
	Seed  string `proxy:"seed"`
}

type zeroTierOrbit struct {
	world uint64
	seed  ZT.Address
}

type zeroTierInboundFrame struct {
	node  *ZT.Node
	frame ZT.Frame
}

type zeroTierStateFS struct {
	directory string
	outbound  string
}

type zeroTierPacketConn struct {
	net.PacketConn
	validateDestination func(netip.Addr) error
}

func (c *zeroTierPacketConn) WriteTo(packet []byte, destination net.Addr) (int, error) {
	address := M.SocksaddrFromNet(destination).Unwrap()
	if !address.IsIP() {
		return 0, fmt.Errorf("invalid ZeroTier UDP destination %v", destination)
	}
	if err := c.validateDestination(address.Addr); err != nil {
		return 0, err
	}
	return c.PacketConn.WriteTo(packet, destination)
}

func (f zeroTierStateFS) Open(name string) (fs.File, error) {
	return os.Open(filepath.Join(f.directory, filepath.FromSlash(name)))
}

func (f zeroTierStateFS) WriteFile(name string, data []byte, perm fs.FileMode) error {
	path := filepath.Join(f.directory, filepath.FromSlash(name))
	err := os.MkdirAll(filepath.Dir(path), 0700)
	if err == nil {
		err = os.WriteFile(path, data, perm)
	}
	if err != nil {
		log.Warnln("[ZeroTier](%s) unable to write state object %s: %v", f.outbound, name, err)
	}
	return err
}

func (f zeroTierStateFS) Remove(name string) error {
	return os.Remove(filepath.Join(f.directory, filepath.FromSlash(name)))
}

func NewZeroTier(option ZeroTierOption) (*ZeroTier, error) {
	networkID, err := ZT.ParseNetworkID(option.Network)
	if err != nil {
		return nil, err
	}
	if option.MTU != 0 && (option.MTU < ZT.MinNetworkMTU || option.MTU > ZT.MaxNetworkMTU) {
		return nil, fmt.Errorf("ZeroTier MTU must be between %d and %d", ZT.MinNetworkMTU, ZT.MaxNetworkMTU)
	}
	if option.PhysicalMTU != 0 && (option.PhysicalMTU < ZT.MinPhysicalMTU || option.PhysicalMTU > ZT.MaxPhysicalMTU) {
		return nil, fmt.Errorf("ZeroTier physical MTU must be between %d and %d", ZT.MinPhysicalMTU, ZT.MaxPhysicalMTU)
	}
	var planet *ZT.World
	if option.Planet != "" {
		option.Planet = C.Path.Resolve(option.Planet)
		if !C.Path.IsSafePath(option.Planet) {
			return nil, C.Path.ErrNotSafePath(option.Planet)
		}
		data, readErr := os.ReadFile(option.Planet)
		if readErr != nil {
			return nil, fmt.Errorf("read ZeroTier planet: %w", readErr)
		}
		world, parseErr := ZT.ParsePlanet(data)
		if parseErr != nil {
			return nil, fmt.Errorf("parse ZeroTier planet: %w", parseErr)
		}
		planet = &world
	}
	orbits := make([]zeroTierOrbit, 0, len(option.Orbit))
	seenWorlds := make(map[uint64]struct{}, len(option.Orbit))
	for _, orbit := range option.Orbit {
		world, seed, err := ZT.ParseOrbit(orbit.World, orbit.Seed)
		if err != nil {
			return nil, err
		}
		if _, exists := seenWorlds[world]; exists {
			return nil, fmt.Errorf("duplicate ZeroTier orbit world %016x", world)
		}
		seenWorlds[world] = struct{}{}
		orbits = append(orbits, zeroTierOrbit{world: world, seed: seed})
	}
	tcpFallbackMode, err := ZTTransport.ParseTCPFallbackMode(option.TCPFallbackMode)
	if err != nil {
		return nil, err
	}
	option.TCPFallbackMode = tcpFallbackMode.String()
	if option.TCPFallbackRelay == "" {
		option.TCPFallbackRelay = ZTTransport.DefaultTCPFallbackRelay
	}
	var remoteTraceTarget ZT.Address
	if option.RemoteTraceTarget != "" {
		remoteTraceTarget, err = ZT.ParseAddress(option.RemoteTraceTarget)
		if err != nil || remoteTraceTarget.IsReserved() {
			return nil, errors.New("ZeroTier remote trace target must be a 10-digit node ID")
		}
	}
	if option.RemoteTraceLevel > ZT.TraceLevelInsane {
		return nil, fmt.Errorf("ZeroTier remote trace level must be between %d and %d", ZT.TraceLevelNormal, ZT.TraceLevelInsane)
	}
	var nameServers []dns.NameServer
	if option.RemoteDnsResolve && len(option.Dns) > 0 {
		nameServers, err = dns.ParseNameServer(option.Dns)
		if err != nil {
			return nil, err
		}
	}
	if option.StateDir == "" {
		instance := sha256.Sum256([]byte(option.Name))
		option.StateDir = filepath.Join(zeroTierDefaultStateDir, fmt.Sprintf("%s-%x", option.Network, instance[:6]))
	}
	option.StateDir = C.Path.Resolve(option.StateDir)
	if !C.Path.IsSafePath(option.StateDir) {
		return nil, C.Path.ErrNotSafePath(option.StateDir)
	}
	ctx, cancel := context.WithCancel(context.Background())
	stateStore, err := ZT.NewFileStore(&zeroTierStateFS{directory: option.StateDir, outbound: option.Name})
	if err != nil {
		cancel()
		return nil, err
	}
	outbound := &ZeroTier{
		Base: NewBase(BaseOption{
			Name:         option.Name,
			Addr:         option.Network,
			Type:         C.ZeroTier,
			ProviderName: option.ProviderName,
			UDP:          option.UDP,
			Interface:    option.Interface,
			RoutingMark:  option.RoutingMark,
			Prefer:       option.IPVersion,
		}),
		option:            option,
		networkID:         networkID,
		planet:            planet,
		orbits:            orbits,
		remoteTraceTarget: remoteTraceTarget,
		stateStore:        stateStore,
		dns:               nameServers,
		ctx:               ctx,
		cancel:            cancel,
		frameCh:           make(chan zeroTierInboundFrame, zeroTierFrameQueueSize),
		configCh:          make(chan struct{}, 1),
		stateCh:           make(chan struct{}),
	}
	outbound.dialer = option.NewDialer(outbound.DialOptions())
	wireConfig, err := outbound.wireTransportConfig()
	if err == nil {
		err = ZTTransport.ValidateConfig(wireConfig)
	}
	if err != nil {
		cancel()
		return nil, err
	}
	return outbound, nil
}

func (z *ZeroTier) wireTransportConfig() (ZTTransport.Config, error) {
	fallbackMode, err := ZTTransport.ParseTCPFallbackMode(z.option.TCPFallbackMode)
	if err != nil {
		return ZTTransport.Config{}, err
	}
	return ZTTransport.Config{
		Dialer:           z.dialer,
		Interfaces:       zeroTierTransportInterfaces,
		InterfaceName:    z.option.Interface,
		SharedUDP:        z.option.DialerProxy == "",
		PrimaryPort:      z.option.PrimaryPort,
		SecondaryPort:    z.option.SecondaryPort,
		TCPFallbackMode:  fallbackMode,
		TCPFallbackRelay: z.option.TCPFallbackRelay,
		Log: func(level ZTTransport.LogLevel, format string, arguments ...interface{}) {
			message := fmt.Sprintf(format, arguments...)
			if level == ZTTransport.LogInfo {
				log.Infoln("[ZeroTier](%s) %s", z.Name(), message)
			} else {
				log.Debugln("[ZeroTier](%s) %s", z.Name(), message)
			}
		},
	}, nil
}

func zeroTierTransportInterfaces() ([]ZTTransport.Interface, error) {
	interfaces, err := iface.Interfaces()
	if err != nil {
		return nil, err
	}
	result := make([]ZTTransport.Interface, 0, len(interfaces))
	for _, networkInterface := range interfaces {
		result = append(result, ZTTransport.Interface{
			Name:      networkInterface.Name,
			Flags:     networkInterface.Flags,
			Addresses: networkInterface.Addresses,
		})
	}
	return result, nil
}

func (z *ZeroTier) detachStackLocked() wireguard.Device {
	device := z.tunDevice
	z.tunDevice = nil
	z.resolver = nil
	return device
}

func (z *ZeroTier) resetNetworkStateLocked(networkErr error) {
	z.config = ZT.NetworkConfigData{}
	z.latestConfig = ZT.NetworkConfigData{}
	z.haveLatestConfig = false
	z.retryLatestConfig = false
	z.networkErr = networkErr
	z.authURL = ""
	z.configGeneration++
	z.notifyStateLocked()
}

func (z *ZeroTier) setNetworkFailureLocked(err error, authURL string, retryConfig bool) {
	z.networkErr = err
	z.authURL = authURL
	z.retryLatestConfig = retryConfig && z.haveLatestConfig
	z.notifyStateLocked()
}

func (z *ZeroTier) detachRuntimeLocked() (node *ZT.Node, nodeCancel context.CancelFunc, wireTransport *ZTTransport.Transport, device wireguard.Device) {
	node = z.node
	nodeCancel = z.nodeCancel
	wireTransport = z.wire
	device = z.detachStackLocked()
	z.node = nil
	z.nodeCancel = nil
	z.ipLink = nil
	z.wire = nil
	return
}

func closeZeroTierRuntime(node *ZT.Node, nodeCancel context.CancelFunc, wireTransport *ZTTransport.Transport, device wireguard.Device) error {
	if nodeCancel != nil {
		nodeCancel()
	}
	if wireTransport != nil {
		_ = wireTransport.Close()
	}
	if node != nil {
		_ = node.Close()
	}
	if device != nil {
		return device.Close()
	}
	return nil
}

func (z *ZeroTier) start() error {
	z.lifecycleMu.Lock()
	defer z.lifecycleMu.Unlock()
	if z.closed {
		return errZeroTierClosed
	}
	z.linkMu.RLock()
	started := z.node != nil
	z.linkMu.RUnlock()
	if started {
		return nil
	}
	if err := os.MkdirAll(z.option.StateDir, 0700); err != nil {
		return err
	}
	wireConfig, err := z.wireTransportConfig()
	if err != nil {
		return err
	}
	wireTransport, err := ZTTransport.New(wireConfig)
	if err != nil {
		return err
	}
	var node *ZT.Node
	node, err = ZT.NewNode(ZT.NodeConfig{
		Store:  z.stateStore,
		Sender: wireTransport,
		Planet: z.planet,
		OnEvent: func(event ZT.Event) {
			z.handleNodeEvent(node, event)
		},
		PhysicalMTU:       z.option.PhysicalMTU,
		RemoteTraceTarget: z.remoteTraceTarget,
		RemoteTraceLevel:  z.option.RemoteTraceLevel,
		LowBandwidth:      z.option.LowBandwidth,
		EncryptedHello:    z.option.EncryptedHello,
		OnNetworkConfig: func(config ZT.NetworkConfigData) {
			z.enqueueNetworkConfig(node, config)
		},
		OnFrame: func(frame ZT.Frame) {
			select {
			case z.frameCh <- zeroTierInboundFrame{node: node, frame: frame}:
			default:
				log.Warnln("[ZeroTier](%s) dropping inbound frame because the bridge queue is full", z.Name())
			}
		},
		DirectPaths: wireTransport.DirectPaths,
	})
	if err != nil {
		_ = wireTransport.Close()
		return err
	}
	ipLink, err := ZTIP.New(z.networkID, node)
	if err != nil {
		_ = wireTransport.Close()
		_ = node.Close()
		return err
	}
	nodeCtx, nodeCancel := context.WithCancel(z.ctx)
	z.linkMu.Lock()
	z.node = node
	z.nodeCancel = nodeCancel
	z.ipLink = ipLink
	z.wire = wireTransport
	z.resetNetworkStateLocked(nil)
	z.linkMu.Unlock()
	cleanup := func(startErr error) error {
		nodeCancel()
		z.linkMu.Lock()
		if z.node == node {
			z.node = nil
			z.nodeCancel = nil
			z.ipLink = nil
			z.wire = nil
		}
		z.linkMu.Unlock()
		_ = closeZeroTierRuntime(node, nil, wireTransport, nil)
		return startErr
	}
	if err = wireTransport.Start(nodeCtx, node); err != nil {
		return cleanup(err)
	}
	for _, orbit := range z.orbits {
		if err = node.Orbit(orbit.world, orbit.seed); err != nil {
			return cleanup(fmt.Errorf("orbit ZeroTier world %016x: %w", orbit.world, err))
		}
	}
	if err = node.Join(z.networkID); err != nil {
		return cleanup(err)
	}
	if !z.backgroundStarted {
		z.backgroundStarted = true
		go z.runNetworkConfig()
		go z.runInboundFrames()
	}
	go node.RunBackgroundTasks(nodeCtx)
	go ipLink.RunBackgroundTasks(nodeCtx)
	return nil
}

func (z *ZeroTier) enqueueNetworkConfig(source *ZT.Node, config ZT.NetworkConfigData) {
	z.linkMu.Lock()
	if z.ctx.Err() != nil || source == nil || z.node != source {
		z.linkMu.Unlock()
		return
	}
	network, ok := source.Network(z.networkID)
	if !ok || network.Status != ZT.NetworkStatusOK || !network.Config.Equal(config) {
		z.linkMu.Unlock()
		return
	}
	z.latestConfig = config
	z.haveLatestConfig = true
	z.linkMu.Unlock()
	select {
	case z.configCh <- struct{}{}:
	default:
	}
}

func (z *ZeroTier) ensureStarted(ctx context.Context) error {
	if err := z.start(); err != nil {
		return err
	}
	z.retryNetwork()
	for {
		z.linkMu.RLock()
		networkErr := z.networkErr
		device := z.tunDevice
		stateCh := z.stateCh
		z.linkMu.RUnlock()
		if device != nil && networkErr == nil {
			return nil
		}
		if networkErr != nil {
			return networkErr
		}
		select {
		case <-stateCh:
		case <-ctx.Done():
			return ctx.Err()
		case <-z.ctx.Done():
			return errZeroTierClosed
		}
	}
}

func (z *ZeroTier) notifyStateLocked() {
	close(z.stateCh)
	z.stateCh = make(chan struct{})
}

func (z *ZeroTier) runNetworkConfig() {
	for {
		select {
		case <-z.configCh:
			z.linkMu.RLock()
			config := z.latestConfig
			haveConfig := z.haveLatestConfig
			generation := z.configGeneration
			z.linkMu.RUnlock()
			if !haveConfig {
				continue
			}
			if err := z.applyNetworkConfig(config, generation); err != nil && !errors.Is(err, errZeroTierStaleConfig) {
				log.Errorln("[ZeroTier](%s) apply network configuration: %v", z.Name(), err)
				z.recordConfigFailure(generation, err)
			}
		case <-z.ctx.Done():
			return
		}
	}
}

func (z *ZeroTier) handleNodeEvent(source *ZT.Node, event ZT.Event) {
	if z.ctx.Err() != nil {
		return
	}
	// EventNodeUp is emitted synchronously by NewNode before it can be assigned
	// to source. All later events must belong to the currently active node.
	if source != nil {
		z.linkMu.RLock()
		current := z.node == source
		z.linkMu.RUnlock()
		if !current {
			return
		}
	}
	switch event.Type {
	case ZT.EventNodeUp:
		log.Infoln("[ZeroTier](%s) node %s started; authorize this ID on network %016x", z.Name(), event.Address, z.networkID)
	case ZT.EventOnline:
		log.Infoln("[ZeroTier](%s) node %s is online via %s", z.Name(), event.Address, event.Endpoint)
	case ZT.EventOffline:
		log.Warnln("[ZeroTier](%s) node %s is offline", z.Name(), event.Address)
	case ZT.EventNetworkReady:
		log.Infoln("[ZeroTier](%s) network %016x configuration is ready", z.Name(), event.NetworkID)
	case ZT.EventNetworkAccessDenied:
		z.invalidateNetwork(errors.New("ZeroTier network access denied"), "", false)
	case ZT.EventNetworkNotFound:
		z.invalidateNetwork(errors.New("ZeroTier network not found or controller unsupported"), "", false)
	case ZT.EventNetworkAuthenticationRequired:
		authURL, err := event.Authentication.LoginURL()
		if err != nil {
			z.invalidateNetwork(fmt.Errorf("ZeroTier network authentication required: %w", err), "", false)
			return
		}
		z.linkMu.RLock()
		changed := z.authURL != authURL
		z.linkMu.RUnlock()
		if changed {
			log.Infoln("[ZeroTier](%s) network authentication required; complete login at %s", z.Name(), authURL)
		}
		z.invalidateNetwork(nil, authURL, false)
	case ZT.EventFatalIdentityCollision:
		go z.recoverIdentityCollision(event.Address)
	}
}

func (z *ZeroTier) recoverIdentityCollision(address ZT.Address) {
	z.lifecycleMu.Lock()
	if z.closed || z.identityRecovering {
		z.lifecycleMu.Unlock()
		return
	}
	z.linkMu.Lock()
	node := z.node
	if node == nil || node.Address() != address {
		z.linkMu.Unlock()
		z.lifecycleMu.Unlock()
		return
	}
	z.identityRecovering = true
	node, nodeCancel, wireTransport, device := z.detachRuntimeLocked()
	z.resetNetworkStateLocked(nil)
	z.linkMu.Unlock()

	_ = closeZeroTierRuntime(node, nodeCancel, wireTransport, device)
	if err := ZT.RotateIdentityState(z.stateStore); err != nil {
		log.Warnln("[ZeroTier](%s) unable to rotate collided identity: %v", z.Name(), err)
	}
	z.lifecycleMu.Unlock()

	log.Warnln("[ZeroTier](%s) node %s has an identity collision; generating a new identity", z.Name(), address)
	startErr := z.start()
	z.lifecycleMu.Lock()
	z.identityRecovering = false
	z.lifecycleMu.Unlock()
	if startErr != nil && !errors.Is(startErr, errZeroTierClosed) {
		log.Errorln("[ZeroTier](%s) restart after identity collision: %v", z.Name(), startErr)
		z.invalidateNetwork(startErr, "", false)
	}
}

func (z *ZeroTier) retryNetwork() bool {
	z.linkMu.Lock()
	if z.ctx.Err() != nil {
		z.linkMu.Unlock()
		return false
	}
	if z.tunDevice != nil && z.networkErr == nil {
		z.linkMu.Unlock()
		return false
	}
	if z.networkRetrying {
		z.linkMu.Unlock()
		return false
	}
	z.networkRetrying = true
	z.networkErr = nil
	retryConfig := z.retryLatestConfig && z.haveLatestConfig
	config := z.latestConfig
	node := z.node
	generation := z.configGeneration
	z.linkMu.Unlock()
	defer func() {
		z.linkMu.Lock()
		z.networkRetrying = false
		z.linkMu.Unlock()
	}()
	if retryConfig {
		if err := z.applyNetworkConfig(config, generation); err == nil {
			return false
		} else if !errors.Is(err, errZeroTierStaleConfig) {
			z.recordConfigFailure(generation, err)
		}
	}
	if node == nil {
		return false
	}
	if err := node.RefreshNetwork(z.networkID); err != nil {
		if retryConfig {
			log.Debugln("[ZeroTier](%s) refresh network configuration after local apply failure: %v", z.Name(), err)
		} else {
			log.Debugln("[ZeroTier](%s) refresh network configuration: %v", z.Name(), err)
		}
		z.linkMu.Lock()
		if z.ctx.Err() == nil && z.node == node && z.configGeneration == generation {
			z.setNetworkFailureLocked(fmt.Errorf("refresh ZeroTier network configuration: %w", err), z.authURL, z.retryLatestConfig)
		}
		z.linkMu.Unlock()
	}
	return true
}

func (z *ZeroTier) recordConfigFailure(generation uint64, err error) {
	z.linkMu.Lock()
	defer z.linkMu.Unlock()
	if z.ctx.Err() != nil || z.configGeneration != generation {
		return
	}
	z.setNetworkFailureLocked(err, z.authURL, true)
}

func (z *ZeroTier) invalidateNetwork(err error, authURL string, retryConfig bool) {
	z.linkMu.Lock()
	if z.ctx.Err() != nil {
		z.linkMu.Unlock()
		return
	}
	device := z.detachStackLocked()
	z.configGeneration++
	z.setNetworkFailureLocked(err, authURL, retryConfig)
	z.linkMu.Unlock()
	if device != nil {
		go func() { _ = device.Close() }()
	}
}

func (z *ZeroTier) invalidateDevice(device wireguard.Device, err error) {
	z.linkMu.Lock()
	if z.ctx.Err() != nil || z.tunDevice != device {
		z.linkMu.Unlock()
		return
	}
	z.detachStackLocked()
	z.setNetworkFailureLocked(err, z.authURL, true)
	z.linkMu.Unlock()
	go func() { _ = device.Close() }()
}

func (z *ZeroTier) applyNetworkConfig(config ZT.NetworkConfigData, generation uint64) error {
	z.configMu.Lock()
	defer z.configMu.Unlock()
	if z.ctx.Err() != nil {
		return errZeroTierClosed
	}
	z.linkMu.RLock()
	if !z.networkConfigCurrentLocked(config, generation) {
		z.linkMu.RUnlock()
		return errZeroTierStaleConfig
	}
	oldDevice := z.tunDevice
	ipLink := z.ipLink
	oldConfig := z.config
	z.linkMu.RUnlock()
	if ipLink == nil {
		return errors.New("ZeroTier core is not started")
	}
	if len(config.Assigned) == 0 {
		return errors.New("ZeroTier controller assigned no managed addresses")
	}
	remoteResolver, err := z.resolverForNetworkConfig(config)
	if err != nil {
		return err
	}
	mtu := z.effectiveMTU(config)
	replaceDevice := oldDevice == nil || !oldConfig.ManagedAddressesEqual(config) || z.effectiveMTU(oldConfig) != mtu
	device := oldDevice
	if replaceDevice {
		device, err = wireguard.NewStackDevice(config.Assigned, mtu)
		if err != nil {
			return fmt.Errorf("create ZeroTier stack device: %w", err)
		}
		if err = device.Start(); err != nil {
			_ = device.Close()
			return err
		}
		if z.ctx.Err() != nil {
			_ = device.Close()
			return errZeroTierClosed
		}
	}
	z.linkMu.Lock()
	if z.ipLink != ipLink || !z.networkConfigCurrentLocked(config, generation) || (!replaceDevice && z.tunDevice != oldDevice) {
		z.linkMu.Unlock()
		if replaceDevice {
			_ = device.Close()
		}
		return errZeroTierStaleConfig
	}
	if err = ipLink.ApplyNetworkConfig(config); err != nil {
		z.linkMu.Unlock()
		if replaceDevice {
			_ = device.Close()
		}
		return err
	}
	if replaceDevice {
		if resetErr := ipLink.ResetMulticast(); resetErr != nil {
			log.Debugln("[ZeroTier](%s) reset multicast subscriptions: %v", z.Name(), resetErr)
		}
	}
	replacedDevice := z.tunDevice
	z.config = config
	z.tunDevice = device
	z.resolver = remoteResolver
	z.networkErr = nil
	z.authURL = ""
	z.retryLatestConfig = false
	z.configGeneration++
	z.notifyStateLocked()
	z.linkMu.Unlock()
	if !replaceDevice {
		return nil
	}
	go z.runStackPackets(device, ipLink)
	if replacedDevice != nil {
		_ = replacedDevice.Close()
	}
	log.Infoln("[ZeroTier](%s) joined %016x (%s), addresses=%v routes=%v mtu=%d", z.Name(), config.NetworkID, config.Name, config.Assigned, config.Routes, mtu)
	return nil
}

func (z *ZeroTier) networkConfigCurrentLocked(config ZT.NetworkConfigData, generation uint64) bool {
	if z.configGeneration != generation {
		return false
	}
	node := z.node
	if node == nil {
		return false
	}
	network, ok := node.Network(z.networkID)
	return ok && network.Status == ZT.NetworkStatusOK && network.Config.Equal(config)
}

func (z *ZeroTier) effectiveMTU(config ZT.NetworkConfigData) uint32 {
	mtu := config.MTU
	if z.option.MTU != 0 && uint32(z.option.MTU) < mtu {
		mtu = uint32(z.option.MTU)
	}
	return mtu
}

func (z *ZeroTier) resolverForNetworkConfig(config ZT.NetworkConfigData) (resolver.Resolver, error) {
	if !z.option.RemoteDnsResolve {
		return nil, nil
	}
	nameServers := z.dns
	if len(nameServers) == 0 && len(config.DNSServers) != 0 {
		servers := make([]string, 0, len(config.DNSServers))
		for _, server := range config.DNSServers {
			if server.Port() == 0 {
				servers = append(servers, server.Addr().String())
			} else {
				servers = append(servers, server.String())
			}
		}
		var err error
		nameServers, err = dns.ParseNameServer(servers)
		if err != nil {
			return nil, fmt.Errorf("parse ZeroTier controller DNS servers: %w", err)
		}
	}
	if len(nameServers) == 0 {
		return nil, nil
	}
	nameServers = append([]dns.NameServer(nil), nameServers...)
	for i := range nameServers {
		nameServers[i].ProxyAdapter = z
	}
	return resolver.Resolver(dns.NewResolver(dns.Config{Main: nameServers, IPv6: config.HasManagedIPv6()})), nil
}

func (z *ZeroTier) runStackPackets(device wireguard.Device, ipLink *ZTIP.Link) {
	buffer := make([]byte, 64*1024)
	buffers := [][]byte{buffer}
	sizes := []int{0}
	for z.ctx.Err() == nil {
		if _, err := device.Read(buffers, sizes, 0); err != nil {
			if z.ctx.Err() == nil {
				if !errors.Is(err, net.ErrClosed) && !errors.Is(err, os.ErrClosed) {
					log.Errorln("[ZeroTier](%s) stack read: %v", z.Name(), err)
				}
				z.invalidateDevice(device, fmt.Errorf("ZeroTier stack read failed: %w", err))
			}
			return
		}
		packet := append([]byte(nil), buffer[:sizes[0]]...)
		z.linkMu.RLock()
		current := z.tunDevice == device && z.ipLink == ipLink
		z.linkMu.RUnlock()
		if !current {
			return
		}
		err := ipLink.WritePacket(packet)
		if err != nil {
			log.Debugln("[ZeroTier](%s) send IP packet: %v", z.Name(), err)
		}
	}
}

func (z *ZeroTier) runInboundFrames() {
	for {
		select {
		case inbound := <-z.frameCh:
			z.linkMu.RLock()
			current := z.node == inbound.node
			ipLink := z.ipLink
			z.linkMu.RUnlock()
			if !current || ipLink == nil {
				continue
			}
			z.handleInboundFrame(inbound.node, ipLink, inbound.frame)
		case <-z.ctx.Done():
			return
		}
	}
}

func (z *ZeroTier) handleInboundFrame(source *ZT.Node, ipLink *ZTIP.Link, frame ZT.Frame) {
	packet, err := ipLink.HandleFrame(frame)
	if err != nil {
		log.Debugln("[ZeroTier](%s) process inbound frame: %v", z.Name(), err)
	}
	if len(packet) == 0 {
		return
	}
	z.linkMu.RLock()
	current := z.node == source && z.ipLink == ipLink
	device := z.tunDevice
	z.linkMu.RUnlock()
	if current && device != nil {
		if _, err = device.Write([][]byte{packet}, 0); err != nil {
			if !errors.Is(err, net.ErrClosed) && !errors.Is(err, os.ErrClosed) {
				log.Debugln("[ZeroTier](%s) stack write: %v", z.Name(), err)
			}
			if z.ctx.Err() == nil {
				z.invalidateDevice(device, fmt.Errorf("ZeroTier stack write failed: %w", err))
			}
		}
	}
}

func (z *ZeroTier) networkStackFor(destination netip.Addr) (*ZTIP.Link, wireguard.Device, error) {
	z.linkMu.RLock()
	ipLink := z.ipLink
	device := z.tunDevice
	networkErr := z.networkErr
	z.linkMu.RUnlock()
	if ipLink == nil {
		return nil, nil, errors.New("ZeroTier core is not ready")
	}
	if err := ipLink.ValidateDestination(destination); err != nil {
		return nil, nil, err
	}
	if networkErr != nil {
		return nil, nil, networkErr
	}
	if device == nil {
		return nil, nil, errors.New("ZeroTier stack is not ready")
	}
	return ipLink, device, nil
}

type zeroTierNetDialer struct {
	zeroTier *ZeroTier
}

func (d zeroTierNetDialer) DialContext(ctx context.Context, network, address string) (net.Conn, error) {
	destination, err := netip.ParseAddrPort(address)
	if err != nil {
		return nil, err
	}
	_, device, err := d.zeroTier.networkStackFor(destination.Addr())
	if err != nil {
		return nil, err
	}
	return device.DialContext(ctx, network, M.ParseSocksaddr(address).Unwrap())
}

func (z *ZeroTier) DialContext(ctx context.Context, metadata *C.Metadata) (_ C.Conn, err error) {
	if err = z.ensureStarted(ctx); err != nil {
		return nil, err
	}
	z.linkMu.RLock()
	remoteResolver := z.resolver
	z.linkMu.RUnlock()
	var conn net.Conn
	if metadata.Resolved() && remoteResolver == nil {
		conn, err = (zeroTierNetDialer{zeroTier: z}).DialContext(ctx, "tcp", metadata.RemoteAddress())
	} else {
		r := resolver.DefaultResolver
		if remoteResolver != nil {
			r = remoteResolver
		}
		options := z.DialOptions()
		options = append(options, dialer.WithResolver(r), dialer.WithNetDialer(zeroTierNetDialer{zeroTier: z}))
		conn, err = dialer.NewDialer(options...).DialContext(ctx, "tcp", metadata.RemoteAddress())
	}
	if err != nil {
		return nil, err
	}
	if conn == nil {
		return nil, errors.New("conn is nil")
	}
	return NewConn(conn, z), nil
}

func (z *ZeroTier) ListenPacketContext(ctx context.Context, metadata *C.Metadata) (_ C.PacketConn, err error) {
	if err = z.ensureStarted(ctx); err != nil {
		return nil, err
	}
	if err = z.ResolveUDP(ctx, metadata); err != nil {
		return nil, err
	}
	ipLink, device, err := z.networkStackFor(metadata.DstIP)
	if err != nil {
		return nil, err
	}
	packetConn, err := device.ListenPacket(ctx, M.SocksaddrFrom(metadata.DstIP, metadata.DstPort).Unwrap())
	if err != nil {
		return nil, err
	}
	if packetConn == nil {
		return nil, errors.New("packetConn is nil")
	}
	return NewPacketConn(&zeroTierPacketConn{PacketConn: packetConn, validateDestination: ipLink.ValidateDestination}, z), nil
}

func (z *ZeroTier) ResolveUDP(ctx context.Context, metadata *C.Metadata) error {
	if metadata.Host == "" {
		return nil
	}
	z.linkMu.RLock()
	remoteResolver := z.resolver
	z.linkMu.RUnlock()
	r := resolver.DefaultResolver
	if remoteResolver != nil {
		r = remoteResolver
	}
	address, err := resolveIPWithResolver(ctx, metadata.Host, z.prefer, r)
	if err != nil {
		return fmt.Errorf("can't resolve IP: %w", err)
	}
	metadata.DstIP = address
	return nil
}

func (z *ZeroTier) ProxyInfo() C.ProxyInfo {
	info := z.Base.ProxyInfo()
	info.DialerProxy = z.option.DialerProxy
	return info
}

func (z *ZeroTier) IsL3Protocol(*C.Metadata) bool {
	return true
}

func (z *ZeroTier) Close() error {
	z.lifecycleMu.Lock()
	if z.closed {
		z.lifecycleMu.Unlock()
		return nil
	}
	z.closed = true
	z.cancel()
	z.configMu.Lock()
	z.linkMu.Lock()
	node, nodeCancel, wireTransport, device := z.detachRuntimeLocked()
	z.resetNetworkStateLocked(errZeroTierClosed)
	z.linkMu.Unlock()
	z.configMu.Unlock()
	z.lifecycleMu.Unlock()
	return closeZeroTierRuntime(node, nodeCancel, wireTransport, device)
}
