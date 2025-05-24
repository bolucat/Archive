//
//  Config.swift
//  V2rayU
//
//  Created by yanue on 2018/10/9.
//  Copyright © 2018 yanue. All rights reserved.
//

import Cocoa

var v2rayConfig: V2rayConfig = V2rayConfig()

let configWindow = ConfigWindowController()

func OpenConfigWindow(){
    // show window
    configWindow.showWindow(nil)
    configWindow.window?.makeKeyAndOrderFront(configWindow.self)
    // bring to front
    NSApp.activate(ignoringOtherApps: true)
    showDock(state: true)
}

class ConfigWindowController: NSWindowController, NSWindowDelegate, NSTabViewDelegate {

    override var windowNibName: String? {
        return "ConfigWindow" // no extension .xib here
    }

    let tableViewDragType: String = "v2ray.item"

    @IBOutlet weak var tabView: NSTabView!
    @IBOutlet weak var okBtn: NSButtonCell!
    @IBOutlet weak var errTip: NSTextField!
    @IBOutlet weak var configText: NSTextView!
    @IBOutlet weak var serversTableView: NSTableView!
    @IBOutlet weak var addRemoveButton: NSSegmentedControl!
    @IBOutlet weak var jsonUrl: NSTextField!
    @IBOutlet weak var selectFileBtn: NSButton!
    @IBOutlet weak var importBtn: NSButton!

    @IBOutlet weak var sockPort: NSButton!
    @IBOutlet weak var httpPort: NSButton!
    @IBOutlet weak var dnsServers: NSButton!
    @IBOutlet weak var enableUdp: NSButton!
    @IBOutlet weak var enableMux: NSButton!
    @IBOutlet weak var muxConcurrent: NSButton!

    @IBOutlet weak var switchProtocol: NSPopUpButton!

    @IBOutlet weak var serverView: NSView!
    @IBOutlet weak var VmessView: NSView!
    @IBOutlet weak var VlessView: NSView!
    @IBOutlet weak var ShadowsocksView: NSView!
    @IBOutlet weak var SocksView: NSView!
    @IBOutlet weak var TrojanView: NSView!

    // vmess
    @IBOutlet weak var vmessAddr: NSTextField!
    @IBOutlet weak var vmessPort: NSTextField!
    @IBOutlet weak var vmessAlterId: NSTextField!
    @IBOutlet weak var vmessLevel: NSTextField!
    @IBOutlet weak var vmessUserId: NSTextField!
    @IBOutlet weak var vmessSecurity: NSPopUpButton!

    // vless
    @IBOutlet weak var vlessAddr: NSTextField!
    @IBOutlet weak var vlessPort: NSTextField!
    @IBOutlet weak var vlessUserId: NSTextField!
    @IBOutlet weak var vlessLevel: NSTextField!
    @IBOutlet weak var vlessFlow: NSTextField!

    // shadowsocks
    @IBOutlet weak var shadowsockAddr: NSTextField!
    @IBOutlet weak var shadowsockPort: NSTextField!
    @IBOutlet weak var shadowsockPass: NSTextField!
    @IBOutlet weak var shadowsockMethod: NSPopUpButton!

    // socks5
    @IBOutlet weak var socks5Addr: NSTextField!
    @IBOutlet weak var socks5Port: NSTextField!
    @IBOutlet weak var socks5User: NSTextField!
    @IBOutlet weak var socks5Pass: NSTextField!

    // for trojan
    @IBOutlet weak var trojanAddr: NSTextField!
    @IBOutlet weak var trojanPort: NSTextField!
    @IBOutlet weak var trojanPass: NSTextField!
    @IBOutlet weak var trojanAlpn: NSTextField!

    @IBOutlet weak var networkView: NSView!

    @IBOutlet weak var tcpView: NSView!
    @IBOutlet weak var kcpView: NSView!
    @IBOutlet weak var dsView: NSView!
    @IBOutlet weak var wsView: NSView!
    @IBOutlet weak var xhttpView: NSView!
    @IBOutlet weak var h2View: NSView!
    @IBOutlet weak var quicView: NSView!
    @IBOutlet weak var grpcView: NSView!
    @IBOutlet weak var tlsView: NSView!
    @IBOutlet weak var realityView: NSView!

    @IBOutlet weak var switchNetwork: NSPopUpButton!
    @IBOutlet weak var switchSecurity: NSPopUpButton!

    // kcp setting
    @IBOutlet weak var kcpMtu: NSTextField!
    @IBOutlet weak var kcpTti: NSTextField!
    @IBOutlet weak var kcpUplinkCapacity: NSTextField!
    @IBOutlet weak var kcpDownlinkCapacity: NSTextField!
    @IBOutlet weak var kcpSeed: NSTextField!
    @IBOutlet weak var kcpHeader: NSPopUpButton!
    @IBOutlet weak var kcpCongestion: NSButton!

