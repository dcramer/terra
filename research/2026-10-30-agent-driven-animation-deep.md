# Who gets good character animation with AI agents doing the work, and how (deep survey)

2026-10-30. Follow-up to research/2026-10-30-agent-driven-animation.md, which was too shallow: it read mostly vendor pages and concluded "nobody shows it". The developer is sure people have done it. This survey looked for the practitioners themselves.

**Where they are:** mostly not on X or YouTube; the search tools reach those poorly, and Reddit and X refused automated reads. They are in public GitHub repositories, in pull requests written by Claude Code or Codex. Each PR states its pipeline, its numbers and how it was checked. That is better evidence than a tweet, because the method is written down. Vibe-jam games and starter kits fill in the simple end.

**Caveat on "looks good":** few of these show a public film. Most prove it with measured foot sliding plus films or frame strips kept in the repository. Where a public video or GIF exists, it is linked. Nobody found shows agent-made humanoid locomotion at AAA polish. The best shown is "clearly usable, less polished than Epic's captured set" (example 1).

## 1. Who has done it

### 1. Kansei: Rust/WebGPU engine, motion matching on Kimodo-generated clips
- **Link:** [Siroko/kansei PR #106](https://github.com/Siroko/kansei/pull/106), [PR #109](https://github.com/Siroko/kansei/pull/109), [repo](https://github.com/Siroko/kansei) (MIT; has AGENTS.md and CLAUDE.md).
- **Built:** a Rust/wasm and TypeScript WebGPU engine with skinning, motion matching, inertialization (a smooth blend between poses), foot IK and warping. `kansei-anim-bake` bakes clips into motion-matching packs.
- **Pipeline:**
  1. Kimodo-SOMA-RP-v1.1 on an RTX 5090 in Docker. Every clip keeps a JSON note of its prompt, seed, model revision and time.
  2. Prompt sets: a "dance card" of walks, runs, turns, starts and stops at set speeds. A shared style phrase ("limp") is added to every prompt in a pack.
  3. `kimodo_to_gltf.py`: adds a ground `root` joint that carries the smoothed path and heading, then cuts and closes the loops.
  4. Route A uses Kimodo's own SOMA body. Route B retargets onto the UE mannequin ("plain geometry", aligning each bone's rest direction) and mixes with Epic's Game Animation Sample (GASP) data.
  5. A fixed `drive=1` route feeds identical input to two packs, for side-by-side films.
- **Results:**
  - Loops came within 0.05 m/s of the target paces.
  - Feet planted 33/32% of frames against GASP's 31/32%.
  - In the mixed pack, the search picked generated clips 40% of the time, mostly for turns, starts and stops.
  - Faults: "arms forward" around run stops; a stiffer upper body; clips that touch objects float, because Kimodo sees no geometry.
  - A whole pack in one shared style blends cleanly; "a single style clip mixed into a neutral pack would pop".
  - Cost: 37 clips × 2 samples in 139 s of GPU time.
- **Evidence:** films of generated beside GASP (`gasp-vs-generated.mp4`). They are kept private, because GASP data is Unreal-only. Verdict: generated is "clearly usable" but "less polished"; "GASP's purpose-shot data still reads better".
- **For us:** the closest match: Rust, a known-good reference beside, Kimodo, measured paces.

