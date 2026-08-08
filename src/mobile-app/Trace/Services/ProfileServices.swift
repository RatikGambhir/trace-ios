import Foundation

protocol ProfileService: Sendable {
    func currentProfile() async throws -> UserProfile
    func updateProfile(_ profile: UserProfile) async throws
}

protocol PlacesService: Sendable {
    func savedPlaces() async throws -> [Place]
    func removeSavedPlace(id: UUID) async throws
}

protocol CollectionsService: Sendable {
    func collections() async throws -> [PlaceCollection]
    func createCollection(_ collection: PlaceCollection) async throws
}

actor InMemoryTraceStore: ProfileService, PlacesService, CollectionsService {
    private var content: ProfileContent

    init(content: ProfileContent = .preview) {
        self.content = content
    }

    func currentProfile() async throws -> UserProfile {
        try await simulatedLatency()
        return content.profile
    }

    func updateProfile(_ profile: UserProfile) async throws {
        try await simulatedLatency()
        content.profile = profile
    }

    func savedPlaces() async throws -> [Place] {
        try await simulatedLatency()
        return content.savedPlaces
    }

    func removeSavedPlace(id: UUID) async throws {
        try await simulatedLatency()
        content.savedPlaces.removeAll { $0.id == id }
        content.profile.savedPlaceCount = content.savedPlaces.count
    }

    func collections() async throws -> [PlaceCollection] {
        try await simulatedLatency()
        return content.collections
    }

    func createCollection(_ collection: PlaceCollection) async throws {
        try await simulatedLatency()
        content.collections.append(collection)
    }

    private func simulatedLatency() async throws {
        try await Task.sleep(nanoseconds: 180_000_000)
    }
}
