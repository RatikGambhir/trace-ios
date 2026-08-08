import SwiftUI

struct PeopleRootView: View {
    var body: some View {
        ContentUnavailableView {
            Label("People", systemImage: "person.2")
                .font(.system(size: 28, weight: .medium, design: .serif))
        } description: {
            Text("Find friends and trade the places you keep returning to.")
        } actions: {
            Button("Invite a friend") { }
                .buttonStyle(.bordered)
                .tint(TraceTheme.Colors.accent)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(TraceTheme.Colors.background)
        .navigationBarHidden(true)
    }
}
