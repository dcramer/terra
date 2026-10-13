# Characters: bodies and what they wear

How bodies, clothing, armour, packs and weapons are made so that **every item fits every body without being fitted to each one by hand**, and how the game draws them. A new item is made once and works on every character, and a new body works with every item already made: the cost grows with items plus bodies, never items times bodies (D127). Current thinking (D112, D128, Leaning).

The research behind it: [modern-character-variety.md](../research/modern-character-variety.md), [character-systems-industry.md](../research/character-systems-industry.md) (the techniques T1 to T9 named below), [bevy-character-capabilities.md](../research/bevy-character-capabilities.md), [skin-deformation.md](../research/skin-deformation.md), [body-morphs.md](../research/body-morphs.md), [one-mesh-for-every-body.md](../research/one-mesh-for-every-body.md) and [character-creators.md](../research/character-creators.md).

## What the developers want

- **A creator of a few sliders** (D111): sex, build (heavier, thinner, stronger), height (widely, for other races later) and skin colour; hairstyle and hair colour, which can be changed later. No other proportions yet. NPCs come from the same sliders by seed.
- **Gear in every slot fits every body** (D64, D128): head, chest (covering the arms), gloves, legs, feet, back and belt; more may come. Not every slot shows: a quiver on the belt does, not every belt item does. One piece per slot, no layers.
- **No one is ever naked** (D186): the base bodies wear plain, close-fitting underwear. What a character starts in, such as shorts and an undershirt, is ordinary worn items that can be taken off or replaced.
- **Looks good** in the art style ([art-style.md](art-style.md), D14), at life size (D110).
- **Looks right in motion:** no twisting, squeezing or poke-through in any clip, on any body, checked by machine (D69).
- **Many characters on screen**, toward a thousand players (D127).
- **One command** to add a body parameter, an item or a clip, and one command to check everything.

Nothing ready-made fits. Reallusion Character Creator's and Synty Sidekick's licences forbid generated or AI work, Daz needs a game licence per item, MetaHuman is made only in Unreal and is realistic, and Unreal's Mutable and Unity's UMA are tied to their engines. We rebuild their techniques on Bevy (D184).

## The contracts

Games that do this well don't fit items to characters. They fix a few contracts that every asset follows, and the fitting falls out of them:

- **One base body, and every body is a few parameters on it.** Build and sex are morphs of the same mesh, height is a scale. Each slider setting and each named template is a setting of those, not a separate model.
- **One skeleton.** Every body has our skeleton (D115), and every animation plays on every body.
- **Gear is made once, on the base.** It rides the skeleton, so proportions come free, and it carries the base's morphs, copied by a tool. That is how Starfield, Red Dead Redemption 2, Final Fantasy XIV and Bannerlord fit one item to every body.
- **The body hides what is covered.** The body is divided into patches; each item says which it covers, and those are not drawn while it is worn. The patches are the same on every body, because the mesh is.
- **Rigid things hang on named points with fixed axes.** Every body has the same sockets, placed for that body. A sword or a helmet is made once to the socket's convention and fits every body.
- **Generated art supplies the look, not the shape's structure.** Tripo makes a body's look; that body becomes a shape of the shared mesh with its texture copied onto the shared layout. Tripo makes a garment's look once, as the base body wearing it; that look is baked onto the garment.

### Bodies

| Parameter | Technique | How | State |
|---|---|---|---|
| **Build and sex** (heavy, thin, strong, female) | T2: morph targets on the base mesh | Bevy's `MorphWeights`; the game mixes joint offsets per morph | Built |
| **Height** | T1 at its simplest: a uniform scale of the whole character. Gear, sockets and clips scale with it, so nothing is checked per height; what is held keeps its own size | The character's root scale; our skinning keeps a uniform scale | Built |
| **Skin colour** | T8: a tint baked out of the albedo, a skin mask and a melanin-like palette in the shader | The body's material extension | Painted skins built; tint not yet |
| **Other proportions** (limb length, shoulders, head) | T1: bone scales on leaf or helper bones, so nothing shears | A system writes joint rest transforms | Not yet (D111) |
| **Head and face** | T2: blends of head morphs, never animated by clips | Its own weight set | Open |
| **Hair** | A separate part bound to the head like gear (D112); hidden or swapped for a short version under a helmet | A worn look, by name | Not yet |

