import SwiftUI

struct TraceGlassButtonStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .background(
                .ultraThinMaterial,
                in: RoundedRectangle(cornerRadius: TraceTheme.Radius.field)
            )
            .overlay {
                RoundedRectangle(cornerRadius: TraceTheme.Radius.field)
                    .stroke(.white.opacity(0.22), lineWidth: 1)
            }
            .shadow(color: .black.opacity(0.18), radius: 14, y: 8)
            .opacity(configuration.isPressed ? 0.76 : 1)
            .scaleEffect(configuration.isPressed ? 0.98 : 1)
            .animation(.easeOut(duration: 0.16), value: configuration.isPressed)
    }
}

struct TraceFormButtonStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(TraceTheme.Fonts.action)
            .foregroundStyle(.black)
            .frame(maxWidth: .infinity)
            .frame(height: 56)
            .background(
                TraceTheme.Colors.paper,
                in: RoundedRectangle(cornerRadius: TraceTheme.Radius.field)
            )
            .opacity(configuration.isPressed ? 0.82 : 1)
            .scaleEffect(configuration.isPressed ? 0.98 : 1)
            .animation(.easeOut(duration: 0.16), value: configuration.isPressed)
    }
}