### 2. degoldschmidt/rpg-test: Godot, procedural first, then mocap clips with foot locking
- **Link:** [PR #9 procedural feet](https://github.com/degoldschmidt/rpg-test/pull/9), [PR #12 mocap locomotion](https://github.com/degoldschmidt/rpg-test/pull/12) (Claude Code, branch `claude/…`).
- **Pipeline (PR #12):**
  1. `MocapImporter` retargets a Mixamo skeleton onto a 16-body puppet by world-space bone directions, so the source rig's axes and T-pose don't matter. It scales by leg length and caches the result.
  2. It extracts a smoothed hips path and heading, flat-foot contacts, heel and toe support, and the cycle length.
  3. `LocomotionPlayer` blends idle, walk, jog and sprint by capsule speed, **phase-locked on the left foot's touchdown**.
  4. A foot is locked only while flat. At lift-off only the lock's small offset fades, so the foot follows the clip's swing.
  5. Transitions are clips matched by name: "Stop Walking", "Run To Stop", "Idle To Sprint", "Running Turn 180". Feet out of place take settle steps.
  6. Attacks, guard and carry are layered over the clip pose.
- **Evidence:** planted-foot slip 0.000 m/s at stroll, walk and run; pelvis never below 0.84 m; 1000-body crowd at 85 fps. All 198 checks pass with the real clips and without them. Raw Mixamo files are gitignored, as Adobe's terms forbid redistribution. No public film.
- **Lesson:** the procedural gait (PR #9) measured near-zero slip too, and was still replaced by captured clips. Good numbers from a procedural gait did not make it read as a person.

### 3. Kokanee25/Western ("Salt Creek"): Godot, Uthana clips as targets, films, 20 seeded runs
- **Link:** [PR #342](https://github.com/Kokanee25/Western/pull/342), [issue #343](https://github.com/Kokanee25/Western/issues/343), [PR #358](https://github.com/Kokanee25/Western/pull/358) (Claude Code; the developer "Sean" makes the calls).
- **Pipeline:**
  1. Uthana text-to-motion takes (18 for the scene, 9 of them kicks from 3 prompts).
  2. `tools/blender/retarget_motion.py` maps any humanoid take onto 17 segments ("the CMU method"). It trims, holds the planted foot ("the walk's slide 0.52 -> 0.00 m"), marks the strike frame, and ranks the takes in a report.
  3. In game the clip "leads" each joint; physics keeps the final word (sliding, grounding, ragdoll at the end).
  4. The scene is data (`config/moments/card_cheat.json`). A script runs 20 seeded games and counts outcomes.
  5. Films go to the developer (bar, door, old against new, a frame sheet), with a decision asked for.
- **Evidence:** films in the repository; Earl ends on his back in 16 of 20 runs.
- **Gotchas:**
  - "every kick came out high and forward", even when the prompt asked for a low kick.
  - The gun hand shakes, as the prompt asked, so the muzzle wanders.
  - Billing: Uthana through Replicate cost about $0.50 a prediction, not the $0.0001/s the code had read, and the run overspent its $10 approval. Uthana's own API bills per second.

### 4. GTA-6-Claude: C++/D3D12, a crowd's planted feet through turns, stairs and curbs
- **Link:** [PR #26](https://github.com/nichtpascaltarter-sketch/GTA-6-Claude-v0.5/pull/26) ("the animation agent's locomotion pass", Claude Code, with a Codex review).
- **What it did:** feet stay planted through turns, starts, stops, curbs and stairs; a start steps off with the unloaded foot; walkers side-step in close passes.
- **Measured** (mean slide per step, before → after):
  - In the city: 14.9 → 7.4 cm.
  - In a step harness: 90° turn 16.9 → 1.4 cm, stairs up 21.7 → 3.0 cm.
  - Cost: +0.2 ms a frame.
  - Known faults are listed (a trailing foot lifts in sharp AI turns). No film.

### 5. Auvra and Tierlist: play rate matched to ground speed
- **Auvra** ([PR #61](https://github.com/dovvnloading/Auvra/pull/61), TypeScript, Claude Code):
  - A clip's speed is the median travel of its planted foot relative to the hips, measured on a bone-only copy of the rig.
  - Play rate = smoothed ground speed ÷ clip speed, clamped to 0.5–2×. Root-motion states are skipped.
  - Planted-foot speed fell from 0.67 to 0.12 of body speed on a rifle walk; about 0.1 is the measurement's floor.
- **Tierlist** ([PR #689](https://github.com/ProposalWizard/Tierlist/pull/689), Claude Code):
  - Walk, jog, run and sprint run on **one shared stride clock**; every change fades from the current pose (minimum 0.08 s); turns are eased.
  - Results: pops 4 → 0; mean slide over 46 speeds 7.1 → 5.0 cm/m; worst 32.3 → 14.5 cm/m.
  - It says honestly: "Not seen on a screen yet; measured headless", which is the trap our doc names.

### 6. project-whiskeyjack: Quaternius clips, game speeds retuned to the clips
- **Link:** [PR #24](https://github.com/zttimmerman/project-whiskeyjack/pull/24) (Godot, Claude Code; a "motion judge" scores evidence packets).
- **Fix:**
  - The bigger body (+13%) needed a faster jog than the clip's playback range allowed ("4.0 isn't reachable within 0.75–1.5×"). The **game's speeds were changed** (4.0 → 4.6 m/s); the clip was not stretched.
  - The slide metric was itself wrong (the contact point hopped between joints, and loop seams were counted twice) and was fixed first.
- **Result:** p90 slide 0.799 → 0.211 m/s.

### 7. ninth-chamber: Quaternius, then Mixamo clips on the player, frame strips reviewed
- **Link:** [PR #10](https://github.com/ezar/ninth-chamber/pull/10), [PR #11](https://github.com/ezar/ninth-chamber/pull/11) (TypeScript, Claude Code, with a Codex review).
- **PR #10:** Quaternius UAL (CC0) idle, walk, run and jump, "retargeted by direction" onto the hero's skeleton. "A foot pass pins planted feet, keeps soles above the floor and follows steps up and down". Clips blend by speed "on one shared step cycle"; 141 tests include loop and foot-slide checks; "Run-cycle filmstrip reviewed".
- **PR #11:** 26 Mixamo clips matched to the game's 2.2 and 5.4 m/s, with foot planting. Frame strips reviewed for walk, run, jump, hang, climb and push. Codex caught a hip-collapse bug in the backward blend.

### 8. Islandyout/engine and ThreeMojo: the world-space retarget, checked against renders
- **Islandyout** ([PR #58](https://github.com/Islandyout/engine/pull/58), Claude Code): merges a Mixamo-rigged copy into a 65-joint Quaternius rig (21 bones mapped).
  - **The method:**
    1. Take the source bone's world rotation change from its own rest pose.
    2. Apply that change to the target bone's rest pose.
    3. Convert back to local using the target parent's **animated** world rotation, parents first.
  - Hips get a drift fix. Two bugs (using the parent's rest pose instead of its animated pose; 21 m of hip drift on `flying`) were caught by **comparing rendered frames against the source**.
- **ThreeMojo** ([PR #850](https://github.com/SethKitchen/ThreeMojo/pull/850), Claude Code): the same method, with hips scaled by the ratio of hip heights.
  - Known limit: forearm roll is not corrected, so a palm can turn differently from the source.
  - Real Mixamo files hit a Float32 bind-matrix rounding error on 13 of 129 bones.

### 9. mixamo-llm-mocap: video to motion, operated end to end by an agent
- **Link:** [squall01337/mixamo-llm-mocap](https://github.com/squall01337/mixamo-llm-mocap) (MIT, 418 stars). Public GIFs: `media/showcase.gif` (nine punches, a slip, a side kick) and a two-performer duel.
- **Pipeline:**
  1. GVHMR pose estimate to SMPL-X, refined with 2D keypoints and heel and toe points.
  2. Retarget to the Mixamo rig: floor, foot locks, the camera's true path.
  3. Headless Blender keys the action.
  4. `render_review.py` makes a 2×2 review video: the source, an overlay, before/after, and zoomed hands and feet.
  5. `eval_fidelity.py` measures limb angles against the video, foot slide and jitter.
  6. `compare_reference.py` flags frame windows that diverge, so the agent retakes those.
  7. `docs/PITFALLS.md` records past mistakes for the next operator.
- **For us:** the loop only. GVHMR and SMPL-X are research-licensed.

### 10. critter-crafter: Unity creatures, procedural feet on IK, owner-reviewed captures
- **Link:** [PR #1](https://github.com/9thLevelSoftware/critter-crafter/pull/1) (Claude Code, then Cursor).
- **Why procedural here:** the baked clips moved at 0.03–0.26 m/s against 2.5–2.8 m/s game speeds.
- **Pipeline:**
  1. A per-skeleton locomotion block in data: hip height, reach, stroke, duty factors, a speed band.
  2. A step planner (Python reference, C# port, 195 parity rows matching to 1e-6).
  3. Unity Animation Rigging two-bone and chain IK.
  4. Gait clips layered on top, kept in time with the steps.
- **Evidence:** all 39 skeletons pass static QA; the owner reviewed captures of every family by eye.
- **For us:** the right tool for many-legged creatures, not for people.

### 11. Vibe-coded three.js games: Mixamo-style clips played in place
- **Viben Ring** ([repo](https://github.com/SabrinaRamonov/viben-ring), Windsurf with Claude 3.7):
  - One FBX per clip: idle, walk in four directions, run, roll, light and heavy melee; a boss with its own set.
  - Her reported fault: the attack "would cut off halfway or get stuck" until several rounds of fixes.
- **Vibe Coding Starter Pack 3D** ([repo](https://github.com/majidmanzarpour/vibe-coding-starter-pack-3d-multiplayer), 322 stars, [demo video](https://x.com/majidmanzarpour/status/1909810088426021192)):
  - Wizard and Paladin, 14 clips each: walk and run in four directions, jump, attack, cast, damage, death.
  - `makeAnimationInPlace` strips root motion; the code moves the body.
- **Vibe Jam 2026 winner** (A Game About Capybaras Delivering Food, $25k):
  - Claude Code with three.js and Tripo models ([coverage](https://wccftech.com/ios-developer-made-capybara-game-using-vibe-coding-and-won-25000/)).
  - How the capybara moves was not found. Rider on a bike, not human locomotion.
- **Why these look fine:** the clips are professional mocap, played as made, and speeds are set to suit them. Little is asked of them: no gear, no 7 m/s sprint, no layers.

### 12. Vendor agent tooling with working examples
- **[Cartwheel MCP](https://github.com/Cartwhl/cartwheel-mcp):**
  - 32 tools: text-to-motion ("Swing"), video capture of 1–4 actors ("Comic 4"), loop, stitch, retarget, BVH/FBX/GLB export.
  - Its `npm run example:game` is a three.js scene with idle, walk and run transitions, an interruptible arm gesture and a crowd.
  - Its co-founder showed an agent animating a Blender scene through the MCP ([post](https://x.com/andrew_n_carr/status/2096644188343529743)).
- **[Uthana MCP](https://github.com/Uthana/uthana-mcp)** (Apache-2.0):
  - Tools include `generate_locomotion` ("walk or run cycles from style, strides, speed, and direction"), video-to-motion, loop, stitch, trim, previews (`get_preview_frame`) and `estimate_model_cost`.
  - The hosted MCP is a technical preview.
- **[Meshy 3D agent v0.7.0](https://github.com/meshy-dev/meshy-3d-agent/releases/tag/v0.7.0):**
  - It tells agents to use rigging's bundled walk and run first, then the preset library, and text-to-motion "only when no preset fits".

## 2. The common recipe

Every success above follows the same steps. Agents do the plumbing and the measuring; **none keys motion by hand or by formula** for people.

1. **Captured motion is the source.**
   - Sources: Mixamo, Quaternius, Epic's GASP, Uthana, Kimodo, video capture.
   - Procedural motion survives only for creatures (critter-crafter) and as a fallback when clips are missing (rpg-test).
   - A bad clip is retaken or a different one chosen, with the developer asked (Western's kick).
2. **One target skeleton, one retarget method:** world-space rotation change from rest, parents first, using the parent's animated rotation.
   - Hips are scaled by the ratio of hip (or leg) heights.
   - About 20 bones mapped; fingers optional.
   - Known traps: A-pose against T-pose, forearm roll, hip drift on airborne clips, Float32 bind-matrix rounding, three.js GLTFLoader stripping `:` from `mixamorig:` bone names.
3. **In-place locomotion; the code moves the capsule.**
   - Root motion is stripped (`makeAnimationInPlace`, Tripo's `animate_in_place`), or extracted from the hips into a smoothed `root` joint (Kansei).
   - Root motion is kept only for committed actions.
4. **Speed matching, three ways, often together:**
   - (a) Measure each clip's own speed from its planted foot.
   - (b) Play rate = ground speed ÷ clip speed, clamped (0.5–2× in Auvra, 0.75–1.5× in whiskeyjack).
   - (c) When the clamp can't reach the speed, **change the game's speed or get a new clip**, never stretch.
   - Gaits share **one stride clock or phase** (synced on a foot's touchdown) and blend by speed.
5. **A thin post-pass:**
   - Lock a foot only while it is flat, and fade only the lock's offset at lift-off.
   - Settle steps when standing feet are out of place.
   - Two-bone leg IK for steps and slopes; drop the pelvis when a foot can't reach.
   - Cross-fades of 0.08–0.25 s, or inertialization.
6. **Verification:** numbers as guard rails, eyes as the judge.
   - Numbers: planted-foot slip (m/s or cm per metre), pops, the worst single-frame jump, pelvis height, outcome counts over seeded runs.
   - Eyes: films or frame strips from the game, and **a known-good reference beside** (Kansei against GASP, Islandyout against the source renders, mixamo-llm-mocap against the video).
   - The developer decides on look questions.
7. **Licence hygiene:** raw Mixamo or GASP files never enter the public repository (gitignored, or kept in a private folder). Generated clips carry a provenance note.

## 3. Tools ranked for us

For a Rust/Bevy game in a public MIT repository, sold commercially, with agents doing the work on a low budget. We pay for Tripo and Uthana, and own Quaternius UAL Pro and Source.

| # | Tool | Cost | Licence for us | API / how an agent drives it | Verdict |
|---|---|---|---|---|---|
| 1 | **Quaternius UAL1 Pro + UAL2 Source** | owned | CC0: in the repo, shipped | Files; Blender scripts retarget onto our skeleton (D542) | First for every locomotion and common move; it is hand-keyed, so judge it on film against capture |
| 2 | **Uthana** | Pay as you go: locomotion $0.02/s, text-to-motion $0.02–0.10/s, video-to-motion $0.05/s of input, loop and stitch $0.01/s ([pricing](https://uthana.com/pricing)) | "Paid usage includes commercial use of generated outputs, subject to Uthana's Terms of Use"; who owns the output is still to be read in the Terms | GraphQL API, Python/TypeScript SDKs, [MCP](https://github.com/Uthana/uthana-mcp) with `generate_locomotion`, preview frames, a cost estimate | Second: captured-trained, made at a stated speed and stride; call Uthana directly, not through Replicate; expect prompt misses (high kicks) |
| 3 | **Kimodo** (NVIDIA) | free; local GPU (a 32 GB card in Kansei's run) | Weights under the NVIDIA Open Model License (commercial use allowed); NVIDIA claims no ownership of outputs; the Llama 3 text encoder is gated | CLI and Python; constraints by keys and paths; packs at set paces | Third, for what neither has (D532): "clearly usable, less polished" than GASP; arms forward at stops; can't touch objects |
| 4 | **Tripo animate** | in our Studio subscription; Studio: 20 credits to rig, 20 per generated animation | Paid output is ours under our plan | API `POST /v3/animations/rig`, then `/animations/retarget` with `preset:walk`…, `animate_in_place`, glb/fbx; rig v2.5 has 16 universal presets ([docs](https://developers.tripo3d.com/en/docs/animations-retarget)); text-to-motion (1–10 s) is shown in Studio, its API not confirmed | For creatures and quick props; no independent quality report for humans |
| 5 | **Cartwheel** | subscription; API on paid plans; Swing 1 credit/s, capture 1 credit per person per second ([pricing](https://getcartwheel.com/pricing)) | Terms not read | MIT [MCP](https://github.com/Cartwhl/cartwheel-mcp), 32 tools, BVH/FBX/GLB | A trial only if Uthana fails a move; one blogger saw exports differ from the preview |
| 6 | **Meshy animation** | subscription | Free plan output is CC BY; paid is private | API, but animation works only on Meshy's own rigs; outputs deleted after 3 days | No: tied to Meshy rigs |
| 7 | Video to motion: GVHMR (mixamo-llm-mocap, Motion2MixamoRig) | free | SMPL-X research licence | Agent-run loop | No for shipping; copy the review loop |
| 8 | Mixamo | free | No raw files in a public repo; no official API (the agent tools use the account's browser token) | Unofficial scripts | No for us |
| 9 | Epic GASP | free | Unreal Engine only | n/a | No; a private reference to film beside ours at most, as Kansei did |
| 10 | HY-Motion 1.0 (Tencent) | free | Licence and **outputs** excluded in the EU, UK and South Korea; checked in the repo's LICENSE | Local model | No |
| — | DeepMotion, Rokoko Vision, Move.ai, Radical, Plask, Cascadeur | various | Move.ai keeps ownership of output (research/2026-10-26-animation-sourcing.md); DeepMotion's free tier is non-commercial | Cascadeur has no headless API | Not worth a trial: no agent practitioner found using them |

**Bevy and Rust specifics:**
- Bevy binds clips to bones by `AnimationTargetId`, a UUID from the bone path ([issue #15612](https://github.com/bevyengine/bevy/issues/15612)). So the retarget belongs offline, baked onto our skeleton, which is what D542 does.
- [bevy_animation_graph](https://github.com/mbrea-c/bevy_animation_graph) (0.10, Feb 2026):
  - Has a speed-based blend that scales playback, two-bone IK, foot-down and foot-up events, a mirror node and an editor.
  - Retargeting is by bone-path overrides only.
  - Support for Bevy 0.20 is not confirmed.
- [voxell-tech/bevy_motion_matching](https://github.com/voxell-tech/bevy_motion_matching): motion matching from BVH, about 50 stars, not on crates.io.
- [Humentity](https://github.com/rolfsormo/Humentity): MakeHuman bodies in Bevy with CPU retargeting.
- Kansei (example 1) is the proof that motion matching with inertialization, foot IK and warping runs in Rust on WebGPU, from generated packs of 7–25 minutes.
- Bevy projects with Claude-written animation PRs exist ([decentraland bevy-explorer #1407](https://github.com/decentraland/bevy-explorer/pull/1407), [AABevy #36](https://github.com/AAEmu/AABevy1.2/pull/36)). They are plumbing, not looks.

## 4. What we're doing differently from them

1. **They ask less of the clips.**
   - Most play stock clips as made, in place, at the speeds the clips were made at, on a body that carries nothing.
   - We run a 7.4 m/s sprint and a quiver, belt and shield on our own skeleton (Unreal names, twist bones). So every clip goes through our retarget and our layers. Each of those is a place to go wrong that they don't have.
2. **They change the game's speed to suit the clip; we stretched the clip.**
   - whiskeyjack moved the game's jog from 4.0 to 4.6 m/s because a clamp of 0.75–1.5× couldn't reach it.
   - We played a 3.9 m/s clip at 7.4. docs/animation.md now says the same, ±10%. They allow far wider play-rate clamps (up to 0.5–2×), but only as a fade between gaits, never as a sprint's whole speed.
3. **They phase-lock the gaits.**
   - Several (rpg-test, Tierlist, ninth-chamber) put walk, jog and sprint on one stride clock synced on a foot's touchdown before blending.
   - docs/animation.md says "blend by speed" but not that the cycles share a phase. Without it, a blend of two gaits crosses the feet.
4. **Their best judge is a known-good reference beside, not a score.**
   - The ones that look right compare on screen against something real: Kansei against GASP, Islandyout against the source render, mixamo-llm-mocap against the video.
   - The ones judged by numbers alone say so ("not seen on a screen yet").
   - We had numbers alone until 2026-10-29. The stock-against-ours film is the fix, and it matches what worked for them.
5. **They keep few layers.** None stacks per-joint fix-ups. A foot lock that holds only while the foot is flat, leg IK, and a fade. We had twelve layers; D584 cuts them to three, which matches.
6. **Motion matching is not as far off as our doc says.**
   - docs/animation.md says it "needs hours of capture".
   - Kansei ran it in Rust on 74–131 generated clips (7–12 minutes), or 25 minutes when mixed with GASP. Kimodo filled the turns, starts and stops, and a shared style prompt kept a pack consistent.
   - Not for now (blend spaces come first), but the reason given in the doc should be corrected.
7. **They retake and ask; we patched.** Western puts the kick's faults to its developer with three options. rpg-test threw out a procedural gait that measured perfectly. That is our rule 3, now written; they were already doing it.
8. **What we share with them:**
   - Licence hygiene: Mixamo and GASP stay out of the repo, as our sourcing note already decided.
   - The flaws of generated motion: prompts missed, objects ignored, arms forward at stops. That is the case for Quaternius first and Uthana second.
