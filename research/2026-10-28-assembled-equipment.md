# Assembled equipment: how shipped games build weapons and tools from parts, and the answers to D565's seven questions

2026-10-28, characters and items. Question: D565 ([docs/assembled-items.md](../docs/assembled-items.md)) proposes that weapons and tools be put together from parts joined at named connection points, and asks the developers seven questions. The developer: "I dont have answers to these as im not a game developer. We need to look at our goals, the state of the art approach, and the way we'd do this most reliably so it will not be buggy". So, for each question: what our decided goals say, what shipped games do, and which answer has the least to get right by hand.

*Read* marks a claim checked in a source this session, *recalled* one from memory, not re-checked.

## Answer

Every shipped game that builds weapons from parts does four things:

- **A few parts a kind.** Two (Veloren), four for a sword (Mount & Blade II).
- **One part sets what the item is.** The head or blade gives its name and look; the handle sets its length and how many hands hold it.
- **Parts come from lists the item's kind and maker allow, never everything with everything.** Borderlands has weighted lists per maker; Call of Duty has families of guns that share attachments.
- **Hand-made uniques sit beside the assembled items.** Borderlands' legendaries, Path of Exile's uniques, Bannerlord's non-crafted items.

Where these systems break is known:

- **The joins.** Bannerlord's blades float off their guards when a blade is resized ([Steam, 2022](https://steamcommunity.com/app/261550/discussions/0/3493131640458455702), *read*).
- **The grip.** Bannerlord's custom swords are held by the guard ([Steam, 2020](https://steamcommunity.com/app/261550/discussions/0/2279330983544366958), *read*).

No Man's Sky avoids join faults entirely. Every variant is modelled already in place on one base scene, and the generator only picks one of each group.

The developer's main worry is that parts will not attach correctly for the animations. The procedural games never fit parts to an animation. The assembled item meets the clip at one point, its grip, and the parts are kept inside what the class's clips allow. The section below says how, with a recommendation for each step.

So D565's design holds, with five changes the prior art argues for:

1. **Never scale a part.** A longer handle is another handle, made by code.
2. **Joins along the handle are one number and a diameter**, not a free point in space.
3. **The grip, the second grip and the edge come from the handle's and head's own points**, never from the item's shape.
4. **Mass and balance are computed from the parts' geometry and material.** They feed the tempo and reach rule we already have (D459), so parts change play without a number set per item.
5. **An item is stored as its recipe** (part ids, material, seed), as a Borderlands serial is. The look is derived from the recipe and cached by its hash (D552).

## Making items move right through animation

The developer: "the biggest issues im seeing here are our inability to get the parts to attach in a correct way for the animations ... idk how the procedural folks do it either".

**How the procedural games do it: they never attach parts to an animation.** The parts are joined into one item, and only the item meets the animation, at one point, its grip, on one hand. The parts are kept inside what the class's clips allow:
- **Borderlands:** a body per gun type, and its parts only from that type's lists (*read* lists; *recalled* bodies).
- **Veloren:** three haft lengths a kind (*read*).
- **No Man's Sky:** parts modelled in place on a fixed base (*read*).
- **Bannerlord:** templates per weapon type (*read*).

So the problem with parts is the problem with whole items, plus the rule that keeps every assembly inside its class's envelope. Assembly makes the grip exact. It does not make the clips right for every item; the envelope and the checks do.

