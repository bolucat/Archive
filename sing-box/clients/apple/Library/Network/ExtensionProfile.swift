import Foundation
import Libbox
import NetworkExtension

public class ExtensionProfile: ObservableObject {
    private let manager: NEVPNManager
    private var connection: NEVPNConnection
    private var observer: Any?

    @Published public var status: NEVPNStatus

    public init(_ manager: NEVPNManager) {
        self.manager = manager
        connection = manager.connection
        status = manager.connection.status
    }

    public func register() {
        observer = NotificationCenter.default.addObserver(
            forName: NSNotification.Name.NEVPNStatusDidChange,
            object: manager.connection,
            queue: .main
        ) { [weak self] notification in
            guard let self else {
                return
            }
            self.connection = notification.object as! NEVPNConnection
            self.status = self.connection.status
        }
    }

    private func unregister() {
        if let observer {
            NotificationCenter.default.removeObserver(observer)
        }
    }

    private func setOnDemandRules() {
        let interfaceRule = NEOnDemandRuleConnect()
        interfaceRule.interfaceTypeMatch = .any
        let probeRule = NEOnDemandRuleConnect()
        probeRule.probeURL = URL(string: "http://captive.apple.com")
        manager.onDemandRules = [interfaceRule, probeRule]
    }

    public func updateAlwaysOn(_ newState: Bool) async throws {
        manager.isOnDemandEnabled = newState
        setOnDemandRules()
        try await manager.saveToPreferences()
    }

    @available(iOS 16.0, macOS 13.0, tvOS 17.0, *)
    public func fetchLastDisconnectError() async throws {
        try await connection.fetchLastDisconnectError()
    }

    public func start() async throws {
        await fetchProfile()
        manager.isEnabled = true
        if await SharedPreferences.alwaysOn.get() {
            manager.isOnDemandEnabled = true
            setOnDemandRules()
        }
        #if !os(tvOS)
            if let protocolConfiguration = manager.protocolConfiguration {
                let includeAllNetworks = await SharedPreferences.includeAllNetworks.get()
                protocolConfiguration.includeAllNetworks = includeAllNetworks
                if #available(iOS 16.4, macOS 13.3, *) {
                    protocolConfiguration.excludeCellularServices = !includeAllNetworks
                }
            }
        #endif
        try await manager.saveToPreferences()
        #if os(macOS)
            if Variant.useSystemExtension {
                try manager.connection.startVPNTunnel(options: [
                    "username": NSString(string: NSUserName()),
                ])
                return
            }
        #endif
        try manager.connection.startVPNTunnel()
    }

    public func fetchProfile() async {
        do {
            if let profile = try await ProfileManager.get(Int64(SharedPreferences.selectedProfileID.get())) {
                if profile.type == .icloud {
                    _ = try profile.read()
                }
            }
        } catch {}
    }

    public func stop() async throws {
        if manager.isOnDemandEnabled {
            manager.isOnDemandEnabled = false
            try await manager.saveToPreferences()
        }
        do {
            try LibboxNewStandaloneCommandClient()!.serviceClose()
        } catch {}
        manager.connection.stopVPNTunnel()
    }

    public static func load() async throws -> ExtensionProfile? {
        let managers = try await NETunnelProviderManager.loadAllFromPreferences()
        if managers.isEmpty {
            return nil
        }
        let profile = ExtensionProfile(managers[0])
        return profile
    }

    public static func install() async throws {
        let manager = NETunnelProviderManager()
        manager.localizedDescription = Variant.applicationName
        let tunnelProtocol = NETunnelProviderProtocol()
        if Variant.useSystemExtension {
            tunnelProtocol.providerBundleIdentifier = "\(FilePath.packageName).system"
        } else {
            tunnelProtocol.providerBundleIdentifier = "\(FilePath.packageName).extension"
        }
        tunnelProtocol.serverAddress = "sing-box"
        manager.protocolConfiguration = tunnelProtocol
        manager.isEnabled = true
        try await manager.saveToPreferences()
    }
}
