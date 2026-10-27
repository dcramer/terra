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
| `hafted` | an axe, a pickaxe, a hammer, a torch | in hand | 0.25 to 1.0 m long | 4 to 12 cm: half a fist's width (about 9 cm across the knuckles), and a knob's |
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
| **Hung from the hand** | a basket, a bucket | the grip, a finger's width under its handle's top | it hangs down -Z from there, its long way along +Y (forward), `out` metres toward +X (out from the leg) | its source model's | `carry_tilt`, `"hold": "hung"` | `fit_held.py`, `"fit": "hung"` |
| **Strapped** | a shield (D504) | the grip its fist closes on | its board across Z, its front toward -Z, its grip along +Y | its source model's | `carry_tilt`, `strap` | `fit_held.py`, `"points"` with a `"strap"` |
| **Rigid on a socket** | a quiver; later a pouch, a sheath | where it hangs from (a quiver's mouth, its `hangs_at`), saved in its socket's space: the node sits there, the mesh about it | its socket's | a share of the body's height (`size`) on the base | `socket`, `morphs`, `swing`, `covers` | `fit_gear.py`, `"fit": "belt"`, then `carry_gear.py` |
| **Skinned** | clothing, armour, caps and coifs, packs | the body's: feet on the origin, in the base's T-pose | the body's | the base body: made on it | `covers`, `hung`, `icon_turn`, `icon_leaves_out` | `shell_garment.py`, `hung_garment.py`, `fit_gear.py` (`"fit": "back"`), then `carry_gear.py` |

- **In hand.** The edge's side follows the strikes: through the fight's clips (`Keyed_*`, `Open_*`) and `Hammer_Loop`, a point 0.8 m up `socket.hand.R`'s +Y moves mostly toward -Z, away from the wrist (-0.77 to -0.90 of its motion at speed, measured 2026-10-27), so a one-sided head leads there; a sword's two edges serve both ways. The grip is the handle's middle, not the middle of the model's bounds: the two differ by the head's overhang.
- **Rigid on a socket.** Its node's origin is what it swings about (swing.rs): `swing.tail` is where it rests from there. Per morph, its socket's shift does not follow the skin under it, so the look records its own shift beyond the socket's at each morph's full weight (`morphs`), which the game mixes by the body's morph weights.
- **Skinned.** Its joints are the base skeleton's, by name, and its morph targets are the base's (`female`, `heavy`, `strong`, `thin`), so the game binds it to any body's joints and drives its morphs with the body's (character.rs, D128). Parts that must not bend ride one bone whole: a pack's bag is weighted wholly to `spine_03`, its straps made from the skin (`bands.py`, D189); a cap and a coif are made on the head as clothing is.

## Extras

Read from the glTF `extras` of the look's mesh node (Bevy's `GltfExtras`); Blender writes an object's custom properties there.

| Key | On | Value | Written by | Read by |
|---|---|---|---|---|
| `carry_tilt` | every held look | degrees it tips when carried, not used: negative tips its head forward and down in a hanging hand, positive raises it | `fit_held.py` | `terra_walker::character` (`held_ready`) |
| `hold` | a hung look | `"hung"` | `fit_held.py` | `attach_contract.ts`; tools that must tell it from a look in hand |
| `strap` | a strapped look | `{"bone", "at", "face", "grip"}`: the bone it rides; where its origin sits in that bone's frame (m); the directions in that frame its front (-Z) and grip (+Y) turn to | `fit_held.py` from its data | `terra_walker::strap`, `tools/characters/moves.ts`, the icon rig |
| `socket` | a rigid look | the socket's name | `fit_gear.py` | `terra_walker::character` (dress) |
| `morphs` | a rigid look | `{morph: [x, y, z]}`, its shift beyond its socket's at the morph's full weight, m, in the socket's space | `carry_gear.py` | `terra_walker::character` |
| `swing` | a loose rigid look | `{"stiffness", "drag", "gravity", "tail": [x, y, z]}` | `fit_gear.py` | `terra_walker::swing` |
| `covers` | a worn look | the patches of skin it hides, ascending, each under 1,024 | `fit_gear.py`, `shell_garment.py`, `carry_gear.py` | `terra_walker::character`, `body_mask.wesl` |
| `hung` | a loose skinned garment (a skirt, an apron) | `1` | `hung_garment.py` | `carry_gear.py` |
| `icon_turn` | a worn look | degrees about +Y its icon is seen from (a pack's 180, from behind) | `fit_gear.py` | `terra_survival::icons` |
| `icon_leaves_out` | a worn look | material roles its icon does not draw (a pack's `Harness`) | `fit_gear.py` | `terra_survival::icons` |

Materials are named by role (Wood, Metal, Leather, Trim), so a tint recolours one look many ways (characters.md, Colour).

## The check

`node tools/characters/attach_contract.ts` reads every committed look in `assets/models/held` and `gear`, their data and `base.glb`, and holds each to its kind's rules above (its kind named, its mount's origin, axes, facing and extras, its life size, its grip across and along the handle), and the body's sockets to the table; its test runs in `cargo xtask check`. Given files, it judges each by the folder it sits in. A fault known today is listed with why in `tools/characters/attach_faults.json`; a fixed one fails the test until its line is removed, so the list only shrinks.

Known today (2026-10-27): the eight looks fitted by length (both axes, both pickaxes, the hammer, the knife, the spear, the bow) lie across X, 90 degrees off, as `fit_held.py` never turns them about +Y; six of them sit 2.5 to 4.7 cm off their grip, as the length fit centres the model's bounds; and the basket predates its `hold` marker.

## How others do it

- **glTF 2.0** (cited above): the axes and units, and `extras` for an application's own data on any node or mesh.
- **Unreal** (from memory): a skeletal mesh socket is a named bone offset; a weapon's mesh has its pivot at its grip and is attached by `AttachToComponent` with `SnapToTarget` rules, so a weapon fits every character with a hand socket. Modular characters share one skeleton, each part a skinned mesh following the leader's pose (the Leader Pose Component).
- **Godot** ([BoneAttachment3D](https://docs.godotengine.org/en/stable/classes/class_boneattachment3d.html)): a node that follows one bone, under which held things are placed.
- **Bethesda's Creation Engine** (from memory): the skeleton has attach nodes (`WEAPON`, `SHIELD`, `WeaponBack`, `QUIVER`); a weapon's model has its grip at its origin; armour is skinned to the skeleton and claims body slots that hide what is under it, as our `covers` does.
- **Unity** (from memory): `Animator.GetBoneTransform(HumanBodyBones)` with a child as the attach point, the same idea.
- **VRM** ([VRMC_springBone](https://github.com/vrm-c/vrm-specification/tree/master/specification/VRMC_springBone-1.0)): spring bones for what swings, which our `swing` follows.
- **Where we depart**: hand sockets are placed per body from the skin, not authored per skeleton; a rigid look carries its own shift per morph, since a socket follows bones and a heavy body's skin moves past them; a pack's bag is skinned whole to one bone rather than attached, so it and its skin-made straps move as one.
