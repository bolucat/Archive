// Copyright (C) 2023  mieru authors
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

package appctl

import (
	"context"
	"fmt"
	"io"
	"net"
	"os"
	"path/filepath"
	"sort"
	"sync"
	"sync/atomic"
	"time"

	"github.com/enfein/mieru/v3/pkg/appctl/appctlgrpc"
	pb "github.com/enfein/mieru/v3/pkg/appctl/appctlpb"
	"github.com/enfein/mieru/v3/pkg/common"
	"github.com/enfein/mieru/v3/pkg/egress"
	"github.com/enfein/mieru/v3/pkg/log"
	"github.com/enfein/mieru/v3/pkg/metrics"
	"github.com/enfein/mieru/v3/pkg/protocol"
	"github.com/enfein/mieru/v3/pkg/socks5"
	"github.com/enfein/mieru/v3/pkg/stderror"
	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials/insecure"
	"google.golang.org/protobuf/proto"
)

var (
	// ServerRPCServerStarted is closed when server RPC server is started.
	ServerRPCServerStarted chan struct{} = make(chan struct{})

	cachedServerConfigDir      string = "/etc/mita"
	cachedServerConfigFilePath string = "/etc/mita/server.conf.pb"
	cachedServerUDS            string = "/var/run/mita.sock"

	// serverIOLock is required to load server config and store server config.
	serverIOLock sync.Mutex

	// serverRPCServerRef holds a pointer to server RPC server.
	serverRPCServerRef atomic.Pointer[grpc.Server]

	// socks5ServerGroup holds a pointer to server socks5 server.
	socks5ServerRef atomic.Pointer[socks5.Server]

	// serverMuxRef holds a pointer to server multiplexier.
	serverMuxRef atomic.Pointer[protocol.Mux]
)

func SetServerRPCServerRef(server *grpc.Server) {
	serverRPCServerRef.Store(server)
}

func SetSocks5Server(server *socks5.Server) {
	socks5ServerRef.Store(server)
}

func SetServerMuxRef(mux *protocol.Mux) {
	serverMuxRef.Store(mux)
}

// ServerUDS returns the UNIX domain socket that mita server
// is listening to RPC requests.
func ServerUDS() string {
	if v, found := os.LookupEnv("MITA_UDS_PATH"); found {
		cachedServerUDS = v
	}
	return cachedServerUDS
}

// serverLifecycleService implements ServerLifecycleService defined in lifecycle.proto.
type serverLifecycleService struct {
	appctlgrpc.UnimplementedServerLifecycleServiceServer
}

func (s *serverLifecycleService) GetStatus(ctx context.Context, req *pb.Empty) (*pb.AppStatusMsg, error) {
	status := GetAppStatus()
	log.Infof("return app status %s back to RPC caller", status.String())
	return &pb.AppStatusMsg{Status: &status}, nil
}

func (s *serverLifecycleService) Start(ctx context.Context, req *pb.Empty) (*pb.Empty, error) {
	log.Infof("received start request from RPC caller")
	config, err := LoadServerConfig()
	if err != nil {
		return &pb.Empty{}, fmt.Errorf("LoadServerConfig() failed: %w", err)
	}
	if err = ValidateFullServerConfig(config); err != nil {
		return &pb.Empty{}, fmt.Errorf("ValidateFullServerConfig() failed: %w", err)
	}
	loggingLevel := config.GetLoggingLevel().String()
	if loggingLevel != pb.LoggingLevel_DEFAULT.String() {
		log.SetLevel(loggingLevel)
	}
	if socks5ServerRef.Load() != nil {
		log.Infof("socks5 server already exist")
		return &pb.Empty{}, nil
	}

	SetAppStatus(pb.AppStatus_STARTING)

	mux := protocol.NewMux(false).SetServerUsers(UserListToMap(config.GetUsers()))
	SetServerMuxRef(mux)
	mtu := common.DefaultMTU
	if config.GetMtu() != 0 {
		mtu = int(config.GetMtu())
	}
	endpoints, err := PortBindingsToUnderlayProperties(config.GetPortBindings(), mtu)
	if err != nil {
		return &pb.Empty{}, err
	}
	mux.SetEndpoints(endpoints)

	// Create the egress socks5 server.
	socks5Config := &socks5.Config{
		AllowLocalDestination: config.GetAdvancedSettings().GetAllowLocalDestination(),
		AuthOpts: socks5.Auth{
			ClientSideAuthentication: true,
		},
		EgressController: egress.NewSocks5Controller(config.GetEgress()),
		HandshakeTimeout: 10 * time.Second,
	}
	socks5Server, err := socks5.New(socks5Config)
	if err != nil {
		return &pb.Empty{}, fmt.Errorf(stderror.CreateSocks5ServerFailedErr, err)
	}
	SetSocks5Server(socks5Server)

	// Run the egress socks5 server in the background.
	var initProxyTasks sync.WaitGroup
	initProxyTasks.Add(1)
	go func() {
		if err = mux.Start(); err != nil {
			log.Fatalf("socks5 server listening failed: %v", err)
		}
		initProxyTasks.Done()

		log.Infof("mita server daemon socks5 server is running")
		if err = socks5Server.Serve(mux); err != nil {
			log.Fatalf("run socks5 server failed: %v", err)
		}
		log.Infof("mita server daemon socks5 server is stopped")
	}()

	initProxyTasks.Wait()
	metrics.EnableLogging()
	SetAppStatus(pb.AppStatus_RUNNING)
	log.Infof("completed start request from RPC caller")
	return &pb.Empty{}, nil
}

