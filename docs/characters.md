# Characters and what they wear

How bodies, clothing, armour, packs and weapons are made so that **every item fits every body without being fitted to each one by hand**. Many of everything (D127): a new item is made once and works on every character, and a new body works with every item already made. The cost grows with items plus bodies, never items times bodies. Current thinking (D112, D128, Leaning); the research behind it is [research/one-mesh-for-every-body.md](../research/one-mesh-for-every-body.md) and [research/character-creators.md](../research/character-creators.md).

## The idea

Games that do this well don't fit items to characters. They fix a few contracts that every asset follows, and the fitting falls out of them:

- **One mesh for every body.** Every body is the same mesh, with the same vertices in the same order and the same texture layout, moved by stored shapes. Male, female, each creator template (D111) and any later customisation are shapes of it, not separate models. The mesh is MakeHuman's (CC0), which comes with the shapes, a skeleton placed from the mesh and a clothing format.
- **One skeleton, placed by the body.** Every body has our skeleton (D115), its joints placed from that body's own shape, and every animation plays on every body.
- **Gear is made once, on the shared mesh.** A garment is bound to the mesh once, and on any body it moves as that body's surface does. That is how The Sims, BodySlide and MetaHuman fit one garment to every body.
- **The body hides what is covered.** The body is divided into patches; each item says which it covers, and those are not drawn while it is worn. The patches are the same on every body, because the mesh is.
- **Rigid things hang on named points with fixed axes.** Every body has the same sockets, placed for that body. A sword or a helmet is made once to the socket's convention and fits every body.
- **Generated art supplies the look, not the shape's structure.** Tripo makes a body's look; that body becomes a shape of the shared mesh with its texture copied onto the shared layout. Tripo makes a garment's look once, as the base body wearing it; that look is baked onto the garment.

## The contracts

### Bodies

- **The shared mesh:** MakeHuman's body (hm08), 13,380 vertices, all quads, without its helpers. Its shapes are our templates. Its own targets (gender, weight, muscle, height, proportions) are there if customisation grows.
- **Our skeleton** (the Quaternius one, plus forearm twist bones) in a T-pose, its joints placed from the mesh for each shape. Clips carry rotations, and the hips' height scaled to the body, so they play on any proportions.
- **A new body** (a template, D111) is made by Tripo from its sheet, as now, then fitted: the shared mesh's own shapes first, then a smooth residual, and its texture baked onto the shared layout. The result is one more shape and one texture, and works with every item already made.
- **Sockets**, the same names and axes on every body, placed from each body's own joints and skin: hands, head, back, belt.
- **Patches** (1024), cut once on the shared mesh.
- Hair is a separate part, bound to the head like gear (D112).

### Clothing and armour (they bend with the body)

- **Made once, on the base shape.** A piece is the shared mesh's faces it covers (said once per item as a coverage: which parts, where its hems fall), pushed out to its stand-off. Its colour and surface detail are baked from one Tripo model of the base wearing it. Its texture is on its own layout, so it is the same image on every body.
- **On any body**, each vertex of the piece follows the body vertex it was made from, pushed out along that body's surface, and takes that vertex's skin weights. Parts made of their own geometry (a collar, a hood, plate) are bound to the nearest three body vertices, MakeHuman's way, and loose parts (a skirt, coat tails) to a smooth helper instead of the legs.
- **It says what it covers**: the patches whose skin lies wholly inside it, in its glTF extras.

### Heights off the skin

No layers (one piece per slot), but pieces from different slots overlap: a pack's straps cross the chest piece. Each slot has a height off the skin it stays within, so they stack without fitting every pair: chest pieces within 1.8 cm above the waist, where straps cross them (below it, a hem may stand out further); what crosses them, such as straps, solid from the skin to 2.2 cm, so it rests on bare skin and shows over a coat alike.

### Rigid worn things (helmets, packs, quivers, belt pouches)

- **Made once, in its socket's space**, by a rule for that socket: a helmet's inner crown at the origin, a pack's back panel on the socket's plane, a quiver's belt loop at the origin. Scaled from the body's own measure at that socket (head width, back width) when fitted to each base. No numbers per item beyond which socket.
- A helmet covers the head's top and says so; hair under it is hidden or swapped for a short version (D112).

