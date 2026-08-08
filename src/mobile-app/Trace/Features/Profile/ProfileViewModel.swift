import Foundation
import Observation

@MainActor
@Observable
final class ProfileViewModel {
    private(set) var state: LoadState<ProfileContent> = .idle
    private(set) var isRefreshing = false

    private let profileService: any ProfileService
    private let placesService: any PlacesService
    private let collectionsService: any CollectionsService

    init(
        profileService: any ProfileService,
        placesService: any PlacesService,
        collectionsService: any CollectionsService
    ) {
        self.profileService = profileService
        self.placesService = placesService
        self.collectionsService = collectionsService
    }

    var profile: UserProfile? {
        guard case .loaded(let content) = state else { return nil }
        return content.profile
    }

    func load() async {
        guard case .idle = state else { return }
        state = .loading
        await fetch()
    }

    func refresh() async {
        guard !isRefreshing else { return }
        isRefreshing = true
        defer { isRefreshing = false }
        await fetch()
    }

    func updateProfile(_ profile: UserProfile) async {
        do {
            try await profileService.updateProfile(profile)
            guard case .loaded(var content) = state else { return }
            content.profile = profile
            state = .loaded(content)
        } catch {
            state = .failed(error)
        }
    }

    func removeSavedPlace(id: UUID) async {
        do {
            try await placesService.removeSavedPlace(id: id)
            guard case .loaded(var content) = state else { return }
            content.savedPlaces.removeAll { $0.id == id }
            content.profile.savedPlaceCount = content.savedPlaces.count
            state = .loaded(content)
        } catch {
            state = .failed(error)
        }
    }

    func createCollection(
        name: String,
        description: String?,
        visibility: CollectionVisibility
    ) async {
        let collection = PlaceCollection(
            id: UUID(),
            name: name,
            description: description,
            coverImageURL: nil,
            placeIDs: [],
            visibility: visibility
        )

        do {
            try await collectionsService.createCollection(collection)
            guard case .loaded(var content) = state else { return }
            content.collections.append(collection)
            state = .loaded(content)
        } catch {
            state = .failed(error)
        }
    }

    private func fetch() async {
        do {
            async let profile = profileService.currentProfile()
            async let savedPlaces = placesService.savedPlaces()
            async let collections = collectionsService.collections()
            let content = try await ProfileContent(
                profile: profile,
                savedPlaces: savedPlaces,
                collections: collections
            )
            state = .loaded(content)
        } catch is CancellationError {
            if case .loading = state {
                state = .idle
            }
        } catch {
            state = .failed(error)
        }
    }
}

extension ProfileViewModel {
    static func preview(state: LoadState<ProfileContent> = .loaded(.preview)) -> ProfileViewModel {
        let store = InMemoryTraceStore()
        let viewModel = ProfileViewModel(
            profileService: store,
            placesService: store,
            collectionsService: store
        )
        viewModel.state = state
        return viewModel
    }
}
