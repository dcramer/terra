# Traps

Each cost a job before (iso-art j1 to j10). The fix follows the dash.

## Tripo and the asset

- `hurt` is not a hit: 13.8 s lying wounded from frame 0 — a hit is the game's flash, flinch and hit-stop, or a `--prompt` motion (20 credits, humanoid only).
- `slash` is 6.6 s with a 3 s return to guard — `window` cuts it to 0.75 to 2.75 s.
- `--in-place` keeps Tripo's `root` joint still while the pelvis walks off — `inPlace()` takes each chain joint's drift out.
- The V3 rig's `root` joint sits on the ground above the pelvis and was read as a foot — feet are joints under the body joint.
- The run is nearly two strides that do not meet (15 to 23%) — `loop()` cuts one stride and spreads the rest; read the ledger's "ends X% apart before closing", since `check` then reads 0.
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
- A flat emissive over 1 makes the hit creature a pink silhouette — emissive texture = albedo, strength under 1.
- A soft black decal under a 140k lm pool does not show — a texture with rings and flecks, or a darker albedo.
- A glossy, reflective water face under a strong point light renders as one flat lit disc — check the light's specular before tuning a faked reflection (the ruin's pool is unlanded: `~/.cache/terra-handoff/iso-art-j10-pool/`).
- Fog cards over the whole frame lift the mids into lavender smudges — keep them low and in the pools.
- Water at a true Fresnel (~0.02) reads as mud; at a lift of 6 too — the sky lifted ~25 times (pool.rs `Look.sky.a`), darker near the banks.
- A light's reflection lands ~0.7 x its height toward the camera from its foot: a fire up the bank glints on mud — glinting lights on the water's lip or in it.
- A sun under ~15 degrees turns every blade's shadow into a frame-wide streak (the marsh's reeds) — `NotShadowCaster` on blades and sedge.
- Blades under ~1.5 cm wide alias into dark hair at the camera (a pixel is ~1 cm) — 3 to 6 cm, lighter albedo.
- A wisp's 6 cm core and 0.6 m glow are dots at 960 px — a 1.3 m soft orb and a 3 m faint haze card about a 10 cm core.
- `patch` is a reserved word in WGSL: the shader fails at run time with only an ERROR line in the run's log, the frame still saved — grep the run for ERROR before judging a still.
- Effects sized as they feel read as nothing at 70 px a metre (motes under 0.07 m, banner tears) — size them at the game camera.
- A 0.7 m soft halo over a 0.13 m core reads as a soft blue blob in every biome's judge — a 0.04 m core drawn out along the flight, a 0.3 m dim halo, a thin white ribbon inside the coloured trail, and a hard star card (kit::star) for the impact, not a swelling disc; a 4k lm bolt light lights no floor beside 60k to 140k lm braziers — about 30k.
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
- Look at stills at 960 or 640 wide, 1:1 crops only where you judge; two film frames tiled at 640 cost little.
- A film's frames read darker than the 1920x1080 still — compare like with like.
- Every job hit the 80% context hook at about 120k by its third shot — plan one look an effect, not a look a tweak, and land each checked step at once.
