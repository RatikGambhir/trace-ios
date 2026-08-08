import SwiftUI

enum ThemePreference: String, CaseIterable, Identifiable {
    case light
    case dark

    static let storageKey = "themePreference"

    var id: String { rawValue }

    var colorScheme: ColorScheme {
        switch self {
        case .light: .light
        case .dark: .dark
        }
    }

    var title: String {
        switch self {
        case .light: "Light"
        case .dark: "Dark"
        }
    }

    var symbol: String {
        switch self {
        case .light: "sun.max.fill"
        case .dark: "moon.fill"
        }
    }
}