func (s *serverLifecycleService) Stop(ctx context.Context, req *pb.Empty) (*pb.Empty, error) {
	SetAppStatus(pb.AppStatus_STOPPING)
	log.Infof("received stop request from RPC caller")
	if socks5ServerRef.Load() != nil {
		log.Infof("stopping socks5 server")
		if err := socks5ServerRef.Load().Close(); err != nil {
			log.Infof("socks5 server Close() failed: %v", err)
		}
		SetSocks5Server(nil)
	} else {
		log.Infof("active socks5 servers not found")
	}
	SetAppStatus(pb.AppStatus_IDLE)
	log.Infof("completed stop request from RPC caller")
	return &pb.Empty{}, nil
}

func (s *serverLifecycleService) Reload(ctx context.Context, req *pb.Empty) (*pb.Empty, error) {
	log.Infof("received start request from RPC caller")
	config, err := LoadServerConfig()
	if err != nil {
		return &pb.Empty{}, fmt.Errorf("LoadServerConfig() failed: %w", err)
	}
	if err = ValidateFullServerConfig(config); err != nil {
		return &pb.Empty{}, fmt.Errorf("ValidateFullServerConfig() failed: %w", err)
	}

	// Adjust loggingLevel.
	// This needs to happen before adjusting other settings.
	loggingLevel := config.GetLoggingLevel().String()
	if loggingLevel != pb.LoggingLevel_DEFAULT.String() {
		log.SetLevel(loggingLevel)
	}

	mux := serverMuxRef.Load()
	if mux != nil {
		// Adjust portBindings.
		mtu := common.DefaultMTU
		if config.GetMtu() != 0 {
			mtu = int(config.GetMtu())
		}
		endpoints, err := PortBindingsToUnderlayProperties(config.GetPortBindings(), mtu)
		if err != nil {
			return &pb.Empty{}, err
		}
		mux.SetEndpoints(endpoints)

		// Adjust users.
		mux.SetServerUsers(UserListToMap(config.GetUsers()))
	}
	return &pb.Empty{}, nil
}

func (s *serverLifecycleService) Exit(ctx context.Context, req *pb.Empty) (*pb.Empty, error) {
	SetAppStatus(pb.AppStatus_STOPPING)
	log.Infof("received exit request from RPC caller")
	if socks5ServerRef.Load() != nil {
		log.Infof("stopping socks5 server")
		if err := socks5ServerRef.Load().Close(); err != nil {
			log.Infof("socks5 server Close() failed: %v", err)
		}
		SetSocks5Server(nil)
	} else {
		log.Infof("active socks5 servers not found")
	}
	SetAppStatus(pb.AppStatus_IDLE)

	grpcServer := serverRPCServerRef.Load()
	if grpcServer != nil {
		log.Infof("stopping RPC server")
		go grpcServer.GracefulStop()
	} else {
		log.Infof("RPC server reference not found")
	}
	log.Infof("completed exit request from RPC caller")
	return &pb.Empty{}, nil
}

func (s *serverLifecycleService) GetMetrics(ctx context.Context, req *pb.Empty) (*pb.Metrics, error) {
	b, err := metrics.GetMetricsAsJSON()
	if err != nil {
		return &pb.Metrics{}, err
	}
	return &pb.Metrics{Json: proto.String(string(b))}, nil
}

