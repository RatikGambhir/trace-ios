import Foundation
import SwiftUI

struct AuthenticationFlowView: View {
    let onComplete: () -> Void
    @State private var path: [AuthenticationRoute]

    init(
        initialRoute: AuthenticationRoute? = nil,
        onComplete: @escaping () -> Void
    ) {
        self.onComplete = onComplete
#if DEBUG
        let arguments = ProcessInfo.processInfo.arguments
        if arguments.contains("-showRegister") {
            _path = State(initialValue: [.createAccount])
        } else if arguments.contains("-showLogin") {
            _path = State(initialValue: [.logIn])
        } else {
            _path = State(initialValue: initialRoute.map { [$0] } ?? [])
        }
#else
        _path = State(initialValue: initialRoute.map { [$0] } ?? [])
#endif
    }

    var body: some View {
        NavigationStack(path: $path) {
            WelcomeView(onComplete: onComplete)
                .navigationDestination(for: AuthenticationRoute.self) { route in
                    switch route {
                    case .createAccount:
                        CreateAccountView(onComplete: onComplete)
                    case .logIn:
                        LogInView(onComplete: onComplete)
                    }
                }
        }
        .tint(.white)
    }
}
