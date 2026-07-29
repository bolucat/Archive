RFC: The Anytls Protocol Version 2
Category: Informational
Date: April 2026

# The Anytls Protocol Version 2

## Abstract

This document describes the Anytls protocol, a secure, multiplexed proxy protocol that operates over Transport Layer Security (TLS). The protocol is designed to provide robust obfuscation against traffic analysis through dynamic padding schemes, multiplex multiple logical streams over a single session, and protect against active probing by falling back to standard protocols.

## 1. Introduction

Anytls is a proxy protocol that establishes a secure session over a standard TLS connection. It consists of an initial authentication phase followed by a session layer that multiplexes multiple logical streams. To counter advanced Deep Packet Inspection (DPI) heuristics, Anytls utilizes a dynamic `paddingScheme` to alter traffic fingerprints actively.

## 2. Conventions and Terminology

The key words "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT", "SHOULD", "SHOULD NOT", "RECOMMENDED", "MAY", and "OPTIONAL" in this document are to be interpreted as described in RFC 2119.

- **Session**: A single established TLS connection running the Anytls session layer.
- **Stream**: A multiplexed logical channel within a Session used to proxy a specific connection.
- **Client**: The software initiating the Anytls connection.
- **Server**: The Anytls endpoint receiving the connection and performing the proxying.

## 3. Protocol Architecture

The overall protocol stack is layered as follows:

```text
+-------------------------------------------------+
|               User TCP/UDP Proxy                |
+-------------------------------------------------+
|             Anytls Stream Layer                 |
+-------------------------------------------------+
|             Anytls Session Layer                |
+-------------------------------------------------+
|        Transport Layer Security (TLS)           |
+-------------------------------------------------+
|        Transmission Control Protocol (TCP)      |
+-------------------------------------------------+
```

## 4. Authentication Phase

Immediately after the TLS handshake completes, the Client MUST send an authentication request. 

### 4.1. Client Authentication Request

The authentication payload is structured as follows:

```text
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
+                                                               +
|                                                               |
+                                                               +
|                                                               |
+                                                               +
|                                                               |
+                                                               +
|                                                               |
+                                                               +
|                                                               |
+                                                               +
|                                                               |
+                                                               +
|                  sha256(password) (32 Bytes)                  |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|       padding0 length         |    padding0 (Variable) ...    |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

- **sha256(password)**: 32 bytes. The SHA-256 hash of the pre-shared protocol password.
- **padding0 length**: 2 bytes (Big-Endian uint16). The length of the subsequent padding.
- **padding0**: Variable length. Random padding data.

*Note: The overhead for the authentication portion is 34 bytes (excluding the variable padding).*

### 4.2. Server Authentication Response

The Server MUST read the first packet and verify the authentication request (including fully reading `padding0`). 
- If authentication is successful, the Server enters the Session loop.
- If authentication fails, the Server MUST immediately close the connection OR fallback to a standard HTTP/L7 service to defend against active probing.

## 5. Session Layer

Once authenticated, both endpoints enter a session event loop.

### 5.1. Frame Format

The Session layer communicates using the following frame format:

```text
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|    command    |               streamId                        |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|               |          data length          |               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                         data (Variable)                       |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

- **command**: 1 byte (uint8). Identifies the frame type.
- **streamId**: 4 bytes (Big-Endian uint32). Identifies the logical stream.
- **data length**: 2 bytes (Big-Endian uint16). The length of the payload.
- **data**: Variable length payload.

### 5.2. Commands

The following commands are defined. Unless explicitly specified below, commands MUST NOT carry a data payload.

