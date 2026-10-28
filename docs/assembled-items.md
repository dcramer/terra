# Assembled items: equipment made from parts by rules

Status: proposal (D565, Open). The developers decide the design questions below; nothing is built on it until they have.

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

## Questions for the developers

Each with the agent's recommendation.

1. **What is assembled?** Recommended: every ordinary weapon and tool. Unique and legendary items stay whole, hand-made, and checked once by `fit.ts`.
2. **The parts of each kind.** Recommended, three or four slots a kind: axe (handle, head, binding); sword (blade, guard, grip, pommel); spear (shaft, head, butt cap); hammer and pick (handle, head); shield (board, boss, rim, painted face); bow (stave, grip wrap, string).
3. **What picks the parts?** Recommended: the maker's people and realm pick the style family, the material (stone, bronze, iron, steel) the finish; 20 to 40 looks a kind a player tells apart (D182).
4. **Do parts change how an item plays?** Recommended: yes, visibly. A longer handle reaches further, a heavier head swings slower and hits harder, a wide blade cuts and a narrow one thrusts (D122). The alternative: parts are looks, numbers come from the tier.
5. **Does the player choose parts?** Recommended: at a station, from parts known and affordable, with a preview; found and traded items come assembled in their makers' style.
6. **Do parts wear separately?** Recommended: not yet; wear shows on the whole item.
7. **One size for every body?** Recommended: an item has one real size; hands and arms adapt. A giant's axe is a different item.

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

1. A research note confirming the prior art above.
2. The connection-point convention and the kinds table, as data, reviewed.
3. The part checker and the assembler.
4. A proof: three handles and three heads of one kind assembled by rule, held and carried on every body with no number per item, on one sheet with the grid check's results. If the nine come out right, the approach holds.
5. The grid check and kind sheets, then the other kinds one at a time.
