# Multiplayer and persistence

Status: the approach and the scale are decided; how to reach the scale is not. The game is server-first (D51), and on 2026-10-10 the developers set the requirement at **a thousand concurrent players on one persistent server**, in a fully procedural world that is very optimised (D52). Nothing here is built or measured.

## A thousand

This section was added when the number went from "ideally a hundred" to a thousand. The rest of the doc was written for a hundred and still holds; this says what the larger number adds.

- **One process cannot be the whole server.** A hundred players spread over a planet might be run by one program. A thousand will not be. The world has to be divided by area among several workers that hand players and changes across their borders, and that division has to be designed in, not added.
- **Nothing may be sent to everyone.** Every message has to be about a place and go only to who is near it. With a thousand players there is no "small enough to broadcast", except the planet's simulation, which is already one small snapshot (D29).
- **Computed, not stored, becomes the main lever** (D47). A fully procedural world means a server holds only what players changed. The cost of a player then depends on how much they have built and dug, not on how far they have walked.
- **Crowds.** A thousand players will sometimes be in one place. Drawing a few hundred animated figures and their bases is a rendering problem of its own, separate from the server's.
- **The planet is too small.** At a 2 km radius the planet is about 50 square kilometres, or twenty players to the square kilometre if they spread evenly. Planet size (D13) and more than one planet (D17) are now pressing questions.
- **Building at this scale.** Bases are lists of operations on a half-metre grid (D57). A thousand builders make a great many of them; each area needs a compact record of its current state, and new arrivals need that, not the history.
- **Strangers.** A thousand people are not friends. Ownership, permissions, griefing and cheating all need rules (open questions 3 and 6).

What to measure first, before promising any of this: the memory and processor time one idle and one busy player cost a server; how much is sent per player per second; how long an arrival waits for an area to be correct; and all of it with a thousand simulated players, spread out and then crowded together.

## What changed

The record so far says co-op for up to four players, with one of them hosting (D2).

From the developers, 2026-10-10 (D51):

> We'd also like the world to be able to support 100 concurrent players, I think, ideally, so it could be a persistent server, since the game is somewhat of a sandbox.

These are two different games to build, and the gap between them is larger than the gap between one player and four.

| | Four friends | A hundred on a persistent server |
|---|---|---|
| Who runs the world | One player's machine, while they play | A server that is always on |
| When the world exists | While the host is playing | All the time, including when nobody is there |
| Who you play with | People you invited | Possibly strangers |
| Trust | Everyone is a friend | Any client might cheat or grief |
| Joining | Everyone starts together, more or less | People come and go constantly |
| What each player is sent | Nearly everything | Only what is near them |

Close references for the larger version: Eco (a shared planet with a simulated ecosystem, on persistent servers), Minecraft servers, Vintage Story servers, and Space Engineers' dedicated servers.

## What already fits

Several choices made for four players hold at a hundred, because they were made to avoid sending or storing more than necessary.

- **Changes are operations in one authority's order** (D21). That authority can be a server as easily as a host.
- **The world is computed, not stored** (D47). Only what players change is kept, which is what makes a long-lived world affordable.
- **The simulation runs in one place and is sent as snapshots** (D29). It is small.
- **The rules live in crates with no graphics** (D28). A server with no window was always the intent.
- **Every machine generates the same world** (D30).

## What it adds

None of this exists, and none of it was planned for four players.

- **A server program**, separate from the game, that runs the world with nobody looking at it.
- **Sending each player only what is near them.** With four players you can send everyone everything. With a hundred, the server has to track who can see what, and bring a player up to date as they move into an area.
- **Per-area records of change.** A new arrival cannot replay a year of digging. Each area needs a compact record of its current state, which the engine plan noted as "needed eventually" and now becomes central.
- **A database and backups**, and a way to carry a world forward when the game's rules change.
- **Not trusting clients.** Every action is checked by the server: is that in reach, do you have it, are you allowed.
- **Ownership.** Who may take apart whose base. Among friends this needs no rules; among strangers it needs some.
- **Running costs and upkeep**, if we host anything ourselves.

## What it does to the design

These are design questions the larger number raises, whatever the technology.

- **Is the planet big enough?** A hundred people on a 2 km planet is one person per half a square kilometre. That may be lively or cramped.
- **Do resources run out?** If trees and ore do not come back, a hundred players strip a world. Regrowth and scarcity stop being optional questions.
- **Whose planet is it?** Terraforming one shared world is a fine fantasy for four. For a hundred it is either a grand communal project or a fight over the thermostat.
- **What happens when nobody is online?** Does the planet keep changing?
- **More planets.** A second planet was to wait (D17). A crowded server makes it more pressing.

## Server-first

Decided (D51). The costly mistake would be to build for four in a way that cannot grow. The affordable hedge is to build as if there were always a server, and let the small case be a server that happens to run on a player's machine.

- **The game always talks to a server.** In solo or friends play, that server starts inside the game. On a persistent world, it runs somewhere else. One code path.
- **The server always decides and always sends by area**, even when there are four players and it could send everything.
- **Start by making four work well.** Then measure what a hundred costs, with simulated players, before promising it.

That keeps a large persistent world possible without paying for all of it now. What it does not avoid is the design work above.

## Open questions

1. **Answered: a thousand is a requirement** (D52). Still open: by when. "Must work at launch" and "must be what the architecture is built for" lead to different orders of work.
2. **Who runs the servers?** Players running their own, as with Minecraft and Valheim, costs us a server program and documentation. Running official ones costs money and ongoing attention.
3. **Friends or strangers?** A private server for a community is close to large co-op. A public one needs moderation, ownership rules and far more defence against cheating.
4. **Does small co-op, hosted by a player, remain the main way to play?**
5. **Does the world carry on with nobody in it?**
6. **Do the four-player decisions still stand** for anything other than the number: no player-against-player (D2), and no fail state (D18)?

## What would be measured first

Technical, for when this is agreed.

- How much a server needs per player who is somewhere nobody else is: memory and processor time for the terrain and objects around them.
- How much is sent per player per second, standing still and travelling.
- How long a new arrival waits before the area around them is correct.
- All three with a hundred simulated players spread over the planet, and again with a hundred in one place.
