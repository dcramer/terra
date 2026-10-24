# Movement and fighting

Status: **proposal, 2026-10-17**, for the developers to read before anything is built (D49). The architecture is technical and mine to propose; the questions at the end are design, Open, for the developers. Nothing here changes a decision yet. The research behind it is [research/2026-10-17-movement-and-fighting.md](../research/2026-10-17-movement-and-fighting.md).

How every body in the game moves and fights: the player, people, recruits and monsters from a hare to a dragon, on one system, as shipped action games do it.

## Why a step back

The developer, after five rounds of the troll fight ([troll-fight.md](troll-fight.md)): "troll is still terrible, both looks and mechanics of our guy, some texture clipping, some movements not working, you need to think in terms of how to design moving system for both our character and monsters." The fight had no movement system under it: a clip player (`terra_creatures::staged`) and a script that placed every figure by hand. Each round rebuilt one piece of a game's animation system inside the script (root motion, contact frames, hit reactions, a retarget pose) and the next review found the piece still missing (the research note's first table maps each symptom to its cause). The game's own player plays one clip at a time with clip names in code (`terra_walker/src/figure.rs`), and a swing does its damage on the click (`terra_creatures/src/combat.rs`). Both need the same thing: one system, built once, that a player, a mind and a film all drive.

## Constraints

- **Enshrouded is the baseline** (D180): fast, smooth, a move starting the moment it is pressed, chaining and cancelling into a dodge; a fight is timing and footwork. Third person (D97).
- **Where a blow lands matters** (D119): hurt zones on the body, hit volumes on the weapon. **Damage shows on the body** (D132): reactions, not numbers.
- **Realistic but fun** (D122): weight and momentum read true; responsiveness wins where realism would be a chore.
- **The server decides** (D51) and **shared state stays out of Bevy** (D28): where a body is, what move it is in, and whether a blow landed are simulation; the pose is presentation.
- **Many of everything** (D127) and **kinds as data** (D136): a thousand players, crowds of people, packs of monsters; a new creature or move is a file, never code.
- **One skeleton for people** (D115), clips made with Kimodo (D316) and carried by aim (D360); creatures that are not people are animated by rule over their own skeletons (creatures.md).
- **Layers** (D198): what a body does to move belongs to the walker; moves, hits and reactions to gameplay.

## The layers

Every moving body runs the same pipeline each tick, its numbers from its kind's file. The top three are simulation, Bevy-free and run on the server (a new crate, `terra_motion`, D28); the rest are presentation on the client, in Bevy.

| # | Layer | What it does | Shipped model |
|---|---|---|---|
| 1 | **Intent** | What the body wants: a velocity, a facing, a look target, a move asked for. Written by player input, a creature's mind or a scene; the only way in | Every engine's controller input |
| 2 | **Mover** | The body in the world: a capsule moved by code toward the intent, with its kind's speeds, acceleration and turn rate, stopped by ground, walls and other bodies. Inside a move, the move's root motion drives it instead, warped toward its target | Holden's code- and data-driven displacement; Unreal's Character Movement and Motion Warping |
| 3 | **Actions** | The move runner: which move a body is in and how far, its phases (wind-up, active, recovery), its cancel windows, an input buffered into the next window, poise through a heavy wind-up, and what a hit does to the one struck (flinch, stagger, knockdown) | Fighting-game frame data; FromSoftware's event tracks; God of War's cancels |
| 4 | **Pose** | The skeleton's pose: locomotion by blend spaces (speed, and direction against facing) in a small state machine (idle, start, move, stop, turn, fall, land); the move's clip over it, whole-body or upper-body by mask; inertialised transitions, so the new pose shows at once | Godot's AnimationTree; Bollo's inertialisation |
| 5 | **Fix-up** | After the pose: feet on the ground with a pelvis offset; the off hand on a two-handed grip and the shield arm on its strap; head and spine turned to look and aim; held things pushed out of the body's own capsules; additive lean and hit flinches; spring bones | Two-bone and look-at IK (Godot, ozz); additive layers |
| 6 | **Physics** (later) | Bodies pushed by blows through motors that pull toward the pose, and ragdolls for the dead | Unreal's Physical Animation, Jolt's ragdoll drive, Euphoria |

