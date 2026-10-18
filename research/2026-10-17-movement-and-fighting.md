# How shipped games move and fight their characters and monsters

2026-10-17. **Question:** after five rounds of a scripted troll fight (docs/troll-fight.md, D402) that the developer still called terrible, "both looks and mechanics of our guy, some texture clipping, some movements not working", how do shipped games and engines build the system that moves and fights a player, people and monsters of many sizes, and which of it do we port (D196)? It informs [docs/movement.md](../docs/movement.md).

## Answer

The troll fight failed because it had no movement system under it, only a clip player (`play::staged`) and a script. Each round patched in the script one piece of what a game's animation system does: root motion (round 3), contact timed by the blade (round 4), hit reactions (round 5), a retarget pose (`staged.bend`, round 2). Shipped games build these once, in layers, and every character goes through them: an intent from a player, a mind or a scene; a capsule moved by code; actions (moves) as data with phases, cancel windows and hit volumes; root motion warped to a target; a pose from blend spaces and a state machine, with inertialised transitions; IK and additive layers after it; and three kinds of volume per body (push, hurt, hit) so nothing passes through anything. Everything we need is either in Bevy 0.20's animation graph or portable from MIT sources (Godot, ozz-animation, O3DE, Jolt); Unreal's features are worked from its documentation only.

## What failed in the troll fight, and the cause

From the five reviews and handoffs in `~/.cache/terra-review/troll/` (r1 4/10, r2 to r5 rising to 6/10 and stopping there):

| Symptom the review saw | Cause | What a game has instead |
|---|---|---|
| Blows swung at air 3 to 4 m away (r2) | The clips carry their steps in the hips; nothing extracts them | Root motion extracted at import, the capsule moved by it |
| The figure jumped when one clip gave way to the next; false cuts (r4) | The scene moved the figure to where the hips were by hand | The same: root motion accumulated every frame, not settled at the end |
| He froze mid wind-up at "contact" (r3) | Contact read from the hand's speed | Contact and active frames marked on the move, measured from the weapon |
| The man thrown before the fist lands; a cartwheel for a dodge (r5) | Reactions started by a frame count in the script; the roll clip is not a dodge | Reactions caused by the hit event; a dodge clip made to a spec |
| The troll's hands in its belly, the shield through his chest (r1) | Clips made on a slim person played on heavier bodies | A retarget pose per body, baked at export; hand IK to the grip; self-collision fix-up |
| The man inside the troll's silhouette, his arm through its leg (r4, r5) | No volume keeps two bodies apart | A pushbox per body, separated every tick |
| The troll stood like a pillar when cut (r4) | No reaction layer | Additive flinches by zone, staggers by poise |
| Stalls between moves (r1, r4) | Moves started by the script, idles in between | A move buffered in the last one's cancel window |
| The roll a "superman float" into a boulder (r5) | Movement by script, no collision with the world | The capsule collides; the move's root motion is clamped by it |

## How shipped games do each part