func (s *serverLifecycleService) GetSessionInfo(context.Context, *pb.Empty) (*pb.SessionInfo, error) {
	mux := serverMuxRef.Load()
	if mux == nil {
		return &pb.SessionInfo{}, fmt.Errorf("server multiplexier is unavailable")
	}
	return &pb.SessionInfo{Table: mux.ExportSessionInfoTable()}, nil
}

func (s *serverLifecycleService) GetThreadDump(ctx context.Context, req *pb.Empty) (*pb.ThreadDump, error) {
	return &pb.ThreadDump{ThreadDump: proto.String(common.GetAllStackTrace())}, nil
}

func (s *serverLifecycleService) StartCPUProfile(ctx context.Context, req *pb.ProfileSavePath) (*pb.Empty, error) {
	err := common.StartCPUProfile(req.GetFilePath())
	return &pb.Empty{}, err
}

func (s *serverLifecycleService) StopCPUProfile(ctx context.Context, req *pb.Empty) (*pb.Empty, error) {
	common.StopCPUProfile()
	return &pb.Empty{}, nil
}

func (s *serverLifecycleService) GetHeapProfile(ctx context.Context, req *pb.ProfileSavePath) (*pb.Empty, error) {
	err := common.GetHeapProfile(req.GetFilePath())
	return &pb.Empty{}, err
}

func (s *serverLifecycleService) GetMemoryStatistics(ctx context.Context, req *pb.Empty) (*pb.MemoryStatistics, error) {
	return &pb.MemoryStatistics{Json: proto.String(common.GetMemoryStats())}, nil
}

// NewServerLifecycleService creates a new ServerLifecycleService RPC server.
func NewServerLifecycleService() *serverLifecycleService {
	return &serverLifecycleService{}
}

// NewServerLifecycleRPCClient creates a new ServerLifecycleService RPC client.
func NewServerLifecycleRPCClient() (appctlgrpc.ServerLifecycleServiceClient, error) {
	rpcAddr := "unix://" + ServerUDS()
	conn, err := grpc.NewClient(rpcAddr, grpc.WithTransportCredentials(insecure.NewCredentials()), grpc.WithDefaultCallOptions(grpc.MaxCallRecvMsgSize(MaxRecvMsgSize)))
	if err != nil {
		return nil, fmt.Errorf("grpc.NewClient() failed: %w", err)
	}
	return appctlgrpc.NewServerLifecycleServiceClient(conn), nil
}

// serverConfigService implements ServerConfigService defined in servercfg.proto.
type serverConfigService struct {
	appctlgrpc.UnimplementedServerConfigServiceServer
}

func (s *serverConfigService) GetConfig(ctx context.Context, req *pb.Empty) (*pb.ServerConfig, error) {
	config, err := LoadServerConfig()
	if err != nil {
		return &pb.ServerConfig{}, fmt.Errorf("LoadServerConfig() failed: %w", err)
	}
	return config, nil
}

func (s *serverConfigService) SetConfig(ctx context.Context, req *pb.ServerConfig) (*pb.ServerConfig, error) {
	if err := StoreServerConfig(req); err != nil {
		return &pb.ServerConfig{}, fmt.Errorf("StoreServerConfig() failed: %w", err)
	}
	config, err := LoadServerConfig()
	if err != nil {
		return &pb.ServerConfig{}, fmt.Errorf("LoadServerConfig() failed: %w", err)
	}
	return config, nil
}

// NewServerConfigService creates a new ServerConfigService RPC server.
func NewServerConfigService() *serverConfigService {
	return &serverConfigService{}
}

// NewServerConfigRPCClient creates a new ServerConfigService RPC client.
func NewServerConfigRPCClient() (appctlgrpc.ServerConfigServiceClient, error) {
	rpcAddr := "unix://" + ServerUDS()
	conn, err := grpc.NewClient(rpcAddr, grpc.WithTransportCredentials(insecure.NewCredentials()), grpc.WithDefaultCallOptions(grpc.MaxCallRecvMsgSize(MaxRecvMsgSize)))
	if err != nil {
		return nil, fmt.Errorf("grpc.NewClient() failed: %w", err)
	}
	return appctlgrpc.NewServerConfigServiceClient(conn), nil
}

