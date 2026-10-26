# What each hand holds, and how every body moves with it

Status: **accepted by the developers, 2026-10-25** ("Right model", on the review desk); D460 to D465 are decided. The architecture is technical and recorded as D459, by the agent; the questions at the end were answered by the developers (D460 to D465). The research behind it is [research/2026-10-25-loadouts-and-animation-layers.md](../research/2026-10-25-loadouts-and-animation-layers.md). It sits under [movement.md](movement.md) (the pose and fix-up layers) and [items.md](items.md) (equipment D64, the action bar D66).

The developer, 2026-10-25: "we have various weapons, two arms, sometimes two handed items, sometimes two one handed items, sometimes one one handed item... sometimes one in a left arm sometimes one in the right arm (it varies right). and then how do animations work? what happens with variance? i dont know if we figured that out vs just yeeting shit into here". We had not. Today (steps 2 and 3 of section 6, combat rounds 26 and 27) the sword, shield and torch are items with `hold` entries; each hand's socket shows its own hand's item (`terra_survival::held`), the off hand's from the off hand's slot (D460); and each fighter plays the set its hands give, by the rule and lookup in `terra_motion::hold`. T still gives the sword, and the shield when the off hand's slot is empty (D405's shortcut).

## Constraints

- **Kinds as data** (D136): a new weapon is an entry and a model, never code or a clip of its own. **Hundreds, not millions** (D182): a few dozen move sets a player tells apart.
- **One skeleton for people** (D115). **Gear fits by contract** (D128): held things are made once to the hand socket's axes ([characters.md](characters.md#held-things-tools-weapons)).
- **As shipped games do it** (research note): Lyra's shape (one shared graph, plus a layer a weapon only fills with clips and numbers) and FromSoftware's rules (a set per weapon class, stance from the pair of hands).
- **The server decides** (D51, D28): what is in each hand, the stance, the move and its variant are simulation (`terra_motion`); the pose, the hold layer and hand IK are presentation.

## 1. What a hand holds

Each of a body's two hands holds one item or nothing. A body has a **main hand**, right or left (its handedness), and an **off hand**. An item says how it is held, in a `hold` entry in `assets/data/items.ron`:

| Field | What it says | Examples |
|---|---|---|
| `class` | Which move sets it uses; every item of a class shares them | `sword`, `axe`, `mace`, `dagger`, `spear`, `greatsword`, `great_hammer`, `staff`, `bow`, `shield`, `torch`; tools keep their own (`hammer`, `pickaxe`, `knife`) |
| `grip` | Which hands may hold it | `Main` (a one-hander, main hand: a sword), `Either` (a one-hander, either hand: a dagger, a torch), `Off` (off hand only: a shield), `Two` (both hands: a greatsword, a spear, a bow) |
| `lead` | For `Two`: which hand its grip socket rides in; the other reaches its `second` grip | `Main` for a greatsword; `Off` for a bow (held in the off hand, drawn by the main) |
| `carried` | Held, but with no moves: it only lays its arm's pose (a lit torch, a lantern) | `true` for a torch |
| `length`, `mass`, `balance` | Metres; kilograms; the balance point's distance from the grip. Timing and reach come from these by rule (section 2) | an arming sword 0.95, 1.2, 0.12 |

### The stance, by rule from the pair

