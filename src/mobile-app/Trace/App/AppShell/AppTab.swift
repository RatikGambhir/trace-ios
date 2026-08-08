import SwiftUI

enum AppTab: String, CaseIterable, Hashable, Identifiable {
    case places
    case discover
    case people
    case profile

    var id: Self { self }

    var title: String {
        switch self {
        case .places: "Places"
        case .discover: "Discover"
        case .people: "People"
        case .profile: "Profile"
        }
    }

    var symbol: String {
        switch self {
        case .places: "map"
        case .discover: "safari"
        case .people: "person.2"
        case .profile: "person.crop.circle"
        }
    }

    var selectedSymbol: String {
        switch self {
        case .places: "map.fill"
        case .discover: "safari.fill"
        case .people: "person.2.fill"
        case .profile: "person.crop.circle.fill"
        }
    }
}
