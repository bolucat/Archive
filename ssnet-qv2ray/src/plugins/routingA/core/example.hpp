#pragma once

constexpr auto example_data = R"(
# customize inbound. we support http, socks
# 自定义入站 inbound，支持http, socks

stream:streamA=ws(test1, test2, test5, path: "/")

inbound:httpauthin=http(address: 0.0.0.0, port: 1081, user: user1, pass: user1pass, user:user2, pass:user2pass)
inbound:socksauthin=socks(address: 0.0.0.0, port: 1082, user: 123, pass: 123)
inbound:sockslocalin=socks(address: 127.0.0.1, port: 1080)

# customize outbound. we support http,socks,freedom
# 自定义出站 outbound，支持http, socks, freedom
outbound:httpout=http(address: 127.0.0.1, port: 8080, user: 'my-username', pass: 'my-password')
outbound:socksout=socks(address: 127.0.0.1, port: 10800, user: "my-username", pass: "my-password")
outbound:special=freedom(domainStrategy: AsIs, redirect: "127.0.0.1:3366", userLevel: 0)

# set default outbound. unsetting it means proxy (this option only applies to preset inbounds)
# 设置默认outbound，不设置则默认为proxy （该选项只作用于默认入站）
default: httpout

# proxy, block and direct are preset outbounds
# 预设三个outbounds: proxy, block, direct

# domain rule
# 域名规则
domain(domain: v2raya.mzz.pub) -> socksout
domain(full: dns.google) -> proxy
domain(contains: facebook) -> proxy
domain(regexp: \.goo.*\.com$) -> proxy
domain(geosite:category-ads) -> block
domain(geosite:cn)->direct
# target IP rule
# 目的IP规则
ip(8.8.8.8) -> direct
ip(101.97.0.0/16) -> direct
ip(geoip:private) -> direct
# source IP rule
# 源IP规则
source(192.168.0.0/24) -> proxy
source(192.168.50.0/24) -> direct

# multiple domains
# 多域名规则
domain(contains: google, domain: www.twitter.com, domain: mzz.pub) -> proxy
# multiple IPs
# 多IP规则
ip(geoip:cn, geoip:private) -> direct
ip(9.9.9.9, 223.5.5.5) -> direct
source(192.168.0.6, 192.168.0.10, 192.168.0.15) -> direct

# inbound rule
# inbound 入站规则
inboundTag(httpauthin, socksauthin) -> direct
inboundTag(sockslocalin) -> special

# AND rule
# 同时满足规则
ip(geoip:cn) && port(80) && user(mzz2017@tuta.io) -> direct
ip(8.8.8.8) && network(tcp, udp) && port(1-1023, 8443) -> proxy
ip(1.1.1.1) && protocol(http) && source(10.0.0.1, 172.20.0.0/16) -> direct

)";
