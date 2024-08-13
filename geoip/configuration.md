# 简介

本项目使用的配置文件格式为 `json`，其中包含 `input` 和 `output` 两个数组，每个数组包含一个或多个输入/输出格式的具体配置。

```json
{
  "input":  [],
  "output": []
}
```

支持的 `input` 输入格式：

- **clashRuleSet**：ipcidr 类型的 Clash RuleSet
- **clashRuleSetClassical**：classical 类型的 Clash RuleSet
- **cutter**：用于裁剪前置步骤中的数据
- **json**：JSON 数据格式
- **maxmindGeoLite2ASNCSV**：MaxMind GeoLite2 ASN CSV 数据格式（`GeoLite2-ASN-CSV.zip`）
- **maxmindGeoLite2CountryCSV**：MaxMind GeoLite2 country CSV 数据格式（`GeoLite2-Country-CSV.zip`）
- **maxmindMMDB**：MaxMind GeoLite2 country mmdb 数据格式（`GeoLite2-Country.mmdb`）
- **mihomoMRS**：mihomo MRS 数据格式（`geoip-cn.mrs`）
- **private**：局域网和私有网络 CIDR（例如：`192.168.0.0/16` 和 `127.0.0.0/8`）
- **singboxSRS**：sing-box SRS 数据格式（`geoip-cn.srs`）
- **stdin**：从 standard input 获取纯文本 IP 和 CIDR（例如：`1.1.1.1` 或 `1.0.0.0/24`）
- **surgeRuleSet**：Surge RuleSet
- **text**：纯文本 IP 和 CIDR（例如：`1.1.1.1` 或 `1.0.0.0/24`）
- **v2rayGeoIPDat**：V2Ray GeoIP dat 数据格式（`geoip.dat`）

支持的 `output` 输出格式：

- **clashRuleSet**：ipcidr 类型的 Clash RuleSet
- **clashRuleSetClassical**：classical 类型的 Clash RuleSet
- **lookup**：从指定的列表中查找指定的 IP 或 CIDR
- **maxmindMMDB**：MaxMind mmdb 数据格式（`GeoLite2-Country.mmdb`）
- **mihomoMRS**：mihomo MRS 数据格式（`geoip-cn.mrs`）
- **singboxSRS**：sing-box SRS 数据格式（`geoip-cn.srs`）
- **stdout**：将纯文本 CIDR 输出到 standard output（例如：`1.0.0.0/24`）
- **surgeRuleSet**：Surge RuleSet
- **text**：纯文本 CIDR（例如：`1.0.0.0/24`）
- **v2rayGeoIPDat**：V2Ray GeoIP dat 数据格式（`geoip.dat`）

## `input` 输入格式配置项

### **clashRuleSet**：ipcidr 类型的 Clash RuleSet

- type：（必须）输入格式的名称
- action：（必须）操作类型，值为 `add`（添加 IP 地址）或 `remove`（移除 IP 地址）
- args：（必须）
  - name：类别名称。（不能与 `inputDir` 同时使用；需要与 `uri` 同时使用）
  - uri：Clash `ipcidr` 类型的 ruleset 文件路径，可为本地文件路径或远程 `http`、`https` 文件 URL。（不能与 `inputDir` 同时使用；需要与 `name` 同时使用）
  - inputDir：需要遍历的输入目录（不遍历子目录）。（遍历的文件名作为类别名称；不能与 `name` 和 `uri` 同时使用）
  - wantedList：（可选，数组）指定需要的类别/文件。（与 `inputDir` 同时使用）
  - onlyIPType：（可选）只处理的 IP 地址类型，值为 `ipv4` 或 `ipv6`。

```jsonc
{
  "type": "clashRuleSet",
  "action": "add",     // 添加 IP 地址
  "args": {
    "name": "cn",
    "uri": "./cn.yaml" // 读取本地文件 cn.yaml 的 IPv4 和 IPv6 地址，并添加到 cn 类别中
  }
}
```

```jsonc
{
  "type": "clashRuleSet",
  "action": "add",                    // 添加 IP 地址
  "args": {
    "inputDir": "./clash/yaml",       // 遍历 ./clash/yaml 目录内的所有文件（不遍历子目录）
    "wantedList": ["cn", "us", "jp"], // 只需要 ./clash/yaml 目录内文件名去除扩展名后，名为 cn、us、jp 的文件
    "onlyIPType": "ipv6"              // 只添加 IPv6 地址
  }
}
```

```jsonc
{
  "type": "clashRuleSet",
  "action": "remove",                     // 移除 IP 地址
  "args": {
    "name": "cn",
    "uri": "https://example.com/cn.yaml", // 读取网络文件内容
    "onlyIPType": "ipv6"                  // 只从 cn 类别中移除 IPv6 地址
  }
}
```

### **clashRuleSetClassical**：classical 类型的 Clash RuleSet

- type：（必须）输入格式的名称
- action：（必须）操作类型，值为 `add`（添加 IP 地址）或 `remove`（移除 IP 地址）
- args：（必须）
  - name：类别名称。（不能与 `inputDir` 同时使用；需要与 `uri` 同时使用）
  - uri：Clash `classical` 类型的 ruleset 文件路径，可为本地文件路径或远程 `http`、`https` 文件 URL。（不能与 `inputDir` 同时使用；需要与 `name` 同时使用）
  - inputDir：需要遍历的输入目录（不遍历子目录）。（遍历的文件名作为类别名称；不能与 `name` 和 `uri` 同时使用）
  - wantedList：（可选，数组）指定需要的类别/文件。（与 `inputDir` 同时使用）
  - onlyIPType：（可选）只处理的 IP 地址类型，值为 `ipv4` 或 `ipv6`。