    @IBOutlet weak var tcpHeaderType: NSPopUpButton!
    @IBOutlet weak var tcpHost: NSTextField!
    @IBOutlet weak var tcpPath: NSTextField!

    @IBOutlet weak var wsHost: NSTextField!
    @IBOutlet weak var wsPath: NSTextField!
    
    @IBOutlet weak var xhttpMode: NSTextField!
    @IBOutlet weak var xhttpPath: NSTextField!

    @IBOutlet weak var h2Host: NSTextField!
    @IBOutlet weak var h2Path: NSTextField!

    @IBOutlet weak var dsPath: NSTextField!

    @IBOutlet weak var quicKey: NSTextField!
    @IBOutlet weak var quicSecurity: NSPopUpButton!
    @IBOutlet weak var quicHeaderType: NSPopUpButton!

    @IBOutlet weak var grpcServiceName: NSTextField!
    @IBOutlet weak var grpcUseragent: NSTextField!
    @IBOutlet weak var grpcMulti: NSButton!

    @IBOutlet weak var streamSecurity: NSPopUpButton!
    @IBOutlet weak var streamTlsAllowInsecure: NSButton!
    @IBOutlet weak var streamTlsServerName: NSTextField!
    @IBOutlet weak var streamTlsAlpn: NSTextField!
    @IBOutlet weak var streamRealityServerName: NSTextField!
    @IBOutlet weak var streamRealityPublicKey: NSTextField!
    @IBOutlet weak var streamRealityShortId: NSTextField!
    @IBOutlet weak var streamRealitySpiderX: NSTextField!
    
    override func awakeFromNib() {
        // set table drag style
        serversTableView.registerForDraggedTypes([NSPasteboard.PasteboardType(rawValue: tableViewDragType)])
        serversTableView.allowsMultipleSelection = true

        if V2rayServer.count() == 0 {
            // add default
            V2rayServer.add(remark: "default", json: "", isValid: false)
        }
        self.shadowsockMethod.removeAllItems()
        self.shadowsockMethod.addItems(withTitles: V2rayOutboundShadowsockMethod)

        self.configText.isAutomaticQuoteSubstitutionEnabled = false
    }

    override func windowDidLoad() {
        super.windowDidLoad()
        
        V2rayServer.loadConfig()
        // table view
        self.serversTableView.delegate = self
        self.serversTableView.dataSource = self
        self.serversTableView.reloadData()
        // tab view
        self.tabView.delegate = self
    }

    @IBAction func addRemoveServer(_ sender: NSSegmentedCell) {
        // 0 add,1 remove
        let seg = addRemoveButton.indexOfSelectedItem
        DispatchQueue.global().async {
            switch seg {
                // add server config
            case 0:
                // add
                V2rayServer.add()
                
                DispatchQueue.main.async {
                    V2rayServer.loadConfig()
                    // reload data
                    self.serversTableView.reloadData()
                    // selected current row
                    self.serversTableView.selectRowIndexes(NSIndexSet(index: V2rayServer.count() - 1) as IndexSet, byExtendingSelection: false)
                    
                    menuController.showServers()
                }
                break
                
                // delete server config
            case 1:
                DispatchQueue.main.async {
                    // get seleted index
                    let idx = self.serversTableView.selectedRow
                    // remove
                    V2rayServer.remove(idx: idx)
                    
                    // reload
                    V2rayServer.loadConfig()
                    menuController.showServers()
                    
                    // selected prev row
                    let cnt: Int = V2rayServer.count()
                    var rowIndex: Int = idx - 1
                    if idx > 0 && idx < cnt {
                        rowIndex = idx
                    }
                    
                    // reload
                    self.serversTableView.reloadData()
                    // fix
                    if cnt > 1 {
                        // selected row
                        self.serversTableView.selectRowIndexes(NSIndexSet(index: rowIndex) as IndexSet, byExtendingSelection: false)
                    }
                
                    if rowIndex >= 0 {
                        self.loadJsonData(rowIndex: rowIndex)
                    } else {
                        self.serversTableView.becomeFirstResponder()
                    }
                }
                
                // refresh menu
                menuController.showServers()
                break
                
                // unknown action
            default:
                return
            }
        }
    }

    // switch tab view
    func tabView(_ tabView: NSTabView, didSelect tabViewItem: NSTabViewItem?) {
        guard let item = tabViewItem else {
            print("not found tab view")
            return
        }

        let tab = item.identifier! as! String
        if tab == "Manual" {
            self.switchToManualView()
        } else {
            self.switchToImportView()
        }
    }