| Question | What shipped games do | Recommendation | Confidence |
|---|---|---|---|
| **Prop bone or hand bone?** | **Shooters drive by the prop.** Unreal's mannequin carries `ik_hand_gun`, "located at the right hand, and parent of" the two hand IK bones. The weapon is attached to it, so the clip animates the gun, and the hands keep their place on it "regardless of limb length" on a retargeted body (*read*, Epic's IK Retarget Weapon Goals; Rifle AnimSet Pro forum). A mocap sword pack ships "motions of the weapon bone of both hands" and warns they are lost without those bones (*read*, Unreal Marketplace). **Melee and RPGs drive by the hand.** The weapon rides a node on the hand: Skyrim's `WEAPON`, Valheim's `attach` placed "based on the item type" (*read*), FromSoftware's weapons (*recalled*), Lyra's `AttachSocket` (*read*, attachment.md). A two-handed melee weapon then pulls the off hand on by IK. When "the animation was for a different sized weapon", the fix in Unreal's forums is IK switched by a curve in the clip (*read*) | **Hand-driven, as today, for every one-hander.** For two-handers (D461, when built), the lead hand drives and the off hand is IK'd to the item's `second` point (holding.md). A runtime prop bone, as `ik_hand_gun`, earns its cost only for reloads, throws or a hand change mid-move: revisit then | high for one-handers, medium for two-handers |
| **A reference prop per class?** | Animators swing "real props, or objects of similar weight", and heavier blades get "extra drag and delay" (*read*, Animation Mentor). FromSoftware keeps one move set a class, and the hit volumes come from the weapon model's own points. A swapped model therefore hits wider or narrower in the same clip: DS3 model-swap mods warn that the swapped weapon brings its own hitbox size, an unfair advantage in PvP (*read* in search summaries of Nexus pages, which refused a direct fetch; DSAnimStudio takes hitboxes from "the right weapon"). No source gives a tolerance on how far an item may differ from the class's prop | **Each class has a reference item, written down**: its grip, length, mass, balance and `second`. D459 already names `I_ref`; this makes it the item the clips are generated for (D450's hand keys at grip points are written for it). Real items stay inside an envelope round it: tempo within D459's 0.85–1.15 clamp, length within the kind's life size (D546), and a working end that leads the strike. Hit volumes come from the item, as FromSoftware's do (holding.md: reach is the item's own length) | high on the reference item; medium on the envelope's numbers, which are measured, not found |
| **Keeping generated items inside the class** | Parts lists per gun type (Borderlands), three fixed hafts (Veloren), one base per variant group (No Man's Sky), templates (Bannerlord) (*read*). None publishes a numeric envelope | **The assembler refuses an assembly outside its class's envelope**, computed from the parts before anything is drawn. Its length, mass and balance give its tempo, which must be inside the clamp, or the item is named another class. Its points must be inside the kind's ranges (D546). An allowed pairing list is kept per kind (Q5) | high |
| **Checking the fit of clip and item** | Mostly nothing at run time beyond the off hand's IK. Weapons pass through bodies in shipped melee games, and nobody is found avoiding it at run time (*recalled*). The checks are by eye. The hitbox mistakes are found by players (*read*: DS2's ported moves "did not do damage" until hitbox rows were added) | **Offline, across the envelope, on every body**: holding.md's move check already runs "with the class's shortest and longest items". Add the two extremes of each envelope as items of their own. Add the check attachment.md's "Under animation" asks for: the working face leads at speed, -Z at least 0.7 of the motion. At run time keep only what we have: the grip (D562), the wrist (D564), the arms off the hips, and IK in centimetres. No collision avoidance on items | high |
| **Clips generated without a prop** (Kimodo, Uthana) | Research models make the object's motion with the body's: CHOIS from text and object waypoints, OMOMO the body from the object's motion through hand positions, HOI-Diff with contact guidance (*read*, abstracts). Kimodo and Uthana make neither. Our D450 already keys the right hand's end effector to grip points, so a take holds its pose where the reference item would be | **Bake an item track per clip**: the hand socket's frame on every frame, in the body's space, taken from the take and carrying the reference item. Then clean it in two steps. First, smooth the handle's turn about its own axis, so the generator's wrist jitter does not wobble a long item's tip. Second, turn the edge to lead where the check says it should. Then solve the wrist back onto the cleaned track (D564's wrist layer is the same kind of fix). Store the track beside the clip. The checks read the track and not the raw hand: edge leads, tip path, clearance. A two-hander's off hand is IK'd to `second` on it. This is the prop bone's benefit, an item path you can check and correct, without a bone in every skeleton | medium: no shipped game was found doing this for generated clips; it is the shooter pattern rebuilt offline |

## Terra's goals that bear on it

| Decision | What it asks of equipment |
|---|---|
| D182 (Decided) | Things from parts by rules, as Borderlands makes guns; a theme (realm, maker, people) picks which parts; hundreds of variants a player tells apart; the generator says how many and what tells them apart |
| D122, D93 (Decided) | Things work as in the world, so a player can reason about them by looking: a longer haft reaches further because it is longer |
| D187 (Decided) | Intricate systems for each component |
| D128 (Leaning), D546 (Decided) | Every item fits every body by contract. Held things are made once to the hand socket's axes; a kind is solved once; a look of the wrong size for its kind is refused |
| D552, D562, D564 (Decided) | One command fits an item. The fingers close on the handle read from the mesh, and a carried tool tips at the wrist. These become the kinds' half of D565 |
| D459 (Leaning, holding.md) | Tempo is `sqrt(I_ref/I)` of the item's moment of inertia about the grip, clamped to 0.85–1.15. Reach comes from length. An item past the clamp is another class |
| D99 (Decided), crafting.md | Items wear as a whole and are repaired cheaply at a station. A tool's material decides what it can work (the Valheim ladder). You learn to make a thing by finding it (D92), from people and cultures (D86) and by skill (D88) |

## The prior art

| Game or engine | What is assembled, and how parts join | Hands and animation | Parts in play; who picks; how many looks | Known faults and limits |
|---|---|---|---|---|
| **Borderlands 2 to 4** | Guns assembled from parts: body, barrel, grip, stock, sight, accessory, element. A weapon's balance definition holds a weighted list for each part slot, each entry gated by maker and game stage (*read*, BLCM wiki). In BL4, "licensed parts" put one maker's behaviour on another's gun; up to 3 slots, more with rarity (*read*). The parts snap to points on the body, which is made per maker and gun type (*recalled*) | Animations by gun type (*recalled*) | Parts change stats: the barrel sets accuracy and the magazine capacity (*read*, fan guides). Parts are random drops, never chosen; BL4 adds no part swapping (*read*). Marketed as "30 billion" guns (*read*), but players tell them apart by maker, type and named legendaries, which have unique parts (*recalled*) | No source found on clipping |
| **No Man's Sky** | Every part variant is modelled in place in one scene. A descriptor file lists groups (`_ROOF_1`, `_WALLS_2`), and the game "picks only one … node, along with its children, using the specified chance", seeded per planet (*read*, Step wiki). Multitools are made the same way (*recalled*) | One set of animations per multitool class (*recalled*) | Parts are looks; stats come from class and rarity. Players buy or find multitools whole (*recalled*) | No join can be wrong, because nothing is joined at run time. In exchange, a part fits only the base it was modelled on (*inferred*) |
| **Veloren** (GPL: only its data files and docs read) | Two parts. The **primary** (axe head, hammer head, sword blade, bow limbs, staff shaft) has one model per material: `component/axe/axe/{bronze,iron,steel,…}.vox`. The **secondary** (haft, hilt, grip) comes in `long`, `medium` and `short` (*read*, asset tree). The design note: "a single component determines the weapon's visual, to avoid inconsistent appearance" (*read*) | One hold and one set of animations per tool kind (*recalled*; the code was not read) | Stats are the parts' multipliers, so a long haft gives power ×1.15 and speed ×0.8. `hand_restriction: Some(Two)` makes the weapon two-handed, and the name follows: "Hatchet" one-handed, "Axe" two-handed (*read*, `axe.ron`, `long.ron`). The player picks both parts and an animal material at a station (*read*, wiki) | The visual comes from one part, so the other part can change only stats, not looks (*read*) |
| **Mount & Blade II: Bannerlord** | Templates of slots: Blade, Guard, Handle, Pommel (*read*, modding docs). Pieces stack along the weapon's axis by numbers set per piece: `length`, `distance_to_next_piece`, `distance_to_previous_piece`, and a `BuildData` with `piece_offset`, `next_piece_offset`, `previous_piece_offset`. Optional `scale_factor`; per-piece `holster_mesh`; `culture` (*read*, a mod's `crafting_pieces.xml`) | Animations by item usage, set by template (*recalled*) | Physics: "swing speed is affected not only by weight but also by the distribution of weight around the pivot", and the game simulates each crafted weapon's swing and thrust (*read*, dev blog via GameBanshee). The player picks from pieces unlocked by smithing (*recalled*); pieces are tagged by culture | Blades float off guards when resized. Custom swords are gripped by the guard, which clips through the hand (*read*, both Steam threads). Players hunt for the best length-to-speed ratio (*read*). A new piece cannot be added to a template without overriding the whole template (*read*, Nexus forum) |
| **Call of Duty MW (2019), MWII** | A weapon has its own attachment set, some shared with other weapons (*read*). MWII's "platforms" are families of guns that share attachments (*read*) | By weapon (*recalled*) | Attachments change handling; the player picks them in the Gunsmith (*read*) | Compatibility is a list per weapon or family, never universal (*read*) |
| **Escape from Tarkov** | Each slot of a weapon lists the item ids it accepts (*recalled*, not confirmed) | — | Parts change ergonomics and recoil; the player picks them freely (*recalled*) | — |
| **Valheim** | Whole prefabs. Each holds an `attach` child, which the game places "based on the item type"; a complex item is skinned to the avatar (`attach_skin`) instead (*read*, modding wiki) | By item type (*read*) | Upgrades change stats, not looks (*recalled*) | — |
| **Enshrouded** | Whole items | By class (loadouts note) | Upgraded by runes to "LEVEL n/3" (*read*). Players ask for upgrades to change how weapons look ("Visual Changes for High-End Weaponry", feature board, *read*) | No visible progression on one item |
| **Kenshi** | One model per weapon type (*recalled*) | By type (*recalled*) | Grade and maker scale damage and weight, from Rusted Junk 0.46 cut to Meitou 2.64 on the nodachi (*read*, pindrop) | No source on whether grades look different |
| **Skyrim (Creation Engine)** | Whole models. The weapon's `Prn` names its stowed node (*read*, attachment.md) | By weapon type (*recalled*) | Smithing improves stats, not looks (*recalled*) | Without the node, the weapon "will likely show up at your feet" (*read*) |
| **Elden Ring** | Whole weapons | One move set per class, shared (loadouts note) | An affinity changes scaling and damage type; an Ash of War adds one skill (*read*) | — |
| **Diablo II** | Whole base types. Each has a three-letter graphic code (`alternategfx`) and a layer of the composed sprite (`component`: right hand, left hand, shield) (*read*) | Animations and their lengths by weapon class (1HS, 1HT, 2HT, STF…) (*read*) | Affixes change stats, never the look (*recalled*) | — |
| **Path of Exile** | Whole base types. Several bases share one piece of art (*read*); uniques have distinct art of their own (*read*, PoE2 wiki) | — | The base sets the implicit stats; rarity rolls before the base (*read*) | — |
| **Blade & Sorcery** (physics VR) | Whole items with Handle components. A handle has an axis length to slide along and a set of allowed orientations. A support handle is grabbable only once the main one is held (*read*) | A hand pose per handle, tuned with a gizmo; several orientations "can need many separate pose entries" (*read*) | — | The poses multiply with orientations: authored per handle, not per kind |
| **Unreal: sockets, Lyra, Mutable, Control Rig** | Sockets on the skeleton and on the weapon (attachment.md). Mutable merges skeletal meshes at run time, but a third-party kit notes it "doesn't support arbitrary skeletons" (*read*) | Lyra links an animation layer per weapon type. IK puts the off hand on the weapon after the pose (loadouts note, *read*). Control Rig is the IK tool (*recalled*) | — | — |
| **Unity: Animation Rigging** | — | Two Bone IK with a target and a hint; Multi-Parent hands an object between parents (attachment.md, *read*) | — | — |
| **Papers and tools** | Volcano (Loiacono, Mainetti, Pirovano, IEEE GEM 2015) evolves 3D swords interactively (*read*, abstract). Houdini and Unity sword generators sell parameters for blades and hilts (*read*) | — | — | No paper on making attachments reliable was found |

