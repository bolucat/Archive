---
icon: material/new-box
---

!!! question "Since sing-box 1.14.0"

### Structure

A string or an object.

When string, the tag of a shared [HTTP Client](/configuration/shared/http-client/) defined in top-level `http_clients`.

When object:

```json
{
  "version": 0,
  "disable_version_fallback": false,
  "headers": {},

  ... // HTTP2 Fields

  "tls": {},

  ... // Dial Fields
}
```

### Fields

#### version

HTTP version.

Available values: `1`, `2`, `3`.

`2` is used by default.

When `3`, [HTTP2 Fields](#http2-fields) are replaced by [QUIC Fields](#quic-fields).

#### disable_version_fallback

Disable automatic fallback to lower HTTP version.

#### headers

Custom HTTP headers.

`Host` header is used as request host.

### HTTP2 Fields

When `version` is `2` (default).

See [HTTP2 Fields](/configuration/shared/http2/) for details.

### QUIC Fields

When `version` is `3`.

See [QUIC Fields](/configuration/shared/quic/) for details.

### TLS Fields

See [TLS](/configuration/shared/tls/#outbound) for details.

### Dial Fields

See [Dial Fields](/configuration/shared/dial/) for details.
