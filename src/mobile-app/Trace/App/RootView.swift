import SwiftUI

struct RootView: View {
    let appState: AppState

    var body: some View {
        Group {
            switch appState.destination {
            case .authentication:
                AuthenticationFlowView {
                    withAnimation(.easeInOut(duration: 0.3)) {
                        appState.completeAuthentication()
                    }
                }
                .transition(.opacity)

            case .home:
                HomeView()
                    .transition(.opacity)
            }
        }
    }
}