## Reliability of each way

| Way | Set by hand | Checked automatically | What failed for others |
|---|---|---|---|
| **A. Whole models fitted per item** (today) | A turn, grip, scale and carry per item: 13 looks so far, and thousands later | D546's contract, D552's measures (38 of 41 broken copies caught) | Our own audit: 8 of 13 looks 90° off, 6 off their grip by 2.5–4.7 cm (D546). Every new item repeats the risk |
| **B. Variants modelled in place on a base** (No Man's Sky) | Every variant is modelled on its base, so the joins are right by modelling | Little to check | A part fits one base only, so variety is a product of bases. It cannot carry D459's physics, because lengths are fixed per base |
| **C. Parts at named connection points** (D565; Bannerlord, Veloren, Borderlands) | Each part once, against its kind's template: points, size, budgets | Each part's points and size; every allowed pairing's join (gap, buried, coplanar faces); every pairing on every body and clip (D565's grid) | Bannerlord's floating blades come from scaling. Its grip faults come from a grip not tied to the handle. Its hand-set offsets are numbers per piece along one axis |
| **D. Meshes generated whole by parameters** (Houdini, Volcano) | The generator's rules | Every output can be measured. Exactness is good, but the look against D14's style is the hard part | The look of generated heads and blades: none of the shipped games above does it for heads. Handles are the exception: a cylinder with a taper is safe to generate |

