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

## The connection-point convention

Technical, by the agent (D566, step 1 of the order of work), for review. It sits here, not in [attachment.md](attachment.md): attachment.md (D546) is how a finished look goes on a body; this is how parts make that look, and it ends where attachment.md begins, with the item's grip at its origin and its axes its mount's, so every check of a look reads an assembled item unchanged.

- **Axes**: glTF's (D303), metres, +Y up, front +Z, +X the model's left. **Each part is made in the frame it has in the finished item**, facing as attachment.md's mount says (in hand: a one-sided head toward -Z, a blade's flats ±X; strapped: the board in X-Y, its front -Z), so no part is turned when assembled.
- **One line**: every item has one line, its +Y: a handle's axis, a blade's tang, a shield's grip along the forearm, a bow's riser. Handles, structural joins and ring trims lie on it, so **a join along a handle is a position and a diameter** (`{name, type, y, size}`, the point at (0, y, 0) facing +Y), and assembling them is a shift along Y.
- **A connection point** is a name, a position (m, the part's frame), a direction, a type and a size (m: a diameter; [width, thickness] for a flat tang). Types: `eye` (a hole a haft passes through), `socket` (a tube a haft's end enters), `tang` (a blade's flat tang), `bore` (a trim's hole round the line), `seat` (where a full-point trim sits), `nock` (a string's end); working points `grip`, `edge`, `tip`, `face`, `flame`. Parts join only where types pair (`eye`, `socket` and `bore` with a handle's section; `tang` with a blade's handle and a guard's slot; `seat` with its board point) and sizes match.
- **Full points**, a position, a direction and an up (the roll), brought onto the partner's by one rigid transform, **only for a shield's boss and rim and a bow's nocks**: what sits off the line.
- **Parts are never scaled**: a part is placed by a shift along the line or by one rigid transform, nothing else; a bigger head is another part (Bannerlord's resized blades float off their guards). A code-made handle is made at its length, not stretched.
- **Exact by making, not tolerance**: a code-made handle takes the diameter at each join from its partner's point, linearly between them, so it fits exactly; model meets model (a guard on a blade's shoulder) within 1 mm, the part checker's bar. A haft stands 4 mm proud of its head's `eye.upper` so its end never lies in the head's top face (no two surfaces in one plane); in a `socket` it ends at the socket's bottom, unseen. A trim covers a join by at least 1 cm (a real collar's 1 to 2 cm, [parts as art](../research/2026-10-28-assembled-parts-art.md) section 3).

| Part type | Made by | Its frame's origin | Its points |
|---|---|---|---|
| `haft` (axe, hammer, pick, torch, spear) | code | the butt's centre, +Y to its top | `butt`, `top`; its section a diameter at each join and the grip |
| `blade_handle` (sword, knife) | code | the tang's end, +Y to the shoulder | its section round the tang, from under the guard to the pommel |
| `axe_head`, `hammer_head`, `pick_head` | model | the centre of the eye's lower opening, +Y up the eye | `eye.lower`, `eye.upper`; `edge.lower`, `edge.upper` (the bit's span on -Z), or `face`, or `tip` |
| `spear_head`, `torch_head` | model | the socket's mouth, +Y to the tip | `socket.mouth`, `socket.bottom`; `tip` and `edge.*` (±Z), or `flame` |
| `sword_blade`, `knife_blade` | model | the shoulder's centre (where a guard sits), +Y to the tip | `shoulder`, `tang.end` (tangs); `edge.lower`, `edge.upper` (±Z, or -Z for one edge), `tip` |
| `guard`, `pommel`, `collar`, `cap` | model | its bore's centre, +Y the line | `bore` (its size the line's there), and how far along Y it covers |
| `round_board`, `heater_board` | model | the grip, where the fist closes (the strapped mount's origin) | `boss`, `rim` (full); `arm_strap` (a heater's, on the line) |
| `boss`, `rim` | model | its `seat`, -Z out of the front | `seat` (full) |
| `grip_bar`, `enarmes` | code | the board's origin, +Y | a length and a diameter (or a strap's width) |
| `bow_stave` | model | the grip on the riser, +Y to the upper nock, the string toward +Z | `nock.upper`, `nock.lower` (full) |
| `bow_string` | code | between the nocks | a diameter |
| `basket_body` | model | its grip, 1.5 cm under the handle's top (the hung mount) | `handle_top`; `mouth` (full, for a cover) |

