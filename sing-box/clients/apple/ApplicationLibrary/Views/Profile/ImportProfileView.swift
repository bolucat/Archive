#if os(tvOS)

    import DeviceDiscoveryUI
    import Libbox
    import Library
    import SwiftUI

    @MainActor
    public struct ImportProfileView: View {
        @EnvironmentObject private var environments: ExtensionEnvironments
        @Environment(\.dismiss) private var dismiss

        @State private var isLoading = false
        @State private var selected = false
        @State private var alert: Alert?
        @State private var connection: NWConnection?
        @State private var socket: NWSocket?
        @State private var profiles: [LibboxProfilePreview]?
        @State private var isImporting = false

        public init() {}
        public var body: some View {
            VStack(alignment: .center) {
                if !selected {
                    Form {
                        Section {
                            EmptyView()
                        } footer: {
                            Text("To import configurations from your iPhone or iPad, make sure sing-box is the **same version** on both devices and **VPN is disabled**.")
                        }

                        DevicePicker(
                            .applicationService(name: "sing-box:profile"))
                        { endpoint in
                            selected = true
                            Task {
                                await handleEndpoint(endpoint)
                            }
                        } label: {
                            Text("Select Device")
                        } fallback: {
                            EmptyView()
                        } parameters: {
                            .applicationService
                        }
                    }
                } else if let profiles {
                    Form {
                        Section {
                            EmptyView()
                        } footer: {
                            Text("\(profiles.count) Profiles")
                        }
                        ForEach(profiles, id: \.profileID) { profile in
                            Button(profile.name) {
                                isLoading = true
                                Task {
                                    selectProfile(profileID: profile.profileID)
                                    isLoading = false
                                }
                            }.disabled(isLoading || isImporting)
                        }
                    }
                } else {
                    Text("Connecting...")
                }
            }
            .focusSection()
            .alertBinding($alert)
            .navigationTitle("Import Profile")
        }

        private func reset() {
            if let connection {
                connection.stateUpdateHandler = nil
                connection.cancel()
                self.connection = nil
            }
            if let socket {
                socket.cancel()
                self.socket = nil
            }
            selected = false
            profiles = nil
        }

        private func handleEndpoint(_ endpoint: NWEndpoint) async {
            let connection = NWConnection(to: endpoint, using: NWParameters.applicationService)
            self.connection = connection
            socket = NWSocket(connection)
            connection.stateUpdateHandler = { state in
                switch state {
                case let .failed(error):
                    DispatchQueue.main.async { [self] in
                        reset()
                        alert = Alert(error)
                    }
                default: break
                }
            }
            connection.start(queue: .global())
            do {
                try await loopMessages()
            } catch {
                alert = Alert(error)
                reset()
            }
        }

        private nonisolated func loopMessages() async throws {
            guard let socket = await socket else {
                return
            }
            var message: Data
            while true {
                do {
                    message = try socket.read()
                } catch {
                    throw NSError(domain: "read from connection: \(error.localizedDescription)", code: 0)
                }
                var error: NSError?
                switch Int64(message[0]) {
                case LibboxMessageTypeError:
                    let message = LibboxDecodeErrorMessage(message, &error)
                    if let error {
                        throw error
                    }
                    if let message {
                        throw NSError(domain: "remote error: \(message.message)", code: 0)
                    }
                case LibboxMessageTypeProfileList:
                    let decoder = LibboxProfileDecoder()
                    try decoder.decode(message)
                    let iterator = decoder.iterator()!
                    var profiles = [LibboxProfilePreview]()
                    while iterator.hasNext() {
                        let profile = iterator.next()!
                        if profile.type == LibboxProfileTypeiCloud {
                            // not supported on tvOS
                            continue
                        }
                        profiles.append(profile)
                    }
                    await MainActor.run { [self, profiles] in
                        self.profiles = profiles
                        isImporting = false
                    }
                case LibboxMessageTypeProfileContent:
                    let content = LibboxDecodeProfileContent(message, &error)
                    if let error {
                        throw error
                    }
                    try await importProfile(content!)
                    return
                default:
                    throw NSError(domain: "unknown message type \(message[0])", code: 0)
                }
            }
        }

        private func selectProfile(profileID: Int64) {
            guard let connection else {
                return
            }
            guard let socket else {
                return
            }
            connection.stateUpdateHandler = nil
            let request = LibboxProfileContentRequest()
            request.profileID = profileID
            do {
                try socket.write(request.encode())
                isImporting = true
            } catch {
                alert = Alert(error)
                reset()
            }
        }

        private nonisolated func importProfile(_ content: LibboxProfileContent) async throws {
            var type: ProfileType = .local
            switch content.type {
            case LibboxProfileTypeLocal:
                type = .local
            case LibboxProfileTypeiCloud:
                type = .icloud
            case LibboxProfileTypeRemote:
                type = .remote
            default:
                break
            }
            let nextProfileID = try await ProfileManager.nextID()
            let profileConfigDirectory = FilePath.sharedDirectory.appendingPathComponent("configs", isDirectory: true)
            try FileManager.default.createDirectory(at: profileConfigDirectory, withIntermediateDirectories: true)
            let profileConfig = profileConfigDirectory.appendingPathComponent("config_\(nextProfileID).json")
            try content.config.write(to: profileConfig, atomically: true, encoding: .utf8)
            var lastUpdated: Date?
            if content.lastUpdated > 0 {
                lastUpdated = Date(timeIntervalSince1970: Double(content.lastUpdated))
            }
            try await ProfileManager.create(Profile(name: content.name, type: type, path: profileConfig.relativePath, remoteURL: content.remotePath, autoUpdate: content.autoUpdate, lastUpdated: lastUpdated))
            await reset()
            await MainActor.run {
                environments.profileUpdate.send()
                dismiss()
            }
        }
    }

#endif
