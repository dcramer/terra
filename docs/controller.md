# Character controller

Status: **design, 2026-10-24.** The mover is movement.md's layer 2: how every body (the player, people, creatures) is moved through the world and stopped by it. The technical shape was decided in D53 by the agent. The questions at the end are design questions and are Open, for the developers. The research behind it is [research/2026-10-24-character-controller.md](../research/2026-10-24-character-controller.md).

## Intent

A body goes where its intent or its move's root motion asks, as far as the world lets it. It is never inside anything, never through anything at any speed, and it is held on whatever ground is there now: dug tunnels, a building's floors, stairs, a rock. Down is toward the world's centre wherever it stands (D63). The same rules apply to the player and to a troll, with only the numbers different.

## Constraints

- **Bevy-free, in `terra_motion`** (D28, D447): the server runs it (D51), and a client predicts its own player with the same code.
- **Fixed tick, deterministic** (D138, 64 a second). Bodies are processed in the order of their IDs. The iteration counts are fixed and trigonometry goes through `libm` (D30). Given the same world and intents, every machine gets the same positions.
- **Ported, not invented** (D196): collide-and-slide is Jolt 5.6's `CharacterVirtual` (MIT), and its notice is kept in the ported module. The floor settings come from Godot 4.7's `CharacterBody3D` (MIT).
- **Kinds as data** (D136): every number below comes from the body's kind file, and nothing in code names a kind.
- **Many bodies** (D52, D127): the cost per body is benched, and a body that stands still with nothing moving near it costs almost nothing. Bodies find each other through a grid of bodies by place (the broadphase), so a body's cost grows with the bodies near it, not with how many there are; it changes no answer, only which bodies are asked.

## How a tick moves a body

