# Assembled items: equipment made from parts by rules

Status: decided (D565, the developers approved the recommended answers on 2026-10-28). Parts are made only from the approved style reference picture, `art/style/reference.png` (the developers' pick, 2026-10-28, candidate A). A theme (a realm or a people) may get its own reference later, and a part's art can be remade from a new one without changing any item's recipe.

Weapons and tools are made from a few parts per kind, each made once with named connection points, and put together by rules. Every attachment is then exact by construction: a new item is a choice of parts, never a model to be fitted, measured or tuned. It is how the game reaches thousands of items without a number set by hand for any one of them, and it is D182 (hundreds of variants from parts by rules) applied to equipment.

## Why

The developers, 2026-10-28: "we keep making minor incremental tweaks to make attachments work. I want a deterministic system as best as possible. If i have to help you do it im going to have to be taught how, but that seems unscaleable if we have 1000s of items". Then: "maybe we need to reconsider our approach and look more at procedural approaches but im still worried about how they attach", and "how do we make sure they render correctly".

Today a held item arrives as a whole model (Tripo, a free pack) facing whichever way its maker chose. Its grip and facing are worked out from its shape (`detect.ts`), and what is left is tuned (a carry tilt, a grip offset). Each step can be wrong in a way only a still shows, and it repeats per item.

## How shipped games do it

From what is known of them; a research note should confirm the details before this is built.

- **Unreal and Unity**: sockets on the skeleton (hand, back, hip) and on the weapon (grip, off-hand grip, tip, trail). Attaching snaps the weapon's grip to the hand's socket. Animations go by weapon class (Lyra links an animation layer per weapon type), and IK pulls the off hand to the weapon's off-hand socket. A model with a wrong pivot is refused at import.
- **Borderlands**: guns from parts (barrel, grip, stock, sight, magazine) snapped onto named points on a body per maker and gun type; the body says where the hands go, the type which animations play, a rule table which parts.
- **No Man's Sky**: multitools, ships and creatures from parts authored to plug-in points on a base.
- **Veloren** (GPL: its docs only): modular weapons of a head and a handle; one hold and one animation set per tool kind.
- **Unreal's Mutable**: characters' clothes and body parts merged at run time from authored pieces on a shared skeleton.

The common rule: attachment points come from how an item is made, not from measuring it.

## The design

### Parts and connection points

- **A part** is a model of one piece (a handle, an axe head, a blade, a guard) made to its type's template frame: glTF axes (D303), real size.
- **A connection point** is a named point with a direction, in the part's frame, with a type and size (a round haft top of 3 cm, say). Parts join only where types and sizes match.
- **Assembling** places each part so its point coincides with its partner's, one fixed transform, the same every time. The item's grip, second grip and working edge are its parts' points carried through the assembly.
- **Handles are made by code** (a shaft of a given length and thickness), so their grip, ends and thickness are exact. Heads, blades and guards are the looks, made once and checked.

### Kinds

A kind (one-handed haft, blade, polearm, strapped shield, bow, hung item, torch) owns everything that is not the parts: its slots and which parts fit them, its hand pose, how it is held, used, carried, stowed and blocked with, and its animation set. IK takes up a longer or shorter item and a bigger or smaller body. A kind is decided once and covers every item of it.

### Clothes and armour

The same idea by body slot, with what we already do for fitting (D128): pieces made on the base body with the shared skeleton, refitted to each body shape, the skin under them hidden; variety from which piece a slot takes and colour by role.

## Answers

From Terra's goals, the prior art and what fails least, researched in [2026-10-28-assembled-equipment.md](../research/2026-10-28-assembled-equipment.md). The developers approve or change them.

1. **What is assembled:** every ordinary weapon and tool. Hand-made uniques stay whole, but declare the same named points and pass the same checks. (High confidence.)
2. **The parts:** two structural parts a kind, the working part (head, blade) and the handle, plus at most two trims (binding, pommel). Handles, shafts and strings are made by code; the head or blade sets what the item is and how it looks, as Veloren does. (High.)
3. **What picks the parts:** the theme (the maker's people and realm) gives weighted lists per slot, as Borderlands does; the material is the finish and tier; a seed from the item's identity picks. 20 to 40 looks a kind, the count printed by the generator (D182). (Medium-high.)
4. **Do parts change play:** yes, through physics only. Mass, balance and length come from the parts' shapes and materials and feed the existing tempo and reach rule (D459); shape gives the damage type, material the tier, and no other number sits on a part. Past the class's limits it is another class. (Medium: it must be played.)
5. **Does the player choose:** yes, at a station, from parts learned by finding them (D92, D86), with a preview; the allowed pairings are a list per kind, its sheet approved once. (Medium.)
6. **Separate wear:** no; wear is on the whole item (D99). (High.)
7. **One size:** yes, one real size; hands, arms and IK adapt (D128, D546). (High.)

Two questions the research adds for the developers: whether a handle can change an item's class (Veloren's long haft makes a hatchet a two-handed axe), and whether items can be taken apart to learn or reuse their parts.

## Through the animations

The assembled item meets a clip at one point, its grip; the parts are kept inside what the class's clips allow. No shipped game fits parts to a clip.

- **Hand-driven:** a one-handed item rides the hand, as melee games do (a prop bone, `ik_hand_gun`, is the shooters' answer for two-handed guns). A two-handed item rides the lead hand; the off hand is pulled by IK onto its second grip.
- **One reference item per class:** the item the class's clips are generated for (D459's reference), with an envelope round it (length, head mass, balance) from D459's clamp and D546's ranges. The assembler refuses an item outside its class's envelope before anything is drawn.
- **The clip is made right for the item before it is generated, not cleaned after:** the hand's position and turn are keyed to Kimodo for the class's reference item, so its edge leads the strike and the wrist stays in human range; afterwards only a check and at most about 15° of correction ([feasibility](../research/2026-10-28-assembled-items-animation.md): the keyed takes snap the wrist 39–90° in one key and bend it past human range today, which cleaning cannot fix). Each clip gets an item track baked from the hand, which the checks read and which is the server's hit track (one per take per body, at 120 Hz, the item's own length applied).
- **A two-handed class needs its own takes, keyed with both hands;** IK then corrects only a few centimetres (reach at most 0.95 of the arm).
- **The envelope is narrow:** D459's tempo clamp leaves about ±10% of length at a fixed head mass, so a kind's looks come from shape and material, not size; a distinctly bigger weapon is another kind with its own clips, made only when wanted. A technical limit, decided by the agent (the developers: "im not sure on the envelope as i dont know technical design for this kind of system"); revisited when the first kind's sheet is seen.
- **Checked over a grid of the envelope** (five lengths by three head masses), not only its ends: an axe failed in the middle of its range and passed at both ends. On every body, with wrist and edge-leads rows. At run time only the grip, wrist and IK already in the game; no collision avoidance.

