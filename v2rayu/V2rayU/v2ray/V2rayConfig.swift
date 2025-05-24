//
//  V2rayConfig.swift
//  V2rayU
//
//  Created by yanue on 2018/10/25.
//  Copyright © 2018 yanue. All rights reserved.
//

import Cocoa
import SwiftyJSON
import JavaScriptCore

let jsSourceFormatConfig =
        """
        /**
         * V2ray Config Format
         * @return {string}
         */
        var V2rayConfigFormat = function (encodeV2rayStr, encodeDnsStr) {
            var deV2rayStr = decodeURIComponent(encodeV2rayStr);
            if (!deV2rayStr) {
                return "error: cannot decode uri"
            }

            var dns = {};
            try {
                dns = JSON.parse(decodeURIComponent(encodeDnsStr));
            } catch (e) {
                console.log("error", e);
            }

            try {
                var obj = JSON.parse(deV2rayStr);
                if (!obj) {
                    return "error: cannot parse json"
                }

                var v2rayConfig = {};
                // ordered keys
                v2rayConfig["log"] = obj.log;
                v2rayConfig["inbounds"] = obj.inbounds;
                v2rayConfig["outbounds"] = obj.outbounds;
                v2rayConfig["api"] = obj.api;
                v2rayConfig["dns"] = dns;
                v2rayConfig["stats"] = obj.stats;
                v2rayConfig["routing"] = obj.routing;
                v2rayConfig["policy"] = obj.policy;
                v2rayConfig["reverse"] = obj.reverse;
                v2rayConfig["transport"] = obj.transport;

                return JSON.stringify(v2rayConfig, null, 2);
            } catch (e) {
                console.log("error", e);
                return "error: " + e.toString()
            }
        };


        /**
         * json beauty Format
         * @return {string}
         */
        var JsonBeautyFormat = function (en64Str) {
            var deStr = decodeURIComponent(en64Str);
            if (!deStr) {
                return "error: cannot decode uri"
            }
            try {
                var obj = JSON.parse(deStr);
                if (!obj) {
                    return "error: cannot parse json"
                }

                return JSON.stringify(obj, null, 2);
            } catch (e) {
                console.log("error", e);
                return "error: " + e.toString()
            }
        };
        """


class V2rayConfig: NSObject {

    var v2ray: V2rayStruct = V2rayStruct()
    var isValid = false

    var error = ""
    var errors: [String] = []

    // base
    var logLevel = "info"
    var socksPort = "1080"
    var socksHost = "127.0.0.1"
    var httpPort = "1087"
    var httpHost = "127.0.0.1"
    var enableSocks = true
    var enableUdp = false
    var enableMux = false
    var enableSniffing = false
    var mux = 8
    var dnsJson = UserDefaults.get(forKey: .v2rayDnsJson) ?? ""

    // server
    var serverProtocol = V2rayProtocolOutbound.vmess.rawValue
    var serverVmess = V2rayOutboundVMessItem()
    var serverSocks5 = V2rayOutboundSocks()
    var serverShadowsocks = V2rayOutboundShadowsockServer()
    var serverVless = V2rayOutboundVLessItem()
    var serverTrojan = V2rayOutboundTrojanServer()

    // transfer
    var streamNetwork = V2rayStreamSettings.network.tcp.rawValue
    var streamTcp = TcpSettings()
    var streamKcp = KcpSettings()
    var streamDs = DsSettings()
    var streamWs = WsSettings()
    var streamXhttp = XhttpSettings()
    var streamH2 = HttpSettings()
    var streamQuic = QuicSettings()
    var streamGrpc = GrpcSettings()
    var routing = V2rayRouting()

    // tls 默认需为none,shadowsocks需为none
    var streamSecurity = "none" // none|tls|xtls|reality
    var securityTls = TlsSettings() // tls|xtls
    var securityReality = RealitySettings() // reality

    private var foundHttpPort = false
    private var foundSockPort = false
    private var foundServerProtocol = false

    // Initialization
    override init() {
        super.init()

        self.enableMux = UserDefaults.getBool(forKey: .enableMux)
        self.enableUdp = UserDefaults.getBool(forKey: .enableUdp)
        self.enableSniffing = UserDefaults.getBool(forKey: .enableSniffing)

        self.httpPort = UserDefaults.get(forKey: .localHttpPort) ?? "1087"
        self.httpHost = UserDefaults.get(forKey: .localHttpHost) ?? "127.0.0.1"
        self.socksPort = UserDefaults.get(forKey: .localSockPort) ?? "1080"
        self.socksHost = UserDefaults.get(forKey: .localSockHost) ?? "127.0.0.1"

        self.mux = Int(UserDefaults.get(forKey: .muxConcurrent) ?? "8") ?? 8

        self.logLevel = UserDefaults.get(forKey: .v2rayLogLevel) ?? "info"
    }

    // combine manual edited data
    // by manual tab view
    func combineManual() -> String {
        // combine data
        self.combineManualData()

        // 1. encode to json text
        let encoder = JSONEncoder()
        let data = try! encoder.encode(self.v2ray)
        var jsonStr = String(data: data, encoding: .utf8)!

        // 2. format json text by javascript
        jsonStr = self.formatJson(json: jsonStr)

        return jsonStr
    }

