// Copyright (C) 2021  mieru authors
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

package cli

import (
	"context"
	"encoding/hex"
	"errors"
	"fmt"
	"io"
	"net"
	"net/http"
	"os"
	"os/exec"
	"runtime/pprof"
	"strconv"
	"strings"
	"sync"
	"time"

	"github.com/enfein/mieru/pkg/appctl"
	"github.com/enfein/mieru/pkg/appctl/appctlpb"
	"github.com/enfein/mieru/pkg/cipher"
	"github.com/enfein/mieru/pkg/http2socks"
	"github.com/enfein/mieru/pkg/log"
	"github.com/enfein/mieru/pkg/metrics"
	"github.com/enfein/mieru/pkg/protocolv2"
	"github.com/enfein/mieru/pkg/socks5"
	"github.com/enfein/mieru/pkg/socks5client"
	"github.com/enfein/mieru/pkg/stderror"
	"github.com/enfein/mieru/pkg/util"
	"github.com/enfein/mieru/pkg/util/sockopts"
	"google.golang.org/grpc"
	"google.golang.org/protobuf/proto"
)

// RegisterClientCommands registers all the client side CLI commands.
func RegisterClientCommands() {
	RegisterCallback(
		[]string{"", "help"},
		func(s []string) error {
			return unexpectedArgsError(s, 2)
		},
		clientHelpFunc,
	)
	RegisterCallback(
		[]string{"", "start"},
		func(s []string) error {
			return unexpectedArgsError(s, 2)
		},
		clientStartFunc,
	)
	RegisterCallback(
		[]string{"", "run"},
		func(s []string) error {
			return unexpectedArgsError(s, 2)
		},
		clientRunFunc,
	)
	RegisterCallback(
		[]string{"", "stop"},
		func(s []string) error {
			return unexpectedArgsError(s, 2)
		},
		clientStopFunc,
	)
	RegisterCallback(
		[]string{"", "status"},
		func(s []string) error {
			return unexpectedArgsError(s, 2)
		},
		clientStatusFunc,
	)
	RegisterCallback(
		[]string{"", "test"},
		func(s []string) error {
			if len(s) > 3 {
				return fmt.Errorf("usage: mieru test [URL]. More than 1 URL is provided")
			}
			if len(s) == 3 {
				if !strings.HasPrefix(s[2], "http://") && !strings.HasPrefix(s[2], "https://") {
					return fmt.Errorf("provided URL is invalid, it must start with %q or %q", "http://", "https://")
				}
			}
			return nil
		},
		clientTestFunc,
	)
	RegisterCallback(
		[]string{"", "apply", "config"},
		func(s []string) error {
			if len(s) < 4 {
				return fmt.Errorf("usage: mieru apply config <FILE>. No config file is provided")
			} else if len(s) > 4 {
				return fmt.Errorf("usage: mieru apply config <FILE>. More than 1 config file is provided")
			}
			return nil
		},
		clientApplyConfigFunc,
	)
	RegisterCallback(
		[]string{"", "describe", "config"},
		func(s []string) error {
			return unexpectedArgsError(s, 3)
		},
		clientDescribeConfigFunc,
	)
	RegisterCallback(
		[]string{"", "import", "config"},
		func(s []string) error {
			if len(s) < 4 {
				return fmt.Errorf("usage: mieru import config <URL>. No URL is provided")
			} else if len(s) > 4 {
				return fmt.Errorf("usage: mieru import config <URL>. More than 1 URL is provided")
			}
			return nil
		},
		clientImportConfigFunc,
	)
	RegisterCallback(
		[]string{"", "export", "config"},
		func(s []string) error {
			return unexpectedArgsError(s, 3)
		},
		clientExportConfigFunc,
	)
	RegisterCallback(
		[]string{"", "delete", "profile"},
		func(s []string) error {
			if len(s) < 4 {
				return fmt.Errorf("usage: mieru delete profile <PROFILE_NAME>. no profile is provided")
			} else if len(s) > 4 {
				return fmt.Errorf("usage: mieru delete profile <PROFILE_NAME>. more than 1 profile is provided")
			}
			return nil
		},
		clientDeleteProfileFunc,
	)
	RegisterCallback(
		[]string{"", "delete", "http", "proxy"},
		func(s []string) error {
			return unexpectedArgsError(s, 4)
		},
		clientDeleteHTTPProxyFunc,
	)
	RegisterCallback(
		[]string{"", "delete", "socks5", "authentication"},
		func(s []string) error {
			return unexpectedArgsError(s, 4)
		},
		clientDeleteSocks5AuthenticationFunc,
	)
	RegisterCallback(
		[]string{"", "version"},
		func(s []string) error {
			return unexpectedArgsError(s, 2)
		},
		versionFunc,
	)
	RegisterCallback(
		[]string{"", "check", "update"},
		func(s []string) error {
			return unexpectedArgsError(s, 3)
		},
		checkUpdateFunc,
	)
	RegisterCallback(
		[]string{"", "get", "metrics"},
		func(s []string) error {
			return unexpectedArgsError(s, 3)
		},
		clientGetMetricsFunc,
	)
	RegisterCallback(
		[]string{"", "get", "connections"},
		func(s []string) error {
			return unexpectedArgsError(s, 3)
		},
		clientGetConnectionsFunc,
	)
	RegisterCallback(
		[]string{"", "get", "thread-dump"},
		func(s []string) error {
			return unexpectedArgsError(s, 3)
		},
		clientGetThreadDumpFunc,
	)
	RegisterCallback(
		[]string{"", "get", "heap-profile"},
		func(s []string) error {
			if len(s) < 4 {
				return fmt.Errorf("usage: mieru get heap-profile <FILE>. no file save path is provided")
			} else if len(s) > 4 {
				return fmt.Errorf("usage: mieru get heap-profile <FILE>. more than 1 file save path is provided")
			}
			return nil
		},
		clientGetHeapProfileFunc,
	)
	RegisterCallback(
		[]string{"", "get", "memory-statistics"},
		func(s []string) error {
			return unexpectedArgsError(s, 3)
		},
		clientGetMemoryStatisticsFunc,
	)
	RegisterCallback(
		[]string{"", "profile", "cpu", "start"},
		func(s []string) error {
			if len(s) < 5 {
				return fmt.Errorf("usage: mieru profile cpu start <FILE>. no file save path is provided")
			} else if len(s) > 5 {
				return fmt.Errorf("usage: mieru profile cpu start <FILE>. more than 1 file save path is provided")
			}
			return nil
		},
		clientStartCPUProfileFunc,
	)
	RegisterCallback(
		[]string{"", "profile", "cpu", "stop"},
		func(s []string) error {
			return unexpectedArgsError(s, 4)
		},
		clientStopCPUProfileFunc,
	)
}

