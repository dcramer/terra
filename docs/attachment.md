# Attachment: how a held or worn look goes on a body

Status: technical, recorded as D546 (by the agent, 2026-10-27). It extends the model contract (D303, [art-style.md](art-style.md#the-model-contract)) to what a character holds and wears, and is how every item fits every body (D128, [characters.md](characters.md#the-contracts)). Where a hand's item comes from and how the arms move with it: [holding.md](holding.md).

Every look the game puts on a character is made once, to its kind's rules below, so the game puts it on any body from its file alone: no number in code or in the catalogue per item or per character. The numbers a look is made from (its source, turn, size, grip) are data beside its art, read only by the tool that makes it: `art/held/<item>.json` (`fit_held.py`), `art/gear/<item>/fit.json` and `art/gear/harnesses.json` (`fit_gear.py`).

## Axes

glTF's, as every model's (D303): metres, +Y up, the front facing +Z, so +X is the model's left; right-handed ([glTF 2.0, "Coordinate System and Units"](https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html#coordinate-system-and-units)). Bevy loads glTF on the same axes (its loader's `GltfConvertCoordinates` is left at the default, which rotates nothing), so a model's front, +Z, is a `Transform`'s `back()`: Bevy's `forward()` is -Z. Blender's are Z up and front -Y; the exporter turns them.

## Sockets

A socket is an empty on the body's skeleton named `socket.<name>`, as Unreal's skeletal mesh sockets and Godot's `BoneAttachment3D` are: a bone, and a fixed offset and turn from it. Every body has the same sockets with the same axes; `export_character.py` places each from that body's own bones and skin, so nothing is set per character. Each is at scale 1, so a look is drawn at its own size; a character's height scales the whole, sockets with it.

Axes at rest, in the T-pose, in the body's glTF space (+X the body's left, +Y up, +Z its front), measured on `assets/models/character/base.glb`:

| Socket | Bone | Where (base, m) | +X | +Y | +Z |
|---|---|---|---|---|---|
| `socket.hand.R` | `hand_r` | the middle of a closed fist, 1.5 cm under the palm: (-0.733, 1.392, 0.034) | up, out of the back of the hand | forward, along a handle out of the thumb's side of the fist | toward the wrist |
| `socket.hand.L` | `hand_l` | its mirror: (0.74, 1.39, 0.04) | down, out of the palm | forward, as the right's | toward the wrist |
| `socket.head` | `Head` | the top of the head: (0.002, 1.800, 0.024) | the body's left | up | forward |
| `socket.back` | `spine_03` | the middle of the upper back, on the skin: (0, 1.262, -0.169) | the body's left | up | forward |
| `socket.belt.R` | `pelvis` | the right side of the waist, 5 cm over the hips' joint: (-0.151, 1.07, 0.016) | the body's left | up | forward |
| `socket.belt.L` | `pelvis` | its mirror | the body's left | up | forward |

- A hand socket moves with its hand. In an arm hanging at the side, the right's +Z runs up the arm, +Y forward and +X out from the body.
- The left hand mirrors the right across the body's middle, with its +X turned to keep the frame right-handed: so a look's X is mirrored between the hands and its Y and Z are not. A look whose working end lies in the Y-Z plane looks the same in either hand.
- The others face as the body does, whatever their bone's own frame.

## Kinds

Each kind of thing is solved once, so a new item of a kind needs only its model and its data. An item's data names its kind, `"kind"` in `art/held/<item>.json` or `art/gear/<item>/fit.json`; a skinned look with no data is a garment. Nothing is inferred from a look's shape or its maker's fit: the check refuses a look whose data names no kind, and one whose data fits it otherwise than its kind goes on. Each kind's numbers are data, `tools/characters/attach_kinds.json`, which the check reads; the table is its summary.