C with D for the handles is the best trade. The number of things checked is parts plus pairings plus kinds, not items. Each check has a measured answer (millimetres, degrees). And the two faults Bannerlord shipped are both removable by rule:

- **No scaling.** Bannerlord's blades float when resized; a part made at one size never changes size.
- **A grip taken from the handle.** Bannerlord's swords are held by the guard; a grip that is the handle's own point cannot land there.

## The seven questions

**1. What is assembled?** Every ordinary weapon and tool is assembled. Hand-made uniques stay whole, but each declares the same named points an assembly would (grip, second grip, edge, stow). A unique is then a one-part assembly, and it passes the same per-kind checks rather than a check of its own.
- **Why:** D182 asks for parts by rules. Every parts game keeps uniques beside its parts (Borderlands' legendaries, PoE's uniques with their own art). Today's per-item fitting is what failed (D546's audit).
- **Main risk:** uniques drift into a second pipeline. **Check:** a unique without the points fails `attach_contract.ts`, as any part does.
- **Confidence:** high. Only "are there uniques at all" is design; the rest is technical.

**2. The parts of each kind.** Two structural parts a kind, the working part and the handle, plus at most two small trims at named points on the handle:
- **axe, hammer, pick:** head and handle, plus a binding or wedge;
- **sword, knife:** blade and hilt, plus guard and pommel;
- **spear:** head and shaft, plus a butt cap;
- **shield:** board plus boss and rim, with the painted face a texture layer, not a part;
- **bow:** stave plus a grip wrap, with the string made by code between the stave's two nock points.

