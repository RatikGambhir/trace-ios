import SwiftUI

struct DiscoverRootView: View {
    var body: some View {
        ScrollView {
            LazyVStack(alignment: .leading, spacing: TraceTheme.Spacing.large) {
                VStack(alignment: .leading, spacing: TraceTheme.Spacing.small) {
                    Text("Discover")
                        .font(.system(size: 32, weight: .medium, design: .serif))
                    Text("A short list for your next open afternoon.")
                        .font(TraceTheme.Fonts.profileBody)
                        .foregroundStyle(TraceTheme.Colors.profileSecondaryText)
                }

                DiscoveryStory(
                    eyebrow: "CHICAGO · WEEKEND",
                    title: "Three rooms with a sense of occasion",
                    symbol: "fork.knife"
                )

                DiscoveryStory(
                    eyebrow: "ALONG THE LAKE",
                    title: "A slow walk, a good coffee, and nowhere else to be",
                    symbol: "cup.and.saucer"
                )
            }
            .padding(TraceTheme.Spacing.medium)
            .frame(maxWidth: 720)
            .frame(maxWidth: .infinity)
        }
        .background(TraceTheme.Colors.background)
        .navigationBarHidden(true)
    }
}

private struct DiscoveryStory: View {
    let eyebrow: String
    let title: String
    let symbol: String

    var body: some View {
        VStack(alignment: .leading, spacing: TraceTheme.Spacing.medium) {
            Image(systemName: symbol)
                .font(.system(size: 30, weight: .light))
                .foregroundStyle(TraceTheme.Colors.accent)
                .frame(width: 56, height: 56)
                .background(TraceTheme.Colors.background, in: Circle())

            Text(eyebrow)
                .font(TraceTheme.Fonts.label)
                .tracking(1.4)
                .foregroundStyle(TraceTheme.Colors.profileSecondaryText)

            Text(title)
                .font(.system(size: 24, weight: .medium, design: .serif))
                .foregroundStyle(TraceTheme.Colors.primaryText)
        }
        .padding(TraceTheme.Spacing.large)
        .frame(maxWidth: .infinity, minHeight: 220, alignment: .bottomLeading)
        .background(TraceTheme.Colors.surface)
        .clipShape(RoundedRectangle(cornerRadius: TraceTheme.Radius.large, style: .continuous))
    }
}
