\page ss-nat ss-nat

\brief helper script to setup NAT rules for transparent proxy

\section ss_nat_synopsis SYNOPSIS

`ss-nat [options]`

\section ss_nat_description DESCRIPTION

*shadowsocks-c* is a lightweight and secure socks5 proxy.
It is a port of the original shadowsocks created by clowwindy.
*shadowsocks-c* is written in pure C and takes advantage of libuv to
achieve both high performance and low resource consumption.

ss-nat(1) sets up NAT rules for ss-redir(1) to provide traffic redirection.
It requires netfilter's NAT module and iptables(8).
For more information, check out shadowsocks-libev(8) and the following
`EXAMPLE` section.

\section ss_nat_options OPTIONS

Options include all supported platform variants; restrictions are noted below.

\snippet{doc} ss-nat cli-options

\section ss_nat_example EXAMPLE

`ss-nat` requires iptables(8). Here is an example:

```
# Enable NAT rules for shadowsocks,
# with both TCP and UDP redirection enabled,
# and applied for both PREROUTING and OUTPUT chains
root@Wrt:~# ss-nat -s 192.168.1.100 -l 1080 -u -o

# Disable and flush all NAT rules for shadowsocks
root@Wrt:~# ss-nat -f
```

\section ss_nat_see_also SEE ALSO

ss-local(1),
ss-server(1),
ss-tunnel(1),
ss-manager(1),
shadowsocks-libev(8),
iptables(8),
/etc/shadowsocks-libev/config.json
