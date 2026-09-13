//go:build linux

package firewall

import (
	"errors"
	"net"
	"strings"
	"testing"

	eUtils "github.com/apernet/hysteria/extras/v2/utils"
	"github.com/stretchr/testify/require"
)

type fakeRunner struct {
	paths map[string]bool
	cmds  [][]string
	fail  int
}

func (r *fakeRunner) LookPath(file string) (string, error) {
	if r.paths[file] {
		return "/usr/sbin/" + file, nil
	}
	return "", errors.New("not found")
}

func (r *fakeRunner) Run(name string, args ...string) error {
	r.cmds = append(r.cmds, append([]string{name}, args...))
	if r.fail > 0 && len(r.cmds) == r.fail {
		return errors.New("boom")
	}
	return nil
}

func TestSetupUDPPortRedirectWithRunnerNFTables(t *testing.T) {
	runner := &fakeRunner{paths: map[string]bool{"nft": true}}
	addr := &net.UDPAddr{IP: net.IPv4(1, 2, 3, 4), Port: 20000}
	ports := eUtils.PortUnion{{20000, 20002}}

	cleanup, err := setupUDPPortRedirectWithRunner(runner, addr, ports)
	require.NoError(t, err)
	require.NotNil(t, cleanup)
	require.Contains(t, runner.cmds[0], "add")
	require.Contains(t, runner.cmds[0], "table")
	require.Contains(t, runner.cmds[3], "udp")
	require.Contains(t, runner.cmds[3], "dport")
	require.Contains(t, runner.cmds[3], "20001-20002")
	require.Contains(t, runner.cmds[3], ":20000")

	require.NoError(t, cleanup.Close())
	require.Contains(t, runner.cmds[len(runner.cmds)-1], "delete")
}

func TestSetupUDPPortRedirectWithRunnerIPTablesFallback(t *testing.T) {
	runner := &fakeRunner{paths: map[string]bool{"iptables": true}}
	addr := &net.UDPAddr{IP: net.IPv4(1, 2, 3, 4), Port: 20000}
	ports := eUtils.PortUnion{{20000, 20000}, {20002, 20003}}

	cleanup, err := setupUDPPortRedirectWithRunner(runner, addr, ports)
	require.NoError(t, err)
	require.NotNil(t, cleanup)
	require.Equal(t, "iptables", runner.cmds[0][0])
	require.Contains(t, runner.cmds[0], "-N")
	require.Contains(t, runner.cmds[1], "REDIRECT")
	require.Contains(t, runner.cmds[2], "PREROUTING")
	require.Contains(t, runner.cmds[2], "20002:20003")

	require.NoError(t, cleanup.Close())
	foundDelete := false
	for _, cmd := range runner.cmds {
		for _, arg := range cmd {
			if arg == "-D" {
				foundDelete = true
				break
			}
		}
	}
	require.True(t, foundDelete)
}

func TestSetupUDPPortRedirectWithRunnerNFTablesIPv6Specific(t *testing.T) {
	runner := &fakeRunner{paths: map[string]bool{"nft": true}}
	addr := &net.UDPAddr{IP: net.ParseIP("2001:db8::1"), Port: 20000}
	ports := eUtils.PortUnion{{20000, 20002}}

	cleanup, err := setupUDPPortRedirectWithRunner(runner, addr, ports)
	require.NoError(t, err)
	require.NotNil(t, cleanup)

	for _, cmd := range runner.cmds {
		hasUDP := false
		for _, arg := range cmd {
			if arg == "udp" {
				hasUDP = true
				break
			}
		}
		if !hasUDP {
			continue
		}
		hasDnat := false
		hasRedirect := false
		for _, arg := range cmd {
			if arg == "dnat" {
				hasDnat = true
			}
			if arg == "redirect" {
				hasRedirect = true
			}
		}
		require.True(t, hasDnat, "IPv6 specific address rule should use dnat: %v", cmd)
		require.False(t, hasRedirect, "IPv6 specific address rule must not use redirect: %v", cmd)
		require.Contains(t, cmd, "[2001:db8::1]:20000")
	}

	require.NoError(t, cleanup.Close())
}

func TestSetupUDPPortRedirectWithRunnerNFTablesIPv6Unspecified(t *testing.T) {
	runner := &fakeRunner{paths: map[string]bool{"nft": true}}
	addr := &net.UDPAddr{IP: net.IPv6unspecified, Port: 20000}
	ports := eUtils.PortUnion{{20000, 20002}}

	cleanup, err := setupUDPPortRedirectWithRunner(runner, addr, ports)
	require.NoError(t, err)
	require.NotNil(t, cleanup)

	for _, cmd := range runner.cmds {
		hasDnat := false
		for _, arg := range cmd {
			if arg == "dnat" {
				hasDnat = true
				break
			}
		}
		require.False(t, hasDnat, "IPv6 unspecified address rule must not use dnat: %v", cmd)
	}

	require.NoError(t, cleanup.Close())
}