Handles, shafts and strings are made by code.
- **Why:** each join is a place to fail, and the fewer the joins, the fewer the checks. Veloren ships two parts; Bannerlord ships four for a sword and its faults are at the joins. One part, the head or blade, sets what the item is and how it looks (Veloren), which keeps a combination from looking incoherent. D122: a real axe is a head on a haft with a wedge.
- **Main risk:** too few looks per kind. **Check:** the generator's count, required by D182, printed per kind.
- **Confidence:** high on two structural parts and code-made handles; medium on which trims.

**3. What picks the parts?** The item's theme (its maker's people and realm) gives a weighted list of the parts allowed in each slot. The material (stone, bronze, iron, steel) is the finish and the tier. A seed drawn from the item's identity makes the pick, so the server and every client get the same item. Aim for 20 to 40 looks a kind a player tells apart, counted as head or blade families times materials, with handles as a second, smaller axis.
- **Why:** D182 names Borderlands, whose weighted lists by maker and stage are exactly this. D59 and D86 make every world and culture thematic. Bannerlord tags pieces by culture. No Man's Sky seeds by place. Crafting.md says a tool's material decides what it can work.
- **Main risk:** near-duplicates a player cannot tell apart (D182). **Check:** the generator prints its count and flags any two heads of a kind whose silhouettes differ by less than a set share on the kind sheet. That bar is to be measured on the first kind.
- **Confidence:** medium-high.