    // switch to manual
    func switchToManualView() {
        v2rayConfig = V2rayConfig()

        defer {
            if self.configText.string.count > 0 {
                self.bindDataToView()
            }
        }

        // re parse json
        v2rayConfig.parseJson(jsonText: self.configText.string)
        if v2rayConfig.errors.count > 0 {
            self.errTip.stringValue = v2rayConfig.errors[0]
            return
        }

        self.saveConfig()
    }

    // switch to import
    func switchToImportView() {
        // reset error
        self.errTip.stringValue = ""
        self.exportData()

        v2rayConfig.checkManualValid()

        if v2rayConfig.isValid {
            let jsonText = v2rayConfig.combineManual()
            self.configText.string = jsonText
            self.saveConfig()
        } else {
            self.errTip.stringValue = v2rayConfig.error
        }
    }

    // export data to V2rayConfig
    func exportData() {
        // ========================== server start =======================
        if self.switchProtocol.indexOfSelectedItem >= 0 {
            v2rayConfig.serverProtocol = self.switchProtocol.titleOfSelectedItem!
        }

        // vmess
        v2rayConfig.serverVmess.address = self.vmessAddr.stringValue
        v2rayConfig.serverVmess.port = Int(self.vmessPort.intValue)
        var user = V2rayOutboundVMessUser()
        user.alterId = Int(self.vmessAlterId.intValue)
        user.level = Int(self.vmessLevel.intValue)
        user.id = self.vmessUserId.stringValue
        if self.vmessSecurity.indexOfSelectedItem >= 0 {
            user.security = self.vmessSecurity.titleOfSelectedItem!
        }
        if v2rayConfig.serverVmess.users.count == 0 {
            v2rayConfig.serverVmess.users = [user]
        } else {
            v2rayConfig.serverVmess.users[0] = user
        }

        // vless
        v2rayConfig.serverVless.address = self.vlessAddr.stringValue
        v2rayConfig.serverVless.port = Int(self.vlessPort.intValue)
        var vless_user = V2rayOutboundVLessUser()
        vless_user.id = self.vlessUserId.stringValue
        vless_user.level = Int(self.vlessLevel.intValue)
        vless_user.flow = self.vlessFlow.stringValue
        if v2rayConfig.serverVless.users.count == 0 {
            v2rayConfig.serverVless.users = [vless_user]
        } else {
            v2rayConfig.serverVless.users[0] = vless_user
        }

        // shadowsocks
        v2rayConfig.serverShadowsocks.address = self.shadowsockAddr.stringValue
        v2rayConfig.serverShadowsocks.port = Int(self.shadowsockPort.intValue)
        v2rayConfig.serverShadowsocks.password = self.shadowsockPass.stringValue
        if self.vmessSecurity.indexOfSelectedItem >= 0 {
            v2rayConfig.serverShadowsocks.method = self.shadowsockMethod.titleOfSelectedItem ?? "aes-256-cfb"
        }

        // trojan
        v2rayConfig.serverTrojan.address = self.trojanAddr.stringValue
        v2rayConfig.serverTrojan.port = Int(self.trojanPort.intValue)
        v2rayConfig.serverTrojan.password = self.trojanPass.stringValue

        // socks5
        if v2rayConfig.serverSocks5.servers.count == 0 {
            v2rayConfig.serverSocks5.servers = [V2rayOutboundSockServer()]
        }
        v2rayConfig.serverSocks5.servers[0].address = self.socks5Addr.stringValue
        v2rayConfig.serverSocks5.servers[0].port = Int(self.socks5Port.intValue)

        var sockUser = V2rayOutboundSockUser()
        sockUser.user = self.socks5User.stringValue
        sockUser.pass = self.socks5Pass.stringValue
        if self.socks5User.stringValue.count > 0 || self.socks5Pass.stringValue.count > 0 {
            v2rayConfig.serverSocks5.servers[0].users = [sockUser]
        } else {
            v2rayConfig.serverSocks5.servers[0].users = nil
        }
        // ========================== server end =======================

        // ========================== stream start =======================
        if self.switchNetwork.indexOfSelectedItem >= 0 {
            v2rayConfig.streamNetwork = self.switchNetwork.titleOfSelectedItem!
        }
        // security
        if self.streamSecurity.indexOfSelectedItem >= 0 {
            v2rayConfig.streamSecurity = self.streamSecurity.titleOfSelectedItem!
        }
        // tls
        v2rayConfig.securityTls.allowInsecure = self.streamTlsAllowInsecure.state.rawValue > 0
        v2rayConfig.securityTls.serverName = self.streamTlsServerName.stringValue
        let streamTlsAlpn = self.streamTlsAlpn.stringValue
        if streamTlsAlpn.count != 0 {
            v2rayConfig.securityTls.alpn = [streamTlsAlpn]
        } else {
            v2rayConfig.securityTls.alpn = []
        }
        // reality
        v2rayConfig.securityReality.serverName = self.streamRealityServerName.stringValue
        v2rayConfig.securityReality.publicKey = self.streamRealityPublicKey.stringValue
        v2rayConfig.securityReality.shortId = self.streamRealityShortId.stringValue
        v2rayConfig.securityReality.spiderX = self.streamRealitySpiderX.stringValue
        
        // tcp
        if self.tcpHeaderType.indexOfSelectedItem >= 0 {
            v2rayConfig.streamTcp.header.type = self.tcpHeaderType.titleOfSelectedItem!
        }
        if v2rayConfig.streamTcp.header.type == "http" {
            var tcpRequest = TcpSettingHeaderRequest()
            tcpRequest.path = [self.tcpPath.stringValue]
            tcpRequest.headers.host = [self.tcpHost.stringValue]
            v2rayConfig.streamTcp.header.request = tcpRequest
        }

        // kcp
        if self.kcpHeader.indexOfSelectedItem >= 0 {
            v2rayConfig.streamKcp.header.type = self.kcpHeader.titleOfSelectedItem!
        }
        v2rayConfig.streamKcp.mtu = Int(self.kcpMtu.intValue)
        v2rayConfig.streamKcp.tti = Int(self.kcpTti.intValue)
        v2rayConfig.streamKcp.uplinkCapacity = Int(self.kcpUplinkCapacity.intValue)
        v2rayConfig.streamKcp.downlinkCapacity = Int(self.kcpDownlinkCapacity.intValue)
        v2rayConfig.streamKcp.seed = self.kcpSeed.stringValue
        v2rayConfig.streamKcp.congestion = self.kcpCongestion.state.rawValue > 0

        // h2
        let h2HostString = self.h2Host.stringValue
        if h2HostString.count != 0 {
            v2rayConfig.streamH2.host = [h2HostString]
        } else {
            v2rayConfig.streamH2.host = []
        }
        v2rayConfig.streamH2.path = self.h2Path.stringValue

        // xhttp
        v2rayConfig.streamXhttp.mode = self.xhttpMode.stringValue
        v2rayConfig.streamXhttp.path = self.xhttpPath.stringValue
        
        // ws
        v2rayConfig.streamWs.path = self.wsPath.stringValue
        v2rayConfig.streamWs.headers.host = self.wsHost.stringValue

        // domainsocket
        v2rayConfig.streamDs.path = self.dsPath.stringValue

        // quic
        v2rayConfig.streamQuic.key = self.quicKey.stringValue
        if self.quicHeaderType.indexOfSelectedItem >= 0 {
            v2rayConfig.streamQuic.header.type = self.quicHeaderType.titleOfSelectedItem!
        }
        if self.quicSecurity.indexOfSelectedItem >= 0 {
            v2rayConfig.streamQuic.security = self.quicSecurity.titleOfSelectedItem!
        }

        // grpc
        v2rayConfig.streamGrpc.serviceName = self.grpcServiceName.stringValue
        v2rayConfig.streamGrpc.user_agent = self.grpcUseragent.stringValue
        v2rayConfig.streamGrpc.multiMode = self.grpcMulti.state.rawValue > 0

        // ========================== stream end =======================
    }

