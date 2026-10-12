# Character pipeline audit: what is sound, what is a bug, what is duct tape

2026-10-12. Question: across the character pipeline (rig, skin deformation, body hiding, garments, rigid gear, morphs, the game's side, and the checks), which parts are done as the industry does them, which are wrong, and which are heuristics that happen to work? Asked by the developers after limbs were seen twisting and squeezing, garment edges looked ragged, and fixes kept being improvised.

**Method:**
- Four research surveys of the state of the art, summarised under each heading. Their claims were checked against our files where they touch them: one, that Tripo's rig carries weights on its twist bones, was false.
- Measurements on our own files:
  - `tools/characters/deform_check.py`: rings of skin along every limb, squeeze and twist through our clips.
  - The twist each clip puts on each joint.
  - `sockets.py --pose` against the running game's skeleton.

**Verdicts:**
- **sound:** how shipped games or published methods do it;
- **bug:** gives wrong results;
- **duct tape:** a heuristic or one-off number that happens to work on what we tried;
- **unverified:** claimed but not checked.

## Rig and skin deformation: the cause of the twisting and squeezing

**Superseded the same day by [skin-deformation.md](skin-deformation.md).** The measurements below were taken on a stale 2025 copy of the animation library and Tripo's old V1 rig. Its forearm figures used a ring measure since corrected. The fix is the current libraries, Tripo's V3 rig with its weighted twist bones, a twist step baked into the clips, and dual quaternion skinning.

**Measured on the base body** (`deform_check.py`, through idle, walk, jog, sword swing, punch, crouch and roll):

- **The middle of the right forearm squeezes 84% in the sword swing.** That's a ring of skin left at a sixth of its width: the candy wrapper.
- **Over 60%:**
  - the left forearm in the roll squeezes 44%;
  - the rest of the forearm squeezes 27–47%.
- **Elsewhere, 12–20%.** The shoulders, hips, knees and the ankle end of the shin lose that much in deep bends.
- **The clips do turn the forearm hard.** They roll it against the upper arm by up to 170° in the sword swing, 114° in the roll and 57° in the punch. A real forearm turns about that much.

| Part | What we do | State of the art | Verdict |
|---|---|---|---|
| Twist and share bones | None along any limb but the forearm. Tripo's rig has 22 twist and share bones (two twist bones per upper arm, forearm, thigh and calf; elbow and knee share bones; neck), but checked on our file, not one vertex is weighted to any of them, so `convert_rig.py` folding them loses nothing | Two twist bones per limb segment (Unreal's mannequin), each taking a share of the turn, with weights ramped along the segment; share bones taking half a bend | **Missing**, the main cause: the turn of every segment happens at one joint |
| Forearm twist | `twist.py` adds one bone at the elbow that follows the bend and takes none of the turn; the forearm bone takes all of it, so the skin between blends 0% with 100% | Twist taken from the hand's or forearm's turn by swing-twist extraction and shared along the segment (about 50% mid, 75–100% at the wrist); the shoulder end of the upper arm and the hip end of the thigh untwist | **Bug** |
| Weight smoothing | Two rounds of surface smoothing of every group | Smoothing along each limb's axis, the blend at a joint about the joint's radius; prune to 4 and renormalise after twist bones | Duct tape |
| Skinning | Linear blend (Bevy), four influences | Linear plus twist bones and correctives in games. Optimised centres of rotation (Le and Hodgins 2016) removes candy wrapping and collapse at little cost, and is feasible in a Bevy shader | Limitation; fine once twist is right |
| Correctives | None | Helper bones or shapes at the deltoid and armpit, elbow, groin and glute, and knee, driven by joint angle. Joint-based ones carry to gear and morphs | Missing; later, if the check still fails |
| Bone roll | Our bones keep the mannequin's directions; roll not checked against the clips' | Rest roll must match the clips', or twist splits wrongly | Unverified |
| Rig check | Rings only at the elbows, wrists and knees, 20% shrink allowed, no twist | Range-of-motion clip, twist per centimetre, volume per region, inverted triangles | Gap; `deform_check.py` begins to fill it |

## Body hiding under clothing

