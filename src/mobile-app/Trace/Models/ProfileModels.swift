import Foundation

struct UserProfile: Identifiable, Codable, Hashable, Sendable {
    let id: UUID
    var displayName: String
    var username: String?
    var bio: String?
    var city: String?
    var state: String?
    var country: String?
    var avatarURL: URL?
    var coverImageURL: URL?
    var savedPlaceCount: Int
    var inviteCount: Int

    var initials: String {
        let parts = displayName.split(separator: " ").prefix(2)
        let value = parts.compactMap(\.first).map(String.init).joined()
        return value.isEmpty ? "T" : value.uppercased()
    }

    var locationSummary: String? {
        [city, state].compactMap { $0 }.filter { !$0.isEmpty }.joined(separator: ", ").nilIfEmpty
    }

    var savedPlaceSummary: String {
        "\(savedPlaceCount) \(savedPlaceCount == 1 ? "place" : "places")"
    }
}

struct Place: Identifiable, Codable, Hashable, Sendable {
    let id: UUID
    let name: String
    let imageURL: URL?
    let neighborhood: String?
    let city: String
    let state: String?
    let country: String?
    let category: String?
    let latitude: Double
    let longitude: Double

    var locationSummary: String {
        let cityRegion = [city, state].compactMap { $0 }.filter { !$0.isEmpty }.joined(separator: ", ")
        return [neighborhood, cityRegion].compactMap { $0 }.filter { !$0.isEmpty }.joined(separator: " • ")
    }
}

enum CollectionVisibility: String, Codable, Hashable, Sendable, CaseIterable {
    case privateOnly
    case shared
    case publicProfile
}

struct PlaceCollection: Identifiable, Codable, Hashable, Sendable {
    let id: UUID
    var name: String
    var description: String?
    var coverImageURL: URL?
    var placeIDs: [UUID]
    var visibility: CollectionVisibility
}

struct ProfileContent: Hashable, Sendable {
    var profile: UserProfile
    var savedPlaces: [Place]
    var collections: [PlaceCollection]
}

enum LoadState<Value> {
    case idle
    case loading
    case loaded(Value)
    case failed(any Error)
}

private extension String {
    var nilIfEmpty: String? { isEmpty ? nil : self }
}

extension ProfileContent {
    static let preview = ProfileContent(
        profile: UserProfile(
            id: UUID(uuidString: "B749551B-E316-4B29-B771-43B846D9E5FA")!,
            displayName: "Ratik Gambhir",
            username: "ratik",
            bio: nil,
            city: "Chicago",
            state: "IL",
            country: "United States",
            avatarURL: nil,
            coverImageURL: nil,
            savedPlaceCount: 1,
            inviteCount: 1
        ),
        savedPlaces: [
            Place(
                id: UUID(uuidString: "39B41C54-689D-46D8-B464-ED252A377E65")!,
                name: "Momotaro",
                imageURL: nil,
                neighborhood: "West Loop",
                city: "Chicago",
                state: "Illinois",
                country: "United States",
                category: "Japanese Restaurant",
                latitude: 41.8844,
                longitude: -87.6475
            )
        ],
        collections: []
    )
}
