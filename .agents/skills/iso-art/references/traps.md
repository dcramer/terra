# Traps

Each cost a job before (iso-art j1 to j10). The fix follows the dash.

## Tripo and the asset

- `hurt` is not a hit: 13.8 s lying wounded from frame 0 — a hit is the game's flash, flinch and hit-stop, or a `--prompt` motion (20 credits, humanoid only).
- `slash` is 6.6 s with a 3 s return to guard — `window` cuts it to 0.75 to 2.75 s.
- `--in-place` keeps Tripo's `root` joint still while the pelvis walks off — `inPlace()` takes each chain joint's drift out.
- The V3 rig's `root` joint sits on the ground above the pelvis and was read as a foot — feet are joints under the body joint.
- The run is nearly two strides that do not meet (15 to 23%) — `loop()` cuts one stride and spreads the rest; read the ledger's "ends X% apart before closing", since `check` then reads 0.
- The run's in-place loop played half a body's height ahead of its entity (cut from where Tripo's pelvis gathers speed) and leapt there at each switch from walk — `centre()` in export; `check`'s "run centred" fails a GLB exported before it.
- A walk or run that hitches is not always its seam: Tripo's seams measured fine; measure the clip in Node (every joint, world space) before blaming the loop, and check the view's sampling.
- A non-biped rig gets one walk preset: no idle, attack or death — plan its scene around walking, or use a repository animal and say so.
- The month's 100 free images run out (a 403 "Insufficient credits") — concepts cost 10 to 20; try no other `--model` without the credits.
- Tripo's file is about 1 unit tall and may lie turned (the wagon 45 degrees off X) — export scales by `--height`; turn a prop in the scene.
- A re-export Tripo has cached takes 0.0 min — cost from the first export.
- The knight's GLB shows no sword at game size — check a weapon reads before staging an attack on it; hang one on `hand_r` (asset.md, held props).
- A held prop under the bone inherits the armature's 1.89 scale, and a finder of bodies by `WorldAssetRoot` near a place takes the prop for the body — undo the bone's scale; filter finders `Without<ChildOf>`.
- `tripo.ts list` shows 20 projects — `list -n 300` to find an older one by id.

## The scene

- A slab a block, a hue a slab — a checkerboard; one cut mesh of a scan at its true size, value tint only.
- Alpha-masked holes in the paving rendered black: the depth prepass ignores the vertex alpha discard — clip the triangles at the contour.
- Bare `Rock_1..10` render near white at night — the `Rock_Moss_*` set.
- A brazier's bowl shadows its own light into a black disc — `NotShadowCaster` on the bowl.
- A far fill light flattens the pools into one wash; the sky fill lifts p5 — one short shadowed light a fire, sky fill about 1.5 nits.
- A rim light from behind spills on everything (light `RenderLayers` filter by view, not mesh: bevy_light's cluster assign) — a fresnel emissive material.
- A fresnel rim at the ARPG camera washes a figure (j28: the ogre milky, a crown frosted): at 70 px its tail is half the body, and on the normal-mapped normal every fold grazes. Draw a hard band on the mesh's own normal (rim.wesl: whole under cos 0.12, gone over 0.4), masked off tops and undersides by n.y as Dota 2's rim keeps to a top-down mask, at about 4x the fresnel's light; a leafy crown takes it on every leaf, so give it a share (`Fighter::rim`). Debug a band at (10, 0, 10) magenta first.
- A still stands mid hit flash (0.05 s after the blow): hit.rs lights the struck body's base-colour texture, which a value tint of the base colour does not reach; rim.rs's `mirror` scales it by the body's value. The forest's treant-a stayed pale at value 0.55: no emissive, vertex colour or metal in any creature's GLB (their albedo is dark, luminance 0.05 to 0.08); in the sun it is Tripo's bark at the dirt's own value, plus 4% dielectric specular (4 to 5 L*), the rim band (4) and the flash (1). Measure a body by a magenta mask (all bodies emissive magenta in one shot) against a 10 px ring of ground, in L*, never a box mean (j34). A mask at emissive 50 blooms: its body is white, its halo magenta, so a magenta test measures the ground round the body as the body (j38's moose "dL* -0.2" was its halo; on the white core, -4.3, j47): count the core (r, b > 240, g > 190).
- A flat emissive over 1 makes the hit creature a pink silhouette — emissive texture = albedo, strength under 1.
- A soft black decal under a 140k lm pool does not show — a texture with rings and flecks, or a darker albedo.
- A glossy, reflective water face under a strong point light renders as one flat lit disc — check the light's specular before tuning a faked reflection (the ruin's pool is unlanded: `~/.cache/terra-handoff/iso-art-j10-pool/`).
- Fog cards over the whole frame lift the mids into lavender smudges — keep them low and in the pools.
- Water at a true Fresnel (~0.02) reads as mud; at a lift of 6 too — the sky lifted ~25 times (pool.rs `Look.sky.a`), darker near the banks.
- The kit water's `deep` is an albedo lit by the key, and a stand-in glint's smooth lane spreads over a whole small pond: by day a body of 0.03 with a low `absorb`, or a glint's `gain` 0.15, reads as a milky grey sheet (forest j28, j33: box sRGB 126-133 against the ref's 59, 71, 77), which a night hides. Body near black (sRGB 0.035), absorb 10 to 14, lane off: the mirror carries the face (40, 47, 46, j33).
- A light's reflection lands ~0.7 x its height toward the camera from its foot: a fire up the bank glints on mud — glinting lights on the water's lip or in it.
- A sun under ~15 degrees turns every blade's shadow into a frame-wide streak (the marsh's reeds) — `NotShadowCaster` on blades and sedge.
- Blades under ~1.5 cm wide alias into dark hair at the camera (a pixel is ~1 cm) — 3 to 6 cm, lighter albedo.
- A wisp's 6 cm core and 0.6 m glow are dots at 960 px — a 1.3 m soft orb and a 3 m faint haze card about a 10 cm core.
- `patch` is a reserved word in WGSL: the shader fails at run time with only an ERROR line in the run's log, the frame still saved — grep the run for ERROR before judging a still.
- Effects sized as they feel read as nothing at 70 px a metre (motes under 0.07 m, banner tears) — size them at the game camera.
- A 0.7 m soft halo over a 0.13 m core reads as a soft blue blob in every biome's judge — a 0.04 m core drawn out along the flight, a 0.3 m dim halo, a thin white ribbon inside the coloured trail, and a hard star card (kit::star) for the impact, not a swelling disc; a 4k lm bolt light lights no floor beside 60k to 140k lm braziers — about 18k to 30k.
- A spell's light riding 2 m up lights an even wash, not a pool (2 m out it still gives 0.35x, and beside warm pools its blue greys to violet) — a second short-range light about 0.5 m over the floor under the effect (under 10% a metre out), as the ARPGs fake it with a light or a glow decal.
- The effect lights all at once wash the fight's floor white (j43: the ruin's p95 0.20 against the refs' 0.13): at the still's moment the bolts fly and the blow bites, so the bite (40k), the slash's edge (28k) and the pools (15k to 22k) count, not the impact's — map the pixels over the refs' p95 before cutting the light that seems to blame.
- A light's glint in a puddle cut by a straight edge is the floor's normal map through the water, not its line or a shadow (j43; an uncommitted run painting the wet mask as colour shows it) — the water's normal fully flat, its stone only in its colour; a glossy rim round a puddle draws a cut-out's outline — keep the rim matte. Yet j43's flat, lit puddles lost the blind judge to the old black pits with an ember glint ("depth, not glowing decals") and were reverted: at night a puddle reads by what it reflects, not by being lit.
- Mist over night water lost the blind judge twice (j44 broad sheets: "a shapeless milky haze, lifts the blacks"; j45 tattered streaks: "smeared pink-white streaks, a render artifact") and was reverted with its lamp fills, which lifted p5 more than the mist did (map the darkest 5% against the kept still first). Next try: fog the refs' way, unseen except in a lamp's cone (a lit volume or a light-shaft card), never a sheet lying on the water.
- An attack held at the wrong moment reads as arms raised — a sheet of times first (below).

