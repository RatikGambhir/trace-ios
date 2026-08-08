import SwiftUI

struct ProfileView: View {
    let viewModel: ProfileViewModel
    let onRoute: (ProfileRoute) -> Void
    let onSignOut: () -> Void
    let onShowLogin: () -> Void

    @State private var placePendingRemoval: Place?
    @State private var showSignOutConfirmation = false

    var body: some View {
        Group {
            switch viewModel.state {
            case .idle, .loading:
                ProfileLoadingView()
            case .loaded(let content):
                profileContent(content)
            case .failed:
                ProfileErrorView {
                    await viewModel.refresh()
                }
            }
        }
        .background(TraceTheme.Colors.background)
        .toolbar(.hidden, for: .navigationBar)
        .confirmationDialog(
            "Remove saved place?",
            isPresented: Binding(
                get: { placePendingRemoval != nil },
                set: { if !$0 { placePendingRemoval = nil } }
            ),
            titleVisibility: .visible
        ) {
            Button("Remove", role: .destructive) {
                guard let place = placePendingRemoval else { return }
                Task { await viewModel.removeSavedPlace(id: place.id) }
                placePendingRemoval = nil
            }
            Button("Cancel", role: .cancel) { placePendingRemoval = nil }
        } message: {
            if let place = placePendingRemoval {
                Text("\(place.name) will be removed from your saved places.")
            }
        }
        .confirmationDialog(
            "Sign out of Trace?",
            isPresented: $showSignOutConfirmation,
            titleVisibility: .visible
        ) {
            Button("Sign Out", role: .destructive, action: onSignOut)
            Button("Cancel", role: .cancel) { }
        }
    }

    private func profileContent(_ content: ProfileContent) -> some View {
        ScrollView {
            LazyVStack(alignment: .leading, spacing: 0) {
                ProfileHeader(
                    profile: content.profile,
                    onInvites: { onRoute(.invites) },
                    onEditProfile: { onRoute(.editProfile) },
                    onSettings: { onRoute(.settings) },
                    onSignOut: { showSignOutConfirmation = true }
                )

                ProfileIdentity(
                    profile: content.profile,
                    onEditBio: { onRoute(.editProfile) }
                )
                .padding(.horizontal, TraceTheme.Spacing.medium)

                ProfileActionButtons(
                    onAdd: { onRoute(.addPlace) },
                    onMap: { onRoute(.map) },
                    onShowLogin: onShowLogin
                )
                .padding(.horizontal, TraceTheme.Spacing.medium)
                .padding(.top, TraceTheme.Spacing.large)
                .padding(.bottom, TraceTheme.Spacing.xLarge)

                ProfileSectionDivider()

                SavedPlacesSection(
                    places: content.savedPlaces,
                    onOpen: { onRoute(.savedPlace($0.id)) },
                    onMove: { _ in onRoute(.createCollection) },
                    onMap: { _ in onRoute(.map) },
                    onRemove: { placePendingRemoval = $0 }
                )

                ProfileSectionDivider()

                CollectionsSection(
                    collections: content.collections,
                    onCreate: { onRoute(.createCollection) },
                    onOpen: { onRoute(.collection($0.id)) }
                )
                .padding(.bottom, TraceTheme.Spacing.xLarge)
            }
            .frame(maxWidth: 760)
            .frame(maxWidth: .infinity)
        }
        .background(TraceTheme.Colors.background)
        .refreshable {
            await viewModel.refresh()
        }
    }
}

private struct ProfileIdentity: View {
    let profile: UserProfile
    let onEditBio: () -> Void

    var body: some View {
        VStack(spacing: TraceTheme.Spacing.small) {
            Text(profile.displayName)
                .font(TraceTheme.Fonts.profileName)
                .foregroundStyle(TraceTheme.Colors.primaryText)
                .multilineTextAlignment(.center)
                .lineLimit(2)
                .minimumScaleFactor(0.78)

            Button(action: onEditBio) {
                Text(profile.bio.nilIfBlank ?? "Tap to add bio")
                    .font(TraceTheme.Fonts.profileBody)
                    .foregroundStyle(TraceTheme.Colors.profileSecondaryText)
                    .multilineTextAlignment(.center)
            }
            .buttonStyle(.plain)
            .frame(minHeight: 44)

            HStack(spacing: TraceTheme.Spacing.xSmall) {
                if let location = profile.locationSummary {
                    Text(location)
                    Text("·")
                        .accessibilityHidden(true)
                }
                Text(profile.savedPlaceSummary)
            }
            .font(TraceTheme.Fonts.profileBody)
            .foregroundStyle(TraceTheme.Colors.primaryText.opacity(0.78))
            .accessibilityElement(children: .combine)
        }
        .frame(maxWidth: .infinity)
    }
}

