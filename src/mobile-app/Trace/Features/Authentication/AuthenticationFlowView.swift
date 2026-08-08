import SwiftUI

struct AuthenticationFlowView: View {
    let onComplete: () -> Void

    var body: some View {
        NavigationStack {
            WelcomeView()
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