var clientHelpFunc = func(s []string) error {
	helpFmt := helpFormatter{
		appName: "mieru",
		entries: []helpCmdEntry{
			{
				cmd:  "help",
				help: "Show mieru client help.",
			},
			{
				cmd:  "start",
				help: "Start mieru client in background.",
			},
			{
				cmd:  "stop",
				help: "Stop mieru client.",
			},
			{
				cmd:  "status",
				help: "Check mieru client status.",
			},
			{
				cmd:  "test [URL]",
				help: "Test mieru client connection to the Internet via proxy server.",
			},
			{
				cmd:  "apply config <FILE>",
				help: "Apply client configuration from JSON file.",
			},
			{
				cmd:  "describe config",
				help: "Show current client configuration.",
			},
			{
				cmd:  "import config <URL>",
				help: "Import client configuration from URL.",
			},
			{
				cmd:  "export config",
				help: "Export client configuration as URL.",
			},
			{
				cmd:  "delete profile <PROFILE_NAME>",
				help: "Delete an inactive client configuration profile.",
			},
			{
				cmd:  "delete http proxy",
				help: "Delete HTTP(S) proxy. Allow socks5 user password authentication to be used.",
			},
			{
				cmd:  "delete socks5 authentication",
				help: "Delete socks5 user password authentication. Allow HTTP(S) proxy to be used.",
			},
			{
				cmd:  "get metrics",
				help: "Get mieru client metrics.",
			},
			{
				cmd:  "get connections",
				help: "Get mieru client connections.",
			},
			{
				cmd:  "version",
				help: "Show mieru client version.",
			},
			{
				cmd:  "check update",
				help: "Check mieru client update.",
			},
		},
		advanced: []helpCmdEntry{
			{
				cmd:  "run",
				help: "Run mieru client in foreground.",
			},
			{
				cmd:  "get thread-dump",
				help: "Get mieru client thread dump.",
			},
			{
				cmd:  "get heap-profile <GZ_FILE>",
				help: "Get mieru client heap profile and save results to the file.",
			},
			{
				cmd:  "get memory-statistics",
				help: "Get mieru client memory statistics.",
			},
			{
				cmd:  "profile cpu start <GZ_FILE>",
				help: "Start mieru client CPU profile and save results to the file.",
			},
			{
				cmd:  "profile cpu stop",
				help: "Stop mieru client CPU profile.",
			},
		},
	}
	helpFmt.print()
	return nil
}