    func bindDataToView() {
        // ========================== base start =======================
        // base
        self.httpPort.title = v2rayConfig.httpPort
        self.sockPort.title = v2rayConfig.socksPort
        self.enableUdp.intValue = v2rayConfig.enableUdp ? 1 : 0
        self.enableMux.intValue = v2rayConfig.enableMux ? 1 : 0
        self.muxConcurrent.intValue = Int32(v2rayConfig.mux)
        // ========================== base end =======================

        // ========================== server start =======================
        self.switchProtocol.selectItem(withTitle: v2rayConfig.serverProtocol)
        self.switchOutboundView(protocolTitle: v2rayConfig.serverProtocol)

        // vmess
        self.vmessAddr.stringValue = v2rayConfig.serverVmess.address
        self.vmessPort.intValue = Int32(v2rayConfig.serverVmess.port)
        if v2rayConfig.serverVmess.users.count > 0 {
            let user = v2rayConfig.serverVmess.users[0]
            self.vmessAlterId.intValue = Int32(user.alterId)
            self.vmessLevel.intValue = Int32(user.level)
            self.vmessUserId.stringValue = user.id
            self.vmessSecurity.selectItem(withTitle: user.security)
        }

        // vless
        self.vlessAddr.stringValue = v2rayConfig.serverVless.address
        self.vlessPort.intValue = Int32(v2rayConfig.serverVless.port)
        if v2rayConfig.serverVless.users.count > 0 {
            let user = v2rayConfig.serverVless.users[0]
            self.vlessLevel.intValue = Int32(user.level)
            self.vlessFlow.stringValue = user.flow
            self.vlessUserId.stringValue = user.id
        }

        // shadowsocks
        self.shadowsockAddr.stringValue = v2rayConfig.serverShadowsocks.address
        if v2rayConfig.serverShadowsocks.port > 0 {
            self.shadowsockPort.stringValue = String(v2rayConfig.serverShadowsocks.port)
        }
        self.shadowsockPass.stringValue = v2rayConfig.serverShadowsocks.password
        self.shadowsockMethod.selectItem(withTitle: v2rayConfig.serverShadowsocks.method)

        // socks5
        if v2rayConfig.serverSocks5.servers.count > 0 {
            self.socks5Addr.stringValue = v2rayConfig.serverSocks5.servers[0].address
            self.socks5Port.stringValue = String(v2rayConfig.serverSocks5.servers[0].port)
            let users = v2rayConfig.serverSocks5.servers[0].users
            if users != nil && users!.count > 0 {
                let user = users![0]
                self.socks5User.stringValue = user.user
                self.socks5Pass.stringValue = user.pass
            }
        }

        // trojan
        self.trojanAddr.stringValue = v2rayConfig.serverTrojan.address
        self.trojanPass.stringValue = v2rayConfig.serverTrojan.password
        if v2rayConfig.serverTrojan.port > 0 {
            self.trojanPort.stringValue = String(v2rayConfig.serverTrojan.port)
        }


        // ========================== server end =======================

        // ========================== stream start =======================
        self.switchNetwork.selectItem(withTitle: v2rayConfig.streamNetwork)
        self.switchSteamView(network: v2rayConfig.streamNetwork)

        self.switchSecurityView(securityTitle: v2rayConfig.streamSecurity)
        self.streamSecurity.selectItem(withTitle: v2rayConfig.streamSecurity)
        self.streamTlsAllowInsecure.intValue = v2rayConfig.securityTls.allowInsecure ? 1 : 0
        self.streamTlsServerName.stringValue = v2rayConfig.securityTls.serverName
        self.streamTlsAlpn.stringValue = v2rayConfig.securityTls.alpn.count > 0 ? v2rayConfig.securityTls.alpn[0] : ""
        
        // reality
        self.streamRealityServerName.stringValue = v2rayConfig.securityReality.serverName
        self.streamRealityPublicKey.stringValue = v2rayConfig.securityReality.publicKey
        self.streamRealityShortId.stringValue = v2rayConfig.securityReality.shortId
        self.streamRealitySpiderX.stringValue = v2rayConfig.securityReality.spiderX
        
        // tcp
        self.tcpHeaderType.selectItem(withTitle: v2rayConfig.streamTcp.header.type)
        if let req = v2rayConfig.streamTcp.header.request {
            if req.path.count>0 {
                self.tcpPath.stringValue = req.path[0]
            }
            if req.headers.host.count>0{
                self.tcpHost.stringValue = req.headers.host[0]
            }
        } else {
            self.tcpPath.stringValue = ""
            self.tcpHost.stringValue = ""
        }

        // kcp
        self.kcpHeader.selectItem(withTitle: v2rayConfig.streamKcp.header.type)
        self.kcpMtu.intValue = Int32(v2rayConfig.streamKcp.mtu)
        self.kcpTti.intValue = Int32(v2rayConfig.streamKcp.tti)
        self.kcpUplinkCapacity.intValue = Int32(v2rayConfig.streamKcp.uplinkCapacity)
        self.kcpDownlinkCapacity.intValue = Int32(v2rayConfig.streamKcp.downlinkCapacity)
        self.kcpSeed.stringValue = v2rayConfig.streamKcp.seed
        self.kcpCongestion.intValue = v2rayConfig.streamKcp.congestion ? 1 : 0

        // h2
        self.h2Host.stringValue = v2rayConfig.streamH2.host.count > 0 ? v2rayConfig.streamH2.host[0] : ""
        self.h2Path.stringValue = v2rayConfig.streamH2.path

        // xhttp
        self.xhttpMode.stringValue = v2rayConfig.streamXhttp.mode
        self.xhttpPath.stringValue = v2rayConfig.streamXhttp.path
        
        // ws
        self.wsPath.stringValue = v2rayConfig.streamWs.path
        self.wsHost.stringValue = v2rayConfig.streamWs.headers.host

        // domainsocket
        self.dsPath.stringValue = v2rayConfig.streamDs.path

        // quic
        self.quicKey.stringValue = v2rayConfig.streamQuic.key
        self.quicSecurity.selectItem(withTitle: v2rayConfig.streamQuic.security)
        self.quicHeaderType.selectItem(withTitle: v2rayConfig.streamQuic.header.type)

        // grpc
        self.grpcServiceName.stringValue = v2rayConfig.streamGrpc.serviceName
        self.grpcUseragent.stringValue = v2rayConfig.streamGrpc.user_agent ?? ""
        self.grpcMulti.intValue = v2rayConfig.streamGrpc.multiMode ? 1 : 0

        // ========================== stream end =======================
    }