## The worker and the guard

- `--frames 60` for a still saves a black frame: the scene is not up — use the default 300.
- WGSL type errors show only when the pipeline is made at run time; the build passes — grep the run log for `ERROR` after every shot.
- A `sh -c` ending in a grep that matches nothing fails the job though the still was made — end with `; true`.
- `--collect` files land in `~/.cache/terra-review/remote/<job>/target/job/`, not at the local path; it takes folders, several times; a folder named `frames` is never collected.
- No loops, variables or `cp` and `python3` in one command: the worktree guard refuses them — unroll; one plain command a call.
- Wait on a worker job with `cargo xtask remote-render --wait <job>` in the foreground, or `pgrep -f` in one call and `tail --pid=<pid> -f /dev/null` in the next; never a poll.
- Never `git pull --autostash`: it uses the shared stash stack — commit only the step's files; leave a hack uncommitted.
- Bevy 0.20's `Assets::get_mut` returns a guard: `if let Some(mut m) = …`.
- To swap another module's material without editing it: match its `StandardMaterial` at PostStartup, remove `MeshMaterial3d<StandardMaterial>`, insert the extended one.
- Shader files beside a module: `embedded_asset!(app, "x.wesl")` gives `embedded://iso/ruin/ambient/x.wesl`.

## Looking and context

- A sheet of clip times: an uncommitted env-var hack (`KNIGHT_AT`) read in `fight.rs`, six shots at 1280x720 in one worker job, cropped and stacked with ffmpeg; remove the hack before committing.
- A screenshot shows the pose set in its own frame (the animation applies in that frame's PostUpdate): every sheet before iso-art j31 was off by one, each strip frame the next shot's pose and each clip's last the next clip's first — set the next pose two frames after the shot (view.rs `act`), and suspect sampling when a strip's last frame looks wrong.
- Look at stills at 960 or 640 wide, 1:1 crops only where you judge; two film frames tiled at 640 cost little.
- A film's frames read darker than the 1920x1080 still — compare like with like.
- Every job hit the 80% context hook at about 120k by its third shot — plan one look an effect, not a look a tweak, and land each checked step at once.
- Three verdicts, the code and three 960 stills cost a helper about 120k before its first edit — brief a helper on one task with its diagnosis, not five.
- The crate is rustfmt-clean since it joined the workspace (2026-11-07), and the commit hook holds it so: `cargo fmt -p terra_iso` before committing.
- Sample a ref's and a still's colours by box (ffmpeg to raw rgb24 at 960x540, a mean a box in node) before tuning a hue: the eye called the marsh's water blue while it measured violet grey.