```jsonc
{
  "type": "clashRuleSetClassical",
  "action": "add",     // 添加 IP 地址
  "args": {
    "name": "cn",
    "uri": "./cn.yaml" // 读取本地文件 cn.yaml 的 IPv4 和 IPv6 地址，并添加到 cn 类别中
  }
}
```

```jsonc
{
  "type": "clashRuleSetClassical",
  "action": "add",                    // 添加 IP 地址
  "args": {
    "inputDir": "./clash/yaml",       // 遍历 ./clash/yaml 目录内的所有文件（不遍历子目录）
    "wantedList": ["cn", "us", "jp"], // 只需要 ./clash/yaml 目录内文件名去除扩展名后，名为 cn、us、jp 的文件
    "onlyIPType": "ipv6"              // 只添加 IPv6 地址
  }
}
```

```jsonc
{
  "type": "clashRuleSetClassical",
  "action": "remove",                     // 移除 IP 地址
  "args": {
    "name": "cn",
    "uri": "https://example.com/cn.yaml", // 读取网络文件内容
    "onlyIPType": "ipv6"                  // 只从 cn 类别中移除 IPv6 地址
  }
}
```

### **cutter**：用于裁剪前置步骤中的数据

- type：（必须）输入格式的名称
- action：（必须）操作类型，值只能是 `remove`（移除 IP 地址）
- args：（必须）
  - wantedList：（可选，数组）指定需要的类别/文件。
  - onlyIPType：（可选）只处理的 IP 地址类型，值为 `ipv4` 或 `ipv6`。

```jsonc
{
  "type": "cutter",
  "action": "remove",                // 移除 IP 地址
  "args": {
    "wantedList": ["cn", "us", "jp"] // 移除名为 cn、us、jp 这三个类别的 IPv4 和 IPv6 地址，即删除这三个类别
  }
}
```

```jsonc
{
  "type": "cutter",
  "action": "remove",                 // 移除 IP 地址
  "args": {
    "wantedList": ["cn", "us", "jp"],
    "onlyIPType": "ipv6"              // 只移除名为 cn、us、jp 这三个类别的 IPv6 地址
  }
}
```

### **json**：JSON 数据格式