    func loadJsonData(rowIndex: Int) {
        defer {
            self.bindDataToView()
            // replace current
            self.switchToImportView()
        }

        // reset
        v2rayConfig = V2rayConfig()
        if rowIndex < 0 {
            return
        }

        let item = V2rayServer.loadV2rayItem(idx: rowIndex)
        self.configText.string = item?.json ?? ""
        v2rayConfig.isValid = item?.isValid ?? false
        self.jsonUrl.stringValue = item?.url ?? ""
        
        v2rayConfig.parseJson(jsonText: self.configText.string)
        if v2rayConfig.errors.count > 0 {
            self.errTip.stringValue = v2rayConfig.errors[0]
            return
        }
    }

    func saveConfig() {
        let text = self.configText.string

        v2rayConfig.parseJson(jsonText: self.configText.string)
        if v2rayConfig.errors.count > 0 {
            DispatchQueue.main.async {
                self.errTip.stringValue = v2rayConfig.errors[0]
            }
        }

        // save
        let errMsg = V2rayServer.save(idx: self.serversTableView.selectedRow, isValid: v2rayConfig.isValid, jsonData: text)
        if errMsg.count == 0 {
            if self.errTip.stringValue == "" {
                self.errTip.stringValue = "save success"
                DispatchQueue.main.asyncAfter(deadline: .now() + 2.0) {
                    // your code here
                    self.errTip.stringValue = ""
                }
            }
            self.refreshServerList(ok: errMsg.count == 0)
        } else {
            DispatchQueue.main.async {
                self.errTip.stringValue = errMsg
            }
        }
    }

