import SwiftUI

struct SavedPlaceRow: View {
    let place: Place
    let onOpen: () -> Void
    let onMove: () -> Void
    let onMap: () -> Void
    let onRemove: () -> Void

    var body: some View {
        HStack(spacing: TraceTheme.Spacing.medium) {
            Button(action: onOpen) {
                HStack(spacing: TraceTheme.Spacing.medium) {
                    PlaceThumbnail(place: place)

                    VStack(alignment: .leading, spacing: TraceTheme.Spacing.xSmall) {
                        Text(place.name)
                            .font(.system(size: 17, weight: .semibold))
                            .foregroundStyle(TraceTheme.Colors.primaryText)

                        Text(place.locationSummary)
                            .font(TraceTheme.Fonts.profileBody)
                            .foregroundStyle(TraceTheme.Colors.profileSecondaryText)
                            .lineLimit(2)

                        if let category = place.category {
                            Text(category)
                                .font(TraceTheme.Fonts.profileCaption)
                                .foregroundStyle(TraceTheme.Colors.profileSecondaryText)
                                .lineLimit(1)
                        }
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)

            Menu {
                Button("Move to collection", systemImage: "folder", action: onMove)
                ShareLink(
                    item: "\(place.name) in \(place.city)",
                    label: { Label("Share", systemImage: "square.and.arrow.up") }
                )
                Button("View on map", systemImage: "map", action: onMap)
                Divider()
                Button("Remove from saved places", systemImage: "trash", role: .destructive, action: onRemove)
            } label: {
                Image(systemName: "ellipsis")
                    .font(.system(size: 18, weight: .semibold))
                    .foregroundStyle(TraceTheme.Colors.profileSecondaryText)
                    .frame(width: 44, height: 44)
                    .contentShape(Rectangle())
            }
            .accessibilityLabel("More options for \(place.name)")
        }
        .padding(.vertical, TraceTheme.Spacing.medium)
    }
}

private struct PlaceThumbnail: View {
    let place: Place

    var body: some View {
        Group {
            if let url = place.imageURL {
                AsyncImage(url: url) { phase in
                    switch phase {
                    case .success(let image): image.resizable().scaledToFill()
                    case .failure: placeholder
                    case .empty:
                        Rectangle()
                            .fill(TraceTheme.Colors.divider)
                            .overlay { ProgressView().tint(TraceTheme.Colors.accent) }
                    @unknown default: placeholder
                    }
                }
            } else {
                placeholder
            }
        }
        .frame(width: 80, height: 80)
        .clipShape(RoundedRectangle(cornerRadius: TraceTheme.Radius.small, style: .continuous))
        .accessibilityHidden(true)
    }

    private var placeholder: some View {
        Rectangle()
            .fill(TraceTheme.Colors.surface)
            .overlay {
                Image(systemName: "fork.knife")
                    .font(.system(size: 24, weight: .medium))
                    .foregroundStyle(TraceTheme.Colors.accent)
            }
    }
}

#Preview {
    SavedPlaceRow(place: ProfileContent.preview.savedPlaces[0], onOpen: {}, onMove: {}, onMap: {}, onRemove: {})
        .padding()
        .background(TraceTheme.Colors.background)
}
