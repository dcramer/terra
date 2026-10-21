# Skin deformation: why limbs twisted and squeezed, and the fix

2026-10-12. **Question:** why do our characters' forearms knot and squeeze in the game's animations, and what makes them deform correctly through every clip?

**Answer:** it was three things together.
1. **Our clips were a stale copy.** They came from a 2025 mirror of Quaternius's Universal Animation Library, whose v2.0 (2026-01-23) lists "Fixed sword swing elbow twist".
2. **The rig and the twist code were wrong.** Tripo's old rig had twist bones with no skin on them, and the twist bones we added in their place were wrong.
3. **Quaternius's clips put an arm's whole roll in the forearm.** Bevy's plain skinning wrings that roll shut wherever it falls.

The fix:
- current libraries, on Tripo's V3 rig with its weighted twist bones;
- a twist step baked into every clip;
- weight-blended skinning in the game: dual quaternions everywhere but round the hips.

Measured through the libraries' 20 hardest clips, the worst squeeze anywhere along a forearm went from 70% to 13%. Renders agree.

## What we found

**The clips.**
- Both libraries (UAL1 v3.0, 120 clips; UAL2 v2.2, 134 clips) never roll the upper arm or thigh about its own length: exactly 0 degrees in every frame of every clip, checked two independent ways.
- An arm's roll, which in a body is mostly at the shoulder, is all in the forearm:

| Where the roll sits | Worst | Median of each clip's worst |
|---|---|---|
| Forearm against upper arm | 164° (`Sword_Attack_Standing`) | 40° |
| Hand, rolling further against the forearm | 118° | — |
| Shin against thigh | 107° | 42° |

- A real forearm turns about 80 degrees each way. Our 2025 copy's `Sword_Attack` reached 170 degrees; v3.0's reaches 114.
- **Engines handle clips like this.** Unreal's post-process step drives twist bones, Unity's humanoid has arm-twist settings, and Maya's HumanIK has roll extraction. Bevy has none of these, so the raw roll went into the skin.

**The rigs.**
- Tripo's V1 rig, which we used, and its V3 `actorcore` skeleton have twist and share bones on every limb that move no vertex at all. V3's `mixamo` and `unity` skeletons have no limb twist bones.
- Only V3's `unreal` skeleton weights its twist bones: one per upper arm, forearm, thigh and calf, halfway along, 300 to 2,300 vertices each.
- Its bone names are the ones UAL v3 uses, so the clips play on it by name.

**Skinning methods.** Bevy blends each vertex's bones linearly; across a big roll that collapses the skin to a neck. Dual quaternion skinning (Kavan et al. 2007; Blender's "Preserve Volume") keeps volume. Its known faults:
- it goes the short way round past half a turn;
- it bulges at sharp bends.

## Measured

**Setup:**
- the base male on Tripo's V3 `unreal` rig, converted onto UAL v3's skeleton;
- the 20 hardest clips of both libraries (`deform_check.py`): sword swings, mining, tree chopping, kip-up, zombie spawn, crawling and so on;
- each ring's worst squeeze in any frame.

| Ring | Clips as they are | Twist step | Dual quaternions | Twist step and dual quaternions |
|---|---|---|---|---|
| Upper arm near the elbow | 70% | 30% | 14% | 13% |
| Forearm, middle to wrist | 4% | 46% | 0% | 0% |
| Spine | 23% | 23% | 9% | 9% |
| Shoulder | 15% | 15% | 7% | 7% |

Renders of the worst moments show:
- **Clips as they are:** a knot at the elbow.
- **Twist step:** the elbow is clean but the forearm narrows toward the wrist. The hand's own roll stacks on the moved roll, up to about 200 degrees, more than one twist bone spreads with linear skinning.
- **Dual quaternions alone:** a crease ring at the elbow.
- **Both:** clean.

**The rig check on the rebuilt base:** every elbow, wrist and knee ring shrinks 5% or less in its seven clips, where the worst before was 28%.

## The fix (D115)

1. **The clips.** The current libraries are kept by each developer in `art/animation/library/`. `clips.py` cuts the clips listed in `art/animation/clips.txt` into `assets/models/character/mannequin.glb`.
2. **The rig.** Tripo's V3 `unreal` rig is converted onto that skeleton (`convert_rig.py`): joints moved, our bone directions kept, Tripo's weights and twist bones kept.
3. **The twist step** (`twist.py`) is baked into every clip as it is sampled for a body. That is Unreal's convention: the mannequin keeps pronation in the hand, and twist bones take shares of it.
   - The forearm's and shin's roll moves to the hand and foot. The forearm then only bends at the elbow, and the hand ends up exactly where the clip puts it, checked frame by frame.
   - The twist bone halfway along takes half the roll between them.
   - The upper arm's and thigh's twist bones follow their bone, as the clips never roll them.