**4. Do parts change how an item plays?** Yes, through physics only, and by the rules we already have. The parts' meshes and materials' densities give the item's mass, balance point and length. D459 turns those into tempo (clamped to 0.85–1.15) and reach. The blade's or head's shape gives its damage type: a wide edge cuts, a point thrusts, a heavy face crushes. The material gives the tier, meaning what it can work and the damage scale. No other stat lives on a part. A haft long enough to push the tempo past the clamp makes the item another class: hatchet against axe, as Veloren's handedness names do.
- **Why:** D122 (a longer haft reaches further because it is longer) and D93 (you learn it by looking). Bannerlord simulates swing speed from weight distribution; Veloren multiplies stats by part. Our D459 already computes tempo from inertia, so this adds no number per item.
- **Main risk:** one combination beats all others, as Bannerlord's players found with the best length-to-speed ratio. **Check:** a table test over every allowed pairing of a kind that no pairing beats another on every number at once. Every pairing must also stay inside its class's clamp, or be named another class.
- **Confidence:** medium. It is design (D412), and the balance has to be played.

**5. Does the player choose parts?** Yes, at a station, from the parts they have learned, with a preview of the item held and carried. A part is learned by finding or taking apart an item that has it (D92, D86). Found and traded items come assembled in their makers' style. A kind lists which heads go with which handles, a per-kind list as in Call of Duty's platforms, never every part with every handle.
- **Why:** D187 (intricate systems) and crafting.md (learn by finding, from cultures). Veloren and Bannerlord let the player choose; Borderlands does not, and BL4's change is still drops.
- **Main risk:** a combination that is correct but ugly. A player can make every allowed pairing, so the per-kind sheet must show them all. **Check:** the kind sheet of every allowed pairing (D565, layer 3), approved once a kind. A pairing nobody approved is not allowed.
- **Confidence:** medium. It is design, and it costs a station screen.

**6. Do parts wear separately?** No. Wear is on the whole item (D99), and a broken item is mended whole at its station. Putting a new part on an item can come later, as a repair or a remake at a station.
- **Why:** D99 and D3 (no chores). Veloren, Bannerlord and Borderlands keep no wear per part (*recalled*). Wear per part multiplies the state the server keeps per item.
- **Main risk:** none found.
- **Confidence:** high.

**7. One size for every body?** Yes. An item has one real size. Hands, arms and the off hand's IK adapt to each body. A body far outside the human range (a giant) uses items of its own kind's sizes.
- **Why:** D128 and D546 (items at their real size, sockets per body); holding.md's IK takes up a longer or shorter arm. Every game above has one model per item for all its bodies (*recalled*).
- **Main risk:** the longest part on the shortest body hits the ground or the body when carried or stowed. **Check:** D565's grid runs every kind's longest and shortest pairing on every template.
- **Confidence:** high. This is mostly settled by D128 and D546 already, so it is barely a design question.

## Wrong questions and missing ones

- **Q1 and Q7 are mostly technical** (D546, D128 already answer them). The design in Q1 is only whether hand-made uniques exist.
- **Q4 is half answered** by D459's tempo and reach rule. What is left to decide is whether parts carry any stat beyond physics, damage type and material tier. The recommendation is that they do not.
- **Missing: does a handle change an item's class?** In Veloren a long haft makes a two-handed axe out of a hatchet's head. Here D459's clamp would decide it: the class follows from the pairing by rule. This needs the developers' yes.
- **Missing: can items be taken apart to learn or reuse their parts?** This interacts with Q5's learning and with D99's repair.

## What the prior art changes in the design

