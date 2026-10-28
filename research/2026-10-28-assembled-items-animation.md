# Assembled items through the animations: can hand-driven items with an item track per clip work without per-item tuning?

2026-10-28, characters, items and combat. Question: D565's design ([docs/assembled-items.md](../docs/assembled-items.md), "Through the animations"; [2026-10-28-assembled-equipment.md](2026-10-28-assembled-equipment.md)) has an assembled item ride the hand, kept inside an envelope round its class's reference item, with an item track baked and cleaned for every generated clip. Can that work reliably, with no tuning per item? The developer: "the biggest issues im seeing here are our inability to get the parts to attach in a correct way for the animations ... research every part of the design ... to make sure we can actually pull this off".

Method: the move check (`tools/characters/moves.ts`, unchanged, every body template, the fists closed and the walker's layers as the game composes them) run on items reshaped within plausible envelopes, plus read-only scripts on the exported clips (`fight.glb`, the base body, 24 keys a second), the move set's active windows and the game's hit code. The scripts were one-off and are not committed; the measures worth keeping are named under "What could fail". *Read* marks a source checked this session, *recalled* one from memory.

## Verdicts

| # | Question | Verdict | Confidence |
|---|---|---|---|
| 1 | Hand-driven, a reference item and an envelope per class | **Yes for one-handers, but the envelope is set by D459's tempo clamp, not by the clips.** The sword's clips held blades from 0.60 to 1.05 m with no new fault, but the clamp allows only about ±10% of length at a fixed head (sword tip 0.76 to 0.93 m; axe haft 0.40 to 0.51 m with a 0.9 kg head). The envelope must be sampled through its middle and not only at its two ends: an axe in the sword's guard went into the woman's head at every length measured in and around the clamp (haft tops 0.37 to 0.52 m), while lengths either side passed | high on the method; medium on the numbers (the masses are modelled) |
| 2 | An item track per generated clip, cleaned | **Baking it is trivial. Cleaning it afterwards is the wrong tool.** Jitter is negligible: 0.2 to 0.3° of roll rms. The real faults are in the keyed takes themselves. The wrist snaps 66 to 90° in a single key next to the hand keys, so the tip jumps up to 1.34 m. The wrist bends 105 to 151°, past the human 70 to 80°, in 14 to 23 keys of each blow. Which side of the item leads is fixed by the take: a one-sided head would need turning 85 to 180° in two of the three blows, which no wrist solve can give. The edge and the wrist have to be right when the take is generated, by the hand keys Kimodo already takes (the back of the hand followed within 1 to 8°, the blade within 13 to 31°). Cleaning afterwards should be a check plus a correction of at most about 15° | high on the measurements; medium that keys fix them |
| 3 | Two-handed: the lead hand drives, IK pulls the off hand to the second grip | **Only on clips made two-handed.** On our one-handed clips the off hand would have to move 54 to 150 cm, and its grip is in reach in 0 to 67% of the keys. A two-handed class needs its own takes with both hands keyed (`kimodo_keys.py` already keys `left`). After that, IK takes up only a few centimetres: reach under 0.95 of the arm, the elbow at least 30° from fully folded | medium (we have no two-handed take to measure) |
| 4 | Hits from the item's track, decided by the server | **Yes, and it is the right shape for the server, but nothing like it exists today.** Today the client reads the rendered hand every frame, and every item has the sword's 0.15 to 0.83 m blade (`contact.rs`). The plain-Rust duel sweeps a stand-in arc. Baked hit tracks are only planned (movement.md). One track for each take and body template, with each item's own length laid along +Y at run time, gives a judgement that is deterministic and needs no skeleton. The track needs a sample every 1/120 s or finer: a straight sweep from tick to tick strays 2 to 17 cm from the true path at 60 Hz, and 5 to 40 cm at 30 Hz | high on the approach; medium on the rate |
| 5 | What could make it fail | **The clips, not the parts.** The four risks below are in order. Each has a cheap test, and the first fails on main today | — |

## 1. The envelope, measured with the move check

The arming sword's blade stretched above its guard, the hilt kept, with the heater in the off hand (the fight set, 22 rows, every template):

| Tip (m from the grip) | 0.60 | 0.70 | 0.83 (reference) | 0.95 | 1.05 |
|---|---|---|---|---|---|
| New faults | none | none | — | none | none |
| Nearest contact ahead of the hips | 0.68 m | 0.75 | 0.85 | 0.94 | 1.02 |
| Tempo, `sqrt(I_ref/I)` (modelled masses) | 1.62 | 1.29 | 1.00 | 0.82 | 0.71 |

The only fault in every run was `Keyed_Guard`'s, which is already on main: the sword 9.2 cm into the shield. `Keyed_Guard` is no longer the game's guard. The fist stayed 0.8 cm off the handle, wrapped 89°. Reach scales with length, as D459 wants.

The iron axe with its haft stretched above the grip and its head carried up with it, through the sword's fight clips (a stand-in: there is no axe set). The number is the deepest the item goes into a body, in cm, and over 1 cm is a fault. It was always the woman's head, in the guard and the walking guard:

| Haft top (m) → | 0.30 | 0.37 | 0.444 (ref) | 0.52 | 0.60 | 0.75 |
|---|---|---|---|---|---|---|
| Head ×1 (0.9 kg) | -2.3 | **1.7** | **2.3** | **2.2** | -2.2 | -3.1 |
| Head ×1.15 | | | **3.9** | | | |
| Head ×1.35 | | **4.8** | **6.1** | **5.5** | | -3.1 |
| Tempo at head ×1 | 1.55 | 1.22 | 1.00 | 0.84 | 0.72 | 0.56 |

Carried (the walker's ten carried clips, the carry rule, the arms off the hips), every variant passed: fist 0.8 cm, wrap 94 to 96°. The heavy blow's spec (a right-to-left sweep) failed for haft tops of 0.52 m and under, because the tip it reads is the axe's top corner, not a blade's end.

What it shows:
- **The clips tolerate length.** A one-hander's clips break where the item passes the body, not where it is longer or shorter. The faults sit in a band of lengths that puts the head beside the fighter's own head in the guard, and both ends of the range miss that band. Checking "the shortest and the longest" (holding.md) would have passed an axe that fails in the middle.
- **The tempo clamp is the binding limit.** Inertia grows as length squared, so ±15% of tempo is about ±10% of length at a fixed head, or -25% to +40% of head mass at a fixed length (with a 0.5 kg head, a haft top of 0.50 to 0.65 m; with 1.3 kg, 0.34 to 0.44 m). Inside a class, parts vary the look far more than the size.
- **A class's own guard decides the clearance.** The reference axe, inside the clamp, fails in the sword's guard. So an axe needs its own guard and set, and no envelope can stand in for that.

## 2. The item track, baked from the hand

The hand socket's frame on every key, a point 0.8 m up its +Y as the tip. At speed means the move set's active window. Edge lead is the share of the tip's motion across the handle that goes along -Z (attachment.md asks at least 0.7 for a one-sided head). Flat is the share along ±X (a blade asks at most 0.5). The turn is the roll about the handle that would put -Z onto the motion. The wrist bend is measured from joint positions (the forearm against the hand) relative to rest. A snap is the largest wrist turn between two keys.

| Clip (source) | Tip peak | Jitter, slow keys (roll / tip) | Snap per key | Wrist bend (max) | Keys past 80° / 35° | Edge lead / flat | Turn to lead (mean, max) |
|---|---|---|---|---|---|---|---|
| Open_Slash (Kimodo, keyed) | 19.6 m/s | 0.30° / 8.6 mm rms | 66°, tip 111 cm | 113° | 16 of 48 | -0.94 / 0.24 | 166°, 180° |
| Open_Backhand (keyed) | 18.2 | 0.31° / 8.0 mm | 90°, tip 134 cm | 151° | 14 of 43 | 0.85 / 0.15 | 10°, 17° |
| Open_Heavy (keyed) | 17.0 | 0.17° / 3.7 mm | 39°, tip 91 cm | 105° | 23 of 58 | 0.09 / 0.54 | 85°, 177° |
| Fight_Slash (Kimodo, text only) | 16.6 | 0.12° / 6.9 mm | 44° | 71° | 4 of 51 | 0.77 / 0.39 | 26°, 60° |
| Fight_Chop (text only) | 14.1 | 0.32° / 6.9 mm | — | 84° | 6 of 48 | 0.12 / 0.69 | 78°, 144° |
| Clear_Block, Low_Guard | 3.3, 0.3 | 0.01° / 0.2 mm | — | 30° | 0 | — | — |

The `Keyed_*` takes are identical in the right arm to the `Open_*` clips the game plays.

- **Noise is not the problem.** Kimodo's output is smooth. Smoothing the roll about the handle would move the wrist under 1.5°, and an axe bit 18 cm off the handle under 1 mm. That cleaning step can go.
- **Snaps are.** In every keyed blow the wrist turns 39 to 90° in one key, 1/24 s of clip time, or about 4,000°/s at the backhand's play rate of 1.9. The snap falls just before a hand key (backhand keys at 0.40 and 0.65 s, snap at 0.58 to 0.63; slash key at 0.45, snap at 0.38 to 0.42). Kimodo meets a keyed hand turn late and fast. The take without keys never turns over 44°.
- **The wrists are already past human range.** The keyed blows bend the hand 105 to 151° off the forearm. People reach about 80° of flexion and 70° of extension (AAOS, *recalled*). A "wrist solved back onto the track" has no room left to turn in.
- **Which edge leads belongs to the take.** The slash leads with the +Z side and the heavy with the flat. That suits a two-edged sword (the slash's flat share is 0.24), but an axe would strike with its back in the slash and its cheek in the heavy. Turning the item 85 to 180° about its handle is a different grip, not a wrist correction. Only the backhand (10°, at most 17°) is within what a wrist can absorb. attachment.md's figure (-0.77 to -0.90 along -Z "through the fight's clips", measured 2026-10-27) does not hold for today's slash and heavy.

**Can Kimodo take the path back as keys?** Yes, in the form it already uses. A `right-hand` constraint holds the hand's position and its global rotation on the frames given ("the wrist position and rotation along with the hand end position", Kimodo's docs, *read*). `kimodo_keys.py` writes these from a spec's grip, blade and back-of-hand. Kimodo's benchmark gives 3.2 cm mean error on hand position at keyed frames and does not measure rotation (*read*). On our kept takes, at the keys' own times, the back of the hand is within 1 to 8° of its key and the blade within 13 to 31° of its key, with a steady 13 to 23° even where the timing is right. Either the take bends the blade line, or the key's blade is not exactly the socket's +Y. So keys steer the edge reliably through the back of the hand, but they do not pin the blade line. The edge has to be keyed before generation (the `back` key), and the wrist checked after.

## 3. Two hands

The off hand's target is `second` metres along the lead hand's +Y (holding.md). Arm: upper 0.232 m, fore 0.212 m, wrist to grip 0.105 m (base body).

| Clip | Second -0.11 m (longsword) | -0.25 m | -0.45 m (long haft) |
|---|---|---|---|
| Open_Slash | 22 of 48 keys in reach, hand moved up to 72 cm | 20, 71 cm | 14, 80 cm |
| Open_Backhand | 20 of 43, 80 cm | 25, 73 cm | 24, 69 cm |
| Open_Heavy | 39 of 58, 79 cm | 27, 83 cm | 21, 101 cm |
| Low_Guard, Walk_Loop, Fight_Chop | 0 | 0 | 0 |

One-handed clips swing the lead arm wide and keep the off arm at the side, so IK can only drag the off hand across. Shipped games author a two-hander's clips with both hands on the weapon and let IK correct centimetres (Lyra and Unity's Two Bone IK in [the loadouts note](2026-10-25-loadouts-and-animation-layers.md); Unreal's forum answer for "animation made for a different sized weapon" is IK switched by a curve, *read* in the earlier note). Limits for the solve: reach at most about 0.95 of the arm, so the elbow does not pop straight; an interior elbow angle of at least 30°; and the second grip varying across the envelope by no more than the IK budget, a few centimetres. So for a two-hander's handle, the second grip is part of the reference item, not of the envelope.

## 4. Hits and the server

- **Today**: `terra_creatures::melee::contact` sweeps a segment from `HILT_M` 0.15 m to `TIP_M` 0.83 m up the posed hand socket, from the last rendered frame to this one, against an upright capsule (`terra_motion::blade`). The length is the sword's for every item, the pose is the client's, and the step is the frame rate. `terra_motion::duel` sweeps a stand-in arc. movement.md plans hit volumes "baked at export into short tracks in the body's root space" for the server (phase 6), and holding.md gives each take its own baked hit track. Neither is built.
- **The item track is that track.** It holds the hand socket's frame by clip time, for each take and each body template (four), in the root's space. Any item's segment is its own hilt and tip along +Y, so there is no baking per item, and assembly gives those two numbers exactly. On a server that decides (D51), a tick reads the track at the move's clip time. That is a pure function of data and time, so it is deterministic on the server. Clients only predict, so lockstep determinism across machines is not needed (For Honor's deterministic peer-to-peer simulation, GDC 2019, is the other road, *read* listing). Chivalry's melee uses tracers along the swinging weapon (*read*, forum), the same idea driven by the skeleton.
- **The sampling rate matters.** A straight sweep from tick to tick against the true path the hand takes between ticks (bones slerped), through the active frames at the move's speed:

| Move | 30 Hz: tip per tick, stray | 60 Hz | 
|---|---|---|
| slash (×1.72) | 39 cm, 4.6 cm | 24 cm, 2.0 cm |
| backhand (×1.9) | 147 cm, 39.6 cm | 102 cm, 16.8 cm |
| heavy (×1.5) | 100 cm, 13.1 cm | 57 cm, 5.4 cm |

  The backhand's 1 m a tick is the wrist snap of section 2. Fix the snap, then bake the track at 120 Hz or sweep in sub-steps, so a stray stays under 2 cm against hurt capsules 15 to 25 cm in radius. The cost is small: a few clips times four templates times 120 samples a second, 28 bytes a sample. A thousand players (D52) cost one segment against a capsule per active blow per tick per nearby body.

## Prior art

- **Holding a weapon in a generated or retargeted clip is solved by IK goals on the item.** Unreal's IK Retargeter has a Weapon Goals op: a `hand_attach_r` bone per hand, a `weapon_r` bone, IK goals that pull the hands onto them, and a Weapon Scale (*read*, Epic API). That fixes hands to a known weapon path, which is our track. It does not make the path itself right.
- **Motion editing**: motion warping (Witkin and Popović 1995) and displacement maps lay a smooth offset over a clip to meet new keys (*recalled*). Gleicher's spacetime retargeting (1998) holds hand-on-object constraints across a clip (*recalled*). Ho, Komura and Tai's interaction mesh (SIGGRAPH 2010) keeps the spatial relations between body parts and objects, sword fighting among its examples, while a motion is adapted (*read*, abstract). All of them edit by small displacements. None turns a blade 90° or more about its handle.
- **Learned hand–object motion**: CHOIS, OMOMO and HOI-Diff (in the earlier note), ManipNet (SIGGRAPH 2021, *recalled*) and GOAL (CVPR 2022, *recalled*) make the object's motion together with the body's, or the body from the object's. A 2026 preprint warns that retargeting their outputs directly lets "wrists become unreachable" (*read*, search summary). None is in Kimodo or Uthana.
- **Motion-capture props**: MotionBuilder's HumanIK fixes hand placement on props after retargeting with its control rig. The forums call adjusting hands on weapons a known pain (*read*, search summaries). Rokoko streams a prop as its own animated object (*read*). Captured props are cleaned as rigid bodies and filtered (*recalled*). Captured sticks therefore have the edge right by construction. Generated clips do not.

## What could fail, and the cheapest test of each

1. **The keyed takes' wrists**: snaps of 39 to 90° a key, bends to 151°. Test: a `moves.ts` row on the composed pose, with flexion at most 80°, extension at most 70°, radial deviation at most 20°, ulnar deviation at most 35°, and a turn of at most 45° a key at 24 fps, on every blow. It fails on main today (16, 14 and 23 keys over). Fix: the hand keys' `back` and timing, more keys at the snap, or a limit on the wrist before the take is kept (clip.ts). This comes before any item work, because a sword already shows it.
2. **The edge leads only if the take is made so.** Test: the edge-lead row attachment.md asks for (-Z at least 0.7 at speed for a one-sided head; ±X at most 0.5 for a blade). On main the slash's -0.94 fails a one-sided head and the heavy's 0.54 flat fails a blade. Fix: each class's takes keyed with the edge's side in `back`, and cleaning limited to about 15°.
3. **An envelope checked only at its ends.** Test: the move check over a grid of 5 lengths × 3 head masses inside the clamp, about 2 minutes on four cores. Passing at the corners alone does not count.
4. **Hit sampling**: the stray of a straight sweep at the server's rate, at most 2 cm.

**The proof** (assembled-items.md's step 3, with numbers): the one-handed axe class gets its own three takes (a swing, a carry, a block) keyed for its reference item: a haft top of 0.444 m and a 0.9 kg head. It is run with 15 items inside the clamp (haft top 0.40, 0.43, 0.46, 0.49 and 0.51 m × heads of 0.7, 0.9 and 1.1 kg, only the pairs whose tempo is within 0.85 to 1.15), on all four templates, on the composed pose. It holds if, on every item, template and key:
- the fist is at most 1 cm off the handle and wraps at least 80°;
- the item is at most 1 cm into the body and the hands at most 2 cm;
- the edge leads at least 0.7 at speed;
- the wrist is within the limits of risk 1, with at most 45° a key;
- cleaning moves the wrist no more than 15°;
- the baked track strays at most 2 cm at 120 Hz.

It fails if three rounds of keys cannot pass the edge and wrist rows together (two jobs without a gain, AGENTS.md). In that case the fallback is a held-item pose layer per class authored by hand, a prop bone as the shooters use.

## Limits

- The masses are modelled: steel at 1.2 kg/m of blade, an ash haft at 0.43 kg/m, the head a point. The tempo numbers move with them, but the conclusion about the clamp does not.
- The axe was measured in the sword's clips. Its own takes will clear differently. What carries over is that failures are non-monotonic in length.
- Wrist angles come from joint positions on the base body, relative to the T-pose. The limits are adult averages.
- No take with two hands keyed exists, so section 3 bounds the problem rather than measuring a solution.

## Sources

- Measured: `tools/characters/moves.ts` (2026-10-28 main), `assets/models/character/fight.glb`, `assets/data/moves/sword.shielded.ron`, `art/animation/sets/fight.json`, `crates/terra_creatures/src/melee/contact.rs`, `crates/terra_motion/src/blade.rs`, `crates/terra_motion/src/duel.rs`.
- Kimodo, its constraints and benchmark: `~/.cache/terra-kimodo/kimodo/docs/source/user_guide/constraints.md`, `key_concepts/constraints.md`, `benchmark/results.md` (changelog of 2026-10-15, Kimodo-SOMA-RP-v1.2); [github.com/nv-tlabs/kimodo](https://github.com/nv-tlabs/kimodo).
- [Epic, IKRetargetWeaponGoalsOpSettings](https://dev.epicgames.com/documentation/en-us/unreal-engine/python-api/class/IKRetargetWeaponGoalsOpSettings); [IK Rig retargeting](https://dev.epicgames.com/documentation/unreal-engine/ik-rig-animation-retargeting-in-unreal-engine).
- [Ho, Komura, Tai, Spatial relationship preserving character motion adaptation (SIGGRAPH 2010)](https://history.siggraph.org/?p=105435).
- [Chivalry tracers, Unreal forum](https://forums.unrealengine.com/t/problems-with-tracers/66903); [For Honor's deterministic simulation, GDC 2019](https://gdcvault.com/play/1026077/Back-to-the-Future-Working); [server-side traces and animation ticking, Unreal forum](https://forums.unrealengine.com/t/collision-trace-not-working-if-server-not-looking/539110).
- [MotionBuilder prop hands, Unreal forum](https://forums.unrealengine.com/t/suggestion-export-animation-as-retargeted/345983); [Rokoko prop streaming](https://github.com/Atlas-3D-Artist/rokoko-studio-live-blender).
- SynManDex, a 2026 preprint (wrists unreachable on direct retarget), seen only in a search summary, its address not confirmed; CHOIS, OMOMO, HOI-Diff as cited in [2026-10-28-assembled-equipment.md](2026-10-28-assembled-equipment.md).