| Kind | Examples | Goes on (mount) | Life size | Where the grip falls, from the -Y end (butt or pommel) |
|---|---|---|---|---|
| `hafted` | an axe, a pickaxe, a hammer, a torch | in hand | 0.25 to 1.0 m long | 4 to 12 cm: half a fist (a fist is about 9 cm across the knuckles) above the butt, more for a knob there |
| `blade` | a knife, a sword | in hand | 0.15 to 1.25 m long | 3 to 15 cm, and 8 to 30% of its length: the middle of the grip between pommel and guard |
| `polearm` | a spear, a staff | in hand | 1.5 to 3.0 m long | 30 to 55% of its length: the lead hand, near the balance |
| `bow` | a bow | in hand | 1.0 to 1.9 m long | 45 to 55% of its length: the riser's middle |
| `shield` | a round or heater shield | strapped | 0.45 to 1.0 m at its longest | its hand strap or centre grip, given by points |
| `hung` | a basket, a bucket | hung | 0.2 to 0.6 m at its longest | under its handle's top |
| `belt` | a quiver; later a pouch, a sheath | rigid | 0.15 to 0.8 m at its longest | none: it hangs from its `hangs_at` |
| `pack` | a small or large pack | skinned | made on the base | none |
| `garment` | clothing, armour, a cap, a coif | skinned | made on the base | none |

- **The grip across the handle**: the middle of the handle where the plane y = 0 cuts it, within 1.5 cm of the origin; never the middle of the model's bounds, which a head's overhang moves off the handle.
- **Life size** is the thing's real size, a real one's range in the kind: a hatchet to a pickaxe, a knife to a longsword, a short bow to a longbow. A look outside its kind's range was left at its source's scale or is of another kind.
- `items.ron`'s `hold.class` (holding.md) is how an item fights, a finer split for move sets (`sword`, `dagger`, `axe`, `mace`...); its kind here is how its look is made and put on.

## Mounts

How each kind goes on: its origin, axes and facing.

