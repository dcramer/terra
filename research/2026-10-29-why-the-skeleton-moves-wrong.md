# Why does our characters' skeleton move wrong: the source clips, the carry onto our body, or the game's layers?

2026-10-29. The developer: "the skeleton just doesnt move right", and "one issue we've had is you twist joints and do weird shit instead of applying the animation? like swimming looks stupid". This note puts a known-good motion (Quaternius's Universal Animation Library walk and jog, the versions we shipped before 2026-10-14, taken from `mannequin.glb` at `5667c078~1`) through each stage beside ours, and measures how far every run-time layer moves every bone off what the clip says. It changes no clip.

## Short answer

It is mostly **the clips themselves** (c), with **two run-time layers** (a) adding to it while moving. The carry onto our body (b) is not at fault.

- **The carry is faithful.** Quaternius's walk and jog on our body point every limb the same way as on his mannequin, frame by frame: 0.0 degrees apart on the upper arms, forearms, hands, thighs and shins. The only differences are fixed and do not move: the collarbones point 41 degrees apart, the feet 6 and the palms 8, all the same in every frame. These come from the two bodies' rest shapes, not from motion.
- **Our clips are worse than the known-good ones before the game touches them.**
  - The **swim** is the worst. It was keyed standing up, because Kimodo cannot make swimming: a high-knee march with the arms windmilling out to the sides. The knees bend up to 168 degrees and the thighs lift to 95. When the game lays the body down 80 degrees, that becomes a person kneeling and pedalling in the air, not a swimmer kicking. Tilting the body can never turn it into a crawl.
  - The **walk** (Kimodo) is a plausible walk, but stiff and upright. Quaternius's leans 11 degrees into the step; ours leans 0 to 2. Our knee never softens as the heel lands (3 degrees against 9 to 14). Our hips turn lopsidedly, 10 degrees one way and 4 the other. Our arms swing a little less.
  - The **jog** has Kimodo's legs under arms written by a formula (`swing_arms` in `made_clips.py`). It runs bolt upright, leaning 11 to 16 degrees where Quaternius's leans 23 to 29, and the head tips back.
- **In play, two layers bend the moving body a lot.** Standing, the layers barely touch the body. Jogging, they do:
  - The **foot lock** bends the legs on flat ground: the foot by up to 95 degrees, the shin 59 and the thigh 40, about 10 to 18 degrees on average, on half of all frames.
  - The **carried-tool rule** turns the right forearm by up to 71 degrees (50 on average) and holds the wrist 30 degrees off the clip all the time.

  On flat ground, with a clip played at its own pace, shipped foot locking moves a leg by a few degrees.

## Films and stills

Under `~/.cache/terra-review/anim-ladder/`. They were rendered in Blender and skinned as the game skins (an uncommitted script, D249, kept with the job's handoff). Each body plays its own clip in place, from the side and from the front-left.

| File | Columns |
|---|---|
| `walk-clips-side-by-side.mp4`, `walk-stills-4-moments.png` | 1 Quaternius's walk on his mannequin, 2 the same carried onto our body, 3 our walk (Kimodo) on the mannequin, 4 our walk carried onto our body |
| `jog-clips-side-by-side.mp4`, `jog-stills-4-moments.png` | the same four for the jog, the player's own running clip (the player never plays `Walk_Loop`: `figure.rs` maps moving to `Jog_Fwd_Loop`; townsfolk and fighters walk) |
| `jog-in-play.mp4` | 5 the player jogging a route in play, every layer on, the building hammer in hand (CPU render, from behind) |
| `swim-clips-side-by-side.mp4`, `swim-stills-4-moments.png` | 1 our crawl as keyed (standing) on the mannequin, 2 carried onto our body, 3 laid 80 degrees as `swim.rs` lays a surface stroke. We have no known-good swim on disk: Quaternius's library copies are not in this checkout, and our clip list never had his swims |

## Stage by stage, in numbers

Joint ranges over each loop, in degrees: the least and the most, and the span between them. "Hip flex" and "arm swing" are the thigh's and the upper arm's angle forward of straight down. "Turn" is the hip line's or shoulder line's turn about the vertical. "Lean" is the pelvis-to-neck line forward of upright.

| Measure | Quaternius walk, mannequin | same, our body | our walk, mannequin | our walk, our body | a person (gait lab norms) |
|---|---|---|---|---|---|
| knee | 9-87 | 9-87 | 3-71 | 3-71 | about 5-20 standing on it, 60-65 swinging |
| hip flex | -21 to 47 | -21 to 47 | -20 to 29 | -20 to 29 | -10 to 30 |
| arm swing | -24 to 9 | same | -17 to 12 | same | about 30 in all |
| hips' turn | -2 to 2 | same | -10 to 4 | same | about ±5, even both ways |
| shoulders' turn | -11 to 9 | same | -7 to 3 | same | about ±5 to 7 |
| lean | 11-12 | 11-13 | 0-2 | -1 to 1 | a few degrees forward |

| Measure | Quaternius jog | our jog | |
|---|---|---|---|
| knee | 0-129 | 23-116 | |
| hip flex | -40 to 78 | -29 to 56 | |
| elbow | 79-94 | 65-100 (rule-made) | |
| lean | 23-29 | 11-18 | |
| pelvis bob | 23 cm | 6-7 cm | Quaternius's is a bouncy, stylised run |

| Measure | our crawl (standing as keyed) | a front crawl |
|---|---|---|
| knee | 5-168 | about 0-40, legs nearly straight |
| hip flex | -12 to 95 | about -10 to 20, a kick from the hip |
| body roll (the shoulders' turn, once laid) | 44-47 span | 70-90 span (±35-45) |
| arms | windmill out to the sides | enter ahead, pull under the body |

The carry, measured between the same clip on the mannequin and on our body: on every limb the mean and the worst are both 0.0 degrees (walk and jog). The collarbone is 40.9 degrees off in every frame, the foot 6.1 and the palm 8.1, so these are rest-shape offsets. The pelvis sits 6 cm higher on our body, which is its longer legs.

## What each run-time layer does to the bones

Measured in play with an uncommitted audit (a module in terra_walker, D249, kept with the job's handoff and not landed). Between consecutive pose writers it takes a snapshot of every bone of the player's body, with the writers held in the one order their constraints allow, so each difference belongs to one layer. The runs were on the bare yard: about 570 frames jogging a route and 450 frames standing, with the building hammer in hand as play starts. "Frames" is the share of frames on which the layer moved any bone by half a degree or more.

| Layer (crates/terra_walker/src) | Bones it touches | Jog: worst, mean, frames | Idle: worst, mean, frames | When it runs | Shipped games? |
|---|---|---|---|---|---|
| `layer` (hold masks) | an arm or both, the whole pose | 0 (nothing held in a stance) | 0 | a stance or a carry pose held (sword drawn, torch) | yes: per-bone layered blend (Unreal, ALS) |
| `carry_arm` | shoulder blade, upper arm | 0 | 0 | a carry pose held (torch) | yes: mesh-space arm overlay |
| `carry_hand` | right forearm and hand | forearm 71 (mean 50, 65% of frames), hand 30 (always) | hand 30 (always) | a tool carried by the rule (the hammer, an axe) | partly: games carry a tool on its own arm pose; turning the forearm per frame by rule is ours |
| `grip` | 15 finger bones | 107 (fingers closing) | 116 | anything in the hand | yes: hand pose per grip |
| `feet` (foot lock, D455) | thigh, calf, foot of both legs | foot 95, calf 59, thigh 40; mean 10-18 on those, 49% of frames | foot 4, under 1 on average | every body with legs on the ground | yes, but on flat ground at the clip's own pace it should move a leg a few degrees |
| `arms` (`keep_off_hips`, D535) | upper arms | 12 (mean 6-8, 48%) | 0 | a hand hanging by the hip | no: a per-pose fix-up |
| `shield_arm` (D554) | shoulder blade, upper arm, forearm | 0 (no shield) | 0 | a shield strapped on | partly: hand IK to a target is normal |
| `gaze` (D535) | neck, head | head 9 (1% of frames) | head 11, neck 7 (always) | always in play | yes: look-at |
| `bend` | none of its own | | | the helper `arms` and `gaze` turn bones with | |
| `lift` | the skeleton's root | 0 (base body) | 0 | a morphed body (heavy) in a clip that meets the floor | rare: per-clip, per-body floor offsets |
| `strap` | none (it moves the shield's look, not a bone) | | | a shield worn | yes |
| swim tilt (`swim::lay`) | the whole body, as one | | | 80 degrees laid while stroking at the surface, up to 170 diving; none treading | yes: Subnautica and Unreal tilt a swimmer, but over a clip made lying down |
| all of them, clip to screen | 27 bones | the fingers' 107 aside, foot 95 | the fingers aside, the wrist 30 and the head 11 | | |

The swim was not run in play: the lake's 30 km world is too slow on the CPU renderer, which is all this job had. From the code: the foot lock is let go while swimming (`feet`: a swimmer is off the ground), and the tilt above lays the whole body down. The hand, grip and gaze layers act as they do standing.

## What bends joints inside the clips before the game sees them

| Step | What it does | Shipped games? |
|---|---|---|
| `made_clips.py`'s carry from Kimodo's skeleton | each bone gets the source bone's turn from its T-pose; the hips' travel is taken off | yes: every retargeter |
| `made_clips.py` roll handed down | an upper arm's or thigh's roll moved to the forearm or shin | yes, with `twist.py`'s twist bones baked in |
| `made_clips.py` loop closing | the last frames eased into the first | yes |
| `stand.py` (the idle's "stand") | the hips raised toward the rest pose, the head drawn back over them | no: a per-clip pose fix-up |
| `swing_arms` (the jog and sprint's "arms") | the arms wholly replaced by a formula for swing, elbow, tuck, wrist and palm | no: games use the capture's arms, or a captured arm overlay |
| `close_hands` ("fist") | the fingers closed to Quaternius's fist | yes (a hand pose) |
| `kimodo_keys.py` reach solver (fight clips) | key poses solved by two-bone IK, then Kimodo fills in between them | no: an animator keys or captures a swing; IK keys fed to a generator are ours |
| `wrist_limit.ts` (fight clips, D565) | every frame's sword wrist clamped into a person's range | rarely: a cleanup pass by hand, not by rule on every frame |
| `hold.py` and `aim.py` (fight clips) | the shield arm solved to its place; a blow turned by its aim (D573) | partly: aim offsets are normal at run time, baked into the clip they are not |
| `mirror.ts`, `markers.ts` | left-hand copies; foot lock curves | yes (ALS's FootLock curves) |

The normal run-time ones that a shipped game also has are foot IK and locking, look-at, a hand pose per grip, hand IK to a weapon or shield, twist bones, and a swim tilt. The ones that are not normal are the per-pose fix-ups (`arms`, `stand.py`), arms written by a formula (`swing_arms`), turning a forearm by rule every frame (`carry_hand`), and clamping every frame by rule (`wrist_limit.ts`). Shipped games get a natural pose from the capture itself and use layers only to fit it to the ground, the target or the hand.

## Read, in plain words

The pipe that carries a motion onto our body works: give it a good walk and our man walks just as well as the mannequin does. What goes in is the problem.

- **The swim** is a man marching on the spot and windmilling his arms, tipped over. No amount of tipping makes that a swimmer.
- **The walk and the jog** are believable but stiff. They stand too straight, with no lean into the step, and the knee stays locked as the heel lands. On the jog, the arms come from a formula rather than from a person.

On top of that, while the player runs, two of the game's own layers wrench the legs and the right forearm by tens of degrees. The foot lock does it to the legs on flat ground, where it should hardly act at all. The carried-tool rule does it to the forearm, all the time. Standing still, the layers leave the body nearly alone, apart from the hand and a 10-degree turn of the head.

## What I recommend

As shipped games do it:

1. **Motion-captured locomotion and swimming, carried by our existing pipe.** Use clips captured from people for the walk, jog, sprint, idle and swims, not generated ones. Candidates:
   - Quaternius's current library for walk and jog: we own it, it is CC0, and it is proven faithful above.
   - A paid capture pack with real swimming, such as MoCap Online's or Kevin Iglesias's (these need the developers' purchase, D526).

   Mixamo's swims are free but carry Adobe's terms. Epic's Game Animation Sample is only licensed for Unreal. Checks: put each through `clip_check.py`, then a film like these beside the mannequin.
2. **Stop rewriting clips by rule.** Retire `swing_arms` and `stand.py` once the source is captured. Keep twist baking, loop closing, mirroring and markers.
3. **Turn the foot lock and the carried-tool rule down to what a game's would do, then measure again** with the audit in this note. Look first at why the lock bends a leg 10 to 18 degrees on flat ground while jogging: a pace mismatch, the lock's step threshold, or the clip's own foot slide. The carried tool wants an arm pose captured for carrying it (an overlay), not a forearm turned every frame.
4. **The fight clips** go the same way, in a later job. A captured or hand-keyed sword set (Quaternius's sword clips, or a paid pack) carried by our pipe, with run-time hand IK only.

What it takes: one job to gather captured candidates and film them beside ours with these tools; a decision by the developers on a paid pack; then one job each to swap the locomotion and the swims, and to cut the run-time layers down.