var clientStartFunc = func(s []string) error {
	// Load and verify client config.
	config, err := appctl.LoadClientConfig()
	if err != nil {
		if err == stderror.ErrFileNotExist {
			return fmt.Errorf(stderror.ClientConfigNotExist)
		} else {
			return fmt.Errorf(stderror.GetClientConfigFailedErr, err)
		}
	}
	if err = appctl.ValidateFullClientConfig(config); err != nil {
		return fmt.Errorf(stderror.ValidateFullClientConfigFailedErr, err)
	}

	if err = appctl.IsClientDaemonRunning(context.Background()); err == nil {
		if config.GetSocks5ListenLAN() {
			log.Infof("mieru client is running, listening to socks5://0.0.0.0:%d", config.GetSocks5Port())
		} else {
			log.Infof("mieru client is running, listening to socks5://127.0.0.1:%d", config.GetSocks5Port())
		}
		return nil
	}

	cmd := exec.Command(s[0], "run")
	if errors.Is(cmd.Err, exec.ErrDot) {
		cmd.Err = nil
	}
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr
	if err := cmd.Start(); err != nil {
		return fmt.Errorf(stderror.StartClientFailedErr, err)
	}

	// Wait until client daemon is running.
	// The maximum waiting time is 10 seconds.
	var lastErr error
	for i := 0; i < 100; i++ {
		lastErr = appctl.IsClientDaemonRunning(context.Background())
		if lastErr == nil {
			if config.GetSocks5ListenLAN() {
				log.Infof("mieru client is started, listening to socks5://0.0.0.0:%d", config.GetSocks5Port())
			} else {
				log.Infof("mieru client is started, listening to socks5://127.0.0.1:%d", config.GetSocks5Port())
			}
			return nil
		}
		time.Sleep(100 * time.Millisecond)
	}
	return fmt.Errorf(stderror.ClientNotRunningErr, lastErr)
}