- **Where hand poses come from.** Shipped games author one per grip type (Lyra, ALS) or one per handle (Blade & Sorcery), never one per item. Ours are solved at run time against the handle (D562), which needs no asset at all. With handles made by code, the grip solver can take the handle's exact radius and line from the handle's parameters instead of reading them from the mesh. That removes the one fallible step left in the grip.
- **IK against authored poses.** Keep what holding.md already says: clips authored per class, both hands keyed, IK correcting centimetres onto the handle's second point (Lyra, Unity's Two Bone IK). Assembly makes the second grip exact, because it is a point on a handle made by code.
- **Connection points.** For hafted items, blades and polearms, a join is a position along the handle's +Y and a socket diameter: one number and a size, as Bannerlord's stacking is, but never scaled. The handle's code sets the diameter at the join, so the head's eye and the haft meet exactly. A full point with a direction is needed only for a shield's boss and rim and a bow's nocks.
- **Stored as a recipe.** An item is part ids, a material and a seed. The look is derived from them and cached by their hash, which extends D552's `source_sha`. The server checks the recipe and the client builds the look.
- **A reference item per class and an envelope round it** (see "Making items move right through animation"). The clips are generated for the reference item, the assembler refuses an assembly outside the envelope, and the move check runs the envelope's two extremes.
- **An item track baked per clip**: the reference item's path taken from the take's hand, cleaned, with the wrist solved back onto it. The checks and the off hand's IK read the track, not the raw hand.
- **The icon.** Each assembled look needs its icon painted again by the item-icons pipeline, so the icon is derived from the recipe too, not hand-made.

## Limits

- Borderlands' and No Man's Sky's attachment internals, and every game's choice of animations, are recalled from memory or modding wikis. No developer talk on either system was found.
- Veloren's code, which places a head on a haft and picks the hold, was not read, by licence. Its data and design note were.
- Bannerlord's pieces were read in one mod's data file, not the game's own; the attribute names are the game's. The dev blog was read in a search summary.
- Tarkov's slot filters, Kenshi's looks per grade, and wear per part elsewhere are unconfirmed.
- Nothing was measured; the distinctness bar in Q3 is to be set on the first kind.
- No source gives a tolerance for how far an item may differ from the prop its clips were made with. The envelope's numbers are ours, from D459's clamp and D546's ranges.
- No shipped game was found baking an item track from clips generated without a prop. That recommendation rests on the shooter's prop bone and on D450's hand keys, not on precedent.
- Unreal's sources on `ik_hand_gun` are Epic's API reference and forum threads; no Epic page states the design's reasons.

## Sources

