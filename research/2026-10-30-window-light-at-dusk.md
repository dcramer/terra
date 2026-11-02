# How shipped games light a town's windows at dusk, and ours

2026-10-30, settlement round 8. Every judge of the settlement at dusk (brief `settlement-dusk`, about 3.1 of 10) named the same fault: flat yellow window squares, with no glow and no warm spill on the walls or the ground. This note is what games do about it and what we drew.

## What the games do

Primary means a source by the developers or a frame capture. Inferred means read from the games' own frames and the common practice of their engines.

- **The glass**: an emissive pane, brighter than anything lit round it, so the tonemapper and the bloom treat it as a light. All of them (inferred). Ours: `GlowMaterial` panes, tonemapped as a flame (D415).
- **Bloom**: the frame's own blur round anything over the white point. It cannot make a pane of a few pixels read: a 4 px pane blooms by about as much again. Ours: Bevy's `Bloom` at 0.03, kept low because more washed the middle distance flat (scene.rs).
- **Coronas, or far-light sprites**: GTA V draws "every single small light spot you can see" at night as "a quad rendered with a small 32x32 texture", "heavily batched into instanced geometry" (primary: Adrian Courrèges, GTA V graphics study part 2, 2015). RDR2 runs on the same RAGE engine, and its towns' windows glow the same way from afar (inferred). Ours: the halo card, `window_halo.wesl` (round 2, D127).
- **Light on what is round the window**: GTA V draws its many lights deferred, as light volumes: "for each light a mesh is drawn", shaped "to touch all the pixels inside the light halo so a pixel shader can be invoked", so "a large number of lights can be rendered with pixel shaders invoked as little as possible" (primary, the same study). Elsewhere the same job is done by baked lightmaps, as in The Witcher 3's and most open worlds' fixed buildings, or by projected decals and lights with a window-shaped cookie. KCD2's CryEngine has SVOGI pick up emissive surfaces (inferred). A town's few lights nearest the eye are real lights with shadows. The many far ones are not.
- **How many**: a lit town in these games shows tens to a few hundred lit windows. GTA V's light sprites run to "tens of thousands of polygons". Ours: the settlement has 34 to 83 lit homes, about three windows lit each (`WINDOWS:` in the log).

## What we tried (settlement rounds 8 and 9, not landed)

- **A light volume a lit window** (`window_spill.wesl`): a box round the window, drawn by its far faces where they lie behind the depth prepass (Bevy's reversed depth, `CompareFunction::Less`). Each pixel it reaches is placed from the prepass depth, with the normal from the normal prepass or from the place's neighbours. It is lit there twice:
  - by the wall's pool round the window, as round 32's card drew it;
  - by the light thrown out of the hole: most straight out, none along the wall, inverse square to nothing at 6 m.
  
  It replaces the pool card one for one, so the town gains no entity, no Bevy light and no draw call. Bevy's own point lights would each be clustered and shaded for every pixel near them, and town-dusk already misses its 99th percentile.
- **A wider halo**: least 0.035 of the view's height, not 0.02, its skirt half the light. Round 6's glow showed only in a 1.5x crop.
- **Brighter than physics**: a hearth seen through a pane lights a street far less than these do at dusk, when the land is still bright. At 0.3 of the pane's glow, a metre out, the pool added about 1.5% to the street's level and did not show. The game lights it as games do, so the town reads lived in.

What it measured (RTX 4090, 4 fresh judges a build on the settlement-dusk brief, its end and t15 views at 18.7 h):

- **Round 8**, volumes reaching 6 m: 3.67 against main's 3.50 (+0.17, p 0.023), but town-dusk's 99th percentile 0.3-0.6 ms worse on every calm run.
- **Round 9**, the boxes about four times smaller (4 m out, 3 m to the sides, 1.5 m over), faded at their sides and top, and an early out before the normal load: the cost is gone (medians 10.01-10.10 ms against main's 10.03-10.10, 99th 13.25-13.60 against 13.09-14.19, two jobs of two runs a side). The windows' crops warmed by 16-17% (r - b, 0.050 to 0.058 at t15). The judges saw no difference: 3.74 against main's 3.75 (p 0.85), and every one of them, on both builds, still named the windows "flat orange squares with no glow or spill". Main had moved on between the rounds (its own 3.50 became 3.75), so round 8's gain was not the change.

At the judged distances, a pane a few pixels across, neither the pool nor the wider halo reads. So it was not landed; the patch is kept in the handoff (settlement-j9).

## What is left

- **The land is too bright for the hour.** Judges call the meadow "lit bright as by day" at 18.7 h. Window light reads in contrast with the land round it, so that is the next lever (FOUND in the settlement-j8 handoff).
- **The windows are all alike.** Every judge names "one tone and size on every house": panes that differ in brightness and warmth within and between homes, curtains or mullions.