### Held things (tools, weapons)

- **Made once, for every body**: the grip at the origin, the handle along +Y toward the working end, the striking edge or face toward +Z, the length real. The hand socket is placed for each body, so nothing is set per character.
- How it is carried and which clips it swings with are in its file, not in code. A two-handed weapon also marks where the off hand holds it, for the arm to reach (later).

### Colour

- Materials are named by role (Leather, Metal, Cloth, Trim), so a tint recolours one look many ways (built).

## In the game

- **A character's appearance is data**: a template, its tints, what it wears. A few bytes, saved and sent as they are; an NPC's comes from a seed.
- **The body is built when a character appears or changes what it wears**, not every frame: the shape's vertices, its joints and each piece fitted to it. The fitted meshes are cached by appearance and item, so a village of people in the same jerkin share one.
- **The fitting is our own Rust**, in a crate that does not depend on Bevy, used by the offline tools and the game alike. It is reimplemented from MakeHuman's formats and the papers, not copied: MakeHuman's code is GPL.
- **Bevy only draws and animates**: an ordinary skinned mesh per body and per piece, sharing one skeleton.

## Building it, in order

1. **Spike:** does the shared mesh, fitted to our two Tripo bodies, look as good as they do, and does one jerkin made once pass the clipping check on both and on a heavier shape? Offline, in Blender, through the existing pipeline.
2. **The fitting crate** and the game building bodies from it, replacing a file per body per item.
3. **The tools** in `tools/characters/` and the playbook, so adding a body or a piece is one command each.

## Checks, so a person isn't the test

Run on every item, on every base body, through clips that bend and reach:

- **Bodies:** the rig checks (built).
- **Clothing and armour:** how much skin in uncovered regions pokes through it, and how far it stands off the body, frame by frame.
- **Rigid worn things:** how much of it rests on the body, and that it never sinks in (the first half built).
- **Held things:** that it never passes through the body or the head through the swings.
- **A sheet of every item on every body**, for a person to look at last, not first.
- **Every item's icon**, rendered from its look in the house style (D130) and read at slot size: a look that breaks a contract shows it here too (built: the `icons` experiment, `.claude/skills/item-icons`).

An item that fails a check isn't used until it passes.

## Against what we have, 2026-10-12

Built: the bodies, their skeleton with twist bones, sockets, held looks, tints, one look per item per body, and a body that hides its skin under what it wears, by 1024 patches of about two centimetres its shader collapses (`patches.py`, `body_mask.wgsl`). Every piece is checked on every body through our clips (`gear_check.py`, the share of nearby skin that comes through it, at most 2%):

| Piece | How it is made | Male | Female |
|---|---|---|---|
| Leather jerkin | On each body (`shell_garment.py`) | at most 1.6% (rolling) | at most 2% (crouching) |
| Leather cap | On each body (`shell_garment.py`, `head_cap`) | 0.6% | 0.4% |
| Packs | Bag at the back, top at the shoulders; straps made from the skin (`bands.py`) | at most 1.5% | at most 0.4% |
| Quiver | At the belt socket, behind the hip | 0% | 0% |

All pass. What comes through is at the hips in a roll or crouch, where the jerkin's hem meets a bending thigh. With 256 patches the strip of skin at a hem, under patches only partly covered and so still drawn, came through at 3%; finer patches made it thin enough. Making the hem flare out instead made it worse. A cap fitted as a rigid object cut into the head above the ears (7-8%); made on the head, it does not. A pack's straps fitted as part of a generated model clipped at the shoulders (5-8%); made from the skin, they do not.

Still open: the edges of a made-on-body piece (armholes, neck) look ragged, where the bake catches the dressed model's own rim. Not built yet: pieces that hang loose (a coat's skirt, a cloak), and a third body to show the rules hold beyond the two they were made with.

## Settled by the developers, 2026-10-12

- **Slots** (D64): head, chest (covering the arms), gloves, legs, feet, back and belt; more may come. Not every slot shows: a quiver on the belt does, not every belt item does. Jewellery is open (two rings and a necklace, or trinket slots), probably unseen.
- **One piece per slot, no layers** for now. The layer contract above waits.
- **Each piece is made on each base body**, with Studio credits. Superseded the same day by the shared mesh: a piece is made once, from one dressed model.