var clientRunFunc = func(s []string) error {
	log.SetFormatter(&log.DaemonFormatter{})
	appctl.SetAppStatus(appctlpb.AppStatus_STARTING)

	logFile, err := log.NewClientLogFile()
	if err == nil {
		log.SetOutput(logFile)
		if err = log.RemoveOldClientLogFiles(); err != nil {
			log.Errorf("remove old client log files failed: %v", err)
		}
	} else {
		log.Infof("log to stdout due to the following reason: %v", err)
	}

	// Load and verify client config.
	config, err := appctl.LoadClientConfig()
	if err != nil {
		if err == stderror.ErrFileNotExist {
			return fmt.Errorf(stderror.ClientConfigNotExist)
		} else {
			return fmt.Errorf(stderror.GetClientConfigFailedErr, err)
		}
	}
	if proto.Equal(config, &appctlpb.ClientConfig{}) {
		return fmt.Errorf(stderror.ClientConfigIsEmpty)
	}
	if err = appctl.ValidateFullClientConfig(config); err != nil {
		return fmt.Errorf(stderror.ValidateFullClientConfigFailedErr, err)
	}

	// Set logging level based on client config.
	loggingLevel := config.GetLoggingLevel().String()
	if loggingLevel != appctlpb.LoggingLevel_DEFAULT.String() {
		log.SetLevel(loggingLevel)
	}

	// Disable server side metrics.
	if serverDecryptionMetricGroup := metrics.GetMetricGroupByName(cipher.ServerDecryptionMetricGroupName); serverDecryptionMetricGroup != nil {
		serverDecryptionMetricGroup.DisableLogging()
	}

	var wg sync.WaitGroup

	// RPC port is allowed to set to 0. In that case, don't run RPC server.
	// When RPC server is not running, mieru commands can't be used to control the proxy client.
	// This mode is typically used by a mobile app, where the app controls the lifecycle of the proxy client.
	if config.GetRpcPort() != 0 {
		wg.Add(1)
		go func() {
			rpcAddr := "localhost:" + strconv.Itoa(int(config.GetRpcPort()))
			listenConfig := sockopts.ListenConfigWithControls()
			rpcListener, err := listenConfig.Listen(context.Background(), "tcp", rpcAddr)
			if err != nil {
				log.Fatalf("listen on RPC address tcp %q failed: %v", rpcAddr, err)
			}
			grpcServer := grpc.NewServer()
			appctl.SetClientRPCServerRef(grpcServer)
			appctlpb.RegisterClientLifecycleServiceServer(grpcServer, appctl.NewClientLifecycleService())
			close(appctl.ClientRPCServerStarted)
			log.Infof("mieru client RPC server is running")
			if err = grpcServer.Serve(rpcListener); err != nil {
				log.Fatalf("run gRPC server failed: %v", err)
			}
			log.Infof("mieru client RPC server is stopped")
			wg.Done()
		}()
		<-appctl.ClientRPCServerStarted
	}

	// Collect remote proxy addresses and password.
	mux := protocolv2.NewMux(true)
	appctl.SetClientMuxRef(mux)
	var hashedPassword []byte
	activeProfile, err := appctl.GetActiveProfileFromConfig(config, config.GetActiveProfile())
	if err != nil {
		return fmt.Errorf(stderror.ClientGetActiveProfileFailedErr, err)
	}
	user := activeProfile.GetUser()
	if user.GetHashedPassword() != "" {
		hashedPassword, err = hex.DecodeString(user.GetHashedPassword())
		if err != nil {
			return fmt.Errorf(stderror.DecodeHashedPasswordFailedErr, err)
		}
	} else {
		hashedPassword = cipher.HashPassword([]byte(user.GetPassword()), []byte(user.GetName()))
	}
	mux = mux.SetClientPassword(hashedPassword)
	mtu := util.DefaultMTU
	if activeProfile.GetMtu() != 0 {
		mtu = int(activeProfile.GetMtu())
	}
	multiplexFactor := 1
	switch activeProfile.GetMultiplexing().GetLevel() {
	case appctlpb.MultiplexingLevel_MULTIPLEXING_OFF:
		multiplexFactor = 0
	case appctlpb.MultiplexingLevel_MULTIPLEXING_LOW:
		multiplexFactor = 1
	case appctlpb.MultiplexingLevel_MULTIPLEXING_MIDDLE:
		multiplexFactor = 2
	case appctlpb.MultiplexingLevel_MULTIPLEXING_HIGH:
		multiplexFactor = 3
	}
	mux = mux.SetClientMultiplexFactor(multiplexFactor)
	endpoints := make([]protocolv2.UnderlayProperties, 0)
	resolver := &util.DNSResolver{}
	for _, serverInfo := range activeProfile.GetServers() {
		var proxyHost string
		var proxyIP net.IP
		if serverInfo.GetDomainName() != "" {
			proxyHost = serverInfo.GetDomainName()
			proxyIP, err = resolver.LookupIP(context.Background(), proxyHost)
			if err != nil {
				return fmt.Errorf(stderror.LookupIPFailedErr, err)
			}
		} else {
			proxyHost = serverInfo.GetIpAddress()
			proxyIP = net.ParseIP(proxyHost)
			if proxyIP == nil {
				return fmt.Errorf(stderror.ParseIPFailed)
			}
		}
		ipVersion := util.GetIPVersion(proxyIP.String())
		portBindings, err := appctl.FlatPortBindings(serverInfo.GetPortBindings())
		if err != nil {
			return fmt.Errorf(stderror.InvalidPortBindingsErr, err)
		}
		for _, bindingInfo := range portBindings {
			proxyPort := bindingInfo.GetPort()
			switch bindingInfo.GetProtocol() {
			case appctlpb.TransportProtocol_TCP:
				endpoint := protocolv2.NewUnderlayProperties(mtu, ipVersion, util.TCPTransport, nil, &net.TCPAddr{IP: proxyIP, Port: int(proxyPort)})
				endpoints = append(endpoints, endpoint)
			case appctlpb.TransportProtocol_UDP:
				endpoint := protocolv2.NewUnderlayProperties(mtu, ipVersion, util.UDPTransport, nil, &net.UDPAddr{IP: proxyIP, Port: int(proxyPort)})
				endpoints = append(endpoints, endpoint)
			default:
				return fmt.Errorf(stderror.InvalidTransportProtocol)
			}
		}
	}
	mux.SetEndpoints(endpoints)

	// Create the local socks5 server.
	var socks5IngressCredentials []socks5.Credential
	for _, auth := range config.GetSocks5Authentication() {
		socks5IngressCredentials = append(socks5IngressCredentials, socks5.Credential{
			User:     auth.GetUser(),
			Password: auth.GetPassword(),
		})
	}
	socks5Config := &socks5.Config{
		UseProxy: true,
		AuthOpts: socks5.Auth{
			ClientSideAuthentication: true,
			IngressCredentials:       socks5IngressCredentials,
		},
		ProxyMux:         mux,
		HandshakeTimeout: 10 * time.Second,
	}
	socks5Server, err := socks5.New(socks5Config)
	if err != nil {
		return fmt.Errorf(stderror.CreateSocks5ServerFailedErr, err)
	}
	appctl.SetClientSocks5ServerRef(socks5Server)

	// Run the local socks5 server in the background.
	var socks5Addr string
	if config.GetSocks5ListenLAN() {
		socks5Addr = util.MaybeDecorateIPv6(util.AllIPAddr()) + ":" + strconv.Itoa(int(config.GetSocks5Port()))
	} else {
		socks5Addr = util.MaybeDecorateIPv6(util.LocalIPAddr()) + ":" + strconv.Itoa(int(config.GetSocks5Port()))
	}
	wg.Add(1)
	go func(socks5Addr string) {
		listenConfig := sockopts.ListenConfigWithControls()
		l, err := listenConfig.Listen(context.Background(), "tcp", socks5Addr)
		if err != nil {
			log.Fatalf("listen on socks5 address tcp %q failed: %v", socks5Addr, err)
		}
		close(appctl.ClientSocks5ServerStarted)
		log.Infof("mieru client socks5 server is running")
		if err = socks5Server.Serve(l); err != nil {
			log.Fatalf("run socks5 server failed: %v", err)
		}
		log.Infof("mieru client socks5 server is stopped")
		wg.Done()
	}(socks5Addr)

	// If HTTP proxy is enabled, run the local HTTP server in the background.
	if config.GetHttpProxyPort() != 0 {
		// HTTP proxy is not compatible with socks5 user password authentication.
		if len(config.GetSocks5Authentication()) > 0 {
			log.Fatalf(`HTTP(S) proxy is not compatible with socks5 user password authentication. Please run "mieru delete socks5 authentication" to stop using user password authentication, or run "mieru delete http proxy" command to stop using HTTP(S) proxy.`)
		}
		wg.Add(1)
		go func(socks5Addr string) {
			var httpServerAddr string
			if config.GetHttpProxyListenLAN() {
				httpServerAddr = util.MaybeDecorateIPv6(util.AllIPAddr()) + ":" + strconv.Itoa(int(config.GetHttpProxyPort()))
			} else {
				httpServerAddr = util.MaybeDecorateIPv6(util.LocalIPAddr()) + ":" + strconv.Itoa(int(config.GetHttpProxyPort()))
			}
			httpServer := http2socks.NewHTTPServer(httpServerAddr, &http2socks.Proxy{
				ProxyURI: "socks5://" + socks5Addr + "?timeout=10s",
			})
			log.Infof("mieru client HTTP proxy server is running")
			wg.Done()
			if err := httpServer.ListenAndServe(); err != nil && !errors.Is(err, http.ErrServerClosed) {
				log.Fatalf("run HTTP proxy server failed: %v", err)
			}
		}(socks5Addr)
	}

	<-appctl.ClientSocks5ServerStarted
	metrics.EnableLogging()

	appctl.SetAppStatus(appctlpb.AppStatus_RUNNING)
	wg.Wait()

	// Stop CPU profiling, if previously started.
	pprof.StopCPUProfile()

	log.Infof("mieru client exit now")
	return nil
}