- **A character is a few slider values** (D111): morph weights, a height and a skin tint, each clamped to what every item passes at. The named templates (`models/character/templates.json`) are presets of the same numbers, for people and tests; an NPC's values come from its seed, within its people's ranges.
- **Sockets**, the same names and axes on every body: hands, head, back, belt.

### Clothing and armour (they bend with the body)

- **Made once, on the base.** A skin garment is the base's faces it covers (said once per item as a coverage: which parts, where its hems fall), pushed out to its stand-off, its edge cut smooth with a rim turned in. A piece with its own geometry (a collar, a hood, plate) is bound to the nearest base surface; a loose one (a skirt, coat tails) to a smooth helper between the legs.
- **It follows any body for free**: skinned to the shared skeleton, it takes the body's proportions, and it carries the body's build morphs, copied from the base by a tool. Parts marked rigid ride one bone instead. One mesh per item, shared by every wearer.
- **Its colour is painted once**, on its own layout, so it is the same image on every body.
- **It says what it covers**: the patches whose skin lies wholly inside it, in its glTF extras.
- **An exception, not a rule**: an item that fails its check on one body gets that body's own model, as Final Fantasy XIV does.

### Heights off the skin

No layers (one piece per slot), but pieces from different slots overlap: a pack's straps cross the chest piece. Each slot has a height off the skin it stays within, so they stack without fitting every pair: chest pieces within 1.8 cm above the waist, where straps cross them (below it, a hem may stand out further); what crosses them, such as straps, solid from the skin to 2.2 cm, so it rests on bare skin and shows over a coat alike.

### Rigid worn things (helmets, packs, quivers, belt pouches)

- **Made once, in its socket's space**, by a rule for that socket: a helmet's inner crown at the origin, a pack's back panel on the socket's plane, a quiver's belt loop at the origin. Scaled from the body's own measure at that socket (head width, back width) when fitted to the base. No numbers per item beyond which socket.
- **It follows the skin under it per morph**: its shift beyond the socket on each morph is recorded in its extras, and the game mixes them by the body's morph weights, so a heavy body's seat does not swallow a quiver.
- A helmet covers the head's top and says so; hair under it is hidden or swapped for a short version (D112).
- **A pack is a bag and a harness** (D189). The bag is any generated model, rigid on the back socket, its own straps cut away; it is fitted so its corners meet the harness's strap ends. The harness is the straps, made once per kind (leather straps, rope, a frame) on the base body like a garment, so they bend with the shoulders and carry the morphs, its path and look taken from a model of the base person wearing one. A new pack is one bag model; it names a harness. The belt is the same: a quiver, a pouch or a sheath hangs from a belt's standard points, from where it hangs on it (a quiver from its mouth).
- **Loose things swing.** What hangs from a strap or a belt (a quiver, a pouch) swings about where it hangs, under gravity, damped and limited, kept off the thigh by a capsule, as spring bones do (the VRM spring-bone model; Source's jiggle bones). Its pivot and stiffness are data in its file. The same system later moves coat tails and long hair (phase 5 of the plan).

### Held things (tools, weapons)

- **Made once, for every body**: the grip at the origin, the handle along +Y toward the working end, the striking edge or face toward +Z, the length real. The hand socket is placed for each body, so nothing is set per character.
- How it is carried and which clips it swings with are in its file, not in code. A two-handed weapon also marks where the off hand holds it, for the arm to reach.

### Colour

- Materials are named by role (Leather, Metal, Cloth, Trim), so a tint recolours one look many ways.

## The base body

- **One mesh, made once and checked**: our own base male in our T-pose, drawn as a design sheet and made by Tripo (D115), wearing plain close-fitting underwear painted on with almost no relief (D186). It has our skeleton, skin weights made for it, the skin patches (1,024) and the socket rules, and passes the rig checks before anything uses it.
- **One skeleton**, with Unreal's bone names, which Quaternius's animation libraries use; Tripo's V3 rig is converted onto it, keeping its weighted twist bones (D115).
- **Twist and skinning**: a limb's roll is moved to the hand and foot and spread over the twist bones, baked into each clip (Unreal's twist bones); bodies and gear are skinned weight-blended, dual quaternions but round the hips (our own skinning shader).
- **The morphs are made once each from designs of the base person** (heavier, thinner, stronger, a woman, later another head): the base's mesh is registered onto each design's model and what moved is kept as a morph, joints included.
- **A template from a design**: the parameters that bring the base closest to it are estimated, and Tripo paints the base's layout in those proportions as that template's skin.

