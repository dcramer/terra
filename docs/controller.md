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
5. **Ground state** (D509): grounded, steep (on ground past the limit, sliding down), airborne or swimming. Swimming means the water is deeper than the kind's swim depth.
6. **After every body has moved, pushboxes** (D479). A step met another body only by its core (`core_m`), so bodies that walk into each other overlap and are pushed apart here. First, since each step met the others where they stood as the tick began, two bodies each moving toward the other by more than its radius in a tick (over 22 m/s each for a man) could cross: a body whose step is long (over 0.75 of its radius less its core) has its core cast along its step relative to each other's, and a pair whose cores would meet is put back along their steps to that moment (Jolt's continuous collision; `tunnel.closing`). Then overlapping bodies are pushed apart, each by its share of the overlap inversely to its mass, a fixed body having infinite mass. Two soft bodies (people) ease apart, by the share of the overlap their kinds' `soft_s` leaves a tick (Detour's crowd separation), summed for each body from where all stood as the tick began and shoved as its step begins, on its own lane; any pair with a hard body (a monster) at once, after the steps, for four rounds in ID order. Each push is a sweep against the world, and what a wall stops of one body's share is handed to the other, so a push never puts a body into a wall. The player is pushed like anyone else.

## What stops a body

The mover asks one query trait, defined in `terra_motion` because the mover is the side that asks (D198). The trait has two questions: the contacts within a distance of a capsule (each a point, a normal, a distance and the other's velocity), and the first contact of a capsule swept along a motion. A body's position is f64 and a capsule's points are f32 offsets from its f64 origin; a source is asked at the origin plus the offset in f64, so 30 km out the field is as fine as at 2 km (D482, Jolt's large worlds). Each source answers from what it already holds, with no collision meshes built:

| Source | Shape | Answered from |
|---|---|---|
| Voxel ground, dug or not | The SDF itself | Distance and gradient (centred differences, one-sided where the step reaches over a convex edge). A sweep advances by distance divided by the field's steepest slope (`Field::steepest`), so a voxel-thin wall cannot be skipped |
| Buildings | The base's cubes as boxes, merged greedily per section (building-plan.md), and a door's leaves where they are drawn | `terra_build_layout`, recast as contacts |
| Props | Each solid item's box, turned with it; hidden ones skipped | `bodies::Solid`, moved into the plain crate |
| Trees, rocks, stumps, logs | Rounded convex pieces with their tops | `terra_core::solid` |

