RFC: TUIC Protocol Specification Version 0x05
Category: Informational
Date: April 2026

# TUIC Protocol Specification

## Status of This Memo

This document specifies the TUIC Protocol version 0x05 for the Internet community. This specification defines a multiplexed, TLS-encrypted streaming protocol designed for efficient network relaying over QUIC transport.

## Abstract

The TUIC protocol provides a command-driven framework for TCP and UDP traffic relaying over encrypted QUIC connections. It supports connection multiplexing, efficient UDP session management with zero round-trip-time establishment, and robust fragmentation mechanisms for large datagrams. This document describes the protocol structure, command formats, operational procedures, and implementation considerations.

## Table of Contents

1. [Introduction](#1-introduction)
2. [Conventions and Terminology](#2-conventions-and-terminology)
3. [Protocol Architecture](#3-protocol-architecture)
4. [Message Format](#4-message-format)
5. [Command Definitions](#5-command-definitions)
6. [Address Encoding](#6-address-encoding)
7. [Protocol Operations](#7-protocol-operations)
8. [Header Size Calculations](#8-header-size-calculations)
9. [Security Considerations](#9-security-considerations)
10. [References](#10-references)

## 1. Introduction

### 1.1. Purpose

The TUIC protocol is designed to provide secure, multiplexed relaying of TCP and UDP traffic over QUIC transport. The protocol addresses the need for:

- Efficient connection multiplexing over a single transport connection
- Zero round-trip-time (0-RTT) UDP session establishment
- Transparent handling of both TCP streams and UDP datagrams
- Robust fragmentation and reassembly for large UDP packets

### 1.2. Protocol Version

This document describes TUIC protocol version 0x05.

### 1.3. Relation to Other Protocols

TUIC is designed to operate over QUIC [RFC9000] as the primary transport layer but maintains transport-agnostic design principles. The protocol can be integrated into existing services, including HTTP/3 [RFC9114].

## 2. Conventions and Terminology

### 2.1. Requirements Language

The key words "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT", "SHOULD", "SHOULD NOT", "RECOMMENDED", "MAY", and "OPTIONAL" in this document are to be interpreted as described in RFC 2119 [RFC2119].

### 2.2. Terminology

- **Client**: The endpoint initiating the TUIC connection and relay requests.
- **Server**: The endpoint accepting TUIC connections and performing relay operations.
- **Command**: A protocol control message with a specific type and payload.
- **Association**: A UDP session identified by a unique 16-bit identifier.
- **Fragment**: A portion of a UDP packet divided for transmission.
- **Target**: The ultimate destination of relayed traffic.

### 2.3. Numeric Conventions

All multi-byte numeric fields in this protocol use network byte order (big-endian) unless explicitly stated otherwise.

Field sizes are denoted as follows:
- 1B = 1 byte (8 bits)
- 2B = 2 bytes (16 bits)
- 4B = 4 bytes (32 bits)
- 16B = 16 bytes (128 bits)

## 3. Protocol Architecture

### 3.1. Layering Model

```
+---------------------------+
|    Application Data       |
+---------------------------+
|    TUIC Protocol          |
+---------------------------+
|  QUIC (with TLS 1.3)      |
+---------------------------+
|    UDP                    |
+---------------------------+
```

### 3.2. Connection Model

A TUIC connection operates over a single QUIC connection. Multiple relay operations (TCP connections and UDP associations) MAY be multiplexed over this single connection.

### 3.3. Stream Usage

- **Unidirectional streams**: Used for commands that do not require a response (e.g., Authenticate, Packet, Dissociate).
- **Bidirectional streams**: Used for TCP relay via Connect command.
- **QUIC datagrams**: MAY be used for UDP packet relay for reduced latency.

## 4. Message Format

### 4.1. Base Command Structure

All TUIC commands share a common header structure:

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|    Version    |     Type      |                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+                               +
|                    Type-Specific Payload...                   |
+                                                               +
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

**Version (1 byte)**: Protocol version identifier. This specification defines version 0x05.

**Type (1 byte)**: Command type identifier (see Section 4.2).

**Type-Specific Payload**: Variable-length payload whose format depends on the Type field.

### 4.2. Command Type Registry

| Value  | Command Name   | Reference     |
|--------|----------------|---------------|
| 0x00   | Authenticate   | Section 5.1   |
| 0x01   | Connect        | Section 5.2   |
| 0x02   | Packet         | Section 5.3   |
| 0x03   | Dissociate     | Section 5.4   |
| 0x04   | Heartbeat      | Section 5.5   |

All other values are reserved for future use.

## 5. Command Definitions

### 5.1. Authenticate Command

The Authenticate command establishes client identity and authorization.

**Type**: 0x00

**Format**:
```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|    Version    |     0x00      |                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+                               +
|                                                               |
+                         UUID (16 bytes)                       +
|                                                               |
+                                                               +
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
+                                                               +
|                                                               |
+                        TOKEN (32 bytes)                       +
|                                                               |
+                                                               +
|                                                               |
+                                                               +
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

**UUID (16 bytes)**: Client identifier represented as a UUID [RFC4122].

**TOKEN (32 bytes)**: Authentication token derived using the TLS Keying Material Exporter [RFC5705] with the following parameters:
- Label: The client's UUID value
- Context: The raw password bytes
- Length: 32 bytes

**Procedure**:
1. Client MUST send Authenticate command on a unidirectional stream before any other commands.
2. Server MUST validate the TOKEN against expected credentials.
3. If validation fails, server MUST terminate the connection.
4. If validation succeeds, server enables processing of subsequent commands.

**Note**: Servers MAY accept command headers before authentication completes and pause processing until authentication succeeds.

### 5.2. Connect Command

The Connect command initiates a TCP relay to a specified target address.

**Type**: 0x01

**Format**:
```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|    Version    |     0x01      |                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+                               +
|                    Target Address (variable)                  |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

**Target Address**: Encoded as specified in Section 6.

**Procedure**:
1. Client opens a bidirectional QUIC stream.
2. Client sends Connect command with target address.
3. Server establishes TCP connection to target.
4. Bidirectional data relay begins between QUIC stream and TCP connection.
5. Stream closure in either direction terminates the relay.

### 5.3. Packet Command

The Packet command relays UDP datagrams with support for fragmentation.

**Type**: 0x02

**Format**:
```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|    Version    |     0x02      |      Association ID           |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|         Packet ID             |  Frag Total   |   Frag ID     |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|         Payload Size          |                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+                               +
|                    Target Address (variable)                  |
+                                                               +
|                         Payload...                            |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

**Association ID (2 bytes)**: UDP session identifier. See Section 7.3.

**Packet ID (2 bytes)**: Sequence number for identifying fragments of the same UDP packet.

**Frag Total (1 byte)**: Total number of fragments for this packet. MUST be at least 1. Maximum value is 255.

**Frag ID (1 byte)**: Zero-based fragment sequence number. MUST be less than Frag Total.

**Payload Size (2 bytes)**: Length of the payload portion in this fragment.

**Target Address**: Encoded as specified in Section 6. For non-first fragments (Frag ID > 0), this SHOULD use the "None" address type (0xFF).

**Payload**: Fragment data of the UDP packet.

**Fragmentation Rules**:
- Fragments with the same Association ID and Packet ID belong to the same UDP packet.
- All fragments MUST have identical Frag Total values.
- Frag ID MUST range from 0 to (Frag Total - 1).
- Receivers MUST reassemble fragments in order by Frag ID before forwarding.
- Incomplete fragment sets SHOULD be discarded after a timeout period (RECOMMENDED: 30 seconds).

### 5.4. Dissociate Command

The Dissociate command terminates a UDP association.

**Type**: 0x03

**Format**:
```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|    Version    |     0x03      |      Association ID           |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

**Association ID (2 bytes)**: Identifier of the UDP session to terminate.

**Procedure**:
1. Client sends Dissociate command on a unidirectional stream.
2. Server closes the associated UDP socket.
3. Server releases all resources associated with this Association ID.
4. Subsequent Packet commands with this Association ID SHOULD be ignored or rejected.

### 5.5. Heartbeat Command

The Heartbeat command maintains connection liveness.

**Type**: 0x04

**Format**:
```
 0                   1
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|    Version    |     0x04      |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

The Heartbeat command contains no payload beyond the base header.

**Procedure**:
- Either endpoint MAY send Heartbeat commands periodically.
- Heartbeat commands SHOULD be sent via QUIC datagrams for minimal overhead.
- RECOMMENDED interval: 30-60 seconds during active relaying.
- No response is required or expected.

## 6. Address Encoding

Target addresses are encoded using a type-length-value format.

### 6.1. Address Format

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|  Addr Type    |            Address (variable)                 |
+-+-+-+-+-+-+-+-+                                               +
|                          Port (2 bytes)                       |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

### 6.2. Address Type Registry

| Value  | Type     | Address Length | Total Size          |
|--------|----------|----------------|---------------------|
| 0x00   | Domain   | 1 + N bytes    | 4 + N bytes         |
| 0x01   | IPv4     | 4 bytes        | 7 bytes             |
| 0x02   | IPv6     | 16 bytes       | 19 bytes            |
| 0xFF   | None     | 0 bytes        | 1 byte              |

### 6.3. Address Type Specifications

**Domain (0x00)**:
```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|     0x00      |    Length     |   Domain Name (variable)...   |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|          Port (2 bytes)       |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```
- Length (1 byte): Number of bytes in the domain name (1-255).
- Domain Name: ASCII or UTF-8 encoded domain name.
- Port (2 bytes): TCP or UDP port number in network byte order.

**IPv4 (0x01)**:
```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|     0x01      |             IPv4 Address (4 bytes)            |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
| IPv4 Address  |          Port (2 bytes)       |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```
- IPv4 Address (4 bytes): IPv4 address in network byte order.
- Port (2 bytes): TCP or UDP port number in network byte order.

**IPv6 (0x02)**:
```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|     0x02      |                                               |
+-+-+-+-+-+-+-+-+                                               +
|                                                               |
+                    IPv6 Address (16 bytes)                    +
|                                                               |
+                                                               +
|                                                               |
+               +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|               |          Port (2 bytes)       |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```
- IPv6 Address (16 bytes): IPv6 address in network byte order.
- Port (2 bytes): TCP or UDP port number in network byte order.

**None (0xFF)**:
```
 0                   1
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5
+-+-+-+-+-+-+-+-+
|     0xFF      |
+-+-+-+-+-+-+-+-+
```
- Used in non-first fragments of Packet commands to reduce overhead.
- MUST NOT be used in Connect commands or first fragments.

## 7. Protocol Operations

### 7.1. Connection Establishment

1. Client establishes QUIC connection to server.
2. Client sends Authenticate command on a new unidirectional stream.
3. Server validates authentication token.
4. If successful, connection is ready for relay operations.

### 7.2. TCP Relaying

**Client-to-Server Direction**:
1. Client opens bidirectional QUIC stream.
2. Client sends Connect command with target address.
3. Server establishes TCP connection to target.
4. Client sends application data on the stream.
5. Server forwards data to target TCP connection.

**Server-to-Client Direction**:
1. Server receives data from target TCP connection.
2. Server sends data on the bidirectional QUIC stream.
3. Client receives and processes data.

**Connection Termination**:
- Either endpoint MAY close the stream to signal connection termination.
- Server MUST close target TCP connection when QUIC stream closes.

### 7.3. UDP Relaying

**Association Establishment**:
1. Client generates unique 16-bit Association ID.
2. Client sends first Packet command with this Association ID.
3. Server creates UDP socket and associates it with Association ID.
4. Both endpoints maintain mapping of Association ID to UDP socket.

**Packet Forwarding**:
- **Client-to-Server**: Client sends Packet command; server extracts target address from ADDR field and forwards to that destination.
- **Server-to-Client**: Server receives UDP packet; server sends Packet command with source address in ADDR field.

**Full Cone NAT Behavior**:
The protocol implements Full Cone NAT semantics:
- Once associated, the UDP socket receives packets from any source.
- All received packets are forwarded to the client with the source address encoded.

**Association Termination**:
1. Client sends Dissociate command.
2. Server closes UDP socket and removes association mapping.

### 7.4. UDP Fragmentation

**When to Fragment**:
Fragmentation is REQUIRED when the UDP packet size exceeds:
```
max_datagram_size - header_overhead
```

**Fragmentation Procedure**:
1. Sender calculates maximum fragment payload size (see Section 8).
2. Sender divides UDP packet into fragments.
3. Sender assigns unique Packet ID for this UDP packet.
4. Sender sets Frag Total to the number of fragments.
5. For each fragment:
   - Set Frag ID to fragment index (0-based).
   - First fragment (Frag ID = 0) includes full target address.
   - Subsequent fragments MAY use None (0xFF) address type.
6. Send each fragment as separate Packet command.

**Reassembly Procedure**:
1. Receiver collects fragments with matching (Association ID, Packet ID).
2. Once all fragments (Frag ID 0 through Frag Total - 1) are received:
   - Concatenate fragment payloads in Frag ID order.
   - Extract target address from first fragment.
   - Forward reassembled UDP packet.
3. If fragments are incomplete after timeout (RECOMMENDED: 30s):
   - Discard partial fragments.
   - Release associated resources.

### 7.5. Error Handling

The protocol follows a fail-silent error model:

**Invalid Commands**:
- Receivers SHOULD silently discard commands with unknown Type values.
- Receivers MAY terminate the connection for malformed commands.

**Authentication Failure**:
- Server MUST terminate the connection immediately.
- Server SHOULD send QUIC CONNECTION_CLOSE frame.

**Network Errors**:
- Stream errors result in stream closure without notification.
- Connection errors result in QUIC connection termination.

**Implementation Recommendations**:
- Log all errors for diagnostic purposes.
- Use connection reset for critical authentication failures.
- Gracefully degrade for recoverable stream errors.

## 8. Header Size Calculations

Understanding header overhead is critical for implementations, particularly for determining maximum payload sizes in fragmented UDP packets.

### 8.1. Base Command Header

All commands include a 2-byte base header:
```
Version (1 byte) + Type (1 byte) = 2 bytes
```

### 8.2. Packet Command Overhead

The Packet command header (excluding address) is 10 bytes:
```
Version    (1 byte)
Type       (1 byte)
Assoc ID   (2 bytes)
Packet ID  (2 bytes)
Frag Total (1 byte)
Frag ID    (1 byte)
Size       (2 bytes)
----------------------------
Total: 10 bytes
```

### 8.3. Address Field Sizes

| Address Type     | Size Calculation           | Example Size |
|------------------|----------------------------|--------------|
| None (0xFF)      | 1                          | 1 byte       |
| IPv4 (0x01)      | 1 + 4 + 2                  | 7 bytes      |
| IPv6 (0x02)      | 1 + 16 + 2                 | 19 bytes     |
| Domain (0x00)    | 1 + 1 + N + 2 (N≤255)      | 4+N bytes    |

### 8.4. Total Packet Command Overhead

**First Fragment** (with full address):
```
Base Header (2) + Packet Fields (8) + Address
```

| Address Type          | Total Overhead |
|-----------------------|----------------|
| IPv4                  | 17 bytes       |
| IPv6                  | 29 bytes       |
| Domain "example.com"  | 25 bytes       |

**Subsequent Fragments** (with None address):
```
Base Header (2) + Packet Fields (8) + None Address (1) = 11 bytes
```

### 8.5. Maximum Payload Calculations

For a given QUIC maximum datagram size D:

**Single Packet** (no fragmentation):
```
max_payload = D - header_overhead
```

**Example with D=1200**:
| Address Type     | Max Payload |
|------------------|-------------|
| IPv4             | 1183 bytes  |
| IPv6             | 1171 bytes  |
| Domain (11 char) | 1175 bytes  |

### 8.6. Fragmentation Size Calculations

**Uniform Fragment Size** (simplified approach):
```
max_fragment_payload = D - (10 + addr_size)
fragment_count = ceil(total_payload / max_fragment_payload)
```

**Optimized Fragment Size** (first fragment with address, rest with None):
```
first_fragment_size = D - (10 + addr_size)
subsequent_fragment_size = D - 11
remaining_after_first = total_payload - first_fragment_size
additional_fragments = ceil(remaining_after_first / subsequent_fragment_size)
fragment_count = 1 + additional_fragments
```

### 8.7. Implementation Constraints

- Fragment count MUST NOT exceed 255 (Frag Total is 1 byte).
- Maximum fragmentable UDP packet size:
  ```
  max_size = (D - 17) + 254 * (D - 11)  // For IPv4
  ```
- Implementations SHOULD use saturating arithmetic to prevent underflow.
- Implementations MUST account for additional QUIC and TLS framing overhead.

## 9. Security Considerations

### 9.1. Authentication

The authentication mechanism relies on TLS Keying Material Exporter [RFC5705]:
- Provides mutual authentication bound to the TLS session.
- Prevents replay attacks across different TLS sessions.
- Requires secure password storage and transmission.

Implementations MUST:
- Use strong passwords with sufficient entropy.
- Implement rate limiting for authentication attempts.
- Terminate connections after authentication failures.

### 9.2. Encryption

All TUIC traffic is encrypted by the underlying TLS 1.3 layer provided by QUIC:
- Commands and payload data are protected from eavesdropping.
- QUIC provides built-in protection against tampering and replay.

### 9.3. Denial of Service

Potential DoS vectors and mitigations:

**Resource Exhaustion**:
- Limit maximum concurrent associations per connection.
- Implement timeouts for incomplete fragment reassembly.
- Enforce maximum fragment count limits.

**Amplification Attacks**:
- UDP relay functionality could be used for amplification.
- Servers SHOULD implement rate limiting on UDP forwarding.
- Servers MAY restrict target addresses to prevent abuse.

### 9.4. Privacy Considerations

- Target addresses are encrypted within the QUIC connection.
- Traffic analysis may reveal connection patterns despite encryption.
- Implementations SHOULD consider traffic padding to resist analysis.

## 10. References

### 10.1. Normative References

**[RFC2119]** Bradner, S., "Key words for use in RFCs to Indicate Requirement Levels", BCP 14, RFC 2119, March 1997.

**[RFC4122]** Leach, P., Mealling, M., and R. Salz, "A Universally Unique Identifier (UUID) URN Namespace", RFC 4122, July 2005.

**[RFC5705]** Rescorla, E., "Keying Material Exporters for Transport Layer Security (TLS)", RFC 5705, March 2010.

**[RFC9000]** Iyengar, J., Ed., and M. Thomson, Ed., "QUIC: A UDP-Based Multiplexed and Secure Transport", RFC 9000, May 2021.

### 10.2. Informative References

**[RFC9114]** Bishop, M., Ed., "HTTP/3", RFC 9114, June 2022.

## Appendix A. Example Packet Encodings

### A.1. Authenticate Command Example

```
Version: 0x05
Type: 0x00
UUID: 550e8400-e29b-41d4-a716-446655440000
TOKEN: [32 bytes of derived key material]

Hex encoding (first 20 bytes):
05 00 55 0e 84 00 e2 9b 41 d4 a7 16 44 66 55 44
00 00 [32 token bytes...]
```

### A.2. Connect Command Example (IPv4)

```
Version: 0x05
Type: 0x01
Address Type: 0x01 (IPv4)
Address: 192.0.2.1
Port: 80

Hex encoding:
05 01 01 c0 00 02 01 00 50
```

### A.3. Packet Command Example (First Fragment)

```
Version: 0x05
Type: 0x02
Association ID: 0x0001
Packet ID: 0x0042
Frag Total: 2
Frag ID: 0
Size: 1183
Address Type: 0x01 (IPv4)
Address: 203.0.113.1
Port: 53

Hex encoding (header only):
05 02 00 01 00 42 02 00 04 9f 01 cb 00 71 01 00 35
[payload follows...]
```

### A.4. Packet Command Example (Subsequent Fragment)

```
Version: 0x05
Type: 0x02
Association ID: 0x0001
Packet ID: 0x0042
Frag Total: 2
Frag ID: 1
Size: 500
Address Type: 0xFF (None)

Hex encoding (header only):
05 02 00 01 00 42 02 01 01 f4 ff
[payload follows...]
```

## Appendix B. Implementation Checklist

Implementers SHOULD verify their implementation handles:

- [ ] Protocol version 0x05 identification
- [ ] All five command types (Authenticate, Connect, Packet, Dissociate, Heartbeat)
- [ ] All four address types (Domain, IPv4, IPv6, None)
- [ ] Big-endian byte order for all multi-byte fields
- [ ] TLS Keying Material Exporter for authentication
- [ ] TCP relay over bidirectional streams
- [ ] UDP relay with association management
- [ ] Fragment generation when exceeding MTU
- [ ] Fragment reassembly with timeout
- [ ] Fragment count limit (≤255)
- [ ] Proper header size calculations
- [ ] Saturating arithmetic to prevent underflow
- [ ] Authentication validation and connection termination
- [ ] Resource limits and DoS mitigation

---

**Authors' Addresses**

[To be completed]

**Document History**

- Version 0x05: Current specification



