//
// Created by yanue on 2021/6/5.
// Copyright (c) 2021 yanue. All rights reserved.
//

import Foundation
import SwiftyJSON

struct VmessShare: Codable {
    var v: String = "2"
    var ps: String = ""
    var add: String = ""
    var port: String = ""
    var id: String = "" // UUID
    var aid: String = "" // alterId
    var net: String = "" // network type: (tcp\kcp\ws\h2\quic\ds\grpc)
    var type: String = "none" // 伪装类型(none\http\srtp\utp\wechat-video) *tcp or kcp or QUIC
    var host: String = "" // host: 1)http(tcp)->host中间逗号(,)隔开,2)ws->host,3)h2->host,4)QUIC->securty
    var path: String = "" // path: 1)ws->path,2)h2->path,3)QUIC->key/Kcp->seed,4)grpc->serviceName
    var tls: String = "tls"
    var security: String = "auto" // 加密方式(security),没有时值默认auto
    var scy: String = "auto" // 同security
    var alpn: String = "" // h2,http/1.1
    var sni: String = ""
    var fp: String = ""
}

class ShareUri {
    var error = ""
    var remark = ""
    var uri: String = ""
    var v2ray = V2rayConfig()

    func qrcode(item: V2rayItem) {
        v2ray.parseJson(jsonText: item.json)
        if !v2ray.isValid {
            self.error = v2ray.errors.count > 0 ? v2ray.errors[0] : ""
            return
        }

        self.remark = item.remark

        if v2ray.serverProtocol == V2rayProtocolOutbound.vmess.rawValue {
            self.genVmessUri()
            return
        }

        if v2ray.serverProtocol == V2rayProtocolOutbound.vless.rawValue {
            self.genVlessUri()
            return
        }

        if v2ray.serverProtocol == V2rayProtocolOutbound.shadowsocks.rawValue {
            self.genShadowsocksUri()
            return
        }

        if v2ray.serverProtocol == V2rayProtocolOutbound.trojan.rawValue {
            self.genTrojanUri()
            return
        }

        self.error = "not support"
    }

    /**s
    分享的链接（二维码）格式：vmess://(Base64编码的json格式服务器数据
    json数据如下
    {
    "v": "2",
    "ps": "备注别名",
    "add": "111.111.111.111",
    "port": "32000",
    "id": "1386f85e-657b-4d6e-9d56-78badb75e1fd",
    "aid": "100",
    "net": "tcp",
    "type": "none",
    "host": "www.bbb.com",
    "path": "/",
    "tls": "tls"
    }
    v:配置文件版本号,主要用来识别当前配置
    net ：传输协议（tcp\kcp\ws\h2)
    type:伪装类型（none\http\srtp\utp\wechat-video）
    host：伪装的域名
    1)http host中间逗号(,)隔开
    2)ws host
    3)h2 host
    path:path(ws/h2)
    tls：底层传输安全（tls)
    */
    private func genVmessUri() {
        var share = VmessShare()

        share.add = self.v2ray.serverVmess.address
        share.ps = self.remark
        share.port = String(self.v2ray.serverVmess.port)
        if self.v2ray.serverVmess.users.count > 0 {
            share.id = self.v2ray.serverVmess.users[0].id
            share.aid = String(self.v2ray.serverVmess.users[0].alterId)
            share.security = self.v2ray.serverVmess.users[0].security // security type
        }
        share.net = self.v2ray.streamNetwork

        if self.v2ray.streamNetwork == "tcp" {
            share.type = self.v2ray.streamTcp.header.type
            if self.v2ray.streamTcp.header.type == "http" {
                if let req = self.v2ray.streamTcp.header.request {
                    if req.path.count > 0 {
                        share.path = req.path[0]
                    }
                    if req.headers.host.count>0 {
                        share.host = req.headers.host[0]
                    }
                }
            }
        }
        
        if self.v2ray.streamNetwork == "kcp" {
            share.type = self.v2ray.streamKcp.header.type
            share.path = self.v2ray.streamKcp.seed
        }
        
        if self.v2ray.streamNetwork == "quic" {
            share.type = self.v2ray.streamQuic.header.type
            share.path = self.v2ray.streamQuic.key
        }
        
        if self.v2ray.streamNetwork == "domainsocket" {
            share.path = self.v2ray.streamDs.path
        }
        
        if self.v2ray.streamNetwork == "h2" {
            if self.v2ray.streamH2.host.count > 0 {
                share.host = self.v2ray.streamH2.host[0]
            }
            share.path = self.v2ray.streamH2.path
        }

        if self.v2ray.streamNetwork == "ws" {
            share.host = self.v2ray.streamWs.headers.host
            share.path = self.v2ray.streamWs.path
        }

        if self.v2ray.streamNetwork == "grpc" {
            share.path = self.v2ray.streamGrpc.serviceName
            if self.v2ray.streamGrpc.multiMode {
                share.type = "multi"
            }
        }

        share.tls = self.v2ray.streamSecurity
        share.sni = self.v2ray.securityTls.serverName
        share.fp = self.v2ray.securityTls.fingerprint
        share.net = self.v2ray.streamNetwork
        // todo headerType
        let encoder = JSONEncoder()
        if let data = try? encoder.encode(share) {
            let uri = String(data: data, encoding: .utf8)!
            self.uri = "vmess://" + uri.base64Encoded()!
        } else {
            self.error = "encode uri error"
        }
    }

