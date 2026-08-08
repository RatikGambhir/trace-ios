import SwiftUI

struct TracePrimaryButtonStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(TraceTheme.Fonts.action)
            .foregroundStyle(.black)
            .padding(.leading, 20)
            .padding(.trailing, 11)
            .frame(maxWidth: .infinity)
            .frame(height: 58)
            .background(TraceTheme.Colors.paper, in: Capsule())
            .opacity(configuration.isPressed ? 0.82 : 1)
            .scaleEffect(configuration.isPressed ? 0.98 : 1)
            .animation(.easeOut(duration: 0.16), value: configuration.isPressed)
    }
}

struct TraceTextButtonStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.system(size: 14, weight: .medium))
            .frame(minHeight: 44)
            .opacity(configuration.isPressed ? 0.72 : 1)
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
