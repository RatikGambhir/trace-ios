import SwiftUI

struct CreateAccountView: View {
    @State private var name = ""
    @State private var email = ""
    @State private var password = ""

    let onComplete: () -> Void

    var body: some View {
        AuthenticationFormLayout(
            title: "Create an Account",
            subtitle: "Start tracing the places and moments that matter to you."
        ) {
            TraceTextField(title: "Full Name", text: $name, contentType: .name)

            TraceTextField(
                title: "Email",
                text: $email,
                contentType: .emailAddress,
                keyboardType: .emailAddress,
                textInputAutocapitalization: .never
            )

            TraceSecureField(title: "Password", text: $password, contentType: .newPassword)

            Button("Create Account", action: onComplete)
                .buttonStyle(TraceFormButtonStyle())
                .padding(.top, TraceTheme.Spacing.small)
        }
    }
}
