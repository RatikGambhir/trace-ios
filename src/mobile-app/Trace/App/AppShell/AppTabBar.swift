import SwiftUI

struct AppTabBar: View {
    let selectedTab: AppTab
    let profile: UserProfile?
    let onSelect: (AppTab) -> Void

    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        HStack(spacing: TraceTheme.Spacing.xSmall) {
            ForEach(AppTab.allCases) { tab in
                Button {
                    onSelect(tab)
                } label: {
                    VStack(spacing: TraceTheme.Spacing.xSmall) {
                        tabIcon(tab)
                            .font(.system(size: 22, weight: .medium))
                            .frame(height: 24)

                        Text(tab.title)
                            .font(.system(size: 12, weight: .medium))
                            .lineLimit(1)
                    }
                    .foregroundStyle(
                        selectedTab == tab
                            ? TraceTheme.Colors.accent
                            : TraceTheme.Colors.profileSecondaryText
                    )
                    .frame(maxWidth: .infinity)
                    .frame(minHeight: 60)
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .accessibilityLabel(tab.title)
                .accessibilityAddTraits(selectedTab == tab ? .isSelected : [])
            }
        }
        .padding(TraceTheme.Spacing.small)
        .background(TraceTheme.Colors.surface, in: Capsule())
        .overlay {
            Capsule()
                .stroke(TraceTheme.Colors.divider.opacity(0.8), lineWidth: 1)
        }
        .shadow(color: .black.opacity(0.14), radius: 24, y: 10)
        .padding(.horizontal, TraceTheme.Spacing.medium)
        .padding(.top, TraceTheme.Spacing.small)
        .padding(.bottom, TraceTheme.Spacing.xSmall)
        .animation(reduceMotion ? nil : .snappy(duration: 0.22), value: selectedTab)
    }

    @ViewBuilder
    private func tabIcon(_ tab: AppTab) -> some View {
        if tab == .profile, let profile {
            Text(profile.initials)
                .font(.system(size: 9, weight: .bold))
                .foregroundStyle(.white)
                .frame(width: 24, height: 24)
                .background(TraceTheme.Colors.accent, in: Circle())
                .accessibilityHidden(true)
        } else {
            Image(systemName: selectedTab == tab ? tab.selectedSymbol : tab.symbol)
                .symbolRenderingMode(.hierarchical)
                .accessibilityHidden(true)
        }
    }
}

#Preview {
    AppTabBar(selectedTab: .profile, profile: ProfileContent.preview.profile) { _ in }
        .padding(.vertical)
        .background(TraceTheme.Colors.background)
}