4. **Weight-blended skinning** for every skinned mesh in the game, as Maya's "weight blended" skinning and Blender's armature modifiers mixed by a group do it: each vertex partly linear blend, partly dual quaternion.
   - **The rule:** dual quaternions throughout, but where the pelvis moves the skin, which takes the linear blend by twice the pelvis's weight. It is data, `assets/models/character/skinning.json`, read by the game and the tools alike.
   - **In the game:** `assets/shaders/skinning.wesl` is put in place of Bevy's `bevy_pbr::render::skinning` by `crates/terra_walker/src/skinning.rs`, so body and gear skin alike. Each vertex's share rides in its joint weights, which then sum to one plus the share, so no material's pipeline changes. A mesh never given shares stays on the plain linear blend.
   - **In the tools:** every tool that poses a body skins it the same way (`look.skin_as_game`); `--linear` and `--dqs` compare.
   - **No flips:** after the twist step, no two neighbouring bones of a limb differ by more than 72 degrees in the game's clips, so dual quaternions' short-way fault never arises.
5. **`clip_check.py`** fails a clip that rolls the upper arm or thigh by more than 30 degrees, or leaves a step over 135 degrees after the twist step. Every game clip and all of UAL2 pass.

## Mistakes on the way, so they are not repeated

| Mistake | Effect |
|---|---|
| The clips were a stale mirror copy; nobody checked the author's page and changelog | Days spent chasing a twist the author had already fixed |
| Twist bones added by us, where Tripo's V1 rig had none weighted | One took none of the turn; a chain was placed along guessed bone tails |
| `compare.py --pose` gave every body the first file's copy of a clip, the second loading as `Name.001` | Renders of twist rigs showed bodies playing an unbaked clip, so differences looked like none. Fixed: each body plays its own and the tool prints which |
| zsh does not split a variable into words | A measurement passed every clip name as one argument and measured nothing. Caught because every reading was zero |
| A roll past half a turn was unwrapped from the first frame | One ambiguous frame turned a twist bone 125 degrees the wrong way for a whole clip. Fixed: unwrap over the clip, then pick the whole-turn shift with the smallest roll |

## Why blended, not dual quaternions throughout

The rig check on the base, through its seven clips:

| Skinning | Worst knee or elbow pinch | Seat through the floor mid-roll | Crouch edges crushed or stretched |
|---|---|---|---|
| Linear blend | 27% | 1.6 cm | 1.7% |
| Dual quaternions throughout | 5% | 8.2 cm | 2.3% |
| Blended, linear round the hips | 5% | 5.4 cm (the thigh), inside the 6 cm limit | 1.8% |

- **Why the hips are the exception.** Dual quaternions keep the knees and elbows full through deep bends as well as the forearms through rolls. Across the wide blend of seat and groin they bulge, and renders of the roll show the seat ballooning. Blended, the hips render close to the linear blend while the limbs stay full.
- **Rejected along the way:** a twist bone share of 0.75 instead of 0.5, matching where its skin lies. It made the linear blend's forearm squeeze worse, 74% against 46%.

## Gear on the bigger bodies

With the forearms fixed, the build found gear failing on the bigger morphs. Both fixes are BodySlide's ideas, done per morph:
- **A hung look follows the skin under it.** A quiver set against the base's hip was swallowed by the strong body's seat: 11% of the skin it watches came through, at rest. A look on a socket now also moves, per morph, by the rule for parts that stand off, less its socket's own move. The shift is recorded in its extras, and the game mixes it by the body's morph weights.
- **The clipping fixer, through our clips.** On the heavy body, the flank came through the small pack's straps in the sword swing: 2.0%, at the limit. `gear_check.py --fix` records how far each part of a look must move out to clear the skin in every frame, plus 4 mm; `carry_gear.py` bakes that into the morph, away from the skin. It was 1.0% after.
- **A mistake on the way.** Pushing along each vertex's own normal first made it worse, 6.1%: the bag's back panel faces the body. The push goes along the nearest skin's normal.
- **Where the fixer cannot help.** It made the coat worse on the heavy body: 1.8%, a pass, became 2.1%, then 2.8% and 4.2% over three rounds. The coat's clipping is a thigh lifting into its front skirt in the jog and crouch: clipping only in pose. A push baked into the morph moves the skirt into the belly at rest. So the fixer keeps a round only while the look gets better, and undoes one that makes it worse. Pose-only clipping wants the skirt weighted partly to the thighs.

## Still open

- **Deep bends.** The ring right at a fully bent elbow or knee still loses 20 to 35% in kip-ups and zombie spawns. That is skinning at a sharp bend, not twist. Shipped games use corrective bones or shapes, driven by the joint's angle.
- **The thigh mid-roll.** Blended skinning still lets it dip 5.4 cm through the floor, inside the rig check's 6 cm.

Sources:
- [Quaternius UAL changelog](https://quaternius.itch.io/universal-animation-library)
- [UE IK Retargeter rotation modes](https://dev.epicgames.com/documentation/en-us/unreal-engine/python-api/class/RetargetRotationMode)
- [Unity lowerArmTwist](https://docs.unity.cn/430/Documentation/ScriptReference/HumanDescription-lowerArmTwist.html)
- [HumanIK roll extraction](https://help.autodesk.com/cloudhelp/2016/ENU/Maya/files/GUID-AB7C11EA-90A3-48C8-BDEB-E087F49214A7.htm)
- [Kavan, dual quaternion skinning](https://users.cs.utah.edu/~ladislav/dq/)
- [Cascadeur twist bones](https://cascadeur.com/help/rig/advanced_rigging/advanced_rigging_techniques/twist_bones)