private struct SavedPlacesSection: View {
    let places: [Place]
    let onOpen: (Place) -> Void
    let onMove: (Place) -> Void
    let onMap: (Place) -> Void
    let onRemove: (Place) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: TraceTheme.Spacing.small) {
            Text("Saved places")
                .font(TraceTheme.Fonts.sectionTitle)
                .foregroundStyle(TraceTheme.Colors.primaryText)

            if places.isEmpty {
                ContentUnavailableView(
                    "No saved places yet",
                    systemImage: "bookmark",
                    description: Text("Places you save will live here.")
                )
                .frame(maxWidth: .infinity)
                .padding(.vertical, TraceTheme.Spacing.large)
            } else {
                LazyVStack(spacing: 0) {
                    ForEach(places) { place in
                        SavedPlaceRow(
                            place: place,
                            onOpen: { onOpen(place) },
                            onMove: { onMove(place) },
                            onMap: { onMap(place) },
                            onRemove: { onRemove(place) }
                        )

                        if place.id != places.last?.id {
                            Divider()
                                .overlay(TraceTheme.Colors.divider)
                                .padding(.leading, 96)
                        }
                    }
                }
            }
        }
        .padding(TraceTheme.Spacing.medium)
    }
}

private struct CollectionsSection: View {
    let collections: [PlaceCollection]
    let onCreate: () -> Void
    let onOpen: (PlaceCollection) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: TraceTheme.Spacing.medium) {
            HStack {
                Text("Collections")
                    .font(TraceTheme.Fonts.sectionTitle)
                    .foregroundStyle(TraceTheme.Colors.primaryText)

                Spacer()

                if !collections.isEmpty {
                    Button("New", action: onCreate)
                        .font(.system(size: 15, weight: .semibold))
                }
            }

            if collections.isEmpty {
                CreateCollectionCard(action: onCreate)
            } else {
                LazyVStack(spacing: TraceTheme.Spacing.small) {
                    ForEach(collections) { collection in
                        CollectionCard(collection: collection) {
                            onOpen(collection)
                        }
                    }
                }
            }
        }
        .padding(TraceTheme.Spacing.medium)
    }
}

private struct ProfileSectionDivider: View {
    var body: some View {
        Divider()
            .overlay(TraceTheme.Colors.divider)
    }
}

private struct ProfileLoadingView: View {
    var body: some View {
        ScrollView {
            VStack(spacing: TraceTheme.Spacing.large) {
                Rectangle()
                    .fill(TraceTheme.Colors.divider)
                    .frame(height: 272)

                Circle()
                    .fill(TraceTheme.Colors.surface)
                    .frame(width: 116, height: 116)
                    .overlay { ProgressView().tint(TraceTheme.Colors.accent) }
                    .offset(y: -82)
                    .padding(.bottom, -82)

                VStack(spacing: TraceTheme.Spacing.small) {
                    RoundedRectangle(cornerRadius: 8)
                        .fill(TraceTheme.Colors.divider)
                        .frame(width: 190, height: 28)
                    RoundedRectangle(cornerRadius: 6)
                        .fill(TraceTheme.Colors.divider)
                        .frame(width: 132, height: 16)
                }

                RoundedRectangle(cornerRadius: TraceTheme.Radius.field)
                    .fill(TraceTheme.Colors.surface)
                    .frame(height: 56)
                    .padding(.horizontal, TraceTheme.Spacing.medium)
            }
            .redacted(reason: .placeholder)
        }
        .ignoresSafeArea(edges: .top)
        .accessibilityLabel("Loading profile")
    }
}

private struct ProfileErrorView: View {
    let retry: () async -> Void

    var body: some View {
        ContentUnavailableView {
            Label("Profile unavailable", systemImage: "wifi.exclamationmark")
        } description: {
            Text("We couldn't load your places. Check your connection and try again.")
        } actions: {
            Button("Try Again") {
                Task { await retry() }
            }
            .buttonStyle(.borderedProminent)
            .tint(TraceTheme.Colors.accent)
        }
    }
}

private extension Optional where Wrapped == String {
    var nilIfBlank: String? {
        guard let value = self else { return nil }
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? nil : trimmed
    }
}

#Preview("Profile") {
    NavigationStack {
        ProfileView(
            viewModel: .preview(),
            onRoute: { _ in },
            onSignOut: { },
            onShowLogin: { }
        )
    }
}

#Preview("Profile — Loading") {
    ProfileView(
        viewModel: .preview(state: .loading),
        onRoute: { _ in },
        onSignOut: { },
        onShowLogin: { }
    )
}

#Preview("Profile — Empty") {
    var content = ProfileContent.preview
    content.savedPlaces = []
    content.profile.savedPlaceCount = 0
    return ProfileView(
        viewModel: .preview(state: .loaded(content)),
        onRoute: { _ in },
        onSignOut: { },
        onShowLogin: { }
    )
}
