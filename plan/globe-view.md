# Globe View

A full-screen, interactive 3D Earth in SwiftUI: satellite texture, atmospheric
haze on the limb, a star field behind it, and tappable markers pinned to
lat/lon coordinates.

## What "done" looks like

1. Launching the app shows a globe filling the screen, slowly rotating (or
   parked on a starting coordinate).
2. The Earth is rendered with real satellite imagery, not a flat map.
3. There is a visible atmosphere glow along the horizon and a dark, starry
   space background around the sphere.
4. Markers can be added at arbitrary coordinates. They stay stuck to their
   point on the surface as the globe rotates, hide when they swing around the
   back, and respond to taps.
5. Everything above is driven from SwiftUI — no `UIViewController` in the app's
   view hierarchy beyond whatever a SDK wrapper requires.

## Options considered

| Approach | Earth texture | Atmosphere | Stars | Markers | Cost / risk |
| --- | --- | --- | --- | --- | --- |
| **Mapbox Maps SDK v11 (SwiftUI)** | Built in (`.standardSatellite`) | Built in (`Atmosphere`) | Built in (`starIntensity`) | Built in (view + layer annotations) | Needs account + token, billed per MAU |
| RealityKit (`RealityView`, iOS 18+) | Ship your own equirectangular texture | Hand-written shader / second translucent sphere | Skybox HDR or SwiftUI canvas | Manual lat/lon → 3D math + projection to screen | No vendor, but everything is hand-built |
| SceneKit (`SCNView`) | Same as RealityKit | `SCNSphere` + material tricks | `background.contents` cube map | Manual | Soft-deprecated at WWDC25 — critical-bug fixes only |
| MapKit | Satellite imagery, but no globe framing or space background | — | — | Native | Doesn't do the thing |

### Recommendation

**Go with Mapbox Maps SDK v11 in its SwiftUI form.** The four requirements —
Earth texture, atmosphere, stars, markers — are each a first-class feature of
the SDK rather than something to build. It's also a direct translation of the
`GlobeViewController` sketch already in hand, so the risk is mostly plumbing.

