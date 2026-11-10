# Holding and carrying items programmatically: how others do it, and whether we are doing it right

2026-11-09. The question, the developer's, today: "how would you do this programatically and are we doing it right". "This" is a character holding and carrying tools, weapons and props correctly (grip, size in the hand, carry pose, no clipping through jumps and crouches, across body shapes and many item variants) with no animator's eye on every item. It informs [docs/holding.md](../docs/holding.md), [docs/attachment.md](../docs/attachment.md) and [docs/assembled-items.md](../docs/assembled-items.md); it decides nothing.

## The answer

1. **The architecture is the industry's**: a grip at the item's origin on a hand socket, a carry pose per kind overlaid on one arm in the body's frame, a finger layer, IK for the second hand. Lyra, ALS, Unity and Unreal's retargeting all have this shape.
2. **The measuring is ahead of practice**: no source found describes a studio judging holds in numbers with no GPU, each bar proven by a planted fault. Studios use an animator's eye and QA. Keep `holds.ts`; it is what makes the rest possible without an animator.
3. **The solver's objective is wrong in one place**, and it is the coordinator's risk exactly: it ranks a wider margin above staying near the authored pose, so it pays any amount of strangeness for a millimetre of clearance. The basket's answer moved the forearm 26 degrees for 0.5 cm margins and nobody has looked at it.
4. **Everyone who optimises a pose adds a term that holds it to a known-good one**: distance from the source motion (Gleicher), a learned prior (VPoser, style-based IK), or narrowed bounds. The smallest for us: cap the margin's reward at a target (1 cm), then minimise the joints' turn from the authored pose, in degrees.
5. **Offline search over pose constants is unusual in games and does not scale as used**: tuning one look's pose, held against eleven others, is work per item. With hundreds of variants from parts (D182) a pose belongs to a kind, and clearance must come from the item's own size at run time.
6. **The four global "give" constants are the weakest part**: one set for every item length and every body. The planned run-time constraint is the right replacement; it is what robotics does (IK as a weighted sum with collision terms), and more than shipped games do.
7. **Shipped games mostly do not solve carried-item clipping at all**; no primary source was found saying any of the named games does. Our bar (nothing under the ground or in the leg, in any clip, on six bodies) is stricter than the industry's.
8. **Order**: authored pose per kind carries the look; a run-time constraint carries clearance; the search tunes the constraint's few numbers per kind over a grid of item sizes and stays as the regression check.
9. **Not needed now**: learned grasp synthesis (GrabNet, D-Grasp, ManipNet) and learned pose priors. Their data and licences are for research, and contact closing on a handle already does what they do for a cylinder.
10. **Two lines of ours would change**: the give constants of D560, and assembled-items.md's "no collision avoidance" at run time. Both are technical.

