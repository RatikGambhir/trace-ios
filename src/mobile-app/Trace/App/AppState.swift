import Foundation
import Observation

@Observable
final class AppState {
    enum Destination {
        case authentication(AuthenticationRoute?)
        case home
    }

    private(set) var destination: Destination

    init() {
#if DEBUG
        destination = ProcessInfo.processInfo.arguments.contains("-showAppShell") ? .home : .authentication(nil)
#else
        destination = .authentication(nil)
#endif
    }

    func completeAuthentication() {
        destination = .home
    }

    func signOut() {
        destination = .authentication(nil)
    }

    func showLogin() {
        destination = .authentication(.logIn)
    }
}