| Mount | Examples | Origin | Axes and facing | Its size from | Its extras | Made by |
|---|---|---|---|---|---|---|
| **In hand** | tools, weapons, a torch | the grip: the middle of the handle where the fist closes on it | +Y along the handle toward the working end; its edges or striking face in the Y-Z plane, a one-sided head (an axe's bit, a hammer's face) toward -Z; +X out of the back of the right hand, so a blade's flats face ±X | its real length (`length` or the source model's own size) | `carry_tilt` | `fit_held.py`, `"fit": "length"` or `"points"` |
| **Hung from the hand** | a basket, a bucket | the grip, 1.5 cm under its handle's top | it hangs down -Z from there, its long way along +Y (forward), `out` metres toward +X (out from the leg) | its source model's | `carry_tilt`, `"hold": "hung"` | `fit_held.py`, `"fit": "hung"` |
| **Strapped** | a shield (D504) | the grip its fist closes on | its board in the X-Y plane, thin along Z, its front toward -Z, its grip along +Y | its source model's | `carry_tilt`, `strap` | `fit_held.py`, `"points"` with a `"strap"` |
| **Rigid on a socket** | a quiver; later a pouch, a sheath | where it hangs from (a quiver's mouth, its `hangs_at`), saved in its socket's space: the node sits there, the mesh about it | its socket's | its height, a share of the base's (`size`; the base is 1.80 m); it hangs from `hangs_at`, a share of its height up from its foot | `socket`, `morphs`, `swing`, `covers` | `fit_gear.py`, `"fit": "belt"`, then `carry_gear.py` |
| **Skinned** | clothing, armour, caps and coifs, packs | the body's: feet on the origin, in the base's T-pose | the body's | the base body: made on it | `covers`, `hung`, `icon_turn`, `icon_leaves_out` | `shell_garment.py`, `hung_garment.py`, `fit_gear.py` (`"fit": "back"`), then `carry_gear.py` |

An axe in hand, in its own space, seen from -X (the right hand's palm side; +X, out of the back of the hand, points away from you); a sword is the same with its flats toward you:

```
                 +Y  working end
                  ^
     bit   <==========|=====]  poll        head: its widest way along Z,
                      |                    the one-sided bit toward -Z
                      |  handle
   -Z <---------------o---------------> +Z  toward the wrist
                      |  grip = origin: the middle of the handle at y = 0
                      |  4 to 12 cm up from the butt (hafted)
                      +  butt, the -Y end
```

- **In hand.** The edge's side follows the strikes: through the fight's clips (`Keyed_*`, `Open_*`) and `Hammer_Loop`, a point 0.8 m up `socket.hand.R`'s +Y moves mostly toward -Z, away from the wrist (-0.77 to -0.90 of its motion at speed, measured 2026-10-27), so a one-sided head leads there; a sword's two edges serve both ways. The grip is the handle's middle, not the middle of the model's bounds: the two differ by the head's overhang.
- **Its source model**, by its data's `"fit"` (`fit_held.py`'s head): `length`, standing on its butt along its height (glTF +Y), scaled to `length` and gripped `grip_from_end` up from its foot; `points`, any pose, given `grip` (the fist's middle), `toward` (a point up the handle toward the working end, which becomes +Y) and `out` (the direction that becomes +X), in metres in the source's glTF axes; `hung`, upright, its handle's top its highest point. A worn piece's (`fit_gear.py`'s head): `turn` degrees about up that bring its front to the body's front.
- **Strapped.** Where the board sits is the pose's, not the strap's (D554): each shield pose names the board's place in the chest's frame (`art/animation/shield_targets.json`, `tools/characters/shield_place.ts`), and the arm is solved at bake time to put the strapped board there, so a change of strap or bones re-solves the arm and the board stays. The strap is to be physical and per body (the hand strap at that body's fist, the board along its true forearm, the per-body forearm socket); until then `strap` is one place on `lowerarm_l` for every body.
- **Rigid on a socket.** Its node's origin is what it swings about (swing.rs): `swing.tail` is where it rests from there. Per morph, its socket's shift does not follow the skin under it, so the look records its own shift beyond the socket's at each morph's full weight (`morphs`), which the game mixes by the body's morph weights.
- **Skinned.** Its joints are the base skeleton's, by name, and its morph targets are the base's (`female`, `heavy`, `strong`, `thin`), so the game binds it to any body's joints and drives its morphs with the body's (character.rs, D128). Parts that must not bend ride one bone whole: a pack's bag is weighted wholly to `spine_03`, its straps made from the skin (`bands.py`, D189); a cap and a coif are made on the head as clothing is.

## Second grip

A thing held in two hands marks where the other hand goes: Unreal's UE5 skeleton carries it as a bone, `ik_hand_l`, "the location of the left hand IK position while holding a two-handed prop", under `ik_hand_gun` at the right hand, keyed in each clip ([IK Retarget weapon goals](https://dev.epicgames.com/documentation/en-us/unreal-engine/python-api/class/IKRetargetWeaponGoalsOpSettings)); we keep it per item instead, since one class's clips serve items of different lengths. Holding.md section 3 says how the hand is pulled there and checked; this says where, in the look's own space:

| Kind | Key, in its data and its extras | Value | The other hand there |
|---|---|---|---|
| `polearm`, and any item whose `hold` grip is `Two` (a greatsword, a great hammer) | `second` | metres along +Y from the grip, negative behind it; on the handle, at least 4 cm from its -Y end and short of its working end | its hand socket takes the look's own axes at (0, `second`, 0): +Y toward the working end, +Z toward the wrist, so the off (left) socket's +X, out of its palm, is the look's +X and the palms face opposite ways, as hands on a bat or a two-handed sword do |
| `bow` | `nock` | [x, y, z], m: the string's middle at rest | the drawing hand takes it; while drawn the string's middle follows that hand (holding.md). The bow rides its lead hand, the off hand (`lead: Off`), so its own origin is the off hand's socket |

The check requires the key of a kind that has one, and the extras to say what the data does. Today neither the spear nor the bow gives it (listed faults); the round that builds the two-handed IK sets them with the clips that use them. The `Two` items of other kinds are not checked until `items.ron`'s `hold` is read by the check.

## Under animation

The facing above is static: no check yet confirms that a kind's working face (an axe's bit, a blade's edge) leads its strike in the clips its items use. The -Z side was measured once, by a scratch script; `Sword_Attack` and `Fight_Chop`, which today's game and tools play, lead flat-first (-X, 0.79 and 0.91 of the tip's motion), so a sword's flat strikes. A check needs: the clips each class strikes in (the move sets, `assets/data/moves/<class>.<stance>.ron`, with their active frames; today every blow plays `Sword_Attack`, `terra_walker::figure`); the hand socket posed through those frames on each body (`glb.ts`); a point at the look's working end; and the rule that, at speed, the point's motion leads along its one-sided side (-Z, at least 0.7 of it) or, for a two-edged blade, lies in its edge plane (no more than 0.5 of it along ±X). It is a `moves.ts` row, owned by the controls audit; the flat-first clips are the combat round's.

## Stowed (D551)

Decided (D551, the developers): what the two hand slots hold shows on the body when put away, one thing a socket; whatever else is carried stays in the pack, unseen.

| Kind | Put away on |
|---|---|
| `blade` (one-handed) | the hip away from the main hand: `socket.belt.L` on a right-handed body |
| `hafted` (one-handed) | the hip on the main hand's side: `socket.belt.R` |
| `shield` | `socket.back`, first |
| `bow`, `polearm`, any held `Two` | across `socket.back`, beside a shield there |
| `belt` (the quiver) | where it hangs today |

It goes on as data, never code: a kind's `stowed` in `attach_kinds.json`, `{"socket", "turn", "at"}`, the look's grip placed on that socket with a turn (degrees about the socket's X, Y, Z) and an offset (m, in the socket's space), mirrored to the other side for a left-handed body, an item overriding it in its own data; as the Creation Engine's `Prn` names the node a weapon is put away on. The turns and offsets are not set yet: the next step, with a still of each kind put away on the base, `heavy` and `female`. What the inspirations do: Kenshi shows both weapon slots, small weapons at the hip and large on the back (at most two on the back, one at the hip; [Steam](https://steamcommunity.com/app/233860/discussions/0/1728711392727520401/), [Steam](https://steamcommunity.com/app/233860/discussions/3/5234891274081830469/)); RDR2 at most four arms on the body, one a slot, the rest on the horse ([Steam](https://steamcommunity.com/app/1174180/discussions/0/3320862598764546941)); Enshrouded nothing put away but the shield, left on the arm, which players call an eyesore ([Steam](https://steamcommunity.com/app/1203620/discussions/0/568164715699705921/)). Not found for Soulmask, Medieval Dynasty or Dune: Awakening.

## Fitting a new item

How a new model's turn is found, by kind (`fits` in `attach_kinds.json`):

- **By a detector** (`hafted`, `blade`, `polearm`, `hung`): its long axis is the vertices' principal axis; its working end the end whose cross-sections are wider; its handle the narrowest run of sections; a head's widest direction across the axis goes to Z, its further-reaching side to -Z, a blade's thinnest to ±X; its grip by its kind's rule from the butt, centred on the handle's section there, not the bounds. A person checks the workbench sheet.
- **By a person, once** (`bow`, its draw plane and string side being its clips'; `shield`, front and hand strap; `belt`, its mouth; `pack`, Tripo's front): the turn or points in its data.
- **Fallback**: a turn in the data always wins (`"turn"`, degrees about +Y after the length fit, or the `"points"` fit). The detector refuses rather than guesses when a measure is not decisive (the long axis under 3 times the next; the head's section under 1.5 times the handle's; a head neither one-sided by 2 cm nor even within 1 cm) and names which, and a person sets the turn once, looking at the sheet, kept in the data so the item is never fitted by hand again.
- **Its icon in the same round**: a held look's turn, grip or size changed (its data, then `fit_held.py`) is its icon painted again in the same commit ([item-icons](../.agents/skills/item-icons/SKILL.md)): the icon rig lays a held look by its bounds, so a quarter turn shows another side, and the icons test fails on the changed look until it is.

**The fitter** (D552; the controls audit's to build, specified here): one command, `node tools/characters/fit.ts <item>...`, from an item's data to a checked look, a line a stage (D388):

1. **Read** its data and kind; refuse a data file with no `kind`, or a source not there.
2. **Detect**, for a detector kind with no `turn` or `points`: read the source with `glb.ts` (no Blender), measure as above, and write the turn and grip into its data, or refuse naming the measure that was not decisive.
3. **Make** the look with its kind's maker (`fit_held.py`; `fit_gear.py` then `carry_gear.py` for one worn item on every morph, not the whole `build_bodies.sh`), writing `source_sha` into its extras.
4. **Check** it with `attach_contract.ts`; a fault stops here.
5. **Show** one workbench sheet (a held look in hand from the front and side, carried and mid-swing; a worn one on the base, `heavy` and `female`), skipped when the look's bytes are those of the last sheet; its path printed for a person's look.
6. **Icon**: paint it again (`paint.ts <item>`) when the look's bytes changed, skipped otherwise.

`--turn DEG` writes a person's turn into the data and runs again from 3. Every number it finds goes into the data, so the data stays the one source and a refit needs no person.

## Who owns what

- **This spec** (the attachment spec rounds): this doc, `attach_kinds.json`, `attach_contract.ts` and its faults list, the shape of each item's data.
- **The controls audit** (the "Gameplay character controls audit" session): the per-body forearm socket for strapped items; sockets checked under animation, the working face leading its strike among them; the one-command fitter for a new item; the fist closing on the grip; the two-handed IK, with the spear's `second` and the bow's `nock`.
- **The combat round**: the clips that strike flat-first.

## Extras

Read from the glTF `extras` of the look's mesh node (Bevy's `GltfExtras`); Blender writes an object's custom properties there.

| Key | On | Value | Written by | Read by |
|---|---|---|---|---|
| `carry_tilt` | every held look | degrees it tips when carried, not used: negative tips its head forward and down in a hanging hand, positive raises it | `fit_held.py` | `terra_walker::character` (`held_ready`) |
| `hold` | a hung look | `"hung"` | `fit_held.py` | `attach_contract.ts`; tools that must tell it from a look in hand |
| `strap` | a strapped look | `{"bone", "at", "face", "grip"}`: the bone it rides; where its origin sits in that bone's frame (m); the directions in that frame its front (-Z) and grip (+Y) turn to | `fit_held.py` from its data | `terra_walker::strap`, `tools/characters/moves.ts`, the icon rig |
| `second` | a look held in two hands along its handle | metres along +Y where the other hand closes ("Second grip") | `fit_held.py` from its data | `attach_contract.ts`; the two-handed IK (not built) |
| `nock` | a bow | [x, y, z], its string's middle at rest | `fit_held.py` from its data | `attach_contract.ts`; the bow's draw (not built) |
| `socket` | a rigid look | the socket's name | `fit_gear.py` | `terra_walker::character` (dress) |
| `morphs` | a rigid look | `{morph: [x, y, z]}`, its shift beyond its socket's at the morph's full weight, m, in the socket's space | `carry_gear.py` | `terra_walker::character` |
| `swing` | a loose rigid look | `{"stiffness", "drag", "gravity", "tail": [x, y, z]}` | `fit_gear.py` | `terra_walker::swing` |
| `covers` | a worn look | the patches of skin it hides, ascending, each under 1,024 | `fit_gear.py`, `shell_garment.py`, `carry_gear.py` | `terra_walker::character`, `body_mask.wesl` |
| `hung` | a loose skinned garment (a skirt, an apron) | `1` | `hung_garment.py` | `carry_gear.py` |
| `icon_turn` | a worn look | degrees about +Y its icon is seen from (a pack's 180, from behind) | `fit_gear.py` | `terra_survival::icons` |
| `source_sha` | a look made from a source model | the first 12 hex digits of the SHA-256 of its source's bytes (`"source"` under `assets/models` for a held look; `art/gear/<model>/model.glb` for a worn one), so a source changed and its look not made again is found | the fitter | `attach_contract.ts` (when given) |
| `icon_leaves_out` | a worn look | material roles its icon does not draw (a pack's `Harness`) | `fit_gear.py` | `terra_survival::icons` |

Materials are named by role (Wood, Metal, Leather, Trim), so a tint recolours one look many ways (characters.md, Colour).

## The check

`node tools/characters/attach_contract.ts` reads every committed look in `assets/models/held` and `gear`, their data and `base.glb`, and holds each to its kind's rules above (its kind named, its mount's origin, axes, facing and extras, its life size, its grip across and along the handle), and the body's sockets to the table; its test runs in `cargo xtask check`. It also catches what a new item gets wrong on its way in:

- **Inside out**: over a quarter of its surface wound against its own normals (vertices mirrored by a script without flipping the faces).
- **Off the body** (skinned): over 2% of its vertices more than 0.6 m from their most-weighted joint at rest (1.2 m for a `hung` garment's free hem), so made elsewhere or at another scale; or over 10% of those more than 5 cm to one side weighted to a joint of the other side (mirrored); a pack's middle under 5 cm behind the body's (its front kept as Tripo's).
- **Stale**: its look not made from its data as it stands: a `length` fit's length and grip within 5 mm, a `hung` fit's `out`, a `belt` fit's height within 3% and `hangs_at` within 0.02, the `swing` numbers; and its data's `source` there.
- **Its hold**: an item with a `hold` in `items.ron` fights as one of its kind's `classes` (`attach_kinds.json`), its `hold.length` within 3% of its look's, and one held `Two` gives a `second`.

- **Its source changed**: a look whose extras give `source_sha` made from a source whose bytes have since changed. Required once the fitter writes it for every look.

What it cannot catch yet: a source changed under a look with no `source_sha`, and the kind of an item with no `hold` (a tool), which only its size and grip bound. Given files, it judges each by the folder it sits in. A fault known today is listed with why in `tools/characters/attach_faults.json`; a fixed one fails the test until its line is removed, so the list only shrinks.

Known today (2026-10-27): the eight looks fitted by length (both axes, both pickaxes, the hammer, the knife, the spear, the bow) lie across X, 90 degrees off, as `fit_held.py` never turns them about +Y; six of them sit 2.5 to 4.7 cm off their grip, as the length fit centres the model's bounds; and the basket predates its `hold` marker.

## How others do it

Researched 2026-10-27 (the attachment review); a claim without a link is from memory.

- **glTF 2.0** (cited above): the axes and units; `extras`, "application-specific data" ([schema](https://raw.githubusercontent.com/KhronosGroup/glTF/main/specification/2.0/schema/extras.schema.json)). No Khronos extension defines attach points ([index](https://github.com/KhronosGroup/glTF/blob/main/extensions/README.md)), so ours are named nodes and extras.
- **Unreal sockets**: a bone name and a location, turn and scale relative to it ([skeletal mesh sockets](https://dev.epicgames.com/documentation/en-us/unreal-engine/skeletal-mesh-sockets-in-unreal-engine)). **Lyra** keeps each item's offset from the socket as data on its equipment definition, `AttachSocket` and `AttachTransform`, applied with `KeepRelativeTransform` ([LyraEquipmentDefinition.h](https://git.singinwhale.com/singinwhale/RealtimeStyleTransferRuntime/src/commit/36c097e26e1c986719379225361358396ae26261/Source/LyraGame/Equipment/LyraEquipmentDefinition.h), [LyraEquipmentInstance.cpp](https://git.singinwhale.com/singinwhale/RealtimeStyleTransferRuntime/src/commit/36c097e26e1c986719379225361358396ae26261/Source/LyraGame/Equipment/LyraEquipmentInstance.cpp), a mirror of Epic's sample).
- **Unreal modular characters**: parts follow one skeleton by Leader Pose (cheap to set up, a draw a part), Copy Pose (dearest), or a merged mesh (one draw, no morphs) ([modular characters](https://dev.epicgames.com/documentation/en-us/unreal-engine/working-with-modular-characters-in-unreal-engine)). **Mutable** hides or reshapes a body under clothes by tagged modifiers, Remove Mesh and Clip Morph ([Remove Mesh](https://dev.epicgames.com/documentation/en-us/unreal-engine/python-api/class/CustomizableObjectNodeModifierRemoveMesh), [Clip Morph](https://dev.epicgames.com/documentation/en-us/unreal-engine/python-api/class/CustomizableObjectNodeModifierClipMorph)).
- **Creation Engine**: a weapon's model names the skeleton node it is put away on in a `Prn` string (`WeaponSword`, `WeaponBack`, `WeaponBow`, `QUIVER`, `SHIELD`...), without which it "will likely show up at your feet" ([Beyond Skyrim wiki](https://wiki.beyondskyrim.org/wiki/Arcane_University:Nifskope_Weapons_Setup)); armour claims biped slots, which must match between model and data ([BODT](https://en.uesp.net/wiki/Skyrim_Mod:Mod_File_Format/BODT_Field)).
- **World of Warcraft**: a model's attachments are an id (`HandRight`, `HandLeft`, `SheathMainHand`, `Back`...), a bone and a position from it ([wowdev M2](https://wowdev.wiki/M2)).
- **Unity**: `Animator.GetBoneTransform(HumanBodyBones)` finds a humanoid's bone ([docs](https://docs.unity.com/en-us/engine/current/script-reference/unityengine/animator/getbonetransform)); Animation Rigging's Two Bone IK (target and hint) places a hand, and Multi-Parent blends an object between parents ([Two Bone IK](https://docs.unity3d.com/Packages/com.unity.animation.rigging@1.3/manual/constraints/TwoBoneIKConstraint.html), [Multi-Parent](https://docs.unity3d.com/Packages/com.unity.animation.rigging@1.3/manual/constraints/MultiParentConstraint.html)).
- **Godot** ([BoneAttachment3D](https://docs.godotengine.org/en/stable/classes/class_boneattachment3d.html)): a node that follows one bone, under which held things are placed.
- **VRM** ([VRMC_springBone](https://github.com/vrm-c/vrm-specification/tree/master/specification/VRMC_springBone-1.0)): spring bones for what swings (stiffness, gravity, drag), which our `swing` follows.
- **Checks and staleness**: Unreal's Data Validation runs a validator per asset class on save and in a commandlet ([docs](https://dev.epicgames.com/documentation/en-us/unreal-engine/data-validation-in-unreal-engine)); Unity re-imports when a hash of an asset's inputs and its importer's version changes ([importer consistency](https://docs.unity.com/en-us/engine/current/manual/assets-and-media/import-assets/import-analyze/importer-consistency)), as Unreal keys derived data by input hashes and a version.

Our terms against the industry's: a **look** is a mesh asset; a **socket** is Unreal's and Godot's (WoW's attachment); a **kind** is the item type a model declares, as Skyrim's `Prn`; the **grip** is the pivot at the grip; **`second`** is the left-hand IK target; **rigid on a socket** is an attached static mesh; **skinned** is a modular part on the leader's pose; **`covers`** is Skyrim's biped slots or Mutable's Remove Mesh.

**Where we depart**: an item's offset from its socket is baked into its look (its grip at its origin), not kept as Lyra's `AttachTransform`: one convention every reader of the file follows, checked from the file alone, at the cost of a Blender run per change. Hand sockets are placed per body from the skin, not authored per skeleton. A rigid look carries its own shift per morph, since a socket follows bones and a heavy body's skin moves past them. A pack's bag is skinned whole to one bone rather than attached, so it and its skin-made straps move as one.
