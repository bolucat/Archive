RFC: The Hysteria 2 Protocol Specification
Category: Informational
Date: April 2026

# The Hysteria 2 Protocol Specification

## Abstract

Hysteria is a TCP and UDP proxy protocol based on QUIC, designed for speed, security, and censorship resistance. This document describes the protocol used by Hysteria starting with version 2.0.0 (internally referred to as the "v4" protocol). It details the underlying wire format, the HTTP/3 masquerading mechanism for authentication, proxy request multiplexing, congestion control signaling, and an optional obfuscation layer.

## 1. Introduction

The Hysteria protocol leverages QUIC to provide a secure and multiplexed proxy connection. By masquerading its initial authentication as standard HTTP/3 traffic, it aims to thwart active probing and Deep Packet Inspection (DPI) heuristics. Once authenticated, the protocol multiplexes TCP streams and datagrams (UDP) over the established QUIC connection.

## 2. Conventions and Terminology

The key words "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT", "SHOULD", "SHOULD NOT", "RECOMMENDED", "MAY", and "OPTIONAL" in this document are to be interpreted as described in [RFC 2119].

## 3. Underlying Protocol & Wire Format

The Hysteria protocol MUST be implemented on top of the standard QUIC transport protocol [RFC 9000] and MUST support the Unreliable Datagram Extension [RFC 9221].

- **Byte Order:** All multibyte numbers use Big Endian format.
- **Variable-Length Integers:** All variable-length integers ("varints") are encoded and decoded as defined in Section 16 of QUIC [RFC 9000].

## 4. Authentication & HTTP/3 Masquerading

A core feature of the Hysteria protocol is that to an unauthenticated observer (a middlebox or active prober), a Hysteria proxy server behaves exactly like a standard HTTP/3 web server. The encrypted traffic appears indistinguishable from normal HTTP/3 traffic.

Therefore, a Hysteria server MUST implement an HTTP/3 server (as defined by [RFC 9114]) and handle standard HTTP requests normally. To prevent pattern detection, servers SHOULD host actual content or function as a reverse proxy for other web services.

### 4.1. Client Authentication Request

Upon establishing the QUIC connection, an actual Hysteria client MUST send a specific HTTP/3 request to the server:

```http
:method: POST
:path: /auth
:host: hysteria
Hysteria-Auth: [string]
Hysteria-CC-RX: [uint]
Hysteria-Padding: [string]
```

- `Hysteria-Auth`: Authentication credentials.
- `Hysteria-CC-RX`: The client's maximum receive rate in bytes per second. A value of `0` indicates the rate is unknown.
- `Hysteria-Padding`: A randomly generated padding string of variable length used for obfuscation.

### 4.2. Server Authentication Response

The Hysteria server MUST identify this special request. Instead of serving content or forwarding it upstream, it MUST attempt to authenticate the client using the provided credentials.

If authentication is successful, the server MUST send the following HTTP/3 response:

```http
:status: 233 HyOK
Hysteria-UDP: [true/false]
Hysteria-CC-RX: [uint/"auto"]
Hysteria-Padding: [string]
```

- `:status`: MUST be exactly `233`.
- `Hysteria-UDP`: Indicates whether the server supports UDP relay.
- `Hysteria-CC-RX`: The server's maximum receive rate in bytes per second. A value of `0` indicates unlimited; the literal string `"auto"` indicates the server refuses to provide a value and requests the client to use congestion control to determine the rate autonomously.
- `Hysteria-Padding`: A randomly generated padding string of variable length.

*Note: The `Hysteria-Padding` header is OPTIONAL, intended solely to obfuscate request/response sizes, and SHOULD be ignored by the receiving endpoint.*

#### 4.2.1. Authentication Failure

If authentication fails, the server MUST act identically to a standard web server that does not understand the request (e.g., returning a 404 or 403 status), or forward the request to an upstream site and return its response. 

The client MUST check the `:status` code. If the code is anything other than `233`, the client MUST consider authentication failed and MUST disconnect from the server immediately.

#### 4.2.2. Post-Authentication

After (and only after) a client successfully authenticates, the server MUST transition the QUIC connection state to consider it a Hysteria proxy connection and begin processing proxy requests.

## 5. Proxy Requests

### 5.1. TCP Proxying

For each new TCP connection, the client MUST open a new QUIC bidirectional stream and send a `TCPRequest` message.

#### 5.1.1. TCPRequest Message