### Feasibility

Researched 2026-10-28 in three notes: [parts as art](../research/2026-10-28-assembled-parts-art.md), [through the animations](../research/2026-10-28-assembled-items-animation.md), [engine and pipeline](../research/2026-10-28-assembled-items-pipeline.md).

- **Proven or low risk:** joins hidden by a head's eye or a trim; a handle's line and thickness found by a tool (within 0.5°); parts drawn as separate shared meshes, batched (never one merged mesh per item); items stored as recipes and built by a pure Rust assembler, the same bytes every time; one-handed items riding the hand across a ±10% envelope; the full grid in about 2½ minutes on 16 cores.
- **Needs a decision or a proof:** parts from separate Tripo runs looking like one tool (the deciding art risk; an approved style reference image first); a head's eye, which a tool cannot find from shape (2.6–3.2 cm off on axe and pick), so it comes from how the head is made or is marked once by hand.
- **Icons** (the developers, 2026-10-28: "icons can use credits thats fine and we probably wont have a unique icon for every unique combination of parts"): painted with credits, one a look family (a head or blade style in its material), shared by every combination using it.
- **Must be fixed first, in the fight clips themselves:** wrist snaps and out-of-range bends, and the edge that does not lead, in today's keyed takes; a wrist row on the composed pose fails on main with the sword already. And the game's hit sweep uses the sword's fixed length for every item and steps at the frame rate.
- **Content cost:** only the sword-and-shield move set exists; every other class needs its clips, two-handed ones keyed with both hands.
- **Check fidelity:** the move check poses in TypeScript copies of the game's pose layers, only partly tested against the Rust; they must be one path or fully tested, or a check passes what the game draws wrong.

