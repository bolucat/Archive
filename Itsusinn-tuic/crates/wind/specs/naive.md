RFC: NaïveProxy Protocol Specification
Category: Informational
Date: April 2026

# NaïveProxy Protocol Specification

## Abstract

This document specifies the NaïveProxy protocol, a censorship-resistant transport protocol that utilizes standard HTTP/2 and HTTP/3 to tunnel network traffic. By adopting the exact network behavior of mainstream web browsers and using standard web server camouflage to respond to unauthorized probes, NaïveProxy effectively mitigates passive traffic analysis and active probing attacks.

## Table of Contents

1. [Introduction](#1-introduction)
2. [Conventions and Terminology](#2-conventions-and-terminology)
3. [Transport Protocol](#3-transport-protocol)
4. [Connection Establishment & Authentication](#4-connection-establishment--authentication)
   - 4.1. [Fast Open Behavior](#41-fast-open-behavior)
5. [Traffic Analysis Mitigation (Padding)](#5-traffic-analysis-mitigation-padding)
   - 5.1. [Header Padding](#51-header-padding)
   - 5.2. [Payload Padding](#52-payload-padding)
6. [Active Probing Resistance (Camouflage)](#6-active-probing-resistance-camouflage)
7. [Security Considerations](#7-security-considerations)
8. [Error Handling](#8-error-handling)
9. [References](#9-references)

## 1. Introduction

Deep Packet Inspection (DPI) systems frequently identify and block proxy traffic by analyzing TLS fingerprints, payload lengths, and server responses to active probes. The NaïveProxy protocol counters these techniques by strictly adhering to HTTP/2 and HTTP/3 standards and utilizing traffic padding to obfuscate frame sizes.

This document outlines the wire format and server behavior required to implement a compliant NaïveProxy server (such as `naive-rs`).

## 2. Conventions and Terminology

The key words "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT", "SHOULD", "SHOULD NOT", "RECOMMENDED", "NOT RECOMMENDED", "MAY", and "OPTIONAL" in this document are to be interpreted as described in BCP 14 [RFC2119] [RFC8174] when, and only when, they appear in all capitals.

## 3. Transport Protocol

NaïveProxy tunnels MUST operate over standard HTTP/2 (over TLS/TCP) or HTTP/3 (over QUIC/UDP). Tunnels are established using the HTTP `CONNECT` method [RFC9110].

## 4. Connection Establishment & Authentication

To establish a tunnel, the client sends an HTTP `CONNECT` request to the proxy server. This request MUST include a `Proxy-Authorization` header containing HTTP Basic Authentication credentials.

**Example Request:**
```http
CONNECT target.example.com:443 HTTP/2
Host: target.example.com:443
Proxy-Authorization: Basic dXNlcjpwYXNz
Padding: !#$()+<>?@[]^`{}~~~~~~~~~~~~~~~~~~~
```

If the credentials are valid, the server MUST establish a connection to the target and return a `200 OK` response.

### 4.1. Fast Open Behavior

To minimize latency, the server SHOULD implement "Fast Open". After successfully authenticating the client, the server MAY send the HTTP `200 OK` response header to the client immediately, before the upstream TCP/UDP connection is fully established.

## 5. Traffic Analysis Mitigation (Padding)

Standard HTTP/2 and HTTP/3 multiplexing may expose upstream data sizes. NaïveProxy introduces a custom padding scheme at both the header and payload levels.

### 5.1. Header Padding

Both the client's `CONNECT` request and the server's `200 OK` response MUST include a custom `Padding` header. The purpose of this header is to randomize the size of the initial HEADERS frame, making fingerprinting difficult.

The Padding value consists of 30 to 61 bytes. The first 16 characters MUST be randomly selected from the following non-Huffman-encoded ASCII character set:

```text
! # $ ( ) + < > ? @ [ ] ^ ` { }
```

The remaining characters (up to the selected random length) MUST be padded with tildes (`~`).

### 5.2. Payload Padding

Payload padding is applied only to the first 8 read and write operations (`FIRST_PADDINGS = 8`) on a newly established tunnel.

For the first 8 data blocks in each direction, data MUST be framed using the following binary format:

```text
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|       Orig_Size (2B)          | Pad_Len (1B)  |               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+               +
|                                                               |
+                 Original Data (Orig_Size bytes)               +
|                                                               |
+               +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|               |              Zeros (Pad_Len bytes)            |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

- **Orig_Size**: 16-bit unsigned integer (Big Endian), indicating the length of the actual payload data.
- **Pad_Len**: 8-bit unsigned integer (0-255), indicating the number of padding bytes.
- **Original Data**: The unmodified payload bytes.
- **Zeros**: Padding bytes containing `0x00`.

After 8 frames have been successfully sent and received, the padding codec MUST be disabled, and all subsequent data MUST be transmitted as raw bytes to reduce overhead.

## 6. Active Probing Resistance (Camouflage)

Censors often employ active probing techniques, acting as clients sending unauthorized or malformed requests to suspected proxy IP addresses.

A NaïveProxy server MUST NOT expose its nature as a proxy.

If a request meets any of the following conditions:
1. The HTTP method is not `CONNECT` (e.g., `GET`, `POST`)
2. The method is `CONNECT`, but the `Proxy-Authorization` header is missing or invalid

The server MUST proxy the request to a pre-configured camouflage upstream (a legitimate HTTP server) or serve static, benign HTML content. The server's response headers, status codes, and response body MUST strictly match what a normal web server would return (e.g., returning `200 OK` for valid paths, `404 Not Found` for missing paths, or `405 Method Not Allowed` for unauthorized CONNECT requests handled by the upstream).

## 7. Security Considerations

- Encryption implementation relies entirely on the underlying TLS and QUIC implementations.
- Basic Authentication credentials are sent over an encrypted channel; therefore, as long as the TLS/QUIC layer is secure, Man-in-the-Middle credential theft is effectively mitigated.
- The server MUST strictly parse padding boundaries to avoid memory leaks or out-of-bounds reads.

## 8. Error Handling

When processing a legitimate proxy request (an authenticated `CONNECT` request) but an upstream error occurs, the server MUST return appropriate HTTP status codes to inform the client without exposing its proxy characteristics to unauthorized observers. Because the client is authenticated, returning specific gateway errors is safe:

- If the target hostname cannot be resolved (DNS failure), the server SHOULD return `502 Bad Gateway`.
- If the TCP/UDP handshake to the target host times out or is refused, the server SHOULD return `504 Gateway Timeout` or `502 Bad Gateway`.
- If a protocol violation occurs or the upstream resets the connection prematurely, the server SHOULD return `502 Bad Gateway`.
- If the client request contains malformed payload padding data, the server MUST immediately terminate the tunnel (e.g., by sending an HTTP/2 `RST_STREAM` or HTTP/3 `STOP_SENDING`/`RESET_STREAM`) and log the potentially anomalous behavior.

## 9. References

**[RFC9110]** Fielding, R., Ed., Nottingham, M., Ed., and J. Reschke, Ed., "HTTP Semantics", STD 97, RFC 9110, June 2022.