| Command Name | Value | Introduced | Direction | Description |
|---|---|---|---|---|
| `cmdWaste` | 0 | v1 | Both | Padding. Data MUST be read and silently discarded. |
| `cmdSYN` | 1 | v1 | Client -> Server | Open a new Stream. |
| `cmdPSH` | 2 | v1 | Both | Push data to a Stream. |
| `cmdFIN` | 3 | v1 | Both | Close a Stream (EOF). |
| `cmdSettings` | 4 | v1 | Client -> Server | Client Settings negotiation. |
| `cmdAlert` | 5 | v1 | Server -> Client | Fatal error alert from Server. |
| `cmdUpdatePaddingScheme` | 6 | v1 | Server -> Client | Dynamic padding scheme update. |
| `cmdSYNACK` | 7 | v2 | Server -> Client | Stream open acknowledgment. |
| `cmdHeartRequest` | 8 | v2 | Both | Keep-alive request. |
| `cmdHeartResponse` | 9 | v2 | Both | Keep-alive response. |
| `cmdServerSettings` | 10 | v2 | Server -> Client | Server Settings negotiation. |

#### 5.2.1. cmdSettings (4)
The Client MUST send a `cmdSettings` frame immediately upon opening a new Session. If a Server receives a `cmdSYN` before `cmdSettings`, it MUST reject the Session.

The payload is a newline (`\n`) separated list of key-value pairs separated by `=`. Encoded in UTF-8.
Example:
```text
v=2
client=anytls/0.0.1
padding-md5=(lowercase hex encoded md5 of current paddingScheme)
```

#### 5.2.2. cmdServerSettings (10)
If the Client reports version `v >= 2`, the Server MUST reply with `cmdServerSettings` immediately after receiving `cmdSettings`.
Example:
```text
v=2
```

#### 5.2.3. cmdAlert (5)
The payload contains a warning text string sent by the Server. The Client MUST read and log this message, after which both endpoints MUST close the Session. A Server MAY send this to reject outdated or non-compliant Clients.

#### 5.2.4. cmdUpdatePaddingScheme (6)
If the Server detects that the Client's `padding-md5` (from `cmdSettings`) differs from the Server's current scheme, the Server MUST send this command. The payload format is described in Section 6.

#### 5.2.5. cmdHeartRequest (8) and cmdHeartResponse (9)
When an endpoint receives a `cmdHeartRequest`, it MUST respond with a `cmdHeartResponse`. These are used to detect and recover from stuck tunnels.

### 5.3. Stream Lifecycle Commands

#### 5.3.1. cmdSYN (1)
Notifies the Server to open a new Stream. The Client MUST generate a monotonically increasing `streamId` within the Session.

#### 5.3.2. cmdSYNACK (7)
For Client version `v >= 2`, the Server SHOULD send a `cmdSYNACK` with the corresponding `streamId` after the outbound proxy TCP handshake completes. 
- A payload-less `cmdSYNACK` indicates a successful proxy stream connection.
- If data is present, it represents an error message. The Client MUST close the corresponding Stream upon receiving an error payload.

#### 5.3.3. cmdPSH (2)
The data payload carries the actual proxied traffic for the Stream.

#### 5.3.4. cmdFIN (3)
Notifies the peer to close the specified Stream.
- If the Session is healthy, the receiving endpoint closes the local Stream but DOES NOT need to reply with its own `cmdFIN`.
- If the Session is already closing, `cmdFIN` does not need to be sent.

## 6. Dynamic Padding Scheme

The Padding Scheme dictates how packets are fragmented and padded to obfuscate traffic patterns.

### 6.1. Padding Scheme Format

The scheme is sent as a newline-separated string. Example:
```text
stop=8
0=30-30
1=100-400
2=400-500,c,500-1000,c,500-1000,c,500-1000,c,500-1000
3=9-9,500-1000
```

### 6.2. Scheme Directives

- `stop`: E.g., `stop=8` dictates that only the first 8 packets (indexes 0 to 7) are subjected to the padding scheme.
- `0=X-Y`: Rules for the 0th packet (`padding0` during Authentication). This cannot be fragmented. The Client sends padding between X and Y bytes.
- `1` and above: Rules for Session phase packets. 

