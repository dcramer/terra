# Your character: body, movement and what is on screen

Part of the game's design ([gameplay.md](gameplay.md), adopted as the source of truth on 2026-10-12, D89). Each item is as settled as the decision it cites; the rest is a proposal for the developers to react to.

## What the developers have said

- A person in the world with tools in hand (D89); third person, with first person perhaps an option later (D97); the character swims (D74); gear is yours wherever you go (D86).
- Rich systems, little on screen, somewhat realistic, with some on-screen display and character stats (D93). No hand-holding (D92).
- Equipment slots (D64), a backpack (D65, D84) and an action bar (D66): see [items.md](items.md).
- A map that shows only where you have been (D76), and an options menu (D80).

## Who you are

- **No class at the start.** What you become comes from what you practise (D88) and what essences you take ([magic.md](magic.md#essences)).
- **Creation is the look** (D111): a few sliders, not templates: sex, build, height and skin colour, then hairstyle and hair colour, which can be changed later in the game. How bodies are made is in [characters.md](characters.md).
- **Where you come from** is open ([gameplay.md](gameplay.md#questions-for-the-developers), question 8): a newcomer to the frontier, or someone born there.

## Moving

You are a person in the world, not a hand hovering over it: you do things by carrying a tool to where the thing is and using it.

- **Down is toward the world's centre** (D63), so it changes as you walk round a world.
- **On foot**: walk, sprint, jump, climb, swim, all on stamina. Slopes slow you and steep ones stop you. Climbing is a skill by doing (D88).
- **The ground is whatever is there now**: movement works in holes and tunnels players dug, on a building's floors, and on stairs between storeys.
- **No glider, grappling hook or jetpack** for now; they can be added later if wanted. A player who digs a deep hole climbs or digs a way out.
- Open: **does a long fall hurt?**

## Acting on the world

Proposed.

- **Aim with the middle of the view.** Whatever is under it, within reach, is the target: ground, an object, a wall, a station.
- **The game says what you are aiming at and what the tool in hand would do**, before you do it.
- **Some actions take time.** Felling a tree is a few seconds of work, not a click.
- **Every change to the world is an action the server checks** and that returns what it produced (D51), so it can be sent to other players and saved.
- Open: **reach**, arm's length or a few metres.

## Getting around

Worlds are round and large enough that walking round is ridiculous (D4, D13), so how you cross them matters.

- **No fast travel between claims** (D72). Quick travel is by magic (astral magic's markers, recall and gates, [magic.md](magic.md)), by portals found or built (D72), and by mounts; what you carry goes with you.
- **Roads** worn by travellers between settlements ([people.md](people.md)).
- **Mounts**: probably, by taming (D100, [creatures.md](creatures.md)). Boats: open.

## The map

There is a map (D76). It shows only where you have been: the rest is hidden until explored. For now it shows where you are; whether to hide that, as Outward does, is for later. It does not hold your hand (D92): no quest markers. Proposed: it also shows what you mark ([gameplay.md](gameplay.md#no-hand-holding)).

Open:

1. **Is it a thing you carry**, a screen in a claim, or always to hand?
2. **Is it shared** within a tribe, and does it mark claims, players and finds?
3. **Does it cover other realms**, or only the one you are in?

Technical, mine: a map of a world is cheap if it is drawn from the same rules that generate the world (D59) rather than from what has been loaded, so the whole world can be sampled coarsely without generating any of it in detail.

## What is on screen

Within D93: what your body would feel, and a few compromises for play.

| Shown | Not shown |
|---|---|
| Health, stamina, mana ([magic.md](magic.md#mana)), breath under water | A minimap, quest markers, objective arrows |
| The action bar, what is in hand | Names and levels floating over people and monsters |
| A character screen: essences, abilities, ranks, skills, equipment, backpack | A journal (D92) |
| The map, only where you have been, with what you mark, and where you are (D76) | What you have not explored |

**Sound carries information**: a monster's call, a settlement's bell, rain coming, a predator behind you. Music sparse, and from the world where it can be ([audio.md](audio.md)).

## Options

An options menu with what players expect of one (D80), including audio and remapping keys. Proposed contents:

- **Audio**: overall, music, effects, voices.
- **Controls**: remap every key and button, mouse sensitivity, invert look. Every control is looked up by action, not by key.
- **Display**: window or full screen, resolution, frame-rate cap and vsync, field of view.
- **Graphics**: a few quality levels, and the settings under them (shadows, draw distance, grass).
- **Interface**: text size, and hiding the frame-time readout (D77).

Built in the sandbox's `play` experiment, opened with Esc: remapping every control to any key or mouse button, two per action; look speed; invert look; field of view; full screen; vsync; master, music, ambience and effects volume. Saved as the player changes them. Not yet: voices, resolution, frame-rate cap, graphics quality, text size.

## What it asks of the engine

Technical, mine: a character controller that walks on round, dug voxel ground and on building floors; one way to ask what is under the aim across ground, objects and buildings; and smoothing for other players' movement.

## Questions

1. Answered: **third person**, first person perhaps optional later (D97).
2. Answered: **no glider or grappling hook** for now.
3. Answered for now: **the map shows where you are**, and **only where you have been** (D76).
4. **A compass**: on screen, as an item, or none?
5. **Names over heads**: none at all, or other players' names only?
