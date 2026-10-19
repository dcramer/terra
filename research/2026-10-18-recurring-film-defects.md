# Recurring film defects: what came back, how often, and how engines stop it

2026-10-18. The question: which defects in the trailer, lake, castle, settlement, mountain, troll and forest films came back across rounds or films, how often, what each cost, what in the code lets it happen, and how shipped engines and games keep each right by construction and check it automatically. It is the evidence for [docs/scene-rules.md](../docs/scene-rules.md).

## Answer

- **Eleven kinds of defect recurred.** Colour casts and haze (about 35 of 60 reviews), the subject hidden or cropped (about 30), far detail losing its texture (about 28) and fire's colour (21) lead; wildlife posing came back in six lake rounds running; fire was a top gap in every one of the castle's eleven reviewed rounds.
- **Most were found by a person reading frames**, after renders of 20 to 75 minutes. The tools that would have caught several exist (`pops.ts`, `look.ts`, `collide.ts`, `life`, `--trace`) but run on the forest's bench route, not on films.
- **Several are by construction in the code today**: encounters are staged where the camera sees them (D384), so animals appear in view; creatures know trunks only to climb (`play/creatures/mind.rs`), so nothing steers round them; Bevy 0.20 drops a directional-light shadow caster outside the camera's visibility range (`bevy_light` `check_dir_light_mesh_visibility`) and the cascades end at 300 m (`quality.rs`); far land is drawn in the sky shader (`sky.wesl` `far_land`), so it can neither cast a shadow nor be reflected as geometry; the lake mirrors only what is on screen (`lake.wesl` `on_screen`) and fell back to a teal probe; fire's colour passes through the grade's sectional saturation (0.2 in the shadows at dusk, D302) and was patched per film with lifts past the tonemapper's knee (1.5, 1.6, 1.7, D377); `trailer.ts` judges staleness by three mtimes and needs 7,000 MB for every place.
- **Engines own each rule in one system** (below), and studios check with automated screenshot and budget tests (Unreal's Automation screenshot comparison, Unity's Graphics Test Framework, Unreal's `memreport` and streaming pool budgets).

## How the count was made

Sources: every `review.md` (60) and `HANDOFF.md` (39) under `~/.cache/terra-review/` (castle2 r1-r12, dragon r4 and r7, final/lake-troll r2-r12, final/settlement2-12, settlement c1-c11 and its later cuts, mountain4-7, troll r1-r5, forest-life r1-r2) and `final/LESSONS.md`. Each class was searched by its words, then its matches read; a review counts once per class. Counts are about ±3: a review may name a defect in words the search missed, and a few matches were the reviewer's praise ("no pop-in was seen").

## The catalogue, with evidence

| Defect | Reviews | Where | Lost |
|---|---|---|---|
| Colour cast, veil, tint | ~35 | sepia or yellow-grey veil in settlement c1-c11, s3-s5, d1, d2, e1, final s3-s12 ("milky", shaft stripes s9-s12); magenta dusk lake r8-r10 ("peaks 111,72,111"); salmon or pink stone castle r2, r4-r7, r9; blue-metal roofs in nine settlement cuts; lime unshadowed crowns s4, s5 | lake r11 unreviewed on a sky artefact (Bevy's grade crossfading shadows into highlights, fixed in r12) |
| Subject hidden or cropped, camera in geometry | ~30 | town cut by the left and bottom edges in 17 settlement reviews; dragon behind rubble 6 s (castle r2), lost in smoke (r4-r6); troll's contacts hidden behind bodies (r2-r5); the end shot "from inside tall grass cards" (troll r1) | castle r3's hover behind the SE tower roof, a 42-minute rerender (LESSONS) |
| Far detail lost | ~28 | smeared, blotchy roofs at distance (settlement c1-c11); "flat cream boxes" (castle r1, r2, r4-r6, r9); houses lose textures from 15 s (s5-s7); wall and castle "cream cardboard" (s9, s10); the far hull's per-cube tones an egg crate (s11) | — |
| Fire | 21 | castle r1-r12 every review: stickers, olive (r5), khaki (r6), salmon tissue (r7), mustard tube (r8), salmon tube (r9), traffic-cone hose with lime (r10), khaki cone, 0-2% yellow (r11), pastel salmon walls (r12); lake r4-r12: peach see-through cards, "two flat cards" unchanged r9-r12; dragon r4, r7 | lake r2 and r3 ended unreviewed past 200k context chasing a campfire that drew nothing; castle r6 needed three probes to find AutoExposure behind the olive; castle plateaued 6 → 6.5 → 6 → 6 |
| Reflections and shadows | 17 | lake water reflects neither sky nor shore (r6), teal reflections and "SSR-miss fallback" (r8, r9), water "no longer belongs to the sky" (r10), "reflections do not mirror their sources" (r12); the settlement's lake (11 cuts); the castle going dark at its reveal, cause unfound (s12) | — |
| Appearing whole or for a frame | 11 | gravel yard and stalls pop in finished (s3), a roof on in 0.2 s (s3), single pieces pop (s8), haze off for one frame and decals vanish (s4), a one-frame pop (c1), one-frame flashes on camera cuts (castle r1, r2) | — |
| Destruction | 10 | "towers sink in place into a neat heap" (dragon r7), "a lattice of hollow cubes" (dragon r4), "rigid voxel stacks" castle r2, r4-r10 ("same since r5"); then `fragments::topple`, a scripted rigid tip, which the developer calls unrealistic | — |
| Wildlife | 8 | boars stare (lake r6), fox and stag "staring at the camera" (r7, r8), "performs for the runner" (r9, r10), the heron flushing at the open (r4), a crossing through the walker's line (forest-life r2); gone in r12 by a 1.2 s look cap; the developer then saw animals pop in and a fox run through a tree, which no review caught | six lake rounds |
| Layout | 5 | castle "150 px wide off to the side" at the forest edge (s9), small and low on the right (s11, s12); empty meadow inside the wall (s9-s12); "one wall filling the sky" (mountain6 r1); the developer: castle within the walls, mountains smaller | — |
| Out of memory | 5 renders | the dragon film at its fire peak, three times (50 min); settlement c4 at detail 1.9 and 1.6 (45 min); s9 peaked at 9,076 MiB of 10 GiB | about 95 min |
| Stale renders | 3 | `trailer.ts` reused a clip when only the scene changed (castle r3, 20 min); a render crashed on a release binary older than the pulled data (troll); assets hot-reloaded mid-film from another worktree's edit (settlement) | about 1 h |

A hung probe held the GPU for 27 minutes with no output (castle r6), which a watchdog ends.

## How engines and games keep each right

- **Spawning unseen.** Open-world population systems (Grand Theft Auto's ambient peds and traffic) place ambient life outside the view and walk it in. What must appear in view fades: Bevy's visibility-range dither (used by D240), Godot 4's `GeometryInstance3D.visibility_range_fade_mode` (MIT), Unreal's dithered LOD transition.
- **Steering.** Craig Reynolds' steering behaviours (1999) are the base; Recast Navigation's DetourCrowd (zlib) ships `dtObstacleAvoidanceQuery`, sampled velocity avoidance against circles and segments, a direct fit for trunks; Godot's avoidance is RVO2 (Apache-2.0).
- **Casters apart from the view.** Unreal: Far Shadow cascades (`bCastFarShadow`), distance-field shadows, and ray tracing's own culling (`r.RayTracing.Culling`, `.Radius`); Unity HDRP's ray tracing settings "Extend Shadow Culling" and "Extend Camera Culling". Bevy 0.20 culls each cascade by its own frustum, but also by the camera's `VisibilityRange` (read in `bevy_light` 0.20 `lib.rs`), so a range set for drawing ends the shadow too.
- **Reflections.** Unreal's SSR falls back to reflection captures and the sky light; its Planar Reflection draws the scene mirrored; water in many shipped games is a planar or captured reflection, not screen space alone.
- **Fire.** Blackbody shading of temperature (Unreal's BlackBody material node, Houdini Pyro, EmberGen); emissive in absolute luminance with exposure compensation, so it holds across exposures (Lagarde and de Rousiers, "Moving Frostbite to PBR", 2014). Our own measurements (castle r11, r12): a flamethrower frame is about 25% yellow; ours was 0-2% under the grade, 12-20% after lifting past the knee.
- **Cameras.** Spring arms that probe for collision (Unreal's SpringArm, Godot's `SpringArm3D`, MIT); Cinemachine's Deoccluder and Decollider (documentation only). A camera cut resets temporal history (Unreal's camera-cut flag; Bevy's `TemporalAntiAliasing::reset`).
- **Far detail.** Unreal's HLOD and Simplygon bake a proxy's materials from its sources.
- **Destruction.** Unreal's Chaos geometry collections break clusters at damage thresholds; Teardown's voxels become rigid bodies when cut free; Red Faction Guerrilla computes structural stress so buildings fail where they are weak (GDC talks). Masonry has almost no tensile strength, so a leaning tower breaks into courses as it falls rather than tipping whole.
- **Town layout.** Real burgs keep the castle inside the town walls or as part of them (Carcassonne, Conwy). watabou's town generator is GPL: documentation only (D196).
- **Budgets and freshness.** Unreal's texture streaming pool and `memreport` budgets; render and QA builds stamp their build into what they make.

## What is already ours

`pops.ts` (pop and flicker by frame differencing, D230), `lodcheck`, `look.ts` and `style.json` (bands per place, D356), `collide.ts` (the player against meshes, D267), `life` (people stuck or through a wall, D326), `diverge.ts` with `--trace` (the play state a frame), `artifacts.ts` (building's junk on frames, D287). None runs on a film today.
