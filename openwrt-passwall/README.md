## 📌如何能编译到最新代码？

### 方法1：

执行 `./scripts/feeds update -a` 操作前，在 `feeds.conf.default` **顶部**插入如下代码：

```
src-git passwall_packages https://github.com/Openwrt-Passwall/openwrt-passwall-packages.git;main
src-git passwall_luci https://github.com/Openwrt-Passwall/openwrt-passwall.git;main
```

### 方法2：

在 `./scripts/feeds install -a` 操作完成后，执行以下命令：

```shell
# 移除 openwrt feeds 自带的核心库
rm -rf feeds/packages/net/{xray-core,v2ray-geodata,sing-box,chinadns-ng,dns2socks,hysteria,ipt2socks,microsocks,naiveproxy,shadowsocks-rust,shadowsocksr-libev,simple-obfs,tcping,v2ray-plugin,xray-plugin,geoview,shadow-tls,smartdns}
git clone https://github.com/Openwrt-Passwall/openwrt-passwall-packages package/passwall-packages

# 移除 openwrt feeds 过时的luci版本
rm -rf feeds/luci/applications/luci-app-passwall
git clone https://github.com/Openwrt-Passwall/openwrt-passwall package/passwall-luci
```

## 🚀 配套固件与工具

### 🔥 [OpenWrtAction](https://github.com/smallprogram/OpenWrtAction) —— 开箱即用的 Passwall 固件

[![Stars](https://img.shields.io/github/stars/smallprogram/OpenWrtAction?style=flat-square)](https://github.com/smallprogram/OpenWrtAction)

基于 GitHub Actions 全自动云编译的固件仓库，集成最新 Passwall，刷入即用，无需自行折腾编译环境。

- 📦 **多源码分支**：官方 OpenWrt 25.12、ImmortalWrt 25.12、Lean (Snapshot)
- 🖥️ **多设备覆盖**：x86、R2S、R2C、R4S、R5S、R5C、H66K、H68K、H69K、R66S、R68S 等
- 🔄 **持续更新**：CI 自动构建，紧跟上游与 Passwall 最新版本
- ⚡ **GitHub 一键更新**：支持直接从 GitHub Releases 一键在线升级固件，告别手动下载、上传、刷写

👉 前往下载：[OpenWrtAction Releases](https://github.com/smallprogram/OpenWrtAction/releases)

### 🧩 [Are-u-ok](https://github.com/bcseputetto/Are-u-ok) —— iStoreOS `.run` 更新包

[![Stars](https://img.shields.io/github/stars/bcseputetto/Are-u-ok?style=flat-square)](https://github.com/bcseputetto/Are-u-ok)

为 **iStoreOS** 用户提供 `.run` 格式的 Passwall 更新包，无需重刷固件，上传到设备执行即可完成安装/升级。

👉 前往获取：[Are-u-ok](https://github.com/bcseputetto/Are-u-ok)

---



