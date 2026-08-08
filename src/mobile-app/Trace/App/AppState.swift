import Observation

@Observable
final class AppState {
    enum Destination {
        case authentication
        case home
    }

    private(set) var destination: Destination = .authentication

    func completeAuthentication() {
        destination = .home
    }
}