| Main hand | Off hand | Stance | Move set |
|---|---|---|---|
| nothing | nothing | **Unarmed** | `unarmed` |
| one-hander of class C | nothing, or a `carried` item | **Single** | `C.single` (the off arm in the carried item's pose) |
| one-hander C | a shield | **Shielded** | `C.shielded` |
| one-hander C | a one-hander D (any two, D462) | **Dual** (D462) | `C.dual` when C and D share a family, else `C.single` plus D's off-hand light |
| a two-hander C | put away | **Two** | `C.two` |
| nothing | a shield | **Shield** | `shield.alone` (bash, block, fists) |
| a tool | anything | **Tool** | the tool's use moves (today's, `items.ron`'s `tool`) |

- **A `carried` item counts as nothing** in either hand: a torch in the off hand with a sword is Single, a torch alone Unarmed, both with the carried item's arm pose.
- **A two-hander empties the off hand**: the off item stays equipped but rides its stowed socket (a shield on the back, a blade at the hip), and comes back when the two-hander leaves, as Elden Ring does.
- **An item that cannot go where it is put is refused by the rule**, not by the screen: a shield in the main hand, a second two-hander, a tool in the off hand. So is a pair the table does not list: a blade in the off hand with the main hand empty.
- **Handedness is a mirror**, not a second set of data. Every set is made right-handed. A left-handed body plays it mirrored (section 2), and its items hang on the mirrored sockets, turned, never scaled by −1, so a shield's face is not drawn inside out. Whether bodies may be left-handed is D463.
- **How items reach the hands** (D460): the bar's picked item goes to the main hand (D66), and an **off-hand slot** on the equipment sheet (D64) holds a shield, a torch or a second blade, in hand whenever the main hand's item leaves the off hand free: everything but a two-hander, a tool included (the stance table pairs a tool with anything). An item goes in that slot by its `hold` grip, `Off` or `Either`.

## 2. How animation is chosen

Movement.md's pose layer, filled per stance. Every layer is chosen by rule, from the stance and the move:

| Layer | What it holds | Shared by | Chosen by |
|---|---|---|---|
| **Locomotion** (whole body) | Idle, walk, jog, sprint, strafes, starts, stops, turns, jump, fall, land | Every person, whatever is held | Speed and direction (blend spaces, movement.md) |
| **Hold** (arms and upper spine, by mask) | The stance's arms over locomotion: **ready** (the guard; a shield rests low at the hip and only the block raises it, D531) and **carried** (relaxed, the item tilted, today's `carry_tilt`) | Every item of a class in a stance | The stance; ready while fighting, carried otherwise |
| **Move** (whole body or upper, as the move says) | A blow, a block, a roll, a use | The set | The button, the chain, the runner (movement.md layer 3) |
| **Additive** | Breath, sway, lean, flinch, per-person style | Everyone | Always, small (section 4) |
| **Fix-up** | Off hand onto a two-handed grip, shield on its strap, feet (D455), look | Everyone | After the pose (section 3) |