    func refreshServerList(ok: Bool = true) {
        // refresh menu
        menuController.showServers()
        // if server is current
        if let curName = UserDefaults.get(forKey: .v2rayCurrentServerName) {
            let v2rayItemList = V2rayServer.all()
            if v2rayItemList.count > self.serversTableView.selectedRow && curName == v2rayItemList[self.serversTableView.selectedRow].name {
                if ok {
                    V2rayLaunch.startV2rayCore()
                } else {
                    V2rayLaunch.stopV2rayCore()
                }
            }
        }
    }

    @IBAction func ok(_ sender: NSButton) {
        // set always on
        self.okBtn.state = .on
        // in Manual tab view
        if "Manual" == self.tabView.selectedTabViewItem?.identifier as! String {
            self.switchToImportView()
        } else {
            self.saveConfig()
        }
    }

    @IBAction func importConfig(_ sender: NSButton) {
        self.configText.string = ""
        if jsonUrl.stringValue.trimmingCharacters(in: .whitespaces) == "" {
            self.errTip.stringValue = "error: invaid url"
            return
        }

        self.importJson()
    }

    func saveImport(importUri: ImportUri) {
        if importUri.isValid {
            self.configText.string = importUri.json
            if importUri.remark.count > 0 {
                V2rayServer.edit(rowIndex: self.serversTableView.selectedRow, remark: importUri.remark)
            }

            // refresh
            self.refreshServerList(ok: true)
        } else {
            self.errTip.stringValue = importUri.error
        }
    }

    func importJson() {
        let text = self.configText.string
        let uri = jsonUrl.stringValue.trimmingCharacters(in: .whitespaces)
        // edit item remark
        V2rayServer.edit(rowIndex: self.serversTableView.selectedRow, url: uri)

        if let importUri = ImportUri.importUri(uri: uri, checkExist: false) {
            self.saveImport(importUri: importUri)
        }
    }

    @IBAction func goTcpHelp(_ sender: NSButtonCell) {
        guard let url = URL(string: "https://www.v2ray.com/chapter_02/transport/tcp.html") else {
            return
        }
        DispatchQueue.main.async{
            NSWorkspace.shared.open(url)
        }
    }

    @IBAction func goDsHelp(_ sender: Any) {
        guard let url = URL(string: "https://www.v2ray.com/chapter_02/transport/domainsocket.html") else {
            return
        }
        DispatchQueue.main.async{
            NSWorkspace.shared.open(url)
        }
    }

    @IBAction func goQuicHelp(_ sender: Any) {
        guard let url = URL(string: "https://www.v2ray.com/chapter_02/transport/quic.html") else {
            return
        }
        DispatchQueue.main.async{
            NSWorkspace.shared.open(url)
        }
    }

