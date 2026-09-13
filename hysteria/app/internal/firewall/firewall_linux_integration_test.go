//go:build linux

package firewall

import (
	"bufio"
	"fmt"
	"net"
	"os"
	"os/exec"
	"strings"
	"testing"

	eUtils "github.com/apernet/hysteria/extras/v2/utils"
	"github.com/stretchr/testify/require"
)

// Run an already compiled test binary as root with
// HYSTERIA_FIREWALL_INTEGRATION=1. All rules, interfaces, and routes are confined
// to temporary network namespaces; the host's firewall is never modified.
func TestUDPPortRedirectIntegration(t *testing.T) {
	if os.Getenv("HYSTERIA_FIREWALL_INTEGRATION") != "1" {
		t.Skip("set HYSTERIA_FIREWALL_INTEGRATION=1 to run privileged network namespace tests")
	}
	require.Equal(t, 0, os.Geteuid(), "network namespaces require root")
	for _, tool := range []string{"ip", "nft", "iptables", "ip6tables", "python3", "sysctl"} {
		_, err := exec.LookPath(tool)
		require.NoError(t, err)
	}
	for _, backend := range []string{"nftables", "iptables"} {
		t.Run(backend, func(t *testing.T) {
			t.Setenv(firewallBackendEnv, backend)
			for _, bind := range []string{"", "192.0.2.1", "fd00:1::1"} {
				name := bind
				if name == "" {
					name = "wildcard"
				}
				t.Run(name, func(t *testing.T) {
					server, left, right := firewallTestNamespaces(t)
					startFirewallEcho(t, server, "41000", "server")
					startFirewallEcho(t, right, "41001", "remote")
					runner := namespaceRunner{namespace: server}
					cleanup, err := setupUDPPortRedirectWithRunner(runner,
						&net.UDPAddr{IP: net.ParseIP(bind), Port: 41000},
						eUtils.PortUnion{{41000, 41002}})
					require.NoError(t, err)
					t.Cleanup(func() { require.NoError(t, cleanup.Close()) })

					for _, tc := range []struct{ local, remote, loopback string }{
						{"192.0.2.1", "198.51.100.2", "127.0.0.1"},
						{"fd00:1::1", "fd00:2::2", "::1"},
					} {
						// The same destination port must still reach a remote UDP
						// service via both OUTPUT and forwarded PREROUTING traffic.
						firewallProbe(t, server, tc.remote, "41001", "remote")
						firewallProbe(t, left, tc.remote, "41001", "remote")
						if bind != "" && bind != tc.local {
							continue
						}
						// Incoming port hopping, including both ends of the range.
						for _, port := range []string{"41000", "41001", "41002"} {
							firewallProbe(t, left, tc.local, port, "server")
							firewallProbe(t, server, tc.local, port, "server")
						}
						if bind == "" {
							firewallProbe(t, server, tc.loopback, "41001", "server")
						}
					}
					require.NoError(t, cleanup.Close())
					for _, cmd := range [][]string{{"nft", "list", "ruleset"}, {"iptables-save"}, {"ip6tables-save"}} {
						out := firewallNSCommand(t, server, cmd...)
						require.NotContains(t, strings.ToLower(out), "hysteria", "cleanup must remove all generated rules")
					}
				})
			}
		})
	}
}

type namespaceRunner struct{ namespace string }

func (r namespaceRunner) LookPath(name string) (string, error) { return exec.LookPath(name) }

func (r namespaceRunner) Run(name string, args ...string) error {
	out, err := exec.Command("ip", append([]string{"netns", "exec", r.namespace, name}, args...)...).CombinedOutput()
	if err != nil {
		return fmt.Errorf("%s %v: %w: %s", name, args, err, out)
	}
	return nil
}

func firewallCommand(t *testing.T, args ...string) string {
	t.Helper()
	out, err := exec.Command(args[0], args[1:]...).CombinedOutput()
	require.NoError(t, err, "%v: %s", args, out)
	return string(out)
}

func firewallNSCommand(t *testing.T, ns string, args ...string) string {
	t.Helper()
	return firewallCommand(t, append([]string{"ip", "netns", "exec", ns}, args...)...)
}

