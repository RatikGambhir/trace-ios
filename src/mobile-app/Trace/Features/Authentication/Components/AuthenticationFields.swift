import SwiftUI

struct TraceTextField: View {
    let title: String
    @Binding var text: String
    let contentType: UITextContentType?
    var keyboardType: UIKeyboardType = .default
    var textInputAutocapitalization: TextInputAutocapitalization? = .sentences

    var body: some View {
        TraceFieldContainer(title: title) {
            TextField(title, text: $text)
                .textContentType(contentType)
                .keyboardType(keyboardType)
                .textInputAutocapitalization(textInputAutocapitalization)
                .autocorrectionDisabled(keyboardType == .emailAddress)
        }
    }
}

struct TraceSecureField: View {
    let title: String
    @Binding var text: String
    let contentType: UITextContentType

    var body: some View {
        TraceFieldContainer(title: title) {
            SecureField(title, text: $text)
                .textContentType(contentType)
        }
    }
}

private struct TraceFieldContainer<Field: View>: View {
    let title: String
    @ViewBuilder let field: () -> Field

    var body: some View {
        VStack(alignment: .leading, spacing: TraceTheme.Spacing.small) {
            Text(title.uppercased())
                .font(TraceTheme.Fonts.label)
                .tracking(1.5)
                .foregroundStyle(TraceTheme.Colors.tertiaryText)

            field()
                .foregroundStyle(.white)
                .padding(.horizontal, TraceTheme.Spacing.medium)
                .frame(height: 54)
                .background(
                    TraceTheme.Colors.fieldFill,
                    in: RoundedRectangle(cornerRadius: TraceTheme.Radius.field)
                )
                .overlay {
                    RoundedRectangle(cornerRadius: TraceTheme.Radius.field)
                        .stroke(TraceTheme.Colors.fieldBorder, lineWidth: 1)
                }
        }
    }
}