- type：（必须）输入格式的名称
- action：（必须）操作类型，值为 `add`（添加 IP 地址）或 `remove`（移除 IP 地址）
- args：（必须）
  - name：类别名称。（不能与 `inputDir` 同时使用；需要与 `uri` 同时使用）
  - uri：JSON 文件路径，可为本地文件路径或远程 `http`、`https` 文件 URL。（不能与 `inputDir` 同时使用；需要与 `name` 同时使用）
  - inputDir：需要遍历的输入目录（不遍历子目录）。（遍历的文件名作为类别名称；不能与 `name` 和 `uri` 同时使用）
  - wantedList：（可选，数组）指定需要的类别/文件。
  - onlyIPType：（可选）只处理的 IP 地址类型，值为 `ipv4` 或 `ipv6`。
  - jsonPath：（必须，数组）项目 [@tidwall/gjson](https://github.com/tidwall/gjson) 定义的 JSON 数据读取路径，用于从 JSON 格式数据中提取需要的 IPv4 地址 和 IPv6 地址，语法参考：[https://github.com/tidwall/gjson/blob/master/SYNTAX.md](https://github.com/tidwall/gjson/blob/master/SYNTAX.md)

```jsonc
{
  "type": "json",
  "action": "add", // 添加 IP 地址
  "args": {
    "name": "fastly",
    "uri": "https://api.fastly.com/public-ip-list",
    "jsonPath": ["addresses", "ipv6_addresses"]
  }
}
```

```jsonc
{
  "type": "json",
  "action": "add",                    // 添加 IP 地址
  "args": {
    "inputDir": "./json",
    "wantedList": ["cn", "us", "jp"], // 只需要 ./json 目录内文件名去除扩展名后，名为 cn、us、jp 的 JSON 文件
    "onlyIPType": "ipv6",             // 只添加 IPv6 地址
    "jsonPath": ["prefixes.#.ipv4Prefix", "prefixes.#.ipv6Prefix"]
  }
}
```

```jsonc
{
  "type": "json",
  "action": "remove",     // 移除 IP 地址
  "args": {
    "name": "cn",
    "uri": "./cn.json",
    "onlyIPType": "ipv6", // 只移除类别为 cn 的 IPv6 地址
    "jsonPath": ["prefixes.#.ipv4Prefix", "prefixes.#.ipv6Prefix"]
  }
}
```

### **maxmindGeoLite2ASNCSV**：MaxMind GeoLite2 ASN CSV 数据格式（`GeoLite2-ASN-CSV.zip`）

- type：（必须）输入格式的名称
- action：（必须）操作类型，值为 `add`（添加 IP 地址）或 `remove`（移除 IP 地址）
- args：（可选）
  - ipv4：（可选）MaxMind GeoLite2 ASN IPv4 文件路径（`GeoLite2-ASN-Blocks-IPv4.csv`），可为本地文件路径或远程 `http`、`https` 文件 URL。
  - ipv6:（可选）MaxMind GeoLite2 ASN IPv6 文件路径（`GeoLite2-ASN-Blocks-IPv6.csv`），可为本地文件路径或远程 `http`、`https` 文件 URL。
  - wantedList：（可选）指定需要的类别/文件。
  - onlyIPType：（可选）只处理的 IP 地址类型，值为 `ipv4` 或 `ipv6`。

```jsonc
// 默认使用文件：
// ./geolite2/GeoLite2-ASN-Blocks-IPv4.csv
// ./geolite2/GeoLite2-ASN-Blocks-IPv6.csv
{
  "type": "maxmindGeoLite2ASNCSV",
  "action": "add" // 添加 IP 地址
}
```

```jsonc
{
  "type": "maxmindGeoLite2ASNCSV",
  "action": "add", // 添加 IP 地址
  "args": {
    "ipv4": "./geolite2/GeoLite2-ASN-Blocks-IPv4.csv",
    "ipv6": "./geolite2/GeoLite2-ASN-Blocks-IPv6.csv"
  }
}
```

```jsonc
{
  "type": "maxmindGeoLite2ASNCSV",
  "action": "add",                   // 添加 IP 地址
  "args": {
    "wantedList": ["cn", "us", "jp"] // 只需要添加名为 cn、us、jp 的这三个类别的 IPv4 地址 和 IPv6 地址
  }
}
```

```jsonc
{
  "type": "maxmindGeoLite2ASNCSV",
  "action": "remove",                                   // 移除 IP 地址
  "args": {
    "ipv4": "./geolite2/GeoLite2-ASN-Blocks-IPv4.csv",
    "ipv6": "./geolite2/GeoLite2-ASN-Blocks-IPv6.csv",    
    "wantedList": ["cn", "us", "jp"],                  // 只移除名为 cn、us、jp 的这三个类别的 IPv6 地址
    "onlyIPType": "ipv6"                               // 只移除 IPv6 地址
  }
}
```

### **maxmindGeoLite2CountryCSV**：MaxMind GeoLite2 country CSV 数据格式（`GeoLite2-Country-CSV.zip`）

- type：（必须）输入格式的名称
- action：（必须）操作类型，值为 `add`（添加 IP 地址）或 `remove`（移除 IP 地址）
- args：（可选）
  - country：（可续）MaxMind GeoLite2 Country CSV location 文件路径（`GeoLite2-Country-Locations-en.csv`），可为本地文件路径或远程 `http`、`https` 文件 URL。
  - ipv4：（可选）MaxMind GeoLite2 Country IPv4 文件路径（`GeoLite2-Country-Blocks-IPv4.csv`），可为本地文件路径或远程 `http`、`https` 文件 URL。
  - ipv6:（可选）MaxMind GeoLite2 Country IPv6 文件路径（`GeoLite2-Country-Blocks-IPv6.csv`），可为本地文件路径或远程 `http`、`https` 文件 URL。
  - wantedList：（可选）指定需要的类别/文件。
  - onlyIPType：（可选）只处理的 IP 地址类型，值为 `ipv4` 或 `ipv6`。

```jsonc
// 默认使用文件：
// ./geolite2/GeoLite2-Country-Locations-en.csv
// ./geolite2/GeoLite2-Country-Blocks-IPv4.csv
// ./geolite2/GeoLite2-Country-Blocks-IPv6.csv
{
  "type": "maxmindGeoLite2CountryCSV",
  "action": "add" // 添加 IP 地址
}
```

```jsonc
{
  "type": "maxmindGeoLite2CountryCSV",
  "action": "add",                     // 添加 IP 地址
  "args": {
    "country": "./geolite2/GeoLite2-Country-Locations-en.csv",
    "ipv4": "./geolite2/GeoLite2-Country-Blocks-IPv4.csv",
    "ipv6": "./geolite2/GeoLite2-Country-Blocks-IPv6.csv"
  }
}
```

```jsonc
{
  "type": "maxmindGeoLite2CountryCSV",
  "action": "add",                   // 添加 IP 地址
  "args": {
    "wantedList": ["cn", "us", "jp"] // 只需要添加名为 cn、us、jp 的这三个类别的 IPv4 地址 和 IPv6 地址
  }
}
```

```jsonc
{
  "type": "maxmindGeoLite2CountryCSV",
  "action": "remove",                 // 移除 IP 地址
  "args": {  
    "wantedList": ["cn", "us", "jp"], // 只移除名为 cn、us、jp 的这三个类别的 IPv6 地址
    "onlyIPType": "ipv6"              // 只移除 IPv6 地址
  }
}
```

### **maxmindMMDB**：MaxMind GeoLite2 country mmdb 数据格式（`GeoLite2-Country.mmdb`）

- type：（必须）输入格式的名称
- action：（必须）操作类型，值为 `add`（添加 IP 地址）或 `remove`（移除 IP 地址）
- args：（可选）
  - uri：（可选）MaxMind GeoLite2 Country mmdb 格式文件路径，可为本地文件路径或远程 `http`、`https` 文件 URL。
  - wantedList：（可选）指定需要的类别/文件。
  - onlyIPType：（可选）只处理的 IP 地址类型，值为 `ipv4` 或 `ipv6`。

```jsonc
// 默认使用文件：
// ./geolite2/GeoLite2-Country.mmdb
{
  "type": "maxmindMMDB",
  "action": "add"       // 添加 IP 地址
}
```

```jsonc
{
  "type": "maxmindMMDB",
  "action": "add",       // 添加 IP 地址
  "args": {
    "uri": "./geolite2/GeoLite2-Country.mmdb"
  }
}
```

```jsonc
{
  "type": "maxmindMMDB",
  "action": "add",                        // 添加 IP 地址
  "args": {
    "uri": "https://example.com/my.mmdb",
    "wantedList": ["cn", "us", "jp"],    // 只需要名为 cn、us、jp 的类别
    "onlyIPType": "ipv4"                 // 只添加 IPv4 地址
  }
}
```

```jsonc
{
  "type": "maxmindMMDB",
  "action": "remove",                    // 添加 IP 地址
  "args": {
    "uri": "https://example.com/my.mmdb",
    "wantedList": ["cn", "us", "jp"],    // 只移除名为 cn、us、jp 这三个类别的 IPv4 地址
    "onlyIPType": "ipv4"                 // 只移除 IPv4 地址
  }
}
```

### **mihomoMRS**：mihomo MRS 数据格式（`geoip-cn.mrs`）

- type：（必须）输入格式的名称
- action：（必须）操作类型，值为 `add`（添加 IP 地址）或 `remove`（移除 IP 地址）
- args：（必须）
  - name：类别名称。（不能与 `inputDir` 同时使用；需要与 `uri` 同时使用）
  - uri：mihomo MRS 格式文件路径，可为本地文件路径或远程 `http`、`https` 文件 URL。（不能与 `inputDir` 同时使用；需要与 `name` 同时使用）
  - inputDir：需要遍历的输入目录（不遍历子目录）。（遍历的文件名作为类别名称;不能与 `name` 和 `uri` 同时使用）
  - wantedList：（可选，数组）指定需要的类别/文件。（与 `inputDir` 同时使用）
  - onlyIPType：（可选）只处理的 IP 地址类型，值为 `ipv4` 或 `ipv6`。

```jsonc
{
  "type": "mihomoMRS",
  "action": "add",    // 添加 IP 地址
  "args": {
    "name": "cn",
    "uri": "./cn.mrs" // 读取本地文件 cn.mrs 的 IPv4 和 IPv6 地址，并添加到 cn 类别中
  }
}
```

```jsonc
{
  "type": "mihomoMRS",
  "action": "add",                    // 添加 IP 地址
  "args": {
    "inputDir": "./mihomo/mrs",       // 遍历 ./mihomo/mrs 目录内的所有文件（不遍历子目录）
    "wantedList": ["cn", "us", "jp"], // 只需要 ./mihomo/mrs 目录里文件名去除扩展名后，名为 cn、us、jp 的文件
    "onlyIPType": "ipv6"              // 只添加 IPv6 地址
  }
}
```

```jsonc
{
  "type": "mihomoMRS",
  "action": "remove",                    // 移除 IP 地址
  "args": {
    "name": "cn",
    "uri": "https://example.com/cn.mrs", // 读取网络文件内容
    "onlyIPType": "ipv6"                 // 只从 cn 类别中移除 IPv6 地址
  }
}
```

### **private**：局域网和私有网络 CIDR（例如：`192.168.0.0/16` 和 `127.0.0.0/8`）

- type：（必须）输入格式的名称
- action：（必须）操作类型，值为 `add`（添加 IP 地址）或 `remove`（移除 IP 地址）

> `private` 默认添加或移除的 CIDR 地址，见 [private.go](https://github.com/Loyalsoldier/geoip/blob/HEAD/plugin/special/private.go#L16-L36)

```jsonc
{
  "type": "private",
  "action": "add"   // 添加 IP 地址
}
```

```jsonc
{
  "type": "private",
  "action": "remove" // 移除 IP 地址
}
```

### **singboxSRS**：sing-box SRS 数据格式（`geoip-cn.srs`）

- type：（必须）输入格式的名称
- action：（必须）操作类型，值为 `add`（添加 IP 地址）或 `remove`（移除 IP 地址）
- args：（必须）
  - name：类别名称。（不能与 `inputDir` 同时使用；需要与 `uri` 同时使用）
  - uri：sing-box SRS 格式文件路径，可为本地文件路径或远程 `http`、`https` 文件 URL。（不能与 `inputDir` 同时使用；需要与 `name` 同时使用）
  - inputDir：需要遍历的输入目录（不遍历子目录）。（遍历的文件名作为类别名称；不能与 `name` 和 `uri` 同时使用）
  - wantedList：（可选，数组）指定需要的类别/文件。（与 `inputDir` 同时使用）
  - onlyIPType：（可选）只处理的 IP 地址类型，值为 `ipv4` 或 `ipv6`。

```jsonc
{
  "type": "singboxSRS",
  "action": "add",    // 添加 IP 地址
  "args": {
    "name": "cn",
    "uri": "./cn.srs" // 读取本地文件 cn.srs 的 IPv4 和 IPv6 地址，并添加到 cn 类别中
  }
}
```

```jsonc
{
  "type": "singboxSRS",
  "action": "add",                    // 添加 IP 地址
  "args": {
    "inputDir": "./singbox/srs",      // 遍历 ./singbox/srs 目录内的所有文件（不遍历子目录）
    "wantedList": ["cn", "us", "jp"], // 只需要 ./singbox/srs 目录内文件名去除扩展名后，名为 cn、us、jp 的文件
    "onlyIPType": "ipv6"              // 只添加 IPv6 地址
  }
}
```

```jsonc
{
  "type": "singboxSRS",
  "action": "remove",                    // 移除 IP 地址
  "args": {
    "name": "cn",
    "uri": "https://example.com/cn.srs", // 读取网络文件内容
    "onlyIPType": "ipv6"                 // 只从 cn 类别中移除 IPv6 地址
  }
}
```

### **stdin**：从 standard input 获取纯文本 IP 和 CIDR（例如：`1.1.1.1` 或 `1.0.0.0/24`）

- type：（必须）输入格式的名称
- action：（必须）操作类型，值为 `add`（添加 IP 地址）或 `remove`（移除 IP 地址）
- args：（必须）
  - name：（必须）类别名称
  - onlyIPType：（可选）只处理的 IP 地址类型，值为 `ipv4` 或 `ipv6`

```jsonc
{
  "type": "stdin",
  "action": "add", // 添加 IP 地址
  "args": {
    "name": "cn"
  }
}
```

```jsonc
{
  "type": "stdin",
  "action": "add",       // 添加 IP 地址
  "args": {
    "name": "cn",
    "onlyIPType": "ipv6" // 只添加 IPv6 地址
  }
}
```

### **surgeRuleSet**：Surge RuleSet

- type：（必须）输入格式的名称
- action：（必须）操作类型，值为 `add`（添加 IP 地址）或 `remove`（移除 IP 地址）
- args：（必须）
  - name：类别名称。（不能与 `inputDir` 同时使用；需要与 `uri` 同时使用）
  - uri：Surge ruleset 文件路径，可为本地文件路径或远程 `http`、`https` 文件 URL。（不能与 `inputDir` 同时使用；需要与 `name` 同时使用）
  - inputDir：需要遍历的输入目录（不遍历子目录）。（遍历的文件名作为类别名称；不能与 `name` 和 `uri` 同时使用）
  - wantedList：（可选，数组）指定需要的类别/文件。（与 `inputDir` 同时使用）
  - onlyIPType：（可选）只处理的 IP 地址类型，值为 `ipv4` 或 `ipv6`。

```jsonc
{
  "type": "surgeRuleSet",
  "action": "add",       // 添加 IP 地址
  "args": {
    "name": "cn",
    "uri": "./cn.txt"   // 读取本地文件 cn.txt 的 IPv4 和 IPv6 地址，并添加到 cn 类别中
  }
}
```

```jsonc
{
  "type": "surgeRuleSet",
  "action": "add",                    // 添加 IP 地址
  "args": {
    "inputDir": "./surge",            // 遍历 ./surge 目录内的所有文件（不遍历子目录）
    "wantedList": ["cn", "us", "jp"], // 只需要 ./surge 目录内文件名去除扩展名后，名为 cn、us、jp 的文件
    "onlyIPType": "ipv6"              // 只添加 IPv6 地址
  }
}
```

```jsonc
{
  "type": "surgeRuleSet",
  "action": "remove",                    // 移除 IP 地址
  "args": {
    "name": "cn",
    "uri": "https://example.com/cn.txt", // 读取网络文件内容
    "onlyIPType": "ipv6"                 // 只从 cn 类别中移除 IPv6 地址
  }
}
```

### **text**：纯文本 IP 和 CIDR（例如：`1.1.1.1` 或 `1.0.0.0/24`）

- type：（必须）输入格式的名称
- action：（必须）操作类型，值为 `add`（添加 IP 地址）或 `remove`（移除 IP 地址）
- args：（必须）
  - name：类别名称。（不能与 `inputDir` 同时使用；需要与 `uri` 同时使用）
  - uri：纯文本 txt 文件路径，可为本地文件路径或远程 `http`、`https` 文件 URL。（不能与 `inputDir` 同时使用；需要与 `name` 同时使用）
  - inputDir：需要遍历的输入目录（不遍历子目录）。（遍历的文件名作为类别名称；不能与 `name` 和 `uri` 同时使用）
  - wantedList：（可选，数组）指定需要的类别/文件。（与 `inputDir` 同时使用）
  - onlyIPType：（可选）只处理的 IP 地址类型，值为 `ipv4` 或 `ipv6`
  - removePrefixesInLine：（可选，数组）每一行需要移除的字符串前缀
  - removeSuffixesInLine：（可选，数组）每一行需要移除的字符串后缀

```jsonc
{
  "type": "text",
  "action": "add",                                 // 添加 IP 地址
  "args": {
    "name": "cn",
    "uri": "./cn.txt",                            // 读取本地文件 cn.txt 的 IPv4 和 IPv6 地址，并添加到 cn 类别中
    "removePrefixesInLine": ["Host,", "IP-CIDR"], // 从读取的文件中移除多种不同的行前缀
    "removeSuffixesInLine": [",no-resolve"]       // 从读取的文件中移除行后缀
  }
}
```

```jsonc
{
  "type": "text",
  "action": "add", // 添加 IP 地址
  "args": {
    "inputDir": "./text",                         // 遍历 ./text 目录内的所有文件（不遍历子目录）
    "wantedList": ["cn", "us", "jp"],             // 只需要 ./text 目录里文件名去除扩展名后，名为 cn、us、jp 的文件
    "onlyIPType": "ipv6",                         // 只添加 IPv6 地址
    "removePrefixesInLine": ["Host,", "IP-CIDR"], // 从读取的文件中移除多种不同的行前缀
    "removeSuffixesInLine": [",no-resolve"]       // 从读取的文件中移除行后缀
  }
}
```

```jsonc
{
  "type": "text",
  "action": "remove",                             // 移除 IP 地址
  "args": {
    "name": "cn",
    "uri": "https://example.com/cn.txt",          // 读取网络文件内容
    "onlyIPType": "ipv6",                         // 只从 cn 类别中移除 IPv6 地址
    "removePrefixesInLine": ["Host,", "IP-CIDR"], // 从读取的文件中移除多种不同的行前缀
  }
}
```

```jsonc
{
  "type": "text",
  "action": "remove",                       // 移除 IP 地址
  "args": {
    "name": "cn",
    "uri": "https://example.com/cn.txt",    // 读取网络文件内容
    "onlyIPType": "ipv6",                   // 只从 cn 类别中移除 IPv6 地址
    "removeSuffixesInLine": [",no-resolve"] // 从读取的文件中移除行后缀
  }
}
```

### **v2rayGeoIPDat**：V2Ray GeoIP dat 数据格式（`geoip.dat`）

- type：（必须）输入格式的名称
- action：（必须）操作类型，值为 `add`（添加 IP 地址）或 `remove`（移除 IP 地址）
- args：（必须）
  - uri：（必须）V2Ray dat 格式 geoip 文件路径，可为本地文件路径或远程 `http`、`https` 文件 URL。（不能与 `inputDir` 同时使用；需要与 `name` 同时使用）
  - wantedList：（可选，数组）指定需要的类别/文件。
  - onlyIPType：（可选）只处理的 IP 地址类型，值为 `ipv4` 或 `ipv6`

```jsonc
{
  "type": "v2rayGeoIPDat",
  "action": "add",         // 添加 IP 地址
  "args": {
    "uri": "./cn.dat"      // 读取本地文件 cn.dat 中的类别、IPv4 和 IPv6 地址
  }
}
```

```jsonc
{
  "type": "v2rayGeoIPDat",
  "action": "add",                    // 添加 IP 地址
  "args": {
    "uri": "./geoip.dat",             // 读取本地文件 geoip.dat 中的类别
    "wantedList": ["cn", "us", "jp"], // 只需要 geoip.dat 中名为 cn、us、jp 的类别
    "onlyIPType": "ipv6"              // 只添加 IPv6 地址
  }
}
```

```jsonc
{
  "type": "v2rayGeoIPDat",
  "action": "remove",                       // 移除 IP 地址
  "args": {
    "uri": "https://example.com/geoip.dat", // 读取网络文件内容
    "onlyIPType": "ipv6"                    // 移除所有类别的 IPv6 地址
  }
}
```

## `output` 输出格式配置项

### **clashRuleSet**：ipcidr 类型的 Clash RuleSet

- type：（必须）输入格式的名称
- action：（必须）操作类型，值必须为 `output`
- args：（可选）
  - outputDir：（可选）输出目录
  - outputExtension：（可选）输出文件的扩展名
  - wantedList：（可选，数组）指定需要输出的类别
  - onlyIPType：（可选）输出的 IP 地址类型，值为 `ipv4` 或 `ipv6`

```jsonc
// 默认输出目录 ./output/clash/ipcidr
{
  "type": "clashRuleSet",
  "action": "output"
}
```

```jsonc
{
  "type": "clashRuleSet",
  "action": "output",
  "args": {
    "outputDir": "./clash/ipcidr", // 输出文件到目录 ./clash/ipcidr
    "outputExtension": ".yaml"     // 输出文件的扩展名为 .yaml
  }
}
```

```jsonc
{
  "type": "clashRuleSet",
  "action": "output",
  "args": {
    "outputDir": "./clash/ipcidr",   // 输出文件到目录 ./clash/ipcidr
    "outputExtension": ".yaml",      // 输出文件的扩展名为 .yaml
    "wantedList": ["cn", "us", "jp"] // 只输出名为 cn、us、jp 这三个类别的 IPv4 和 IPv6 地址
  }
}
```

```jsonc
{
  "type": "clashRuleSet",
  "action": "output",
  "args": {
    "outputDir": "./clash/ipcidr",    // 输出文件到目录 ./clash/ipcidr
    "outputExtension": ".yaml",       // 输出文件的扩展名为 .yaml
    "wantedList": ["cn", "us", "jp"], // 只输出名为 cn、us、jp 这三个类别的 IPv4 地址
    "onlyIPType": "ipv4"
  }
}
```

### **clashRuleSetClassical**：classical 类型的 Clash RuleSet

- type：（必须）输入格式的名称
- action：（必须）操作类型，值必须为 `output`
- args：（可选）
  - outputDir：（可选）输出目录
  - outputExtension：（可选）输出文件的扩展名
  - wantedList：（可选，数组）指定需要输出的类别
  - onlyIPType：（可选）输出的 IP 地址类型，值为 `ipv4` 或 `ipv6`

```jsonc
// 默认输出目录 ./output/clash/classical
{
  "type": "clashRuleSetClassical",
  "action": "output"
}
```

```jsonc
{
  "type": "clashRuleSetClassical",
  "action": "output",
  "args": {
    "outputDir": "./clash/classical", // 输出文件到目录 ./clash/classical
    "outputExtension": ".yaml"        // 输出文件的扩展名为 .yaml
  }
}
```

```jsonc
{
  "type": "clashRuleSetClassical",
  "action": "output",
  "args": {
    "outputDir": "./clash/classical", // 输出文件到目录 ./clash/classical
    "outputExtension": ".yaml",       // 输出文件的扩展名为 .yaml
    "wantedList": ["cn", "us", "jp"]  // 只输出名为 cn、us、jp 这三个类别的 IPv4 和 IPv6 地址
  }
}
```

```jsonc
{
  "type": "clashRuleSetClassical",
  "action": "output",
  "args": {
    "outputDir": "./clash/classical", // 输出文件到目录 ./clash/classical
    "outputExtension": ".yaml",       // 输出文件的扩展名为 .yaml
    "wantedList": ["cn", "us", "jp"], // 只输出名为 cn、us、jp 这三个类别的 IPv4 地址
    "onlyIPType": "ipv4"
  }
}
```

### **lookup**：从指定的列表中查找指定的 IP 或 CIDR，将结果输出到 standard output

- type：（必须）输入格式的名称
- action：（必须）操作类型，值必须为 `output`
- args：（必须）
  - search：（必须）指定需要查询的 IP 或 CIDR
  - searchList：（可选，数组）从指定的类别中查询

```jsonc
{
  "type": "lookup",
  "action": "output",
  "args": {
    "search": "1.1.1.1" // 查询 IP 地址 1.1.1.1 所在的类别
  }
}
```

```jsonc
// 返回结果为单个类别名称，如：cn
// 或由英文逗号连接的类别字符串，如: au,cn
{
  "type": "lookup",
  "action": "output",
  "args": {
    "search": "1.1.1.0/24",          // 查询 CIDR 1.1.1.0/24 是否存在于类别 au、cn、us 中
    "searchList": ["au", "cn", "us"]
  }
}
```

### **maxmindMMDB**：MaxMind mmdb 数据格式（`GeoLite2-Country.mmdb`）

- type：（必须）输入格式的名称
- action：（必须）操作类型，值必须为 `output`
- args：（可选）
  - outputName：（可选）输出的文件名
  - outputDir：（可选）输出目录
  - onlyIPType：（可选）输出的 IP 地址类型，值为 `ipv4` 或 `ipv6`
  - wantedList：（可选，数组）指定需要输出的类别
  - overwriteList：（可选，数组）指定最后写入的类别（原因见👇）

> 由于 MaxMind mmdb 文件格式的限制，当不同列表的 IP 或 CIDR 数据有交集或重复项时，后写入的列表的 IP 或 CIDR 数据会覆盖（overwrite）之前已写入的列表的数据。譬如，IP 1.1.1.1 同属于列表 `AU` 和列表 `Cloudflare`。如果 `Cloudflare` 在 `AU` 之后写入，则 IP `1.1.1.1` 归属于列表 `Cloudflare`。
>
> 为了确保某些指定的列表、被修改的列表一定囊括属于它的所有 IP 或 CIDR 数据，可在 output 输出格式为 `maxmindMMDB` 的配置中增加选项 `overwriteList`，该选项中指定的列表会在最后逐一写入，列表中最后一项优先级最高。若已设置选项 `wantedList`，则无需设置 `overwriteList`。`wantedList` 中指定的列表会在最后逐一写入，列表中最后一项优先级最高。

```jsonc
// 默认输出目录 ./output/maxmind
{
  "type": "maxmindMMDB",
  "action": "output"
}
```

```jsonc
{
  "type": "maxmindMMDB",
  "action": "output",
  "args": {
    "outputDir": "./output",                      // 输出文件到 output 目录
    "outputName": "Country-only-cn-private.mmdb", // 输出文件名为 Country-only-cn-private.mmdb
    "wantedList": ["cn", "private"]               // 只输出 cn、private 类别
  }
}
```

```jsonc
{
  "type": "maxmindMMDB",
  "action": "output",
  "args": {
    "outputName": "Country.mmdb",     // 输出文件名为 Country.mmdb
    "overwriteList": ["cn", "google"] // 确保 cn、google 类别后写入，且 google 最后写入
  }
}
```

```jsonc
{
  "type": "maxmindMMDB",
  "action": "output",
  "args": {
    "outputName": "Country.mmdb",      // 输出文件名为 Country.mmdb
    "overwriteList": ["cn", "google"], // 确保 cn、google 类别后写入，且 google 最后写入
    "onlyIPType": "ipv4"               // 只输出 cn、private 类别的 IPv4 地址
  }
}
```

### **mihomoMRS**：mihomo MRS 数据格式（`geoip-cn.mrs`）

- type：（必须）输入格式的名称
- action：（必须）操作类型，值必须为 `output`
- args：（可选）
  - outputDir：（可选）输出目录
  - wantedList：（可选，数组）指定需要输出的类别
  - onlyIPType：（可选）输出的 IP 地址类型，值为 `ipv4` 或 `ipv6`

```jsonc
// 默认输出目录 ./output/mrs
{
  "type": "mihomoMRS",
  "action": "output"
}
```

```jsonc
{
  "type": "mihomoMRS",
  "action": "output",
  "args": {
    "outputDir": "./output",         // 输出文件到 output 目录
    "wantedList": ["cn", "private"]  // 只输出 cn、private 类别
  }
}
```

```jsonc
{
  "type": "mihomoMRS",
  "action": "output",
  "args": {
    "onlyIPType": "ipv4" // 只输出 IPv4 地址
  }
}
```

### **singboxSRS**：sing-box SRS 数据格式（`geoip-cn.srs`）

- type：（必须）输入格式的名称
- action：（必须）操作类型，值必须为 `output`
- args：（可选）
  - outputDir：（可选）输出目录
  - wantedList：（可选，数组）指定需要输出的类别
  - onlyIPType：（可选）输出的 IP 地址类型，值为 `ipv4` 或 `ipv6`

```jsonc
// 默认输出目录 ./output/srs
{
  "type": "singboxSRS",
  "action": "output"
}
```

```jsonc
{
  "type": "singboxSRS",
  "action": "output",
  "args": {
    "outputDir": "./output",        // 输出文件到 output 目录
    "wantedList": ["cn", "private"] // 只输出 cn、private 类别
  }
}
```

```jsonc
{
  "type": "singboxSRS",
  "action": "output",
  "args": {
    "onlyIPType": "ipv4" // 只输出 IPv4 地址
  }
}
```

### **stdout**：将纯文本 CIDR 输出到 standard output（例如：`1.0.0.0/24`）

- type：（必须）输入格式的名称
- action：（必须）操作类型，值必须为 `output`
- args：（可选）
  - wantedList：（可选，数组）指定需要输出的类别
  - onlyIPType：（可选）输出的 IP 地址类型，值为 `ipv4` 或 `ipv6`

```jsonc
{
  "type": "stdout",
  "action": "output" // 输出所有类别到 standard output
}
```

```jsonc
{
  "type": "stdout",
  "action": "output",
  "args": {
    "wantedList": ["cn", "private"] // 只输出 cn、private 类别到 standard output
  }
}
```

```jsonc
{
  "type": "stdout",
  "action": "output",
  "args": {
    "onlyIPType": "ipv4" // 只输出 IPv4 地址到 standard output
  }
}
```

### **surgeRuleSet**：Surge RuleSet

- type：（必须）输入格式的名称
- action：（必须）操作类型，值必须为 `output`
- args：（可选）
  - outputDir：（可选）输出目录
  - outputExtension：（可选）输出的文件的扩展名
  - wantedList：（可选，数组）指定需要输出的类别
  - onlyIPType：（可选）输出的 IP 地址类型，值为 `ipv4` 或 `ipv6`

```jsonc
// 默认输出目录 ./output/surge
{
  "type": "surgeRuleSet",
  "action": "output"
}
```

```jsonc
{
  "type": "surgeRuleSet",
  "action": "output",
  "args": {
    "outputDir": "./surge",    // 输出文件到目录 ./surge
    "outputExtension": ".conf" // 输出文件的扩展名为 .conf
  }
}
```

```jsonc
{
  "type": "surgeRuleSet",
  "action": "output",
  "args": {
    "outputDir": "./surge",          // 输出文件到目录 ./surge
    "outputExtension": ".conf",      // 输出文件的扩展名为 .conf
    "wantedList": ["cn", "us", "jp"] // 只输出名为 cn、us、jp 这三个类别的 IPv4 和 IPv6 地址
  }
}
```

```jsonc
{
  "type": "surgeRuleSet",
  "action": "output",
  "args": {
    "outputDir": "./surge",           // 输出文件到目录 ./surge
    "outputExtension": ".conf",       // 输出文件的扩展名为 .conf
    "wantedList": ["cn", "us", "jp"], // 只输出名为 cn、us、jp 这三个类别的 IPv4 地址
    "onlyIPType": "ipv4"
  }
}
```

### **text**：纯文本 CIDR（例如：`1.0.0.0/24`）

- type：（必须）输入格式的名称
- action：（必须）操作类型，值必须为 `output`
- args：（可选）
  - outputDir：（可选）输出目录
  - outputExtension：（可选）输出的文件的扩展名
  - wantedList：（可选，数组）指定需要输出的类别
  - onlyIPType：（可选）输出的 IP 地址类型，值为 `ipv4` 或 `ipv6`
  - addPrefixInLine：（可选）给输出的每一行添加的字符串前缀
  - addSuffixInLine：（可选）给输出的每一行添加的字符串后缀

```jsonc
// 默认输出目录 ./output/text
{
  "type": "text",
  "action": "output",
  "args": {
    "outputDir": "./text",           // 输出文件到目录 ./text
    "outputExtension": ".conf",      // 输出文件的扩展名为 .conf
    "addPrefixInLine": "IP-CIDR,",
    "addSuffixInLine": ",no-resolve"
  }
}
```

```jsonc
{
  "type": "text",
  "action": "output",
  "args": {
    "outputDir": "./text",           // 输出文件到目录 ./text
    "outputExtension": ".conf",      // 输出文件的扩展名为 .conf
    "addPrefixInLine": "IP-CIDR,",
    "addSuffixInLine": ",no-resolve"
  }
}
```

```jsonc
{
  "type": "text",
  "action": "output",
  "args": {
    "outputDir": "./text",            // 输出文件到目录 ./text
    "outputExtension": ".conf",       // 输出文件的扩展名为 .conf
    "wantedList": ["cn", "us", "jp"], // 只输出名为 cn、us、jp 这三个类别的 IPv4 和 IPv6 地址
    "addPrefixInLine": "HOST,"
  }
}
```

```jsonc
{
  "type": "text",
  "action": "output",
  "args": {
    "outputDir": "./text",            // 输出文件到目录 ./text
    "outputExtension": ".conf",       // 输出文件的扩展名为 .conf
    "wantedList": ["cn", "us", "jp"], // 只输出名为 cn、us、jp 这三个类别的 IPv4 地址
    "onlyIPType": "ipv4",
    "addSuffixInLine": ";"
  }
}
```

### **v2rayGeoIPDat**：V2Ray GeoIP dat 数据格式（`geoip.dat`）

- type：（必须）输入格式的名称
- action：（必须）操作类型，值必须为 `output`
- args：（可选）
  - outputName：（可选）输出的文件名
  - outputDir：（可选）输出目录
  - wantedList：（可选，数组）指定需要输出的类别
  - onlyIPType：（可选）输出的 IP 地址类型，值为 `ipv4` 或 `ipv6`
  - oneFilePerList：（可选）每个类别输出为一个单独的文件，值为 `true` 或 `false`（默认值）

```jsonc
// 默认输出目录 ./output/dat
{
  "type": "v2rayGeoIPDat",
  "action": "output"      // 输出全部类别
}
```

```jsonc
{
  "type": "v2rayGeoIPDat",
  "action": "output",
  "args": {
    "oneFilePerList": true // 每个类别输出为一个单独的文件
  }
}
```

```jsonc
{
  "type": "v2rayGeoIPDat",
  "action": "output",
  "args": {
    "outputDir": "./output",                   // 输出文件到 output 目录
    "outputName": "geoip-only-cn-private.dat", // 输出文件名为 geoip-only-cn-private.dat
    "wantedList": ["cn", "private"]            // 只输出 cn、private 类别
  }
}
```

```jsonc
{
  "type": "v2rayGeoIPDat",
  "action": "output",
  "args": {
    "outputName": "geoip-asn.dat",        // 输出文件名为 geoip-asn.dat
    "wantedList": ["telegram", "google"], // 只输出 telegram、google
    "onlyIPType": "ipv4"                  // 只输出 telegram、google 类别的 IPv4 地址
  }
}
```

```jsonc
{
  "type": "v2rayGeoIPDat",
  "action": "output",
  "args": {
    "wantedList": ["telegram", "google"], // 只输出 telegram、google、cloudflare
    "onlyIPType": "ipv4",                 // 只输出 telegram、google、cloudflare 类别的 IPv4 地址
    "oneFilePerList": true                // 每个类别输出为一个单独的文件
  }
}
```
