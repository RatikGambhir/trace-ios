import SwiftUI

struct TraceGlassPanel<Content: View>: View {
    private let content: Content

    init(@ViewBuilder content: () -> Content) {
        self.content = content()
    }

    var body: some View {
        content
            .padding(22)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background {
                RoundedRectangle(cornerRadius: TraceTheme.Radius.panel, style: .continuous)
                    .fill(.ultraThinMaterial)
                RoundedRectangle(cornerRadius: TraceTheme.Radius.panel, style: .continuous)
                    .fill(.black.opacity(0.22))
                RoundedRectangle(cornerRadius: TraceTheme.Radius.panel, style: .continuous)
                    .stroke(.white.opacity(0.12), lineWidth: 1)
            }
    }
}
