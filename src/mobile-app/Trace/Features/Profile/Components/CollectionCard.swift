import SwiftUI

struct CreateCollectionCard: View {
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: TraceTheme.Spacing.medium) {
                Image(systemName: "rectangle.stack.badge.plus")
                    .font(.system(size: 28, weight: .light))
                    .foregroundStyle(TraceTheme.Colors.accent)
                    .frame(width: 72, height: 72)
                    .background(TraceTheme.Colors.background)
                    .clipShape(RoundedRectangle(cornerRadius: TraceTheme.Radius.small, style: .continuous))

                VStack(alignment: .leading, spacing: TraceTheme.Spacing.xSmall) {
                    Text("Create a collection")
                        .font(.system(size: 18, weight: .semibold))
                        .foregroundStyle(TraceTheme.Colors.primaryText)

                    Text("Group places, plan trips, or share lists with others.")
                        .font(TraceTheme.Fonts.profileBody)
                        .foregroundStyle(TraceTheme.Colors.profileSecondaryText)
                        .multilineTextAlignment(.leading)
                }

                Spacer(minLength: 0)

                Image(systemName: "chevron.right")
                    .font(.system(size: 16, weight: .semibold))
                    .foregroundStyle(TraceTheme.Colors.primaryText)
            }
            .padding(TraceTheme.Spacing.medium)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(TraceTheme.Colors.surface)
            .clipShape(RoundedRectangle(cornerRadius: TraceTheme.Radius.large, style: .continuous))
            .overlay {
                RoundedRectangle(cornerRadius: TraceTheme.Radius.large, style: .continuous)
                    .stroke(TraceTheme.Colors.divider, lineWidth: 1)
            }
        }
        .buttonStyle(.plain)
        .accessibilityHint("Opens the collection creator")
    }
}

struct CollectionCard: View {
    let collection: PlaceCollection
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: TraceTheme.Spacing.medium) {
                Image(systemName: "rectangle.stack.fill")
                    .font(.system(size: 24))
                    .foregroundStyle(TraceTheme.Colors.accent)
                    .frame(width: 56, height: 56)
                    .background(TraceTheme.Colors.background, in: RoundedRectangle(cornerRadius: TraceTheme.Radius.small))

                VStack(alignment: .leading, spacing: TraceTheme.Spacing.xSmall) {
                    Text(collection.name)
                        .font(.system(size: 17, weight: .semibold))
                        .foregroundStyle(TraceTheme.Colors.primaryText)
                    Text("\(collection.placeIDs.count) \(collection.placeIDs.count == 1 ? "place" : "places")")
                        .font(TraceTheme.Fonts.profileCaption)
                        .foregroundStyle(TraceTheme.Colors.profileSecondaryText)
                }

                Spacer()
                Image(systemName: "chevron.right")
                    .foregroundStyle(TraceTheme.Colors.profileSecondaryText)
            }
            .padding(TraceTheme.Spacing.medium)
            .background(TraceTheme.Colors.surface)
            .clipShape(RoundedRectangle(cornerRadius: TraceTheme.Radius.field, style: .continuous))
        }
        .buttonStyle(.plain)
    }
}
