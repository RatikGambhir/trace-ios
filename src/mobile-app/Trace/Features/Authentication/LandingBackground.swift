import SwiftUI

struct LandingBackground: View {
    var body: some View {
        Image(.landingBackground)
            .resizable()
            .scaledToFill()
            .containerRelativeFrame([.horizontal, .vertical])
            .clipped()
            .ignoresSafeArea()
    }
}