    @IBAction func goProtocolHelp(_ sender: NSButton) {
        guard let url = URL(string: "https://www.v2ray.com/chapter_02/protocols/vmess.html") else {
            return
        }
        DispatchQueue.main.async{
            NSWorkspace.shared.open(url)
        }
    }

    @IBAction func goVersionHelp(_ sender: Any) {
        guard let url = URL(string: "https://www.v2ray.com/chapter_02/01_overview.html") else {
            return
        }
        DispatchQueue.main.async{
            NSWorkspace.shared.open(url)
        }
    }

    @IBAction func goStreamHelp(_ sender: Any) {
        guard let url = URL(string: "https://www.v2ray.com/chapter_02/05_transport.html") else {
            return
        }
        DispatchQueue.main.async{
            NSWorkspace.shared.open(url)
        }
    }

    func switchSteamView(network: String) {
        DispatchQueue.main.async{
            self.networkView.subviews.forEach {
                $0.isHidden = true
            }
            
            switch network {
            case "tcp":
                self.tcpView.isHidden = false
                break;
            case "kcp":
                self.kcpView.isHidden = false
                break;
            case "domainsocket":
                self.dsView.isHidden = false
                break;
            case "ws":
                self.wsView.isHidden = false
                break;
            case "xhttp":
                self.xhttpView.isHidden = false
                break;
            case "h2":
                self.h2View.isHidden = false
                break;
            case "quic":
                self.quicView.isHidden = false
                break;
            case "grpc":
                self.grpcView.isHidden = false
                break;
            default: // vmess
                self.tcpView.isHidden = false
                break
            }
        }
    }

    func switchOutboundView(protocolTitle: String) {
        DispatchQueue.main.async{
            self.serverView.subviews.forEach {
                $0.isHidden = true
            }
            
            switch protocolTitle {
            case "vmess":
                self.VmessView.isHidden = false
                break
            case "vless":
                self.VlessView.isHidden = false
                break
            case "shadowsocks":
                self.ShadowsocksView.isHidden = false
                break
            case "socks":
                self.SocksView.isHidden = false
                break
            case "trojan":
                self.TrojanView.isHidden = false
                break
            default: // vmess
                self.VmessView.isHidden = true
                break
            }
        }
    }
    
    func switchSecurityView(securityTitle: String) {
        DispatchQueue.main.async{
            print("switchSecurityView",securityTitle)
            self.tlsView.isHidden = true
            self.realityView.isHidden = true
            if securityTitle == "reality" {
                self.realityView.isHidden = false
            } else {
                self.tlsView.isHidden = false
            }
        }
    }

    func reloadData(){
        DispatchQueue.main.async{
            if self.serversTableView != nil {
                V2rayServer.loadConfig()
                self.serversTableView.reloadData()
            }
        }
    }
    
    @IBAction func switchSteamSecurity(_ sender: NSPopUpButtonCell) {
        if let item = switchSecurity.selectedItem {
            self.switchSecurityView(securityTitle: item.title)
        }
    }
    
    @IBAction func switchSteamNetwork(_ sender: NSPopUpButtonCell) {
        if let item = switchNetwork.selectedItem {
            self.switchSteamView(network: item.title)
        }
    }

    @IBAction func switchOutboundProtocol(_ sender: NSPopUpButtonCell) {
        if let item = switchProtocol.selectedItem {
            self.switchOutboundView(protocolTitle: item.title)
        }
    }

    @IBAction func switchUri(_ sender: NSPopUpButton) {
        guard let item = sender.selectedItem else {
            return
        }
        DispatchQueue.main.async{
            // url
            if item.title == "url" {
                self.jsonUrl.stringValue = ""
                self.selectFileBtn.isHidden = true
                self.importBtn.isHidden = false
                self.jsonUrl.isEditable = true
            } else {
                // local file
                self.jsonUrl.stringValue = ""
                self.selectFileBtn.isHidden = false
                self.importBtn.isHidden = true
                self.jsonUrl.isEditable = false
            }
        }
    }

    @IBAction func browseFile(_ sender: NSButton) {
        DispatchQueue.main.async{
            self.jsonUrl.stringValue = ""
            let dialog = NSOpenPanel()
            
            dialog.title = "Choose a .json file";
            dialog.showsResizeIndicator = true;
            dialog.showsHiddenFiles = false;
            dialog.canChooseDirectories = true;
            dialog.canCreateDirectories = true;
            dialog.allowsMultipleSelection = false;
            dialog.allowedFileTypes = ["json", "txt"];
            
            if (dialog.runModal() == NSApplication.ModalResponse.OK) {
                let result = dialog.url // Pathname of the file
                
                if (result != nil) {
                    self.jsonUrl.stringValue = result?.absoluteString ?? ""
                    self.importJson()
                }
            } else {
                // User clicked on "Cancel"
                return
            }
        }
    }