// GetServerStatusWithRPC gets server application status via ServerLifecycleService.GetStatus() RPC.
func GetServerStatusWithRPC(ctx context.Context) (*pb.AppStatusMsg, error) {
	client, err := NewServerLifecycleRPCClient()
	if err != nil {
		return nil, fmt.Errorf("NewServerLifecycleRPCClient() failed: %w", err)
	}
	timedctx, cancelFunc := context.WithTimeout(ctx, RPCTimeout)
	defer cancelFunc()
	status, err := client.GetStatus(timedctx, &pb.Empty{})
	if err != nil {
		return nil, fmt.Errorf("ServerLifecycleService.GetStatus() failed: %w", err)
	}
	return status, nil
}

// IsServerDaemonRunning returns nil if app status shows server daemon is running.
func IsServerDaemonRunning(appStatus *pb.AppStatusMsg) error {
	if appStatus == nil {
		return fmt.Errorf("AppStatusMsg is nil")
	}
	if appStatus.GetStatus() == pb.AppStatus_UNKNOWN {
		return fmt.Errorf("mita server status is %q", appStatus.GetStatus().String())
	}
	return nil
}

// IsServerProxyRunning returns nil if app status shows proxy function is running.
func IsServerProxyRunning(appStatus *pb.AppStatusMsg) error {
	if err := IsServerDaemonRunning(appStatus); err != nil {
		return err
	}
	if appStatus.GetStatus() != pb.AppStatus_RUNNING {
		return fmt.Errorf("mita server status is %q", appStatus.GetStatus().String())
	}
	return nil
}

// GetJSONServerConfig returns the server config as JSON.
func GetJSONServerConfig() (string, error) {
	config, err := LoadServerConfig()
	if err != nil {
		return "", fmt.Errorf("LoadServerConfig() failed: %w", err)
	}
	b, err := common.MarshalJSON(config)
	if err != nil {
		return "", fmt.Errorf("common.MarshalJSON() failed: %w", err)
	}
	return string(b), nil
}

// LoadServerConfig reads server config from disk.
func LoadServerConfig() (*pb.ServerConfig, error) {
	serverIOLock.Lock()
	defer serverIOLock.Unlock()

	fileName, fileType, err := serverConfigFilePath()
	if err != nil {
		return nil, fmt.Errorf("serverConfigFilePath() failed: %w", err)
	}
	if err := checkServerConfigDir(); err != nil {
		return nil, fmt.Errorf("checkServerConfigDir() failed: %w", err)
	}

	log.Debugf("loading server config from %q", fileName)
	f, err := os.Open(fileName)
	if err != nil && os.IsNotExist(err) {
		return nil, stderror.ErrFileNotExist
	} else if err != nil {
		return nil, fmt.Errorf("os.Open() failed: %w", err)
	}
	defer f.Close()

	b, err := io.ReadAll(f)
	if err != nil {
		return nil, fmt.Errorf("io.ReadAll(%q) failed: %w", fileName, err)
	}

	s := &pb.ServerConfig{}
	switch fileType {
	case PROTOBUF_CONFIG_FILE_TYPE:
		if err := proto.Unmarshal(b, s); err != nil {
			return nil, fmt.Errorf("proto.Unmarshal() failed: %w", err)
		}
	case JSON_CONFIG_FILE_TYPE:
		if err := common.UnmarshalJSON(b, s); err != nil {
			return nil, fmt.Errorf("common.UnmarshalJSON() failed: %w", err)
		}
	default:
		return nil, fmt.Errorf("config file type is invalid")
	}

	return s, nil
}

// StoreServerConfig writes server config to disk.
func StoreServerConfig(config *pb.ServerConfig) error {
	serverIOLock.Lock()
	defer serverIOLock.Unlock()

	if config == nil {
		return fmt.Errorf("ServerConfig is nil")
	}
	config.Users = HashUserPasswords(config.GetUsers(), false)

	fileName, fileType, err := serverConfigFilePath()
	if err != nil {
		return fmt.Errorf("serverConfigFilePath() failed: %w", err)
	}
	if err := checkServerConfigDir(); err != nil {
		return fmt.Errorf("checkServerConfigDir() failed: %w", err)
	}

	var b []byte
	switch fileType {
	case PROTOBUF_CONFIG_FILE_TYPE:
		if b, err = proto.Marshal(config); err != nil {
			return fmt.Errorf("proto.Marshal() failed: %w", err)
		}
	case JSON_CONFIG_FILE_TYPE:
		if b, err = common.MarshalJSON(config); err != nil {
			return fmt.Errorf("common.MarshalJSON() failed: %w", err)
		}
	default:
		return fmt.Errorf("config file type is invalid")
	}

	err = os.WriteFile(fileName, b, 0660)
	if err != nil {
		return fmt.Errorf("os.WriteFile(%q) failed: %w", fileName, err)
	}
	return nil
}