Fall back to RealityKit only if the Mapbox account/billing story is a blocker
(see [Cost and licensing](#cost-and-licensing)) or if the design calls for a
stylised, non-cartographic Earth. That fallback is sketched at the end.

### What Mapbox constrains

The SDK is a very good fit, but it is a *map engine wrapped onto a sphere*, not
a 3D object viewer. Four consequences worth accepting deliberately rather than
discovering in Phase 4:

1. **The globe unwraps into a flat map at ~zoom 5.** Globe projection switches
   to Mercator automatically once the camera passes the transition zoom
   (`GLOBE_ZOOM_THRESHOLD_MIN` is 5, with a blend up to ~6), and the atmosphere
   and stars go with it. This is the single biggest behavioral decision in the
   feature. Either accept the unwrap as a "zoom into the map" affordance, or
   clamp it with `cameraBounds(CameraBounds(maxZoom: 5))` so the app is always a
   globe. Pick before building Phase 5.
2. **Interaction is map-like, not trackball-like.** Dragging pans lat/lon and
   the view stays north-up; it does not tumble freely on an arbitrary axis.
   Pitch is limited at low zoom. For "spin the Earth, tap a place" this is
   exactly right. For "toss the planet and let it wobble," it is not.
3. **Tiles stream over the network.** Satellite imagery is fetched, not
   bundled, so a cold launch on a bad connection shows a blurry or empty globe.
   If the globe is the app's first screen, that is the first impression. Budget
   for either a bundled low-zoom fallback image behind the map or a preloaded
   offline tile pack for zoom 0–3.
4. **The SDK is not small.** `MapboxMaps` adds meaningful binary weight to what
   is currently a near-empty app. Fine for a real product, worth knowing if
   Trace is meant to stay lightweight.

None of these are reasons to avoid Mapbox for this feature. They are the shape
of the tool.

## Prerequisites

Two different Mapbox tokens are needed; they are not interchangeable.

- **Download token** — scope `DOWNLOADS:READ`. Goes in `~/.netrc`, never in the
  repo:

  ```
  machine api.mapbox.com
    login mapbox
    password <secret token with DOWNLOADS:READ>
  ```

  Without it, SPM cannot resolve the package.

- **Public access token** — used at runtime for tile requests.

The project generates its `Info.plist` (`GENERATE_INFOPLIST_FILE = YES`), so
there's no plist file to drop `MBXAccessToken` into. Two workable paths:

- Set it in code at launch: `MapboxOptions.accessToken = ...`, reading from an
  xcconfig-injected build setting. Preferred — keeps the token out of source
  control and out of the asset catalog.
- Or add an `INFOPLIST_KEY_MBXAccessToken` build setting, which Xcode merges
  into the generated plist. Simpler, but the value lands in `project.pbxproj`.

Add a `Secrets.xcconfig` (gitignored) with a checked-in `Secrets.example.xcconfig`
next to it so a fresh clone knows what's missing.

## Architecture

New files, all under `src/Trace/Trace/`. The Xcode project uses file-system
synchronized groups, so dropping `.swift` files in is enough — no
`project.pbxproj` surgery. The SPM dependency itself *does* require a project
edit (do it through Xcode's UI, not by hand).

```
src/Trace/Trace/
├── TraceApp.swift              # set MapboxOptions.accessToken at launch
├── Globe/
│   ├── GlobeView.swift         # the SwiftUI Map + style + annotations
│   ├── GlobeViewModel.swift    # @Observable: viewport, markers, selection
│   ├── GlobeStyle.swift        # atmosphere/projection tuning in one place
│   ├── GlobeMarker.swift       # Identifiable model: id, coordinate, title, kind
│   └── MarkerView.swift        # the SwiftUI pin rendered per marker
└── ContentView.swift           # hosts GlobeView
```

Keeping `GlobeStyle` separate matters: atmosphere values are pure taste and get
tweaked a lot, and they shouldn't be tangled up with view code.

## Implementation phases

> **The code below is illustrative, not compiled.** There is no macOS toolchain
> in the environment this plan was written in, so no sample here has been built
> against the SDK. Treat the shapes as correct and the exact signatures as
> things to confirm against the version SPM actually resolves — particularly the
> `Atmosphere` builder methods, whose declarative-DSL spellings differ from the
> imperative struct's properties.

### Phase 1 — Dependency and a map on screen

Add `https://github.com/mapbox/mapbox-maps-ios` via
**File ▸ Add Package Dependencies**, pinning to the latest 11.x. Link the
`MapboxMaps` product to the `Trace` target.

Set the token at launch:

```swift
@main
struct TraceApp: App {
    init() {
        MapboxOptions.accessToken = Bundle.main
            .object(forInfoDictionaryKey: "MBXAccessToken") as? String ?? ""
    }
    var body: some Scene { WindowGroup { ContentView() } }
}
```

Then the smallest possible map, to confirm the token and the build work before
any styling:

```swift
import SwiftUI
import MapboxMaps

struct GlobeView: View {
    var body: some View {
        Map(initialViewport: .camera(
            center: CLLocationCoordinate2D(latitude: 41.8781, longitude: -87.6298),
            zoom: 0.8
        ))
        .ignoresSafeArea()
    }
}
```

**Checkpoint:** a map renders and no `401`s appear in the console.

### Phase 2 — Globe projection, satellite texture, atmosphere

Three things to set. The SwiftUI equivalents of the UIKit sketch:

```swift
Map(viewport: $viewport) {
    StyleProjection(name: .globe)
    Atmosphere()
        .range(0.8, 8)
        .horizonBlend(0.5)
        .starIntensity(0.15)
        .color(StyleColor(.init(red: 0.86, green: 0.90, blue: 1.0, alpha: 1)))
        .highColor(StyleColor(.init(red: 0.14, green: 0.24, blue: 0.55, alpha: 1)))
        .spaceColor(StyleColor(.init(red: 0.02, green: 0.03, blue: 0.07, alpha: 1)))
}
.mapStyle(.standardSatellite(
    lightPreset: .night,
    showPointOfInterestLabels: false,
    showTransitLabels: false,
    showPlaceLabels: true,
    showRoadLabels: false
))
.ignoresSafeArea()
```

Notes on each piece:

- **Projection.** `StyleProjection(name: .globe)` is declarative map style
  content in v11, so it can live inside the `Map` builder. Standard and Standard
  Satellite already default to globe at low zoom — verify on device before
  assuming the explicit call is required; keep it if it removes ambiguity.
- **Texture.** `.standardSatellite` is the satellite-imagery variant of the
  Standard style. All the Standard configuration knobs apply except 3D-object
  toggling and color theming. Turning off POI/road/transit labels is what makes
  it read as a globe rather than a map.
- **Atmosphere.** `starIntensity` runs 0 (no stars) to 1 (max). `spaceColor` is
  the color beyond the horizon blend; `horizonBlend` controls how far the
  atmosphere fades up into space. Start at the values above and tune by eye —
  the default `Atmosphere()` is tuned for daylight maps and reads washed out
  against a dark UI.

**Checkpoint:** a sphere, satellite-textured, with glow at the limb and faint
stars around it.

### Phase 3 — Star background

Two routes, in order of preference.

1. **Use the SDK's stars.** Raise `starIntensity` (0.15–0.6) and darken
   `spaceColor`. Zero extra code, and the stars parallax correctly with camera
   movement. Caveat: stars only render in globe projection at low zoom, so
   they'll fade out as the user zooms in — which is usually what's wanted
   anyway.

2. **Composite a custom star field behind the map.** If the design needs
   specific art (nebula, branded gradient, denser stars), make the map's space
   region transparent and stack something behind it:

   ```swift
   ZStack {
       StarFieldView()          // Canvas of seeded random dots, or an Image
       Map(viewport: $viewport) {
           StyleProjection(name: .globe)
           Atmosphere()
               .spaceColor(StyleColor(UIColor.clear))   // alpha 0 = transparent
               .starIntensity(0)
       }
       .opaque(false)           // let the map view itself be non-opaque
   }
   ```

   `spaceColor` opacity 0 gives a transparent background, and the `opaque(false)`
   modifier stops the map view from painting over what's behind it. Cost: an
   extra transparency pass and a star field that no longer moves with the
   camera unless it's animated by hand.

Start with route 1. Only reach for route 2 if the art direction demands it.

### Phase 4 — Markers

Model first:

```swift
struct GlobeMarker: Identifiable, Hashable {
    let id: UUID
    let coordinate: CLLocationCoordinate2D
    let title: String
}
```

Then pick an annotation kind — this is the one real design decision in the
feature:

- **`MapViewAnnotation`** hosts an arbitrary SwiftUI view at a coordinate. Full
  styling freedom (SF Symbols, images, labels, animation), but each one is a
  real UIKit-hosted view, so it costs more per marker. Right for tens of
  markers.
- **`PointAnnotation`** renders natively inside the map's layers. Supports
  clustering, sits correctly between map layers, far cheaper. Requires a
  bitmap image rather than a SwiftUI view. Right for hundreds or thousands.

Recommendation: **start with `MapViewAnnotation`**, because Trace will likely
want custom-looking pins and the counts will be small. Keep the annotation
construction behind a single `markerContent` function so swapping to
`PointAnnotation` later touches one place.

```swift
Map(viewport: $viewport) {
    StyleProjection(name: .globe)
    Atmosphere()...

    ForEvery(model.markers) { marker in
        MapViewAnnotation(coordinate: marker.coordinate) {
            MarkerView(marker: marker, isSelected: marker.id == model.selectedID)
                .onTapGesture { model.select(marker) }
        }
        .allowOverlap(false)
        .variableAnchors([.init(anchor: .bottom)])
    }
}
```

Use `ForEvery` rather than `ForEach` — it's the `MapContent` equivalent and is
what makes annotations diff correctly when the marker array changes.

Behaviors to handle explicitly:

- **Back-of-globe occlusion.** Mapbox implements occlusion for markers on the
  far side of the globe. Verify it on device with a marker at the antipode of
  the camera center; if a marker bleeds through, fall back to computing the dot
  product of the marker's unit vector against the camera's and setting
  `visible` accordingly.
- **Selection.** Keep `selectedID` in the view model, not in the annotation, so
  a detail sheet and the pin stay in sync.
- **Fly-to.** On selection, animate the camera:
  `withViewportAnimation(.easeInOut(duration: 1.2)) { viewport = .camera(center: marker.coordinate, zoom: 2) }`.

**Checkpoint:** markers stay glued to their coordinates while dragging the
globe, disappear around the back, and tapping one selects it.

### Phase 5 — Motion and polish

- **Idle rotation.** Drive it from a `Timer` or `TimelineView` that nudges the
  viewport's center longitude each tick, wrapped in `withViewportAnimation(.linear)`.
  Stop it on first user interaction — `transitionsToIdleUponUserInteraction(false)`
  keeps the SDK from fighting the manual camera.
- **Gesture trimming.** `.gestureOptions(GestureOptions)` — set
  `pitchEnabled = false` and `pinchRotateEnabled = false` if the globe should
  only spin and zoom. Programmatic camera changes still work when these are off.
- **Zoom clamp.** If the globe should never unwrap into a flat map, apply
  `.cameraBounds(CameraBounds(maxZoom: 5))` — see
  [What Mapbox constrains](#what-mapbox-constrains).
- **Ornaments.** `ornamentOptions` — the Mapbox logo and attribution must stay
  visible per the terms of service, but they can be repositioned.
- **Frame rate.** `frameRate(range:preferred:)` to cap at 30fps for battery if
  idle rotation runs continuously.
- **Accessibility.** Annotations need labels; the globe itself should expose a
  summary rather than being an opaque blob to VoiceOver.

## Cost and licensing

Mobile maps bill per **monthly active user** — a user counts the moment the app
renders a map. The free tier is 25,000 MAU/month, and a credit card is required
to activate it even at zero usage. Attribution must remain visible. If any of
that is unacceptable, take the fallback below before building on Mapbox.

## Fallback: RealityKit globe

If the vendor dependency is ruled out. iOS 18+ only (`RealityView` on iOS), so
the deployment target moves from 17.0 to 18.0.

1. `RealityView` with a `ModelEntity(mesh: .generateSphere(radius: 1))`.
2. Earth texture: NASA Blue Marble equirectangular imagery (public domain),
   downsampled to 8192×4096 or smaller and shipped as a compressed asset.
   Mip-map it — a full-res texture on a small sphere aliases badly.
3. Atmosphere: a second, slightly larger sphere with an inverted-normal
   translucent material whose alpha falls off toward the center — a Fresnel rim
   glow. `CustomMaterial` with a Metal surface shader if the built-in materials
   can't get there.
4. Stars: an HDR skybox via `ImageBasedLightComponent`/environment, or simply a
   SwiftUI `Canvas` of seeded dots behind the `RealityView`.
5. Markers: convert lat/lon to a unit vector
   (`x = cos(lat)cos(lon)`, `y = sin(lat)`, `z = cos(lat)sin(lon)`), place small
   entities on the surface, and — since SwiftUI attachments are visionOS-only —
   project entity positions to screen space each frame to lay SwiftUI labels on
   top. Occlusion is a dot-product test against the camera direction.

This is roughly a week of work to reach parity with what Phase 2–4 gets in a
day, and none of it comes with real cartography. Choose it for art direction,
not for convenience.

## Risks and open questions

- **Token handling** — needs a decision on xcconfig vs. build setting before
  Phase 1, and a note in the README so a fresh clone isn't a mystery.
- **Bundle identifier** — still `com.example.Trace`. Mapbox doesn't care, but
  device testing does.
- **Standard Satellite defaults** — whether globe projection and a usable
  atmosphere come for free in v11 needs to be confirmed on device rather than
  assumed from docs; the explicit `StyleProjection` and `Atmosphere` calls are
  cheap insurance either way.
- **Marker occlusion** — assumed to work on iOS as it does in GL JS. Verify
  early; the manual fallback is straightforward but should be scheduled if
  needed.
- **Deployment target** — stays at 17.0 for the Mapbox path, moves to 18.0 for
  the RealityKit path.
- **The SwiftUI API has carried an "experimental" label** — Mapbox shipped it in
  v11 with a note that it may change until it stabilizes. This plan rests
  entirely on it. Check whether that caveat still applies to the version SPM
  resolves; if it does, pin an exact version rather than a range, and expect
  minor-version upgrades to need a look.
- **Zoom behavior is unresolved** — whether the globe clamps at zoom 5 or is
  allowed to unwrap into a map is a product decision nobody has made yet, and it
  changes what Phase 5 builds.
- **Cold-launch over poor network** — no mitigation designed yet; see constraint
  3 above.
- **Testing** — the map is hard to unit-test. Test `GlobeViewModel` (marker
  add/remove/select, coordinate math, rotation stepping) and leave rendering to
  manual/snapshot checks.

## Task checklist

- [ ] Decide: does the globe clamp at zoom 5, or unwrap into a flat map?
- [ ] Decide token storage (xcconfig recommended); add `Secrets.example.xcconfig`
- [ ] Add `mapbox-maps-ios` 11.x via SPM; link `MapboxMaps`
- [ ] `MapboxOptions.accessToken` wired up in `TraceApp`
- [ ] `GlobeView` renders a map (Phase 1 checkpoint)
- [ ] Globe projection + `.standardSatellite` + tuned `Atmosphere` (Phase 2)
- [ ] Star background dialed in (Phase 3)
- [ ] `GlobeMarker`, `MarkerView`, `GlobeViewModel`, annotations, tap → selection (Phase 4)
- [ ] Verify back-of-globe occlusion on device
- [ ] Idle rotation + gesture/ornament/zoom-clamp config (Phase 5)
- [ ] Decide cold-launch fallback (bundled low-zoom image or offline tile pack)
- [ ] `ContentView` hosts `GlobeView`; delete the tap-counter placeholder
- [ ] Unit tests for `GlobeViewModel`
- [ ] README: token setup steps

## References

- [SwiftUI | Maps SDK | iOS | Mapbox](https://docs.mapbox.com/ios/maps/guides/swift-ui/)
- [Declarative Map Styling | Maps SDK | iOS | Mapbox](https://docs.mapbox.com/ios/maps/guides/styles/declarative-map-styling/)
- [Display a globe | Maps SDK | iOS | Mapbox](https://docs.mapbox.com/ios/maps/examples/globe/)
- [Create a rotating globe | Maps SDK | iOS | Mapbox](https://docs.mapbox.com/ios/maps/examples/rotating-globe/)
- [View annotations | Maps SDK | iOS | Mapbox](https://docs.mapbox.com/ios/maps/guides/add-your-data/view-annotations/)
- [Annotations | Maps SDK | iOS | Mapbox](https://docs.mapbox.com/ios/maps/guides/add-your-data/annotations/)
- [Atmosphere `spaceColor` | Maps SDK | iOS | Mapbox](https://docs.mapbox.com/ios/maps/api/11.8.0-rc.1/documentation/mapboxmaps/atmosphere/spacecolor/)
- [Fog | Mapbox Style Spec](https://docs.mapbox.com/style-spec/reference/fog/)
- [Get Started with Maps SDK for iOS](https://docs.mapbox.com/ios/maps/guides/install/)
- [Pricing | Maps SDK | iOS | Mapbox](https://docs.mapbox.com/ios/maps/guides/pricing/)
- [Bring your SceneKit project to RealityKit — WWDC25](https://developer.apple.com/videos/play/wwdc2025/288/)
- [Displaying 3D objects with RealityView on iOS, iPadOS and macOS](https://www.createwithswift.com/displaying-3d-objects-with-realityview-on-ios-ipados-and-macos/)