var clientStopFunc = func(s []string) error {
	ctx, cancelFunc := context.WithTimeout(context.Background(), appctl.RPCTimeout)
	defer cancelFunc()
	client, running, err := newClientLifecycleRPCClient(ctx)
	if !running {
		log.Infof(stderror.ClientNotRunning)
		return nil
	}
	if err != nil {
		return err
	}

	if _, err = client.Exit(ctx, &appctlpb.Empty{}); err != nil {
		return fmt.Errorf(stderror.ExitFailedErr, err)
	}
	log.Infof("mieru client is stopped")
	return nil
}

var clientStatusFunc = func(s []string) error {
	if err := appctl.IsClientDaemonRunning(context.Background()); err != nil {
		if stderror.IsConnRefused(err) {
			// This is the most common reason, no need to show more details.
			return fmt.Errorf(stderror.ClientNotRunning)
		} else if errors.Is(err, stderror.ErrFileNotExist) {
			// Ask the user to create a client config.
			return fmt.Errorf(stderror.ClientConfigNotExist + ", please create one with \"mieru apply config <FILE>\" command")
		} else {
			return fmt.Errorf(stderror.ClientNotRunningErr, err)
		}
	}
	log.Infof("mieru client is running")
	return nil
}

var clientTestFunc = func(s []string) error {
	if err := appctl.IsClientDaemonRunning(context.Background()); err != nil {
		return fmt.Errorf(stderror.ClientNotRunning)
	}
	config, err := appctl.LoadClientConfig()
	if err != nil {
		return fmt.Errorf(stderror.GetClientConfigFailedErr, err)
	}

	httpClient := &http.Client{
		Transport: &http.Transport{
			Dial: socks5client.Dial(fmt.Sprintf("socks5://127.0.0.1:%d", config.GetSocks5Port()), socks5client.ConnectCmd),
		},
		CheckRedirect: func(req *http.Request, via []*http.Request) error {
			return nil
		},
		Timeout: appctl.RPCTimeout,
	}

	destination := "https://google.com/generate_204"
	if len(s) == 3 {
		destination = s[2]
	}
	beginTime := time.Now()
	resp, err := httpClient.Get(destination)
	if err != nil {
		return err
	}
	endTime := time.Now()
	d := endTime.Sub(beginTime).Round(time.Millisecond)
	defer resp.Body.Close()
	io.ReadAll(resp.Body)

	if resp.StatusCode < 200 || resp.StatusCode >= 300 {
		return fmt.Errorf("received unexpected status code %d after %v", resp.StatusCode, d)
	}
	log.Infof("Connected to %q after %v", destination, d)
	return nil
}

