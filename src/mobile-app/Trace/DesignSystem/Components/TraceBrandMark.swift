import SwiftUI

struct TraceBrandMark: View {
    var compact = false

    var body: some View {
        VStack(spacing: compact ? 7 : TraceTheme.Spacing.small) {
            Text("TRACE")
                .font(.system(size: compact ? 18 : 23, weight: .semibold, design: .serif))
                .tracking(compact ? 6 : 8)
                .padding(.leading, compact ? 6 : 8)

            Text("JOURNEYS, REMEMBERED")
                .font(.system(size: compact ? 8 : 9, weight: .semibold))
                .tracking(compact ? 2.4 : 2.8)
                .foregroundStyle(.white.opacity(compact ? 0.48 : 0.54))
        }
        .foregroundStyle(.white)
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isHeader)
    }
}
