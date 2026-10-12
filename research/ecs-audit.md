# ECS audit

**Question (2026-10-12):** Does the game's code keep to our ECS rule (D58) and to "many of everything" (D127)? And is ECS the right model for a game meant to hold a thousand players on one server?

## What was read

All of `crates/terra_game` (about 20,000 lines) was read or searched: every `Resource`, every system that assumes a single player (`Single<…>`), the uses of change detection, the messages between plugins, and every place that names a particular kind of thing in code. Nothing was measured: the cost figures below come from what the code does, not from a profiler.

## Findings

### 1. Per-player state kept in resources

D58 allows a resource only for something that is truly one of a kind. A good deal of what belongs to *a player* was a resource:

| Resource | What it is | Where |
|---|---|---|
| `Digging`, `Spoil` | what the player is digging, and the loose stone and ore dug | `play/dig.rs` |
| `Cutting` | what the player is cutting | `play/harvest.rs` |
| `Swing` | time since the player's last swing | `play/combat.rs` |
| `Builder` | the builder tool's chosen part | `play/tool.rs` |
| `Claim` | the player's claim stone | `play/claim.rs` |
| `Arrival` | where the player comes back after going down | `play/death.rs` |
| `View` | where the player is looking | `play/mod.rs` |
| `Chart.seen` | what of the map the player has seen (D93) | `play/map.rs` |
| `SmithAtHand` | whether a smith lives at the claim | `play/crafting.rs` |

With two players these are shared by both, which is wrong. D58 said the builder would move into components; the inventory did (`Backpack`, `Bar` and `Gear` are components on the player), the rest did not.

Client-only presentation state is fine as a resource, since a client has one viewer: `Notice`, `BuildMenu`, `InventoryScreen`, `Shake`, `Zoom`, `Settings`, `Mix`, and asset handles such as `Models` or `Kinds`. So are true singletons such as `Planet` and the terrain's chunk table.

### 2. Systems that assume one player

There were 116 `Single<…>` queries across 39 files. On the client most are right: the camera, the window, input. But the world's own behaviour also assumed one player: creatures fleeing and hunting (`creatures.rs`), blows landing (`combat.rs`), people following (`people.rs`), stations worked while someone is near (`crafting.rs`), and bodies kept off the player (`bodies.rs`). On a server those must act on every player, or on the nearest.

### 3. One component doing several jobs

`Player` (`play/mod.rs`) held movement, being in the air, combat timers, a roll, a guard and lying downed, all in one. Every system that touched one part was tied to the rest, and change detection could not tell which part had changed.

### 4. Kinds of thing written in code (D127)