What each layer must not do: the pose never moves the body (only the mover does, from intent or root motion); a scene never sets a pose or a position (it writes intents); a hit never starts by a timer (only by a hit volume meeting a hurt zone in an active frame).

### Root motion

Locomotion is code-driven: the capsule moves as the input asks, at once, and the legs are made to fit it (playback rate by pace, which `pace.rs` does now, then stride and orientation warping). Moves are data-driven: a lunge, a dodge, a stagger or a heavy blow moves the body by its clip's own root, extracted from the hips at export, so the travel is part of how it looks. A move may bend its root motion inside a window marked on it, up to its own limits of distance and turn, so a blow ends where the target is (Motion Warping): this replaces the troll scene's placing of the attacker by hand.

### Bodies that do not pass through

Three kinds of volume on every body, as fighting games name them, all data from the kind's file:

- **Pushbox:** one upright capsule, separated from every other body's each tick, the heavier giving way less. A troll shoves a man aside; a man cannot walk into a troll.
- **Hurt zones:** capsules on bones (head, body, limbs; D119), carrying how much a blow there counts.
- **Hit volumes:** in a move's active frames, a capsule from hilt to tip of the held weapon (from its socket and its length), or along the striking limb, swept from last tick's place to this one's so a fast blade cannot skip through.

For the server, a move's hit volume and its mover's hurt zones are baked at export into short tracks in the body's root space, so a blow is judged without a skeleton. Gear stays out of its own body by a retarget pose per body baked into its clips at export (the proper home of today's `staged.bend`), by hand IK, and by the penetration check below. Two bodies that must touch (a grab, a finisher) do so only through a paired move made for both and aligned before it plays.

### Creatures of every size

One pipeline, a few body plans (creatures.md): people and things shaped like people on our skeleton (a troll, an ogre); the four-legged runner and the bird that `tools/creatures/animal.py` already makes by rule; a many-legged walker whose feet step by rule when they stray past their reach, in alternating groups, the body riding the plane of its planted feet; a crawler whose spine follows its head; fliers (D382). Size is a scale and changes numbers, not code: timing by Froude scaling (a body twice as tall plays its clips about 1.4 times slower, its stride covering about 1.4 times more ground a second), turn rate and acceleration falling with mass, longer wind-ups and recoveries, more poise, and the pushbox's weight. Big bodies keep their feet on the ground by IK and turn in place by clips; they never slide or spin on the spot.

## Moves as data

A body's kind names its **move set** (`assets/data/moves/*.ron`): locomotion clips by gait, and its moves. A move says:

- its clip, and whether its root motion drives the body;
- its phases, **measured by the tools** from the clip, never typed: wind-up from the clip's start of motion, active frames from when the weapon's tip (followed through the skeleton) sweeps forward of the body, recovery after;
- its cancel windows (into a dodge, a block, the next move in a chain), its hit-stop and its poise;
- its warp limits (how far and how much turn toward a target, and in which frames);
- what it strikes with (a held item's socket, or bones), its damage and stamina, its sounds and effects by name.

A **reaction set** per kind: additive flinches per zone, a stagger, a knockdown, a death, chosen by the blow's poise damage against the body's poise. A new move is a Kimodo prompt (`art/animation/sets/*.json`, made to start and end in its set's guard pose) and a few lines; a new creature is a body, a plan, a size, its volumes and its move set.

## What we port, build and keep

| Part | From | How |
|---|---|---|
| Graph, blends, masks, additive, clip events | Bevy 0.20 | Use as is |
| State machine, blend spaces, root-motion accumulation | Godot 4.7 AnimationTree (MIT) | Port onto Bevy's graph |
| Root extraction from the hips | ozz-animation 0.17's motion extraction (MIT) | Port into the export tools |
| Inertialised transitions | Bollo (GDC 2018), Holden's articles | Build from the papers; replaces `AnimationTransitions` |
| Two-bone, look-at and aim IK, foot IK and pelvis | Godot's SkeletonModifier3D, ozz's samples (MIT) | Port, or take `bevy_skeleton_modifiers` (MIT, a port of Godot's) once it is on 0.20 |
| Motion warping, stride and orientation warping | Unreal (documentation only) | Build from the documentation |
| Move phases, cancels, frame data | Fighting games, FromSoftware's event tracks (documentation only) | Build |
| Physical animation and ragdolls (later) | Jolt 5.6's ragdoll drive (MIT), on whatever physics D33 settles on | Port the method |
| Motion matching (only if measured short) | O3DE's MotionMatching gem (Apache-2.0 or MIT), its data from Kimodo | Port |
| Spring bones, pace, our skinning, clip and gear checks | Ours | Keep |

Motion matching is not the first step: it needs tens of minutes of consistent motion per body and style, and costs more per character than a blend space, for a thousand players. Kimodo could make that data, so it stays a later option if blend spaces measure short.

## Variants (D182)

Move sets by weapon kind and combat style (D124): about ten kinds (sword, sword and shield, axe, spear, greatsword, hammer, dagger, staff, bow, unarmed) by two to four styles, each six to ten moves: about 250 moves, told apart by silhouette, reach and rhythm. Creatures: five or six body plans by four size classes, each kind with its own few moves and tells: a few dozen distinct movers, told apart by gait, weight and how they wind up.

## Checked without a person (D69)

- **Clip and move checks**, in the build: the root extracted (no hips travel left in a move); a move starting and ending in its set's guard pose; its measured phases and reach written beside it; feet sliding no more than 3 cm a contact at the clip's pace.
- **A penetration ratchet**: every move on every body, frame by frame, how deep a hand, a held item or a worn piece goes into the body's own capsules, kept from rising as the gear check is.
- **A move sheet** on the workbench: each move from the side and above, its hit volume's sweep drawn and its phases marked.
- **A duel run** in the sandbox: any two or more kinds, driven by scripted intents (never clips), headless, logging every move start, contact, hit and miss, reaction, pushbox overlap, foot slide and gap between moves asked for. It fails on a blow in range that misses at its active frames, bodies overlapping by more than 5 cm, a move starting more than a frame after it is asked, or an idle where a chain was asked for. A film is the same run with a camera.

## Open questions for the developers

1. **Commitment.** Can most moves be cancelled into a dodge or block late in the wind-up and in recovery (Enshrouded, Devil May Cry), or does a swing commit you until its recovery, as in Souls games? Recommended: light moves cancel, heavy ones commit, a dodge out of any recovery. Open.
2. **Lock-on** (combat.md's question 3). Recommended: a soft lock that warps your blows toward the nearest enemy you face, and a hard lock on a key. Open.
3. **Blades that stop.** Does a blade bounce off a shield, stone or heavy armour and stagger its wielder (For Honor, Kingdom Come), or pass through with a hit-stop? Recommended: bounce on shields and stone, hit-stop on flesh. Open.
4. **Bodies blocking bodies.** Do players push each other and their recruits, or pass through? Recommended: players and recruits push softly, monsters hard. Open.
5. **Paired moves.** Finishers, grabs, a monster seizing a player: recommended few, later, as paired clips. Open.
6. **Climbing big monsters** (Shadow of the Colossus, Dragon's Dogma): recommended not for now; it makes a body a moving surface. Open.
7. **Deaths and knockdowns**: by clip now and physics later, as recommended, or physics from the start? Open.

## Order of work

Each stage is done when its checks pass and a duel run or film shows it; the troll fight is reshot on stage 3, not before.

| Stage | Builds | Done when |
|---|---|---|
| 1. Clips fit for a game | Root extraction at export; a retarget pose per body at export; phases and reach measured from the weapon's tip; guard poses at start and end | Clip and move checks pass on the man's and the troll's sets; no hips travel left in a move |
| 2. Mover and pose | `terra_motion`'s intent, mover and pushbox; blend spaces and the state machine; inertialised transitions; the player's figure and people moved onto it | The walker walks, jogs, starts, stops and turns with feet sliding under 3 cm; input to motion within one frame |
| 3. Moves and contact | Move sets as data; the move runner with phases, cancels and buffering; root-motion warping; hit volumes, hurt zones, hit-stop and reactions; `terra_creatures::combat` and `terra_creatures::staged` on it | A duel run of the man against the troll passes; the troll fight reshot by intents |
| 4. Fix-up | Foot IK and pelvis, hand IK on grips and shields, look-at and aim, additive lean | The penetration ratchet at zero for shield and off hand; feet on slopes within 2 cm |
| 5. Creatures | The big biped, the runner and the bird on the same pipeline; the many-legged walker; size by scale | A duel run per plan, at three sizes |
| 6. Many | Animation and IK by distance; hits judged on baked tracks on the server | The crowd benchmark with fighters, measured on both GPUs |
| 7. Physics (after D33) | Physical animation on hits, ragdolls | Measured against the clip reactions on the duel run |

## Prototype (D405)

Before the layers, a playable sword-and-shield prototype in `terra_creatures` (`melee`), for the developers to feel and give feedback on: in the yard (`cargo xtask yard`) or `make play`, T draws a sword and shield and brings a sparring partner. Its moves are data (`assets/data/moves/sword_and_shield.ron`) run by the move runner with phases, cancels and a buffer; active frames typed from `tools/characters/phases.ts`; blades swept against hurt capsules. Shortcuts the layers replace: drive curves stand in for root motion; a guard is judged by facing, not the shield's volume; one hurt capsule a body; a clip picked by the dominant direction, no blend space; the build keys reused in stance (left click, right click, Q, Ctrl); the sword and shield given on drawing, not from the inventory; the spar's health apart from Vitals.

### Second round

The developer's first play, 2026-10-24:
1. The hands sit inside the body, even at idle.
2. A swing goes to the left, not ahead.
3. The props are hung wrong: the partner's shield goes into its head when it guards.
4. The frame rate is atrocious.
5. The sounds make no sense.
6. Rolling, jumping, swinging and blocking cost no stamina.
7. Nothing chains.

They added: "we probably need to build better tools so you can ensure things are correct".

| What was seen | Cause, as found | What checks it |
|---|---|---|
| 1. Hands in the body | Measured (`moves.ts`): blows put a hand or arm 11 to 22 cm into the torso, but the guard's arms stand clear and its shield is 7.6 cm into the head, so the idle seen may be the shield arm; a still is to confirm it. The Kimodo fight clips are carried onto the base body by bone rotation, not by aim (`aim.py`, D360). The base is broader than the clip's source, so the arms close into it | Penetration: hands, forearms and held looks against the body's own capsules, every frame of every move |
| 2. Swings to the left | The clips themselves: the slash is cut "high right to low left", and the backhand's tip is fastest 1.40 m to the left (`phases.ts`). A prompt does not set a blade's path | Arc: the tip's path through the active frames must cross ahead of the body |
| 3. Shield in the head | The shield hangs on `socket.hand.L`, not on the forearm a strap holds | Held fit: the sword's grip along the hand, the shield's face out from the forearm, both in the penetration check |
| 4. Frame rate | Not found offscreen: the yard on the RTX 5090 at 1920x1080 runs 7.3 ms median in release with the sword drawn and the partner fighting, 5.4 ms in dev (`cargo xtask yard`'s profile), with one 31 ms frame as the partner spawns. Still to measure: windowed play with vsync, and the integrated GPU | The yard benched with the sword drawn |
| 5. Sounds | A block plays the anvil and a parry the bell (`terra_game/src/play/sounds.rs`), and the swing is a made whoosh | The duel run logs each cue it plays |
| 6. Stamina | Drawn, the walker's roll gives way to the stance's, which cost nothing, nor did blows or a guard (against D61); the jump always paid its 8. Now each move's cost is a field of its set (light 10, heavy 22, roll 18), a blow on the guard costs a point a point of its damage, the partner has its own pool, and out of stamina is a placeholder (D449, Open) | The duel run logs stamina; tests of the costs, the out-of-stamina rules and the jump and roll drawn |
| 7. No chain | The finisher reuses the slash's clip, and no move starts or ends in the set's guard pose, so a chain is a cross-fade between unrelated poses | Chain: one move's last pose against the next one's first; the duel run fails on an idle where a chain was asked for |

**The goal**, from the developers on 2026-10-24: "our main goal is fluid combat mechanics, albeit limited in scope right now, visually appealing, with something like enshrouded being our measuring stick for what a reasonable combat system looks like in the context of our game", and "toolchain/pipeline is most important here, because you're an agent and we're newbs". So the pipeline comes first. A move goes from its spec to a judged film by commands an agent runs, and each step fails on its own. It is the loop the environment skill uses (a measured reference, fixed views, checks, a scored review):

| Step | What it does | Fails on |
|---|---|---|
| Reference | Enshrouded's combat measured from first-party trailers and footage: a light blow's length, the cadence of a chain, wind-up, hit-stop, the roll's length and time, how fast a guard rises, a stagger, how close fighters stand, and the camera's distance and framing. The numbers and their sources are committed; the pictures are kept out of the repository | — |
| Clip | One command a move: its spec (start and end in the guard pose, the way its blade sweeps, its length) made from the chosen source, carried onto the base body by aim and exported; its phases written into the move set from the clip, never typed | `moves.ts` |
| Rules | `terra_motion`'s duel test; our timings against the reference's ranges | The duel test; a timing check |
| Look | A scripted duel in the yard, filmed from fixed cameras and the player's: a frame sheet and checks on the film (feet sliding, a gap where a chain was asked for, bodies overlapping) | The film's checks |
| Review | A blind adversarial review of the sheet against Enshrouded's frames, scored out of 10; stopped at the bar, or after two rounds without a gain | The score |

A `combat` skill in `.agents/skills` ties these together, so any agent runs the loop the same way.

The order of work, each round bounded, the code split as D447 says:

1. **Checks first**:
   - the penetration, arc, held-fit and chain checks over the move set, in `tools/characters`;
   - the duel run, as a plain test;
   - the yard benched with the sword drawn.

   All of them run on today's clips for a baseline.
2. **What waits on no clip**, side by side:
   - the frame rate, fixed by what the bench shows;
   - stamina, as move-set data, with the roll and jump paying again;
   - sounds by what is struck (a wood or metal shield, flesh), and a swing heard by its speed.

   The runner's rules move to `terra_motion` here.
3. **A bake-off of where clips come from**, scored by the checks on the same seven moves:
   - Kimodo with constraints, if it takes them;
   - a CC0 capture library;
   - blows made by us, from key poses along a blade arc;
   - a commercial set, as the reference only.
4. **The set rebuilt from the winner**:
   - every move starts and ends in one guard pose;
   - three distinct light blows that chain, a heavy blow, block and parry, the roll, strafes;
   - clips fitted to the base body;
   - the shield on the forearm.

   Done when every check passes and the duel run passes. Then the developers play it again.

## What happens to what exists

`terra_creatures::staged` keeps its remote calls but tells figures intents and moves, not clips and positions; its bends move into the export and its hips bookkeeping into the mover. `terra_walker/src/figure.rs`'s clip names become the player's move set. `pace.rs` is the pose layer's speed matching. `swing.rs` stays as the spring-bone fix-up. `tools/creatures/animal.py` keeps making the runner's and bird's clips, played through the same layers. Kimodo stays the source of clips for people (D316), made to each move's spec.