- **A move set is a file a stance-and-class pair**, `assets/data/moves/<class>.<stance>.ron`: its hold poses, its moves, the buttons they answer and its numbers, shaped as `sword_and_shield.ron` is today. A set may name another to start from (`from: "sword.single"`) and add or replace moves. So `sword.shielded` is `sword.single` with the shield's block and bash.
- **A set falls back** from class to family to the stance's generic set, so a new class plays before its own clips exist: `sabre.single`, then `blade.single`, then `one_hand.single` (`two_hand.two`, `shield.alone`, `unarmed` for the others). The families are `blade`, `blunt`, `polearm` and `heavy`, each class's in `assets/data/moves/families.ron`.
- **Combat styles** (D124, [magic.md](magic.md#combat-styles)) are a few moves added to a set, with their own chains, not sets of their own. Stillwater Guard adds to `sword.shielded`, the Wolf's Run to `dagger.dual`.
- **Locomotion is never per item.** The hold layer is laid over it at run time by Bevy's mask groups (`bevy_animation` 0.20: Blend and Add nodes, masks), as Lyra's item layers and Godot's filtered Blend2 do. Built for the guard over the walk (D458, combat round 28): `terra_walker::layer` masks every bone into the arms (from the clavicles) or the rest, and a clip asked with a hold plays off the arms with the hold's first pose on them, taking the weight its transitions give it; baking every hold over every locomotion clip would be 20 × 25 clips.

### Which button does what

Two hand buttons, named by hand: **main** (left click) and **off** (right click). Heavy is main with the modifier; dodge is its own key. Today these are the walker's Use, Use with Sprint, BuildMenu and Dodge. Each set names the move each button asks for (`buttons: (main: "slash", main_heavy: "heavy", off: "block", dodge: "roll")` in `sword.shielded`), by this table.

| Stance | Main | Main, heavy | Off | Off, heavy |
|---|---|---|---|---|
| Unarmed | Jab, cross chain | A haymaker | Guard with the forearms | A grab (paired moves, later: movement.md's question 5) |
| Single | Light chain | Heavy | A guard on the blade: a smaller block, a parry at its start (Enshrouded blocks only with a shield or a two-hander: Open in D461) | — |
| Shielded | Light chain | Heavy | Block, parry at its start (today's) | Shield bash |
| Dual | Main blade's light | Heavy with both | Off blade's light: the main's light mirrored, as Dark Souls 3 does | Both together (the dual set's own) |
| Two | Light chain | Heavy | Block with the weapon | — |
| Shield | Jab | — | Block | Bash |
| Tool | Use | — | Today's (the build menu) | — |

### Mirroring

- **One table pairs the skeleton's `_l` and `_r` bones**; the centre bones map to themselves, across the body's own side-to-side plane. It is Unreal's Mirror Data Table, built from its documentation, since Bevy has none. The table also swaps the sockets named in a set (`hand.R` with `hand.L`) and flips its warp's side.
- **Baked at export, not done at run time**: `tools/characters` writes a set's mirrored clips beside it, as D454 and D458 bake their holds. The move check then measures what the game plays, on both sides. It also costs nothing a frame, for a thousand players. The cost is twice the clips for a set used both ways.
- **Locomotion is not mirrored**: a walk holds nothing. Only the hold layer and the moves are.
- **Dual uses it whatever the handedness**: the off blade's light blow is the main's light blow mirrored, so a dual set needs only its few both-blade moves made.

### What items of a class share, and what changes by rule

Items of a class share every clip, hold pose, phase, chain and button. Each item has its own model, its grip points, its `length`, `mass` and `balance`, its kit (what its blade and face are made of, D447), and its damage and stamina, which are design numbers.

| What changes | Rule | Why |
|---|---|---|
| Tempo | Play rate `sqrt(I_ref / I)`, clamped to 0.85 to 1.15, with `I = mass × (balance² + length²/12)` the item's moment of inertia about the grip, and `I_ref` the class's reference item | The same arm turning a heavier or longer thing takes longer, as the square root of its inertia. Past the clamp the item belongs to another class |
| Reach | The hit volume is the item's own length from its socket; the warp's `stop_at` moves by its length less the reference's | A longer blade strikes from further away |
| Active frames | Measured again on the item's own tip path by `phases.ts`, on the class's clips | The tip of a longer blade crosses ahead sooner |
| Size of the body | Froude: play rate `1 / sqrt(height scale)` (movement.md, D443) | A bigger body moves slower and reads heavier |

## 3. Fit to any body

| What | How | State |
|---|---|---|
| A held item | Made once in its grip's space: the grip at the origin, the handle along +Y, the edge toward +Z, its length real ([characters.md](characters.md#held-things-tools-weapons), D128). It rides its lead hand's socket, `socket.hand.R` or `socket.hand.L`, placed for each body | Built (right hand; the left for the shield) |
| A two-handed grip | The item marks its `second` grip, metres along +Y. After the pose, the other hand is pulled onto it by two-bone IK (Godot's TwoBoneIK3D, MIT, already ported for the feet, D455), its elbow on the clip's side. A move may let go in marked windows (`free`, a spear's slide). The clips are keyed with both hands (D450), so the IK corrects centimetres, not poses | Not built |
| A shield | On a strap on the forearm, not the hand: its file says the bone, where the strap sits, and which way its face and handle point (`strap` in its glTF extras, `terra_walker::strap`, `fit_held.py`). Recorded as part of D459. The hand closes on the handle; IK only where it is more than 1 cm off | Built |
| A bow | Held in the off hand; while drawn, the string's middle follows the main hand, which stays on the clip | Not built |
| Stowed items | On the body's back and belt sockets, by a `stowed` field in the item's file (the shield's back, the blade's hip on the side away from the main hand) | Not built |
| Many bodies | Sockets placed per body, items at their own size (characters.md, height); IK takes up a longer or shorter arm | Contract built |

## 4. Variance

| Source | How it is made | How it is picked | Rule |
|---|---|---|---|
| **Takes of a move** | `clip.ts` makes several Kimodo takes from a move's spec (D450). It keeps up to the set's `takes` that pass `moves.ts` and differ: the tip's path at least 10 cm apart somewhere in the active frames. Today it keeps the first that passes | A shuffle bag per body: every take once before any repeats, never the same twice running (Unreal's Random Sequence Player in shuffle). Seeded by the body and the move's count, so the server and the client draw the same take | The takes of a move share their phases within a frame and their reach within 10 cm, so a blow's timing and judging do not depend on the draw. Each has its own baked hit track |
| **Additive noise** | One small clip each of breath, sway and lean, made once on the base | Always on, its phase offset by the body's seed; masked off the weapon arm through active frames | It may not move a hand more than 2 cm, so it never changes a hit |
| **Size** | None | The body's height | Froude, as above |
| **Personal style** | A few styles, about four: guarded, loose, heavy, quick. Each is a play rate (±8%), a guard height and width (an additive pose) and a lean | An NPC's from its seed, within its people's range; the player's from their character | It changes how a body looks, never a blow's phases beyond the 0.85 to 1.15 clamp |
| **Handedness** | The mirror, baked | The body's handedness (D463) | — |

### The count (D182)

| What | How many | Told apart by |
|---|---|---|
| Stances | 7 (unarmed, single, shielded, dual, two, shield, tool) | Where the hands and items are: the silhouette |
| Move sets (class × stance that are allowed) | About 20 at most: single and shielded for sword, axe, mace and spear (8); dual for dagger and sword (2); two for greatsword, great hammer, spear, staff and bow (5); unarmed, shield alone; the tools' (3) | Silhouette, reach, the rhythm of the chain, the way the blade sweeps |
| Combat styles | About 12 (magic.md), each 3 to 5 moves on its set | A move a player learns to watch for |
| Moves | About 20 sets × 8 moves, plus about 50 from styles: **about 210**, within movement.md's 250 | As above |
| Clips made | Light blows, blocks, idles and reactions × 3 takes; heavies and specials × 1: **about 450**. The same baked again mirrored, for the sets used left-handed, about 900 | A player sees the takes of a move as one move done a little differently |
| Items | Unbounded, as data: each class's items differ by look, length, weight (tempo within ±15%) and numbers | Look and feel in the hand; no clips of their own |
| A body's look in a fight | 4 styles × 2 hands × the height range: **some dozens**, not millions | How it stands and how quickly it moves |

## 5. How it is checked

| Check | Fails on | Where |
|---|---|---|
| The stance rule | Any pair of grips giving a stance the table above does not, or an item allowed where its grip says no | A table test in `terra_motion` |
| The move check, per set and side | `moves.ts`'s rows (hands or items in the body, the blade's arc, guard start and end, chains) on every set, both sides where mirrored, with the class's shortest and longest items | `node tools/characters/moves.ts --set <class.stance>`, every body template |
| Hold over locomotion | A held item or hand in the body on any locomotion clip under any stance's hold | `moves.ts --also`, a row a clip under a hold (`Walk_Loop+Keyed_Guard`: the same mask, `glb.ts`'s `layered`) |
| Second grip | The off hand more than 1 cm from the second grip after IK, or more than 8 cm before it (the clip wrong, not the IK), or an arm at full stretch | A new `moves.ts` row |
| Mirror | A mirrored clip's left bones not the original's right ones reflected, within 1 mm and 0.1° | A test of the mirror step |
| Takes | Two kept takes of a move closer than 10 cm in the tip's path (duplicates); phases more than a frame apart; reach more than 10 cm apart | `clip.ts --measure` |
| Timing by rule | A heavier item of a class not slower by the rule, or outside the clamp | A test in `terra_motion` |
| The scorecard | Rows per stance (`sword.shielded.*`, `greatsword.two.*`), each held by its own baseline (D452) | `cargo xtask combat` |
| The duel | A duel script per stance pair: shielded against two-handed, single against shielded, then the rest | `cargo xtask duel` with `duels/<pair>.ron` |

## 6. What changes from today, and the order of work

| Today | Proposed |
|---|---|
| Both hands' sockets have a `Grip`; each shows its own hand's item, the off hand's slot's in the off hand (D460) | Done |
| The sword is given on T, with the shield when the off hand's slot is empty (D405's shortcut); each fighter plays the set its hands give | The sword from the bar, once a drawn sword is the bar's item |
| One set, `sword.shielded.ron`, chosen by the stance rule and the lookup, with the partner's slower blows as moves of their own (its slash plays at a third of the player's rate, past a style's ±8%) | The partner plays it at its style's rate |
| Buttons by the stance table, named in the set (`buttons`) | Done |
| The guard's arms laid over the library's walk at run time by a bone mask (`terra_walker::layer`, D458); the strafes and the drawn jump keep their own arms | A hold pose for the strafes and the jump; the body's locomotion set as data |
| `figure.rs` names locomotion clips in code | The body's locomotion set as data |
| One take a move | Up to the set's `takes`, drawn by shuffle |

Each step is done when its checks pass and nothing on the scorecard gets worse:

1. **The developers answer D460 to D465** (D49). Steps 2 to 4 wait only on D460.
2. **Hands as data, no new clips** (with movement.md stage 3):
   - `hold` in `items.ron`, with the sword and shield as items;
   - the stance rule in `terra_motion`;
   - sets looked up by class and stance, with the fallback;
   - `sword.shielded.ron`;
   - buttons by stance.

   Done when the prototype plays as it does now, from items, and the scorecard holds. Built in combat round 26, but for a set's `from` (nothing needs it until a second set) and the partner at its style's rate (a design call: today's partner blows are far slower than a style may make them).
3. **Both hands shown from the loadout**: `held` on both sockets, and `melee` takes its looks from the hands. Done when the penetration rows hold, and a torch carried in the off hand with a sword passes the held-fit check. Built in combat round 27: the torch rows (`held.torch.*`, the guard, walk and strafes) pass; a sword with a torch is not drawn until a `single` set is made (D461's order).
4. **The hold layer at run time** (movement.md stage 2), in place of `Fight_Walk`. Done when every stance's hold passes over every locomotion clip. Built in combat round 28 (D458): the guard over the walk, jog and sprint passes (`move.Fight_Walk.*`, `move.Jog_Fwd_Loop.*`, `move.Sprint_Loop.*`), every number of the walk as `Fight_Walk` had it, which is deleted. The guard over the strafes and the jump does not (the sword 15-20 cm into the lifting thigh), so they keep their own arms until a hold pose for them is made.
5. **The second stance, two-handed** (greatsword), with second-grip IK (movement.md stage 4). It proves the model: a new class and stance by data, clips and IK. Done when the duel of two-handed against shielded passes and the grip rows pass.
6. **Two items of one class** (the arming sword and a longer, heavier sword) on one set. Done when the timing rule holds in the duel's records.
7. **Variants**: kept takes, the shuffle bag, the additive layer. Done when the take rows pass.
8. **The mirror**, if D463 or D462 asks for it.
9. **The other stances**, in the order D461 gives.

## 7. Open questions for the developers

Each is an Open design decision; nothing is built on it until it is answered.

| # | Question | Options | Recommended |
|---|---|---|---|
| D460 | Where does the off hand's item come from? | (a) An off-hand slot on the equipment sheet, used whenever the main hand's item allows it; (b) a bar slot holds a pair; (c) two bars, one a hand | **(a)**: one more slot (D64), the bar unchanged (D66), and switching the main item keeps the shield on |
| D461 | Which stances at the start, after sword and shield? | (a) Sword and shield only until it feels right; (b) add two-handed, then single, then unarmed; (c) everything at once | **(b)**: two-handed proves the model; unarmed is what everyone is without a weapon. Single blocks on the blade, which Enshrouded does not have |
| D462 | Dual wielding? | (a) None; (b) paired items only (twin daggers as one item, one bar slot, one set), as Elden Ring's claws are; (c) any two `Either` one-handers, a dual set when they share a family; (d) any two one-handers | **(b)** now, for the Wolf's Run (magic.md); **(c)** later if wanted |
| D463 | Can a body be left-handed? | (a) Everyone right-handed; (b) chosen at character creation, and an NPC's from its seed (about one in ten); (c) switchable at any time | **(b)**: realism (D122) and variety among people, for one baked mirror and the checks run twice |
| D464 | Can a one-handed weapon be held in two hands? | (a) Never; (b) a `versatile` grip on a few classes (sword, axe, spear) with their own two-handed set, on a key, the off item stowed; (c) any one-hander, with one generic two-handed set | **(b)**, after the two-handed stance exists: a set per versatile class |
| D465 | How many variants of a move? | (a) One take, variety only from additive layers and style; (b) three takes of each light blow, block, idle and reaction, one of each heavy and special; (c) four or more of everything | **(b)**: about 450 clips, made unattended (D450), where repetition shows most |
