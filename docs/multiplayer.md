# Multiplayer and persistence

Status: the approach and the scale are decided; how to reach the scale is not. The game is server-first (D51), and everything is built toward **a thousand concurrent players on one persistent server**, in a fully procedural world that is very cheap to compute, send and store (D52). It is a test each decision has to pass, not a launch promise. No server exists yet and nothing has been measured at any scale.

## What a thousand asks

- **One process cannot be the whole server.** The world has to be divided by area among several workers that hand players and changes across their borders, and that division has to be designed in, not added.
- **Nothing may be sent to everyone.** Every message is about a place and goes only to who is near it.
- **Computed, not stored, is the main lever** (D47). A fully procedural world means a server holds only what players changed. The cost of a player then depends on how much they have built and dug, not on how far they have walked.
- **Per-area records of change.** A new arrival cannot replay a year of digging and building. Each area needs a compact record of its current state, and arrivals are sent that, not the history.
- **Crowds.** A thousand players will sometimes be in one place. Drawing a few hundred animated figures and their bases is a rendering problem of its own, separate from the server's.
- **Room for everyone.** One realm is over ten thousand square kilometres (D13), and there is no fixed limit on realms (D17); the question is whether players can find each other.
- **Strangers.** A thousand people are not friends. Ownership, permissions, griefing and cheating all need rules.
- **A database and backups**, and a way to carry a world forward when the game's rules change.

Close references: Eco (a shared world on persistent servers), Minecraft, Vintage Story and Space Engineers' dedicated servers.

## Playing together

Each player is a body in the same world, with their own tools and belongings. Anyone in a tribe can dig, harvest and build within its claims (D166), and the server decides the order of every change (D21).

## What already fits

- **Changes are operations in the server's order** (D21, D29), on the server's fixed tick (D138).
- **The world is computed, not stored** (D47). Only what players change is kept, which is what makes a long-lived world affordable.
- **The rules live in crates with no graphics** (D28), so a server with no window is the same code.
- **Every machine generates the same world** (D30).
- **Claims bound what is stored** to what players hold, not to everything they ever touched (D94, [building.md](building.md#claims)), and the world heals outside them.

## Server-first

Decided (D51). The game always talks to a server, which decides everything and sends each player what is near them.

- **One code path.** Solo or with friends, the server starts inside the game; on a persistent world it runs alone on another machine.
- **The server always decides and always sends by area**, even when a handful of players could be sent everything.
- **Not trusting clients.** Every action is checked by the server: is that in reach, do you have it, are you allowed.
- **A handful of players first**, then measure what larger worlds cost with simulated players before promising them.

## What it does to the design

These are design questions the scale raises, whatever the technology.

- **Do resources run out?** If trees and ore do not come back, a thousand players strip a world. Regrowth and scarcity are not optional.
- **What happens when nobody is online?** Does the world keep changing?
- **Ownership.** Who may take apart whose base, and who may open what (D94, D166).
- **More worlds** spread a crowd out, and raise how worlds are shared among servers, and which are running; a world instance is the unit a server runs, freezes and frees ([instances.md](instances.md), D468).

## Agents as players

The game can be played headless, by an agent alone (D73). Server-first (D51) makes the agent a client like any other, without a window:

- **The agent's client** connects to a server as a player does, and is held to the same rules: the server decides what it may do, so an agent cannot do more than a person could.
- **What it can see**: not pixels, but a description of what is around it, what it carries, its attributes, and what it is aimed at. The server already decides what each player is sent by area, so this is a different view of the same data.
- **What it can do**: the actions a player has, as calls: move toward a place, use the tool in hand on something, build a part here, put an item on the bar.
- **How it is reached**: as QA for agents is (D69), a command line over the game's own methods; an MCP server only if a client that cannot run commands needs one.

QA for agents drives a game from outside, with a tester's powers; playing is the same idea inside the rules, with a player's. What others have tried is in [research/agents-playing-games.md](../research/agents-playing-games.md).

Open: whether agent players are for us (testing, tuning, filling a server) or a feature for players too, and how a server tells them apart.

## Open questions

1. **Who runs the servers?** Players running their own, as with Minecraft and Valheim, costs us a server program and documentation. Running official ones costs money and ongoing attention.
2. **Friends or strangers?** A private server for a community is close to playing with friends. A public one needs moderation, ownership rules and far more defence against cheating.
3. **Does a small game, hosted by a player, remain a main way to play?**
4. **Does the world carry on with nobody in it?**
5. **Can players get in each other's way**: blocking a doorway, digging under someone, taking apart someone else's wall?

There is no player against player (D2) and no fail state (D18).

## What would be measured first

- How much a server needs per player who is somewhere nobody else is: memory and processor time for the terrain and objects around them.
- How much is sent per player per second, standing still and travelling.
- How long a new arrival waits before the area around them is correct.
- All three with a thousand simulated players spread over a world, and again crowded into one place.
