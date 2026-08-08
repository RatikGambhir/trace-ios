import SwiftUI

struct ProfileActionButtons: View {
    let onAdd: () -> Void
    let onMap: () -> Void
    let onShowLogin: () -> Void

    var body: some View {
        VStack(spacing: TraceTheme.Spacing.small) {
            HStack(spacing: TraceTheme.Spacing.small) {
                ProfileActionButton(
                    title: "Add",
                    symbol: "plus",
                    isProminent: true,
                    action: onAdd
                )
                ProfileActionButton(
                    title: "Map",
                    symbol: "globe.americas",
                    isProminent: false,
                    action: onMap
                )
            }

            Button(action: onShowLogin) {
                HStack(spacing: TraceTheme.Spacing.small) {
                    Image(systemName: "arrow.backward.circle.fill")
                        .font(.system(size: 18, weight: .semibold))
                        .symbolRenderingMode(.hierarchical)
                        .foregroundStyle(TraceTheme.Colors.accent)

                    Text("View login")
                        .font(.system(size: 15, weight: .semibold))

                    Spacer()

                    Text("AUTH PREVIEW")
                        .font(TraceTheme.Fonts.label)
                        .tracking(1.2)
                        .foregroundStyle(TraceTheme.Colors.profileSecondaryText)
                }
                .foregroundStyle(TraceTheme.Colors.primaryText)
                .padding(.horizontal, TraceTheme.Spacing.medium)
                .frame(height: 48)
                .background(TraceTheme.Colors.surface, in: Capsule())
                .overlay {
                    Capsule()
                        .stroke(TraceTheme.Colors.divider, lineWidth: 1)
                }
                .contentShape(Capsule())
            }
            .buttonStyle(.plain)
            .accessibilityLabel("View login screen")
            .accessibilityHint("Returns to the login form without signing out through the welcome screen")
        }
    }
}

private struct ProfileActionButton: View {
    let title: String
    let symbol: String
    let isProminent: Bool
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Label(title, systemImage: symbol)
                .font(.system(size: 17, weight: .semibold))
                .foregroundStyle(isProminent ? .white : TraceTheme.Colors.primaryText)
                .frame(maxWidth: .infinity)
                .frame(height: 56)
                .background(
                    isProminent ? TraceTheme.Colors.accent : TraceTheme.Colors.surface,
                    in: Capsule()
                )
                .overlay {
                    if !isProminent {
                        Capsule()
                            .stroke(TraceTheme.Colors.divider, lineWidth: 1)
                    }
                }
                .contentShape(Capsule())
        }
        .buttonStyle(.plain)
        .accessibilityLabel(title == "Map" ? "View saved places on map" : "Add a place")
    }
}