**The item's points come from its parts'**, never from its mesh:

- **Grip**: the handle's point at its kind's rule (a haft's `grip_from_end` up from its butt; a blade's handle's middle between guard and pommel; a polearm's share of its length; a board's, stave's or basket's own origin). The assembler shifts the whole item so the grip is its origin (D546), and writes the handle's line and diameter there for the grip (D562), which then reads numbers, not the mesh.
- **Second grip**: a point on the handle at its kind's fixed distance from the grip, part of the reference item, not the envelope ([animation note](../research/2026-10-28-assembled-items-animation.md) section 3), written as the `second` extra; a bow's `nock` is the midpoint of its two nocks, the string straight at rest.
- **Edge**: the working part's `edge.*`, `tip` or `face` carried through its placement: a span along +Y and its side (-Z for a one-sided head, ±Z for two edges). That span is the item's hit segment on its item track (the same note, section 4), in place of one sword's length for every item.
- **Mass**: each part's data gives its mass and centre of mass (the part checker writes a model's; a handle's from its volume and wood), so the item's mass, balance and inertia about the grip are its parts'. The kinds' envelopes use D459's rule, I = m (b² + L²/12), on the reference's numbers, and the assembler applies the same rule to the item.

### The kinds table

One file a kind, `assets/data/kinds/<kind>.json`, with its schema `kind.schema.json` beside it, which refuses a blank: every situation is a fact with its source, a mechanism that exists, or "open" with its owner (the developers' for design, technical for an agent's) and why. A kind here is one a move-set class (a reference item and its clips each), finer than D546's kind, which each file names (`attach_kind`). It lives in the asset root, not `art/`, because the assembler reads it at run time, in play and on the server, to refuse a recipe outside its kind ([pipeline note](../research/2026-10-28-assembled-items-pipeline.md), "In play"); `art/` holds data only the tool that makes a look reads. JSON, not RON like its neighbours, so the TypeScript checks read it with no parser and a JSON Schema holds it (the game's crates already use `serde_json`). Its readers: the kind check and the part checker (`tools/items/kind.ts`, `part.ts`) and the Rust assembler (`terra_items::assembly`, step 2). The part types' templates (who makes each, its slot, its points, how it is placed, its budgets and roles) and the join rules' numbers are data beside the parts, `assets/data/parts/types.json`, read by all three; a part's own data is `assets/data/parts/<part>.json` (`part.schema.json`).

The reference items' masses are stand-ins (recalled real tools, or `items.ron`'s), each with its source, replaced by the parts' weighed masses when the reference is made; the envelope is D459's clamp (the inertia within I_ref/1.15² to I_ref/0.85²) seen one number at a time: length ±10% (measured), head mass about -25% to +40%, balance solved, each at the reference's others.

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