func TestSetupUDPPortRedirectWithRunnerIPTablesIPv6Specific(t *testing.T) {
	runner := &fakeRunner{paths: map[string]bool{"ip6tables": true}}
	addr := &net.UDPAddr{IP: net.ParseIP("2001:db8::1"), Port: 20000}
	ports := eUtils.PortUnion{{20000, 20002}}

	cleanup, err := setupUDPPortRedirectWithRunner(runner, addr, ports)
	require.NoError(t, err)
	require.NotNil(t, cleanup)

	foundTarget := false
	for _, cmd := range runner.cmds {
		hasDNAT := false
		hasREDIRECT := false
		for _, arg := range cmd {
			if arg == "DNAT" {
				hasDNAT = true
			}
			if arg == "REDIRECT" {
				hasREDIRECT = true
			}
		}
		require.False(t, hasREDIRECT, "IPv6 specific address rule must not use REDIRECT: %v", cmd)
		if hasDNAT {
			foundTarget = true
			require.Contains(t, cmd, "[2001:db8::1]:20000")
		}
	}
	require.True(t, foundTarget, "expected a DNAT rule for specific IPv6 bind")

	require.NoError(t, cleanup.Close())
}

func TestSetupUDPPortRedirectWithRunnerIPTablesIPv6Unspecified(t *testing.T) {
	runner := &fakeRunner{paths: map[string]bool{"iptables": true, "ip6tables": true}}
	addr := &net.UDPAddr{IP: net.IPv6unspecified, Port: 20000}
	ports := eUtils.PortUnion{{20000, 20002}}

	cleanup, err := setupUDPPortRedirectWithRunner(runner, addr, ports)
	require.NoError(t, err)
	require.NotNil(t, cleanup)

	for _, cmd := range runner.cmds {
		for _, arg := range cmd {
			require.NotEqual(t, "DNAT", arg, "unspecified address rule must not use DNAT: %v", cmd)
		}
	}

	require.NoError(t, cleanup.Close())
}

func TestSetupUDPPortRedirectWithRunnerIPTablesIPv4Specific(t *testing.T) {
	runner := &fakeRunner{paths: map[string]bool{"iptables": true}}
	addr := &net.UDPAddr{IP: net.IPv4(1, 2, 3, 4), Port: 20000}
	ports := eUtils.PortUnion{{20000, 20002}}

	cleanup, err := setupUDPPortRedirectWithRunner(runner, addr, ports)
	require.NoError(t, err)
	require.NotNil(t, cleanup)

	foundRedirect := false
	for _, cmd := range runner.cmds {
		for _, arg := range cmd {
			require.NotEqual(t, "DNAT", arg, "IPv4 rule must keep REDIRECT, not DNAT: %v", cmd)
			if arg == "REDIRECT" {
				foundRedirect = true
			}
		}
	}
	require.True(t, foundRedirect, "expected REDIRECT rule for IPv4 bind")

	require.NoError(t, cleanup.Close())
}

func TestSetupUDPPortRedirectWithRunnerRollback(t *testing.T) {
	runner := &fakeRunner{
		paths: map[string]bool{"iptables": true},
		fail:  3,
	}
	addr := &net.UDPAddr{IP: net.IPv4(1, 2, 3, 4), Port: 20000}
	ports := eUtils.PortUnion{{20000, 20001}}

	_, err := setupUDPPortRedirectWithRunner(runner, addr, ports)
	require.Error(t, err)
	require.Contains(t, runner.cmds[len(runner.cmds)-1], "-X")
}

// Wildcard listeners must only capture traffic addressed to this host, including
// locally generated traffic. Explicit binds must retain their address filter.
func TestUDPPortRedirectDestinationScope(t *testing.T) {
	for _, backend := range []string{"nftables", "iptables"} {
		t.Run(backend, func(t *testing.T) {
			t.Setenv(firewallBackendEnv, backend)
			for _, tc := range []struct {
				name string
				ip   net.IP
			}{
				{"implicit", nil},
				{"wildcard4", net.IPv4zero},
				{"wildcard6", net.IPv6unspecified},
				{"specific4", net.ParseIP("192.0.2.1")},
				{"specific6", net.ParseIP("2001:db8::1")},
			} {
				t.Run(tc.name, func(t *testing.T) {
					runner := &fakeRunner{paths: map[string]bool{"nft": true, "iptables": true, "ip6tables": true}}
					cleanup, err := setupUDPPortRedirectWithRunner(runner,
						&net.UDPAddr{IP: tc.ip, Port: 443}, eUtils.PortUnion{{443, 443}, {20000, 60000}})
					require.NoError(t, err)
					wildcard := tc.ip == nil || tc.ip.IsUnspecified()
					rules := 0
					var addedIPTablesRules []string
					for _, cmd := range runner.cmds {
						line := strings.Join(cmd, " ")
						if !strings.Contains(line, "dport") {
							continue
						}
						rules++
						if backend == "nftables" {
							if wildcard {
								require.Contains(t, line, "fib daddr type local udp dport")
							} else {
								require.Contains(t, line, "daddr "+tc.ip.String()+" udp dport")
								require.NotContains(t, line, "fib")
							}
						} else {
							if wildcard {
								require.Contains(t, line, "-m addrtype --dst-type LOCAL -p udp")
							} else {
								require.Contains(t, line, "-d "+tc.ip.String()+" -p udp")
								require.NotContains(t, line, "addrtype")
							}
							addedIPTablesRules = append(addedIPTablesRules, line)
						}
					}
					if wildcard {
						require.Equal(t, 4, rules, "both hooks and both IP families")
					} else {
						require.Equal(t, 2, rules, "both hooks for the bound IP family")
					}
					n := len(runner.cmds)
					require.NoError(t, cleanup.Close())
					var deleted []string
					for _, cmd := range runner.cmds[n:] {
						deleted = append(deleted, strings.Join(cmd, " "))
					}
					for _, added := range addedIPTablesRules {
						require.Contains(t, deleted, strings.Replace(added, " -A ", " -D ", 1))
					}
				})
			}
		})
	}
}