    func formatJson(json: String) -> String {
        var jsonStr = json
        if let context = JSContext() {
            context.evaluateScript(jsSourceFormatConfig)
            // call js func
            if let formatFunction = context.objectForKeyedSubscript("V2rayConfigFormat") {
                let escapedV2String = jsonStr.addingPercentEncoding(withAllowedCharacters: .urlHostAllowed)
                let escapedDnsString = self.dnsJson.addingPercentEncoding(withAllowedCharacters: .urlHostAllowed)
                if let result = formatFunction.call(withArguments: [escapedV2String as Any, escapedDnsString as Any]) {
                    // error occurred with prefix "error:"
                    if let reStr = result.toString(), reStr.count > 0 {
                        if !reStr.hasPrefix("error:") {
                            // replace json str
                            jsonStr = reStr
                        } else {
                            self.error = reStr
                        }
                    }
                }
            }
        }

        return jsonStr
    }

    func combineManualData() {
        // base
        self.v2ray.log.loglevel = V2rayLog.logLevel(rawValue: UserDefaults.get(forKey: .v2rayLogLevel) ?? "info") ?? V2rayLog.logLevel.info

        // ------------------------------------- inbound start ---------------------------------------------
        var inSocks = V2rayInbound()
        inSocks.port = self.socksPort
        inSocks.listen = self.socksHost
        inSocks.protocol = V2rayProtocolInbound.socks
        inSocks.settingSocks.udp = self.enableUdp
        if self.enableSniffing {
            inSocks.sniffing = V2rayInboundSniffing()
        }

        // check same
        if self.httpPort == self.socksPort {
            self.httpPort = String((Int(self.socksPort) ?? 1080) + 1)
        }
        var inHttp = V2rayInbound()
        inHttp.port = self.httpPort
        inHttp.listen = self.httpHost
        inHttp.protocol = V2rayProtocolInbound.http
        if self.enableSniffing {
            inHttp.sniffing = V2rayInboundSniffing()
        }

        // inbounds
        var inbounds: [V2rayInbound] = []
        if (self.v2ray.inbounds != nil && self.v2ray.inbounds!.count > 0) {
            for (_, item) in self.v2ray.inbounds!.enumerated() {
                if item.protocol == V2rayProtocolInbound.http || item.protocol == V2rayProtocolInbound.socks {
                    continue
                }
                inbounds.append(item)
            }
        }
        // for ping just use http
        if self.enableSocks {
            inbounds.append(inSocks)
        }
        inbounds.append(inHttp)
        self.v2ray.inbounds = inbounds

        // ------------------------------------- inbound end ----------------------------------------------

        // ------------------------------------- outbound start -------------------------------------------
        // outbound Freedom
        var outboundFreedom = V2rayOutbound()
        outboundFreedom.protocol = V2rayProtocolOutbound.freedom
        outboundFreedom.tag = "direct"
        outboundFreedom.settingFreedom = V2rayOutboundFreedom()

        // outbound Blackhole
        var outboundBlackhole = V2rayOutbound()
        outboundBlackhole.protocol = V2rayProtocolOutbound.blackhole
        outboundBlackhole.tag = "block"
        outboundBlackhole.settingBlackhole = V2rayOutboundBlackhole()

        // outbound
        let outbound = self.getOutbound() // get from setting
        var outbounds: [V2rayOutbound] = [outbound]

        if (self.v2ray.outbounds != nil && self.v2ray.outbounds!.count > 0) {
            for var (i, item) in self.v2ray.outbounds!.enumerated() {
                // the first one is just from manual settings
                if i == 0 {
                    continue
                }
                // ignore freedom and blackhole
                if item.protocol == V2rayProtocolOutbound.freedom || item.protocol == V2rayProtocolOutbound.blackhole {
                    continue
                }
                outbounds.append(item)
            }
        }
        outbounds.append(outboundFreedom)
        outbounds.append(outboundBlackhole)

        self.v2ray.outbounds = outbounds

        // ------------------------------------- outbound end ---------------------------------------------

        // ------------------------------------- routing start --------------------------------------------
        let routingRule = UserDefaults.get(forKey: .routingSelectedRule) ?? RoutingRuleGlobal
        let rule = RoutingItem.load(name: routingRule)
        if rule != nil{
            self.v2ray.routing = rule!.parseRule()
        }
        // ------------------------------------- routing end ----------------------------------------------
    }

