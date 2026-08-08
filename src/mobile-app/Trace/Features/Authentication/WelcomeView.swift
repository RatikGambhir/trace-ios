import SwiftUI

struct WelcomeView: View {
    var body: some View {
        ZStack {
            LinearGradient(
                stops: [
                    .init(color: .black.opacity(0.32), location: 0),
                    .init(color: .clear, location: 0.32),
                    .init(color: .clear, location: 0.60),
                    .init(color: .black.opacity(0.72), location: 1)
                ],
                startPoint: .top,
                endPoint: .bottom
            )
            .ignoresSafeArea()

            VStack(spacing: 0) {
                TraceBrandMark()
                    .padding(.top, TraceTheme.Spacing.large)

                Spacer()

                TraceGlassPanel {
                    VStack(alignment: .leading, spacing: 0) {
                        Text("Keep the places\nthat move you.")
                            .font(TraceTheme.Fonts.display)
                            .tracking(-0.6)
                            .foregroundStyle(.white)
                            .lineSpacing(1)

                        Text("A beautiful home for every journey, memory, and mile.")
                            .font(TraceTheme.Fonts.body)
                            .foregroundStyle(TraceTheme.Colors.secondaryText)
                            .lineSpacing(3)
                            .padding(.top, 12)
                            .padding(.bottom, TraceTheme.Spacing.large)

                        NavigationLink(value: AuthenticationRoute.createAccount) {
                            HStack {
                                Text("Get Started")
                                Spacer()
                                Image(systemName: "arrow.up.right")
                                    .font(.system(size: 14, weight: .semibold))
                                    .frame(width: 34, height: 34)
                                    .background(.black.opacity(0.08), in: Circle())
                            }
                        }
                        .buttonStyle(TracePrimaryButtonStyle())

                        NavigationLink(value: AuthenticationRoute.logIn) {
                            HStack(spacing: 6) {
                                Text("Already have an account?")
                                    .foregroundStyle(.white.opacity(0.56))
                                Text("Log In")
                                    .foregroundStyle(.white)
                                Image(systemName: "arrow.right")
                                    .font(.system(size: 12, weight: .semibold))
                            }
                            .frame(maxWidth: .infinity)
                        }
                        .buttonStyle(TraceTextButtonStyle())
                        .padding(.top, 18)
                    }
                }
                .padding(.horizontal, TraceTheme.Spacing.medium)
                .padding(.bottom, 10)
            }
        }
        .background { LandingBackground() }
        .toolbar(.hidden, for: .navigationBar)
        .preferredColorScheme(.dark)
    }
}

#Preview {
    NavigationStack {
        WelcomeView()
    }
}