    @IBAction func openLogs(_ sender: NSButton) {
        OpenLogs()
    }

    @IBAction func clearLogs(_ sender: NSButton) {
        ClearLogs()
    }

    @IBAction func cancel(_ sender: NSButton) {
        // hide dock icon and close all opened windows
        showDock(state: false)
    }

    @IBAction func goAdvanceSetting(_ sender: Any) {
        DispatchQueue.main.async {
            preferencesWindowController.show(preferencePane: .advanceTab)
            showDock(state: true)
        }
    }

    @IBAction func goSubscribeSetting(_ sender: Any) {
        DispatchQueue.main.async {
            preferencesWindowController.show(preferencePane: .subscribeTab)
            showDock(state: true)
        }
    }

    @IBAction func goRoutingRuleSetting(_ sender: Any) {
        DispatchQueue.main.async {
            preferencesWindowController.show(preferencePane: .routingTab)
            showDock(state: true)
        }
    }
}

// NSv2rayItemListSource
extension ConfigWindowController: NSTableViewDataSource {

    func numberOfRows(in tableView: NSTableView) -> Int {
        return V2rayServer.count()
    }

    func tableView(_ tableView: NSTableView, objectValueFor tableColumn: NSTableColumn?, row: Int) -> Any? {
        let v2rayItemList = V2rayServer.list()
        // set cell data
        if row < v2rayItemList.count {
            return v2rayItemList[row].remark
        }
        return nil
    }

    // edit cell
    func tableView(_ tableView: NSTableView, setObjectValue: Any?, for tableColumn: NSTableColumn?, row: Int) {
        guard let remark = setObjectValue as? String else {
            NSLog("remark is nil")
            return
        }
        DispatchQueue.global().async {
            // edit item remark
            V2rayServer.edit(rowIndex: row, remark: remark)
            // reload table on main thread
            DispatchQueue.main.async {
                tableView.reloadData()
                // reload menu
                menuController.showServers()
            }
        }
    }
}

// NSTableViewDelegate
extension ConfigWindowController: NSTableViewDelegate {
    // For NSTableViewDelegate
    func tableViewSelectionDidChange(_ notification: Notification) {
        self.loadJsonData(rowIndex: self.serversTableView.selectedRow)
        self.errTip.stringValue = ""
    }

    // Drag & Drop reorder rows
    func tableView(_ tableView: NSTableView, pasteboardWriterForRow row: Int) -> NSPasteboardWriting? {
        let item = NSPasteboardItem()
        item.setString(String(row), forType: NSPasteboard.PasteboardType(rawValue: tableViewDragType))
        return item
    }

    func tableView(_ tableView: NSTableView, validateDrop info: NSDraggingInfo, proposedRow row: Int, proposedDropOperation dropOperation: NSTableView.DropOperation) -> NSDragOperation {
        if dropOperation == .above {
            return .move
        }
        return []
    }

    func tableView(_ tableView: NSTableView, acceptDrop info: NSDraggingInfo, row: Int, dropOperation: NSTableView.DropOperation) -> Bool {
        var oldIndexes = [Int]()
        info.enumerateDraggingItems(options: [], for: tableView, classes: [NSPasteboardItem.self], searchOptions: [:]) { (draggingItem, _, _) in
            if let str = (draggingItem.item as! NSPasteboardItem).string(forType: NSPasteboard.PasteboardType(rawValue: self.tableViewDragType)),
               let index = Int(str) {
                oldIndexes.append(index)
            }
        }

        var oldIndexOffset = 0
        var newIndexOffset = 0
        var oldIndexLast = 0
        var newIndexLast = 0

        // For simplicity, the code below uses `tableView.moveRow(at:to:)` to move rows around directly.
        // You may want to move rows in your content array and then call `tableView.reloadData()` instead.
        for oldIndex in oldIndexes {
            if oldIndex < row {
                oldIndexLast = oldIndex + oldIndexOffset
                newIndexLast = row - 1
                oldIndexOffset -= 1
            } else {
                oldIndexLast = oldIndex
                newIndexLast = row + newIndexOffset
                newIndexOffset += 1
            }
        }
        DispatchQueue.global().async {
            // move
            V2rayServer.move(oldIndex: oldIndexLast, newIndex: newIndexLast)
            DispatchQueue.main.async {
                // set selected
                self.serversTableView.selectRowIndexes(IndexSet(integer: newIndexLast), byExtendingSelection: false)
                // reload table
                self.serversTableView.reloadData()
                // reload menu
                menuController.showServers()
            }
        }
        return true
    }
}
