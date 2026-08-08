import MapKit
import SwiftUI

struct SettingsView: View {
    let onSignOut: () -> Void
    @State private var showSignOutConfirmation = false

    var body: some View {
        List {
            Section("Account") {
                SettingsLink(title: "Edit Profile", symbol: "person.crop.circle")
                SettingsLink(title: "Email", symbol: "envelope")
                SettingsLink(title: "Phone", symbol: "phone")
                SettingsLink(title: "Password", symbol: "key")
            }

            Section("Preferences") {
                SettingsLink(title: "Notifications", symbol: "bell")
                SettingsLink(title: "Location", symbol: "location")
                NavigationLink {
                    AppearanceSettingsView()
                } label: {
                    Label("Appearance", systemImage: "circle.lefthalf.filled")
                }
            }

            Section("Privacy") {
                SettingsLink(title: "Profile Visibility", symbol: "eye")
                SettingsLink(title: "Blocked Users", symbol: "person.crop.circle.badge.xmark")
                SettingsLink(title: "Location Sharing", symbol: "location.circle")
            }

            Section("App") {
                SettingsLink(title: "Help & Support", symbol: "questionmark.circle")
                SettingsLink(title: "About", symbol: "info.circle")
                SettingsLink(title: "Terms", symbol: "doc.text")
                SettingsLink(title: "Privacy Policy", symbol: "hand.raised")
            }

            Section {
                Button("Sign Out", role: .destructive) {
                    showSignOutConfirmation = true
                }
            }
        }
        .navigationTitle("Settings")
        .confirmationDialog(
            "Sign out of Trace?",
            isPresented: $showSignOutConfirmation,
            titleVisibility: .visible
        ) {
            Button("Sign Out", role: .destructive, action: onSignOut)
            Button("Cancel", role: .cancel) { }
        }
    }
}

private struct SettingsLink: View {
    let title: String
    let symbol: String

    var body: some View {
        NavigationLink {
            SettingsDetailView(title: title, symbol: symbol)
        } label: {
            Label(title, systemImage: symbol)
        }
    }
}

private struct SettingsDetailView: View {
    let title: String
    let symbol: String

    var body: some View {
        ContentUnavailableView(
            title,
            systemImage: symbol,
            description: Text("This setting is ready for its production service integration.")
        )
        .navigationTitle(title)
        .navigationBarTitleDisplayMode(.inline)
    }
}

struct EditProfileView: View {
    let profile: UserProfile
    let onSave: (UserProfile) async -> Void

    @Environment(\.dismiss) private var dismiss
    @State private var displayName: String
    @State private var bio: String
    @State private var city: String
    @State private var state: String
    @State private var isSaving = false

    init(profile: UserProfile, onSave: @escaping (UserProfile) async -> Void) {
        self.profile = profile
        self.onSave = onSave
        _displayName = State(initialValue: profile.displayName)
        _bio = State(initialValue: profile.bio ?? "")
        _city = State(initialValue: profile.city ?? "")
        _state = State(initialValue: profile.state ?? "")
    }

    var body: some View {
        Form {
            Section("Profile") {
                TextField("Name", text: $displayName)
                    .textContentType(.name)
                TextField("Bio", text: $bio, axis: .vertical)
                    .lineLimit(3...6)
            }

            Section("Location") {
                TextField("City", text: $city)
                    .textContentType(.addressCity)
                TextField("State", text: $state)
                    .textContentType(.addressState)
            }
        }
        .navigationTitle("Edit Profile")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .confirmationAction) {
                Button(isSaving ? "Saving…" : "Save") {
                    Task {
                        isSaving = true
                        var updated = profile
                        updated.displayName = displayName.trimmingCharacters(in: .whitespacesAndNewlines)
                        updated.bio = bio.trimmedOrNil
                        updated.city = city.trimmedOrNil
                        updated.state = state.trimmedOrNil
                        await onSave(updated)
                        isSaving = false
                        dismiss()
                    }
                }
                .disabled(displayName.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || isSaving)
            }
        }
    }
}

struct InvitesView: View {
    let inviteCount: Int

    var body: some View {
        Group {
            if inviteCount == 0 {
                ContentUnavailableView(
                    "No new invites",
                    systemImage: "person.2",
                    description: Text("New invitations will appear here.")
                )
            } else {
                List {
                    ForEach(0..<inviteCount, id: \.self) { index in
                        HStack(spacing: TraceTheme.Spacing.medium) {
                            Image(systemName: "person.crop.circle.fill")
                                .font(.system(size: 42))
                                .foregroundStyle(TraceTheme.Colors.accent)

                            VStack(alignment: .leading, spacing: TraceTheme.Spacing.xSmall) {
                                Text(index == 0 ? "A fellow traveler" : "Traveler \(index + 1)")
                                    .font(.headline)
                                Text("Invited you to share places")
                                    .font(.subheadline)
                                    .foregroundStyle(.secondary)
                            }

                            Spacer()
                            Button("Accept") { }
                                .buttonStyle(.bordered)
                        }
                        .padding(.vertical, TraceTheme.Spacing.xSmall)
                    }
                }
            }
        }
        .navigationTitle("Invites")
    }
}