    func checkManualValid() {
        defer {
            if self.error != "" {
                self.isValid = false
            } else {
                self.isValid = true
            }
        }
        // reset error first
        self.error = ""
        // check main outbound
        switch self.serverProtocol {
        case V2rayProtocolOutbound.vmess.rawValue:
            if self.serverVmess.address.count == 0 {
                self.error = "missing vmess.address";
                return
            }
            if self.serverVmess.port == 0 {
                self.error = "missing vmess.port";
                return
            }
            if self.serverVmess.users.count > 0 {
                if self.serverVmess.users[0].id.count == 0 {
                    self.error = "missing vmess.users[0].id";
                    return
                }
            } else {
                self.error = "missing vmess.users";
                return
            }
            break
        case V2rayProtocolOutbound.vless.rawValue:
            if self.serverVless.address.count == 0 {
                self.error = "missing vmess.address"
                return
            }

            if self.serverVless.port == 0 {
                self.error = "missing vmess.port"
                return
            }

            if self.serverVless.users.count > 0 {
                if self.serverVless.users[0].id.count == 0 {
                    self.error = "missing vless.users[0].id"
                    return
                }
            } else {
                self.error = "missing vless.users"
                return
            }
            break
        case V2rayProtocolOutbound.shadowsocks.rawValue:
            if self.serverShadowsocks.address.count == 0 {
                self.error = "missing shadowsocks.address";
                return
            }
            if self.serverShadowsocks.port == 0 {
                self.error = "missing shadowsocks.port";
                return
            }
            if self.serverShadowsocks.password.count == 0 {
                self.error = "missing shadowsocks.password";
                return
            }
            if self.serverShadowsocks.method.count == 0 {
                self.error = "missing shadowsocks.method";
                return
            }
            self.streamSecurity = "none" // 需为none
            break
        case V2rayProtocolOutbound.socks.rawValue:
            if self.serverSocks5.servers.count == 0 {
                self.error = "missing socks.servers";
                return
            }
            if self.serverSocks5.servers[0].address.count == 0 {
                self.error = "missing socks.address";
                return
            }
            if self.serverSocks5.servers[0].port == 0 {
                self.error = "missing socks.port";
                return
            }
            break
        case V2rayProtocolOutbound.trojan.rawValue:
            if self.serverTrojan.address.count == 0 {
                self.error = "missing trojan.address"
                return
            }

            if self.serverTrojan.port == 0 {
                self.error = "missing trojan.port"
                return
            }
            break
        default:
            self.error = "missing outbound.protocol";
            return
        }

        // check stream setting
        switch self.streamNetwork {
        case V2rayStreamSettings.network.h2.rawValue:
            break
        case V2rayStreamSettings.network.ws.rawValue:
            break
        default:
            break
        }
    }

    private func getOutbound() -> V2rayOutbound {
        var outbound = V2rayOutbound()
        outbound.protocol = V2rayProtocolOutbound(rawValue: self.serverProtocol)!
        outbound.tag = "proxy"

        switch outbound.protocol {
        case V2rayProtocolOutbound.vmess:
            var vmess = V2rayOutboundVMess()
            vmess.vnext = [self.serverVmess]
            outbound.settingVMess = vmess

            // enable mux only vmess
            var mux = V2rayOutboundMux()
            mux.enabled = self.enableMux
            mux.concurrency = self.mux
            outbound.mux = mux

            break
        case V2rayProtocolOutbound.vless:
            var vless = V2rayOutboundVLess()
            vless.vnext = [self.serverVless]
            outbound.settingVLess = vless

            var mux = V2rayOutboundMux()
            mux.enabled = false
            mux.concurrency = self.mux
            outbound.mux = mux

            break
        case V2rayProtocolOutbound.shadowsocks:
            var ss = V2rayOutboundShadowsocks()
            ss.servers = [self.serverShadowsocks]
            outbound.settingShadowsocks = ss
            break

        case V2rayProtocolOutbound.socks:
            outbound.settingSocks = self.serverSocks5
            break

        case V2rayProtocolOutbound.trojan:
            var trojan = V2rayOutboundTrojan()
            trojan.servers = [self.serverTrojan]
            outbound.settingTrojan = trojan

            var mux = V2rayOutboundMux()
            mux.enabled = false
            mux.concurrency = self.mux
            outbound.mux = mux
            break

        default:
            break
        }

        outbound.streamSettings = self.getStreamSettings()

        return outbound
    }

    private func getStreamSettings() -> V2rayStreamSettings {
        // streamSettings
        var streamSettings = V2rayStreamSettings()
        streamSettings.network = V2rayStreamSettings.network(rawValue: self.streamNetwork) ?? V2rayStreamSettings.network.tcp
        switch streamSettings.network {
        case .tcp:
            streamSettings.tcpSettings = self.streamTcp
            break
        case .kcp:
            streamSettings.kcpSettings = self.streamKcp
            break
        case .xhttp:
            streamSettings.xhttpSettings = self.streamXhttp
            break
        case .http, .h2:
            streamSettings.httpSettings = self.streamH2
            break
        case .ws:
            streamSettings.wsSettings = self.streamWs
            break
        case .domainsocket:
            streamSettings.dsSettings = self.streamDs
            break
        case .quic:
            streamSettings.quicSettings = self.streamQuic
            break
        case .grpc:
            streamSettings.grpcSettings = self.streamGrpc
            break
        }

        if self.streamSecurity == "tls" {
            streamSettings.security = .tls
            streamSettings.tlsSettings = self.securityTls
        }

        if self.streamSecurity == "xtls" {
            streamSettings.security = .xtls
            streamSettings.xtlsSettings = self.securityTls
        }

        if self.streamSecurity == "reality" {
            streamSettings.security = .reality
            streamSettings.realitySettings = self.securityReality
        }

        return streamSettings
    }

