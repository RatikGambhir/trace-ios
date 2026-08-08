import MapKit
import SwiftUI

struct PlacesRootView: View {
    private let chicago = CLLocationCoordinate2D(latitude: 41.8781, longitude: -87.6298)

    var body: some View {
        Map {
            Marker("Chicago", systemImage: "mappin.and.ellipse", coordinate: chicago)
                .tint(TraceTheme.Colors.accent)
        }
        .mapStyle(.standard(elevation: .realistic))
        .overlay(alignment: .topLeading) {
            VStack(alignment: .leading, spacing: TraceTheme.Spacing.xSmall) {
                Text("Places")
                    .font(.system(size: 32, weight: .medium, design: .serif))
                Text("Your map of places worth remembering")
                    .font(TraceTheme.Fonts.profileBody)
                    .foregroundStyle(TraceTheme.Colors.profileSecondaryText)
            }
            .padding(TraceTheme.Spacing.medium)
            .background(.regularMaterial, in: RoundedRectangle(cornerRadius: TraceTheme.Radius.large))
            .padding(TraceTheme.Spacing.medium)
        }
        .navigationBarHidden(true)
    }
}