struct PlaceDetailView: View {
    let place: Place

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: TraceTheme.Spacing.large) {
                Rectangle()
                    .fill(TraceTheme.Colors.surface)
                    .frame(height: 240)
                    .overlay {
                        Image(systemName: "fork.knife")
                            .font(.system(size: 42, weight: .light))
                            .foregroundStyle(TraceTheme.Colors.accent)
                    }
                    .clipShape(RoundedRectangle(cornerRadius: TraceTheme.Radius.large, style: .continuous))

                VStack(alignment: .leading, spacing: TraceTheme.Spacing.small) {
                    Text(place.name)
                        .font(.system(size: 32, weight: .medium, design: .serif))
                    Text(place.locationSummary)
                        .font(TraceTheme.Fonts.profileBody)
                        .foregroundStyle(TraceTheme.Colors.profileSecondaryText)
                    if let category = place.category {
                        Label(category, systemImage: "fork.knife")
                            .font(.subheadline)
                            .padding(.top, TraceTheme.Spacing.small)
                    }
                }
            }
            .padding(TraceTheme.Spacing.medium)
        }
        .background(TraceTheme.Colors.background)
        .navigationBarTitleDisplayMode(.inline)
    }
}

struct ProfileMapView: View {
    let places: [Place]
    @State private var cameraPosition: MapCameraPosition = .automatic

    var body: some View {
        Group {
            if places.isEmpty {
                ContentUnavailableView(
                    "No places to map",
                    systemImage: "map",
                    description: Text("Save a place and it will appear here.")
                )
            } else {
                Map(position: $cameraPosition) {
                    ForEach(places) { place in
                        Marker(
                            place.name,
                            coordinate: CLLocationCoordinate2D(
                                latitude: place.latitude,
                                longitude: place.longitude
                            )
                        )
                        .tint(TraceTheme.Colors.accent)
                    }
                }
            }
        }
        .navigationTitle("Your Map")
        .navigationBarTitleDisplayMode(.inline)
    }
}

struct AddPlaceView: View {
    var body: some View {
        List {
            Section {
                AddPlaceLink(title: "Search for a place", symbol: "magnifyingglass")
                AddPlaceLink(title: "Add custom place", symbol: "mappin.and.ellipse")
                AddPlaceLink(title: "Import from link", symbol: "link")
            } footer: {
                Text("Search and saving will connect to the places service in the production data layer.")
            }
        }
        .navigationTitle("Add a Place")
    }
}

private struct AddPlaceLink: View {
    let title: String
    let symbol: String

    var body: some View {
        NavigationLink {
            ContentUnavailableView(
                title,
                systemImage: symbol,
                description: Text("This entry point is ready for its place-search integration.")
            )
            .navigationTitle(title)
            .navigationBarTitleDisplayMode(.inline)
        } label: {
            Label(title, systemImage: symbol)
        }
    }
}

struct CreateCollectionView: View {
    let onCreate: (String, String?, CollectionVisibility) async -> Void

    @Environment(\.dismiss) private var dismiss
    @State private var name = ""
    @State private var description = ""
    @State private var visibility: CollectionVisibility = .privateOnly
    @State private var isSaving = false

    var body: some View {
        Form {
            Section("Collection") {
                TextField("Name", text: $name)
                TextField("Description", text: $description, axis: .vertical)
                    .lineLimit(2...5)
            }

            Section("Visibility") {
                Picker("Who can see this?", selection: $visibility) {
                    Text("Only me").tag(CollectionVisibility.privateOnly)
                    Text("People with access").tag(CollectionVisibility.shared)
                    Text("Public profile").tag(CollectionVisibility.publicProfile)
                }
                .pickerStyle(.inline)
            }
        }
        .navigationTitle("New Collection")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .confirmationAction) {
                Button(isSaving ? "Creating…" : "Create") {
                    Task {
                        isSaving = true
                        await onCreate(
                            name.trimmingCharacters(in: .whitespacesAndNewlines),
                            description.trimmedOrNil,
                            visibility
                        )
                        isSaving = false
                        dismiss()
                    }
                }
                .disabled(name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || isSaving)
            }
        }
    }
}

struct CollectionDetailView: View {
    let collection: PlaceCollection

    var body: some View {
        ContentUnavailableView(
            collection.name,
            systemImage: "rectangle.stack",
            description: Text(collection.description ?? "Places added to this collection will appear here.")
        )
        .navigationTitle(collection.name)
        .navigationBarTitleDisplayMode(.inline)
    }
}

struct MissingContentView: View {
    let title: String
    let symbol: String

    var body: some View {
        ContentUnavailableView(
            title,
            systemImage: symbol,
            description: Text("Return to the profile and try again.")
        )
    }
}

private extension String {
    var trimmedOrNil: String? {
        let trimmed = trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? nil : trimmed
    }
}

#Preview("Settings") {
    NavigationStack { SettingsView(onSignOut: {}) }
}

#Preview("Place Detail") {
    NavigationStack { PlaceDetailView(place: ProfileContent.preview.savedPlaces[0]) }
}
