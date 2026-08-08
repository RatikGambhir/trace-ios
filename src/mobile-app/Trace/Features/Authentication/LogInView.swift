import SwiftUI

struct LogInView: View {
    let onComplete: () -> Void

    @State private var showPhoneSheet = false

    var body: some View {
        ZStack {
            Color.black.opacity(0.4)
                .ignoresSafeArea()

            VStack(spacing: 0) {
                TraceBrandMark(compact: true)
                    .padding(.top, TraceTheme.Spacing.xLarge)

                Spacer()

                VStack(alignment: .leading, spacing: TraceTheme.Spacing.large) {
                    VStack(alignment: .leading, spacing: 10) {
                        Text("Welcome Back")
                            .font(TraceTheme.Fonts.formTitle)
                            .tracking(-0.4)
                            .foregroundStyle(.white)

                        Text("Log in to continue your journey with Trace.")
                            .font(TraceTheme.Fonts.body)
                            .foregroundStyle(TraceTheme.Colors.secondaryText)
                            .lineSpacing(3)
                    }

                    VStack(spacing: TraceTheme.Spacing.small + 4) {
                        Button(action: onComplete) {
                            HStack(spacing: 10) {
                                Image(systemName: "apple.logo")
                                    .font(.system(size: 19, weight: .medium))
                                Text("Log in with Apple ID")
                            }
                        }
                        .buttonStyle(TraceFormButtonStyle())

                        Button {
                            showPhoneSheet = true
                        } label: {
                            HStack(spacing: 10) {
                                Image(systemName: "phone.fill")
                                    .font(.system(size: 16, weight: .semibold))
                                Text("Log in with phone number")
                            }
                            .font(TraceTheme.Fonts.action)
                            .foregroundStyle(.white)
                            .frame(maxWidth: .infinity)
                            .frame(height: 56)
                        }
                        .buttonStyle(TraceGlassButtonStyle())
                    }
                }
                .padding(.horizontal, TraceTheme.Spacing.large)
                .padding(.bottom, TraceTheme.Spacing.xLarge)
            }
        }
        .background {
            LandingBackground()
                .blur(radius: 10)
                .scaleEffect(1.08)
        }
        .sheet(isPresented: $showPhoneSheet) {
            PhoneLogInSheet {
                showPhoneSheet = false
                onComplete()
            }
            .presentationDetents([.height(380)])
            .presentationDragIndicator(.visible)
            .presentationBackground(TraceTheme.Colors.background)
            .presentationCornerRadius(TraceTheme.Radius.panel)
        }
        .navigationBarTitleDisplayMode(.inline)
        .toolbarBackground(.hidden, for: .navigationBar)
    }
}

struct PhoneLogInSheet: View {
    let onNext: () -> Void

    @State private var phoneNumber = ""
    @FocusState private var isPhoneFocused: Bool

    private var digits: String { phoneNumber.filter(\.isNumber) }
    private var canContinue: Bool { digits.count >= 7 }

    var body: some View {
        VStack(alignment: .leading, spacing: TraceTheme.Spacing.large) {
            VStack(alignment: .leading, spacing: 10) {
                Text("Log in with phone")
                    .font(.system(size: 27, weight: .medium, design: .serif))
                    .foregroundStyle(TraceTheme.Colors.primaryText)

                Text("You'll be able to save places, get recommendations, and do other nice things.")
                    .font(TraceTheme.Fonts.body)
                    .foregroundStyle(TraceTheme.Colors.profileSecondaryText)
                    .lineSpacing(3)
            }

            HStack(spacing: TraceTheme.Spacing.small) {
                HStack(spacing: 6) {
                    Text("🇺🇸")
                    Text("+1")
                        .foregroundStyle(TraceTheme.Colors.primaryText)
                    Image(systemName: "chevron.down")
                        .font(.system(size: 12, weight: .semibold))
                        .foregroundStyle(TraceTheme.Colors.profileSecondaryText)
                }
                .font(TraceTheme.Fonts.action)
                .padding(.horizontal, TraceTheme.Spacing.medium)
                .frame(height: 54)
                .background(
                    TraceTheme.Colors.surface,
                    in: RoundedRectangle(cornerRadius: TraceTheme.Radius.field)
                )
                .overlay {
                    RoundedRectangle(cornerRadius: TraceTheme.Radius.field)
                        .stroke(TraceTheme.Colors.divider, lineWidth: 1)
                }

                TextField("Phone number", text: $phoneNumber)
                    .keyboardType(.phonePad)
                    .textContentType(.telephoneNumber)
                    .focused($isPhoneFocused)
                    .foregroundStyle(TraceTheme.Colors.primaryText)
                    .padding(.horizontal, TraceTheme.Spacing.medium)
                    .frame(maxWidth: .infinity)
                    .frame(height: 54)
                    .background(
                        TraceTheme.Colors.surface,
                        in: RoundedRectangle(cornerRadius: TraceTheme.Radius.field)
                    )
                    .overlay {
                        RoundedRectangle(cornerRadius: TraceTheme.Radius.field)
                            .stroke(TraceTheme.Colors.divider, lineWidth: 1)
                    }
            }

            Button("Next", action: onNext)
                .buttonStyle(TraceFormButtonStyle())
                .disabled(!canContinue)
                .opacity(canContinue ? 1 : 0.5)

            Text("We'll text you a code to verify your number. Message and data rates may apply.")
                .font(.system(size: 12))
                .foregroundStyle(TraceTheme.Colors.profileSecondaryText)
                .lineSpacing(2)

            Spacer(minLength: 0)
        }
        .padding(TraceTheme.Spacing.large)
        .onAppear { isPhoneFocused = true }
    }
}

#Preview("Log In") {
    NavigationStack {
        LogInView(onComplete: {})
    }
}