Buildings, props and trees and rocks are the mover's obstacles (`terra_motion::mover::shapes`): a turned box (a cube, a leaf, a prop) or a solid's pieces, each its exact distance, those near a capsule answered together as one exact field, so cubes side by side have no seam. The ground stays a source apart, so a box standing on it meets it at a true corner, not a blended one.
| Other bodies | Their cores (a thin capsule on the pushbox's axis), moving | Treated as moving obstacles in the sweep, then pushed apart afterwards (step 6) |
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
- **Swimming**: swim depth as a fraction of height, the share of a jog a wader slows to by that depth (D523's 0.6), float depth (buoyancy), the dive rate and the look past which a swimmer dives, the highest ledge it climbs out onto (D523's about 1 m), where its mouth is, and breath: how long it lasts, how long it takes to come back and how long drowning takes (D478's placeholders).
- **Stamina** costs of sprinting, jumping and swimming (D61; jumping costs 8 today).
- **Falls** (D476): the landing speed into the floor a fall starts to hurt at, and the one it kills at.
- **Pushing** (D479): soft or hard, the core another body is stopped by (0.05 m), and how long a soft pair takes to ease apart (0.05 s, for review).

Gravity's strength belongs to the world, not the body (D63 leaves its value open).

## What is presentation

The server never sees any of these, and the mover never waits for them:

- **Step smoothing**. Stepping up moves the capsule at once, as Jolt does. The shown body keeps a vertical offset that runs down to zero over about 0.12 s, so a step reads as a stride and not a pop. This is Unreal's mesh smoothing and the Source engine's view smoothing. The camera follows the figure as shown, not the capsule (`terra_walker::stepping`).
- Interpolation between ticks, which is in place already (`terra_world::tick`).
- Feet planted and stepped by rule for every body, walking or fighting (D455, `terra_walker::feet`): a stride's clip plays in place at the body's speed (`pace.rs`, speed matching) and its planted feet are read on the ground it passes, so they lock where they land; when a foot is locked is baked per clip into its file as a lock curve (ALS's FootLock curves, motion matching's contact labels: heel or toe still over the ground at the clip's one pace, `tools/characters/markers.ts`), summed over the clips coming in, and a lock eases back to the clip as its curve falls; a lock starts only once the foot as placed is down, its sole within 7 cm of where it rests over the ground as probed, not as eased (`feet::PLANT_M`, Unreal Foot Placement's DistanceToGround), so a stop's fade or a foot over lower ground locks when it lands, not in the air; a foot turned 45 deg about the up from its clip steps (turn in place, `turn.foot`). Feet are put on the ground under them (foot IK, Unreal's Foot Placement and ALS: a locked foot probed under heel and ball once at its lock and that ground frozen for the lock, a swinging foot under heel and ball once a frame, pitched and rolled to it (at a riser flat on the higher tread, not pitched toward the lower), raised or lowered, eased in the world, a swinging foot's raise lagging no more than half the clip's clearance so it touches down only as the clip's does, a locked foot's not at all; the pelvis dropped by the lower foot on a critically damped spring, capped where a planted leg folds; `terra_motion::placement`), measured on the world's ground sampled down from the heel and ball apart from that probe (criterion 21b). The game's clip files keep their baked lock markers (`clips.markers`). The probe reads the ground straight under heel and ball (its sphere's touch on a slope is uphill of the point, so a line from where it stopped finds the height, as Unreal's floor check does), and the ball is where the figure draws it, at the body's scale. A stride clip made slower or faster than the body moves is played faster or slower first, within 0.5-1.6 of its rate, and a speed past what the fastest play rate covers is taken by lengthening its stride (Lyra's capped play rate, then Unreal's Stride Warping; `terra_walker::pace::gait`): each foot's travel along the way from its hip stretched up to 1.4 times, eased over a tenth of a second, before the lock and placement, so a lock takes the warped foot and placement probes under it; the pelvis lowered so a planted leg reaches its warped foot no farther than the clip's leg reaches its own (`placement::Grounded::stretch`, Unreal's pelvis adjustment). The sprint's clip (3.9 m/s, Kimodo makes none faster) is shown at 7.4 m/s at 1.6 of its rate and 1.19 of its stride, about 3.8 steps a second of 1.9 m as a sprinter runs; the jog within its play rate is not warped (`pace.match`, criterion 21c). A foot locked straddling a riser is probed again while it straddles, so a toe-off rolling its heel over a step down's edge sets it on the lower tread. Still to come: a sprint clip at its gait's speed, sync markers between gaits, blends by speed and direction, starts, stops and turns, and lean into turns.
- **The swim pose** (D514, `terra_walker::swim`): the figure treads water, strokes at the surface or dives by the mover's swim (`Swim_Tread_Loop` under 0.4 m/s, `Swim_Fwd_Loop` over 0.6, `Swim_Dive_Loop` with the mouth under), blended in 400 ms, and is tilted from upright along the way it goes (level at the surface but for 10 deg of head and shoulders out, pitched along a dive), eased about the float height, as Unreal's swimmers are tilted by their pitch; its feet are let go. Measured on the filmed lake (`swim.pose`).

## Climbing

Status: **decided** (D477, D505-D511, the developers, 2026-10-26: "Recommended set"); the mover holds, starts and climbs a face, faster on one less steep (`mover::climb`, `Ground::Climbing`, units C3-C4), pulls up onto a top (C5), lunges along a face or kicks off it (C6), and spends stamina, holds no face of no grip or in a fight stance and lets go at none or at a blow (C7), climbs a dug cliff over its treads at the gap (C8), on the course only: the game's man has climbing off (`terra_walker` `movement::man`) until the figure has clips, and the walk's own rows ask a man with it off (`Kind::walking`). Held: `cost.climb`, 12-34 µs a climbing tick against the bar's 25 (C9: the wall and the dug cliff under it, the base's wall over by its cubes' field); the game's walk and figure. Free climbing on stamina; the research is [research/2026-10-25-climbing.md](../research/2026-10-25-climbing.md).

### Intent

A body can hold a face too steep to walk and move over it on stamina, up a cliff, a dug pit's side, a base's wall, a rock or a trunk, and pull itself onto the top. It reads as the world does (D122): rough rock holds, a steeper face tires sooner, a tired climber lets go and falls. It is the way out of a hole dug too deep (player.md, which has no glider or hook) and a skill by doing (D88).

### What was decided