```text
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|       0x401 (varint)          |  Address length (varint)      |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                 Address string (host:port) ...                |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
| Padding length (varint)       |      Random padding ...       |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

#### 5.1.2. TCPResponse Message

The server MUST respond with a `TCPResponse` message on the same stream:

```text
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|    Status     |  Message length (varint)      |               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                       Message string ...                      |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
| Padding length (varint)       |      Random padding ...       |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

- **Status**: 1 byte (uint8). `0x00` = OK, `0x01` = Error.

If the Status is `0x00` (OK), the server MUST immediately begin full-duplex forwarding of data between the client QUIC stream and the specified TCP destination until either side closes the connection. If the Status is `0x01` (Error), the server MUST close the QUIC stream.

### 5.2. UDP Proxying

UDP packets MUST be encapsulated in a `UDPMessage` and sent over the QUIC Unreliable Datagram channel (in both directions). If a server does not support UDP relay (as signaled during authentication), it SHOULD silently discard all UDP messages received from the client.

#### 5.2.1. UDPMessage Format

```text
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                           Session ID                          |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|           Packet ID           |  Fragment ID  | Fragment Count|
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
| Address length (varint)       | Address string (host:port) ...|
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                           Payload ...                         |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

- **Session ID**: 4 bytes (uint32). The client MUST use a unique Session ID for each logical UDP session. The server SHOULD assign a unique outbound UDP port to each Session ID (unless using mechanisms like symmetric NAT).
- **Packet ID**: 2 bytes (uint16). Identifier for fragmented packets.
- **Fragment ID**: 1 byte (uint8). 0-indexed fragment number.
- **Fragment Count**: 1 byte (uint8). Total number of fragments.

#### 5.2.2. UDP Session Lifecycle

There is no explicit mechanism to close a UDP session. The client MAY retain and reuse a Session ID indefinitely. The server SHOULD implement timeouts to release and reassign ports associated with inactive Session IDs. If the server receives a UDP message for an unrecognized or expired Session ID, it MUST treat it as a new session and allocate a new outbound port.

#### 5.2.3. Fragmentation

Due to QUIC datagram size limits, large UDP packets MUST be either fragmented or discarded.

- For unfragmented packets, `Fragment Count` MUST be `1`. The `Packet ID` and `Fragment ID` values are irrelevant.
- For fragmented packets, all fragments MUST carry the same `Packet ID`. The `Fragment ID` indicates the current index. Both endpoints MUST buffer fragments and wait for all parts of a packet to arrive before processing. If any fragment is lost, the entire packet MUST be discarded.

## 6. Congestion Control Signaling

Hysteria allows explicit signaling of the Tx/Rx (upload/download) rates during authentication to optimize congestion control.

- The client signals its Rx rate via the `Hysteria-CC-RX` header.
- The server signals its Rx rate via the `Hysteria-CC-RX` response header.

Special signaling values:
1. **Client sends `0`**: The client does not know its Rx limit. The server MUST rely on a standard congestion control algorithm (e.g., BBR, Cubic) to manage its transmission rate.
2. **Server sends `0`**: The server has no bandwidth limit. The client MAY transmit at any rate it desires.
3. **Server sends `"auto"`**: The server refuses to specify a limit. The client MUST use a standard congestion control algorithm to manage its transmission rate.

## 7. "Salamander" Obfuscation (Optional)

The protocol defines an OPTIONAL obfuscation layer codenamed "Salamander". When enabled, Salamander encapsulates all underlying QUIC packets.

### 7.1. Encapsulation Format

```text
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                           Salt (8 bytes)                      |
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                      Obfuscated Payload ...                   |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

### 7.2. Obfuscation Algorithm

For each QUIC packet sent:
1. The sender MUST generate a random 8-byte `Salt`.
2. The sender MUST calculate a 32-byte hash using BLAKE2b-256 over the concatenation of a Pre-Shared Key (PSK) and the generated Salt:
   `hash = BLAKE2b-256(PSK + Salt)`
3. The sender MUST obfuscate the original QUIC payload by XORing it with the calculated hash cyclically:
   ```python
   for i in range(0, len(payload)):
       obfuscated_payload[i] = payload[i] ^ hash[i % 32]
   ```

### 7.3. Deobfuscation Algorithm

For each received packet:
1. The receiver extracts the 8-byte `Salt` and the `Obfuscated Payload`.
2. The receiver MUST calculate the 32-byte hash using the same BLAKE2b-256 algorithm with the known PSK and the extracted Salt.
3. The receiver MUST deobfuscate the payload using the same cyclic XOR operation.
4. If the resulting deobfuscated payload is not a valid QUIC packet, the entire packet MUST be silently discarded.