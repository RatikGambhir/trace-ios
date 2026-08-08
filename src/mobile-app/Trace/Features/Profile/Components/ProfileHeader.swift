import SwiftUI

struct ProfileHeader: View {
    let profile: UserProfile
    let onInvites: () -> Void
    let onEditProfile: () -> Void
    let onSettings: () -> Void
    let onSignOut: () -> Void

    var body: some View {
        ZStack(alignment: .bottom) {
            ZStack(alignment: .top) {
                coverImage
                    .frame(maxWidth: .infinity)
                    .frame(height: 272)
                    .clipped()
                    .ignoresSafeArea(edges: .top)
                    .accessibilityHidden(true)

                LinearGradient(
                    colors: [.black.opacity(0.44), .clear],
                    startPoint: .top,
                    endPoint: .bottom
                )
                .frame(height: 128)
                .accessibilityHidden(true)

                headerActions
                    .padding(.top, TraceTheme.Spacing.small)
                    .padding(.horizontal, TraceTheme.Spacing.medium)
            }

            ProfileAvatar(profile: profile)
                .offset(y: 58)
        }
        .padding(.bottom, 58)
    }

    @ViewBuilder
    private var coverImage: some View {
        if let url = profile.coverImageURL {
            AsyncImage(url: url) { phase in
                switch phase {
                case .success(let image):
                    image.resizable().scaledToFill()
                case .failure:
                    fallbackCover
                case .empty:
                    Rectangle()
                        .fill(TraceTheme.Colors.divider)
                        .overlay { ProgressView().tint(TraceTheme.Colors.accent) }
                @unknown default:
                    fallbackCover
                }
            }
        } else {
            fallbackCover
        }
    }

    private var fallbackCover: some View {
        Image(.landingBackground)
            .resizable()
            .scaledToFill()
    }

    private var headerActions: some View {
        HStack(alignment: .top) {
            Button(action: onInvites) {
                HStack(spacing: TraceTheme.Spacing.small) {
                    Image(systemName: "person.text.rectangle")
                    Text("Invites")
                        .font(.system(size: 15, weight: .semibold))
                }
                .foregroundStyle(.white)
                .padding(.horizontal, TraceTheme.Spacing.medium)
                .frame(minHeight: 48)
                .background(.ultraThinMaterial, in: Capsule())
                .overlay { Capsule().fill(.black.opacity(0.22)) }
                .overlay(alignment: .topTrailing) {
                    if profile.inviteCount > 0 {
                        Text("\(profile.inviteCount)")
                            .font(.system(size: 12, weight: .bold))
                            .foregroundStyle(.white)
                            .frame(minWidth: 26, minHeight: 26)
                            .background(TraceTheme.Colors.accent, in: Circle())
                            .offset(x: 5, y: -7)
                            .accessibilityHidden(true)
                    }
                }
            }
            .buttonStyle(.plain)
            .accessibilityLabel("Invites, \(profile.inviteCount) unread")

            Spacer()

            ShareLink(item: "Follow \(profile.displayName)'s places on Trace") {
                HeaderCircleLabel(symbol: "square.and.arrow.up")
            }
            .accessibilityLabel("Share profile")

            Menu {
                Button("Edit Profile", systemImage: "pencil", action: onEditProfile)
                Button("Settings", systemImage: "gearshape", action: onSettings)
                Button("Privacy", systemImage: "hand.raised", action: onSettings)
                ShareLink(
                    item: "Follow \(profile.displayName)'s places on Trace",
                    label: { Label("Share Profile", systemImage: "square.and.arrow.up") }
                )
                Divider()
                Button("Sign Out", systemImage: "rectangle.portrait.and.arrow.right", role: .destructive, action: onSignOut)
            } label: {
                HeaderCircleLabel(symbol: "ellipsis")
            }
            .accessibilityLabel("Profile options")
        }
    }
}

private struct HeaderCircleLabel: View {
    let symbol: String

    var body: some View {
        Image(systemName: symbol)
            .font(.system(size: 18, weight: .semibold))
            .foregroundStyle(.white)
            .frame(width: 48, height: 48)
            .background(.ultraThinMaterial, in: Circle())
            .overlay { Circle().fill(.black.opacity(0.22)) }
            .contentShape(Circle())
    }
}

struct ProfileAvatar: View {
    let profile: UserProfile

    var body: some View {
        Group {
            if let url = profile.avatarURL {
                AsyncImage(url: url) { phase in
                    switch phase {
                    case .success(let image):
                        image.resizable().scaledToFill()
                    case .failure:
                        initials
                    case .empty:
                        Circle()
                            .fill(TraceTheme.Colors.surface)
                            .overlay { ProgressView().tint(TraceTheme.Colors.accent) }
                    @unknown default:
                        initials
                    }
                }
            } else {
                initials
            }
        }
        .frame(width: 116, height: 116)
        .clipShape(Circle())
        .overlay {
            Circle()
                .stroke(TraceTheme.Colors.background, lineWidth: 6)
        }
        .shadow(color: .black.opacity(0.10), radius: 12, y: 4)
        .accessibilityLabel("Profile photo for \(profile.displayName)")
    }

    private var initials: some View {
        Circle()
            .fill(TraceTheme.Colors.accent)
            .overlay {
                Text(profile.initials)
                    .font(.system(size: 34, weight: .medium, design: .serif))
                    .foregroundStyle(.white)
            }
    }
}

#Preview {
    ProfileHeader(
        profile: ProfileContent.preview.profile,
        onInvites: {},
        onEditProfile: {},
        onSettings: {},
        onSignOut: {}
    )
    .background(TraceTheme.Colors.background)
}
