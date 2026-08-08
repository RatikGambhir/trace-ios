import SwiftUI

struct RootView: View {
    let appState: AppState

    var body: some View {
        Group {
            switch appState.destination {
            case .authentication(let initialRoute):
                AuthenticationFlowView(initialRoute: initialRoute) {
                    withAnimation(.easeInOut(duration: 0.3)) {
                        appState.completeAuthentication()
                    }
                }
                .transition(.opacity)

            case .home:
                AppShellView(
                    onShowLogin: {
                        withAnimation(.easeInOut(duration: 0.3)) {
                            appState.showLogin()
                        }
                    },
                    onSignOut: {
                        withAnimation(.easeInOut(duration: 0.3)) {
                            appState.signOut()
                        }
                    }
                )
                    .transition(.opacity)
            }
        }
    }
}