- Borderlands: [BLCM wiki, parts definitions](https://github-wiki-see.page/m/BLCM/BLCMods/wiki/Understanding-Item-and-Weapon-Parts-Definitions); [Sportskeeda, BL4 licensed parts](https://www.sportskeeda.com/esports/what-licensed-parts-system-borderlands-4); [Borderlands wiki, BL4 parts](https://borderlands.fandom.com/wiki/Borderlands_4_Parts_System); [Notebookcheck, 30 billion](https://www.notebookcheck.net/Borderlands-4-to-feature-30-billion-unique-weapons-thanks-to-procedual-generation-and-weapon-customization.1085980.0.html).
- No Man's Sky: [Step wiki, DESCRIPTOR files](https://stepmodifications.org/wiki/NoMansSky:Reference_Guides/DESCRIPTOR_Files); [GDC Vault, Continuous World Generation](https://gdcvault.com/play/1024265/Continuous-World-Generation-in-No).
- Veloren: [wiki, Modular Crafting](https://wiki.veloren.net/wiki/Modular_Crafting); [design note](https://hackmd.io/@veloren/H1UwcDpi_); data files `assets/common/items/modular/weapon/primary/axe/axe.ron`, `secondary/axe/long.ron`, `assets/voxygen/voxel/weapon/component/axe/` in [the repository](https://gitlab.com/veloren/veloren).
- Mount & Blade II: [CraftedItem docs](https://docs.bannerlordmodding.com/_xmldocs/items/CraftedItem); [Piece docs](https://docs.bannerlordmodding.com/_xmldocs/items/crafteditem/pieces/Piece); [AD1259 mod's crafting pieces](https://github.com/Knightfall-dh/AD1259/blob/main/ModuleData/OSA_crafting_pieces.xml); [dev blog, weapon physics](https://www.gamebanshee.com/k43ha); [Steam, floating parts, 2022](https://steamcommunity.com/app/261550/discussions/0/3493131640458455702); [Steam, gripped by the guard, 2020](https://steamcommunity.com/app/261550/discussions/0/2279330983544366958); [Nexus forum, templates](https://forums.nexusmods.com/profile/622054-verdigo/content/).
- Call of Duty: [Charlie Intel, MW Gunsmith](https://www.charlieintel.com/modern-warfare/call-of-duty-modern-warfare-introduces-the-gunsmith-weapon-customization-system-55552/); [Upcomer, MWII Gunsmith](https://upcomer.com/how-the-gunsmith-works-in-modern-warfare-ii-multiplayer/).
- Valheim: [modding wiki, attaching weapons](https://github-wiki-see.page/m/Valheim-Modding/Wiki/wiki/Attaching-Weapons-and-Armor).
- Enshrouded: [feature board, visual changes](https://enshrouded.featureupvote.com/suggestions/670918/comment/755617); [Gameluster, improving weapons](https://gameluster.com/enshrouded-improving-your-weapons-guide/).
- Kenshi: [pindrop, nodachi](https://pindrop.gg/kenshi/weapons/nodachi).
- Elden Ring: [wiki.gg, affinities](https://eldenring.wiki.gg/wiki/Affinities); [wiki.gg, Ash of War](https://eldenring.wiki.gg/wiki/Ash_of_War).
- Diablo II: [data file guide](https://d2runewizard.com/game-files/guide.html); [StrategyWiki, classes](https://StrategyWiki.org/wiki/Diablo_II/Classes); [MergeDCC](https://www.allegro.cc/depot/MergeDCC).
- Path of Exile: [poewiki, Sun Plate](https://www.poewiki.net/wiki/Sun_Plate); [poe2wiki, unique item](https://poe2wiki.net/wiki/Unique_item).
- Blade & Sorcery: [Handle Pose](https://warpfrog.notion.site/Handle-Pose-648bb859b8ef4ecfb9d9292378416709); [Handle](https://warpfrog.notion.site/Handle-1159504e32304980a094055aaab22030); [Nexus, half-sword handles](https://nexusmods.com/bladeandsorcery/mods/10151).
- Unreal: [Mutable overview](https://dev.epicgames.com/documentation/unreal-engine/mutable-overview-in-unreal-engine?lang=en-US); [forum, a weapon kit on Mutable](https://forums.unrealengine.com/t/dspazio-armoury/2674318); sockets, Lyra and Unity in [attachment.md](../docs/attachment.md#how-others-do-it) and [the loadouts note](2026-10-25-loadouts-and-animation-layers.md).
- Animation and props: [Epic, IK Retarget Weapon Goals](https://dev.epicgames.com/documentation/en-us/unreal-engine/python-api/class/IKRetargetWeaponGoalsOpSettings); [Rifle AnimSet Pro forum](https://forums.unrealengine.com/t/rifle-animset-pro/11293?page=4); [Fortune Favours the Bold, sword mocap pack](https://docs.unrealengine.com/marketplace/ja/product/fortune-favours-the-bold-motion-captured-animation-pack); [Unreal forum, attach to the right hand](https://forums.unrealengine.com/t/what-is-the-best-practice-to-attach-the-gun-onto-right-hand-but-still-allow-it-to-be-detached-when-needed/472909); [Esoteric, holding a two-handed weapon](https://fr.esotericsoftware.com/blog/Holding-a-two-handed-weapon); [Animation Mentor, sword fights](https://www.animationmentor.com/blog/tutorial-animate-sword-fights-battle-sequences/); [DSAnimStudio releases](https://github.com/Meowmaritus/DSAnimStudio/releases?page=5); [DS3 model-swap mods, e.g. 792](https://www.nexusmods.com/darksouls3/mods/792); [DS2 hitbox fix](https://www.nexusmods.com/darksouls2/mods/1683); [CHOIS](https://arxiv.org/abs/2312.03913v2); [OMOMO](https://par.nsf.gov/biblio/10521377); [HOI-Diff](https://arxiv.org/html/2312.06553v3).
- Papers and tools: [Volcano, IEEE GEM 2015](https://air.unimi.it/handle/2434/470211); [Houdini sword generator](https://ffd.artstation.com/store/L8X7/sword-generator).