// ApplyJSONServerConfig applies user provided JSON server config from path.
func ApplyJSONServerConfig(path string) error {
	b, err := os.ReadFile(path)
	if err != nil {
		return fmt.Errorf("os.ReadFile(%q) failed: %w", path, err)
	}
	s := &pb.ServerConfig{}
	if err = common.UnmarshalJSON(b, s); err != nil {
		return fmt.Errorf("common.UnmarshalJSON() failed: %w", err)
	}
	if err := ValidateServerConfigPatch(s); err != nil {
		return fmt.Errorf("ValidateServerConfigPatch() failed: %w", err)
	}
	config, err := LoadServerConfig()
	if err != nil {
		return fmt.Errorf("LoadServerConfig() failed: %w", err)
	}
	if err = mergeServerConfig(config, s); err != nil {
		return fmt.Errorf("mergeServerConfig() failed: %w", err)
	}
	if err = ValidateFullServerConfig(config); err != nil {
		return fmt.Errorf("ValidateFullServerConfig() failed: %w", err)
	}
	if err = StoreServerConfig(config); err != nil {
		return fmt.Errorf("StoreServerConfig() failed: %w", err)
	}
	return nil
}

// DeleteServerUsers deletes the list of users from server config.
func DeleteServerUsers(names []string) error {
	config, err := LoadServerConfig()
	if err != nil {
		return fmt.Errorf("LoadServerConfig() failed: %w", err)
	}
	users := config.GetUsers()
	remaining := make([]*pb.User, 0)
	// The complexity of the following algorithm is O(total_users * users_to_delete).
	// This seems to be high, however in reality the number of users to delete is typically 1,
	// so it is faster than using a set to find the difference, then sort the users by name.
	for _, user := range users {
		shouldDelete := false
		for _, toDelete := range names {
			if user.GetName() == toDelete {
				shouldDelete = true
				break
			}
		}
		if !shouldDelete {
			remaining = append(remaining, user)
		}
	}
	config.Users = remaining
	if err := StoreServerConfig(config); err != nil {
		return fmt.Errorf("StoreServerConfig() failed: %w", err)
	}
	return nil
}

