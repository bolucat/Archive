---
icon: material/new-box
---

!!! quote "sing-box 1.14.0 中的更改"

    :material-alert: `headers`、`tls`、拨号字段已移至 [HTTP 客户端字段](#http-客户端字段)

!!! question "自 sing-box 1.12.0 起"

# DNS over HTTPS (DoH)

### 结构

```json
{
  "dns": {
    "servers": [
      {
        "type": "https",
        "tag": "",

        "server": "",
        "server_port": 0,

        "path": "",
        "method": "",

        ... // HTTP 客户端字段
      }
    ]
  }
}
```

### 字段

#### server

==必填==

DNS 服务器的地址。

如果使用域名，还必须设置 `domain_resolver` 来解析 IP 地址。

#### server_port

DNS 服务器的端口。

默认使用 `443`。

#### path

DNS 服务器的路径。

默认使用 `/dns-query`。

#### method

HTTP 请求方法。

可用值：`GET`、`POST`。

默认使用 `POST`。

### HTTP 客户端字段

参阅 [HTTP 客户端字段](/zh/configuration/shared/http-client/) 了解详情。
