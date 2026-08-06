# RealityKit Globe — Implementation Plan

An interactive, decorative 3D Earth for Trace, in the style of the Apple Maps
globe: a lit sphere on a black starfield, a soft blue atmospheric rim, a pulsing
blue "you are here" dot, drag to spin, pinch to zoom, and a recenter control that
flies the globe back to the user's location.

This document is the design and build plan. No code has landed yet.

---

## Table of contents

1. [What the reference screenshot actually contains](#1-what-the-reference-screenshot-actually-contains)
2. [Framework decision](#2-framework-decision)
3. [Prerequisite: deployment target](#3-prerequisite-deployment-target)
4. [Architecture](#4-architecture)
5. [Coordinate system and the math](#5-coordinate-system-and-the-math)
6. [Geometry: a custom UV sphere](#6-geometry-a-custom-uv-sphere)
7. [Textures and assets](#7-textures-and-assets)
8. [Materials and lighting](#8-materials-and-lighting)
9. [The atmosphere](#9-the-atmosphere)
10. [The starfield](#10-the-starfield)
11. [Graticule and place labels](#11-graticule-and-place-labels)
12. [Camera rig and projection](#12-camera-rig-and-projection)
13. [Interaction: pan, pinch, inertia](#13-interaction-pan-pinch-inertia)
14. [Recenter](#14-recenter)
15. [Location markers](#15-location-markers)
16. [State management and the SwiftUI ↔ RealityKit bridge](#16-state-management-and-the-swiftui--realitykit-bridge)
17. [Performance budget](#17-performance-budget)
18. [Accessibility and motion](#18-accessibility-and-motion)
19. [Testing strategy](#19-testing-strategy)
20. [File layout](#20-file-layout)
21. [Milestones](#21-milestones)
22. [Risks and open questions](#22-risks-and-open-questions)
23. [Appendix A — iOS 17 fallback via ARView](#appendix-a--ios-17-fallback-via-arview)
24. [Appendix B — references](#appendix-b--references)

---

## 1. What the reference screenshot actually contains

Worth decomposing before choosing tools, because several of these layers are not
3D at all and trying to make them 3D is the main way this goes wrong.

| # | Layer | Observation | Where it should live |
|---|-------|-------------|----------------------|
| 1 | Star field | Small, static, uniform white dots on pure black. They do **not** parallax with the globe. | 2D, behind everything |
| 2 | Atmosphere glow | Soft blue halo extending ~2–4% beyond the globe silhouette, brightest right at the limb, falling off outward. | 2D radial gradient, or 3D fresnel shell |
| 3 | Globe surface | Equirectangular Earth imagery. Bathymetry visible (ocean depth shading), land in green/tan. Evenly lit across the whole visible disc — **no day/night terminator, no specular sun highlight**. | 3D |
| 4 | Limb darkening | The globe's own edge is a darker, more saturated blue than its center. | 3D (fresnel) or baked |
| 5 | Graticule | Equator as a solid thin line; Tropics and polar circles as dashed lines. All curve correctly with the sphere, so they are surface-locked. | 3D overlay texture |
| 6 | Place labels | "NORTH AMERICA", "North Pacific Ocean" etc. Letter-spaced, follow the surface, scale with the globe, and are occluded by the limb. | 3D overlay texture (v2) |
| 7 | User location | Solid blue disc, white ring, faint blue halo/beam. Perfectly circular — it is **billboarded**, not foreshortened onto the surface. | 2D overlay, positioned by projection |
| 8 | Chrome | Recenter and mode buttons, search bar. | Plain SwiftUI |

The important read: **item 3 is the only thing that genuinely needs a 3D
renderer.** Items 1, 2, 7, 8 are cheaper, sharper, and far more controllable as
SwiftUI drawn on top of and behind the 3D view. Plan accordingly — this is a
layer cake, not a scene graph.

Note also that the globe is *unlit*. There is no terminator. That single
observation removes an entire category of work (sun positioning, IBL setup,
day/night blending shaders) from v1.

---

## 2. Framework decision

RealityKit is the right call, with eyes open about what it does and does not
give us.

**What we get for free**

- Metal render loop, depth, culling, MSAA, sRGB handling
- `Entity` transform hierarchy with `simd` quaternions
- `MeshResource` / `MeshDescriptor` for custom geometry
- `TextureResource` async loading with mipmaps
- `UnlitMaterial`, `PhysicallyBasedMaterial`, `ShaderGraphMaterial`
- ECS (`System`, `Component`) — a clean home for per-frame inertia
- First-class SwiftUI embedding via `RealityView` (iOS 18+)
- Entity transform animation (`Entity.move(to:relativeTo:duration:timingFunction:)`)

**What we must write ourselves**

- The sphere mesh, if we want reliable UVs (see §6 — the built-in one has a seam)
- Latitude/longitude ⇄ 3D vector conversion
- Pan / pinch gesture → orientation, with clamping and inertia
- Recenter target orientation + animation
- Marker placement, billboarding, and horizon occlusion
- Atmosphere (no built-in fresnel/rim material)
- World → screen projection (no documented public API on iOS `RealityView`;
  trivial to write ourselves given a fixed camera — see §12)

**Alternatives considered**

- **SceneKit** — would also work and has `SCNSphere` + built-in projection
  (`renderer.projectPoint`), but it is in maintenance mode and its SwiftUI
  bridge is a `UIViewRepresentable`. Not the direction to invest in.
- **MapKit's `MKMapView` in globe mode** — gives the *actual* Apple Maps globe,
  labels and all, nearly for free. Rejected because Trace wants a decorative,
  restyled globe it fully controls, not Apple Maps. Worth re-checking if the
  requirement ever softens to "just show a globe" — it would save weeks.
- **Metal from scratch** — full control over the atmosphere shader, but we'd be
  writing a renderer. Only justified if the atmosphere becomes the centerpiece.

---

## 3. Prerequisite: deployment target

**`RealityView` is iOS 18.0+.** The project is currently at `IPHONEOS_DEPLOYMENT_TARGET = 17.0`
(`src/Trace/Trace.xcodeproj/project.pbxproj:167`, `:211`).

Two paths:

**A. Bump to iOS 18.0 (recommended).** One build-setting change in both Debug and
Release configs. Gets us `RealityView`, `RealityViewCameraContent.camera = .virtual`,
and the modern gesture APIs. Given the app is a shell with no users, there is no
cost to this.

**B. Stay on 17.0 and wrap `ARView` in a `UIViewRepresentable`.** Works —
`ARView(frame:cameraMode:.nonAR, automaticallyConfigureSession: false)` is the
long-standing non-AR path, and it has a public `project(_:) -> CGPoint?`. See
[Appendix A](#appendix-a--ios-17-fallback-via-arview). Costs a representable
wrapper and a manual entity-lifecycle story.

Also worth flipping while we're in there: `SWIFT_VERSION = 5.0` → `6.0`, since
we'll be touching `@Observable` and main-actor isolation and Swift 6 concurrency
checking will otherwise be a surprise later.

**Decision needed from the project owner.** The plan below assumes A.

---

## 4. Architecture

```
┌──────────────────────────────────────────────────┐
│ GlobeScreen (SwiftUI)                            │
│                                                  │
│  ZStack {                                        │
│    StarfieldView          ← Canvas, static       │
│    AtmosphereGlow(behind) ← RadialGradient       │
│    RealityView { ... }    ← the globe, 3D        │
│    AtmosphereGlow(front)  ← thin rim, plusLighter│
│    MarkerOverlay          ← projected 2D markers │
│    GlobeControls          ← recenter, buttons    │
│  }                                               │
└──────────────────────────────────────────────────┘
              │ reads/writes
              ▼
     ┌────────────────────┐
     │ GlobeViewModel     │  @Observable, @MainActor
     │  · orientation     │  simd_quatf (source of truth)
     │  · cameraDistance  │  Float
     │  · markers         │  [Marker]
     │  · viewportSize    │  CGSize
     └────────────────────┘
              │ drives                    ▲ writes back each frame
              ▼                           │
     ┌────────────────────────────────────┴───────┐
     │ RealityKit scene                            │
     │  root                                       │
     │   ├── globeRoot (Entity)  ← orientation     │
     │   │    ├── surface  (ModelEntity, sphere)   │
     │   │    ├── graticule(ModelEntity, r×1.001)  │
     │   │    └── markers  (Entity, optional 3D)   │
     │   └── camera (PerspectiveCamera) @ +Z       │
     │                                             │
     │  GlobeRotationSystem ← inertia, per frame   │
     └─────────────────────────────────────────────┘
```

**The load-bearing decision: rotate the globe, not the camera.**

RealityKit ships `.realityViewCameraControls(.orbit)`, which gives orbit + dolly
for free. Do not use it here. Reasons:

1. We need to *read* the current orientation to compute recenter targets and
   marker visibility. An orbit controller owns that state opaquely.
2. We need to *write* orientation (recenter animation, deep-link to a
   coordinate). The orbit controller fights us.
3. There is a known defect where reassigning `cameraTarget` mid-gesture leaves
   the orbit latched to the old target
   ([forums](https://developer.apple.com/forums/thread/825543)).
4. A fixed camera makes world→screen projection a constant, closed-form
   expression we can unit-test (§12).

So: camera parked at `(0, 0, d)` looking down −Z, never rotates. Pinch moves it
along Z only. All rotation lives in one `simd_quatf` on `globeRoot`.

---

## 5. Coordinate system and the math

This is the part that must be written down precisely once, tested, and then never
thought about again. All of it goes in `GlobeMath.swift`, which imports only
`simd` — no RealityKit, no SwiftUI, so it is trivially unit-testable.

### Conventions

- RealityKit is right-handed: **+X right, +Y up, +Z toward the viewer.**
- Globe local frame: **+Y is the north pole. The prime meridian (0° lon) faces +Z**,
  i.e. it points at the camera when `orientation == .identity`.
- Sphere is a unit sphere scaled by `globeRadius`; centered at the world origin.

### Geographic → 3D

```swift
/// Unit vector on the globe's local sphere for a geographic coordinate.
static func unitVector(lat: Double, lon: Double) -> SIMD3<Float> {
    let phi = Float(lat * .pi / 180)      // latitude,  +N
    let lam = Float(lon * .pi / 180)      // longitude, +E
    return SIMD3(cos(phi) * sin(lam),     // x
                 sin(phi),                // y
                 cos(phi) * cos(lam))     // z
}
```

Sanity checks (these become the first unit tests):

| Coordinate | Expected vector |
|---|---|
| 0°N, 0°E (Gulf of Guinea) | `(0, 0, 1)` — facing camera |
| 0°N, 90°E | `(1, 0, 0)` — right limb |
| 90°N (north pole) | `(0, 1, 0)` |
| 0°N, 180°E (date line) | `(0, 0, −1)` — far side |

### 3D → geographic

```swift
static func coordinate(from v: SIMD3<Float>) -> (lat: Double, lon: Double) {
    let n = normalize(v)
    return (Double(asin(n.y)) * 180 / .pi,
            Double(atan2(n.x, n.z)) * 180 / .pi)
}
```

### Equirectangular UV

Standard Blue Marble imagery is 2:1 equirectangular with the prime meridian down
the horizontal center and the north pole along the top edge:

```swift
static func uv(lat: Double, lon: Double) -> SIMD2<Float> {
    SIMD2(Float(0.5 + lon / 360),
          Float(0.5 - lat / 180))
}
```

The mesh generator in §6 derives *both* position and UV from the same
`(lat, lon)`, which is precisely how we avoid the seam problem.

### Recenter: the target orientation

Given a coordinate, we want the quaternion `q` such that `q · p = (0, 0, 1)` —
the point ends up dead center, facing the camera, with north still up.

Decompose into yaw-about-Y then pitch-about-X. Let `p = (cosφ·sinλ, sinφ, cosφ·cosλ)`.

*Step 1 — yaw by `−λ` about +Y.* Rotation about Y by θ maps
`x' = x·cosθ + z·sinθ`, `z' = −x·sinθ + z·cosθ`. With `θ = −λ`:

```
x' = cosφ·sinλ·cosλ − cosφ·cosλ·sinλ = 0
z' = cosφ·sinλ·sinλ + cosφ·cosλ·cosλ = cosφ
```

giving `(0, sinφ, cosφ)` — on the prime-meridian plane.

*Step 2 — pitch by `φ` about +X.* Rotation about X by ψ maps
`y' = y·cosψ − z·sinψ`, `z' = y·sinψ + z·cosψ`. Setting `y' = 0`:

```
sinφ·cosψ = cosφ·sinψ  ⟹  tanψ = tanφ  ⟹  ψ = φ
z' = sin²φ + cos²φ = 1
```

giving exactly `(0, 0, 1)`. ∎

```swift
static func orientation(centering lat: Double, _ lon: Double) -> simd_quatf {
    let yaw   = simd_quatf(angle: Float(-lon * .pi / 180), axis: [0, 1, 0])
    let pitch = simd_quatf(angle: Float( lat * .pi / 180), axis: [1, 0, 0])
    return pitch * yaw          // quaternion product applies yaw first
}
```

### The inverse: what am I looking at?

Needed for a "centered on ___" readout and for restoring state:

```swift
static func centeredCoordinate(for q: simd_quatf) -> (lat: Double, lon: Double) {
    coordinate(from: q.inverse.act([0, 0, 1]))
}
```

### Yaw/pitch decomposition

Because we clamp pitch (§13), the view model stores `yaw` and `pitch` as scalars
and derives the quaternion, rather than accumulating quaternion multiplications
(which drift and can't be clamped):

```swift
var orientation: simd_quatf {
    simd_quatf(angle: pitch, axis: [1, 0, 0]) * simd_quatf(angle: yaw, axis: [0, 1, 0])
}
```

Note this matches the recenter formula exactly: `yaw = −lon`, `pitch = +lat`.

---

## 6. Geometry: a custom UV sphere

`MeshResource.generateSphere(radius:)` is one line and tempting. Don't use it for
a textured Earth. Developers hit a visible seam where UVs wrap 1.0 → 0.0
([forum thread](https://developer.apple.com/forums/thread/816453)), and we have
no control over pole handling, ring density, or tangent generation.

Generate our own with `MeshDescriptor`. It's ~60 lines and buys us correctness.

```swift
enum GlobeSphereMesh {
    /// - Parameters:
    ///   - rings: latitude bands (recommend 128)
    ///   - segments: longitude divisions (recommend 256)
    static func make(radius: Float, rings: Int = 128, segments: Int = 256) throws -> MeshResource {
        var positions: [SIMD3<Float>] = []
        var normals:   [SIMD3<Float>] = []
        var uvs:       [SIMD2<Float>] = []
        var indices:   [UInt32] = []

        for r in 0...rings {
            let v = Float(r) / Float(rings)          // 0 at north pole
            let lat = 90.0 - Double(v) * 180.0
            for s in 0...segments {                   // note: 0...segments, not 0..<
                let u = Float(s) / Float(segments)
                let lon = Double(u) * 360.0 - 180.0
                let n = GlobeMath.unitVector(lat: lat, lon: lon)
                positions.append(n * radius)
                normals.append(n)
                uvs.append(SIMD2(u, v))
            }
        }
        // ... two triangles per quad, skipping degenerate ones at the poles
        var d = MeshDescriptor(name: "globe")
        d.positions  = .init(positions)
        d.normals    = .init(normals)
        d.textureCoordinates = .init(uvs)
        d.primitives = .triangles(indices)
        return try MeshResource.generate(from: [d])
    }
}
```

Points that matter:

- **`0...segments`, not `0..<segments`.** The extra column duplicates the seam
  vertices with `u = 0` and `u = 1` respectively. This is the fix for the wrap
  seam — same position, different UV.
- **Poles.** The top and bottom rings collapse to a point but keep distinct UVs
  across the row, which is correct for equirectangular and avoids a pinched
  texture. Skip the degenerate triangles (`r == 0` gets one triangle per quad,
  not two; likewise `r == rings - 1`).
- **Winding.** RealityKit is counter-clockwise-front. Get this wrong and the
  globe renders inside-out (which looks subtly *fine* until you notice the
  continents are mirrored). Verify by checking that North America appears on the
  left of the Atlantic.
- **Tangents.** Only needed if we add a normal map (§8, v2). `MeshDescriptor`
  can carry `tangents` / `bitangents`; for a UV sphere the tangent is
  `normalize(cross([0,1,0], n))`.
- **Resolution.** 128×256 = 33k verts, 65k tris. Negligible on any device that
  runs iOS 18, and smooth enough that the silhouette has no visible faceting at
  full-screen. Do not go below 96 rings — faceting on the limb is the first
  thing that betrays a low-poly sphere, and the limb is exactly where the
  atmosphere draws the eye.

**Globe radius:** use `1.0` in scene units and set camera distance in the same
scale. Keeping the radius at unity makes every dot-product and projection
formula below drop a term.

---

## 7. Textures and assets

### Source imagery

| Map | Purpose | Phase |
|---|---|---|
| Day / color | The base Earth. NASA Blue Marble Next Generation, or Natural Earth III for the stylized look in the screenshot. | v1 |
| Graticule overlay | Equator + tropics + polar circles, transparent PNG, pre-antialiased. | v1 |
| Night lights | Emissive city lights on the dark side. | v3, only if we add a terminator |
| Specular / roughness | Glossy oceans, matte land. | v2 |
| Normal / bump | Terrain relief. | v2, optional — barely visible at globe scale |

**Licensing.** NASA Visible Earth imagery is public domain and free of licensing
fees; Natural Earth III is public domain; third-party derivatives (Solar System
Scope etc.) are typically CC-BY 4.0 and need an attribution string. Whatever we
ship, record the source and license in `Resources/ATTRIBUTION.md` and surface it
in an app "About" screen. Do this at the time we add the file, not later.

The screenshot's look — visible bathymetry, saturated teal-green land, deep navy
oceans — is closer to a Natural Earth III / bathymetry composite than raw Blue
Marble. Expect a colour-grading pass in an image editor to match it.

### Resolution and memory

This is the single biggest performance lever, and it is easy to get badly wrong.

| Size | Uncompressed RGBA8 | + mips | ASTC 6×6 + mips |
|---|---|---|---|
| 2048×1024 | 8 MB | 10.7 MB | ~0.6 MB |
| 4096×2048 | 32 MB | 42.7 MB | ~2.3 MB |
| 8192×4096 | 128 MB | 170.7 MB | ~9.3 MB |

**Recommendation: 4096×2048, GPU-compressed, via the asset catalog.** At the
zoom levels a decorative globe supports (the globe never fills more than the
screen), 4K equirectangular gives roughly 1 texel per screen pixel at the
sub-camera point. 8K is only justified if we later allow zooming into
country-level detail, and at that point the right answer is tiles, not a bigger
single texture.

Practical steps:

- Put the image in an `.xcassets` Data/Image set with **Preserve Vector Data off**
  and the GPU compression set to ASTC. Asset catalogs do the compression at build
  time; the runtime cost is zero and the app size cost is small.
- Load with `TextureResource(named:in:)` and
  `TextureResource.CreateOptions(semantic: .color, mipmapsMode: .allocateAndGenerateAll)`.
  Mipmaps are **not optional** here — without them the limb, where the texture is
  compressed into a few pixels, will shimmer violently as the globe rotates.
- Set **anisotropic filtering to 16** on the sampler. At the limb the texture is
  viewed at extreme grazing angles; trilinear alone will smear it to mush.
  `PhysicallyBasedMaterial.Texture(resource, sampler: .init(descriptor))` with an
  `MTLSamplerDescriptor` whose `maxAnisotropy = 16`.
- Load **off the main actor**, show a placeholder (a flat navy sphere or the
  atmosphere glow alone) until it resolves, then swap the material. A 4K decode
  is tens of milliseconds and will drop frames if done inline.

### Colour space

Blue Marble source data is sRGB. Use `semantic: .color` so RealityKit applies the
right transfer function. Non-colour maps (roughness, normal) must use
`semantic: .raw` / `.normal` or they'll be gamma-decoded and look wrong in a way
that is very hard to spot by eye.

---

## 8. Materials and lighting

### v1: `UnlitMaterial` — deliberately

Re-read §1, item 3: the reference globe has **no terminator and no specular
highlight**. It is uniformly bright across the disc. That is an unlit render.

```swift
var material = UnlitMaterial()
material.color = .init(tint: .white, texture: .init(dayTexture))
```

This is not a shortcut, it is the correct match to the target look, and it comes
with real benefits: no light rig to tune, no IBL asset to ship, deterministic
appearance across devices and the simulator, and the lowest possible fragment
cost.

The limb darkening in the screenshot (item 4) is then produced by the atmosphere
layer (§9) drawing over the edge, not by lighting.

### v2: `PhysicallyBasedMaterial` — if we want a sun

If a day/night terminator is later wanted:

```swift
var m = PhysicallyBasedMaterial()
m.baseColor = .init(texture: .init(dayTexture))
m.roughness = .init(texture: .init(roughnessTexture))   // oceans glossy
m.metallic  = 0.0
m.emissiveColor = .init(texture: .init(nightLightsTexture))
```

plus a `DirectionalLight` for the sun and a low-intensity
`ImageBasedLightComponent` for fill. The catch: `emissiveColor` is *additive
everywhere*, so city lights would glow through the daylit side too. Correct
day/night blending needs the emissive masked by `dot(normal, sunDirection)`,
which PBR material parameters cannot express.

### v3: `ShaderGraphMaterial` — for real day/night

Authored in **Reality Composer Pro** (macOS only) as a MaterialX graph in an
`.usda` inside a Reality Composer Pro Swift package, then loaded:

```swift
let m = try await ShaderGraphMaterial(named: "/Root/EarthMaterial",
                                      from: "GlobeScene.usda",
                                      in: globeAssetsBundle)
try m.setParameter(name: "sunDirection", value: .simd3Float(sunDir))
```

Graph outline: `dot(N, sunDir)` → `smoothstep(-0.1, 0.1)` → `mix(nightTex, dayTex)`.
Nice-to-have; explicitly out of scope for v1.

`CustomMaterial` (hand-written Metal surface shaders, RealityKit 2, iOS 15+) is
the other route and is more flexible, but it is **unavailable on visionOS**. If
there is any chance Trace goes to visionOS, prefer `ShaderGraphMaterial` so the
material layer ports.

### Lighting note

Under `UnlitMaterial` no lights are needed at all. Do not add a light "just in
case" — an unused `DirectionalLight` still costs a shadow-map decision in the
render graph.

---

## 9. The atmosphere

Three viable implementations, in increasing cost and fidelity.

### Option A — SwiftUI radial gradients (recommended for v1)

The globe's silhouette is a perfect circle whose screen radius we can compute in
closed form (§12). So draw the glow in SwiftUI, exactly registered to that
circle.

```swift
// Behind the RealityView: the outer halo bleeding into space.
Circle()
    .fill(RadialGradient(
        colors: [.clear, Color(red: 0.25, green: 0.5, blue: 0.95).opacity(0.55), .clear],
        center: .center,
        startRadius: discRadius * 0.97,
        endRadius:   discRadius * 1.10))
    .frame(width: discRadius * 2.4, height: discRadius * 2.4)
    .blur(radius: 12)
```

plus a second, thinner ring in front with `.blendMode(.plusLighter)` to get the
bright hairline right at the limb.

**Why this is the right v1:** pixel-level control over the exact look, no shader
authoring, no transparency sort order to fight, trivially animatable with SwiftUI,
and it composites over the starfield correctly for free. The one thing it cannot
do is respond to lighting direction — irrelevant, since v1 is unlit.

**The catch:** the glow is drawn in screen space, so if we ever let the camera
orbit off-axis or the globe leave the screen center, the registration breaks.
With the fixed camera of §4 it cannot.

### Option B — a fresnel shell entity

A second sphere at `radius × 1.04` with **inverted winding** (so we see its
inside), `blending = .transparent`, `faceCulling = .front`, and a
`ShaderGraphMaterial` whose opacity is `pow(1 - saturate(dot(N, V)), k)`.

Correct in 3D, works from any camera angle, and is what a "real" atmosphere
looks like. Costs a Reality Composer Pro package and a macOS authoring step, and
introduces transparency sorting against the marker entities. Worth doing in v2 if
Option A's screen-space limitation ever bites.

### Option C — `CustomMaterial` with a Metal fragment shader

Same shell, hand-written `.metal` surface shader. Most control (can do genuine
Rayleigh-ish scattering falloff). iOS/macOS only. Only if the atmosphere becomes
a signature visual.

**Decision: A for v1, with the shell geometry stubbed but disabled so B is a
material swap, not a rewrite.**

---

## 10. The starfield

Static, non-parallaxing (verified against the screenshot — the stars do not move
when the globe spins). So: SwiftUI `Canvas`, generated once from a seeded RNG so
it is stable across redraws and screenshot tests.

```swift
struct StarfieldView: View {
    let stars: [Star]   // generated once with a fixed seed

    var body: some View {
        Canvas { ctx, size in
            for s in stars {
                let r = s.radius
                ctx.opacity = s.brightness
                ctx.fill(Path(ellipseIn: CGRect(x: s.x * size.width  - r,
                                                y: s.y * size.height - r,
                                                width: r * 2, height: r * 2)),
                         with: .color(.white))
            }
        }
        .background(.black)
        .drawingGroup()      // rasterize once into a Metal layer
    }
}
```

Details that make it look right rather than like static noise:

- **~250–400 stars** for a phone screen. The screenshot is sparse.
- **Radius 0.5–1.5 pt**, brightness 0.25–1.0, both drawn from a distribution
  skewed heavily toward small and dim. A uniform distribution reads as noise.
- **Seeded RNG** (`SystemRandomNumberGenerator` won't do — use a small
  `SeededGenerator` struct) so the field is identical every launch. A star field
  that reshuffles on every view update is instantly noticeable.
- `.drawingGroup()` so it rasterizes once instead of re-running Canvas per frame.
- Optional polish: a very slow twinkle on a 5% subset via `TimelineView`.
  Gate this behind `accessibilityReduceMotion`.

**Do not** implement this as an inverted 3D sky-sphere. It would parallax against
the fixed camera (wrongly, since our camera never rotates — the stars would be
*perfectly static* anyway) while costing a texture and a draw call. The 2D
version is strictly better here.

---

## 11. Graticule and place labels

### Graticule (v1)

Equator solid, tropics and polar circles dashed. Two ways:

**Baked overlay texture (recommended).** A 2048×1024 transparent PNG with the
lines drawn at the right latitudes. Applied to a second sphere at `radius ×
1.0015` with an `UnlitMaterial`, `blending = .transparent`. Reasons: costs
nothing to implement, gets perfect antialiasing from mipmaps, and lets a designer
iterate on line weight and dash pattern in an image editor without touching code.

The line latitudes: Equator 0°, Tropics ±23.4366°, Polar circles ±66.5634°.

**Generated line geometry.** `MeshDescriptor` with `.lineStrip` primitives, or thin
ribbon quads. More correct (constant screen-space width at any zoom), more work,
and RealityKit line rendering has no width control. Skip unless the baked
version's width variation across zoom levels proves objectionable.

### Place labels (v2, explicitly deferred)

The screenshot's "NORTH AMERICA" / "North Pacific Ocean" labels follow the
sphere's curvature and are occluded by the limb. Options:

1. **Bake into the overlay texture.** Cheap, correct occlusion and curvature for
   free. Downside: labels rotate with the globe, so near the poles they appear
   upside-down, and they cannot be localized without re-baking per language.
2. **SwiftUI overlay via projection** (same machinery as the markers, §15).
   Always upright and readable, localizable, but needs per-label occlusion
   testing and does not curve.

Apple's own labels curve, so they're baked or geometry-based. **Recommend: skip
labels entirely in v1.** They are a large amount of polish for a decorative
element, and the globe reads fine without them. Revisit after the interaction
model is proven.

---

## 12. Camera rig and projection

```swift
let camera = PerspectiveCamera()
camera.camera.fieldOfViewInDegrees = 40        // vertical FOV
camera.camera.near = 0.01
camera.camera.far  = 100
camera.position = [0, 0, cameraDistance]       // looking down −Z, no rotation ever
```

A narrower FOV (35–45°) flattens perspective and makes the globe read as
*distant*, which is the look we want. A wide FOV makes it look like a beach ball
held at arm's length.

**Distance range.** With `globeRadius = 1.0`:
- `minDistance = 1.6` — globe fills most of the screen
- `defaultDistance = 3.2` — matches the screenshot's framing (globe ≈ 55% of screen height)
- `maxDistance = 8.0` — globe small in a field of stars

### Silhouette radius on screen

Needed by the atmosphere (§9). For a sphere of radius `R` at the origin with the
camera at distance `d`, the silhouette's angular radius is `α = asin(R/d)` —
note this is the *tangent* circle, slightly smaller than a naive `atan(R/d)`:

```swift
func discRadiusInPoints(viewportHeight: CGFloat) -> CGFloat {
    let alpha = asin(globeRadius / cameraDistance)
    return viewportHeight / 2 * CGFloat(tan(alpha) / tan(fovY / 2))
}
```

### World → screen projection

There is no documented public world-to-screen API on `RealityView` for iOS
(`ARView.project(_:)` exists, which is one more argument for the fallback path in
Appendix A). With our fixed, unrotated camera the projection is a handful of
lines and — importantly — is pure, so it unit-tests:

```swift
/// - Returns: point in SwiftUI view coordinates, or nil if behind the camera.
static func project(_ world: SIMD3<Float>,
                    cameraDistance d: Float,
                    fovY: Float,
                    viewport: CGSize) -> CGPoint? {
    let view = world - SIMD3(0, 0, d)     // camera at +Z, identity rotation
    guard view.z < 0 else { return nil }  // behind camera
    let t = tan(fovY / 2)
    let aspect = Float(viewport.width / viewport.height)
    let ndcX = (view.x / -view.z) / (aspect * t)
    let ndcY = (view.y / -view.z) / t
    return CGPoint(x: CGFloat(ndcX * 0.5 + 0.5) * viewport.width,
                   y: CGFloat(0.5 - ndcY * 0.5) * viewport.height)
}
```

Validate against RealityKit once, empirically, by placing a marker entity and a
projected SwiftUI dot at the same coordinate and confirming they coincide at
several zoom levels and both device orientations. Then trust the formula.

---

## 13. Interaction: pan, pinch, inertia

### Pan → yaw/pitch

Attach a `DragGesture(minimumDistance: 0)` to the `RealityView` (not to a
`.gesture` targeting an entity — we want drags anywhere on screen to spin the
globe, including in empty space, which matches Apple Maps).

```swift
DragGesture(minimumDistance: 0)
    .onChanged { value in
        let k = model.radiansPerPoint          // see below
        model.yaw   = yawAtGestureStart   + Float(value.translation.width)  * k
        model.pitch = pitchAtGestureStart + Float(value.translation.height) * k
        model.pitch = min(max(model.pitch, -maxPitch), maxPitch)
    }
    .onEnded { value in
        model.beginInertia(from: value)
    }
```

**Signs, derived rather than guessed:**
- Dragging **right** (`width > 0`) should move surface features right. Rotation
  about +Y by +θ maps `(0,0,1) → (sinθ, 0, cosθ)` — rightward. So `yaw += dx·k`. ✅
- Dragging **down** (`height > 0`, since SwiftUI's y grows downward) should move
  features down. Rotation about +X by +θ maps `(0,0,1) → (0, −sinθ, cosθ)` —
  downward. So `pitch += dy·k`. ✅

Both positive. This also matches the recenter formula (`pitch = +lat`): dragging
down brings northern latitudes to center, which is what you'd expect.

**Gain (`radiansPerPoint`).** A fixed value feels wrong at different zoom levels.
Derive it so that a drag across the globe's on-screen diameter rotates roughly
180°, which makes the globe feel *grabbed* rather than *nudged*:

```swift
var radiansPerPoint: Float {
    Float(.pi / (2 * discRadiusInPoints(viewportHeight: viewport.height)))
}
```

Then zooming in automatically slows rotation, which is exactly the behaviour that
makes fine positioning possible when close.

**Pitch clamp.** `maxPitch = 85°` (not 90°). At exactly 90° the pole is dead
center and yaw becomes degenerate — the globe spins in place under the cursor,
which feels broken. Clamping at 85° keeps the poles reachable and viewable while
preserving a sane up-vector. Add a rubber-band: past the clamp, apply the
overshoot at 25% gain and spring back on release.

### Pinch → zoom

```swift
MagnifyGesture()
    .onChanged { value in
        let d = distanceAtGestureStart / Float(value.magnification)
        model.cameraDistance = clampWithRubberBand(d, min: 1.6, max: 8.0)
    }
    .onEnded { _ in model.settleZoom() }
```

Divide, don't multiply: pinching *out* (magnification > 1) should bring the globe
*closer*, i.e. reduce distance.

**Compose the two gestures with `.simultaneously(with:)`** so a two-finger
gesture that drifts still pans. Capture `yawAtGestureStart` etc. on the first
`onChanged` (SwiftUI gestures give cumulative translation, so accumulating deltas
frame-to-frame double-counts).

### Inertia

The screenshot can't show it, but the Apple Maps globe carries momentum, and
without it the globe feels dead. Implement as a RealityKit `System` so it ticks
in lockstep with rendering rather than on a SwiftUI timer:

```swift
struct GlobeSpinComponent: Component {
    var angularVelocity: SIMD2<Float> = .zero   // (yawRate, pitchRate) rad/s
}

struct GlobeRotationSystem: System {
    static let query = EntityQuery(where: .has(GlobeSpinComponent.self))

    func update(context: SceneUpdateContext) {
        let dt = Float(context.deltaTime)
        for entity in context.entities(matching: Self.query, updatingSystemWhen: .rendering) {
            guard var spin = entity.components[GlobeSpinComponent.self] else { continue }
            guard length(spin.angularVelocity) > 0.001 else {
                spin.angularVelocity = .zero
                entity.components[GlobeSpinComponent.self] = spin
                continue
            }
            // apply, then decay ~ e^(-4t): loses 98% of speed in 1s
            model.apply(delta: spin.angularVelocity * dt)
            spin.angularVelocity *= exp(-4.0 * dt)
            entity.components[GlobeSpinComponent.self] = spin
        }
    }
}
```

Register with `GlobeRotationSystem.registerSystem()` once, before the scene is
built.

Seed the velocity from `DragGesture.Value.predictedEndTranslation` minus
`translation`, divided by a nominal fling duration — SwiftUI's prediction already
encodes the platform's flick feel, so using it keeps the globe consistent with
scroll views elsewhere in the app.

Pitch inertia should decay faster than yaw (or not exist at all) so flings don't
slam into the pitch clamp. Recommend: damp pitch velocity by an extra factor of
~2, and zero it entirely on clamp contact.

**Idle auto-rotation** (a very slow eastward drift when untouched) is a nice
touch that makes the globe feel alive on a launch screen. If added: ~0.6°/s,
starting after 3s of no interaction, ramping in over 1s, and hard-disabled under
Reduce Motion.

---

## 14. Recenter

The button in the screenshot's lower-right cluster. Behaviour: fly the globe so
the user's location is centered, and simultaneously ease the zoom to a standard
"located" distance.

```swift
func recenter(on coord: Coordinate, animated: Bool = true) {
    let target = GlobeMath.orientation(centering: coord.lat, coord.lon)
    // decompose back to yaw/pitch so the clamp and gestures stay consistent
    let targetYaw   = Float(-coord.lon * .pi / 180)
    let targetPitch = Float( coord.lat * .pi / 180)
    ...
}
```

Three things that are easy to get wrong:

1. **Take the short way around.** Yaw is periodic. Naively animating from
   `yaw = 170°` to `yaw = −170°` sweeps 340° the wrong way. Wrap the delta into
   `(−π, π]` before animating:
   ```swift
   var d = targetYaw - currentYaw
   d = d.truncatingRemainder(dividingBy: 2 * .pi)
   if d >  .pi { d -= 2 * .pi }
   if d < -.pi { d += 2 * .pi }
   ```
   Slerping the quaternions directly would also take the short path, but then we
   lose the yaw/pitch clamp during the flight — hence animating the scalars.

2. **Animate yaw/pitch/distance together on one curve.** Use a single normalized
   progress `t ∈ [0,1]` driven by the same `GlobeRotationSystem`, with an
   ease-in-out curve. Independent animations desynchronize and the motion reads
   as sloppy. Duration ~0.9s, or scale with angular distance
   (`0.5s + 0.5s × (angle/π)`) so short hops feel snappy.

3. **Cancel on touch.** Any drag or pinch during the flight kills it immediately
   and hands control back. Nothing feels worse than a control that ignores you.

Optional flourish matching Apple Maps: zoom *out* slightly during the first half
of the flight and back in during the second (an arc, not a straight dolly). Adds
a lot of perceived quality for one extra term in the distance curve:
`d(t) = lerp(d₀, d₁, t) × (1 + 0.35 × sin(πt))`.

If location isn't available, the recenter button should be visibly disabled, not
silently inert. Tapping it while permission is undetermined should trigger the
permission prompt.

---

## 15. Location markers

### Placement

```swift
let n = GlobeMath.unitVector(lat: coord.lat, lon: coord.lon)      // local space
let worldNormal = model.orientation.act(n)                        // after rotation
let worldPos    = worldNormal * globeRadius                       // sphere at origin
```

### 2D or 3D?

Per §1 item 7, the marker in the screenshot is a perfect circle even when it
would be foreshortened — it's billboarded. Two implementations:

**SwiftUI overlay via projection (recommended).** Project `worldPos` with §12,
draw the dot in SwiftUI. Advantages: crisp at any zoom (vector, not texture),
pulse/halo animation is a one-liner, tap targets and accessibility labels come
free, and it's trivially restyled. This is what makes the marker look *native*
rather than *rendered*.

**3D entity with a billboard system.** A small disc `ModelEntity` at
`worldPos × 1.002` with an `UnlitMaterial`, orientation updated each frame to
face the camera by a `System`. (`BillboardComponent` is visionOS-only, so this
is manual.) Gets occlusion and depth sorting for free but needs its own texture
and looks softer.

**Recommendation: SwiftUI overlay,** with explicit occlusion (below). Publish the
projected points from the same `System` that drives inertia, so they update in
lockstep with the render rather than a frame behind.

### Horizon occlusion — the detail everyone gets wrong

The naive test is `dot(worldNormal, [0,0,1]) > 0` — "is it on the near
hemisphere". That's the *orthographic* horizon and it's wrong for a perspective
camera: points just past the geometric edge are still visible under it, and
points just inside it are already hidden. With the camera at distance `d` and
sphere radius `R`, the true horizon is where the view ray is tangent:

```
visible  ⟺  dot(worldNormal, normalize(cameraPos)) > R / d
```

With `cameraPos = (0,0,d)` this reduces to `worldNormal.z > R/d`. At the default
`d = 3.2`, `R = 1.0`, the cutoff is `0.3125` — about **18° inside** the apparent
edge. Using `> 0` instead would leave markers visible floating over the limb.

Fade rather than pop, over ~6° of arc:

```swift
let c = worldNormal.z
let cutoff = globeRadius / cameraDistance
let opacity = smoothstep(cutoff, cutoff + 0.10, c)     // 0 at horizon → 1 inside
```

### Visual treatment

Matching the screenshot: a `Circle` in system blue, ~11pt radius, with a 3pt
white stroke, a soft shadow, and a slow pulsing halo:

```swift
ZStack {
    Circle().fill(.blue.opacity(0.25)).frame(width: 44).scaleEffect(pulse)
    Circle().fill(.white).frame(width: 22)
    Circle().fill(.blue).frame(width: 16)
}
.opacity(occlusionOpacity)
```

Scale the whole marker slightly with `1/cameraDistance` so it doesn't dominate
when zoomed out — but clamp it, a marker that shrinks to nothing is worse than
one that's a bit big.

### Core Location

- `INFOPLIST_KEY_NSLocationWhenInUseUsageDescription` as a build setting (the
  target uses `GENERATE_INFOPLIST_FILE = YES`, so there is no `Info.plist` file
  to edit — see `project.pbxproj:230`).
- `CLLocationManager` with `desiredAccuracy = kCLLocationAccuracyKilometer`.
  A globe cannot resolve better than ~50km per pixel; requesting best accuracy
  wastes battery for zero visible benefit.
- Request `whenInUse` on first tap of recenter, not at launch. An unexplained
  permission prompt on cold start is a conversion killer.
- Wrap in a `LocationProvider` protocol with a `PreviewLocationProvider` returning
  a fixed coordinate, so SwiftUI previews and tests never touch CoreLocation.

---

## 16. State management and the SwiftUI ↔ RealityKit bridge

```swift
@Observable @MainActor
final class GlobeViewModel {
    var yaw: Float = 0
    var pitch: Float = 0
    var cameraDistance: Float = 3.2
    var markers: [Marker] = []
    var viewport: CGSize = .zero
    private(set) var projectedMarkers: [ProjectedMarker] = []   // written per frame
}
```

### The pitfall

`RealityView`'s `update:` closure runs when observed SwiftUI state changes — it is
**not** a per-frame hook. Driving 60fps rotation by mutating `@Observable` state
and letting `update:` apply it means a full SwiftUI invalidation per frame. That
works, and on a simple view tree it's even fine, but it couples render smoothness
to view-tree complexity and will degrade as the screen gains chrome.

### The bridge

- **Gestures** (discrete, user-driven) → mutate the view model → `update:` applies
  the new orientation. Fine, these are already at input rate.
- **Inertia and recenter animations** (continuous) → run inside
  `GlobeRotationSystem`, mutating `globeRoot.orientation` **directly**, and write
  the resulting yaw/pitch back to the view model only when it settles (plus once
  per frame for `projectedMarkers`, which the overlay genuinely needs).

Keep one source of truth for orientation, and be explicit about who owns it at
any moment: gestures own it during a drag, the system owns it during inertia and
recenter, and the view model is the durable record between the two. Write this
down as a comment at the top of `GlobeViewModel` — it is the thing a future
reader will get wrong.

### Entity lookup

Don't search the scene graph by name every update. Capture entity references in
the `make:` closure into a small `GlobeSceneHandles` box held by the view model.

### Lifecycle

- Build the scene once in `make:`; never rebuild it in `update:`.
- `TextureResource` and `MeshResource` are reference types backed by GPU
  resources — hold them, don't recreate them.
- On `.onDisappear`, stop the inertia system (or the update loop keeps ticking
  invisibly and draining battery on a backgrounded screen).

---

## 17. Performance budget

Target: **60fps sustained on iPhone 12 and later**, 120fps on ProMotion, with
thermal headroom (this is a decorative background, it must not warm the device).

| Item | Budget | Notes |
|---|---|---|
| Draw calls | ≤ 4 | globe, graticule, atmosphere shell (if enabled), markers |
| Triangles | ~66k | one 128×256 sphere + one overlay sphere |
| Texture memory | ≤ 12 MB | 4K ASTC day map + 2K graticule, both with mips |
| Fragment cost | low | unlit, no shadows, no post-processing |
| SwiftUI invalidations | ≤ 1/frame | and ideally 0 during inertia — see §16 |

**Things to actually measure, not assume:**

1. **Instruments → Metal System Trace** for GPU frame time. If the globe is
   costing >4ms, the texture sampling at the limb is the first suspect.
2. **Memory footprint at launch** — the 4K texture decode is the peak. Confirm it
   doesn't spike above ~60MB transiently, which is where a jetsam on older
   devices starts to become plausible if the app grows around it.
3. **`ENABLE_METAL_API_VALIDATION`** in the Debug scheme catches winding and
   sampler mistakes early. Turn it *off* before measuring performance.
4. **Energy Log** for a 60s idle-with-globe-visible session. If idle
   auto-rotation costs meaningful energy, gate it more aggressively.

**Known simulator caveats.** RealityKit non-AR rendering does work in the iOS 18
simulator with Xcode 16, but there are reports of crashes on `ARView` presentation
and general instability
([forums](https://developer.apple.com/forums/thread/771281)). Plan on doing all
visual and performance verification on device, and treat the simulator as
suitable only for layout and the non-3D layers. Budget for this in the schedule —
it means the person building this needs a physical device.

---

## 18. Accessibility and motion

Easy to skip and genuinely wrong to skip on a screen this animated.

- **`@Environment(\.accessibilityReduceMotion)`** — disable idle auto-rotation
  and star twinkle; replace the recenter *flight* with a cross-fade or an instant
  cut. Inertia can stay (it's direct manipulation, not gratuitous animation), but
  shorten the decay.
- **VoiceOver.** The `RealityView` itself is meaningless to a screen reader. Give
  the container an `.accessibilityElement(children: .ignore)` with a label like
  "Globe, centered on \(placeName)", and expose the markers as separate
  accessibility elements with their own labels. Add `.accessibilityAdjustableAction`
  so rotor swipes can spin the globe.
- **Contrast.** The marker's white ring against a mid-blue ocean is fine; against
  a bright landmass it is marginal. Add a subtle dark outer shadow so it holds up
  everywhere.
- **Dark mode** is a non-issue — the scene is black in both — but make sure the
  chrome (search bar, buttons) uses materials that adapt, since those *are*
  theme-sensitive.
- **Dynamic Type** for any labels or chrome. The globe itself is fixed-size,
  which is correct.

---

## 19. Testing strategy

The reason `GlobeMath.swift` imports only `simd` is so that the hard part — the
math — is testable without a renderer, a device, or a screenshot harness.

**A test target does not exist yet.** Adding one requires editing
`project.pbxproj` (the file-system-synchronized groups only auto-include files
into *existing* targets). This is on the roadmap already
(`plan/roadmap.md:29`) and should be done as part of this work, not after.

### Unit tests — `GlobeMathTests` (Swift Testing)

| Test | Assertion |
|---|---|
| `unitVector` cardinal points | (0,0)→(0,0,1); (0,90)→(1,0,0); (90,·)→(0,1,0); (0,180)→(0,0,−1) |
| Round trip | `coordinate(from: unitVector(lat, lon)) ≈ (lat, lon)` over a grid, excluding poles where lon is undefined |
| Unit length | `\|unitVector(lat,lon)\| == 1` for all sampled inputs |
| `orientation(centering:)` | `q.act(unitVector(lat,lon)) ≈ (0,0,1)` for a grid of coordinates — this is the recenter correctness proof |
| `centeredCoordinate` round trip | `centeredCoordinate(for: orientation(centering: c)) ≈ c` |
| UV mapping | (0,−180)→(0,0.5); (0,180)→(1,0.5); (90,0)→(0.5,0); (−90,0)→(0.5,1) |
| Yaw shortest path | 170° → −170° yields a delta of +20°, not −340° |
| Pitch clamp | Drag deltas beyond ±85° saturate and never wrap |
| `project` | A point at the globe's center projects to the viewport center; a point at 90° lon projects to the computed disc edge |
| Horizon cutoff | At `d = 3.2, R = 1`, a marker at `n.z = 0.30` is hidden and one at `0.35` is visible |
| Mesh integrity | Generated sphere has `(rings+1)×(segments+1)` verts, all at distance `R` from origin, and seam column UVs are exactly 0.0 and 1.0 |

### Snapshot tests

Deterministic given the seeded starfield and fixed camera. Worth adding for the
2D layers (starfield, atmosphere gradient, marker) since those are pure SwiftUI.
**Not** worth it for the RealityKit render — GPU output varies across devices and
OS versions, and the tests will be flaky enough to get deleted.

### Manual verification checklist

Because the 3D part can only really be checked by eye, keep an explicit list and
run it on device before each milestone sign-off:

- [ ] Continents are not mirrored (North America is west of the Atlantic)
- [ ] No visible seam at the date line, at any zoom
- [ ] No texture shimmer at the limb while spinning (mipmaps + aniso working)
- [ ] Poles are not pinched or smeared
- [ ] Drag direction matches finger, at every zoom level
- [ ] Pitch clamps without jitter; rubber-band springs back cleanly
- [ ] Fling decays smoothly, no jerk at handoff from gesture to inertia
- [ ] Recenter takes the short way around from every starting orientation
- [ ] Marker disappears at the correct horizon, fading not popping
- [ ] Atmosphere stays registered to the limb across the full zoom range
- [ ] Rotating the device re-registers the atmosphere and markers correctly
- [ ] Reduce Motion disables auto-rotation and the recenter flight

---

## 20. File layout

The target uses `fileSystemSynchronizedGroups` (`project.pbxproj:63`), so new
`.swift` files and subfolders under `src/Trace/Trace/` are picked up
automatically. No `pbxproj` edits for source files — only for the new test
target.

```
src/Trace/
├── Trace/
│   ├── TraceApp.swift
│   ├── ContentView.swift              → hosts GlobeScreen
│   ├── Globe/
│   │   ├── GlobeScreen.swift          SwiftUI: the ZStack layer cake
│   │   ├── GlobeRealityView.swift     RealityView make/update, gestures
│   │   ├── GlobeScene.swift           entity construction, GlobeSceneHandles
│   │   ├── GlobeSphereMesh.swift      MeshDescriptor UV sphere (§6)
│   │   ├── GlobeMaterials.swift       texture loading, material construction
│   │   ├── GlobeCamera.swift          camera rig, disc radius, projection (§12)
│   │   ├── GlobeViewModel.swift       @Observable state (§16)
│   │   ├── Systems/
│   │   │   └── GlobeRotationSystem.swift   inertia + recenter tick (§13, §14)
│   │   ├── Layers/
│   │   │   ├── StarfieldView.swift    (§10)
│   │   │   ├── AtmosphereGlow.swift   (§9)
│   │   │   └── MarkerOverlay.swift    (§15)
│   │   └── Controls/
│   │       └── GlobeControls.swift    recenter button, chrome
│   ├── Core/
│   │   ├── GlobeMath.swift            pure simd — the tested core (§5)
│   │   └── Coordinate.swift           lat/lon value type
│   ├── Location/
│   │   ├── LocationProvider.swift     protocol + CoreLocation impl
│   │   └── PreviewLocationProvider.swift
│   ├── Resources/
│   │   ├── Globe.xcassets/            earth day map, graticule overlay
│   │   └── ATTRIBUTION.md             imagery sources + licenses
│   └── Assets.xcassets/
└── TraceTests/                        ← NEW TARGET (requires pbxproj edit)
    ├── GlobeMathTests.swift
    ├── GlobeSphereMeshTests.swift
    └── GlobeInteractionTests.swift
```

---

## 21. Milestones

Each milestone is independently demoable and independently revertible. Do not
start the next before the previous one's acceptance criteria pass on device.

### M0 — Groundwork (½ day)
- Bump `IPHONEOS_DEPLOYMENT_TARGET` to 18.0, `SWIFT_VERSION` to 6.0
- Add the `TraceTests` target
- Add `GlobeMath.swift` + `Coordinate.swift` with the full §5 API
- Write `GlobeMathTests` — **all of them, before any rendering code**

*Acceptance:* `xcodebuild test` green. Zero RealityKit code so far, and the
hardest part of the feature is already proven correct.

### M1 — A textured sphere on screen (1 day)
- `GlobeSphereMesh` + `GlobeSphereMeshTests`
- 4K day texture in the asset catalog, ASTC, mips, aniso 16
- `UnlitMaterial`, fixed `PerspectiveCamera` at `d = 3.2`
- `RealityView` embedded in `ContentView`

*Acceptance:* Earth renders, correctly oriented, no seam, no shimmer while
manually stepping the orientation. Verified on device.

### M2 — Interaction (1–1.5 days)
- Drag → yaw/pitch with clamp and rubber-band
- Pinch → camera distance with clamp
- `GlobeRotationSystem` + inertia

*Acceptance:* Feels good in the hand. Manual checklist items 5–7 pass. This is
the milestone most likely to need a round of feel-tuning — budget for it.

### M3 — The look (1–1.5 days)
- `StarfieldView` with seeded RNG
- `AtmosphereGlow` front and back layers, registered to the computed disc radius
- Graticule overlay sphere
- Colour-grade the Earth texture to match the reference

*Acceptance:* Side-by-side with the reference screenshot at the same zoom is
convincingly close.

### M4 — Location (1 day)
- `LocationProvider` + permission flow
- Marker projection, billboarding, horizon fade
- Recenter button with the shortest-path arc flight

*Acceptance:* Blue dot lands on the right place (verify against a known
coordinate — e.g. confirm it sits on the correct city), hides at the right
horizon, and recenter flies the short way from every starting orientation.

### M5 — Polish (1 day)
- Reduce Motion, VoiceOver, Dynamic Type
- Idle auto-rotation
- Instruments pass against the §17 budget
- `ATTRIBUTION.md`, and update `plan/roadmap.md`

*Acceptance:* Full manual checklist passes; 60fps sustained; no thermal warning
after 5 minutes.

**Total: roughly 5–6 focused days**, assuming a physical device is available and
the imagery colour-grading doesn't turn into an art project. M3 is the milestone
most likely to expand.

---

## 22. Risks and open questions

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| iOS 18 minimum is unacceptable | Low | High | Appendix A fallback (`ARView` + `UIViewRepresentable`), costs ~1 day |
| Simulator instability blocks the dev loop | Medium | Medium | Assume device-only for 3D verification; keep 2D layers previewable |
| No documented world→screen API on `RealityView` | Certain | Low | We compute it ourselves (§12) — cheap given the fixed camera, and testable |
| Screen-space atmosphere breaks if the camera ever moves off-axis | Low | Medium | Fixed camera is an architectural invariant; if it changes, switch to the §9 Option B shell |
| Texture doesn't match the reference's stylization | Medium | Low | Budget a colour-grading pass; the source imagery is the variable, not the code |
| Gesture feel needs many iterations | Medium | Low | Expose gain/decay/clamp constants in one `GlobeTuning` struct so they're adjustable without hunting |
| 4K texture memory on older devices | Low | Medium | Measure at M1; fall back to 2K if the footprint is uncomfortable |

**Open questions for the project owner:**

1. **Deployment target** — OK to go iOS 18? (§3) This is the only decision that
   blocks starting.
2. **Is this the app's main screen, or a background element?** Changes the
   performance budget substantially — a background globe behind a scrolling
   feed has a much tighter frame budget than a full-screen one.
3. **Are place labels needed?** §11 recommends deferring them; confirm that's
   acceptable, because they're the largest single chunk of remaining scope.
4. **Should the globe show anything Trace-specific** — traces, paths, visited
   places? If so, the marker system (§15) should be designed for *n* markers plus
   great-circle arcs from the start rather than a single "you are here" dot.
   Arcs specifically are much cheaper to design in now than to retrofit.
5. **Day/night terminator** — the reference doesn't have one. Confirm we don't
   want one, since it's the difference between `UnlitMaterial` (§8 v1) and a
   Reality Composer Pro shader graph pipeline (§8 v3).

---

## Appendix A — iOS 17 fallback via ARView

If the deployment target must stay at 17.0:

```swift
struct GlobeARViewRepresentable: UIViewRepresentable {
    @Bindable var model: GlobeViewModel

    func makeUIView(context: Context) -> ARView {
        let view = ARView(frame: .zero,
                          cameraMode: .nonAR,
                          automaticallyConfigureSession: false)
        view.environment.background = .color(.clear)
        view.renderOptions = [.disableMotionBlur, .disableDepthOfField,
                              .disableGroundingShadows, .disableAREnvironmentLighting]
        GlobeScene.build(into: view.scene, handles: context.coordinator.handles)
        return view
    }

    func updateUIView(_ view: ARView, context: Context) {
        context.coordinator.handles.globeRoot.orientation = model.orientation
        context.coordinator.handles.camera.position = [0, 0, model.cameraDistance]
    }
}
```

Differences from the `RealityView` path:

- **`ARView.project(_:) -> CGPoint?` is public**, so §12's hand-rolled projection
  becomes optional (though still worth keeping for testability).
- Everything else — mesh, materials, math, gestures, systems — is **identical**.
  `Scene`, `Entity`, `System`, `MeshResource` are all shared API.
- Must set `renderOptions` explicitly to disable AR-oriented post-processing that
  `RealityView` skips by default in virtual-camera mode.
- Must set `environment.background` to clear (or black) — the default is camera
  feed, which in `.nonAR` mode is undefined.

The migration cost from this to `RealityView` later is one file. This is a
genuinely low-risk fallback, not a dead end.

---

## Appendix B — references

**Apple**
- [RealityView](https://developer.apple.com/documentation/realitykit/realityview) — iOS 18.0+, macOS 15.0+, visionOS 1.0+
- [MeshResource](https://developer.apple.com/documentation/RealityKit/MeshResource) / `MeshDescriptor`
- [ShaderGraphMaterial](https://developer.apple.com/documentation/RealityKit/ShaderGraphMaterial)
- [RealityCoordinateSpaceConverting](https://developer.apple.com/documentation/realitykit/realitycoordinatespaceconverting)
- [Explore advanced rendering with RealityKit 2 — WWDC21](https://developer.apple.com/videos/play/wwdc2021/10075/) — `CustomMaterial`
- [Better together: SwiftUI and RealityKit — WWDC25](https://developer.apple.com/videos/play/wwdc2025/274/)
- [What's new in RealityKit — WWDC25](https://developer.apple.com/videos/play/wwdc2025/287/)

**Known issues worth knowing before starting**
- [Orbit camera latches onto stale `cameraTarget` mid-gesture](https://developer.apple.com/forums/thread/825543) — §4's argument against `.orbit`
- [UV seam on `generateSphere` with equirectangular textures](https://developer.apple.com/forums/thread/816453) — §6's argument for a custom mesh
- [RealityKit simulator crashes on iOS 18](https://developer.apple.com/forums/thread/771281) — §17's device-only verification note
- [`CustomMaterial` unavailable on visionOS](https://developer.apple.com/forums/thread/732672) — §8's argument for `ShaderGraphMaterial`

**Imagery**
- [NASA Visible Earth — Blue Marble](https://visibleearth.nasa.gov/) — public domain
- [Natural Earth III textures](https://www.shadedrelief.com/natural3/pages/textures.html) — public domain, closest to the reference's style
- [Solar System Scope textures](https://www.solarsystemscope.com/textures/) — CC-BY 4.0, attribution required

**Background**
- [Non-AR RealityKit setup](https://rozengain.medium.com/quick-realitykit-tutorial-programmatic-non-ar-setup-cafaf61e9884)
- [Displaying 3D objects with RealityView on iOS, iPadOS and macOS](https://www.createwithswift.com/displaying-3d-objects-with-realityview-on-ios-ipados-and-macos/)