| Part | What we do | State of the art | Verdict |
|---|---|---|---|
| What is hidden | 1,024 k-means patches; one wholly inside a garment's cut is collapsed by the body shader | Per triangle, conservatively. Hide only what is inside the garment's closed volume in every pose of a range-of-motion sweep, and at least a margin from every opening: 2–3 cm at tight cuffs, 6–10 cm at loose hems. The garments worn are ORed together (UMA, Mutable's "all vertices inside") | Duct tape: patches straddle hems, so skin pokes through where kept and a hole opens where collapsed. Both were seen. |
| Skin kept near an edge | Drawn as is | Pushed inward under the garment by a shape that fades to nothing at the opening (Cyberpunk's garment support, Mutable's clip morph, shrink shapes); a few millimetres of clearance | Missing |
| The mechanism | A per-vertex collapse in the body's vertex shader, one material per mask | Index buffer or mask rebuilt when equipment changes | Sound |

## Garments made on the body

| Part | What we do | State of the art | Verdict |
|---|---|---|---|
| The piece from the body's faces | Copy the covered faces, push them out, keep the weights | MakeHuman's MakeClothes does the same; garments bound to the body's surface | Sound |
| Where its edges fall | Bone weights, `LIMB_SHARE`, smoothing rounds, island sizes | The hem where the dressed model's garment actually ends (rays from the skin, standing off more than 3–5 mm), bounded by the coverage rule; ideally a small library of hem loops on the base, snapped to | Duct tape |
| Edge shape | Along the body's triangles: a sawtooth (since tried, uncommitted: a cut along a smoothed field) | Isoline cut, then the boundary resampled and relaxed along itself (CLOTH3D) | Bug, half fixed |
| Thickness | None: a single sheet | A 1–2 cm turned-in rim at hems and cuffs (Solidify, rim only), its own UVs and normals | Missing |
| Colour at the edges | Baked from the dressed model, which catches its rim and skin (since tried, uncommitted: repainting a strip from inside) | Bake only where the hit on the dressed model is inside its garment: a validity mask, filled from the interior, Extend padding; the rim given a trim colour, never baked | Bug; the repaint was duct tape |
| Height off the skin | Stand-off from the dressed model, capped at 1.8 cm above the waist and 3 cm below | A slot's clearance contract | Sound |

## Rigid gear and packs

| Part | What we do | Verdict |
|---|---|---|
| Turn and size of each model | `PIECES`: a turn per model (Tripo faces its models its own way) and a share of height | Duct tape, per item; should be found from the model |
| The pack's strap loops | Cut below the socket plus 1.5 cm | Duct tape, for one model |
| Strap routes | Rays from the body's joints; a ray that misses moves 15% inward, up to 8 times | Duct tape in its search; the idea is sound |
| Straps on the biggest morphs | Failed in the sword swing on heavy and strong; passed at rest on every morph | **Fixed (2026-10-12):** the clipping fixer, per morph, through our clips (`gear_check.py --fix`, baked by `carry_gear.py`); the heavy body's small pack went from 2.0% to 1.0% in the swing |
| A look hung on a socket across morphs | Moved only with its socket; a bigger seat swallowed the quiver (11% through on strong) | **Fixed (2026-10-12):** it also moves by the standing-off rule, less its socket's move, per morph, in its extras for the game to mix (`character::Shifts`); 0% on every morph |
| `skin_weights` in `fit_gear.py` | Fades weights to one bone away from the skin; called by nothing since the straps were made from the skin | Dead code; delete |

## Morphs and the game

| Part | What we do | Verdict |
|---|---|---|
| Registration | Pose by the base's own skeleton, a pull smoothed over distance, back to the T-pose | Sound (non-rigid fitting) |
| Build morphs keep the skeleton | As Starfield's Heavy | Sound |
| Gear carried by nearest skin | The morph of the nearest skin vertex. BodySlide averages up to ten neighbours by distance; Mutable binds each vertex to nearby faces. Nearest-vertex picks are noisy where a morph changes fast, and a strap's inner and outer faces can pick different vertices | Duct tape: bind to the surface (triangle and barycentric weights), spread through each part's connections as BodySlide's clipping fixer does, so thin parts keep their thickness. The rigid parts' clearance rule matches BodySlide's "solid object" in spirit |
| Joints and sockets in the game | Offsets per morph; bind poses moved | Sound; verified at rest (exact) and live in the sword swing (1–3 mm, 9 mm at the fast hand from frame rounding) |
| Joints kept after animation | Each frame, a joint the clip set is moved by its offset; "set this frame" is guessed from whether it changed since last written | Duct tape. Sound: export clips with only rotations and the hips' translation, so the body's own joint positions are never overwritten |
| Culling of morphed bodies | Bevy's skinned bounds come from the base mesh | Unverified: a heavy body is up to 9 cm larger |
| Icons of rebuilt gear | Fingerprint of the file's bytes; `--keep` when the look did not change | Duct tape: the fingerprint should be of what the icon shows |

## The checks

| Check | Verdict |
|---|---|
| `gear_check`'s through rule | Sound since the change: skin counts only when the piece is beneath it and it is not inside the piece. It was redefined three times in one day, so treat its numbers before that as on a different measure |
| `gear_check`'s holes | Unverified. One version missed holes that were seen; the current one reads 1.5% at idle on the jerkin and needs calibrating. The state of the art is a rendered check: body in one flat colour, the garment's back faces in another, magenta behind, through a range-of-motion sweep. Magenta inside the silhouette is a hole; body colour where the garment should be is poke-through |
| Poses | Only our library's clips. The state of the art adds a range-of-motion clip sweeping each joint to its limits, combined poses included |

## Order of work

1. **Skin deformation, since it shows in every animation:**
   - twist bones on every limb segment, with weights ramped along each segment (Tripo's rig has the bones but no weights on them);
   - drive them by swing-twist extraction with a share per bone, baked into the clips;
   - smooth weights along the limbs;
   - a range-of-motion clip and more measures in `deform_check.py`;
   - check the bone roll.
2. **Body hiding:** per-triangle hiding over a pose sweep, with margins; kept skin pushed in under garments; the rendered check for holes.
3. **Garment edges:**
   - the hem where the dressed garment ends;
   - the cut relaxed;
   - a turned-in rim;
   - the bake masked to inside the dressed garment.
4. **Gear across morphs,** after the deformation work, re-measured first:
   - bind each look to the body's surface instead of the nearest vertex;
   - bake a push-out per morph extreme, at rest and through every clip, into each morph's target: BodySlide's clipping fixer, with a clearance of about 4 mm, spread through connections, seams welded. A large push needed only in pose means the weights need fixing, not more inflation;
   - check a grid of morph mixes, not just each extreme.
5. **The duct tape above,** each replaced by the sound version or kept and marked as authoring data.

Gear across morphs (the fourth survey, which read BodySlide and Outfit Studio's source):
- No shipped system pushes skinned gear out of the body at run time; that is for simulated cloth.
- Gear morphs are authored or corrected per extreme offline: Skyrim's two weight meshes, Starfield's Heavy, Thin and Strong deltas, BodySlide's clipping fixer in its sliders mode.
- Rigid parts move as one; covered skin is hidden.

Sources are in the four surveys this note summarises (the session's research agents); the main ones:
- [Unreal twist bones](https://cascadeur.com/help/rig/advanced_rigging/advanced_rigging_techniques/twist_bones)
- [Le and Hodgins 2016](https://la.disneyresearch.com/wp-content/uploads/Real-time-Skeletal-Skinning-with-Optimized-Centers-of-Rotation-Paper.pdf)
- [Mutable clip with mesh](https://github-wiki-see.page/m/anticto/Mutable-Documentation/wiki/Node-Clip-Mesh-With-Mesh-(v5.5))
- [Mutable clip morph](https://github-wiki-see.page/m/anticto/Mutable-Documentation/wiki/Node-Clip-Mesh-With-Plane-and-Morph-(v5.5))
- [UMA MeshHideAsset](https://raw.githubusercontent.com/umasteeringgroup/UMA/master/UMAProject/Assets/UMA/Core/StandardAssets/UMA/Scripts/MeshHideAsset.cs)
- [Cyberpunk garment support](https://wiki.redmodding.org/cyberpunk-2077-modding/for-mod-creators-theory/3d-modelling/garment-support-how-does-it-work)
- [Robust skin weight transfer](https://www.dgp.toronto.edu/~rinat/projects/RobustSkinWeightsTransfer)
- [CLOTH3D boundary smoothing](https://arxiv.org/pdf/1912.02792)
- [Blender Solidify](https://docs.blender.org/manual/en/latest/modeling/modifiers/generate/solidify.html)
- [BodySlide source](https://github.com/ousnius/BodySlide-and-Outfit-Studio) (`Automorph.cpp`, `ClippingFixer.cpp`)
- [Roblox cages](https://create.roblox.com/docs/art/accessories/caging-best-practices)
