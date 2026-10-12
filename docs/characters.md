# Characters and what they wear

How bodies, clothing, armour, packs and weapons are made so that **every item fits every body without being fitted to each one by hand**. Many of everything (D127): a new item is made once and works on every character, and a new body works with every item already made. Current thinking (D128, Leaning); the research behind it is [research/character-creators.md](../research/character-creators.md).

## The idea

Games that do this well don't fit items to characters. They fix a few contracts that every asset follows, and the fitting falls out of them:

- **One skeleton, one rest pose.** Every humanoid shares bone names, bone directions and a T-pose, so every animation plays on every body (D112, D115).
- **A few base bodies; everyone else is a variation.** Male and female bases, and creator templates as morph targets of a base, not new models (D111, D112). An item is made for each base, never for each character.
- **Items are made on the body, not beside it.** Clothing and armour are shaped around the base body they will be worn on, and take its skin weights and its morph targets. That is why a coat follows a body's bends and its build: it was made from that body.
- **The body hides what is covered.** The body is divided into regions; each item says which regions it covers, and those are not drawn while it is worn. Skin cannot poke through what it is not drawn under. WoW, Skyrim, Cyberpunk and Unreal's Mutable all do this.
- **Rigid things hang on named points with fixed axes.** Every body has the same sockets, placed for that body by its export. A sword or a helmet is made once to the socket's convention and fits every body.

## The contracts

### Bodies

- Our skeleton (the Quaternius one, plus forearm twist bones), T-pose, life size (D110).
- **Regions**, each a separately hideable part of the body mesh: head, neck, torso, upper arms, forearms, hands, hips, thighs, shins, feet (left and right where there are two). Cut by the export from the bones' weights, so every body gets the same regions.
- **Sockets**, the same names and axes on every body (`export_character.py`): hands, head, back, belt.
- Templates are morph targets of the base; hair is a separate part (D112).

### Clothing and armour (they bend with the body)

- **Made on each base body.** The concept is drawn on that base's approved sheet, in its T-pose ("the same person, wearing a leather coat"); Tripo makes the dressed body; a script keeps what stands off the skin and drops the body. So the piece already has the body's proportions and place: no turn, size or position to set. It is made once per base (two today), each from that base's own sheet.
- It takes the base's skin weights and morph targets, so it bends and changes build with the body.
- **It says what it covers and how deep it sits**: the regions whose skin lies inside it, worked out by the script, and a layer (under, over, outer), in its glTF extras. A coat over a shirt hides the shirt's covered parts as it hides skin.

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
- **Each piece is made on each base body**, with Studio credits.