    // parse imported or edited json text
    // by import tab view
    func parseJson(jsonText: String) {
        defer {
            if self.errors.count > 0 {
                self.isValid = false
            } else {
                self.isValid = true
            }
        }

        self.errors = []

        guard let json = try? JSON(data: jsonText.data(using: String.Encoding.utf8, allowLossyConversion: false)!) else {
            self.errors += ["invalid json"]
            return
        }

        if !json.exists() {
            self.errors += ["invalid json"]
            return
        }

        // ignore dns,  use default

        // ============ parse inbound start =========================================
        // use default
        // ------------ parse inbound end -------------------------------------------

        // ============ parse outbound start =========================================
        // > 4.0
        if json["outbounds"].exists() {
            // check outbounds
            if json["outbounds"].arrayValue.count > 0 {
                // outbounds
                var outbounds: [V2rayOutbound] = []
                json["outbounds"].arrayValue.forEach {
                    val in
                    outbounds += [self.parseOutbound(jsonParams: val)]
                }
                self.v2ray.outbounds = outbounds
            } else {
                self.errors += ["missing outbounds"]
            }
        } else {
            // check outbounds
            var outbounds: [V2rayOutbound] = []

            // 1. outbound
            if json["outbound"].dictionaryValue.count > 0 {
                outbounds += [self.parseOutbound(jsonParams: json["outbound"])]
            } else {
                self.errors += ["missing outbound"]
            }

            // outboundDetour
            if json["outboundDetour"].arrayValue.count > 0 {
                json["outboundDetour"].arrayValue.forEach {
                    val in
                    outbounds += [self.parseOutbound(jsonParams: val)]
                }
            }
            self.v2ray.outbounds = outbounds
        }
        // ------------ parse outbound end -------------------------------------------
    }