- **What** (D505): every surface, its material's grip scaling the cost; a few materials cannot be held (ice, glass, plaster), so a base keeps climbers out by its materials and an overhang. Other bodies are never climbed.
- **Cost and zero** (D506): holding costs a quarter of moving; at zero the climber lets go and falls, its landing judged by D476; a staggering blow knocks a climber off (Conan Exiles), so no body hangs out of reach forever.
- **Starting** (D507): a jump at a face grabs it at once; walking into one grabs after half a second's push; never in a fight stance.
- **Pulling up** (D508): at a climb's top, and at any ledge the hands reach from a jump or a fall, about 2.2 m over a man's feet. No running vault.
- **Speed** (D509): about 1 m/s up a sheer face, faster on a face less steep, slower on an overhang.
- **Overhangs** (D510): to about 30 deg past vertical at twice the cost; never a ceiling.
- **Climb jump** (D511): a lunge of about 1.5 m along the face for a lump of stamina; jump with the stick away kicks off backwards, leaving the face.

### Constraints

- **The same sources, no new query.** It climbs what the mover already reads: the voxel ground, a base's cubes and doors, props, trees and rocks, through the one query trait (contacts near a capsule, a swept capsule).
- **A mover state like swimming**: a ground state `Climbing` and a module `mover::climb` beside `mover::swim`, in `terra_motion`, Bevy-free (D28) so the server runs it and a client predicts it.
- **Kinds as data** (D136): every number below is the kind's, in movers.ron; a kind with no climb speed does not climb, so a troll and a man differ only in their files. A material's `grip` is its file's.
- **Deterministic** (D138): fixed probes and iterations, `libm`, nothing from animation; the same bits on every machine.
- **Cost**: a climbing body under `cost.body`'s 25 µs a tick, as a walking one.

### The mover's climb, as shipped games do it

