# PassWall2

[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](https://www.gnu.org/licenses/gpl-3.0)
[![OpenWrt](https://img.shields.io/badge/OpenWrt-21.02%2B-blue)](https://openwrt.org/)
[![LuCI](https://img.shields.io/badge/LuCI-19.07%2B-green)](https://github.com/openwrt/luci)

PassWall2 is a powerful LuCI web interface application for OpenWrt that provides advanced proxy and VPN functionality. It's a comprehensive solution for network traffic management, proxy services, and access control on OpenWrt-based routers.




## 🛠️ Installation 

### Method 1: Using OpenWrt Package Manager

1. **Update package lists:**
   ```bash
   opkg update
   ```

2. **Install PassWall2:**
   ```bash
   opkg install luci-app-passwall2
   ```

3. **Restart LuCI:**
   ```bash
   /etc/init.d/rpcd restart
   ```

### Method 2: Manual Installation

1. **Download the package:**
   ```bash
   wget https://github.com/Openwrt-Passwall/openwrt-passwall2/releases/latest/download/luci-app-passwall2_*.ipk
   ```

2. **Install the package:**
   ```bash
   opkg install luci-app-passwall2_*.ipk
   ```
<details>

<summary>📋 System Requirements </summary>

### OpenWrt Version
- OpenWrt 21.02 or later
- LuCI 19.07 or later

### Hardware Requirements
- Minimum 64MB RAM (128MB recommended)
- Sufficient storage for packages (varies by protocol selection)
- Network interface support for transparent proxy

### Dependencies
The following packages are automatically installed based on your configuration:
- `coreutils`, `curl`, `ip-full`, `libuci-lua`, `lua`, `luci-compat`
- Protocol-specific packages (selected during installation)

</details>

<details>

<summary>⚙️ Configuration </summary>

### Basic Setup

1. **Access LuCI Interface:**
   - Navigate to `Services` → `PassWall2` in your OpenWrt web interface

2. **Add Your First Node:**
   - Go to `Node List` → `Add Node`
   - Select your protocol (e.g., Shadowsocks, V2Ray, etc.)
   - Fill in server details (address, port, password, encryption)

3. **Configure Basic Settings:**
   - Go to `Basic Settings`
   - Select your default node
   - Configure DNS settings
   - Enable transparent proxy

4. **Apply Configuration:**
   - Click `Save & Apply`
   - Wait for services to start

</details>


<details>
<summary>🚀 Features</summary>

### Multi-Protocol Proxy Support
- **Shadowsocks** (Libev & Rust implementations)
- **V2Ray/Xray** with full protocol support
- **Trojan** and **Trojan-Go**
- **NaiveProxy** for advanced obfuscation
- **Hysteria** and **Hysteria2** for high-performance UDP transport
- **Sing-Box** with modern proxy features
- **ShadowsocksR** legacy support
- **WireGuard** integration

### Advanced Traffic Management
- **Load Balancing**: Distribute traffic across multiple nodes
- **URL Testing**: Automatically test and select optimal nodes
- **Smart Routing**: Domain-based and geo-based routing rules
- **DNS Manipulation**: Advanced DNS filtering and manipulation
- **Traffic Sniffing**: Protocol detection and classification

### Node Management
- **Subscription Support**: Import nodes from subscription URLs
- **QR Code Generation**: Generate and scan QR codes for node sharing
- **Node Testing**: Built-in latency and connectivity testing
- **Health Checks**: Automatic node health monitoring
- **Failover Support**: Automatic failover to backup nodes

### Access Control
- **Per-Device Rules**: Configure proxy settings per device
- **Domain Filtering**: Whitelist/blacklist domains
- **IP Filtering**: IP-based access control
- **Interface Control**: Route traffic based on network interfaces
- **Time-based Rules**: Schedule proxy usage by time

### Server-Side Support
- **Multi-User Server**: Host proxy services with user management
- **Protocol Support**: All client protocols available as servers
- **User Management**: Create and manage server users
- **Traffic Monitoring**: Monitor server usage and statistics

### Advanced Features
- **Multi-WAN Support**: Advanced routing for multi-WAN setups
- **IPv6 Support**: Full IPv6 transparency and proxy support
- **Transparent Proxy**: Transparent proxy for entire network
- **Socks5/HTTP Proxy**: Local proxy server support
- **NAT/Firewall Integration**: Seamless integration with OpenWrt firewall

</details>

<details>
<summary>🔧 Advanced Configuration</summary>

#### Load Balancing
1. Create multiple nodes of the same type
2. Go to `Node List` → `Add Node` → `Load Balancing`
3. Select nodes to include in the load balancer
4. Configure balancing strategy and health checks

#### Access Control
1. Go to `Access Control`
2. Add devices by MAC address or IP range
3. Configure proxy rules for each device
4. Set up domain and IP filtering

#### DNS Configuration
1. Go to `Basic Settings` → `DNS Settings`
2. Configure direct and remote DNS servers
3. Set up DNS filtering rules
4. Enable DNS over HTTPS if desired

</details>

<details>
<summary>📚 Supported Protocols</summary>

### Shadowsocks
- **Libev**: Lightweight implementation
- **Rust**: Modern, high-performance implementation
- **Plugins**: Simple-obfs, v2ray-plugin support
- **Encryption**: All standard encryption methods

### V2Ray/Xray
- **Protocols**: VMess, VLESS, Trojan, Shadowsocks
- **Transports**: TCP, mKCP, WebSocket, HTTP/2, QUIC
- **Security**: TLS, XTLS support
- **Features**: Routing, DNS,流量控制

### Trojan
- **Standard Trojan**: Basic trojan protocol
- **Trojan-Go**: Enhanced with additional features
- **TLS Support**: Full TLS certificate support
- **Obfuscation**: Built-in traffic obfuscation

### Hysteria
- **Hysteria 1**: UDP-based transport protocol
- **Hysteria 2**: Improved version with better performance
- **Obfuscation**: Built-in traffic obfuscation
- **UDP Optimization**: Optimized for poor network conditions

### Sing-Box
- **Modern Architecture**: Latest proxy technology
- **Protocol Support**: All major proxy protocols
- **Performance**: High-performance implementation
- **Features**: Advanced routing and filtering

</details>

<details>
<summary>🌐 Language Support</summary>

PassWall2 supports multiple languages:
- 🇨🇳 Chinese (Simplified/Traditional)
- 🇮🇷 Persian_farsi (soon)

Language files are located in `luci-app-passwall2/po/` directory.

</details>


<details>
<summary>🔧 Troubleshooting</summary>

### Common Issues

#### Service Won't Start
1. Check system logs: `logread | grep passwall2`
2. Verify node configuration
3. Check available memory and storage
4. Ensure required packages are installed

#### DNS Issues
1. Verify DNS server configuration
2. Check DNS filtering rules
3. Test with different DNS servers
4. Clear DNS cache if needed

#### Connection Problems
1. Test node connectivity
2. Check firewall rules
3. Verify transparent proxy settings
4. Test with different protocols

#### Performance Issues
1. Monitor system resources
2. Check node health status
3. Adjust connection limits
4. Optimize routing rules

### Debug Mode

Enable debug logging:
1. Go to `Other Settings`
2. Enable debug mode
3. Check logs in `/tmp/log/passwall2.log`

### Log Locations
- Main log: `/tmp/log/passwall2.log`
- Server log: `/tmp/log/passwall2_server.log`
- Temporary files: `/tmp/etc/passwall2_tmp/`

</details>

<details>
<summary>🙏 Acknowledgments</summary>

- OpenWrt community for the excellent platform
- V2Ray/Xray project for the core proxy technology
- All contributors and testers
- The open-source community

</details>

## 📄 License

This project is licensed under the GNU General Public License v3.0 - see the [LICENSE](LICENSE) file for details.


---

**Note**: This software is intended for legal use only. Users are responsible for complying with all applicable laws and regulations in their jurisdiction.


## Stargazers over time
[![Stargazers over time](https://starchart.cc/Openwrt-Passwall/openwrt-passwall2.svg?variant=adaptive)](https://starchart.cc/Openwrt-Passwall/openwrt-passwall2)