    // parse outbound from json
    func parseOutbound(jsonParams: JSON) -> (V2rayOutbound) {
        var v2rayOutbound = V2rayOutbound()

        if !(jsonParams["protocol"].exists()) {
            self.errors += ["missing outbound.protocol"]
            return (v2rayOutbound)
        }

        if (V2rayProtocolOutbound(rawValue: jsonParams["protocol"].stringValue) == nil) {
            self.errors += ["invalid outbound.protocol"]
            return (v2rayOutbound)
        }

        // set protocol
        v2rayOutbound.protocol = V2rayProtocolOutbound(rawValue: jsonParams["protocol"].stringValue)!

        v2rayOutbound.sendThrough = jsonParams["sendThrough"].stringValue

        // fix Outbound tag
        switch v2rayOutbound.protocol {
        case .freedom:
            v2rayOutbound.tag = "direct"
        case .blackhole:
            v2rayOutbound.tag = "block"
        default:
            v2rayOutbound.tag = "proxy"
        }

        // settings depends on protocol
        if jsonParams["settings"].dictionaryValue.count > 0 {
            switch v2rayOutbound.protocol {
            case .blackhole:
                var settingBlackhole = V2rayOutboundBlackhole()
                settingBlackhole.response.type = jsonParams["settings"]["response"]["type"].stringValue
                // set into outbound
                v2rayOutbound.settingBlackhole = settingBlackhole
                break

            case .freedom:
                var settingFreedom = V2rayOutboundFreedom()
                settingFreedom.domainStrategy = jsonParams["settings"]["domainStrategy"].stringValue
                settingFreedom.userLevel = jsonParams["settings"]["userLevel"].intValue
                settingFreedom.redirect = jsonParams["settings"]["redirect"].stringValue
                // set into outbound
                v2rayOutbound.settingFreedom = settingFreedom
                break

            case .dns:
                var settingDns = V2rayOutboundDns()
                settingDns.network = jsonParams["settings"]["network"].stringValue
                settingDns.address = jsonParams["settings"]["address"].stringValue
                settingDns.port = jsonParams["settings"]["port"].intValue
                // set into outbound
                v2rayOutbound.settingDns = settingDns
                break

            case .http:
                var settingHttp = V2rayOutboundHttp()
                var servers: [V2rayOutboundHttpServer] = []

                jsonParams["settings"]["servers"].arrayValue.forEach {
                    val in
                    var server = V2rayOutboundHttpServer()
                    server.port = val["port"].intValue
                    server.address = val["address"].stringValue

                    var users: [V2rayOutboundHttpUser] = []
                    val["users"].arrayValue.forEach {
                        val in
                        var user = V2rayOutboundHttpUser()
                        user.user = val["user"].stringValue
                        user.pass = val["pass"].stringValue
                        // append
                        users.append(user)
                    }

                    server.users = users
                    // append
                    servers.append(server)
                }

                settingHttp.servers = servers

                // set into outbound
                v2rayOutbound.settingHttp = settingHttp

                break

            case .shadowsocks:
                var settingShadowsocks = V2rayOutboundShadowsocks()
                var servers: [V2rayOutboundShadowsockServer] = []
                // servers
                jsonParams["settings"]["servers"].arrayValue.forEach {
                    val in
                    var server = V2rayOutboundShadowsockServer()
                    server.port = val["port"].intValue
                    server.email = val["email"].stringValue
                    server.address = val["address"].stringValue

                    if V2rayOutboundShadowsockMethod.firstIndex(of: val["method"].stringValue) != nil {
                        server.method = val["method"].stringValue
                    } else {
                        server.method = V2rayOutboundShadowsockMethod[0]
                    }

                    server.password = val["password"].stringValue
                    server.ota = val["ota"].boolValue
                    server.level = val["level"].intValue
                    // append
                    servers.append(server)
                }
                settingShadowsocks.servers = servers
                // set into outbound
                v2rayOutbound.settingShadowsocks = settingShadowsocks
                break

            case .socks:
                var settingSocks = V2rayOutboundSocks()
                var servers: [V2rayOutboundSockServer] = []

                jsonParams["settings"]["servers"].arrayValue.forEach {
                    val in
                    var server = V2rayOutboundSockServer()
                    server.port = val["port"].intValue
                    server.address = val["address"].stringValue

                    var users: [V2rayOutboundSockUser] = []
                    val["users"].arrayValue.forEach {
                        val in
                        var user = V2rayOutboundSockUser()
                        user.user = val["user"].stringValue
                        user.pass = val["pass"].stringValue
                        user.level = val["level"].intValue
                        // append
                        users.append(user)
                    }

                    server.users = users
                    // append
                    servers.append(server)
                }

                settingSocks.servers = servers

                // set into outbound
                v2rayOutbound.settingSocks = settingSocks
                break

            case .vmess:
                var settingVMess = V2rayOutboundVMess()
                var vnext: [V2rayOutboundVMessItem] = []

                jsonParams["settings"]["vnext"].arrayValue.forEach {
                    val in
                    var item = V2rayOutboundVMessItem()

                    item.address = val["address"].stringValue
                    item.port = val["port"].intValue

                    var users: [V2rayOutboundVMessUser] = []
                    val["users"].arrayValue.forEach {
                        val in
                        var user = V2rayOutboundVMessUser()
                        user.id = val["id"].stringValue
                        user.alterId = val["alterId"].intValue
                        user.level = val["level"].intValue
                        if V2rayOutboundVMessSecurity.firstIndex(of: val["security"].stringValue) != nil {
                            user.security = val["security"].stringValue
                        }
                        users.append(user)
                    }
                    item.users = users
                    // append
                    vnext.append(item)
                }

                settingVMess.vnext = vnext

                // set into outbound
                v2rayOutbound.settingVMess = settingVMess

                // enable mux only vmess
                var mux = V2rayOutboundMux()
                mux.enabled = self.enableMux
                mux.concurrency = self.mux
                v2rayOutbound.mux = mux

                break

            case .vless:
                var settingVLess = V2rayOutboundVLess()
                var vnext: [V2rayOutboundVLessItem] = []

                jsonParams["settings"]["vnext"].arrayValue.forEach { val in
                    var item = V2rayOutboundVLessItem()

                    item.address = val["address"].stringValue
                    item.port = val["port"].intValue

                    var users: [V2rayOutboundVLessUser] = []
                    val["users"].arrayValue.forEach { val in
                        var user = V2rayOutboundVLessUser()
                        user.id = val["id"].stringValue
                        user.flow = val["flow"].stringValue
                        user.encryption = val["encryption"].stringValue
                        if user.encryption.isEmpty {
                            user.encryption = "none"
                        }
                        user.level = val["level"].intValue
                        users.append(user)
                    }
                    item.users = users
                    vnext.append(item)
                }

                settingVLess.vnext = vnext
                v2rayOutbound.settingVLess = settingVLess

                break

            case .trojan:
                var settingTrojan = V2rayOutboundTrojan()
                var servers: [V2rayOutboundTrojanServer] = []
                // servers
                jsonParams["settings"]["servers"].arrayValue.forEach { val in
                    var server = V2rayOutboundTrojanServer()
                    server.address = val["address"].stringValue
                    server.password = val["password"].stringValue
                    server.port = val["port"].intValue
                    server.level = val["level"].intValue
                    server.email = val["email"].stringValue

                    // append
                    servers.append(server)
                }
                settingTrojan.servers = servers
                // set into outbound
                v2rayOutbound.settingTrojan = settingTrojan

                break
            }
        }

        // stream settings
        if jsonParams["streamSettings"].dictionaryValue.count > 0 {
            v2rayOutbound.streamSettings = self.parseSteamSettings(streamJson: jsonParams["streamSettings"], preTxt: "outbound")
        }

        // set main server protocol
        let mainProtocol: [V2rayProtocolOutbound] = [V2rayProtocolOutbound.blackhole, V2rayProtocolOutbound.freedom, V2rayProtocolOutbound.http, V2rayProtocolOutbound.dns]
        if !self.foundServerProtocol && !mainProtocol.contains(v2rayOutbound.protocol) {
            self.serverProtocol = v2rayOutbound.protocol.rawValue
            self.foundServerProtocol = true

            if v2rayOutbound.protocol == V2rayProtocolOutbound.socks && v2rayOutbound.settingSocks != nil {
                self.serverSocks5 = v2rayOutbound.settingSocks!
            }

            if v2rayOutbound.protocol == V2rayProtocolOutbound.vmess && v2rayOutbound.settingVMess != nil && v2rayOutbound.settingVMess!.vnext.count > 0 {
                self.serverVmess = v2rayOutbound.settingVMess!.vnext[0]
            }

            if v2rayOutbound.protocol == V2rayProtocolOutbound.vless && v2rayOutbound.settingVLess != nil && v2rayOutbound.settingVLess!.vnext.count > 0 {
                self.serverVless = v2rayOutbound.settingVLess!.vnext[0]
            }

            if v2rayOutbound.protocol == V2rayProtocolOutbound.shadowsocks && v2rayOutbound.settingShadowsocks != nil && v2rayOutbound.settingShadowsocks!.servers.count > 0 {
                self.serverShadowsocks = v2rayOutbound.settingShadowsocks!.servers[0]
            }

            if v2rayOutbound.protocol == V2rayProtocolOutbound.trojan && v2rayOutbound.settingTrojan != nil && v2rayOutbound.settingTrojan!.servers.count > 0 {
                self.serverTrojan = v2rayOutbound.settingTrojan!.servers[0]
            }
        }

        return (v2rayOutbound)
    }