Every shipped climber (Breath of the Wild, Genshin, Conan Exiles, Palworld; Unreal's custom movement modes) is one pattern, and ours follows it:

1. **The face.** Two probes ahead of the capsule, at the hands (the kind's `reach`) and at the feet: thin sweeps from its axis reaching 0.6 m past `climb_gap`, so a dug cliff's next riser set back a cell is found. The face's normal is their average, as Unreal's climbers average a sweep's hits; on the ground the SDF's gradient already blends a dug face's 0.5 m steps into one plane.
2. **Starting** (D507): a face the capsule touches (its contacts within `climb_gap`) steeper than the slope limit, not leaning past `overhang_deg` (D510), rising past the step height (a probe there finds it; lower is a step, as now), within 25 deg of the way the body faces, of a material that holds (D505), with stamina left, not in a fight stance; touched in the air, or pushed into for `climb_push_s` on the ground.
3. **A tick of climbing**, in order: refresh the face; stop if it is gone (the body falls), or land if moving down onto walkable floor under the feet (grounded); velocity by the kind's climb acceleration toward the input laid on the face's plane (forward is up the face, the sides along it), gravity off; collide and slide as every tick does, so a corner, a beam or another body stops it as a wall stops a walker; then hold to the face: swept toward it to `climb_gap` (the stick-to-floor step turned toward the face), the facing turned to the face's normal at the turn rate. The capsule stays upright (D53); the shown body leans.
4. **The top** (D508): moving up, the hand probe finds no face and a walkable floor within `reach` over the feet with room for the standing capsule (a swept check); or, in the air, a ledge within `reach` ahead (held at once on touch, then as from a climb): the body is carried onto it over `mantle_s`, up then forward, along the mover's own path (Unreal's ALS mantle); with no room to stand it holds at the lip; a pull-up clip is fitted to that path (started at the height that matches, as Unreal's root-motion mantles are, or warped), never the other way round.
5. **Stamina** (D506, D510, D511): each tick costs the kind's rate moving (the stick held), a quarter of it holding (and over a lunge's ticks), a climb jump its lump, doubled on an overhang (past 5 deg beyond vertical, a placeholder) and over the material's grip (`Query::grip` where the probes touch, a hit's material as Unreal's physical material; 0 never held); at zero, or on a staggering blow (`Intent::knocked`), the body lets go and is airborne that tick, and a knocked one holds nothing again until it stands. The pool is the caller's (`Body::stamina`, of `FULL_STAMINA`, the game's `Stamina`'s 100, D61): written in before a tick, the spend taken back after; the mover never refills it.
6. **Leaving** (D511): a key lets go; jump with the stick away (within 60 deg of straight out) kicks off backwards, launched as a jogging jump off the ground (the kind's `jump_speed` up, its `jog` out); jump with it toward the face, along it or let go lunges `climb_jump` that way along the face, still holding, rising as a hop under the world's gravity does (fast, slowing to a stop where the hands hold again), so its time is gravity's, not a number.

What is presentation, as for walking: hands and feet put on the face by IK from the same probes (Unreal's climbers trace each limb to the face and solve full-body IK), a blend by the velocity on the face (up, along), the pull-up clip, the lean to the face, the ground's foot IK off while climbing.

The kind's numbers (a man's in brackets; the decided ones marked, the rest placeholders to tune): `climb` (1.0 m/s up, D509), `climb_side` (0.8), `climb_accel` (4), `climb_push_s` (0.5, D507), `reach` (2.2 m, the hands over the feet, D508), `climb_gap` (5 cm), `overhang_deg` (30 past vertical, D510), `mantle_s` (0.8 s), `climb_jump` (1.5 m, D511), stamina `climb_stamina_s` (8 a second), holding a quarter of it (D506), `climb_jump_stamina` (20), so a man climbs about 12 m on a full pool of 100.

### Its criteria

Rows of `tools/controller/criteria.json` on the plain-Rust course (`course/climb.rs`), for a man and the second kind; `?` rows say what they need. Each fails on a mover that does not climb.

| Row | Bar | Decision |
|---|---|---|
| `climb.faces` | A sheer wall, a dug cliff of 0.5 m steps, a base's wall of cubes, a rock and a trunk walked into are climbed; a walkable slope, a step-high face and another body never | D505 |
| `climb.grip` | A face of a material that does not hold is never held; grip scales the cost | D505 |
| `climb.start` | A jump at a face holds it within a tick of touching; walking in, half a second's push (within a tick), never sooner | D507 |
| `climb.stance` | Never started in a fight stance | D507 |
| `climb.mantle` | Standing on the top of a climbed wall, and of a ledge reached from a jump; never onto a top with no room to stand | D508 |
| `climb.speed` | Up a sheer face within 2% of the kind's speed (a man's 1 m/s) | D509 |
| `climb.steep` | Faster up a face less steep, slower up an overhang | D509 |
| `climb.overhang` | Held to about 30 deg past vertical, never past 35, never a ceiling | D510 |
| `climb.lunge` | The lunge's rise within 2 cm of the kind's (a man's 1.5 m) | D511 |
| `climb.kickoff` | Jump with the stick away leaves the face airborne, moving away | D511 |
| `climb.stamina` | Holding costs a quarter of moving, an overhang twice, a lunge its lump, within 1%; at zero airborne within a tick | D506, D510, D511 |
| `climb.knock` | A staggering blow knocks a climber off within a tick | D506 |
| `climb.hold` | Within 1 cm of `climb_gap` up the dug cliff's steps and a sheer wall, never deeper than criterion 2's 1 cm | D505 |
| `climb.down` | Climbing down onto walkable floor stands within a tick of reaching it, unharmed | D476 |
| `cost.climb` (`--bench`) | Under 25 µs a body a tick climbing the course's wall, the dug cliff and the base's wall | D52, D138 |

`determinism.crowd` has six men climbing a sheer face beside the walking crowd, up it, along it into each other and down. Later: `climb.hold` round a face's side corners; the filmed yard gains a cliff, the base's wall and a tree, and once stage 4's limb IK lands, a row for hands and feet on the face (under 3 cm off it).

## Production criteria

These are `tools/controller/criteria.json` (D457), in D452's shape: a row for each criterion with its bar, its source and its check, held by a ratchet (`cargo xtask controller`, the `controller` skill). They are measured on two courses:

- **The plain-Rust course** (`terra_motion::course`, driving a mover through its `Mover` trait). Analytic SDF fixtures are run as tests in `terra_motion`, on every core (D406): steps from 0.1 to 0.8 m by 0.1, slopes from 0 to 70 deg by 5, a dug tunnel 2 m high, a gap one voxel wide, a stair, a ledge to drop off, a pool, a sphere-world course for gravity, and a moving crowd.
- **The in-game filmed course**. The same features are dug and built in the yard (the sandbox's `controller-yard` experiment: the ground's steps 0.1-0.5 m down and up, a lump, a mound past the slope limit, a base's stairs, doorway, low passage and a 1.5 m drop; the yard has no water), and a scripted player jogs and sprints through them on its route, filmed through the game's camera; the heavy rows are measured from each run's per-frame walk record, not by eye (`cargo xtask gpu film -- cargo xtask controller --film`, tools/controller/yard.ts). The player has no walk pace to press yet, so walking is not filmed.

| # | Criterion | Bar | Source |
|---|---|---|---|
| 1 | No tunnelling | At any speed up to 60 m/s, no body ends a tick past or inside a wall 1 voxel thick, a door or another body, nor two bodies closing at up to 60 m/s pass through each other | Jolt (swept move, its CCD for bodies closing); ours for the SDF sweep |
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
| 12 | Landing | Grounded within one tick of touching ground, no bounce, landing speed reported | Jolt ground states |
| 12b | Falls | No harm from a jump, a step, stairs or a drop short of the onset; harmed 3% past the onset, alive 3% short of the fatal speed and dead 3% past it; harm never falls with height; a slope takes its share by its normal; no harm into water deep enough to swim in (`course/fall.rs`) | D476 |
| 13 | Input to motion | Velocity moves toward the input on the first tick after it is pressed | D180; movement.md stage 2 |
| 14 | Acceleration and turn as data | Time to top speed and turn rate within one tick of the kind's data, and two kinds' files give two results | D136 |
| 15 | Pushing (D479) | A pair with a hard body under 5 cm overlap after a tick, each body's share within 5% of its mass ratio, a fixed body unmoved, the player included; two people ease apart by their kind's time, a walker loses under 10% of its walk passing one standing and moves them no farther than out of its way; a monster loses under 30% of its walk through a line of men, a man moves it at his share of their mass, and nobody is pushed into a wall (`course/pushes.rs`) | movement.md duel bar; D479; Detour's crowd separation |
| 16 | Fits where it should | Passes gaps 4 cm wider than its diameter, and is stopped by narrower ones | Ours |
| 17 | Swimming | Deep water is swum rather than walled; moving forward looking down dives at a rate the look sets, jump held rises, let alone it floats up to rest with its mouth out and no bob growing; breath is used only with the mouth under, runs out on time and drowns it a share a second, and comes back at the surface; a diver swims along a lake's bed without sinking into it and out up a shore; a wader slows linearly with depth to the kind's `wade` share of a jog at swim depth; a swimmer swimming into a bank no higher than the kind's `climb_out` over the water climbs out onto it, and never onto a higher one (`course/water.rs`, `course/dive.rs`, `course/wade.rs`, `course/ledge.rs`) | D74, D478, D523 |
| 18 | Stamina | Sprinting, jumping and swimming cost the kind's data to within 1% | D61 |
| 19 | Determinism | 10,000 ticks of the crowd course end bit-identical across runs, thread counts and the order bodies are given in | D51, D138 |
| 20 | Cost per body | Under 25 µs a body a tick on one core walking the course and the planet's ground at 2 and 30 km (`cargo xtask controller --bench`, release), under 30 µs on the 2 km planet with 10,000 edits made (`cost.edits`, D524), under 50 µs walking the `walk` scene's route among the lush yard's trees and rocks (`cost.forest`), and a crowd of 1,000 under 4 ms a tick on the server's cores | D52, D138 (a quarter of the 15.6 ms tick) |
| 21 | Feet sliding | Under 3 cm a contact at walk, jog and sprint, and in starts, stops and turns; a contact is a ball of the foot within 2 cm of its lowest over the world's ground under it, and a ball more than 2 cm in the ground is none (counted in the line) | movement.md stage 2; D455 |
| 21b | Feet on the ground | A locked foot's heel and ball within 2 cm of where its clip has them over its flat ground, over the world's ground under them (the worst plant's median; a sole straddling a riser steeper than placement pitches to rests on the higher tread, so its point over the lower is judged by the nearer of its own reading and its height over the higher), never sunk more than 2 cm, never locked with its sole more than 7 cm over where it rests (the lock curves' height gate); the shortfall foot placement adds to a leg over the lock's own under 1 cm at p95 and 3 cm at worst (the pelvis lowered for the lower foot) (`feet.ground`, `feet.sinking`, `feet.aloft`, `pelvis.drop`, `pelvis.drop_worst`) | movement.md stage 4; Unreal Foot Placement, ALS foot IK and pelvis offset; D455 |
| 21c | Stride matches speed | Each gait's clip, shown at the speed its kind moves while it plays (movers.ron's jog and sprint, against its pace: the ground it passes a second at its plants), is played within 10% of its made rate and its stride warped within 10% of its made length, with nothing left for the locked feet to stretch over (`pace.match`, `pace::stretch`); a clip made at its gait's speed passes | Unreal's sync groups and stride warping; Lyra's locomotion (a clip at its gait's speed, a capped play rate); D455 |
| 21d | Starts, stops and turns | The feet at rest within 0.3 s of the body stopping (no shuffle after the stop: `stop.settle`), no foot stepped by the rule as it stops (`stop.steps`); no foot stepped by D455's rule (a foot the clip left behind) in a straight start off standing (`start.steps`) nor in a turn of 45 deg or more from standing or slow, a start or stop that turns or a pivot (`turn.steps`): the transition's clip steps the feet (`tools/controller/transitions.ts`) | Lyra's distance-matched starts and stops and turn-in-place clips; ALS's starts, stops, pivots and rotate-in-place; D455 |
| 22 | Far from the origin | At 30 km, walking level ground, 20 deg up and down and 0.3 m steps: never off the ground nor over 10 cm (a tick at a sprint) from the same walk at 2 km, the foot's gap to the ground within 1 mm of 2 km's, and no more than 10% more field calls a tick | D482; Jolt's large worlds |

No Enshrouded movement numbers have been measured yet; combat's measurements are D180's. The QA stage should measure Enshrouded's walk, sprint and jump speeds and its step height, as the combat reference was measured, and add each one's row once it is.

## Open questions for the developers

1. **Fall damage** (D505): decided, D476 (by landing speed). Built: a landing takes a share of the body's health by the speed it meets the floor at along the floor's normal (`Kind::fall_harm`, `mover::Body::fall`), none below the kind's `fall_onset`, all of it at `fall_fatal`, by the fall's energy between (so linear in the height fallen past the onset's); water deep enough to swim in (the kind's swim depth) breaks it. The numbers are D476's placeholders in movers.ron (12 and 25 m/s); the game takes the harm from the player's health as a blow's (`terra_walker::movement::Fell`, `terra_creatures::combat::take_falls`). Rows `fall.*`.
2. **Climbing** (D506): decided, D477 (free climbing on stamina), its detail D505-D511 ([Climbing](#climbing)). Built in part (`mover::climb`, units C3-C8, on the course only): holding, starting, moving over a face by its steepness, the top, the lunge and the kick-off, stamina, grip, the stance and knocks; the course has its rows (`course/climb.rs`, `climb.*`).
3. **Swimming beyond D74** (D507): decided, D478 (dive by looking down and moving forward, rise with jump, about half a jog, about 30 s of breath). Built (`mover::swim`): a swimmer moving strokes along its look's pitch past the kind's `dive_from_deg` (15 deg for a man, so the camera's rest a little under level does not dive it), down at up to `dive` (1.5 m/s) looking straight down and level at `swim` x cos; jump held strokes up at `dive`, easing into float depth; with neither, buoyancy floats it up (Unreal's `PhysSwimming` goes along the control rotation with a `Buoyancy` share of immersion; ours floats to a depth rather than hanging neutral). Breath (`breath_s` 30) is used while the mouth (`mouth` 0.92 of height) is under, comes back in `breath_back_s` (4) at the surface, and once gone drowns a share a second (`drown_s` 10), taken as a blow's (`terra_walker::movement::Drowned`, `terra_creatures::combat::take_drowning`). Wading slows linearly with depth to `wade` (0.6) of a jog at swim depth (D523, `level_velocity`); 2.58 m/s there against a 2.0 swim leaves a 0.62 m/s step (`wade.step`, held). A swimmer at the surface swimming into a wall whose walkable top is no higher than `climb_out` (1.0 m, placeholder) over the water climbs out (D523, `swim::climb_out`): Unreal's water jump (`CheckWaterJump`: a sweep 1.2 radii ahead for the wall, a line over it at the limit 3.2 radii on), launched up just enough to clear the ledge found where Unreal launches at a fixed `OutofWaterZ`; no climb-out clip yet. A small breath bar over the vitals shows the mover's breath while the mouth is under or the breath refills, fading out once full (D523, `terra_survival::breath`). Still open for the developers: the placeholders and swimming's stamina (criterion 18).
4. **Bodies blocking bodies** (D508): decided, D479 (people softly, monsters hard). Built: step 6, rows `push.*`. In the game players ease apart from each other and from recruits (`terra_walker::bodies`, who give way by themselves until people walk the mover); creatures still stop a player at their width, as they are not the mover's bodies yet.