// ValidateServerConfigPatch validates a patch of server config.
//
// A server config patch must satisfy:
// 1. port bindings are valid
// 2. for each user
// 2.1. user name is not empty
// 2.2. user has either a password or a hashed password
// 2.3. for each quota
// 2.3.1. number of days is valid
// 2.3.2. traffic volume in megabyte is valid
// 3. if set, MTU is valid
// 4. for each egress proxy
// 4.1. name is not empty
// 4.2. name is unique
// 4.3. protocol is valid
// 4.4. host is not empty
// 4.5. port is valid
// 4.6. if socks5 authentication is used, the user and password are not empty
// 5. there is maximum 1 egress rule
// 5.1. the IP ranges must be "*"
// 5.2. the domain names must be "*"
// 5.3. the action must be "PROXY"
// 5.4. the proxy name is defined
func ValidateServerConfigPatch(patch *pb.ServerConfig) error {
	if _, err := FlatPortBindings(patch.GetPortBindings()); err != nil {
		return err
	}
	for _, user := range patch.GetUsers() {
		if user.GetName() == "" {
			return fmt.Errorf("user name is not set")
		}
		if user.GetPassword() == "" && user.GetHashedPassword() == "" {
			return fmt.Errorf("user password is not set")
		}
		for _, quota := range user.GetQuotas() {
			if quota.GetDays() <= 0 {
				return fmt.Errorf("quota: number of days %d is invalid", quota.GetDays())
			}
			if quota.GetMegabytes() <= 0 {
				return fmt.Errorf("quota: traffic volume in megabyte %d is invalid", quota.GetMegabytes())
			}
		}
	}
	if patch.GetMtu() != 0 && (patch.GetMtu() < 1280 || patch.GetMtu() > 1500) {
		return fmt.Errorf("MTU value %d is out of range, valid range is [1280, 1500]", patch.GetMtu())
	}
	usedProxyNames := map[string]bool{}
	for _, proxy := range patch.GetEgress().GetProxies() {
		if proxy.GetName() == "" {
			return fmt.Errorf("egress proxy name is empty")
		}
		if _, found := usedProxyNames[proxy.GetName()]; found {
			return fmt.Errorf("found duplicate egress proxy name %q", proxy.GetName())
		}
		usedProxyNames[proxy.GetName()] = true
		if proxy.GetProtocol() == pb.ProxyProtocol_UNKNOWN_PROXY_PROTOCOL {
			return fmt.Errorf("egress proxy protocol is not set")
		}
		if proxy.GetHost() == "" {
			return fmt.Errorf("egress proxy host is not set")
		}
		if proxy.GetPort() < 1 || proxy.GetPort() > 65535 {
			return fmt.Errorf("egress proxy port number %d is invalid", proxy.GetPort())
		}
		hasSocks5AuthenticationUser := proxy.GetSocks5Authentication().GetUser() != ""
		hasSocks5AuthenticationPassword := proxy.GetSocks5Authentication().GetPassword() != ""
		if !hasSocks5AuthenticationUser && hasSocks5AuthenticationPassword {
			return fmt.Errorf("egress proxy socks5 authentication user is not set")
		}
		if hasSocks5AuthenticationUser && !hasSocks5AuthenticationPassword {
			return fmt.Errorf("egress proxy socks5 authentication password is not set")
		}
	}
	if len(patch.GetEgress().GetRules()) > 1 {
		return fmt.Errorf("found %d egress rules, maximum number of supported rules is 1", len(patch.GetEgress().GetRules()))
	}
	if len(patch.GetEgress().GetRules()) == 1 {
		rule := patch.GetEgress().GetRules()[0]
		if len(rule.GetIpRanges()) != 1 || rule.GetIpRanges()[0] != "*" {
			return fmt.Errorf("egress rule: the only supported IP range value is %q", "*")
		}
		if len(rule.GetDomainNames()) != 1 || rule.GetDomainNames()[0] != "*" {
			return fmt.Errorf("egress rule: the only supported domain name value is %q", "*")
		}
		if rule.GetAction() != pb.EgressAction_PROXY {
			return fmt.Errorf("egress rule: the only supported action is %q", pb.EgressAction_PROXY.String())
		}
		if rule.GetProxyName() == "" {
			return fmt.Errorf("egress rule: proxy name is not set")
		}
		foundProxy := false
		for _, proxy := range patch.GetEgress().GetProxies() {
			if proxy.GetName() == rule.GetProxyName() {
				foundProxy = true
				break
			}
		}
		if !foundProxy {
			return fmt.Errorf("egress rule: proxy %q is not defined", rule.GetProxyName())
		}
	}
	return nil
}

// ValidateFullServerConfig validates the full server config.
//
// In addition to ValidateServerConfigPatch, it also validates:
// 1. there is at least 1 port binding
//
// It is not an error if no user is configured. However mita won't be functional.
func ValidateFullServerConfig(config *pb.ServerConfig) error {
	if err := ValidateServerConfigPatch(config); err != nil {
		return err
	}
	if proto.Equal(config, &pb.ServerConfig{}) {
		return fmt.Errorf("server config is empty")
	}
	if len(config.GetPortBindings()) == 0 {
		return fmt.Errorf("server port binding is not set")
	}
	return nil
}

// PortBindingsToUnderlayProperties converts port bindings to underlay properties.
func PortBindingsToUnderlayProperties(portBindings []*pb.PortBinding, mtu int) ([]protocol.UnderlayProperties, error) {
	endpoints := make([]protocol.UnderlayProperties, 0)
	listenIP := net.ParseIP(common.AllIPAddr())
	if listenIP == nil {
		return endpoints, fmt.Errorf(stderror.ParseIPFailed)
	}
	portBindings, err := FlatPortBindings(portBindings)
	if err != nil {
		return endpoints, fmt.Errorf(stderror.InvalidPortBindingsErr, err)
	}
	n := len(portBindings)
	for i := 0; i < n; i++ {
		proto := portBindings[i].GetProtocol()
		port := portBindings[i].GetPort()
		switch proto {
		case pb.TransportProtocol_TCP:
			endpoint := protocol.NewUnderlayProperties(mtu, common.StreamTransport, &net.TCPAddr{IP: listenIP, Port: int(port)}, nil)
			endpoints = append(endpoints, endpoint)
		case pb.TransportProtocol_UDP:
			endpoint := protocol.NewUnderlayProperties(mtu, common.PacketTransport, &net.UDPAddr{IP: listenIP, Port: int(port)}, nil)
			endpoints = append(endpoints, endpoint)
		default:
			return []protocol.UnderlayProperties{}, fmt.Errorf(stderror.InvalidTransportProtocol)
		}
	}
	return endpoints, nil
}

