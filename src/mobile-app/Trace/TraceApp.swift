import SwiftUI

@main
struct TraceApp: App {
    @State private var appState = AppState()
    @AppStorage(ThemePreference.storageKey) private var themePreference = ThemePreference.dark

    var body: some Scene {
        WindowGroup {
            ContentView(appState: appState)
                .tint(TraceTheme.Colors.accent)
                .preferredColorScheme(activeColorScheme)
        }
    }

    // Single source of truth for the window's scheme. Child views must not set
    // preferredColorScheme: a sheet doing so resets the window to the system
    // scheme on dismiss, overriding this one.
    private var activeColorScheme: ColorScheme {
        if case .authentication = appState.destination {
            return .dark
        }
        return themePreference.colorScheme
    }
}
