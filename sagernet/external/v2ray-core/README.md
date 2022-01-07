# Project V for SagerNet

### Important changes

- concurrency option for outbound observation

```json
{
  "observatory": {
    "enableConcurrency": true
  }
}
```

- DNS sniffer

```json
{
  "routing": {
    "rules": [
      {
        "type": "field",
        "protocol": "dns",
        "outbound": "dns-out"
      }
    ]
  }
}
```

- disableExpire dns option

```json
{
  "dns": {
    "disableExpire": true
  }
}
```

- udp+local dns server

```json
{
  "dns": {
    "servers": [
      "udp+local://8.8.8.8"
      // without routing rule
    ]
  }
}
```

- removed FakeDNS

```
FakeDNS is a bad idea, and v2ray's current implementation
causes memory leaks, whether enabled or not.
```

- wireguard outbound

```json
{
  "outbounds": [
    {
      "protocol": "wireguard",
      "settings": {
        "address": "engage.cloudflareclient.com",
        "localAddresses": [
          "<ipv4 address>",
          "<ipv6 address>"
        ],
        "peerPublicKey": "<public key>",
        "port": 2408,
        "preSharedKey": "<psk>",
        "privateKey": "<private key>",
        "mtu": 1500,
        "userLevel": 0
      }
    }
  ]
}
```

- ssh outbound

```json
{
  "outbounds": [
    {
      "protocol": "ssh",
      "settings": {
        "address": "<your ip>",
        "port": 22,
        "user": "root",
        "password": "<password or passphrase of private key>",
        "privateKey": "<x509 private key>",
        "publicKey": "<public key to verify server>",
        "clientVersion": "SSH-2.0-OpenSSH_114514 (random if empty)",
        "hostKeyAlgorithms": [
          "ssh-ed25519",
          "any u want"
        ],
        "userLevel": 0
      }
    }
  ]
}
```

- add domainStrategy to outbound & preferIPv4/6 to domainStrategy

```json
{
  "outbounds": [
    {
      "protocol": "shadowsocks",
      "settings": {
        ...
      },
      "domainStrategy": "AsIs/UseIP/UseIPv[4/6]/PreferIPv[4/6]"
    }
  ]
}
```

- shadowsocks stream ciphers and xchacha-ietf-poly1305

```
supported cipher list:

none
aes-128-gcm
aes-192-gcm
aes-256-gcm
chacha20-ietf-poly1305
xchacha20-ietf-poly1305
rc4
rc4-md5
aes-128-ctr
aes-192-ctr
aes-256-ctr
aes-128-cfb
aes-192-cfb
aes-256-cfb
aes-128-cfb8
aes-192-cfb8
aes-256-cfb8
aes-128-ofb
aes-192-ofb
aes-256-ofb
bf-cfb
cast5-cfb
des-cfb
idea-cfb
rc2-cfb
seed-cfb
camellia-128-cfb
camellia-192-cfb
camellia-256-cfb
camellia-128-cfb8
camellia-192-cfb8
camellia-256-cfb8
salsa20
chacha20
chacha20-ietf
xchacha20
```

- shadowsocks SIP008 plugin

```json
{
  "outbounds": [
    {
      "protocol": "shadowsocks",
      "settings": {
        ...
        "plugin": "path to plugin",
        "pluginOpts": "args;args2",
        "pluginArgs": [
          "--arg1=true"
        ]
      }
    }
  ]
}
```

- embed v2ray-plugin for shadowsocks

```json
{
  "outbounds": [
    {
      "protocol": "shadowsocks",
      "settings": {
        ...
        "plugin": "v2ray-plugin",
        "pluginOpts": "host=shadow.v2fly.org"
      }
    }
  ]
}
```

- route only sniffing option

```
Allows the sniffed domain to be used for routing only, 
without overriding the destination address. 
This improves the routing accuracy of AsIs, 
and provides the expected connection behavior of the client 
(not resolving the domain name again on the server side)
```

```json
{
  "inbounds": [
    {
      ...
      "sniffing": {
        "destOverride": [
          "http",
          "tls",
          "quic"
        ],
        "enabled": true,
        "routeOnly": true
      },
      "tag": "socks"
    }
  ]
}
```

- endpoint independent mapping support (aka full cone NAT)

`for protocols other than v*ess, no configuration is required.`

```json
{
  "outbounds": [
    {
      "protocol": "v[m/l]ess",
      "settings": {
        "vnext": ...,
        "packetEncoding": "[none/packet/xudp]"
        // none: disabled
        // packet: requires v2ray/v2ray-core v5.0.2+ or SagerNet/v2ray-core
        // xudp: requires XTLS/Xray-core or SagerNet/v2ray-core
      },
      "mux": {
        "enabled": true,
        "packetEncoding": "[none/packet/xudp]" // packetEncoding for mux
      }
    }
  ]
}
```

### License

[GPL v3](https://raw.githubusercontent.com/SagerNet/v2ray-core/main/LICENSE)

### Credits

This repo relies on the following projects:

- [v2fly/v2ray-core](https://github.com/v2fly/v2ray-core)
- [XTLS/Xray-core](https://github.com/XTLS/Xray-core)
- [Shadowsocks-NET/v2ray-go](https://github.com/Shadowsocks-NET/v2ray-go)