## The game's side

- **A character's appearance is data**: its slider values, skin and tints, hairstyle and what it wears. A few bytes, saved and sent as they are; an NPC's comes from a seed.
- **Bevy draws and animates**: one skinned mesh for the body and one per worn piece, sharing the skeleton. Height is the root's scale; build morphs are one set of weights the body and its pieces share. No mesh is built per character, so a crowd in the same jerkin draws one mesh.
- **Each worn look is re-bound to the body's joints**, its file's copy of the skeleton then dropped, and its skin cut down to the joints it uses: a cap needs the head, not 73 joints.
- **One crate turns appearance into numbers.** `terra_body` (not created yet), without Bevy, will turn appearance data into joint transforms and morph weights, tested without a GPU and shared by the tools and the game. Today `terra_game` does this itself.
- **When Bevy won't do, we extend, then replace** (D184). The likeliest replacement is computing joint matrices once per skeleton rather than per skinned mesh, if the crowd measurement asks for it; before that comes merging each character's parts into one skinned mesh with a texture atlas (T9, as UMA and Mutable do; Bevy's own merge does not do skins or morphs).

## Tools and checks

The pipeline is in [tools/characters](../tools/characters/README.md) and the step by step is the `character` skill (`.agents/skills/character/SKILL.md`). Everything the game loads is rebuilt from `art/` by `tools/characters/build_bodies.sh`; a morph is made end to end by `make_morph.sh` and a template's skin by `paint_skin.sh`. The sandbox's workbench shows models in the game's own light through every clip (`--template`, `--wear`, `--joints`, `--sheet`), and its crowd experiment measures what many cost.

The checks run on every item, on every body, through clips that bend and reach, so a person isn't the test:

- **Bodies:** the rig checks against our skeleton and the mannequin, and a deformation check along every limb.
- **Clips:** each one is one our skinning can follow.
- **Clothing and armour:** how much skin pokes through it, frame by frame; the clipping fixer (BodySlide's, per morph, through our clips) records how far each part must move out, and a round is kept only while it helps.
- **Rigid worn things:** how much of it rests on the body, and that it never sinks in (the first half built).
- **Held things:** that it never passes through the body or the head through the swings (not yet).
- **A sheet of every item on every body**, for a person to look at last, not first.
- **Every item's icon**, rendered from its look in the house style (D130) and read at slot size: a look that breaks a contract shows it here too (the `item-icons` skill).

An item that fails a check isn't used until it passes.

## Order of work

Each phase ends when its checks pass in the build and a person has looked at the workbench sheets. What is built and measured is in [body-morphs.md](../research/body-morphs.md#in-the-game-2026-10-12) and [bevy-character-capabilities.md](../research/bevy-character-capabilities.md#measured-2026-10-12).

**Next:** the base bodies are redrawn in plain underwear (D186), then their rigs, morphs and gear rebuilt by `build_bodies.sh`, and today's shorts and undershirt made as worn items. Every phase after builds on them.

| Phase | Builds | Done when | State |
|---|---|---|---|
| **0. Deformation and gear on morphs** | One skeleton, the twist step, weight-blended skinning; build and sex morphs; garments cut from the body; gear carried onto every morph, with the clipping fixer; rigid looks on sockets that follow the skin per morph; skin hidden by 1,024 patches | Every item passes on every body; the forearm squeeze through the libraries' 20 hardest clips is down from 70% to 0–13% | Done |
| **1. Scale foundations** | Copied skeletons dropped; garment skins trimmed; a crowd benchmark (`--experiment crowd`); the body material (`BodyMask`) made bindless, so bodies batch | 1,000 dressed characters measured on the fast and integrated GPUs | Done but bindless. 1,000 characters on the RTX 5090 went from 25.7 to 13.3 ms. The integrated GPU costs 2.5–4 ms per dressed character on a world that alone takes 29.6 ms, which brings LOD forward |
| **2. Body parameters** | `terra_body`; height as a scale; skin tint; templates as data; morph weights shared by reference, garments listing morphs in the body's order; the parameter grid in the build (each limit, and mixes such as half heavy and half strong) | Rig and gear checks pass at every limit and mix; joints in the game match the tools to the millimetre (`--joints`); workbench sheets of each template | Height and templates as data built; the rest not yet |
| **3. Gear from any model** | Weights copied from the body by Epic's inpainting method (T4, MIT reference code); morphs by surface binding with rigid clusters (T2/T3, Outfit Studio's averaging, Mutable's Reshape) in place of nearest vertex; per-triangle hiding from a pose sweep (T6); garment edges; every clip in the rig and gear checks | A boot and a hood made from their own models pass on every template; a rendered hole check finds no holes; no ragged edges on the sheets | Smooth garment edges with a rim built; every clip in the rig and gear checks; hiding from a pose sweep (a patch hidden only while it stays covered in every frame of every clip on every body), still by patches; packs as a bag and a harness (D189); the rest not yet |
| **4. Contact** | Two-bone foot IK after animation; the pelvis offset by body size: for each clip and body shape the tools measure, frame by frame, how much deeper than the mannequin the body goes into the floor (a thicker body in a roll), the base's own lift baked into its clips as the twist step is, and the game adding each morph's share of the rest to the hips after the animation; a held tool's off hand | Feet within 1 cm of the ground while standing on every template; no body below the floor by more than 2 cm deeper than the mannequin in any clip (the rig check, every clip) | Not yet. The new base goes 4.9 cm deeper in the roll |
| **5. Loose parts and hair** | Pose-driven correctives (pose-space deformation, as Unreal's Pose Driver and MetaHuman's correctives): a corrective shape on a garment, its weight set each frame from a joint's angle, so a hem lifts as the thigh rises; then spring bones with capsule colliders (T7, the VRM spring-bone model); hair as parts; hide flags and short-hair variants under helmets | A long coat and long hair pass the gear check through every clip on every template; no hair through a helmet | Swing for hung things built (the quiver). The vest's hem lets the thigh through in the crouch-walk (2.2%), which no push fixes: the first corrective |
| **6. Crowds** (LOD may come forward, as the integrated GPU needs it) | LOD meshes for bodies and gear, and fewer shadow passes for far characters; animation at a lower rate for far characters (as Unreal's Animation Budget Allocator); a merge and atlas per character if still needed; NPC looks from a seed | The crowd budget is met on the integrated GPU | Not yet |
| **7. Bevy 0.20** | Shaders in WESL; the skinning override redone | Every check passes and the sheets match the 0.19 ones | **Done** (2026-10-12), sooner than planned at the developers' request, with the inspector on its open port (D31): the forearm and gear sheets match 0.19's pixel for pixel ([research/bevy-0-20-upgrade.md](../research/bevy-0-20-upgrade.md)) |

## Risks

| Risk | How we would know | If it happens |
|---|---|---|
| Morph-blended shapes clip between the extremes | The parameter grid in the build | A corrective morph for that pair, or clamp the slider |
| Bone scales shear or squash gear | Rig and gear checks at the limits | Scale on helper or leaf bones only (T1) |
| The crowd costs too much | The phase 1 benchmark | Trim skins, then merge per character, then LOD and animation rate, in that order |
| Weight inpainting is slow or unstable on generated meshes | The rig check on the transferred weights | Fall back to nearest weights with smoothing for the parts it fails on |
| Pose-only clipping on loose parts, which no push can fix | The gear check | Helper and spring bones (phase 5), not pushes |
| A third-party clip misbehaves | The clip check | Check the author's current version first; replace the clip |

## Open questions

For the developers. Answered: sliders only, height varying widely and no other proportions yet (D111); sex as a morph of the same base, not a second base (D112, built); plain underwear on the base and starting clothes as items (D186); no layers (D64).

1. **Height range for humans,** and later per race.
2. **Head and face variety:** blends of a few heads, or fixed faces?
3. **Hair:** how many styles to start; long hair with physics, or short only at first?
4. **Loose garments:** are long coats, robes and cloaks wanted early? They need phase 5.
5. **Jewellery** (D64): two rings and a necklace, or a few trinket slots; probably not shown on the body.
