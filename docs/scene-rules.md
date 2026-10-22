# Scene rules

Status: **proposal, 2026-10-18**, for the developers to read before anything is built (D49). The systems and checks are technical and mine to propose; the questions at the end are design, Open, for the developers. Nothing here changes a decision yet. The evidence, with counts and sources, is [research/2026-10-18-recurring-film-defects.md](../research/2026-10-18-recurring-film-defects.md).

Each defect the films keep showing gets two things: a system that owns its rule, so every scene gets it right without being tuned, and an automatic check that fails the film or still before a person sees it.

## Why

The developer, after the lake, castle and settlement films: "BIG theme here is whether we can capture the rules in some static systems/modules that we could reuse so we don't have to always fix those problems. im pretty fucking unhappy right now." His latest notes: "Lake is almost there, fire looks ass, animals pop into existence, fox runs through tree"; "castle needs to be within the city walls, the mountains can be smaller, the terrain must be rendered even if its not fully visible because reflections/ray tracing get fucked, same with shadows"; "destruction of castle went wonky where it topples over it feels unrealistic too, i think this specific part may have looked more realistic earlier".

Across 60 reviews and 39 handoffs, the same gaps came back round after round. A film's agent fixed each in its own scene, with its own constants (a fire's "lift past the knee", a stare cap, a camera nudge), and the next scene broke it again. The castle's fire was a top gap in all eleven of its reviewed rounds; animals posed for the camera in six lake rounds in a row; the settlement's ground haze in nearly every cut. Most of these were caught by a reviewer reading frames, 15 to 40 minutes after a render that took 20 to 75.

## Constraints

