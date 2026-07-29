RFC: NaïveProxy Protocol Specification
Category: Informational
Date: April 2026

# NaïveProxy 协议规范 (NaïveProxy Protocol Specification)

## 摘要 (Abstract)

本文档规定了 NaïveProxy 协议，这是一种抗审查的传输协议，它利用标准的 HTTP/2 和 HTTP/3 来隧道化网络流量。通过采用主流 Web 浏览器完全相同的网络行为，并使用标准 Web 服务器伪装来响应未经授权的探测，NaïveProxy 有效地缓解了被动流量分析和主动探测攻击。

## 目录 (Table of Contents)

1. [引言 (Introduction)](#1-引言-introduction)
2. [约定和术语 (Conventions and Terminology)](#2-约定和术语-conventions-and-terminology)
3. [传输协议 (Transport Protocol)](#3-传输协议-transport-protocol)
4. [连接建立与身份验证 (Connection Establishment & Authentication)](#4-连接建立与身份验证-connection-establishment--authentication)
   - 4.1. [快速打开行为 (Fast Open Behavior)](#41-快速打开行为-fast-open-behavior)
5. [流量分析缓解 (Traffic Analysis Mitigation - Padding)](#5-流量分析缓解填充-traffic-analysis-mitigation---padding)
   - 5.1. [头部填充 (Header Padding)](#51-头部填充-header-padding)
   - 5.2. [负载填充 (Payload Padding)](#52-负载填充-payload-padding)
6. [主动探测抵抗 (Active Probing Resistance - Camouflage)](#6-主动探测抵抗伪装-active-probing-resistance---camouflage)
7. [安全考量 (Security Considerations)](#7-安全考量-security-considerations)
8. [错误处理 (Error Handling)](#8-错误处理-error-handling)
9. [参考文献 (References)](#9-参考文献-references)

## 1. 引言 (Introduction)

深度包检测 (DPI) 系统经常通过分析 TLS 指纹、负载长度和服务器对主动探测的响应来识别和阻止代理流量。NaïveProxy 协议通过严格遵守 HTTP/2 和 HTTP/3 标准，并利用流量填充来混淆帧大小，从而对抗这些技术。

本文档概述了实现兼容的 NaïveProxy 服务器（如 `naive-rs`）所需的线路格式和服务器行为。

## 2. 约定和术语 (Conventions and Terminology)

本文档中的关键词 "MUST" (必须), "MUST NOT" (绝对不能), "REQUIRED" (要求), "SHALL" (将), "SHALL NOT" (绝对不将), "SHOULD" (应该), "SHOULD NOT" (不应该), "RECOMMENDED" (推荐), "NOT RECOMMENDED" (不推荐), "MAY" (可以) 和 "OPTIONAL" (可选) 需按照 BCP 14 [RFC2119] [RFC8174] 中的描述进行解释。

## 3. 传输协议 (Transport Protocol)

NaïveProxy 隧道必须 (MUST) 在标准的 HTTP/2（基于 TLS/TCP）或 HTTP/3（基于 QUIC/UDP）上运行。隧道使用 HTTP CONNECT 方法建立 [RFC9110]。

## 4. 连接建立与身份验证 (Connection Establishment & Authentication)

为了建立隧道，客户端向代理服务器发送 HTTP CONNECT 请求。该请求必须 (MUST) 包含一个 `Proxy-Authorization` 头部，其中包含 HTTP 基本身份验证 (Basic Authentication) 凭据。

**示例请求**：
```http
CONNECT target.example.com:443 HTTP/2
Host: target.example.com:443
Proxy-Authorization: Basic dXNlcjpwYXNz
Padding: !#$()+<>?@[]^`{}~~~~~~~~~~~~~~~~~~~
```

如果凭据有效，服务器必须 (MUST) 建立到目标的连接并返回 `200 OK` 响应。

### 4.1. 快速打开行为 (Fast Open Behavior)

为了最大限度地减少延迟，服务器应该 (SHOULD) 实现“快速打开”。在成功对客户端进行身份验证后，服务器可以 (MAY) 在上游 TCP/UDP 连接完全建立之前，立即向客户端发送 HTTP `200 OK` 响应头。

## 5. 流量分析缓解/填充 (Traffic Analysis Mitigation / Padding)

标准的 HTTP/2 和 HTTP/3 多路复用可能会暴露上游数据大小。NaïveProxy 在头部和负载级别均引入了自定义的填充方案。

### 5.1. 头部填充 (Header Padding)

客户端的 CONNECT 请求和服务器的 200 OK 响应都必须 (MUST) 包含自定义的 `Padding` 头部。该头部的目的是随机化初始 HEADERS 帧的大小，使指纹识别变得困难。

Padding 值由 30 到 61 个字节组成。前 16 个字符必须 (MUST) 从以下非霍夫曼编码的 ASCII 字符集中随机选择：

```text
! # $ ( ) + < > ? @ [ ] ^ ` { }
```

剩余字符（直到选定的随机长度）必须 (MUST) 用波浪号 (`~`) 填充。

### 5.2. 负载填充 (Payload Padding)

负载填充仅应用于新建隧道上的前 8 次读写操作 (`FIRST_PADDINGS = 8`)。

对于每个方向上的前 8 个数据块，数据必须 (MUST) 使用以下二进制格式进行成帧：

```text
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|       Orig_Size (2B)          | Pad_Len (1B)  |               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+               +
|                                                               |
+                 Original Data (Orig_Size 字节)                +
|                                                               |
+               +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|               |              Zeros (Pad_Len 字节)             |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

- **Orig_Size**: 16 位无符号整数 (大端序)，表示实际负载数据的长度。
- **Pad_Len**: 8 位无符号整数 (0-255)，指示填充字节的数量。
- **Original Data**: 未经修改的负载字节。
- **Zeros**: 包含 `0x00` 的填充字节。

在成功发送和接收 8 个帧后，必须 (MUST) 禁用填充编解码器，并且所有后续数据都必须 (MUST) 作为原始字节传输以减少开销。

## 6. 主动探测抵抗/伪装 (Active Probing Resistance / Camouflage)

审查者通常采用主动探测技术，即扮演客户端向可疑的代理 IP 地址发送未经授权或格式错误的请求。

NaïveProxy 服务器绝对不能 (MUST NOT) 暴露其代理性质。

如果请求满足以下任一条件：
1. HTTP 方法不是 `CONNECT` (例如 GET, POST)
2. 方法是 `CONNECT`，但 `Proxy-Authorization` 头部缺失或无效。

服务器必须 (MUST) 将请求代理至预配置的伪装上游（一个合法的 HTTP 服务器），或者提供静态的无害 HTML 内容。服务器的响应头、状态码和响应体必须 (MUST) 严格匹配普通 Web 服务器的返回内容（例如，对有效路径返回 `200 OK`，对缺失路径返回 `404 Not Found`，或对上游处理的未授权 CONNECT 返回 `405 Method Not Allowed`）。

## 7. 安全考量 (Security Considerations)

- 加密实现完全依赖于底层的 TLS 和 QUIC 实现。
- 基本身份验证凭据通过加密通道发送；因此，只要 TLS/QUIC 层安全，就能有效缓解中间人凭据窃取。
- 服务器必须 (MUST) 严格解析填充边界，以避免内存泄漏或越界读取。

## 8. 错误处理 (Error Handling)

当处理合法的代理请求（已通过身份验证的 CONNECT 请求）但发生上游错误时，服务器必须 (MUST) 返回适当的 HTTP 状态码以告知客户端，同时不暴露自身代理特征给未经授权的观察者。由于客户端已认证，返回具体的网关错误是安全的：

- 如果目标主机名无法解析 (DNS 失败)，服务器应该 (SHOULD) 返回 `502 Bad Gateway`。
- 如果连接到目标主机的 TCP/UDP 握手超时或被拒绝，服务器应该 (SHOULD) 返回 `504 Gateway Timeout` 或 `502 Bad Gateway`。
- 如果发生协议冲突或上游提前重置连接，服务器应该 (SHOULD) 返回 `502 Bad Gateway`。
- 如果客户端请求包含格式错误的负载填充数据，服务器必须 (MUST) 立即终止该隧道（例如，通过发送 HTTP/2 `RST_STREAM` 或 HTTP/3 `STOP_SENDING`/`RESET_STREAM`），并记录可能的异常行为。

## 9. 参考文献 (References)

**[RFC9110]** Fielding, R., Ed., Nottingham, M., Ed., and J. Reschke, Ed., "HTTP Semantics", STD 97, RFC 9110, June 2022.