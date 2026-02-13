# PassWall2

[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](https://www.gnu.org/licenses/gpl-3.0)
[![OpenWrt](https://img.shields.io/badge/OpenWrt-21.02%2B-blue)](https://openwrt.org/)
[![LuCI](https://img.shields.io/badge/LuCI-19.07%2B-green)](https://github.com/openwrt/luci)

PassWall2 is a powerful LuCI web interface application for OpenWrt that provides advanced proxy and VPN functionality. It's a comprehensive solution for network traffic management, proxy services, and access control on OpenWrt-based routers.

## 🛠️ Installation

### ⚠️ Pre-installation (Recommended)

```bash
# find it in releases,base of your router Arch
```
**Visit [GitHub Releases](https://github.com/Openwrt-Passwall/openwrt-passwall2/releases/latest) to download the correct package for your system.**

Choose the package format based on your **router's OpenWrt package manager**:

### For OpenWrt with OPKG 

1. **Download the IPK package** from the releases page  
   Look for `luci-app-passwall2_{VERSION}_all.ipk` in the Assets section.  
   > **Note:** If you need localization (Chinese/Persian), download the corresponding language package as well (e.g., `luci-i18n-passwall2-zh-cn...` or `...-fa...`).

2. **Upload to your router** (via SCP, LuCI upload, or wget):
   ```bash
   # Replace {VERSION} with the actual version (e.g., 26.2.5-1)
   wget https://github.com/Openwrt-Passwall/openwrt-passwall2/releases/download/{VERSION}/luci-app-passwall2_{VERSION}_all.ipk
   ```

3. **Install:**
   ```bash
   opkg update
   opkg install luci-app-passwall2_*.ipk
   ```
   
   > If installation fails due to missing dependencies (e.g., `xray-core`), you need to add the PassWall packages feed to `/etc/opkg/customfeeds.conf`.

### For OpenWrt with APK 

1. **Download the APK package** from the releases page  
   Look for `luci-app-passwall2_{VERSION}_all.apk` in the Assets section.
   > **Note:** If you need localization (Chinese/Persian), download the corresponding language package as well (e.g., `luci-i18n-passwall2-zh-cn...` or `...-fa...`).

2. **Upload to your router** (via SCP, LuCI upload, or wget):
   ```bash
   # Replace {VERSION} with the actual version (e.g., 26.2.5-1)
   wget https://github.com/Openwrt-Passwall/openwrt-passwall2/releases/download/{VERSION}/luci-app-passwall2_{VERSION}_all.apk
   ```

3. **Install:**
   ```bash
   apk add --allow-untrusted luci-app-passwall2_*.apk
   ```
   
   > ⚠️ **Security Note:** `--allow-untrusted` bypasses package signature verification. 

> **How to check your package manager:** Run `opkg --version` or `apk --version` to see which one your router uses.

### Restart LuCI
```bash
/etc/init.d/rpcd restart
```

## 📋 System Requirements

### OpenWrt Version
- OpenWrt 21.02 or later
- LuCI 19.07 or later

### Hardware Requirements
- **Minimum 128MB RAM** (256MB recommended for stability with Xray/VLESS)
- Sufficient storage for packages (varies by protocol selection)

### Core Dependencies
The following packages should be resolved automatically by the package manager (if available in your feeds):

- `coreutils`, `coreutils-base64`, `coreutils-nohup`
- `curl`, `ip-full`, `libuci-lua`, `lua`, `luci-compat`, `luci-lib-jsonc`
- `resolveip`, `tcping`, `unzip`
- `xray-core` (core proxy component)
- `geoview`, `v2ray-geoip`, `v2ray-geosite` (geo-routing data)

> **Note:** Actual dependencies may vary based on selected features and your OpenWrt build. Ensure you have the necessary feeds configured.

### Optional Protocol Packages
Selected during installation based on your needs:
- Shadowsocks (Libev/Rust), ShadowsocksR
- V2Ray/Xray (VMess, VLESS, Trojan)
- Sing-Box, Hysteria, Hysteria2, TUIC
- NaiveProxy, HAProxy

## 🚀 Features

### Multi-Protocol Support
- **Shadowsocks** (Libev & Rust implementations)
- **V2Ray/Xray** with full protocol support (VMess, VLESS, Trojan)
- **Sing-Box** with modern proxy features
- **Hysteria** and **Hysteria2** for high-performance UDP transport
- **TUIC** client support
- **NaiveProxy** for advanced obfuscation
- **ShadowsocksR** legacy support

### Traffic Management
- **Load Balancing**: Distribute traffic across multiple nodes
- **Smart Routing**: Domain-based and geo-based routing rules
- **DNS Control**: Advanced DNS filtering and DoH/DoT support
- **Transparent Proxy**: Seamless network-wide proxy

### Node Management
- **Subscription Support**: Import nodes from subscription URLs
- **Node Testing**: Built-in latency and connectivity testing
- **Failover Support**: Automatic failover to backup nodes
- **QR Code**: Generate and scan QR codes for node sharing

### Access Control
- **Per-Device Rules**: Configure proxy settings per device
- **Domain/IP Filtering**: Whitelist/blacklist support
- **Time-based Rules**: Schedule proxy usage

## ⚙️ Configuration

### Basic Setup

1. **Access LuCI Interface:**
   - Navigate to `Services` → `PassWall2`

2. **Add Your First Node:**
   - Go to `Node List` → `Add Node`
   - Select protocol and fill in server details

3. **Configure Basic Settings:**
   - Select your default node
   - Configure DNS settings
   - Enable transparent proxy

4. **Apply Configuration:**
   - Click `Save & Apply`

## 🌐 Language Support

PassWall2 supports multiple languages:
- 🇨🇳 Chinese (Simplified/Traditional)
- 🇮🇷 Persian (فارسی)

Language files are organized in `luci-app-passwall2/po/` subdirectories.

## 🔧 Troubleshooting

### Common Issues

**Service Won't Start**
```bash
logread | grep passwall2
```
Check system logs, verify node configuration, and ensure required packages are installed.

**DNS Issues**
- Disable built-in DNS in browsers (Chrome: Settings → Privacy → Security → Disable "Use secure DNS")
- Clear DNS cache after reboot: `ipconfig /flushdns` (Windows) or toggle airplane mode (mobile)

**Connection Problems**
- Test node connectivity
- Check firewall rules
- Verify transparent proxy settings

### Debug Mode

Enable debug logging in `Other Settings` and check:
- Main log: `/tmp/log/passwall2.log`
- Server log: `/tmp/log/passwall2_server.log`

## 📄 License

This project is licensed under the GNU General Public License v3.0 - see the [LICENSE](LICENSE) file for details.

**Note**: This software is intended for legal use only. Users are responsible for complying with all applicable laws and regulations in their jurisdiction.

---

## Stargazers over time
[![Stargazers over time](https://starchart.cc/Openwrt-Passwall/openwrt-passwall2.svg?variant=adaptive)](https://starchart.cc/Openwrt-Passwall/openwrt-passwall2)