- **A rule lives in the system that makes the thing** (Systems apart, D58): wildlife owns when an animal appears, the camera rig owns where a camera may stand, the town owns where a castle goes. A scene states what it wants; it never re-tunes a rule.
- **A check reads facts, not guesses** (D69): where it can, it reads what the game knows (positions, capsules, spawn frames, light volumes) from the run's trace, and only then the pictures. It is general across places (D71), and quiet (D388).
- **A check is proved on the archive**: it must fail on the archived films whose reviews found its defect and pass on the ones that did not (the lake's r12 for stares, forest-life r3 for pops). That is its answer known another way.
- **Port, don't invent** (D196): each system below names its source.
- **The grade stays neutral** (D370, the developers'); fire is light, not a grade's subject (question 5).

## The catalogue

Counts are reviews of the 60 that name the defect (by search over every review and handoff, then read); "lost" is time or rounds spent with nothing to show.

| Defect | Reviews | Lost | Owner | Check |
|---|---|---|---|---|
| Colour cast, haze veil, tint (sepia veil, magenta dusk, salmon stone, blue-metal roofs, lime crowns) | ~35 | 3 lake rounds on magenta | Colour pipeline | Colour bands |
| Subject hidden or cropped, camera behind or inside geometry, grass in the lens | ~30 | a 42-min rerender (castle r3) | Camera rig | Camera clearance, subject in frame |
| Far detail loses its texture (cream boxes, smeared roofs, egg crate) | ~28 | — | Far bake | Far matches near |
| Fire's colour and look (khaki, olive, salmon, cards, see-through) | 21 | castle 12 rounds at 6-7/10; 2 lake rounds unreviewed | Fire | Fire bands |
| Scene missing from reflections and shadows (teal or blank mirror, far land unshadowed) | 17 | — | Casters and mirrors | Look-away |
| Things appearing whole or for one frame (stalls, pieces, haze, cut flashes) | 11 | — | Appearance | Pops on every film |
| Destruction falls as voxel grids, sinks, or tips rigid | 10 | — | Destruction | Fall physics |
| Wildlife posing, staring, crossing the way; popping in; through trees | 8 | 6 lake rounds | Wildlife, Steering | Stare, pop-in, clip-through |
| Layout: castle outside or apart from the walls, empty walled meadow, mountains too big | 5 | — | Town layout | Layout tests |
| Out of GPU memory | 5 renders died | about 95 min | Render runs | VRAM budget |
| Stale renders (old clip reused, old binary, assets hot-reloaded mid-film) | 3 | about 1 h | Render runs | Render stamp |

## The systems

### Wildlife: when an animal may be seen

- **Spawn out of view.** An animal is placed where the camera does not see it now (frustum and the occlusion rays D384 already casts), and walks or flies into sight; GTA's population spawns out of view the same way. D384's staging keeps its spots, which must now be unseen at placing and seen 1 to 3 s later. Every animal wildlife puts out passes one gate (`wildlife::unseen::put`): unseen now and from the camera 6 m on, tested where it shows (a swimmer on the water's surface, not the bed), each member of a group alone.
- **Fade when it must appear in view** (a teleport, a load): dithered over 0.6 s, as Bevy's visibility-range crossfade (D240) and Godot's `visibility_range_fade_mode` (MIT) do.
- **An attention budget.** A look at someone lasts at most 1.2 s (r12's cap, today a scene constant) and then the animal goes back to feeding or walks off; the cap belongs to the creature's kind, not the film.

### Steering: nothing walks through a tree

Creatures today know trunks only to climb and perch on them (`terra_creatures/src/creatures/mind.rs`); nothing steers round them. Port Recast's `dtObstacleAvoidanceQuery` (DetourCrowd, zlib): each tick a body samples velocities round its wanted one and takes the best that keeps its capsule clear of trunks (`flora::Trunks`), rocks, logs and props (`spatial::SpatialHash`) for a second ahead. Godot's avoidance (RVO2, Apache-2.0) is the alternative for bodies among bodies. It belongs to the mover of [movement.md](movement.md), so creatures and people share it.

### Casters and mirrors: the world exists behind the camera

- **Shadow casters are chosen by the light, not the view.** Bevy 0.20 drops a directional shadow caster when its `VisibilityRange` is out of range of the camera (`check_dir_light_mesh_visibility`), and the cascades end at 300 m (`quality.rs`). Terrain chunks are already chosen by distance alone (`terrain.rs`, `wanted`); they and every caster get a shadow range of their own, as Unreal's Far Shadow cascades and distance-field shadows and HDRP's "Extend Shadow Culling" keep casters outside the view. Past the cascades, far terrain and far land (`sky.wesl`'s `far_land`, which is no geometry at all) shade by a heightfield horizon map baked from the planet, so a mountain's shadow still falls on the valley.
- **Reflections do not depend on the screen.** The lake reflects what is on screen (`lake.wesl`, `on_screen`) and falls back to a probe, which read teal. Engines fill screen-space misses from a capture of the scene: Unreal's SSR falls back to reflection captures, and its Planar Reflection draws the scene mirrored. A water body gets a reflection capture at its level, holding terrain, far land, forest and sky, refreshed as the hour moves; ray tracing later (question 7) culls by a radius, not the view, as Unreal's `r.RayTracing.Culling.Radius` does.

### Fire: one module, physical parameters

A blaze, a campfire, a breath and a torch are one `Fire` with physical inputs: fuel, temperature in kelvin along its body, soot yield, opacity per metre, size. Its colour is blackbody emission at that temperature (Unreal's BlackBody node, Houdini Pyro and EmberGen shade the same way), its brightness in absolute luminance, compensated for the camera's exposure as Frostbite authors emissive (Lagarde and de Rousiers, 2014), so it reads the same at dawn, noon and dusk. It is exempt from the grade's sectional saturation (D302's 0.2 in the shadows turned dusk fire khaki) and given a fixed curve through the tonemapper's shoulder, replacing D377's per-film lifts (1.5, 1.6, 1.7) and footage greens. The look of the forms (raymarched breath, filmed flames, embers, soot) stays D377's and D325's.

### Colour pipeline: every tint has an owner

One sky gradient feeds sky, water and far land (`dusk.wesl` started this in r12); a material's colour is its albedo under the light, and the grade is neutral (D370). A new tint can only come from a light, a material or the air, each with its measured target per place in `style.json` (D356). Nothing new to port; the gain is the check below run on every film, so a cast is found in the first still, not the third review.

### Camera rig: the camera never enters anything

Every camera, the game's boom (D274, D285) and a film's keyed path (`--cine`, D357), goes through one rig. It keeps a clearance sphere of 0.3 m from solids, terrain and cover taller than the lens (Godot's `SpringArm3D`, MIT, to port; Cinemachine's Decollider, documentation only), and parts or fades plants nearer (D271). A shot names its subject; the rig knows the subject's sample points, and a keyed path is planned against the plot before it renders, so a hover behind a tower roof is refused before a 42-minute render. A camera cut resets the temporal history (Bevy's `TemporalAntiAliasing::reset`, Unreal's camera-cut flag), which ends the one-frame flashes on cuts.

### Town layout: constraints, not tuning

`terra_town::town` (D330) states its layout as constraints and tests them: the castle stands inside the curtain or forms one side of it (Carcassonne's château inside the Cité, Conwy's castle in its walls); the curtain hugs what it encloses, leaving no walled meadow wider than a yard; the castle is laid first on the highest ground and the town round it, as such towns grew. watabou's town generator (GPL) is documentation only. Mountains near a place are capped by the place's view (question 2).

### Far bake: far reads as near

A building's far hull (`hull.rs`) bakes its near materials' colour and contrast, as Unreal's HLOD and Simplygon bake proxies, so stone stays stone at 600 m. Its tone steps vary by place, not by a grid.

### Appearance: nothing pops whole

What appears in view either builds in by its stages (D381's steps, each stage at least 0.5 s on screen) or fades; what leaves fades. Haze, decals and far land never drop for a frame.

### Destruction: falls by physics

Masonry has almost no tensile strength: a leaning tower opens on its tension side and breaks into courses as it goes, it does not tip as one. A fall becomes rigid bodies of a few cubes each, bonded by mortar that breaks under strain (Chaos's geometry collections with damage thresholds, Teardown's voxel bodies, Red Faction Guerrilla's structural stress, documentation and talks only), falling under gravity on the physics engine D33 settles on, with dust where they land (D305 keeps the rules of what falls). `fragments::topple`, a scripted rigid tip, went on 2026-10-18 for rules of support on our own fragments (`fragments::collapse` and `fragments::Rubble`, D305): a fall tips only on one-sided support and slenderness and cracks apart within its material's angle, otherwise it crumbles in place onto its heap.

### Key light from behind the camera

A shot's payoff, the pose a film ends on, is keyed from behind the camera: its subject's face lit by a sun within 45° of the camera's back, never against it, and the sun never nearer the view than the frame's half-diagonal plus 15°, so no flare washes a third of the frame. The subject's box sits inside the 5% safe frame, with nothing built between it and the lens. A time-lapse chooses its hours for this (`assets/data/timelapse/*.ron`), and a keyed end pose is checked against the subject's box and the sun before it renders, as cinematographers place the sun over a shoulder for a three-quarter key. Settlement r14 ended with the sun 106° off the camera's back and the castle out of the safe frame; r15's hours and end keys put it 18° off with the castle inside.

### Ground patterns: filtered to the texel they land in

A pattern on the ground finer than the floor's texel (a garden's rows, 1.1 m apart, under the 2 m texel of a camera 50 m up) is painted as its mean over the texel, never as the stripe the texel's middle fell on, which aliased into torn mud blotches (settlement r14). The cover source integrates its own patterns (`CoverSource::growing_over`, the box-filtered pulse train of Apodaca and Gritz), as a mip prefilters a texture. Gardens are framed by paths trodden paler than their beds, so a plot reads as a plot from any height. Tested in `play::town_gardens`.

### Nothing of the game's in a film

A film shows the world's things, not the game's: no claim stone, plan outline, marker or ghost unless a player stands in the shot. A town rising by itself sets its claim unseen (`play::rising`), and demo and film places leave out the combat lair (D421).

### Render runs: fresh, within budget, bounded

Every film, still and probe writes a stamp (the commit, a hash of the uncommitted diff, of the binary, of `assets/` and of the scene) and the asset watcher is off; the review and the trailer's cache read the stamp, not mtimes. A run declares its VRAM need from a probe of its heaviest frame, not `trailer.ts`'s one 7,000 MB for every place, and a watchdog ends a run that writes nothing for five minutes.

## The checks

One new task, the film check (`filmcheck`, to be added to xtask), runs at the end of every `trailer`, `film`, `--stills` and probe, a line a check (D388). Its facts come from a **frame ledger**: the sandbox's `--trace` (read today by `diverge.ts`) grows a record a frame of the camera (pose, clearance to solids, terrain and cover), each creature (capsule, spawn frame, fade, head direction, depth into solids), each fire's projected box, the named subject's visible points, the GPU memory in use and the stamp. A failing film is not sent for review.

| Check | Reads | Fails when |
|---|---|---|
| Stamp | ledger, tree | the film's stamp is not the tree's |
| VRAM budget | probe of the heaviest frame | peak over 85% of the card, before the render starts |
| Camera clearance | ledger | the camera within 0.3 m of a solid or terrain, or under cover taller than the lens |
| Subject in frame | ledger | the subject under half visible for over a fifth of its shot, or out of the 90% safe frame |
| Pop-in | ledger, then frames | a creature seen in the frame it was placed at more than half faded in; any patch appearing not from an edge or an occluder (`pops.ts`, events from the ledger excepted) |
| Stare | ledger | an animal's head within 15° of the camera for over 1.5 s within 40 m |
| Clip-through | ledger | a capsule over 5 cm into a trunk, rock, log, prop or wall in any frame (`life` does it for people) |
| Colour bands | frames, `look.ts` | a band's mean (sky, far land, middle, near, water) outside its place's reference range; a forbidden hue (magenta, lime) over 2% of the land |
| Fire bands | frames, fire boxes | yellow under 10% or near-white core under 2% of fire pixels; red over 70% (r11 measured 0-2% yellow against a flamethrower's 25%) |
| Look-away | two stills | a still and the same view at half the field of view differ by over 0.5% in their overlap: a shadow or mirror that needs its caster on screen |
| Far matches near | workbench sheet | a far hull's mean colour over ΔE 4 from its near, or its contrast under 60% of it |
| Fall physics | ledger | a falling mass accelerates under 80% of g; over 120 cubes rotate 15° unbroken; a fragment rests in the air |
| Layout | unit tests | the castle outside the curtain; walled meadow wider than a yard |

## What we port, build and keep

| Part | From | How |
|---|---|---|
| Obstacle avoidance | Recast's DetourCrowd (zlib); RVO2 via Godot (Apache-2.0, MIT) | Port into the mover |
| Fades on appearing | Godot's visibility-range fade (MIT); Bevy's crossfade | Use and extend |
| Camera clearance | Godot's `SpringArm3D` (MIT) | Port into the rig |
| Far shadow, ray culling | Unreal and HDRP (documentation only) | Build from the documentation |
| Reflection capture | Unreal's SSR fallback (documentation only); Bevy's environment maps | Build on Bevy |
| Blackbody, exposure-relative emissive | Frostbite's PBR notes, Unreal's BlackBody (documentation) | Build |
| Breakable falls | Chaos, Teardown, Red Faction (documentation, talks); D33's engine | Build after D33 |
| Pops, colour bands, collide, life, trace | Ours | Keep, run on every film |

## Order of work

| Stage | Builds | Done when |
|---|---|---|
| 1. Runs you can trust | Stamp, watcher off, VRAM probe, watchdog; the ledger; `filmcheck` with stamp, clearance, subject and pops | Each check fails on its archived bad films (castle r2's courtyard, r3's roof, troll r1's grass, settlement c4's OOM) and passes on forest-life r3 |
| 2. Colour and fire | Colour and fire bands; the Fire module | The castle and lake films pass both bands at dawn, noon and dusk with no per-film constant |
| 3. Wildlife | Out-of-view placing, fade, attention budget, obstacle avoidance | Stare, pop-in and clip-through pass on the lake and forest runs, the fox among the trees too |
| 4. Casters and mirrors | Shadow ranges apart from view, horizon map, water capture | Look-away passes at the lake and the settlement |
| 5. Places | Town constraints, castle first; far bake | Layout tests and far-matches-near pass; the settlement film's castle inside its walls |
| 6. Falls | Breakable rigid falls on D33's engine | Fall physics passes on the castle film |

## Open questions for the developers

1. **The castle and the walls.** Inside the curtain always, or may it form one side of it on a rise, as Conwy's does? Recommended: either, never outside. Open.
2. **How big mountains are.** Recommended: from a town, peaks about 1 km over the valley floor (the Tatras over Zakopane), the far ranks higher but hazed. Open.
3. **Destruction.** Which earlier round looked more realistic: the dragon film's r7 (towers sinking into heaps) or a castle round before the topple? Recommended: falls by physics (stage 6), the reference chosen by you. Open.
4. **Animals looking.** May an animal ever look at the player? Recommended: a glance up to 1.2 s, as deer do, never the closing image of a film. Open.
5. **Fire outside the grade.** D370 is yours: may light sources (fire, torches, embers) skip the grade's sectional saturation, since they are the light the grade assumes? Recommended: yes. Open.
6. **Gates or reports.** Should a failing `filmcheck` stop a film reaching you? Recommended: yes, unless the message names the failing check. Open.
7. **Ray tracing.** Is hardware ray tracing (Bevy's Solari) in plan? Recommended: not yet; keep casters view-independent so it can come later. Open.

## What happens to what exists

D384's staging keeps its spots, which must be unseen when placed. D377's lifts and footage greens become the Fire module's temperature and exposure. `pops.ts`, `look.ts`, `collide.ts` and `life` become stages of `filmcheck`; `trailer.ts`'s mtime staleness and `NEEDS_MB` become the stamp and the probed budget. `fragments::topple` is replaced in stage 6. Each stage that lands records its decision and amends the ones named.