Grades: **[P]** primary (the maker's docs, paper, talk or source), **[S]** secondary, **[C]** community, **[M]** from memory, no link checked today. Links were found by search on 2026-11-09 and their summaries read; no page was read in full, so a number quoted from one is the search summary's.

## 1. How shipped games and engines hold items

| What | How | Source |
|---|---|---|
| Lyra (Epic's sample) | One shared animation graph; each weapon is a child Animation Blueprint of `ABP_ItemAnimLayersBase` that fills an interface (`ALI_ItemAnimLayers`) with its own clips and numbers, linked when equipped. "Inspired by Paragon and Fortnite" | [Animation in Lyra](https://dev.epicgames.com/documentation/en-us/unreal-engine/animation-in-lyra-sample-game-in-unreal-engine), [Animation Blueprint linking](https://dev.epicgames.com/documentation/en-us/unreal-engine/animation-blueprint-linking-in-unreal-engine) [P] |
| Lyra, the offset | Each item's offset from its socket is data on the equipment definition (`AttachSocket`, `AttachTransform`) | [docs/attachment.md](../docs/attachment.md#how-others-do-it), sourced there [P] |
| Unreal, hand IK | The weapon rides the right hand; left-hand IK pulls the other hand back onto it where additive layers (aim offsets) make it drift. Epic's staff call the `ik_hand` chain standard on its skeletons | [forum, Epic staff reply](https://forums.unrealengine.com/t/whats-the-purpose-of-right-hand-ik-in-lyra/2743139) [P/C] |
| Unreal, body shapes | The hand IK bones are **not retargeted**, so they stay where the prop needs them on a body of other proportions; the Hand IK Retargeting node then moves the IK chain to the FK hands, favouring one side, and can move "weapon bones" with it. The IK Retargeter has a weapon-goals step | [Intro to retargeting](https://www.unrealengine.com/blog/intro-to-animation-retargeting-in-ue4), [Hand IK Retargeting](https://dev.epicgames.com/documentation/en-us/unreal-engine/animation-blueprint-hand-ik-retargeting-in-unreal-engine), [FIKRetargetWeaponGoalsOpSettings](https://dev.epicgames.com/documentation/unreal-engine/API/Plugins/IKRig/FIKRetargetWeaponGoalsOpSettings) [P] |
| Unity Animation Rigging | Two Bone IK on both arms; Multi-Parent constraints tie the hand effectors to targets under the weapon bone, and blend the weapon between holster and hand by keyed weights | [Unity's blog](https://blog.unity.com/es/games/advanced-animation-rigging-character-and-props-interaction) [P] |
| Ubisoft's IK Rig | Animation kept as goals (hands, feet, hips) and solved per character at run time, so one clip serves any proportions | [GDC Vault 1022984](https://gdcvault.com/play/1022984/), title from the search result [P] |
| ALS (Advanced Locomotion System) | An overlay pose per held thing (rifle, pistol, torch, barrel, box), each with curves saying how much of each arm is laid in local or mesh space | [M]; cited in `carry_arm.rs` by docs only |
| FromSoftware, Monster Hunter | A move set per weapon class, the stance from the pair of hands; items of a class share clips | [our earlier note](2026-10-25-loadouts-and-animation-layers.md) |

**Clipping of a carried item.** No primary source was found (two searches: GDC talks, and developers' statements) saying that Destiny, Monster Hunter, Elden Ring, Valheim, Enshrouded or Crimson Desert prevent a held or stowed item from passing through the body or ground at run time. What was found is players' reports that they do not: a cloak that hides or passes through stowed weapons in World of Warcraft, Baldur's Gate 3 and Dragon's Dogma 2 ([forum](https://us.forums.blizzard.com/en/wow/t/why-nerf-shield-size/907213), [Steam](https://steamcommunity.com/app/1086940/discussions/0/4038103329141472988), [Steam](https://steamcommunity.com/app/2054970/discussions/0/4289188745223041221)) [C]. The practice, from memory [M]: the carry pose is authored against the largest item of its class, classes are kept few, the camera and motion hide the rest, and what cannot be hidden is removed (a stowed weapon hidden under a cloak). First-person games do solve one case at run time, pulling the gun in from a wall by a trace [M]. **Not covered**: a talk from any of the named games on holding; none surfaced.

**How they scale to hundreds of items.** By class, not by item: every item of a class is built to one grip convention and one size envelope, so one set of clips and one hand pose fits all (Lyra's layer per weapon type; FromSoftware's set per class). An item differing in size beyond the envelope is a new class. This is D459's envelope and [assembled-items.md](../docs/assembled-items.md#through-the-animations)'s "one reference item per class" already.

## 2. Programmatic approaches

**Grasp synthesis.**

- GraspIt! (Miller and Allen 2004) closes each finger link until it touches the object and scores the contacts. Ours follows it [M; cited in `grip.rs`].
- GRAB is a captured dataset of whole-body grasps; GrabNet, trained on it, proposes a MANO hand pose for an unseen object in two stages, a coarse pose then a refinement by hand-to-object distances ([GRAB](https://arxiv.org/pdf/2008.11200), [GrabNet](https://ps.is.mpg.de/code/grabnet-generating-3d-hand-grasps-for-unseen-3d-objects)) [P]. Code and data are released for research use.
- ManipNet predicts finger poses frame by frame from the wrist's and object's paths, the object given as a coarse voxel grid plus closest-distance samples near the hand ([paper](https://www.ipab.inf.ed.ac.uk/cgvu/zhang2021.pdf)) [P]. It needs wrist and object motion as input: it does fingers, as ours does, not the arm.
- D-Grasp learns a physically simulated grasp in a simulator (RaiSim) from a static grasp reference ([code](https://github.com/christsa/dgrasp/)) [P].
- What these buy over contact closing is grasps of irregular objects (a mug, a camera). A handle is a cylinder: closing until contact is enough, and it is deterministic and ours to ship. A basket's rim or a sack's neck is where a learned grasp would first earn its place.

**IK with collision avoidance.**

- RelaxedIK (Rakita, Mutlu, Gleicher 2018) writes IK as a **weighted sum** of terms: reach the goal, avoid self-collision, stay off joint limits and singularities, move smoothly from the last frame. Terms relax when they conflict, by weight ([RSS 2018](https://www.roboticsproceedings.org/rss14/p43.html), [code](https://github.com/GITAI/relaxed_ik)) [P]. This is the shape of a run-time carry constraint: the authored pose is the goal, ground and leg clearance are terms, smoothness is a term.
- Robotics resolves a redundant arm (the elbow's swivel is free once the hand is placed) with a secondary cost in the null space, usually distance from the joints' mid-range ("joint centring") [M]. Priority levels rather than weights: Baerlocher and Boulic 2004 [M; a title in a search result only].
- FABRIK (Aristidou and Lasenby 2011), Unreal's Full Body IK and Control Rig, Jolt's constraints: solvers that reach goals; none avoids collision by itself, and a game adds it as goals moved by traces [M]. **Not covered**: no source read on any of these today.

**Optimising a motion or pose.**

- Spacetime constraints (Witkin and Kass 1988): state what the motion must do and an objective for how, solve for the whole motion ([CMU](https://www.ri.cmu.edu/publications/spacetime-constraints)) [P]. Gleicher's retargeting (1998) is the form closest to ours: constraints the result must meet (a hand here, feet on the floor), and an objective that is **the least change from the original motion**, keeping its frequency content ([preprint](https://research.cs.wisc.edu/graphics/Courses/cs-838-1999/Papers/retarget-preprint.pdf)) [P]. The original is the prior.
- Retargeting to body shapes with penetration terms: Villegas et al. 2021 preserve self-contacts and prevent interpenetration on the skinned mesh by optimising in a network's latent space ([paper](https://arxiv.org/pdf/2109.07431)); MeshRet 2024 models dense body-part interaction directly ([paper](https://arxiv.org/abs/2410.20986v2)); ReConForM 2025 does it in real time from key points on the mesh ([paper](https://www.arxiv.org/pdf/2502.21207)) [P]. All three keep the source motion as the thing to stay near.
- Derivative-free tuning: CMA-ES is the usual tool for controller parameters; "Optimizing walking controllers" tunes one controller across body shapes with an objective that includes **naturalness terms** (power, head motion) beside the task ([SIGGRAPH archive](https://history.siggraph.org/?p=196629)) [P]. Neural MoCon uses CMA-ES per pose and **narrows the sampling bounds round a reference pose** "to obtain more natural motion" ([paper](https://arxiv.org/pdf/2203.14065)) [P]. Our Latin hypercube then compass search is a sound choice at 8 parameters and 2 to 7 s a point; CMA-ES earns its place past about 20 coupled parameters [M].

**Pose priors.**

- Style-based IK (Grochow et al. 2004): a model learned from poses; IK returns "the most likely pose satisfying those constraints", and other training data gives another style ([project](https://grail.cs.washington.edu/projects/styleik/)) [P].
- VPoser: a variational autoencoder over body poses from a large capture set, used to penalise impossible poses in fitting ([code](https://github.com/nghorbani/human_body_prior)) [P]. A 2026 paper notes the cost: a strong prior stops the solve reaching its target ([arXiv 2607.15463](https://arxiv.org/pdf/2607.15463)) [P]. SMPL's licence is research-only [M].
- Comfort and ergonomics: RULA (McAtamney and Corlett 1993) scores a posture by bands of joint angle away from neutral, shoulder, elbow and wrist each [M]. Minimum jerk (Flash and Hogan 1985) is the standard smoothness measure for a reach [M]. **Not covered**: neither was found by today's searches.
- Motion matching and learned priors for holding: nothing found; it picks among captured clips and so needs capture of each kind held [M].

## 3. How animation quality is checked automatically

- **Engines validate data, not motion**: Unreal's Data Validation and Unity's importer hashes (sourced in [attachment.md](../docs/attachment.md#how-others-do-it)). Unreal's automation has a screenshot comparison tool for rendering regressions [M].
- **Research measures artefacts**: foot skating (the share of frames a planted foot slides, and by how much), floor penetration depth, interpenetration share, and distribution scores such as FID against captured motion ([an example with skate ratio and sliding distance](https://arxiv.org/pdf/2609.05415), [artefacts persist in commercial data](https://www.arxiv.org/pdf/2601.19036)) [P].
- **Thresholds are perceptual, not zero**: studies of foot sliding find an amount below which viewers do not see it, and that camera motion raises it ([Perceptual evaluation of footskate cleanup](https://diglib.eg.org/handle/10.2312/SCA.SCA11.287-294)) [P].
- **A trap named by one tool**: a floor inferred from a clip's own lowest point lets penetration hide itself ([animationsight](https://pypi.org/project/animationsight/)) [P/C]. Ours measures against the ground under the feet, which avoids it.
- **What they all miss**: whether a pose reads as a person doing the thing. Penetration, limits and sliding say a pose is possible, not that it is likely. The only automatic measures of "likely" are a prior's likelihood and distance from an authored or captured reference.
- **Not found**: any talk on a studio's automated checks of animation or holds. Golden-pose tests are common practice [M] with no source.

## 4. Ours, judged

**Matches practice.**

- Grip at the origin on a hand socket, kinds with size ranges, a move set per class (Unreal's sockets, Lyra, FromSoftware). Baking the offset into the look rather than keeping an `AttachTransform` is a departure attachment.md already names; it is what lets the file be checked alone.
- A carry pose overlaid on one arm, held in the body's frame (ALS's mesh-space curves, Lyra's mesh-space blend).
- A finger-only layer on top of any clip; a second hand by two-bone IK on a marked grip; sockets placed per body with IK taking up the arm's length (Unreal's unretargeted hand IK bones, Ubisoft's IK Rig).
- A reference item and a narrow envelope per class.

**Unusual, and good.**

- Fingers solved by contact from the handle's mesh, where games author a hand pose per grip type. It scales to parts nobody has seen; its limit is anything that is not a handle.
- `holds.ts`: holds judged in numbers, on six bodies, deterministic, each bar proven by a planted fault. Nothing found does this. It is the piece that replaces the animator's first look.
- Bars from human data (NIOSH handle size, AAOS wrist range, Napier's grips).

**Wrong, or likely to fail.**

1. **The objective's order.** `over bars`, then `thinnest margin`, then `distance from the start`. Once nothing is over a bar, any move that widens the thinnest margin wins, however far from the authored pose; distance only breaks ties. That is the reverse of every method in section 2, where the reference is held and clearance is a constraint. The basket shows it: forearm 26 degrees round and 15 up, margins of 0.5 cm, unseen.
2. **Distance is in the wrong space.** A share of each parameter's range is not how far the arm moved: 10 degrees at the shoulder moves the item several times further than 10 at the wrist's roll. And the first term sums centimetres and degrees as they are.
3. **Only the wrist has limits.** A search may put the shoulder or elbow anywhere within the spec's bounds; nothing bars a shoulder turned in past a person's range or an elbow locked straight.
4. **The give is four global constants.** One elbow, back and out angle for every hanging item on every body. A long item needs more than a short one, so each new look re-tunes numbers every other look depends on (the basket's answer changed two of them, held against eleven looks). The handoff's own numbers show the squeeze: 25 degrees back leaves a pickaxe 2.6 cm over the ground, 30 leaves 1.6.
5. **Tuning a pose per look does not scale.** At hundreds of variants a held-passing list grows with the catalogue and a run with it (320 points, 18 minutes, for one look). Practice is a pose per kind and an envelope.
6. **The search sees a sample.** Five clips for the basket, two or three per held look, emulated; single clips at 15 poses a second, never a blend between two clips or a slope. An answer can pass the spec and fail the full report, or pass the report and clip in a transition.
7. **Two implementations of each layer** (`grip.rs` and `grip.ts`, `carry_arm.rs` and `carry_arm.ts`). The checks judge the TypeScript twin, the player sees the Rust one. Every "change both" is a place they can part without a check failing.
8. **Zero-tolerance bars where the eye has a threshold.** A bar of 0 cm at the ground is stricter than shipped games and than what viewers see. Not wrong, but it is what forces the strange poses; a bar at a perceptual threshold would need its own planted-fault proof and a developer's eye.

**The risk named: a clear pose no person would hold.** Others prevent it in four ways, cheapest first.

| Term | Who | Cost for us |
|---|---|---|
| Least change from an authored reference | Gleicher 1998; the retargeting papers | Nothing new: the authored carry pose is the reference |
| Narrow bounds round the reference | Neural MoCon | A spec convention: bounds of about 15 degrees each side |
| Joint limits and comfort (distance from mid-range, RULA's bands) | Robotics' null-space cost; ergonomics | A table of shoulder and elbow ranges beside `wrist.ts` |
| A learned pose prior | Style-based IK, VPoser | A data set of people carrying things, and a licence; not now |

The smallest term for `solve.ts`, in two edits to the objective and none to the search:

- Replace `thinnest margin` by `max(0, target - margin)`, target 1 cm or 1 degree: a margin is worth having up to the target and nothing beyond it.
- Replace `distance from the start` by the mean angle, in degrees, between each arm bone's direction in the candidate pose and in the authored pose (the hand's roll included), measured on the laid pose that `tune.ts` already builds.

The order stays lexicographic: over bars, short of the target margin, turn from the authored pose. The search then returns the pose nearest the authored one that clears by the target, which is Gleicher's formulation. A report line "turned N degrees from the authored pose" gives a reviewer the number to doubt; anything over about 15 degrees gets a still.

## 5. Which part carries what

| Part | Carries | Why |
|---|---|---|
| **Authored pose per kind** (a dozen or two: tool, basket, torch, spear, bow, sack) | How it looks: that this is how a person carries a basket | No measure we have says "natural"; a person's eye does it once per kind, not per item |
| **Run-time constraint** | Clearance of ground, leg and trunk, for this item's size, this body, this frame, blends and slopes included | It is the only part that sees the actual item (its lowest point is known from its parts) and the actual pose. Constants tuned offline see neither |
| **Offline search** | The constraint's few numbers per kind (how far it may lift, how fast it eases, its margins), tuned over a grid of the kind's envelope on six bodies; and the nightly regression | It is the only part that can try thousands of cases; it should tune what is shared, never one item |
| **`holds.ts`** | The verdict on all of it, and the refusal of an item outside its kind's envelope at assembly | Already built |

**Order to build.**

1. The objective's two edits above, and shoulder and elbow limits as bars. Hours; makes every later search safe to apply.
2. The run-time constraint in place of the give: the authored pose is the rest; when the item's lowest point (from its bounds) would pass the ground or the leg's capsule, lift the hand the least distance that clears it, bending the elbow first and keeping the elbow's swivel near the authored one; ease it with a critically damped spring so it never pops. RelaxedIK's terms, solved in closed form on a two-bone arm. Its twin in TypeScript is what `holds.ts` then judges.
3. Sliders for the authored pose and the constraint's numbers, with `holds.ts`'s measures live beside them: the fastest way for a developer to author a kind's pose, and the one place a person's eye enters.
4. Specs per kind over the envelope grid (five lengths by three head sizes, as assembled-items.md has for clips), replacing specs per look.
5. One implementation of grip and carry arm for game and checks (the Rust compiled for the tools, or a test that holds the twins to the same numbers on a planted pose).

## 6. Best in class, for us

A kind (tool, basket, torch) has one carry pose a person authored with sliders and looked at once. Every item of the kind is assembled from parts inside the kind's envelope, its grip at its origin, and refused at assembly if it is outside. In play the fingers close on whatever handle the mesh has; the arm rests in the authored pose and a run-time constraint moves it the least it must to keep this item, on this body, in this frame, off the ground and out of the leg, eased so it never pops. Offline, a seeded search tunes the constraint's few numbers per kind over a grid of item sizes and bodies, always toward the authored pose, and `holds.ts` gates every kind nightly with bars proven by planted faults. A person looks at a kind once, never at an item. That is beyond what the named games are known to do, and every piece is either built or planned.

**The three cheapest changes that move us most.**

1. Reorder the solver's objective: capped margin, then turn from the authored pose in degrees (section 4). Removes the named risk for every future run.
2. Add shoulder and elbow range bars to `holds.ts` beside the wrist's, each with its planted fault. Stops the search and the run-time constraint from buying clearance with an impossible arm.
3. Build the run-time constraint driven by the item's own lowest point, and delete the give constants. Removes the coupling between looks, which is what makes hundreds of items tractable.

## Recommendations

| # | Recommendation | Standing |
|---|---|---|
| 1 | Reorder `solve.ts`'s objective as in section 4, and report the turn from the authored pose | Within current decisions (D71, D632); an agent's tool |
| 2 | Shoulder and elbow limits as bars in `holds.ts`, proven by planted faults | Within current decisions (D632) |
| 3 | Do not apply the basket's answer until it is rerun under 1 and a still of the pose is on the review desk | Within current decisions (D422) |
| 4 | Replace the give constants with a run-time clearance constraint resting on the authored pose | **Changes D560** (the carried arm "gives as the legs fold" by constants) and **contradicts assembled-items.md**'s "At run time only the grip, wrist and IK already in the game; no collision avoidance" (D565's doc). Technical, the agent's to decide; record it and edit both |
| 5 | A carry pose per kind, tuned over the kind's envelope grid; no spec per look | Extends D459's envelope and D182 to carry poses. Needs a line in holding.md; the list of carry kinds is design, the developers' |
| 6 | Sliders with the measures live, as the authoring tool for a kind's pose | Planned; within D71 |
| 7 | One implementation behind game and checks, or a test that holds the twins together | Technical; needs a decision on which (D121 allows Rust for what the game shares) |
| 8 | Consider perceptual bars (a centimetre or two of ground contact for a frame) once the constraint exists | **Needs the developers**: it is a call on what looks acceptable |
| 9 | No learned grasp or pose model for v0; revisit for kinds with no handle (a sack, a crate held in two arms) | Within current decisions (D196: their code and data are research-licensed) |

**Not covered, for a later note**: talks from Destiny, Monster Hunter, Elden Ring, Valheim, Enshrouded or Crimson Desert on holding (none found); Unreal's Full Body IK and Control Rig docs, FABRIK, Jolt-driven solves, RULA, minimum jerk and ProtoRes (from memory only); any measured comparison of a pose prior on our skeleton.
