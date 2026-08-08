import SwiftUI

struct AuthenticationFormLayout<Fields: View>: View {
    let title: String
    let subtitle: String
    @ViewBuilder let fields: () -> Fields

    init(
        title: String,
        subtitle: String,
        @ViewBuilder fields: @escaping () -> Fields
    ) {
        self.title = title
        self.subtitle = subtitle
        self.fields = fields
    }

    var body: some View {
        ZStack {
            Color.black.opacity(0.55)
                .ignoresSafeArea()

            ScrollView {
                VStack(spacing: 0) {
                    TraceBrandMark(compact: true)
                        .padding(.top, TraceTheme.Spacing.xLarge)

                    Spacer(minLength: 88)

                    TraceGlassPanel {
                        VStack(alignment: .leading, spacing: 28) {
                            VStack(alignment: .leading, spacing: 10) {
                                Text(title)
                                    .font(TraceTheme.Fonts.formTitle)
                                    .tracking(-0.4)
                                    .foregroundStyle(.white)

                                Text(subtitle)
                                    .font(TraceTheme.Fonts.body)
                                    .foregroundStyle(TraceTheme.Colors.secondaryText)
                                    .lineSpacing(3)
                            }

                            VStack(spacing: 17) {
                                fields()
                            }
                        }
                    }
                    .padding(.horizontal, TraceTheme.Spacing.medium)
                    .padding(.bottom, 10)
                }
                .frame(maxWidth: .infinity)
                .containerRelativeFrame(.vertical, alignment: .bottom)
            }
            .scrollDismissesKeyboard(.interactively)
        }
        .background {
            LandingBackground()
                .blur(radius: 18)
                .scaleEffect(1.08)
        }
        .navigationBarTitleDisplayMode(.inline)
        .toolbarBackground(.hidden, for: .navigationBar)
    }
}
