import SwiftUI

struct ContentView: View {
    let appState: AppState

    var body: some View {
        RootView(appState: appState)
    }
}

#Preview {
    ContentView(appState: AppState())
}