**Root motion against in-place.** Two ways to move a body: code-driven (a capsule moved by input through springs, the animation chosen to match it) and data-driven (the clip's root moves the body). Locomotion in responsive games is code-driven, with the pose made to fit: Unreal's distance matching, stride warping and orientation warping (Lyra, the Animation Locomotion Library), Holden's damper pulling the character toward the simulated capsule with its deviation clamped. Attacks, dodges, staggers and traversal are data-driven: their root motion moves the capsule, because a lunge's travel is part of how it looks. Unreal's Motion Warping, and Gears of War 4's before it, then bends that root motion inside a marked window so a strike ends at its target: skew it toward the target's position, turn it to face, within limits.

**Motion matching.** Instead of a graph of clips, search a database of long takes every few frames for the frame whose pose and next half second of trajectory best fit what the player asks, and blend to it. Ubisoft's For Honor (Clavet, GDC 2016) shipped it first, with markup on the takes for what cannot be inferred (attack types, guard stances); The Last of Us Part II used it for locomotion, traversal, melee and cover (Mach and Zhuravlov, GDC 2021); Unreal 5 ships it as the Pose Search plugin with a sample of hundreds of captured clips. It needs tens of minutes of dense, consistent motion per body and style, it is costly per character, and it still hands attacks to authored moves. O3DE's MotionMatching gem is an open implementation (Apache-2.0 or MIT).

**Blend spaces and state machines.** Most shipped action games, Elden Ring and Monster Hunter among them, still play locomotion through 1D or 2D blend spaces (speed, and direction against facing) inside a state machine (idle, start, move, stop, turn in place, fall, land), with upper-body masks so an action plays over legs that keep walking. Transitions are inertialised (Bollo, Gears of War, GDC 2018): the new pose plays at once and the difference from the old decays like a spring, so only one pose is evaluated and a transition never lags the input. Godot's AnimationTree (state machine, BlendSpace1D and 2D, one-shots, root motion) is the closest open model.

**IK and look-at.** After the pose: two-bone IK on legs with a pelvis offset puts feet on uneven ground (every shipped third-person game); two-bone IK on arms puts the off hand on a two-handed grip and keeps a shield's arm where its strap is; a look-at and aim chain spread over spine, neck and head turns the body to a target. Godot's SkeletonModifier3D family (two-bone, FABRIK, CCD, look-at, spring bones) and ozz-animation's samples (two-bone, aim, foot IK) are MIT.

**Procedural layers.** Additive layers over the pose: a lean from acceleration and turn rate; breathing; directional hit flinches by zone, short and local, the struck part moving and the rest following. Physical animation drives the skeleton's rigid bodies toward the animated pose with motors, its strength dropped where a blow lands and recovered, so hits push bodies without a clip per direction: NaturalMotion's Euphoria (GTA IV, the Red Dead games) does it with a behaviour controller; Unreal's Physical Animation component and Jolt's ragdoll (`DriveToPoseUsingMotors`, MIT) do the simpler pose-matching version. Deaths go to full ragdoll with the motors off. All of this needs a physics engine in step with the pose.

**Attack data.** Every action game describes a move as frames, not as a clip: start-up (wind-up), active (the hit volume can land) and recovery, from fighting games' frame data. FromSoftware's games carry it in an event track beside each animation (the TAE files modders edit): attack windows that name hit capsules between points on the weapon model ("dummy polys"), invulnerability frames for the roll, windows where input may cancel into the next move, and poise (hyper armour) through a heavy wind-up. Hit volumes ride the weapon: a capsule from hilt to tip swept from last frame's place to this one's, so a fast blade cannot pass through between frames. Monster Hunter gives each monster part its own hitzone values, as our zones (D119), and each move a motion value; hit-stop scales with the weapon (research/melee-feel.md). God of War and Devil May Cry let the next move start in a window of the last one's recovery (animation cancelling); Souls games commit you to a move except into a dodge.

**Big and many-legged creatures.** Big monsters in Monster Hunter, Elden Ring and God of War are keyframed, slower by their size (Froude scaling), with turn-in-place clips, long wind-ups and recoveries, foot IK onto the ground, and root motion on every attack. Bodies unlike a person's are animated by rule over a few poses: Spore fits motion authored on a generic body to any user-made body by IK (Hecker et al., SIGGRAPH 2008); Overgrowth blends a handful of key poses by speed with springs (Rosen, GDC 2014); Rain World draws limbs over a few physics points (GDC 2016). Many-legged walkers step a foot when it strays past its reach from its rest point, in alternating groups (tripods for six legs, diagonals for four), the body's height and tilt following the plane of the planted feet. Our quadruped and bird plans already work this way offline (`tools/creatures/animal.py`).

**Bodies and gear that do not pass through.** Fighting games give every body three kinds of volume: a pushbox that keeps bodies apart, hurtboxes that can be struck, and hitboxes that strike. Action games do the same with a capsule per character separated every tick (a big monster pushing a small one aside, rarely the reverse), capsules on bones for hurt zones, and swept weapon capsules. Gear is kept out of its own body by authoring the moves on the body's proportions (retarget poses, Unreal's IK Retargeter), by IK on the hands, and by checking every clip on every body offline, as our gear check already does. Two bodies in close contact (a grab, a finisher) are paired animations made together and aligned to a shared point before they play (Unreal's Contextual Animation plugin; Skyrim's kill moves; God of War's finishers); nothing else is allowed to touch.

**One system for everyone.** Unreal's Character Movement, Unity's controllers, Godot's CharacterBody and every shipped action game drive the player, people and monsters through the same components: input, AI and cutscene write the same intent, and the same mover, move runner and pose layers serve all, tuned per kind by data. Size changes the numbers (capsule, speeds, turn rate, poise), not the code.

## What Bevy 0.20 has

`bevy_animation` 0.20: an animation graph with clip, blend and additive nodes, bone masks, animation events on a clip's timeline, and `AnimationTransitions`, linear crossfades its own documentation calls an unstable temporary API. No root motion, IK, state machine, blend space, inertialisation or physical animation. Physics: Avian 0.7 is for Bevy 0.19, and no release for 0.20 was found (D33 is Leaning; the workspace has no physics crate). `bevy_skeleton_modifiers` (MIT, last pushed 2026-10-09) ports Godot's IK and spring bones for Bevy 0.19. What we already have: our own skinning shader, spring bones (`swing.rs`), playback rate matched to pace (`pace.rs`), a one-clip-at-a-time player with 180 ms crossfades (`play/figure.rs`), and the staged figures' bends and hips bookkeeping (`play/staged`).

## Port sources (D196), checked 2026-10-17

| Source | Latest | Licence | Take |
|---|---|---|---|
| Godot | 4.7.2, 2026-08 | MIT | AnimationTree's state machine, blend spaces and root-motion accumulation; SkeletonModifier3D's IK, look-at, spring bones |
| ozz-animation | 0.17.0, 2026-08 | MIT | Motion extraction (root from hips), additive and partial blending, two-bone and aim IK, the foot-IK sample |
| O3DE MotionMatching gem | development branch | Apache-2.0 or MIT | Motion matching, if we ever need it |
| Jolt Physics | 5.6.0, 2026-07 | MIT | Ragdoll driven to a pose by motors, as a model for physical animation on whatever physics we use |
| Unreal Engine | 5.x | Proprietary | Documentation only: Motion Warping, stride and orientation warping, distance matching, Physical Animation, Contextual Animation |
| FromSoftware's TAE, as documented by DSAnimStudio | 2026-05 | GPL-3.0 | Documentation only: how a move's event track is laid out |

## Sources

- Simon Clavet, "Motion Matching and The Road to Next-Gen Animation", GDC 2016: https://gdcvault.com/play/1022985/Motion-Matching-and-The-Road
- Michal Mach and Maksym Zhuravlov, "Motion Matching in The Last of Us Part II", GDC 2021: https://www.naughtydog.com/blog/naughty_dog_at_gdc_2021
- Unreal Engine, Motion Matching: https://dev.epicgames.com/documentation/unreal-engine/motion-matching-in-unreal-engine
- Unreal Engine, Motion Warping: https://dev.epicgames.com/documentation/unreal-engine/motion-warping-in-unreal-engine
- Unreal Engine, Locomotion (distance matching, stride and orientation warping): https://dev.epicgames.com/documentation/unreal-engine/locomotion-in-unreal-engine
- Unreal Engine, Physical Animation component: https://dev.epicgames.com/documentation/en-us/unreal-engine/python-api/class/PhysicalAnimationComponent
- Unreal Engine, Contextual Animation: https://dev.epicgames.com/documentation/unreal-engine/API/Plugins/ContextualAnimation/FContextualAnimSet
- David Bollo, "Inertialization: High-Performance Animation Transitions in Gears of War", GDC 2018: https://gdcvault.com/play/1025165/Inertialization-High-Performance-Animation-Transitions
- "Motion Warping in Gears of War 4: Doing More with Less", GDC: https://gdcvault.com/play/1024219/Motion-Warping-in-Gears-of
- Daniel Holden, "Code vs Data Driven Displacement" (2021): https://theorangeduck.com/page/code-vs-data-driven-displacement
- O3DE MotionMatching gem: https://docs.o3de.org/docs/api/gems/motionmatching/annotated
- Richard Oud, "Bringing Life to the Machines of Horizon Zero Dawn", GDC 2018: https://gdcvault.com/play/1025040/Animation-Bootcamp-Bringing-Life-to
- Euphoria (NaturalMotion): https://en.wikipedia.org/wiki/Euphoria_(software)
- Jolt Physics: https://github.com/jrouwe/JoltPhysics
- ozz-animation: https://github.com/guillaumeblanc/ozz-animation
- DSAnimStudio, a TAE editor: https://github.com/Meowmaritus/DSAnimStudio
- Jason McDonald, "Taking an Axe to God of War Gameplay", GDC 2019: https://gdcvault.com/play/1026023/Taking-an-Axe-to-God
- Avian: https://docs.rs/avian3d
- Earlier notes: [2026-10-14-animation.md](2026-10-14-animation.md) (Spore, Overgrowth, Rain World, Kimodo), [melee-feel.md](melee-feel.md) (hit-stop and cancels), [bevy-character-capabilities.md](bevy-character-capabilities.md).

## Not settled

The contents of the Horizon, Last of Us and For Honor talks are from their abstracts and the speakers' reels, not the videos. Whether `bevy_skeleton_modifiers` has moved to Bevy 0.20 was not checked beyond its last push.