The research also sets: a part is never scaled (resized blades float off their guards in Bannerlord); a join along a handle is a position and a diameter, not a free point, full points only for a shield's boss and rim and a bow's nocks; the grip, second grip and edge come from the handle's and head's own points (Bannerlord's custom swords end up held by the guard); the grip (D562) takes a code-made handle's radius and line from its numbers, not its mesh; and an item is stored as a recipe (part ids, material, seed), its look and icon derived from it and cached by its hash.

## How we know it renders right

By checking parts and kinds, never items, and on what the game draws.

1. **Each part, when made or changed** (no rendering): its points where its template says (a grip on the handle's centreline within a millimetre), its size for its slot, its budgets and material roles; and for every pairing it allows, a clean join: no gap, nothing buried in its partner, no two surfaces in one plane.
2. **Each kind, every pairing, every body, every animation** (measured): the fist wraps the handle, the off hand meets its grip, the edge leads the swing, nothing goes into the body held, used, carried, stowed or blocking. Measured on the pose the game shows: the checks and the game share one pose path (`terra_walker::pose`), held by a test. The set is finite (parts times pairings times bodies times clips), run only for what a change touches, on the cached build (D563).
3. **Renders against the last approved** (the game's own renderer): a sheet a kind of its pairings held, carried and stowed; a tool flags the plain faults (missing, black, wrong size, out of frame) and, after a change, only the tiles that changed.
4. **The developers' eye, once a kind**: the sheet when a part or rule changes, changed tiles marked, beside the art-style checklist; an approval is kept, so the next sheet shows only differences.

What no check catches is a correct but ugly combination; that is what the per-kind sheet and a small, deliberately designed parts vocabulary are for.

## Tools

Each one command, a line a stage, every failure naming the number, the bar and the field to change (D388, D556).

- **Part checker**: a part against its template and its points.
- **Assembler**: parts and a rule in, the item and its exact points out; the same input gives the same bytes.
- **Kinds table**: the kinds as data, every situation filled; a blank is an error.
- **The grid check**: layer 2 above, incremental.
- **Kind sheets**: layer 3, with the approved set kept.

## What happens to today's items

The thirteen held looks become assemblies where their kind has parts, or stay as checked uniques. The attachment contract (D546, [attachment.md](attachment.md)), the fitter (D552), the grip (D562), the carry rule and hand (D564) and the shield's targets (D554) stay: they are the kinds' half of this design.

## Order of work, once decided

1. The connection-point convention, the kinds table and each class's reference item and envelope, as data, reviewed.
2. The part checker and the assembler.
3. The fight clips made right first: a wrist row (flex at most 80°, extension 70°, deviation 20/35°, at most 45° a key) and an edge-leads row on the composed pose, and the keyed takes remade until they pass; the move check's pose made one path with the game's, or tested against it in full.
4. The proof, for one class (one-handed axe): its clips keyed for the reference axe, then three heads and three handles over the envelope's grid on every body, measured on the hand on the grip, the wrist, the edge leading and the body clear, and on one sheet; the parts made from one approved style reference. If it holds, the approach holds.
5. The grid check and kind sheets, then the other kinds one at a time, each with its clips.