// checkServerConfigDir validates if server config directory exists.
func checkServerConfigDir() error {
	_, err := os.Stat(cachedServerConfigDir)
	return err
}

// serverConfigFilePath returns the server config file path.
// If environment variable MITA_CONFIG_FILE or MITA_CONFIG_JSON_FILE is specified,
// those values are returned.
func serverConfigFilePath() (string, ConfigFileType, error) {
	if v, found := os.LookupEnv("MITA_CONFIG_FILE"); found {
		cachedServerConfigFilePath = v
		cachedServerConfigDir = filepath.Dir(v)
		return cachedServerConfigFilePath, PROTOBUF_CONFIG_FILE_TYPE, nil
	}
	if v, found := os.LookupEnv("MITA_CONFIG_JSON_FILE"); found {
		cachedServerConfigFilePath = v
		cachedServerConfigDir = filepath.Dir(v)
		return cachedServerConfigFilePath, JSON_CONFIG_FILE_TYPE, nil
	}
	if cachedServerConfigFilePath != "" {
		return cachedServerConfigFilePath, FindConfigFileType(cachedServerConfigFilePath), nil
	}
	return "", INVALID_CONFIG_FILE_TYPE, fmt.Errorf("server config file path is empty")
}

// mergeServerConfig merges the source client config into destination.
// If a user is specified in source, it is added to destination, or replacing existing user in destination.
func mergeServerConfig(dst, src *pb.ServerConfig) error {
	// Port bindings: if src is set, replace dst with src.
	var portBindings []*pb.PortBinding
	if src.PortBindings != nil {
		portBindings = src.GetPortBindings()
	} else {
		portBindings = dst.GetPortBindings()
	}

	// Users: merge src into dst.
	mergedUserMapping := map[string]*pb.User{}
	for _, user := range dst.GetUsers() {
		mergedUserMapping[user.GetName()] = user
	}
	for _, user := range src.GetUsers() {
		mergedUserMapping[user.GetName()] = user
	}
	names := make([]string, 0, len(mergedUserMapping))
	for name := range mergedUserMapping {
		names = append(names, name)
	}
	sort.Strings(names)
	mergedUsers := make([]*pb.User, 0, len(mergedUserMapping))
	for _, name := range names {
		mergedUsers = append(mergedUsers, mergedUserMapping[name])
	}

	var advancedSettings *pb.ServerAdvancedSettings
	if src.AdvancedSettings != nil {
		advancedSettings = src.GetAdvancedSettings()
	} else {
		advancedSettings = dst.GetAdvancedSettings()
	}
	var loggingLevel pb.LoggingLevel
	if src.LoggingLevel != nil {
		loggingLevel = src.GetLoggingLevel()
	} else {
		loggingLevel = dst.GetLoggingLevel()
	}
	var mtu int32
	if src.Mtu != nil {
		mtu = src.GetMtu()
	} else {
		mtu = dst.GetMtu()
	}
	var egress *pb.Egress
	if src.Egress != nil {
		egress = src.GetEgress()
	} else {
		egress = dst.GetEgress()
	}

	proto.Reset(dst)
	dst.PortBindings = portBindings
	dst.Users = mergedUsers
	dst.AdvancedSettings = advancedSettings
	dst.LoggingLevel = &loggingLevel
	dst.Mtu = proto.Int32(mtu)
	dst.Egress = egress
	return nil
}

// deleteServerConfigFile deletes the server config file.
func deleteServerConfigFile() error {
	path, _, err := serverConfigFilePath()
	if err != nil {
		return fmt.Errorf("serverConfigFilePath() failed: %w", err)
	}
	err = os.Remove(path)
	if err != nil && os.IsNotExist(err) {
		return nil
	}
	return err
}
