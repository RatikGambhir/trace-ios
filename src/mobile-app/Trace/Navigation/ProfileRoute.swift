import Foundation

enum ProfileRoute: Hashable {
    case settings
    case editProfile
    case invites
    case savedPlace(UUID)
    case collection(UUID)
    case createCollection
    case map
    case addPlace
}