func firewallTestNamespaces(t *testing.T) (server, left, right string) {
	t.Helper()
	// Tests run sequentially, and cleanup completes before these names are reused.
	prefix := fmt.Sprintf("hy-fw-%d-", os.Getpid())
	server, left, right = prefix+"s", prefix+"l", prefix+"r"
	for _, ns := range []string{server, left, right} {
		firewallCommand(t, "ip", "netns", "add", ns)
		t.Cleanup(func() { firewallCommand(t, "ip", "netns", "del", ns) })
		firewallCommand(t, "ip", "-n", ns, "link", "set", "lo", "up")
	}
	for _, link := range []struct{ ns, iface, local4, peer4, local6, peer6 string }{
		{left, "left", "192.0.2.1/24", "192.0.2.2/24", "fd00:1::1/64", "fd00:1::2/64"},
		{right, "right", "198.51.100.1/24", "198.51.100.2/24", "fd00:2::1/64", "fd00:2::2/64"},
	} {
		firewallCommand(t, "ip", "-n", server, "link", "add", link.iface, "type", "veth", "peer", "name", "eth0", "netns", link.ns)
		for _, end := range []struct{ ns, iface, v4, v6 string }{
			{server, link.iface, link.local4, link.local6}, {link.ns, "eth0", link.peer4, link.peer6},
		} {
			firewallCommand(t, "ip", "-n", end.ns, "addr", "add", end.v4, "dev", end.iface)
			firewallCommand(t, "ip", "-n", end.ns, "-6", "addr", "add", end.v6, "dev", end.iface, "nodad")
			firewallCommand(t, "ip", "-n", end.ns, "link", "set", end.iface, "up")
		}
	}
	firewallNSCommand(t, server, "sysctl", "-qw", "net.ipv4.ip_forward=1", "net.ipv6.conf.all.forwarding=1")
	firewallCommand(t, "ip", "-n", left, "route", "add", "default", "via", "192.0.2.1")
	firewallCommand(t, "ip", "-n", right, "route", "add", "default", "via", "198.51.100.1")
	firewallCommand(t, "ip", "-n", left, "-6", "route", "add", "default", "via", "fd00:1::1")
	firewallCommand(t, "ip", "-n", right, "-6", "route", "add", "default", "via", "fd00:2::1")
	return server, left, right
}

func startFirewallEcho(t *testing.T, ns, port, label string) {
	t.Helper()
	const script = `import socket,select,sys
sockets=[]
for family,host in [(socket.AF_INET,'0.0.0.0'),(socket.AF_INET6,'::')]:
 s=socket.socket(family,socket.SOCK_DGRAM)
 if family==socket.AF_INET6: s.setsockopt(socket.IPPROTO_IPV6,socket.IPV6_V6ONLY,1)
 s.bind((host,int(sys.argv[1]))); sockets.append(s)
print('ready',flush=True)
while True:
 for s in select.select(sockets,[],[])[0]:
  data,addr=s.recvfrom(1024); s.sendto(sys.argv[2].encode()+b':'+data,addr)
`
	cmd := exec.Command("ip", "netns", "exec", ns, "python3", "-u", "-c", script, port, label)
	cmd.Stderr = os.Stderr
	stdout, err := cmd.StdoutPipe()
	require.NoError(t, err)
	require.NoError(t, cmd.Start())
	t.Cleanup(func() {
		_ = cmd.Process.Kill()
		_ = cmd.Wait()
	})
	scanner := bufio.NewScanner(stdout)
	require.True(t, scanner.Scan(), "UDP echo helper failed to start")
	require.Equal(t, "ready", scanner.Text())
}

func firewallProbe(t *testing.T, ns, host, port, label string) {
	t.Helper()
	const script = `import socket,sys
s=socket.socket(socket.AF_INET6 if ':' in sys.argv[1] else socket.AF_INET,socket.SOCK_DGRAM)
s.settimeout(3)
s.sendto(b'probe',(sys.argv[1],int(sys.argv[2])))
print(s.recv(1024).decode())
`
	out := firewallNSCommand(t, ns, "python3", "-c", script, host, port)
	require.Equal(t, label+":probe", strings.TrimSpace(out), "%s -> %s:%s", ns, host, port)
}
