import SwiftUI

struct ContentView: View {
    @State private var count = 0

    var body: some View {
        NavigationStack {
            VStack(spacing: 24) {
                Image(systemName: "waveform.path.ecg")
                    .font(.system(size: 64))
                    .foregroundStyle(.tint)

                Text("Hello, Trace!")
                    .font(.largeTitle.bold())

                Text("You've tapped \(count) time\(count == 1 ? "" : "s").")
                    .font(.body)
                    .foregroundStyle(.secondary)

                Button("Tap me") {
                    count += 1
                }
                .buttonStyle(.borderedProminent)
            }
            .padding()
            .navigationTitle("Trace")
        }
    }
}

#Preview {
    ContentView()
}