    // Shadowsocks
    func genShadowsocksUri() {
        let ss = ShadowsockUri()
        ss.host = self.v2ray.serverShadowsocks.address
        ss.port = self.v2ray.serverShadowsocks.port
        ss.password = self.v2ray.serverShadowsocks.password
        ss.method = self.v2ray.serverShadowsocks.method
        ss.remark = self.remark
        self.uri = ss.encode()
        self.error = ss.error
    }

    // trojan
    func genTrojanUri() {
        let ss = TrojanUri()
        ss.host = self.v2ray.serverTrojan.address
        ss.port = self.v2ray.serverTrojan.port
        ss.password = self.v2ray.serverTrojan.password
        ss.remark = self.remark
        ss.security = self.v2ray.streamSecurity
        ss.network = self.v2ray.streamNetwork
        if self.v2ray.streamNetwork == "tcp" {
            ss.headerType = self.v2ray.streamTcp.header.type
            if self.v2ray.streamTcp.header.type == "http" {
                if let req = self.v2ray.streamTcp.header.request {
                    if req.path.count > 0 {
                        ss.path = req.path[0]
                    }
                    if req.headers.host.count>0 {
                        ss.host = req.headers.host[0]
                    }
                }
            }
        } else if self.v2ray.streamNetwork == "kcp" {
            ss.headerType = self.v2ray.streamKcp.header.type
            ss.netPath = self.v2ray.streamKcp.seed
        } else if self.v2ray.streamNetwork == "quic" {
            ss.headerType = self.v2ray.streamQuic.header.type
            ss.netPath = self.v2ray.streamQuic.key
        } else if self.v2ray.streamNetwork == "domainsocket" {
            ss.netPath = self.v2ray.streamDs.path
        } else if self.v2ray.streamNetwork == "h2" {
            if self.v2ray.streamH2.host.count > 0 {
                ss.netHost = self.v2ray.streamH2.host[0]
            }
            ss.netPath = self.v2ray.streamH2.path
        } else if self.v2ray.streamNetwork == "ws" {
            ss.netHost = self.v2ray.streamWs.headers.host
            ss.netPath = self.v2ray.streamWs.path
        } else if self.v2ray.streamNetwork == "grpc" {
            ss.netPath = self.v2ray.streamGrpc.serviceName
        }
        ss.fp = self.v2ray.securityTls.fingerprint
        ss.flow = self.v2ray.serverTrojan.flow
        ss.sni = self.v2ray.securityTls.serverName

        self.uri = ss.encode()
        self.error = ss.error
    }

    func genVlessUri() {
        let ss = VlessUri()
        ss.address = self.v2ray.serverVless.address
        ss.port = self.v2ray.serverVless.port

        if self.v2ray.serverVless.users.count > 0 {
            ss.id = self.v2ray.serverVless.users[0].id
            ss.level = self.v2ray.serverVless.users[0].level
            ss.flow = self.v2ray.serverVless.users[0].flow
            ss.encryption = self.v2ray.serverVless.users[0].encryption
        }
        ss.remark = self.remark

        ss.security = self.v2ray.streamSecurity
        if self.v2ray.streamSecurity == "reality" {
            ss.pbk = self.v2ray.securityReality.publicKey
            ss.fp = self.v2ray.securityReality.fingerprint
            ss.sid = self.v2ray.securityReality.shortId
            ss.sni = self.v2ray.securityReality.serverName
        } else {
            ss.sni = self.v2ray.securityTls.serverName
            ss.fp = self.v2ray.securityTls.fingerprint
        }

        ss.network = self.v2ray.streamNetwork

        if self.v2ray.streamNetwork == "tcp" {
            ss.headerType = self.v2ray.streamTcp.header.type
            if self.v2ray.streamTcp.header.type == "http" {
                if let req = self.v2ray.streamTcp.header.request {
                    if req.path.count > 0 {
                        ss.path = req.path[0]
                    }
                    if req.headers.host.count>0 {
                        ss.host = req.headers.host[0]
                    }
                }
            }
        }
        
        if self.v2ray.streamNetwork == "kcp" {
            ss.headerType = self.v2ray.streamKcp.header.type
            ss.kcpSeed = self.v2ray.streamKcp.seed
        }
        
        if self.v2ray.streamNetwork == "quic" {
            ss.headerType = self.v2ray.streamQuic.header.type
            ss.kcpSeed = self.v2ray.streamQuic.key
        }
        
        if self.v2ray.streamNetwork == "domainsocket" {
            ss.path = self.v2ray.streamDs.path
        }
        
        if self.v2ray.streamNetwork == "h2" {
            if self.v2ray.streamH2.host.count > 0 {
                ss.host = self.v2ray.streamH2.host[0]
            }
            ss.path = self.v2ray.streamH2.path
        }

        if self.v2ray.streamNetwork == "ws" {
            ss.host = self.v2ray.streamWs.headers.host
            ss.path = self.v2ray.streamWs.path
        }

        if self.v2ray.streamNetwork == "grpc" {
            ss.path = self.v2ray.streamGrpc.serviceName
            if self.v2ray.streamGrpc.multiMode {
                ss.grpcMode = "multi"
            }
        }

        self.uri = ss.encode()
        self.error = ss.error
    }

}