    // parse steamSettings
    func parseSteamSettings(streamJson: JSON, preTxt: String = "") -> V2rayStreamSettings {
        var stream = V2rayStreamSettings()

        if (V2rayStreamSettings.network(rawValue: streamJson["network"].stringValue) == nil) {
            self.errors += ["invalid " + preTxt + ".streamSettings.network"]
        } else {
            // set network
            stream.network = V2rayStreamSettings.network(rawValue: streamJson["network"].stringValue)!
            self.streamNetwork = stream.network.rawValue
        }

        if (V2rayStreamSettings.security(rawValue: streamJson["security"].stringValue) == nil) {
            self.streamSecurity = V2rayStreamSettings.security.none.rawValue
        } else {
            // set security
            stream.security = V2rayStreamSettings.security(rawValue: streamJson["security"].stringValue)!
            self.streamSecurity = stream.security.rawValue
        }

        if streamJson["sockopt"].dictionaryValue.count > 0 {
            var sockopt = V2rayStreamSettingSockopt()

            // tproxy
            if (V2rayStreamSettingSockopt.tproxy(rawValue: streamJson["sockopt"]["tproxy"].stringValue) != nil) {
                sockopt.tproxy = V2rayStreamSettingSockopt.tproxy(rawValue: streamJson["sockopt"]["tproxy"].stringValue)!
            }

            sockopt.tcpFastOpen = streamJson["sockopt"]["tcpFastOpen"].boolValue
            sockopt.mark = streamJson["sockopt"]["mark"].intValue

            stream.sockopt = sockopt
        }

        // steamSettings (same as global transport)
        let transport = self.parseTransport(streamJson: streamJson)
        stream.tlsSettings = transport.tlsSettings
        stream.xtlsSettings = transport.xtlsSettings
        stream.realitySettings = transport.realitySettings
        stream.tcpSettings = transport.tcpSettings
        stream.kcpSettings = transport.kcpSettings
        stream.wsSettings = transport.wsSettings
        stream.xhttpSettings = transport.xhttpSettings
        stream.httpSettings = transport.httpSettings
        stream.dsSettings = transport.dsSettings
        stream.quicSettings = transport.quicSettings
        stream.grpcSettings = transport.grpcSettings

        // for outbound stream
        if preTxt == "outbound" {

            if transport.xtlsSettings != nil {
                self.securityTls.serverName = transport.xtlsSettings!.serverName
                self.securityTls.allowInsecure = transport.xtlsSettings!.allowInsecure
            }
            
            if transport.tlsSettings != nil {
                self.securityTls.serverName = transport.tlsSettings!.serverName
                self.securityTls.fingerprint = transport.tlsSettings!.fingerprint
                self.securityTls.allowInsecure = transport.tlsSettings!.allowInsecure
                self.securityTls.alpn = transport.tlsSettings!.alpn
            }
            
            if transport.realitySettings != nil {
                self.securityReality.serverName = transport.realitySettings!.serverName
                self.securityReality.show = transport.realitySettings!.show
                self.securityReality.fingerprint = transport.realitySettings!.fingerprint
                self.securityReality.publicKey = transport.realitySettings!.publicKey
                self.securityReality.shortId = transport.realitySettings!.shortId
                self.securityReality.spiderX = transport.realitySettings!.spiderX
                if self.securityReality.fingerprint == "" {
                    self.securityReality.fingerprint = "chrome"
                }
            }
            
            if transport.tcpSettings != nil {
                self.streamTcp = transport.tcpSettings!
            }

            if transport.kcpSettings != nil {
                self.streamKcp = transport.kcpSettings!
                print("self.streamKcp",self.streamKcp)
            }

            if transport.wsSettings != nil {
                self.streamWs = transport.wsSettings!
            }

            if transport.xhttpSettings != nil {
                self.streamXhttp = transport.xhttpSettings!
            }
            
            if transport.httpSettings != nil {
                self.streamH2 = transport.httpSettings!
            }

            if transport.dsSettings != nil {
                self.streamDs = transport.dsSettings!
            }

            if transport.quicSettings != nil {
                self.streamQuic = transport.quicSettings!
            }

            if transport.grpcSettings != nil {
                self.streamGrpc = transport.grpcSettings!
            }
        }

        return (stream)
    }