var clientApplyConfigFunc = func(s []string) error {
	_, err := appctl.LoadClientConfig()
	if err == stderror.ErrFileNotExist {
		if err = appctl.StoreClientConfig(&appctlpb.ClientConfig{}); err != nil {
			return fmt.Errorf(stderror.StoreClientConfigFailedErr, err)
		}
	}
	return appctl.ApplyJSONClientConfig(s[3])
}

var clientDescribeConfigFunc = func(s []string) error {
	_, err := appctl.LoadClientConfig()
	if err == stderror.ErrFileNotExist {
		if err = appctl.StoreClientConfig(&appctlpb.ClientConfig{}); err != nil {
			return fmt.Errorf(stderror.StoreClientConfigFailedErr, err)
		}
	}
	out, err := appctl.GetJSONClientConfig()
	if err != nil {
		return fmt.Errorf(stderror.GetClientConfigFailedErr, err)
	}
	log.Infof("%s", out)
	return nil
}

var clientImportConfigFunc = func(s []string) error {
	_, err := appctl.LoadClientConfig()
	if err == stderror.ErrFileNotExist {
		if err = appctl.StoreClientConfig(&appctlpb.ClientConfig{}); err != nil {
			return fmt.Errorf(stderror.StoreClientConfigFailedErr, err)
		}
	}
	return appctl.ApplyURLClientConfig(s[3])
}

