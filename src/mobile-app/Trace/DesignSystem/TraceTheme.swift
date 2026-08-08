import SwiftUI

enum TraceTheme {
    enum Colors {
        static let paper = Color(red: 0.96, green: 0.95, blue: 0.92)
        static let fieldFill = Color.black.opacity(0.18)
        static let fieldBorder = Color.white.opacity(0.13)
        static let secondaryText = Color.white.opacity(0.62)
        static let tertiaryText = Color.white.opacity(0.48)
    }

    enum Spacing {
        static let xSmall: CGFloat = 4
        static let small: CGFloat = 8
        static let medium: CGFloat = 16
        static let large: CGFloat = 24
        static let xLarge: CGFloat = 32
        static let xxLarge: CGFloat = 48
    }

    enum Radius {
        static let field: CGFloat = 16
        static let panel: CGFloat = 30
    }

    enum Fonts {
        static let display = Font.system(size: 36, weight: .medium, design: .serif)
        static let formTitle = Font.system(size: 32, weight: .medium, design: .serif)
        static let body = Font.system(size: 15, weight: .regular)
        static let action = Font.system(size: 16, weight: .semibold)
        static let label = Font.system(size: 10, weight: .semibold)
    }
}