- **Part checker**: a part against its template and its points (`node tools/items/part.ts <part>`, [its README](../tools/items/README.md)).
- **Assembler**: parts and a rule in, the item and its exact points out; the same input gives the same bytes (`terra_items::assembly::assemble`, a recipe of kind, part ids, material and seed; one placement a part, never merged). Built for the line kinds (a haft through an eye or into a socket, trims under the head and over the butt); blades, boards and staves next. Not used by the game yet: that waits for step 4's proof.
- **Kinds table**: the kinds as data, every situation filled; a blank or an unsourced number is an error (`assets/data/kinds/`, held by `node tools/items/kind.ts` in `cargo xtask check`'s tools tests).
- **The grid check**: layer 2 above, incremental.
- **Kind sheets**: layer 3, with the approved set kept.

## What happens to today's items

The thirteen held looks become assemblies where their kind has parts, or stay as checked uniques. The attachment contract (D546, [attachment.md](attachment.md)), the fitter (D552), the grip (D562), the carry rule and hand (D564) and the shield's targets (D554) stay: they are the kinds' half of this design.

## Order of work, once decided

1. The connection-point convention, the kinds table and each class's reference item and envelope, as data, reviewed. Written 2026-10-28 (above and `assets/data/kinds/`), for review.
2. The part checker and the assembler. Built 2026-10-28 and proven on made-up parts (the kind check, `part.ts`, `terra_items::assembly`); the assembler places the line kinds so far.
3. The fight clips made right first: a wrist row (flex at most 80°, extension 70°, deviation 20/35°, at most 45° a key) and an edge-leads row on the composed pose, and the keyed takes remade until they pass; the move check's pose made one path with the game's, or tested against it in full.
4. The proof, for one class (one-handed axe): its clips keyed for the reference axe, then three heads and three handles over the envelope's grid on every body, measured on the hand on the grip, the wrist, the edge leading and the body clear, and on one sheet; the parts made from one approved style reference. If it holds, the approach holds.
5. The grid check and kind sheets, then the other kinds one at a time, each with its clips.

## The kinds, in plain words

The first eleven kinds, for the developers' review: the full detail is in `assets/data/kinds/`. "Hip" and "back" are where it hangs when put away (D551); the exact angle there is not set for any of them yet. The developers answered the six questions it raised (D567, "Approve all six"); what is left is technical.

| Kind | What it is | Its parts | How it's held and used | How it's carried and put away | What exists today | What's open |
|---|---|---|---|---|---|---|
| Axe | one-handed axe, a tool and later a weapon | head, handle; a binding, a butt cap | one hand; swung with the shared tool swing | hangs from the hand, head down; hip, hand's side | the stone and iron axes, as tools | its own swing and guard (it hits the head in the sword's); blocking alone, weaker than a shield (D567) |
| Sword | one-handed sword | blade, grip; guard, pommel | one hand; the sword-and-shield fight | hangs from the hand, tip down; hip, other side | the arming sword and its whole fight | the sword alone (no shield) has no moves yet, its block alone among them (weaker than a shield, D567) |
| Hammer | builder's hammer | head, handle; a binding | one hand; builds | hangs from the hand; hip, hand's side | the hammer, as a tool | a real hammering motion for the player |
| Pickaxe | pick for rock and ore | head, handle; a binding, a butt cap | two hands (D567), one hand today; breaks rock | hangs from the hand; back (D567) | the stone and iron pickaxes | its two-handed swing and second grip |
| Knife | small cutting tool | blade, handle; a bolster, a butt | one hand; cuts plants | hangs from the hand; hip, other side | the knife, as a tool | its own cutting motion |
| Spear | long spear | head, shaft; a binding, a butt spike | two hands, or one with a shield; today swung one-handed as a tool | upright at the side in the lead hand, point up (D567; today tipped head down); back | the spear, as a tool | the two-handed hold and moves; the upright carry |
| Torch | burning torch | head, stick; a binding | either hand, for light; a poor club | held up, flame high, in either hand; put out and hung at the hip (D567) | built: carried raised, lit | putting it out when put away |
| Round shield | round planked shield, iron boss | board, grip bar; boss, rim | off hand, by a grip behind the boss; blocks | on the forearm at the side; back | the look and its carry pose; no item yet | checking the guard and block positions with this board (they were set with the heater) |
| Heater shield | pointed shield on two forearm straps | board, straps; rim | off hand, on the forearm; blocks | on the forearm; back | the fight's shield, guard and block | the shield bash and fighting with a shield alone |
| Bow | wooden bow | stave, string; a grip wrap | off hand holds, other hand draws; shoots | low in the left hand, top tipped forward (D567; today tipped like a tool); back | the bow shoots as a tool, with no drawing motion | the draw and its moves; the low carry |
| Basket | basket hung from the hand | body with its handle, no lid (D567) | the off hand, hanging by its handle (D567); carries things | hangs at the side; put away into the pack, not shown (D567) | the look only | the item, and hanging it from the off hand |