**Packet Counting:**
The packet index corresponds to the number of times `Write()` is called on the underlying TLS connection.
- **Packet 1** typically includes: `cmdSettings` + first Stream's `cmdSYN` + `cmdPSH` (containing proxy target address).
- **Packet 2** typically contains the first chunk of user proxy data (e.g., TLS ClientHello of the proxied connection).

**Fragmentation and Padding Logic:**
If a packet is ruled by `A-B,C-D`:
1. The user data is fragmented. The first chunk's size is randomly chosen between A and B. The second between C and D, etc. (Size refers to TLS PlainText length, excluding TLS encryption overhead).
2. If user data remains after all specified fragments, the remainder is sent natively.
3. If user data is exhausted before all fragments are fulfilled, the endpoint MUST send `cmdWaste` filled with padding (preferably 0s) to fulfill the size requirement.
4. **Check Symbol (`c`)**: If a `,c,` separator is present and the user data is exhausted, the implementation MUST return from the Write operation immediately and MUST NOT send subsequent padding packets defined after the `c`.

### 6.3. Scheme Lifecycle
- A Client MUST store the `paddingScheme` specific to the Server it connects to.
- A Client MUST use the default `paddingScheme` on its first connection.
- Upon receiving a `cmdUpdatePaddingScheme`, the Client MUST use the newly provided scheme for all subsequent Sessions created with that Server. This ensures that any discovered fingerprints only affect a minimal subset of initial connections before the scheme is dynamically rolled.

## 7. Connection Multiplexing

Clients MUST implement session multiplexing using a connection pool. The architecture is:
`TCP Proxy -> Stream -> Session -> TLS -> TCP`

### 7.1. Multiplexing Strategy
- Before creating a new Session, the Client MUST check the pool for "idle" Sessions.
- If available, the Client MUST pick the Session with the highest sequence number (`Seq`) to open the new Stream.
- If no idle Sessions exist, a new Session is created. The `Seq` MUST monotonically increase within the Client instance.
- When a Stream closes normally (and the Session has no errors), the Session is returned to the "idle session pool" and its idle start time is updated to `now`.
- The Client SHOULD periodically (e.g., every 30s) reap Sessions that have been idle for a designated duration (e.g., 60s).
- Servers MAY also periodically reap Sessions that lack uplink/downlink activity for extended periods.

## 8. Proxy Protocol Interaction

### 8.1. TCP Proxy
After opening a Stream (`cmdSYN`), the Client MUST send the target destination address in [RFC 1928 (SOCKS5) Address](https://tools.ietf.org/html/rfc1928#section-5) format inside a `cmdPSH`. Following this, bidirectional proxying commences.

### 8.2. UDP Proxy
UDP proxying relies on the `sing-box udp-over-tcp v2` protocol. The Client acts as if it is making a TCP proxy request to the special domain `sp.v2.udp-over-tcp.arpa`. 

## 9. Protocol Parameters

The Anytls protocol assumes TLS configuration is handled externally. Specific Anytls parameters include:

### 9.1. Client Parameters
- `password` (String, REQUIRED): Protocol authentication password.
- `idleSessionCheckInterval` (Duration, OPTIONAL): Interval for checking the idle pool.
- `idleSessionTimeout` (Duration, OPTIONAL): Max duration a Session can remain idle before closure.
- `minIdleSession` (Integer, OPTIONAL): The number of fresh idle sessions to maintain as warm reserves.

### 9.2. Server Parameters
- `paddingScheme` (String, OPTIONAL): The master padding scheme to enforce on Clients.

## 10. Version History

- **v1**: Initial implementation.
- **v2 (v0.0.8 - April 2025)**: Added `cmdSYNACK` to report outbound connection state and handle stuck tunnels. Added `cmdHeartRequest`/`cmdHeartResponse` keep-alives. Added `cmdServerSettings` for negotiation.
- **v2 (v0.0.10 - September 2025)**: Clarified `cmdFIN` semantics regarding Session and Stream closure.