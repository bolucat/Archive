import MacLibrary
import SwiftUI

@main
struct Application: App {
    @NSApplicationDelegateAdaptor private var appDelegate: ApplicationDelegate

    var body: some Scene {
        MacApplication()
    }
}
