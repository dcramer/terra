# v0 tracks

Each of [v0's](v0-goals.md) tracks (D589): what it has, what it needs and when it is done. The tracks, the gaps, the numbers and the order are the agent's and change as the work teaches; which tracks wait on which, and the order they run in, are in [v0-goals.md](v0-goals.md#tracks). A job of one track reads only its section.

## Front (1)

- **Have**: the title screen (D579, `--title`): Continue, Worlds, New world, The yard, Settings, Quit. Several worlds, each with a name, mode and seed (rolled, or typed; D596), kept on disk. Deleting asks first. Quit to title leaves the world in place, with no new process (launcher.md). Host and Join (D639): a world hosted from its list, a friend's joined by address, the host's land made from its welcome, and every way a session ends told on the title screen in words (`node tools/sandbox/join.ts`). The creator (D111): a Character page of hair, beard, hair colour, skin and eyes for the man (`node tools/sandbox/creator.ts`), each player's features seen in the other's game (`net::looks`), so two players tell each other apart.
- **Need**: nothing for v0. Later: the second sex, set aside by the developers on 2026-11-07 (D111): her body is in the wardrobe and a player stands on the body their features name; she is offered once her hair is settled (the pack's is made on the man's head, D599).
- **Done when**: a scripted run makes two worlds from different seeds, sees different land, and returns to the title and loads the first, in one process. It does: `node tools/sandbox/title.ts`.

## Realms (2)

- **Have**: `terra_world::instance` built: states, scoping, leaving, the census, crossing in one frame, and a round-trip test with no GPU. Most modules are built on entering.
- **Need**: asset sets (D471). The save holds the character's record and each world's (D471, built); not yet in it: what was seen of a world's map. Warming as you approach. A crossing between full worlds: 8 s warmed, 19 to 24 s cold (`gate.ts`); the ground is left. Crossing a shared world (refused in words until D640's instances; the gate's state is shared). The map, the soundscape, a base's fire and the return stack are built.
- **Done when**: instances.md's test passes: five round trips with the census equal and memory within 2%; the cover under 3 s (D501), aiming under 1 s for a warmed realm. Measured 2026-11-07 (`node tools/sandbox/portal.ts`, which holds the 3 s): crossings 2.2 to 2.9 s (one of ten at 3.03 s before the last cuts); counts equal every trip; memory within 2% in two runs of three (2.07% once).

## Realm two (2)

The proof that a realm is data. instances.md's playground proves loading and nothing else; this proves a world of another theme comes out of the same rules. It needs enough of each part to show that, not a full realm's worth.

- **Need**: a theme, which someone settles later; until then Realms works with instances.md's placeholder realm. Then its land recipe, ground materials, plants, sky and light, wildlife, monsters and sound, each by the existing pipelines and the art style (D14). A realm's parts are new; the rules are not. Survival's rules are the same in both realms, as the developers confirmed: the realm file can hold different air, cold and gravity (D61, D63), and v0 differs only by its climate.
- **Done when**: stills and a film pass review beside the first realm, and a stranger could tell them apart at a glance while seeing one game (D14). The generator states how many variants it makes (D182).

## Land (3)

- **Have**: smooth voxels, dug and saved (`terra_voxel`). The land by its rules (`terra_land`). Forest, mountain lake, tundra and town, made as demos on the 30 km world at places their queries found. The game is D13's 30 km world (the developers, 2026-11-04): every world made from the title screen or `cargo xtask play` is made at it, on its own seed, its start searched for (`terra_land::spawn`), and keeps its radius with its save; a scripted run keeping no world still stands on the 2 km test planet its tools are measured on.
- **Need**: mountains and caves in the first realm's 30 km; the second's size comes with its theme. The scripted tools and bench scenes moved off the 2 km test planet onto the game's world. The generator holding up on *any* seed, not on the demos' pinned places: a walkable start, biomes reached by walking, a blend between them, the gate reachable. Natural caves (D591): generated, streamed from below as well as round the player, lit underground, with a camera that works in tight space and a chamber that fits a 4 m troll on every seed. The moving origin (D36), which is not built, though the mover is already in 64-bit (D482). Streaming and load times at that size.
- **Done when**: a sweep over at least 100 seeds (`cargo xtask fields`, by CPU, `terra_gen::par::map`) finds every world valid, a walked film crosses three biomes with no visible seam, and another goes from the surface down into a cave. Terrain still digs everywhere.

## Living world (3)

- **Have**: the climate's fronts, rain, snow and wind as fields (D314, D315). The weather at the eye (D418). Volumetric clouds on high (D515). Snowfall and falling leaves. One wind for grass, bushes and trees, gusts travelling across them, through one shared buffer (D255, D374), so wind already costs little per plant.
- **Need**: weather.md's stages from the storm on: storm cells, falling rain and lightning (none are built), puddles and rain on water, then a whole winter, then a front arriving in one take. Plants for every biome at every distance, the far forest included. The sky the same for every player, which D540 makes nearly free: clock and seed are all that travels.
- **Done when**: a clear-to-storm film with no visible step passes review in each of three biomes, on both realms' terms, and frame time holds in the worst weather on the 3080 (D25).

## Body (4)

"Fully implemented" needs a list, or it never finishes. Proposed:

| Move | State |
|---|---|
| Walk, jog, sprint, jump, fall, land, stairs, steep ground, swim, dive, pushing | In the game on the mover (D53) |
| Dodge | In the fight |
| Starts, stops, turns, lean into turns, blends by speed and direction | Under way: the animation scorecard (`cargo xtask anim`, D631, the anim skill) |
| Climb, pull up, lunge | The mover has them; off in the game until the figure has clips (D477) |
| Crouch or sneak | Not built; waits for gameplay |
| Prediction: the client moves at once, the server corrects | Not built; the mover's plain, fixed-tick code is ready for it (D138) |

- **Done when**: `cargo xtask controller` and `cargo xtask anim` show every row passing, a before-and-after film earns the developers' yes (animation.md), and the same walk feels right at 100 ms of latency.

## Animation (5)

The people, now and next: [v0-characters.md](v0-characters.md).

- **Have**: the method (animation.md, D584, D585). People by the character pipeline (D115), moving onto Quaternius's bodies and outfits in our paint (D599). Sixteen animal kinds on two body plans, their skeletons placed by hand (creature-rigs.md).
- **Need**: creature-rigs.md's order of work: the creature check, body plans as data, skeletons fitted to the mesh, weights, paces from size. Then one template body for each plan monsters need. The man and the woman on D599's bodies, by its jobs, before the player's locomotion and fight sets are finished on them, one job a body at a time. Which plans come first is D436, how monsters move D438, and blows from a pose sheet D588.
- **Done when**: for each plan v0 uses, a new creature goes from a concept to the game with no hand edits, passes the creature check, and scores 7 or more on review (D437). The time and credits a creature takes are reported, since that sets how many a realm can have.

## Creatures (5, a gap in the list)

An animation pipeline gives a body that moves; nothing in the list says what it chooses to do.

- **Have**: wildlife with routines, startled by rule, put out in cells as a player nears (D380, D384). Spiders at a place where magic pools. The fight's rules, and the duel.
- **Need**: behaviour as data: sensing, hunting, fleeing, packs, each kind's attacks and tells, leashing back home. Finding a way over dug ground, which has no solution yet for creatures. Spawning by a realm's theme and where magic pools (D59, D103). All of it on the server, with cells shared between players rather than kept round one eye.
- **How many**: enough hostile and harmless kinds to prove each body plan in each realm's theme. The count is what the pipeline can carry, once a creature's cost is measured.
- **Done when**: a new kind's behaviour is a data file, a duel run per plan passes, and two players at opposite ends of a pack see the same fight.

## Server (6)

The largest risk. The design is [netcode.md](netcode.md) (D34, D651, D652, D636), from the survey in research/2026-11-06-netcode.md; `terra_net` follows its build order.

- **Have**: the fixed tick (D138), rules in plain crates (D28), operations in order (D21), the same world on every machine (D30). `terra_net` (netcode.md's build step 1): the wire, the transport seam with an in-memory transport that shapes latency and loss from a seed, acknowledgements, interest by area, and the server's and client's ends over entities of parts and ordered operations; engine.md's test passes on a toy game, and four clients soak for 30 simulated minutes at 100 ms and 1% loss with no lasting mismatch (6.2 kB/s down, 2.4 kB/s up a player, readings). Step 2: the client's clock and prediction by replay, and two men walking together on the real mover at 100 ms and 1% loss with every predicted tick the server's (`terra_motion::mover::net`). Step 3: the same two ends over UDP on `renet` 2 (the `udp` feature), a game of another version refused in words, and an identity proven by a signed challenge (D592). Rooms and a crossing between them, and one-shot events heard once by those near. Step 4's first part: the walker layer hosts (`terra_walker::net`), a guest walked by the world's own walk and told its body and the host's, tested by a client that is no game; and a joining game's side, its own player predicted by its world's walk and corrected by replay, the others puppets, tested by two worlds side by side; and in the running game, hosting and joining by the remote protocol, each player drawn in the other's game, checked by `node tools/sandbox/net.ts`; and the ground: the host's operation log is the truth, a joined game's digs asked of it and held to the player's reach; and what each player wears, shown on its guest and its puppet.
- **Need**:
  - The design built (netcode.md): written 2026-11-06 for the developers to read (D49); D34 stands on its survey.
  - netcode.md's rest (Host and Join on the title screen and the world's seed from the welcome are built, D639): the host's own player a client (D638), and the fight, pack and creatures across the seam (step 5). The same server running with no window is for scripted runs now (D51); offering it to players comes later.
  - **Always a client.** Even solo, the drawn game is a client of a server. Otherwise there are two code paths. Until the instance host exists a hosted world is its own server, the interim (D638, Decided: the instance host and the host's player as its client are the next Server work).
  - **Two realms at once.** One player can be in each realm, so a host's server runs both while its own client draws one. instances.md's split (a headless `App` per instance on the server, the client's world in the main `World`) means a host holds its realm twice. Measure that memory before building on it. Measured (D640): an empty realm of the walker's layer is 10 MB and 12 us a tick; the game hosts through it once a realm holds trees, props, bases and creatures.
  - Sending by area, a snapshot then changes, others shown between ticks, late join, leaving and rejoining mid-fight, and a version check on connect.
  - Each player's character kept by the server (D592) under an identity, a key kept on the player's machine. Direct address first, Steam's relay later (D34). When the host leaves, the session ends for everyone with a plain message and the world is saved, as the developers confirmed; there is no handing over to another player.
  - Every action checked: in reach, owned, allowed (D51). Built for the ground's changes (reach); the pack's and a base's are Play's and Building's.
  - Text chat and a player list: built in the protocol and by the remote (`net.say`, `net.state`'s players); what shows them on screen is the game's to design.
- **Done when**: engine.md's test passes (two peers end the same, a late joiner catches up), then four clients for 30 minutes at 100 ms and 1% loss show no lasting mismatch, with the bytes per player per second reported. Both pass in one process on the in-memory network, on a toy game and on the real mover (3.1 kB/s down, 1.3 kB/s up a player), and the second against the running game: a hosted copy and four players with no game behind them over UDP for 30 minutes at 100 ms and 1% loss, each ending where the host had it (`node tools/sandbox/net.ts --soak 30`; 4.7 kB/s down, 3.7 kB/s up a player by the carrier's count).

## Sound (7)

- **Have**: `Noise` as a fact about the world, the mix with its levels, `Footsteps`, `Ambient` within earshot, recorded and generated sounds, blows by what met what, loudness checks (D113, D447). Bevy's spatial listener on the camera: left and right, with volume by distance.
- **Need**: a choice of engine, since Bevy's has no reverb or muffling. Candidates: `bevy_seedling`, already named in audio.md, Steam Audio, and FMOD or Wwise as paid options (D526); the first job checks each against Bevy 0.20 and reports. Then, by the developers' three words:
  - **Directional**: front from back and above from below, not only left from right.
  - **Distance**: a curve per kind, far sounds losing their highs and arriving late (thunder at 343 m/s, as weather.md wants).
  - **Ambient**: a bed per biome and realm, blended by place; caves and rooms sounding like themselves (audio.md's question 5), which caves bring into v0 (D591).
  - **The pipeline**: a clip goes from a source to the game through one step that names, measures and checks it. Others' noises arrive from the server.
- **Done when**: a test renders a moving source to a file and measures its level, side and tone against position, with no person listening; then the developers approve a listening film in each realm.

## Play (8)

- **Have**: backpack, equipment, action bar, quiver, wear and repair, crafting at stations, gathering by tool, skills by doing, hunger, death and the dropped pack, the map (prototypes.md). Sword and shield and the bow on the combat scorecard (D452).
- **Need**: the baseline as engine, not as balance: every change to an inventory an operation the server checks, items and gear carried across a crossing, death and return working in either realm, and what another player sees of your gear. Which items, tiers and numbers exist is gameplay, for later.
- **Done when**: two players trade blows with the same monster, swap items through a chest, die and recover their packs, with nothing duplicated or lost across a crossing or a disconnect.

## Proof (a gap in the list)

- **Need**: headless clients that log in and play by script (multiplayer.md's agents as players), `cargo xtask loop` for several clients, latency and loss on demand, and the v0 session as one command. A Windows build a friend can run, with a crash report they can send (D399). Windows is the first build, not a limit: nothing is built to depend on it, and anyone can add another later. One build for a whole playtest, since a save of another version is refused and stays so until further notice (D353).
- **Done when**: [v0's session](v0-goals.md#what-v0-is) runs unattended on main, and someone not on the team joins a game from a build they were sent.
