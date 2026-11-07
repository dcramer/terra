# Netcode

Status: **designed, 2026-11-06; build steps 1 and 2 built** (D34, D651, D652, D636): `terra_net` passes milestone D and the soak on the in-memory transport, and two men walk together on the real mover, each predicting its own body. Step 3 is built too (`cargo test -p terra_net --features udp`): the UDP transport, the version check, the refusals, and an identity proven by a signed challenge. Of step 4 the host's side is built for the walker layer (`terra_walker::net`): a hosted world's own walk moves a guest by the intents its client sends and tells every player's body; and a joining game's side (`terra_walker::net::link`): its own player predicted by its world's walk and corrected by replay, the others puppets. The running game hosts and joins by the remote protocol (`net.host`, `net.join`, `net.state`), guests and puppets are drawn as players in their body's look, and `node tools/sandbox/net.ts` checks two copies against each other. The title screen's Host and Join are not built; a joined game must be started on the host's seed, and is told so and leaves if it was not; the host's own player is not a client of its server (D638); of step 5 the ground and what is worn cross; the pack, vitals, the fight and creatures do not. How the server of [multiplayer.md](multiplayer.md) (D51) talks to its clients: what is sent, when, and how a client stays smooth. The survey behind it is [research/2026-11-06-netcode.md](../research/2026-11-06-netcode.md). It serves v0 (a host and a few friends, four clients at 100 ms and 1% loss, two realms on one host) and keeps D52's thousand possible: nothing is sent to everyone, and every cost is per area.

## The model

As shipped server-authoritative games do it (Source, Overwatch, Unreal, Tribes and Halo for what is sent):

