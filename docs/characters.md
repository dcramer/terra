# Characters and what they wear

How bodies, clothing, armour, packs and weapons are made so that **every item fits every body without being fitted to each one by hand**. Many of everything (D127): a new item is made once and works on every character, and a new body works with every item already made. The cost grows with items plus bodies, never items times bodies. Current thinking (D112, D128, Leaning); the build plan is [character-system-plan.md](character-system-plan.md); the research behind it is [research/modern-character-variety.md](../research/modern-character-variety.md), [research/one-mesh-for-every-body.md](../research/one-mesh-for-every-body.md) and [research/character-creators.md](../research/character-creators.md).

## The idea

Games that do this well don't fit items to characters. They fix a few contracts that every asset follows, and the fitting falls out of them:

- **One base body, and every body is a few parameters on it.** Proportions are bone scales; build, sex and head are a few morphs of the same mesh. Male, female, each creator template (D111) and any later customisation are settings of those, not separate models.
- **One skeleton.** Every body has our skeleton (D115), its proportions from its parameters, and every animation plays on every body.
- **Gear is made once, on the base.** It rides the skeleton, so proportions come free, and it carries the base's morphs, copied by a tool. That is how Starfield, Red Dead Redemption 2, Final Fantasy XIV and Bannerlord fit one item to every body.
- **The body hides what is covered.** The body is divided into patches; each item says which it covers, and those are not drawn while it is worn. The patches are the same on every body, because the mesh is.
- **Rigid things hang on named points with fixed axes.** Every body has the same sockets, placed for that body. A sword or a helmet is made once to the socket's convention and fits every body.
- **Generated art supplies the look, not the shape's structure.** Tripo makes a body's look; that body becomes a shape of the shared mesh with its texture copied onto the shared layout. Tripo makes a garment's look once, as the base body wearing it; that look is baked onto the garment.

## The contracts

### Bodies

- **One base body, made once and checked**: one mesh in our T-pose (our own base male, made by Tripo from its sheet), wearing plain close-fitting underwear painted on with almost no relief (D186), our skeleton, skin weights made for it, the skin patches and the socket rules. It passes the rig checks before anything uses it.
- **A body is a few parameters on it**, as in modern games ([research](../research/modern-character-variety.md)): proportions as bone scales (height, limb length, shoulders, hips, head), build as a few soft morphs (heavy, muscular, thin, chest, belly), the head as blends of head shapes, and a painted skin, tinted. Sex is a morph or a second base; the spike decides.
- **A character is a few slider values** (D111): sex and build as morph weights, height as a uniform scale, skin colour as a tint, each clamped to what every item passes at. The named templates are presets of the same numbers, for people and tests; an NPC's values come from its seed.
- **A template from a design**: Tripo makes the design's model, as now; the parameters that bring the base closest to it are estimated, and Tripo paints the base's layout in those proportions.
- **The morphs themselves are made once each from designs of the base person** (heavier, thinner, stronger, a woman, another head): the base's mesh is registered onto each design's model and what moved is kept as a morph.
- **Sockets**, the same names and axes on every body: hands, head, back, belt.
- Hair is a separate part, bound to the head like gear (D112).

### Clothing and armour (they bend with the body)

- **Made once, on the base.** A skin garment is the base's faces it covers (said once per item as a coverage: which parts, where its hems fall), pushed out to its stand-off. A piece with its own geometry (a collar, a hood, plate) is bound to the nearest base surface; a loose one (a skirt, coat tails) to a smooth helper between the legs.
- **It follows any body for free**: skinned to the shared skeleton, it takes the body's proportions, and it carries the body's build morphs, copied from the base by a tool. Parts marked rigid ride one bone instead. One mesh per item, shared by every wearer.
- **Its colour is painted once**, on its own layout, so it is the same image on every body.
- **It says what it covers**: the patches whose skin lies wholly inside it, in its glTF extras.
- **An exception, not a rule**: an item that fails its check on one template gets that template's own model, as Final Fantasy XIV does.

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

- **A character's appearance is data**: its parameters, skin and tints, hairstyle and what it wears. A few bytes, saved and sent as they are; an NPC's comes from a seed.
- **Bevy draws and animates**: one skinned mesh for the body and one per worn piece, sharing the skeleton; proportions are each body's joint rest positions and scales; build morphs are one set of weights the body and its pieces share. No mesh is built per character, so a crowd in the same jerkin draws one mesh.
- A small Rust crate without Bevy, `terra_body` (not created yet), will turn appearance data into those joint positions, scales and morph weights. The build plan is [character-system-plan.md](character-system-plan.md).

## Checks, so a person isn't the test

Run on every item, on every base body, through clips that bend and reach:

- **Bodies:** the rig checks (built).
- **Clothing and armour:** how much skin in uncovered regions pokes through it, and how far it stands off the body, frame by frame.
- **Rigid worn things:** how much of it rests on the body, and that it never sinks in (the first half built).
- **Held things:** that it never passes through the body or the head through the swings.
- **A sheet of every item on every body**, for a person to look at last, not first.
- **Every item's icon**, rendered from its look in the house style (D130) and read at slot size: a look that breaks a contract shows it here too (built: the `icons` experiment, `.agents/skills/item-icons`).

An item that fails a check isn't used until it passes.

## Where it stands

One base body with its morphs, gear made once, every body a template in the game. What is built and measured is in [research/body-morphs.md](../research/body-morphs.md#in-the-game-2026-10-12); the build plan, its phases and the questions still open are in [character-system-plan.md](character-system-plan.md).

## Settled by the developers

- **Slots** (D64): head, chest (covering the arms), gloves, legs, feet, back and belt; more may come. Not every slot shows: a quiver on the belt does, not every belt item does. Jewellery is open (two rings and a necklace, or trinket slots), probably unseen.
- **One piece per slot, no layers** for now. The layer contract above waits.
- **What a character starts in**, such as shorts and an undershirt, is ordinary worn items that can be taken off or replaced (D186).
