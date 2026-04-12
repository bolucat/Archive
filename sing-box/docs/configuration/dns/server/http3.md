---
icon: material/new-box
---

!!! quote "Changes in sing-box 1.14.0"

    :material-alert: `headers`, `tls`, Dial Fields moved to [HTTP Client Fields](#http-client-fields)

!!! question "Since sing-box 1.12.0"

# DNS over HTTP3 (DoH3)

### Structure

```json
{
  "dns": {
    "servers": [
      {
        "type": "h3",
        "tag": "",

        "server": "",
        "server_port": 0,

        "path": "",
        "method": "",

        ... // HTTP Client Fields
      }
    ]
  }
}
```

### Fields

#### server

==Required==

The address of the DNS server.

If domain name is used, `domain_resolver` must also be set to resolve IP address.

#### server_port

The port of the DNS server.

`443` will be used by default.

#### path

The path of the DNS server.

`/dns-query` will be used by default.

#### method

HTTP request method.

Available values: `GET`, `POST`.

`POST` will be used by default.

### HTTP Client Fields

See [HTTP Client Fields](/configuration/shared/http-client/) for details.
