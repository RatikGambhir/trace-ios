import SwiftUI

struct AppShellView: View {
    @State private var selectedTab: AppTab
    @State private var placesPath = NavigationPath()
    @State private var discoverPath = NavigationPath()
    @State private var peoplePath = NavigationPath()
    @State private var profilePath = NavigationPath()
    @State private var profileViewModel: ProfileViewModel

    let onShowLogin: () -> Void
    let onSignOut: () -> Void

    init(
        onShowLogin: @escaping () -> Void,
        onSignOut: @escaping () -> Void
    ) {
        self.onShowLogin = onShowLogin
        self.onSignOut = onSignOut
#if DEBUG
        _selectedTab = State(
            initialValue: ProcessInfo.processInfo.arguments.contains("-showProfile") ? .profile : .places
        )
#else
        _selectedTab = State(initialValue: .places)
#endif
        let store = InMemoryTraceStore()
        _profileViewModel = State(
            initialValue: ProfileViewModel(
                profileService: store,
                placesService: store,
                collectionsService: store
            )
        )
    }

    var body: some View {
        ZStack {
            tabLayer(.places) {
                NavigationStack(path: $placesPath) {
                    PlacesRootView()
                }
            }

            tabLayer(.discover) {
                NavigationStack(path: $discoverPath) {
                    DiscoverRootView()
                }
            }

            tabLayer(.people) {
                NavigationStack(path: $peoplePath) {
                    PeopleRootView()
                }
            }

            tabLayer(.profile) {
                NavigationStack(path: $profilePath) {
                    ProfileView(viewModel: profileViewModel) { route in
                        profilePath.append(route)
                    } onSignOut: {
                        onSignOut()
                    } onShowLogin: {
                        onShowLogin()
                    }
                    .navigationDestination(for: ProfileRoute.self) { route in
                        profileDestination(route)
                    }
                }
            }
        }
        .background(TraceTheme.Colors.background)
        .safeAreaInset(edge: .bottom, spacing: 0) {
            AppTabBar(
                selectedTab: selectedTab,
                profile: profileViewModel.profile,
                onSelect: select
            )
        }
        .tint(TraceTheme.Colors.accent)
        .task {
            await profileViewModel.load()
        }
    }

    @ViewBuilder
    private func tabLayer<Content: View>(
        _ tab: AppTab,
        @ViewBuilder content: () -> Content
    ) -> some View {
        content()
            .opacity(selectedTab == tab ? 1 : 0)
            .allowsHitTesting(selectedTab == tab)
            .accessibilityHidden(selectedTab != tab)
    }

    private func select(_ tab: AppTab) {
        if selectedTab == tab {
            resetPath(for: tab)
        } else {
            selectedTab = tab
        }
    }

    private func resetPath(for tab: AppTab) {
        switch tab {
        case .places: placesPath = NavigationPath()
        case .discover: discoverPath = NavigationPath()
        case .people: peoplePath = NavigationPath()
        case .profile: profilePath = NavigationPath()
        }
    }

    @ViewBuilder
    private func profileDestination(_ route: ProfileRoute) -> some View {
        switch route {
        case .settings:
            SettingsView(onSignOut: onSignOut)
        case .editProfile:
            if let profile = profileViewModel.profile {
                EditProfileView(profile: profile) { updatedProfile in
                    await profileViewModel.updateProfile(updatedProfile)
                }
            }
        case .invites:
            InvitesView(inviteCount: profileViewModel.profile?.inviteCount ?? 0)
        case .savedPlace(let id):
            if let place = loadedContent?.savedPlaces.first(where: { $0.id == id }) {
                PlaceDetailView(place: place)
            } else {
                MissingContentView(title: "Place unavailable", symbol: "mappin.slash")
            }
        case .collection(let id):
            if let collection = loadedContent?.collections.first(where: { $0.id == id }) {
                CollectionDetailView(collection: collection)
            } else {
                MissingContentView(title: "Collection unavailable", symbol: "folder.badge.questionmark")
            }
        case .createCollection:
            CreateCollectionView { name, description, visibility in
                await profileViewModel.createCollection(
                    name: name,
                    description: description,
                    visibility: visibility
                )
            }
        case .map:
            ProfileMapView(places: loadedContent?.savedPlaces ?? [])
        case .addPlace:
            AddPlaceView()
        }
    }

    private var loadedContent: ProfileContent? {
        guard case .loaded(let content) = profileViewModel.state else { return nil }
        return content
    }
}

#Preview {
    AppShellView(onShowLogin: {}, onSignOut: {})
}
