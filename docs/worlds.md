# Worlds

Status: the developers set out on 2026-10-10 what the planets are to be. What they said is below, as they said it, then what follows from it and what is still open. Nothing in this doc is built except a single stand-in planet in the sandbox.

## What the developers want

| | Said | Status |
|---|---|---|
| **Terrain** | Procedurally generated. | Decided (D8) |
| **More than one planet** | Each one different. Not thousands; "probably a dozen", and maybe more over time. | Decided that there are several (D17); the number is Leaning |
| **Variety** | Planets are thematic and varied: caverns, water features and the like. | Decided (D59) |
| **Terraforming** | Making a planet livable and breathable exists on *some* planets, with all that implies. | Leaning (D60) |
| **Foliage** | Thematically varied from planet to planet. | Decided (D59) |
| **Digging** | You can dig into the ground, much as in No Man's Sky. | Decided (D9, D20) |
| **Survival** | Attributes coupled to survival: health, stamina, oxygen, hunger, thirst, possibly more later. They may work differently on each world. See [survival.md](survival.md). | Decided that they exist (D61); the list may grow |
| **The starter planet** | Normal atmosphere and the like: the easiest place to be, for now. | Leaning (D62) |
| **Gravity** | Planets have gravity. | Decided (D63) |
| **Getting between planets** | By flight, for sure; maybe other ways later. Leaving a planet, being in space and arriving at another has to perform well. | Decided (D72) |
| **How planets differ** | Mostly in the obvious ways. | Answers the old open question 2 |

## What a planet is

A planet is a seed plus a **theme**. The theme is the small set of numbers and lists that make one planet unlike another; the seed makes each place on it unlike every other place. Everything else is computed from those two (D8, D47).

What a theme sets, as a starting list:

- **Size, and the strength of gravity at the surface.**
- **Atmosphere**: whether it can be breathed, its pressure, and how hot or cold it runs. This is what the survival attributes respond to.
- **Ground**: the shape of the land (how mountainous, how flat), what the rock and soil are made of, and how hollow it is: whether there are caverns, how big and how deep.
- **Water**: seas, lakes, underground water, ice, or none.
- **Life**: which kinds of plant grow where, from a palette that belongs to the planet, and how dense.
- **Palette**: the colours of a place, as the art direction asks (D14).
- **Whether it can be terraformed**, and if so where it starts on that path.

A new planet should be a new theme, not new code. That is the same rule the construction system follows (D56): the things that vary are data.

## The starter planet

Breathable, temperate, ordinary gravity, water and plants. The survival attributes all work, but nothing about the place is trying to kill you. It is where the player learns the game, and it is the planet the sandbox's stand-in is closest to.

## Terraforming, on some planets

The simulation of a planet becoming livable (D6, [planet-sim.md](planet-sim.md)) runs on the planets where terraforming is possible. Elsewhere a planet's climate is part of its theme and stays as it is, which costs nothing to compute. The dead-to-alive arc in [vision.md](vision.md) is the story of those planets, not of every planet.

## What it asks of the engine

- **Caverns** are no extra work for the shape of the ground: smooth voxels describe the inside of a planet as easily as its surface (D9). They are extra work for generation (shaping them so they are worth exploring), for light (it is dark underground), and for streaming (what is below you matters as much as what is beside you).
- **Water features** go beyond one sea level (D24). Lakes at different heights and water underground need water to be asked for by place, which the engine's contract already allows (D45), and the decision on pools (D44) is the same decision.
- **Gravity** pulls toward a planet's centre with a strength the planet sets. The player's movement already treats "down" as toward the centre. Between planets, and in orbit, it is open.
- **Several planets, reached by flight** (D72). Leaving a planet, crossing space and arriving at another, without a loading screen (D4) and without the frame rate falling, is a system of its own:
  - **Coordinates.** A position far from the origin loses precision. Each planet needs its own frame, and space needs one that holds them all (D36).
  - **Detail by distance.** From orbit a planet is one simple shape; on the ground it is terrain, plants and bases. The terrain's levels of detail (D9) carry most of this; plants, objects and bases need the same.
  - **What is loaded.** Only the planet you are near is generated in full; the rest are a few numbers until you approach.
  - **The server.** A thousand players across a dozen planets (D52) means a player flying from one to another moves between parts of the server.
  The Godot spike flew from the ground to orbit on one planet; nothing has crossed between planets yet.
- **Digging** is the terrain's job (D9); what you get from it is harvesting (D43).

## Open questions

1. **How many planets**, and are they all in one solar system? Answered in part: you fly between them (D72), perhaps by other means later.
2. Answered: planets mostly differ in the obvious ways (gravity, air, temperature, terrain, water, life). Whether any differ in less obvious ways (day length, weather, resources found nowhere else) can be settled planet by planet.
8. **What do you fly in?** A ship you board, a suit that keeps climbing, or something else? It is the largest open piece of the player's design ([player.md](player.md), question 6).
3. **Which planets can be terraformed**, and is a terraformed planet a reward, a goal, or the endgame?
4. **How is the next planet reached and unlocked?**
5. **Caverns**: a feature of some planets or all? Do they hold anything the surface does not?
6. **Water**: does it flow, or only sit at a level (D24, D44)?
7. **Size**: does every planet share one radius, or do they differ (D13)?