var clientExportConfigFunc = func(s []string) error {
	_, err := appctl.LoadClientConfig()
	if err == stderror.ErrFileNotExist {
		if err = appctl.StoreClientConfig(&appctlpb.ClientConfig{}); err != nil {
			return fmt.Errorf(stderror.StoreClientConfigFailedErr, err)
		}
	}
	out, err := appctl.GetURLClientConfig()
	if err != nil {
		return fmt.Errorf(stderror.GetClientConfigFailedErr, err)
	}
	log.Infof("%s", out)
	return nil
}

var clientDeleteProfileFunc = func(s []string) error {
	_, err := appctl.LoadClientConfig()
	if err != nil {
		return fmt.Errorf(stderror.GetClientConfigFailedErr, err)
	}
	return appctl.DeleteClientConfigProfile(s[3])
}

var clientDeleteHTTPProxyFunc = func(_ []string) error {
	config, err := appctl.LoadClientConfig()
	if err != nil {
		return fmt.Errorf(stderror.GetClientConfigFailedErr, err)
	}
	if config.HttpProxyPort == nil && config.HttpProxyListenLAN == nil {
		log.Infof("HTTP proxy is already deleted from client config.")
		return nil
	}
	config.HttpProxyPort = nil
	config.HttpProxyListenLAN = nil
	if err := appctl.StoreClientConfig(config); err != nil {
		return fmt.Errorf(stderror.StoreClientConfigFailedErr, err)
	}
	log.Infof("HTTP proxy is deleted from client config.")
	return nil
}

var clientDeleteSocks5AuthenticationFunc = func(_ []string) error {
	config, err := appctl.LoadClientConfig()
	if err != nil {
		return fmt.Errorf(stderror.GetClientConfigFailedErr, err)
	}
	if len(config.GetSocks5Authentication()) == 0 {
		log.Infof("socks5 user password authentication is already deleted from client config.")
		return nil
	}
	config.Socks5Authentication = nil
	if err := appctl.StoreClientConfig(config); err != nil {
		return fmt.Errorf(stderror.StoreClientConfigFailedErr, err)
	}
	log.Infof("socks5 user password authentication is deleted from client config.")
	return nil
}

var clientGetMetricsFunc = func(s []string) error {
	ctx, cancelFunc := context.WithTimeout(context.Background(), appctl.RPCTimeout)
	defer cancelFunc()
	client, running, err := newClientLifecycleRPCClient(ctx)
	if !running {
		return fmt.Errorf(stderror.ClientNotRunning)
	}
	if err != nil {
		return err
	}

	metrics, err := client.GetMetrics(ctx, &appctlpb.Empty{})
	if err != nil {
		return fmt.Errorf(stderror.GetMetricsFailedErr, err)
	}
	log.Infof("%s", metrics.GetJson())
	return nil
}

var clientGetConnectionsFunc = func(s []string) error {
	ctx, cancelFunc := context.WithTimeout(context.Background(), appctl.RPCTimeout)
	defer cancelFunc()
	client, running, err := newClientLifecycleRPCClient(ctx)
	if !running {
		return fmt.Errorf(stderror.ClientNotRunning)
	}
	if err != nil {
		return err
	}

	info, err := client.GetSessionInfo(ctx, &appctlpb.Empty{})
	if err != nil {
		return fmt.Errorf(stderror.GetConnectionsFailedErr, err)
	}
	for _, line := range info.GetTable() {
		log.Infof("%s", line)
	}
	return nil
}

