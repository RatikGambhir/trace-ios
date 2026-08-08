import SwiftUI

struct AppearanceSettingsView: View {
    @AppStorage(ThemePreference.storageKey) private var themePreference = ThemePreference.dark
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: TraceTheme.Spacing.large) {
                Text("Choose how Trace looks. Your pick applies right away, everywhere in the app.")
                    .font(TraceTheme.Fonts.profileBody)
                    .foregroundStyle(TraceTheme.Colors.profileSecondaryText)

                HStack(spacing: TraceTheme.Spacing.medium) {
                    ForEach(ThemePreference.allCases) { preference in
                        ThemeOptionCard(
                            preference: preference,
                            isSelected: themePreference == preference
                        ) {
                            withAnimation(reduceMotion ? nil : .easeInOut(duration: 0.25)) {
                                themePreference = preference
                            }
                        }
                    }
                }
            }
            .padding(TraceTheme.Spacing.medium)
            .frame(maxWidth: 560)
            .frame(maxWidth: .infinity)
        }
        .background(TraceTheme.Colors.background)
        .navigationTitle("Appearance")
        .navigationBarTitleDisplayMode(.inline)
    }
}

private struct ThemeOptionCard: View {
    let preference: ThemePreference
    let isSelected: Bool
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            VStack(spacing: TraceTheme.Spacing.small) {
                ThemeMiniPreview(preference: preference)
                    .frame(height: 132)
                    .frame(maxWidth: .infinity)
                    .clipShape(RoundedRectangle(cornerRadius: TraceTheme.Radius.field, style: .continuous))
                    .overlay {
                        RoundedRectangle(cornerRadius: TraceTheme.Radius.field, style: .continuous)
                            .stroke(
                                isSelected ? TraceTheme.Colors.accent : TraceTheme.Colors.divider,
                                lineWidth: isSelected ? 2 : 1
                            )
                    }

                HStack(spacing: TraceTheme.Spacing.xSmall) {
                    Image(systemName: isSelected ? "checkmark.circle.fill" : preference.symbol)
                        .font(.system(size: 14, weight: .semibold))
                    Text(preference.title)
                        .font(.system(size: 15, weight: .semibold))
                }
                .foregroundStyle(isSelected ? TraceTheme.Colors.accent : TraceTheme.Colors.primaryText)
                .frame(minHeight: 28)
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityLabel("\(preference.title) theme")
        .accessibilityAddTraits(isSelected ? [.isSelected] : [])
    }
}

// Fixed palette swatches: each card must show its own theme's colors,
// not the asset colors resolved for the currently active scheme.
private struct ThemeMiniPreview: View {
    let preference: ThemePreference

    var body: some View {
        ZStack {
            background

            VStack(alignment: .leading, spacing: 8) {
                HStack(spacing: 8) {
                    Circle()
                        .fill(accent)
                        .frame(width: 26, height: 26)

                    VStack(alignment: .leading, spacing: 4) {
                        Capsule()
                            .fill(text)
                            .frame(width: 64, height: 7)
                        Capsule()
                            .fill(text.opacity(0.45))
                            .frame(width: 42, height: 5)
                    }
                }

                Capsule()
                    .fill(accent)
                    .frame(height: 18)

                Capsule()
                    .fill(surface)
                    .frame(height: 18)
                    .overlay {
                        Capsule().stroke(text.opacity(0.14), lineWidth: 1)
                    }
            }
            .padding(14)
        }
        .accessibilityHidden(true)
    }

    private var background: Color {
        switch preference {
        case .light: Color(red: 0.965, green: 0.949, blue: 0.918)
        case .dark: Color(red: 0.055, green: 0.082, blue: 0.141)
        }
    }

    private var surface: Color {
        switch preference {
        case .light: Color(red: 0.992, green: 0.984, blue: 0.969)
        case .dark: Color(red: 0.106, green: 0.141, blue: 0.212)
        }
    }

    private var text: Color {
        switch preference {
        case .light: Color(red: 0.110, green: 0.133, blue: 0.188)
        case .dark: Color(red: 0.961, green: 0.949, blue: 0.925)
        }
    }

    private var accent: Color {
        switch preference {
        case .light: Color(red: 0.690, green: 0.322, blue: 0.180)
        case .dark: Color(red: 0.757, green: 0.380, blue: 0.235)
        }
    }
}

#Preview("Appearance") {
    NavigationStack { AppearanceSettingsView() }
}