1. **Velocity from intent**. The move is built from: acceleration toward the wished velocity, the turn rate, the slope speed curve, air control in the air, and buoyancy in water. Gravity is added along the body's down, and a jump, coyote time and the buffer are applied. Inside a move, the move's root motion replaces this (movement.md).
2. **Collide and slide**, as Jolt does it. Contacts within the predictive distance are collected and turned into planes. A plane steeper than the slope limit gets a second, upright plane, so it stops the body like a wall. The velocity is solved against the planes, and the capsule is swept along the result, stopping at the first contact.
3. **Step up**, one rule for every source. If the move was blocked while walking forward, the body tries up the step height, then forward, then down. It keeps the result only if it lands on walkable ground and the floor carries on a short way past the edge (Jolt's forward test). Where the whole rise put the head into what overhangs the way (a doorway's lintel over its threshold) and the feet came down on the edge, it tries again risen only 5 cm over that edge (Jolt and Unreal rise the whole step and stop there). A thing of flora (a stump, a log, a rock) taller than the step over its ground is no step but a thing to jump onto, however it is come at (by its thin end's lower piece, by the round of an edge a little over the piece stood on, or from a slope's high side), unless the feet already stand more than a step over its ground: its contacts are met as walls, their normals laid level (Unreal's `CanCharacterStepUpOn`, and its walkability judged by the face struck, `ImpactNormal`).
4. **Stick to the floor**. A body that was grounded and is not jumping is swept down up to the stick distance and snapped onto walkable ground, so it stays on its feet down a hill or stairs.
5. **Ground state**: grounded, steep (on ground past the limit, sliding down), airborne or swimming. Swimming means the water is deeper than the kind's swim depth.
6. **After every body has moved, pushboxes**. Bodies that overlap are separated by mass: each moves by its share of the overlap, inversely to its mass, and a fixed body has infinite mass. This runs for four rounds in ID order. Each push is a sweep against the world, so a push never puts a body into a wall and a later push cannot undo an earlier one. The player is pushed like anyone else, wherever pushing is allowed (open question 4).

## What stops a body

The mover asks one query trait, defined in `terra_motion` because the mover is the side that asks (D198). The trait has two questions: the contacts within a distance of a capsule (each a point, a normal, a distance and the other's velocity), and the first contact of a capsule swept along a motion. A body's position is f64 and a capsule's points are f32 offsets from its f64 origin; a source is asked at the origin plus the offset in f64, so 30 km out the field is as fine as at 2 km (D482, Jolt's large worlds). Each source answers from what it already holds, with no collision meshes built:

| Source | Shape | Answered from |
|---|---|---|
| Voxel ground, dug or not | The SDF itself | Distance and gradient (centred differences, one-sided where the step reaches over a convex edge). A sweep advances by distance divided by the field's steepest slope (`Field::steepest`), so a voxel-thin wall cannot be skipped |
| Buildings | The base's cubes as boxes, merged greedily per section (building-plan.md), and a door's leaves where they are drawn | `terra_build_layout`, recast as contacts |
| Props | Each solid item's box, turned with it; hidden ones skipped | `bodies::Solid`, moved into the plain crate |
| Trees, rocks, stumps, logs | Rounded convex pieces with their tops | `terra_core::solid` |

Buildings, props and trees and rocks are the mover's obstacles (`terra_motion::mover::shapes`): a turned box (a cube, a leaf, a prop) or a solid's pieces, each its exact distance, those near a capsule answered together as one exact field, so cubes side by side have no seam. The ground stays a source apart, so a box standing on it meets it at a true corner, not a blended one.
| Other bodies | Their pushbox capsules, moving | Treated as moving obstacles in the sweep, then separated by mass afterwards (step 6) |
| Water | A medium, not a contact: the depth of the surface over a point | `Planet::water_radius_m`, and lakes and rivers when they come |

Ceilings need nothing special: a capsule's top meets a roof as its side meets a wall, and the upward velocity into it is removed.

## Numbers that are data

Each kind's file holds these numbers. A man's stand-ins are shown in brackets: today's walker's values, Jolt's defaults, or the industry's.

- **Capsule**: radius (0.3 m), height (1.8 m), mass (75 kg).
- **Speeds** for walk, jog, sprint and swim (jog 4.3, sprint 7.4 m/s, today's values), with acceleration and deceleration on the ground and in water, and a turn rate.
- **Step height**: 0.55 m. That is one voxel of dug ground (0.5 m) with padding, so a one-voxel ledge is walked up, as today's terrain rule allows. It also covers every building stair.
- **Slopes**: the walkable limit (50 deg, Jolt's default and today's 1.2 rise to run), and a speed curve by gradient, uphill and downhill.
- **Jump**: launch speed (6.0 m/s, about 1.1 m apex), coyote time (0.1 s), buffer (0.15 s), and air control as a fraction of ground acceleration.
- **Stick-to-floor distance** (0.5 m, Jolt's), predictive contact distance (0.1 m) and padding (2 cm).
- **Swimming**: swim depth as a fraction of height, and buoyancy.
- **Stamina** costs of sprinting, jumping and swimming (D61; jumping costs 8 today).
- **Pushing**: which kinds push which, taken from the answer to open question 4.

Gravity's strength belongs to the world, not the body (D63 leaves its value open).

## What is presentation

The server never sees any of these, and the mover never waits for them:

- **Step smoothing**. Stepping up moves the capsule at once, as Jolt does. The shown body keeps a vertical offset that runs down to zero over about 0.12 s, so a step reads as a stride and not a pop. This is Unreal's mesh smoothing and the Source engine's view smoothing. The camera follows the figure as shown, not the capsule (`terra_walker::stepping`).
- Interpolation between ticks, which is in place already (`terra_world::tick`).
- Feet planted and stepped by rule (D455), foot IK and the pelvis on slopes and steps (movement.md stage 4), lean into turns, and the swim pose.

## Production criteria

These are `tools/controller/criteria.json` (D457), in D452's shape: a row for each criterion with its bar, its source and its check, held by a ratchet (`cargo xtask controller`, the `controller` skill). They are measured on two courses:

- **The plain-Rust course** (`terra_motion::course`, driving a mover through its `Mover` trait). Analytic SDF fixtures are run as tests in `terra_motion`, on every core (D406): steps from 0.1 to 0.8 m by 0.1, slopes from 0 to 70 deg by 5, a dug tunnel 2 m high, a gap one voxel wide, a stair, a ledge to drop off, a pool, a sphere-world course for gravity, and a moving crowd.
- **The in-game filmed course**. The same features are dug and built in the yard, and a scripted player walks, jogs and sprints through them (`--press`, `cargo xtask gpu film`).

| # | Criterion | Bar | Source |
|---|---|---|---|
| 1 | No tunnelling | At any speed up to 60 m/s, no body ends a tick past or inside a wall 1 voxel thick, a door or another body | Jolt (swept move); ours for the SDF sweep |
| 2 | Penetration depth | Never deeper than 1 cm after a tick, median under 2 mm | Jolt (tolerance 1 mm, padding 2 cm) |
| 3 | Slope limit exact | Walked up to the limit, stopped beyond it, within 0.5 deg on the 0-70 deg course | Jolt's limit; our data |
| 4 | Slope slowdown | Speed on each slope within 2% of the kind's curve | player.md ("slopes slow you"); our data |
| 5 | Step-up consistent | Each source (ground, building, prop, rock) climbs steps up to the step height and stops at steps 1 cm higher; a stump or log taller than the step is never climbed, from eight ways on level ground and a 20 deg slope (`step.flora`) | Jolt WalkStairs; Unreal CanCharacterStepUpOn; our one rule |
| 6 | Step-up smooth | The shown root rises no more than a quarter of the step in any frame, and settles within 0.15 s | Unreal mesh smoothing |
| 7 | Stick to ground | No airborne tick sprinting down a 45 deg slope or a 0.3 m stair | Jolt StickToFloor |
| 8 | Ceilings | A jump in the 2 m tunnel: the head never more than 1 cm into the roof, and the upward speed zero on contact | Jolt contact planes |
| 9 | Gravity toward the centre | At the poles, the equator and in a tunnel, a body lands where the radial line meets the ground (within 1 cm), standing upright to within 0.5 deg | D63 |
| 10 | Jump | Apex within 1 cm of v²/2g, coyote time and buffer each kept to within one tick | Our data; the platformer convention |
| 11 | Air control | Acceleration in the air within 2% of the kind's fraction | Our data |
| 12 | Landing | Grounded within one tick of touching ground, no bounce, landing speed reported (for open question 1) | Jolt ground states |
| 13 | Input to motion | Velocity moves toward the input on the first tick after it is pressed | D180; movement.md stage 2 |
| 14 | Acceleration and turn as data | Time to top speed and turn rate within one tick of the kind's data, and two kinds' files give two results | D136 |
| 15 | Weighted pushing | Overlap under 5 cm after a tick, each body's share within 5% of its mass ratio, a fixed body unmoved, the player included | movement.md duel bar; open question 4 |
| 16 | Fits where it should | Passes gaps 4 cm wider than its diameter, and is stopped by narrower ones | Ours |
| 17 | Swimming | Deep water is swum rather than walled; the body floats to the surface and dives on intent; breath is used with the head under | D74 |
| 18 | Stamina | Sprinting, jumping and swimming cost the kind's data to within 1% | D61 |
| 19 | Determinism | 10,000 ticks of the crowd course end bit-identical across runs, thread counts and the order bodies are given in | D51, D138 |
| 20 | Cost per body | Under 25 µs a body a tick on one core walking the course and the planet's ground at 2 and 30 km (`cargo xtask controller --bench`, release), under 50 µs walking the `walk` scene's route among the lush yard's trees and rocks (`cost.forest`), and a crowd of 1,000 under 4 ms a tick on the server's cores | D52, D138 (a quarter of the 15.6 ms tick) |
| 21 | Feet sliding | Under 3 cm a contact at walk, jog and sprint, and in starts, stops and turns | movement.md stage 2; D455 |
| 22 | Far from the origin | At 30 km, walking level ground, 20 deg up and down and 0.3 m steps: never off the ground nor over 10 cm (a tick at a sprint) from the same walk at 2 km, the foot's gap to the ground within 1 mm of 2 km's, and no more than 10% more field calls a tick | D482; Jolt's large worlds |

No Enshrouded movement numbers have been measured yet; combat's measurements are D180's. The QA stage should measure Enshrouded's walk, sprint and jump speeds and its step height, as the combat reference was measured, and add each one's row once it is.

## Open questions for the developers

1. **Fall damage** (player.md). Does a long fall hurt? Recommended: damage above a landing speed of about 12 m/s (a fall of about 4.5 m at today's gravity), rising to death by about 25 m/s (about 20 m), with water breaking the fall. Open.
2. **Climbing** (player.md lists it, without a design). Recommended: a ledge grab and a mantle now, and free climbing on stamina later if wanted. Open.
3. **Swimming beyond D74**. The questions are how to swim at the surface and how to dive (look to dive, or keys for up and down), how fast, and how long breath lasts. Recommended: dive by looking and pressing forward, rise with jump; a swim speed about half the jog; about 30 s of breath at the start. Open.
4. **Bodies blocking bodies** (movement.md question 4). Recommended: players and recruits push each other softly, and monsters push hard. Open.