var clientGetThreadDumpFunc = func(s []string) error {
	ctx, cancelFunc := context.WithTimeout(context.Background(), appctl.RPCTimeout)
	defer cancelFunc()
	client, running, err := newClientLifecycleRPCClient(ctx)
	if !running {
		return fmt.Errorf(stderror.ClientNotRunning)
	}
	if err != nil {
		return err
	}

	dump, err := client.GetThreadDump(ctx, &appctlpb.Empty{})
	if err != nil {
		return fmt.Errorf(stderror.GetThreadDumpFailedErr, err)
	}
	log.Infof("%s", dump.GetThreadDump())
	return nil
}

var clientGetHeapProfileFunc = func(s []string) error {
	ctx, cancelFunc := context.WithTimeout(context.Background(), appctl.RPCTimeout)
	defer cancelFunc()
	client, running, err := newClientLifecycleRPCClient(ctx)
	if !running {
		return fmt.Errorf(stderror.ClientNotRunning)
	}
	if err != nil {
		return err
	}

	if _, err := client.GetHeapProfile(ctx, &appctlpb.ProfileSavePath{FilePath: proto.String(s[3])}); err != nil {
		return fmt.Errorf(stderror.GetHeapProfileFailedErr, err)
	}
	log.Infof("heap profile is saved to %q", s[3])
	return nil
}

var clientGetMemoryStatisticsFunc = func(s []string) error {
	ctx, cancelFunc := context.WithTimeout(context.Background(), appctl.RPCTimeout)
	defer cancelFunc()
	client, running, err := newClientLifecycleRPCClient(ctx)
	if !running {
		return fmt.Errorf(stderror.ClientNotRunning)
	}
	if err != nil {
		return err
	}

	memStats, err := client.GetMemoryStatistics(ctx, &appctlpb.Empty{})
	if err != nil {
		return fmt.Errorf(stderror.GetMemoryStatisticsFailedErr, err)
	}
	log.Infof("%s", memStats.GetJson())
	return nil
}

var clientStartCPUProfileFunc = func(s []string) error {
	ctx, cancelFunc := context.WithTimeout(context.Background(), appctl.RPCTimeout)
	defer cancelFunc()
	client, running, err := newClientLifecycleRPCClient(ctx)
	if !running {
		return fmt.Errorf(stderror.ClientNotRunning)
	}
	if err != nil {
		return err
	}

	if _, err := client.StartCPUProfile(ctx, &appctlpb.ProfileSavePath{FilePath: proto.String(s[4])}); err != nil {
		return fmt.Errorf(stderror.StartCPUProfileFailedErr, err)
	}
	log.Infof("CPU profile will be saved to %q", s[4])
	return nil
}

var clientStopCPUProfileFunc = func(s []string) error {
	ctx, cancelFunc := context.WithTimeout(context.Background(), appctl.RPCTimeout)
	defer cancelFunc()
	client, running, err := newClientLifecycleRPCClient(ctx)
	if !running {
		return fmt.Errorf(stderror.ClientNotRunning)
	}
	if err != nil {
		return err
	}

	client.StopCPUProfile(ctx, &appctlpb.Empty{})
	return nil
}

// newClientLifecycleRPCClient returns a new client lifecycle RPC client.
// No RPC client is returned if mieru is not running.
func newClientLifecycleRPCClient(ctx context.Context) (client appctlpb.ClientLifecycleServiceClient, running bool, err error) {
	if err := appctl.IsClientDaemonRunning(ctx); err != nil {
		return nil, false, nil
	}
	running = true
	client, err = appctl.NewClientLifecycleRPCClient(ctx)
	if err != nil {
		return nil, true, fmt.Errorf(stderror.CreateClientLifecycleRPCClientFailedErr, err)
	}
	return
}