    func parseTransport(streamJson: JSON) -> V2rayTransport {
        var stream = V2rayTransport()
        // tlsSettings
        if streamJson["tlsSettings"].dictionaryValue.count > 0 {
            let settings = streamJson["tlsSettings"]
            var tlsSettings = TlsSettings()
            tlsSettings.serverName = settings["serverName"].stringValue
            tlsSettings.alpn = settings["alpn"].arrayValue.map {
                $0.stringValue
            }
            tlsSettings.allowInsecure = settings["allowInsecure"].boolValue
            tlsSettings.allowInsecureCiphers = settings["allowInsecureCiphers"].boolValue
            // certificates
            if settings["certificates"].dictionaryValue.count > 0 {
                var certificates = TlsCertificates()
                let usage = TlsCertificates.usage(rawValue: settings["certificates"]["usage"].stringValue)
                if (usage != nil) {
                    certificates.usage = usage!
                }
                certificates.certificateFile = settings["certificates"]["certificateFile"].stringValue
                certificates.keyFile = settings["certificates"]["keyFile"].stringValue
                certificates.certificate = settings["certificates"]["certificate"].stringValue
                certificates.key = settings["certificates"]["key"].stringValue
                tlsSettings.certificates = certificates
            }
            stream.tlsSettings = tlsSettings
        }

        // xtlsSettings
        if streamJson["xtlsSettings"].dictionaryValue.count > 0 {
            let settings = streamJson["xtlsSettings"]
            var tlsSettings = TlsSettings()
            tlsSettings.serverName = settings["serverName"].stringValue
            tlsSettings.fingerprint = settings["fingerprint"].stringValue  // 必填，使用 uTLS 库模拟客户端 TLS 指纹
            tlsSettings.alpn = settings["alpn"].arrayValue.map {
                $0.stringValue
            }
            tlsSettings.allowInsecure = settings["allowInsecure"].boolValue
            tlsSettings.allowInsecureCiphers = settings["allowInsecureCiphers"].boolValue
            // certificates
            if settings["certificates"].dictionaryValue.count > 0 {
                var certificates = TlsCertificates()
                let usage = TlsCertificates.usage(rawValue: settings["certificates"]["usage"].stringValue)
                if (usage != nil) {
                    certificates.usage = usage!
                }
                certificates.certificateFile = settings["certificates"]["certificateFile"].stringValue
                certificates.keyFile = settings["certificates"]["keyFile"].stringValue
                certificates.certificate = settings["certificates"]["certificate"].stringValue
                certificates.key = settings["certificates"]["key"].stringValue
                tlsSettings.certificates = certificates
            }
            stream.xtlsSettings = tlsSettings
        }
        
        // reality
        if streamJson["realitySettings"].dictionaryValue.count > 0 {
            let settings = streamJson["realitySettings"]
            var realitySettings = RealitySettings()
            realitySettings.show = settings["show"].boolValue
            realitySettings.fingerprint = settings["fingerprint"].stringValue  // 必填，使用 uTLS 库模拟客户端 TLS 指纹
            realitySettings.serverName = settings["serverName"].stringValue
            realitySettings.publicKey = settings["publicKey"].stringValue
            realitySettings.shortId = settings["shortId"].stringValue
            realitySettings.spiderX = settings["spiderX"].stringValue
            
            if realitySettings.fingerprint == "" {
                realitySettings.fingerprint = "chrome"
            }
            
            stream.realitySettings = realitySettings
        }
        
        // tcpSettings
        if streamJson["tcpSettings"].dictionaryValue.count > 0 {
            var tcpSettings = TcpSettings()
            var tcpHeader = TcpSettingHeader()

            // type
            if streamJson["tcpSettings"]["header"]["type"].stringValue == "http" {
                tcpHeader.type = "http"
            } else {
                tcpHeader.type = "none"
            }

            // request
            if streamJson["tcpSettings"]["header"]["request"].dictionaryValue.count > 0 {
                let requestJson = streamJson["tcpSettings"]["header"]["request"]
                var tcpRequest = TcpSettingHeaderRequest()
                tcpRequest.version = requestJson["version"].stringValue
                tcpRequest.method = requestJson["method"].stringValue
                tcpRequest.path = requestJson["path"].arrayValue.map {
                    $0.stringValue
                }

                if requestJson["headers"].dictionaryValue.count > 0 {
                    var tcpRequestHeaders = TcpSettingHeaderRequestHeaders()
                    tcpRequestHeaders.host = requestJson["headers"]["Host"].arrayValue.map {
                        $0.stringValue
                    }
                    tcpRequestHeaders.userAgent = requestJson["headers"]["User-Agent"].arrayValue.map {
                        $0.stringValue
                    }
                    tcpRequestHeaders.acceptEncoding = requestJson["headers"]["Accept-Encoding"].arrayValue.map {
                        $0.stringValue
                    }
                    tcpRequestHeaders.connection = requestJson["headers"]["Connection"].arrayValue.map {
                        $0.stringValue
                    }
                    tcpRequestHeaders.pragma = requestJson["headers"]["Pragma"].stringValue
                    tcpRequest.headers = tcpRequestHeaders
                }
                tcpHeader.request = tcpRequest
            }

            // response
            if streamJson["tcpSettings"]["header"]["response"].dictionaryValue.count > 0 {
                let responseJson = streamJson["tcpSettings"]["header"]["response"]
                var tcpResponse = TcpSettingHeaderResponse()

                tcpResponse.version = responseJson["version"].stringValue
                tcpResponse.status = responseJson["status"].stringValue

                if responseJson["headers"].dictionaryValue.count > 0 {
                    var tcpResponseHeaders = TcpSettingHeaderResponseHeaders()
                    // contentType, transferEncoding, connection
                    tcpResponseHeaders.contentType = responseJson["headers"]["Content-Type"].arrayValue.map {
                        $0.stringValue
                    }
                    tcpResponseHeaders.transferEncoding = responseJson["headers"]["Transfer-Encoding"].arrayValue.map {
                        $0.stringValue
                    }
                    tcpResponseHeaders.connection = responseJson["headers"]["Connection"].arrayValue.map {
                        $0.stringValue
                    }
                    tcpResponseHeaders.pragma = responseJson["headers"]["Pragma"].stringValue
                    tcpResponse.headers = tcpResponseHeaders
                }
                tcpHeader.response = tcpResponse
            }

            tcpSettings.header = tcpHeader

            stream.tcpSettings = tcpSettings
        }

        // kcpSettings see: https://www.v2ray.com/chapter_02/transport/mkcp.html
        if streamJson["kcpSettings"].dictionaryValue.count > 0 {
            var kcpSettings = KcpSettings()
            kcpSettings.mtu = streamJson["kcpSettings"]["mtu"].intValue
            kcpSettings.tti = streamJson["kcpSettings"]["tti"].intValue
            kcpSettings.uplinkCapacity = streamJson["kcpSettings"]["uplinkCapacity"].intValue
            kcpSettings.downlinkCapacity = streamJson["kcpSettings"]["downlinkCapacity"].intValue
            kcpSettings.congestion = streamJson["kcpSettings"]["congestion"].boolValue
            kcpSettings.readBufferSize = streamJson["kcpSettings"]["readBufferSize"].intValue
            kcpSettings.writeBufferSize = streamJson["kcpSettings"]["writeBufferSize"].intValue
            kcpSettings.seed = streamJson["kcpSettings"]["seed"].stringValue
            // "none"
            if KcpSettingsHeaderType.firstIndex(of: streamJson["kcpSettings"]["header"]["type"].stringValue) != nil {
                kcpSettings.header.type = streamJson["kcpSettings"]["header"]["type"].stringValue
            }
            stream.kcpSettings = kcpSettings
        }

        // wsSettings see: https://www.v2ray.com/chapter_02/transport/websocket.html
        if streamJson["wsSettings"].dictionaryValue.count > 0 {
            var wsSettings = WsSettings()
            wsSettings.path = streamJson["wsSettings"]["path"].stringValue
            wsSettings.headers.host = streamJson["wsSettings"]["headers"]["host"].stringValue

            stream.wsSettings = wsSettings
        }

        // (HTTP/2)httpSettings see: https://www.v2ray.com/chapter_02/transport/websocket.html
        if streamJson["httpSettings"].dictionaryValue.count > 0 && streamJson["httpSettings"].dictionaryValue.count > 0 {
            var httpSettings = HttpSettings()
            httpSettings.host = streamJson["httpSettings"]["host"].arrayValue.map {
                $0.stringValue
            }
            httpSettings.path = streamJson["httpSettings"]["path"].stringValue

            stream.httpSettings = httpSettings
        }
        
        // xhttpSettings
        if streamJson["xhttpSettings"].dictionaryValue.count > 0 && streamJson["xhttpSettings"].dictionaryValue.count > 0 {
            var xhttpSettings = XhttpSettings()
            xhttpSettings.mode = streamJson["xhttpSettings"]["mode"].stringValue
            xhttpSettings.path = streamJson["xhttpSettings"]["path"].stringValue
            stream.xhttpSettings = xhttpSettings
        }

        // dsSettings
        if streamJson["dsSettings"].dictionaryValue.count > 0 && streamJson["dsSettings"].dictionaryValue.count > 0 {
            var dsSettings = DsSettings()
            dsSettings.path = streamJson["dsSettings"]["path"].stringValue
            stream.dsSettings = dsSettings
        }

        // quicSettings
        if streamJson["quicSettings"].dictionaryValue.count > 0 && streamJson["quicSettings"].dictionaryValue.count > 0 {
            var quicSettings = QuicSettings()
            quicSettings.key = streamJson["quicSettings"]["key"].stringValue
            // "none"
            if QuicSettingsHeaderType.firstIndex(of: streamJson["quicSettings"]["header"]["type"].stringValue) != nil {
                quicSettings.header.type = streamJson["quicSettings"]["header"]["type"].stringValue
            }
            if QuicSettingsSecurity.firstIndex(of: streamJson["quicSettings"]["security"].stringValue) != nil {
                quicSettings.security = streamJson["quicSettings"]["security"].stringValue
            }
            stream.quicSettings = quicSettings
        }

        // grpcSettings
        if streamJson["grpcSettings"].dictionaryValue.count > 0 && streamJson["grpcSettings"].dictionaryValue.count > 0 {
            var grpcSettings = GrpcSettings()
            grpcSettings.serviceName = streamJson["grpcSettings"]["serviceName"].stringValue
            grpcSettings.user_agent = streamJson["grpcSettings"]["user_agent"].stringValue
            grpcSettings.multiMode = streamJson["grpcSettings"]["multiMode"].boolValue
            stream.grpcSettings = grpcSettings
        }
        return stream
    }
}