- **The server decides, on D138's 64 Hz tick.** Ticks are numbered. A client sends intent; only the server's tick changes shared state (v0's third seam).
- **Your own body is predicted.** The client runs the same mover on its intent at once, keeps what it predicted, and when the server's answer for a tick differs, sets the body to the answer and replays the intents since (D651).
- **Everything else is shown in the past**, between the last two states received (snapshot interpolation), about 62 ms back.
- **State is sent as the most recent, never as history**: unreliable, by area, by priority, within a byte budget a packet (D652).
- **World changes are operations in the server's order** (D21, D29), reliable and ordered, kept per area so an arrival is sent the area's record, not the world's.
- **Always a client** (D51) is the aim: solo, the game a client of a server in the same process over the in-memory transport, as Minecraft's single player has been since it moved to an integrated server. Until realms run as instances a hosted world is its own server, a listen server (D638): the host's player is walked by its own keys, guests by their intents, by the one walk.

Not lockstep, though D30 makes every machine compute alike: a drop-in world cannot wait for its slowest player. D30 instead makes predictions right nearly always.

## The crate

`terra_net` is plain Rust with no Bevy and knows no game type (D28). It gives the machinery; each rules crate says what its own state looks like on the wire.

| Module | Holds |
|---|---|
| `wire` | A bit packer and the `Wire` trait (D636): ranged integers, quantised floats, varints. The reader is total: any bytes give a value or an error, never a panic, and every length is capped |
| `transport` | The seam (D34): two lanes, `Ordered` (reliable, in order) and `State` (unreliable), as a server's and a client's trait. `loopback`: in memory, with latency, jitter, loss, duplication and reordering drawn from a seed on a clock the test advances, so a run repeats exactly. `udp` (a feature): `renet` with `renet_netcode`. Neither reads a clock: the caller says how much time passed |
| `ack` | Sequence numbers with an ack and a 32-packet ack field in every `State` packet (Fiedler), ours because the transport's unreliable lane reports none |
| `clock` | The client's estimate of the server's tick and how far ahead of it to run |
| `interest` | Which cells a client is sent, on a hashed grid of cubes over the realm, round or under ground alike |
| `server`, `client` | The two state machines: connect, entities, operations, intents |
| `predict` | The history of a predicted state and its replay |

**A thing on the wire is an entity made of parts.** An entity has an id the server gives, a kind (data, D136), a place (for interest and priority) and a few parts, each a small versioned record a rules crate defines and encodes: a body's motion, its stance, what it wears, its health. The server encodes a changed part once a tick and shares the bytes among every client it goes to, so the cost is by what changed, not by clients times entities. A part is public or its owner's alone (your own inventory, your body at full precision).

**Rooms.** A connection is in one room, a realm instance (instances.md); entities, interest and operation records are a room's. A crossing moves the connection and its character between rooms on the server: the client is welcomed to the new room as it was to the first, holds nothing of the old (state stamped before the crossing is dropped), and is sent the new room's records from their start. State of the new room may reach a client before the welcome to it (the lanes are apart): what is stamped from the welcome's tick on is the room's and is kept. Built, with a round trip tested.

The Bevy side is thin: the server and the client each sit in a resource, a system before `Sim::Act` writes each player's `Intent` from what arrived, one after `Sim::End` writes changed parts and sends. Each layer registers its own parts (D198): the walker a body's motion and look, survival what is carried, creatures theirs. A bot or load-test client uses `terra_net::client` with no Bevy at all (D73).

## Ticks, intent and prediction (D651)

- **Intent**: a client sends a `State` packet every second tick, as the server does, carrying its intents for the last six ticks, so each goes in three packets and two lost in a row lose nothing (every unacknowledged one every tick, at first: seven a packet at 100 ms, 13 kB/s up a player in the game). A view and an aim's direction go to twelve bits an axis, and both ends act on them as they read back. The server keeps a small buffer a player and takes tick T's intent at tick T; if it is missing it repeats the last (Overwatch).
- **Clock**: the client runs ahead of the server by half the round trip plus a margin, so its intent arrives just before it is needed. What is held to four ticks (the next tick's intent, a spare, and the two a lost packet's intents come late by) is the lowest margin of the last second, not the usual one: a game drawing slowly runs several ticks a frame, and the last intent of a burst is the one that comes late (holding a mean cost 160 corrections in a walk on a loaded machine; holding the lowest, one). A margin over four ticks out is jumped to, the whole way on the first reading; later only when readings are late several times running or the lowest of half a second is still early, since a jump voids a dozen ticks of prediction and one late reading is only a lost packet. After a jump no stamp is believed until the server holds an intent of the new run. Time is nudged, never a tick's length: a tick must be the server's tick to the bit. Every server packet says how early the last intent arrived; the client speeds or slows its tick by at most 2% to hold the margin at two ticks plus measured jitter (Overwatch, Rocket League). A jump of more than a second resets it.
- **Prediction**: the client keeps, for each tick not yet answered, its intent and its body after it (`terra_motion`'s `Body` is plain and `Clone`). The server's packet carries the body's authoritative state for the last intent tick it ran. Equal within a tolerance (1 mm, 1 mm/s): the history up to it is dropped. Different: the body is set to the answer and the later intents are replayed against the world as it now is, other bodies where they are shown (Unreal's saved moves). The picture eases over the error (`tick::Smoothed` already separates where a body is from where it is drawn).
- **Others**: shown at the server's time less two send intervals plus jitter; between two states it interpolates (`Samples::around`, `Shown::between`), and past the last it holds: it never runs on beyond what the server said.
- **Blows**: a player's blow is judged on the server against creatures where that player saw them, rewound at most 250 ms (Source's lag compensation). No player strikes a player (D2). It comes with the fight's job, not the first ones.
- **A joined game's own change to the ground shows at once** and is asked of the host, as Minecraft's is: the dig is the game's own code, unchanged. When the host's order of it comes back the joined game knows it for its own and does not make it twice. One the host refuses (out of the player's reach, D51) stays in the joined game that made it, which comes only of a game out of step or dishonest; undoing it is to come.

## What is sent, and when (D652)

- **Rate**: the server sends each client one `State` packet every second tick (32 Hz), at most 1,200 bytes (under any path's MTU, never fragmented).
- **Most recent state, by priority**: for each client the server knows, per part, the version that client last acknowledged. A part whose version is newer is owed. Owed parts gain priority each tick by kind and nearness, the packet is filled from the highest, and what does not fit waits and rises (Tribes, Halo, Fiedler's priority accumulator). A part sent and not acknowledged is sent again after half again the client's measured round trip. A thing at rest costs nothing; a crowd degrades by updating far things less often, never by growing the packet.
- **Interest**: cells of 32 m; a client is sent the entities in cells within its radius (128 m to start, a number per kind later) and stops being sent them one cell further out (hysteresis). Leaving is a part's version too, sent until acknowledged, so no reliable message is needed and order cannot go wrong.
- **Operations**: on `Ordered`. A client asks; the server checks reach, ownership and permission (D51), gives the operation the next number in its region's record (regions of 256 m) and sends it to every client whose interest holds the region. The lane loses nothing, so the server knows how much of each region's record a client has been sent; a client coming to a region is sent what follows, and a late join is the same path. Compacting a long record into a snapshot is D21's later step.
- **Intents** repeat: one equal to the one before it goes up as a bit, so a key held costs little.
- **Events** (a noise, a blow landing), built: on the `State` lane, to those within the radius, each with its number and the tick it happened, sent again until acknowledged and dropped at half a second old: a noise late is worse than none, and one behind a lost packet never waits on the ordered lane. A client hears each once, whatever the link copies.
- **Quantising**: positions to 1/512 m within a cell-relative frame, velocities to 1/64 m/s, directions to 12 bits a turn; a client's own body unquantised.

First numbers, each a reading until its test has a planted fault (D632); they set v0's unset budgets. The readings are the soak's, on the toy game (four players, fifty creatures, 100 ms, 1% loss), payload bytes with no transport overhead, in a debug build:

| Budget | First target | Read 2026-11-06 |
|---|---|---|
| Down, a player among 3 others and 50 creatures | 8 kB/s | 6.2 kB/s |
| Up, a player | 4 kB/s | 2.4 kB/s |
| Server, per client per tick, to pick and pack | 20 µs | 3.9 µs |
| A late joiner correct around itself, on loopback | under 1 s | under 1 s (asserted) |

Against the running game (`node tools/sandbox/net.ts --soak 30`, 2026-11-06: one hosted copy at 75 fps and four players with no game behind them over UDP at 100 ms and 1% loss, for 30 minutes of the clock): each ended where the host had it (0.1 mm at worst), the longest silence 8 ticks, 4.7 kB/s down and 3.7 kB/s up a player by the carrier's count, its framing included: both within their targets. The up was 13.1 kB/s at first, then 5.0; a steering is under 10 bytes and goes in three packets, 32 packets a second.

On the real mover (four men walking, sprinting and jumping for 30 simulated minutes, 100 ms, 1% loss): 3.1 kB/s down and 1.3 kB/s up a player, a body shown in 19 bytes, and no correction at all once the clocks have their margin.

## Connecting

1. The server's challenge, then `Hello`: the protocol's version (a hash of the wire schema and the game's data fingerprint), the player's identity and name, signed with the challenge. The identity is a public key whose secret stays on the player's machine (D592). Steam's identity replaces it with the relay (D34).
2. `Refuse` with a reason a player can read (wrong version, world full, not allowed), or `Welcome`: the tick, the room's realm and seed (v0's second seam: the seed is all a client needs to make the land), the world's clock, the entity that is yours.
3. The area arrives by the ordinary paths (operations for its regions, then state); the client is `Ready` when its own region's record is whole.
4. Leaving, by choice or timeout (10 s): the character is saved where it stood (D592) and its body leaves the world at once (D637); built for where it stood, kept while the world is hosted, so one who comes back is put there. When the host leaves, every client is told why, the world is saved and the session ends; built, on the game's closing and on `net.leave`.
5. From the title screen (D639), built: Host opens the world played (`net::session::open`); Join asks the address typed with no realm of its own, and the welcome's realm (seed and size) is the land the game then makes, the link kept while its clock is held (`link::wait`: heard and answered each frame, no tick walked; the clock and body begin again from the server's newest word, and a clock starting believes no stamp until the server holds an intent of its run). A connection lost before any welcome says nobody answered. The host's first word is a greeting with no entity (`Server::greet`); the joining game says `Ready` on its first walked tick and is then given its body and welcomed with it, so no body stands idle while a game loads. A ready from one not greeted, or with a body already, is out of turn and ends the connection.
6. How each player looks (D111), built: its features are the walker's fourth part of its entity (`net::looks`), a guest's as its game says, as what is worn is.
7. Who is here and what they say, built: each client is told of the others as they come and go, and what one says everyone hears, by name, on the ordered lane (to all, while a world holds a handful; by area with D52).

## How each crate crosses

| Crate | Owns | Sent as |
|---|---|---|
| `terra_motion`, `terra_walker` | Bodies and the mover | A motion part (place, velocity, facing, ground state); the owner's at full precision for prediction; intent up |
| `terra_core`, `terra_voxel`, `terra_build` | The ground and bases | Operations, in order, by region. The ground's is built (`terra_walker::net::ground`): the host's operation log is the truth, whatever it gains is ordered, and a joined game's digs are asked of it; `Operation` has its wire form, held to what could be asked. Bases are to come |
| `terra_items`, `terra_survival` | What is carried, vitals | Operations the server checks; an owner-only part for the pack, a public one for what is worn. What is worn is built (`terra_walker::net::worn`): a player's outfit is a part of its entity, the host's from its gear, a joined player's as its game says (shown, not owned, until the pack is the server's); what is held in hand and a piece's dye do not cross yet |
| `terra_creatures` | Creatures, simulated only on the server, in cells any player is near | Motion, and the move a creature is in with the tick it began, so each client plays the clip itself |
| `terra_climate`, the sky | Nothing travels but the clock and the seed (D540) | `Welcome`, then a clock correction |

## Tests, with no window

Each check has a planted fault beside it (D632).

- **Milestone D** (engine.md), built: a server and two clients on loopback over a toy state end the same; a third joining late catches up. Faults: an operation dropped from one region's record, and a thing moved with nothing sent, must each fail the check.
- **Soak**, built: four clients, 100 ms, 1% loss, jitter and copies, 30 simulated minutes in 2 s of real time: no lasting mismatch, a packet at rest carrying nothing, bytes a player a second reported. Fault: acknowledgements cut off must show as state never settling.
- **Prediction**, built (`terra_motion::mover::net`): two men on the real mover at 100 ms and 1% loss: every tick a client first predicts is where the server put the body, to a millimetre; a shove from the server is mispredicted for under 24 ticks and no longer; and the Server track's bar on the mover: four men for 30 simulated minutes (4.5 s of real time), never mispredicted, those who walk apart forgotten. Fault: a client whose man is a tenth faster must fail the check.
- **The reader**, built: random bytes never panic the wire, the server or a client; on the ordered lane they cost only their sender its connection.

## Build order

1. `terra_net` plain: `wire`, `transport` with loopback, `ack`, `interest`, `server`, `client`, operations, milestone D's test and the soak.
2. `clock` and `predict`, and two bodies walking together on the real mover in one process, with the prediction test and the first bytes-a-second reading.
3. The `udp` transport and connecting: version, identity, refusals. Built: a server's first word to a connection is a challenge never given before, and a hello carries it signed (Ed25519, `ed25519-dalek`), so nobody plays as another and a hello overheard is no use.
4. The game as a client of its own server (the Bevy side), then a second player in it. Built so far, the host's side of the walker layer: `terra_walker::net`'s `Host` (present while a world is hosted), `hear` before the walk (who joined and left; each guest's `Intent` and `View` from what its client sent) and `tell` at the tick's end (each player's mover body as its parts, then the sends). A guest is a `Player` like any other. Tested with no window by a client that is no game. And a joining game's side, `net::link`'s `Link` (present while joined): `listen` before the walk takes the server's answer for this machine's player and, where it differs from what was predicted for that tick, sets the body to it (in the walk, which alone can ask the ground) and walks the ticks since again, restoring what the walk keeps of the player (stance, stamina, motion); `record` keeps each tick's result; `show` puts every other player as a `Puppet` between its last two words; `pace` runs the fixed tick at the clock's rate. Tested with no window by two worlds side by side over a rough link. In the running game: `net::remote`'s methods make a `Host` or a `Link` over UDP for a scripted run, a guest and a puppet are `Player`s with an empty outfit, so each is given a figure in its body's look whole (what a player wears crosses with step 5), and `node tools/sandbox/net.ts` runs two copies, one hosting and one joined, and checks by what each says that each holds the other and both have the joined player in one place (0.0 mm apart after a walk, one correction, on a loaded machine). The welcome carries the host's seed and size (`net::realm_of`); a game that made another world ends its link with words for the player. To come: the title screen's Host and Join, making the world from the welcome's seed rather than only checking it, and the host's own player as a client (D638).
5. The fight, the pack and creatures across the seam, each with its track.
