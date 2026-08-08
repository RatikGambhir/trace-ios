import SwiftUI

struct LogInView: View {
    @State private var email = ""
    @State private var password = ""

    let onComplete: () -> Void

    var body: some View {
        AuthenticationFormLayout(
            title: "Welcome Back",
            subtitle: "Log in to continue your journey with Trace."
        ) {
            TraceTextField(
                title: "Email",
                text: $email,
                contentType: .emailAddress,
                keyboardType: .emailAddress,
                textInputAutocapitalization: .never
            )

            TraceSecureField(title: "Password", text: $password, contentType: .password)

            Button("Log In", action: onComplete)
                .buttonStyle(TraceFormButtonStyle())
                .padding(.top, TraceTheme.Spacing.small)
        }
    }
}