Every item was a variant of a Rust enum (`Raw`, `Tool`, `Apparel`, `Container`), with its name, size, what it does and where it is found written in `match` arms. So were creature species (`Species`, with models, animations and health in a `match`, and behaviour that branched on `Species::Deer`), stations (`Station`, with the forge's need for a smith written as a special case), and recipes (a `const` table). Code also looked for particular things by name: a bed by its model's file name (`"Bed_Single"`) and a seat by its part's name (`"Seat"`). Adding a kind of anything meant changing code in several files.

### 5. Work that grows with the square of the count

`bodies::keep_apart` compared every body with every other body each frame, and every body with every tree and rock through a flat list. The player's walk also checked every obstacle on the planet each frame. Fine for the demo's few dozen; with hundreds of creatures and thousands of trees it would dominate the frame.

## Is ECS the right model?

Yes. What ECS gives:

- **Bulk work over many similar things.** Components of the same kind sit together in memory, so a system that moves every creature reads one tight array rather than chasing pointers. This is the reason Unity built DOTS and the reason Overwatch's game code was written as an ECS ([GDC 2017, Overwatch Gameplay Architecture and Netcode](https://www.gdcvault.com/play/1024001/-Overwatch-Gameplay-Architecture-and)).
- **Parallel systems for free.** Bevy runs systems at the same time when they touch different data. A global `ResMut` serialises every system that writes it, which is one more reason per-player state belongs on entities.
- **State that is easy to save and send.** The state is plain data on entities, so saving, replicating and checking it on a server are the same walk over components. Veloren, a multiplayer voxel game in Rust, is built on an ECS for this ([Veloren book](https://book.veloren.net/contributors/developers/codebase-structure.html)).

What ECS does not give:

- **Not everything should be an entity.** Voxel terrain is dense arrays, with entities only for the chunks on screen; items inside a container are data in that component (D83); grass is drawn by the square, not one entity per blade. All three are right as they are.
- **It does not make a thousand players possible on its own.** That comes from simulating and sending only what is near each player (interest management over a spatial partition), sending only what changed, and updating distant things less often. ECS makes these easier, through change detection and queries filtered by place, but they are work still to do (track D in [engine.md](../docs/engine.md)). The code uses change detection in only 24 places so far.

## What was done

All five were fixed on 2026-10-12, in separate commits, each passing `cargo xtask check` and the play loop end to end on the Windows GPU (`cargo xtask loop`):

1. **A spatial hash** (`crates/terra_game/src/spatial.rs`) files bodies, trees and rocks, and players, by place. Keeping bodies apart, a walker meeting trees and rocks, placing a build clear of them, and a creature finding the nearest player now look only at what is near. The demo has 4,588 solid trees and rocks, which the player's walk used to check every frame.
2. **`Player` is a marker**, beside `Motion` (how the body moves), `Stance` (what the hands and feet are doing) and `Downed` (present only while lying where they fell). Falling is now noticed by `Downed` being added, which also fixed the bag dropping again every frame while the loading screen holds the clock.
3. **What belongs to a player is a component of the player**: what they dig, cut, swing and build, their claim, where they look and arrived, and what of the map they have seen. Each plugin makes its own required by `Player` (`register_required_components`), so a new player gets them all. Systems driven by input or drawing the screen act on the `LocalPlayer` (the `IsLocalPlayer` filter, which names `Player` too so Bevy can tell its queries apart from `Without<Player>`). The world's own act on every `Player`: creatures hunt and flee the nearest awake one, blows land on whoever was bitten, anyone falls and wakes at their own bed, followers follow whoever freed them, stations work while any player is near, and every player has a figure holding what they hold. The forge's smith is read from the people (`crafting::Staff`) instead of a flag beside them. Tests put two players in one world.
4. **Kinds are data** (D136), in `assets/data/`: `items.ron` (every item, what it does in hand and worn, and the starting kit), `stations.ron`, `recipes.ron` and `creatures.ron`. Code asks what a kind does (whether a tool builds, what it cuts, what it shoots; whether a creature is hostile, what its carcass gives) rather than which one it is. A bed and a seat are a property of what is built (`Rest`), not looked for by file or part name. Tests check that every name a file or cost uses is a kind.

## Left to do

- **Polling that remains, all cheap.**
  - The station screen recounts what each recipe needs every frame while it is open.
  - The bag formats its labels every frame, though it writes them only when they differ.
  - The ground's mesher looks over its chunks when nothing is pending.
  - Each is a handful of strings or a short scan. None grows with the world.
- **The ground's materials** (soil, stone, ore, sand) are still constants in `terra_voxel`, laid down by its generation rules. What digging each gives is data (`assets/data/ground.ron`); the materials themselves will move when per-planet ground is designed (D59).

Flora, what digging gives, and the build catalogue were made data on 2026-10-12 too (`flora.ron`, `ground.ron`, `builds.ron`), growing the same world as before: 13,366 trees, rocks and plants, 4,588 of them solid.

## Later on 2026-10-12

The rest of the plan that came out of this audit was done the same day:
- **Intent and the fixed tick:** each player acts on an `Intent` component (D137), and gameplay runs on a fixed tick with interpolation (D138).
- **Registries:** every kind is a registry read from `assets/data`, reloaded when a file changes (D136).
- **Simulation level of detail:** creatures are simulated by distance from the nearest player (D145).
- **Change detection.** A survey found systems doing work every frame for nothing. These were fixed:
  - **Ground material:** it was prepared again every frame, because the sun moving marked the tweaks changed. It is now written only when its own values differ.
  - **Builder:** it raycast every tick without the hammer in hand, and re-inserted its ghost and film every frame.
  - **Backpack grid:** it was rebuilt while the screen was shut.
  - **Screen effects and markers:** the pain overlay was redrawn every frame. The map marks, the dial's labels and the gains feed were written without comparing first.
  - **People:** a held item kept turning after it had settled, a sitting person searched every seat every frame, and the people's animation looked up last positions in a list.
  - **Finding what changed:** the map rescanned every frame, the claim's edge was laid on the ground again every frame, and the shader count ran after loading.
  - **Items:** `Belongings` marked the registry of items changed whenever anything read a player's things.

## Limits

- Nothing was profiled. The quadratic cost of `keep_apart` is inferred from the code, not measured, and so is what the spatial hash saves.
- There is still only one player in a running game, so "works with any number" is checked by tests that spawn two, not by a networked session.
- Input still reaches the player through local systems. A server will need each player's input as a component written by the network; that belongs to track D, not here.
