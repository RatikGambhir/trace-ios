import SwiftUI

struct WelcomeView: View {
    let onComplete: () -> Void

    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var showsLogInOptions = false
    @State private var showsPhoneSheet = false

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
            .contentShape(Rectangle())
            .onTapGesture(perform: collapseLogInOptions)

            VStack(spacing: 0) {
                TraceBrandMark()
                    .padding(.top, TraceTheme.Spacing.large)

                Spacer()

                logInActions
                    .padding(.horizontal, TraceTheme.Spacing.large)
                    .padding(.bottom, TraceTheme.Spacing.xxLarge)
            }
        }
        .background { LandingBackground() }
        .sheet(isPresented: $showsPhoneSheet) {
            PhoneLogInSheet {
                showsPhoneSheet = false
                onComplete()
            }
            .presentationDetents([.height(380)])
            .presentationDragIndicator(.visible)
            .presentationBackground(TraceTheme.Colors.background)
            .presentationCornerRadius(TraceTheme.Radius.panel)
        }
        .toolbar(.hidden, for: .navigationBar)
    }

    private var logInActions: some View {
        VStack(spacing: TraceTheme.Spacing.small + TraceTheme.Spacing.xSmall) {
            if showsLogInOptions {
                Button(action: onComplete) {
                    HStack(spacing: 10) {
                        Image(systemName: "apple.logo")
                            .font(.system(size: 19, weight: .medium))
                        Text("Log in with Apple ID")
                    }
                }
                .buttonStyle(TraceFormButtonStyle())
                .transition(
                    .offset(y: 56 + TraceTheme.Spacing.small + TraceTheme.Spacing.xSmall)
                        .combined(with: .scale(scale: 0.96, anchor: .bottom))
                        .combined(with: .opacity)
                )
            }

            WelcomeLogInButton(
                showsOptions: showsLogInOptions,
                action: handleLogInButton
            )
        }
        .frame(height: 124, alignment: .bottom)
        .animation(logInAnimation, value: showsLogInOptions)
        .accessibilityAction(named: "Collapse login options", collapseLogInOptions)
    }

    private var logInAnimation: Animation {
        reduceMotion
            ? .easeOut(duration: 0.2)
            : .spring(response: 0.48, dampingFraction: 0.86)
    }

    private func handleLogInButton() {
        if showsLogInOptions {
            showsPhoneSheet = true
        } else {
            withAnimation(logInAnimation) {
                showsLogInOptions = true
            }
        }
    }

    private func collapseLogInOptions() {
        guard showsLogInOptions else { return }

        withAnimation(logInAnimation) {
            showsLogInOptions = false
        }
    }
}

private struct WelcomeLogInButton: View {
    let showsOptions: Bool
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            label
        }
        .buttonStyle(TraceGlassButtonStyle())
    }

    private var label: some View {
        ZStack {
            Text("Log In")
                .opacity(showsOptions ? 0 : 1)
                .scaleEffect(showsOptions ? 0.96 : 1)

            HStack(spacing: 10) {
                Image(systemName: "phone.fill")
                    .font(.system(size: 16, weight: .semibold))
                Text("Log in with phone number")
            }
            .opacity(showsOptions ? 1 : 0)
            .scaleEffect(showsOptions ? 1 : 0.96)
        }
        .font(TraceTheme.Fonts.action)
        .foregroundStyle(.white)
        .frame(maxWidth: .infinity)
        .frame(height: 56)
    }
}

#Preview {
    NavigationStack {
        WelcomeView(onComplete: {})
    }
}
