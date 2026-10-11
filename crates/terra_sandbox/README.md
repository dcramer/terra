# Sandbox

A test environment for working on graphics and utilities. It opens a window on a stand-in planet with a free camera, lets you change values while it runs, and can switch on experiments.

It is not the game. Nothing here is gameplay, and the planet is a formula, not voxels.

## Run

```bash
cargo run -p terra_sandbox -- --experiment scatter
```

Under WSL that renders on the CPU, which is slow but correct. To use the real GPU:

```bash
cargo xtask win -p terra_sandbox -- --experiment scatter
```

| Option | What it does |
|---|---|
| `--experiment <name>` | Switch on an experiment. Repeatable. |
| `--list` | List the experiments |
| `--radius <metres>` | Planet radius, 2000 by default |
| `--shot <file.png>` | Save a screenshot once the scene has settled, then exit |
| `--clean` | Leave the frame-time readout off the screen, for pictures |
| `--bench` | Measure frame times with vsync off, print them, then exit |
| `--settle <frames>` | Frames to run before capturing or measuring, 120 by default |
| `--press <key>@<frame>` | Press a letter, digit, F1 to F12, `esc` or an arrow (`left`, `right`, `up`, `down`) on a given frame, for scripted runs. `<key>@<from>-<to>` holds it. The arrows look about, so a script can aim. |
| `--save <file>` | In the play experiment: keep the saved game in this file. Scripted runs save and load only when given one. |
| `--fresh` | In the play experiment: start over, throwing away the saved game |
| `--at <east,south>` | In the play experiment: start the player so many metres east and south of the outpost, for testing. The log says where the deer, the spiders and the village are. |
| `--give <item>:<count>` | In the play experiment: start carrying something extra, such as `stone:10`, or start with `health`, `fed` or `stamina` at a value out of 100, for testing. Repeatable. |
| `--vantage <x,y,z,x,y,z>` | In the play experiment: a fixed camera, given as where it stands and what it looks at, in metres east, up and south of the base's corner |

## Controls

| Input | Action |
|---|---|
| Hold right mouse button, or press M | Look around |
| W A S D | Move |
| Q, E | Down, up |
| Left shift | Move fast |
| Scroll wheel | Change speed |
| F1 | Inspector: every entity, resource and asset, editable live |
| F2 | Tweaks: sun, sea level, ground colours |

The camera is Bevy's free camera, which treats +Y as up. The start is at the planet's pole, where that matches the ground; fly far around the planet and the horizon tilts.

## Working on graphics

The ground's look is in `assets/shaders/ground.wgsl`, the sea's in `water.wgsl`, the sky's in `sky.wgsl`, and the wind that moves plants in `wind.wgsl`. Save one while the sandbox is running and it updates. The F2 panel changes the colours and the sun without touching code, and F1 lets you edit any material.

To check a change without watching it, take a screenshot and read it:

```bash
cargo run -p terra_sandbox -- --shot shots/ground.png
```

Shader reloading works when run through `cargo run`. The Windows build launched by `cargo xtask win` runs from a copy, so edits to the original are not picked up there.

## The play experiment

```bash
cargo xtask win -p terra_sandbox -- --experiment play
```

The tech demo of the MVP's loop (docs/prototypes.md), being built on the earlier proof of concept: an abandoned outpost in a meadow by a lake, a little before sunset, and a player who arrives with a few tools and nothing else. You steer an animated figure in third person. It walks on the planet, climbs the steps to the base's door, is held up by floors, stopped by walls, windows, trees and boulders, and let through doors.

![The outpost](../../docs/images/outpost-2026-10-10.png)

| Input | Action |
|---|---|
| Click | Capture the mouse |
| Esc | The options menu, which gives the mouse back. Esc again returns to the game. |
| Mouse | Look. The arrow keys do the same. |
| W A S D, Shift, Space | Move, sprint, jump |
| 1 to 6 | Pick a slot of the action bar. What is in it is the tool in hand. |
| Tab | Open and close the inventory. I does the same. |
| F | Use what you aim at, within reach: a workbench, smithy or campfire opens its recipes, the claim stone grows your claim, a dropped bag gives back what fits |
| H | Eat the most filling food you carry. Clicking food in the backpack eats it too. |
| Left click | With anything but the hammer in hand: swing at a creature in front of you |
| Hold right mouse | With anything but the hammer in hand: guard, taking a quarter of a blow on stamina |
| Ctrl | Roll clear of blows |
| M | The map: the land around the outpost, only where you have been |
| F5 | Save now. The game also saves every half minute and when it closes. |
| Hold right mouse | With the hammer in hand: the dial of parts. Move the mouse toward one and let go, or press its number. B does the same. |
| Tap right mouse | Put the part away, or put back what you picked up |
| Hold left click | Build, take apart, or cut. It takes a moment; let go early and nothing is done. E does the same. |
| Left click | In move mode, pick an item up and put it down |
| Scroll wheel | Turn an item, a room or a wall; switch a wall between pushing out and pulling in. Z and C do the same. |
| R | The next variant of the part; with Shift, the one before |
| Q | Take-apart mode, and back |
| G | Move mode, and back |
| X | The grid on and off |
| Hold Alt | The opposite of the grid setting, while held |
| Middle click | Pick another of the item you are looking at. V does the same. |
| F3 | More frame timings: per render pass, entities, memory ([development.md](../../docs/development.md#logs-and-the-frame-time-readout)) |

Those are the defaults. In the options menu (Esc) any of them can be set to another key or mouse button, except Esc, the numbers, the wheel and F1 to F3; click a control, then press its new key or button, or right-click it to clear it. The menu also has mouse look speed, invert look, field of view, full screen and vsync. Every change is saved at once to `settings.txt` in the player's settings folder (`~/Library/Application Support/Terra` on macOS, `%APPDATA%\Terra` on Windows, `~/.config/terra` on Linux). Delete it to go back to the defaults. Scripted runs never read or write it, so a script does the same thing on every machine.

The buttons follow Subnautica 2 where we could confirm what it does: right mouse is the build menu, left mouse builds, and taking apart and moving are modes on Q and G (moving was on F until F became the interact key). The rest is the usual habit of building games. See [research/subnautica-2-building.md](../../research/subnautica-2-building.md).

You carry a backpack, on the rules in `terra_items` and the Subnautica baseline (docs/items.md, D84): a grid in which each item fills its cells and nothing stacks. Beside it is the character sheet (D64): a slot each for a pack, the head and the body. You start wearing a small pack, which makes the backpack 6 by 5 cells, up from 5 by 4 with none, and carrying a hammer, a stone axe, a stone pickaxe and a knife (D96). Nothing else: everything is taken from the world. The action bar along the bottom points at tools in the backpack; materials are counted at the top right.

![The character sheet and the backpack at the start](../../docs/images/outpost-backpack-2026-10-11.png)

- **Gathering**: one tool to a job. The axe fells trees for wood, the pickaxe breaks rock for stone and ore rocks (a rusty colour) for ore, the knife cuts bushes and plants for fibre. Aim at one, or just stand in front of it, and hold left click until it gives. A tool's material decides what it can work: the great pines are too tough for a stone axe, and a stone pickaxe is slow on ore. A full backpack takes nothing more, and what does not fit is left behind.
- **Claiming**: you build only inside your claim (D94). With the hammer, pick Craft, then Claim stone, and place it; the claim is 40 metres across, drawn on the ground while the hammer is in hand, and F at the stone grows it to 80 and then 160 for a price (D104). Until you place one, the hammer builds nothing and takes nothing apart.
- **Building** spends materials. Everything has a cost, shown in the prompt before you commit: wood for floors and walls, fibre for windows, hide for a couch. If you are short, the thing shows red and the prompt says what is missing. Taking something apart gives back everything it cost, if there is room in the backpack for it; if not, it stays, and a notice above the bar says why.
- **Crafting**: F at a workbench or a smithy opens its recipes beside the backpack; press a number or click one to make it. The workbench makes stone tools, leather gear and a larger pack; the smithy smelts ore into iron and makes iron tools, but only once a smith lives at your claim. The outpost has an old workbench by its door.
- **Staying alive**: health, stamina and hunger, as bars at the bottom left. Sprinting, jumping and working spend stamina, which comes back when you stop. Hunger falls all the time; hungry, less comes back; starving drains health until you die (D101). Berry bushes give berries to the knife; meat cooks at a campfire into something far more filling.
- **Dying**: you fall, everything in your backpack drops where you fell in a bag, and you wake at a bed in your claim, or where you first arrived. F at the bag takes back what fits; left alone, it is gone after ten minutes (D84). What you wear stays on you, and the action bar still points at your tools when they come back.
- **Skills by doing** (D88): chopping, mining, foraging, building and crafting are learned the first time you do them and improve as you do them more; each step makes that work quicker. The list shows beside the backpack while the inventory is open. Dying costs half the way to each skill's next step, never a step already reached (D101).
- **Creatures** (game design 001, creatures.md): a herd of deer grazes and wanders in the meadow and runs from anyone close or anyone who hurts it; spiders wait in a hollow further off, chase whoever comes near, bite within reach, and go home if led too far. A dead deer can be butchered with F, if you carry a knife, for meat and hide.
- **Fighting** (D97): click to swing at a creature in front of you. The spear, made at the workbench, hits hardest and reaches furthest; tools hit less well. Swings and guards cost stamina; a roll takes you clear of a bite. Landing blows teaches fighting.
- **A place where magic pools** (D103): a hollow further off, lit violet with motes drifting up, seen from a distance. Spiders guard it; while fewer than three remain, another forms there every minute and a half. Nothing marks it on a map (D92).
- **A village** (D95): a few houses, a well and a stall, set down away from the hollow, with people going about their day. F talks to them. The elder, Maren, says the spiders took their smith and which way the glow lies, by the sun; the others say what they know.
- **A rescue** (D90): the smith, Bren, kneels bound at the edge of the hollow. With the spiders near them dead, F frees them, and they follow you. Bring them into your claim and they ask for a smithy; once one stands there, they settle and work it, and the smithy's iron recipes open.
- **The map** (D76): M shows the land around the outpost, 512 metres across, dark until you have been near it; you are marked on it, and so is your claim. Nothing else is: not the village, not the glow (D92).
- **Saving**: what you built, what you carry and wear, your body and skills, your claim and the people you brought home are saved beside the settings, as `save.ron`, every half minute, on F5 and when the game closes, and loaded when it starts. The land, its plants and creatures come from the world's rules and are not saved, so what was cut or killed grows back (D94). Delete the file, or start with `--fresh`, to begin again.
- **The inventory** shows the character sheet and the backpack's grid. Point at a tool and press 1 to 6 to put it in that slot of the bar. Click a slot of the sheet to take off what is in it, and click gear in the backpack to wear it. The pack will not come off while what you carry would not fit without it. Opening it puts away the part in the hammer's hand.

The materials, costs, recipes and what you start with are stand-ins, chosen so the MVP's loop can be walked.

Building, taking apart and cutting all take time. You hold the button, a bar under the aiming mark fills, and it is done when the bar is full; let go early, or walk off, and nothing is done.

- A room, wall or opening being built has a film that climbs it; one being taken apart has a film that drains away.
- An item being built grows inside its ghost; one being taken apart shrinks.
- A tree being cut shudders, then falls over away from you.

How long each takes follows from the thing: what it costs to build, and how tough it is to cut. A tool can be quicker or slower at cutting each sort of thing, or unable to cut it at all, as `Tool::cuts` in `inventory.rs` says.

Building starts from a part. Pick one from the dial, cycle through its variants, and aim. What would be built is shown before you commit: the outline of a room, the cut of a window, the wall a push would move, or a ghost of the item where it would stand. It is red where it cannot go.

![The dial of parts](../../docs/images/outpost-dial-2026-10-10.png)

| Part | Variants | What holding left click does |
|---|---|---|
| Room | Four sizes, in metres; the wheel turns it | Builds a room on the ground you aim at. Aimed across a room already there, it extends that room to the combined shape. |
| Reshape | | Pushes the wall you aim at out by one step of the grid, or pulls it in; the wheel switches between the two |
| Wall | Three lengths; the wheel turns it | Builds a stretch of plain interior wall on the grid line nearest where you aim. Indoors only. |
| Window | Four sizes | Cuts a window that wide in the wall you aim at, outside or interior, centred where you aim |
| Door | | Cuts a door in the wall you aim at, outside or interior |
| Remove (Q) | | Takes away the item, closes the window or door, or takes away an interior wall that has neither |
| Move (F) | | A click, not a hold: picks an item up, then puts it down |
| Seat, Table, Bed, Storage, Kitchen, Decor, Kit, Fire | Twenty-seven items between them | Stands the item on the floor or ground you aim at |

Everything has a box. An item cannot be placed where its box would share space with another item, a wall, a tree or a boulder, and a walker cannot enter one. Rugs are the exception: they lie under anything. When something is in the way, the prompt says what, and an item that is in the way is outlined.

With the grid on, items land on half-metre steps and turn in eighths of a circle. The grid is drawn on the floor or ground round the item, with tile edges stronger than the steps between. With the grid off, items go exactly where you aim and turn in finer steps. Either way:

- Aim at a wall and the item stands flush against it.
- Aim at the side of another item and it stands flush beside that, lined up with it if nearly so.
- Aim at the top of another item and it stands on it.
- Aim at nothing in reach and it is offered a few steps in front of you.

A room is a plain box: lamps and everything else in it are items you place.

Everything is on one grid of half a metre, set by one constant in `terra_build`. A wall moves by it, a window or door can be any number of its cells wide and stand anywhere along a wall, and a new room lines up with the rooms already there because they share it. Prices are by measure (floor by area, walls and windows by length), so they do not change if the grid does. A room's foundation may go into the ground, but the room itself may not: where the ground is higher than its floor, the outline is red and it cannot be built.

![An interior wall, about to have a window cut in it](../../docs/images/outpost-wall-2026-10-10.png)

The game holds the mouse while you play and lets go when the window is no longer in front; click to take it back.

Everything in and around the example base was placed through the same call the builder makes, and can be moved or taken away. The list of parts, with what each costs, is in `catalogue.rs` and is a stand-in.

It is made of these pieces, each a module in `src/experiments/play/`:

| Module | What it is |
|---|---|
| `scene` | The sun, the sky's colours, haze, shadows, and the camera's effects |
| `flora` | Trees, rocks, bushes and flowers placed from the seed, and the grass |
| `catalogue` | The parts the builder can build, their variants and what they cost |
| `inventory` | What is carried, on `terra_items`: the backpack, what is worn and the action bar as components on the player, and the demo's kinds of item |
| `bag` | The action bar, the raw-item count, notices, and the character sheet and backpack's grid on screen |
| `interact` | What the interact key would act on: the nearest station, claim stone or person within reach |
| `crafting` | Stations, recipes, and making things from what you carry |
| `claim` | Your claim: placing it, growing it, and drawing its edge |
| `vitals` | Health, stamina and hunger: what wears them down, what brings them back, and the bars |
| `death` | Going down, the dropped bag, and waking at a bed |
| `skills` | Skills by doing: what was done becomes progress, steps make work quicker, dying costs some |
| `creatures` | Deer and spiders: their models and animations, what each does next, and butchering |
| `combat` | Swinging, guarding and the blows creatures land |
| `people` | Figures who are not the player: their colours, animations, following, and settling to work |
| `pooling` | The place where magic pools: its glow, its motes, and the monsters that form there |
| `settlement` | The village, its people, and what they say |
| `map` | The map: the land drawn from the planet's rules, revealed as you go |
| `save` | Saving and loading the game |
| `harvest` | Gathering with the right tool for each thing, and refusing what its material cannot work |
| `work` | Holding a button until a job is done |
| `tool` | The builder: what it is aimed at, the preview, and building |
| `dial` | The dial of parts and the prompts |
| `props` | Items that have been placed, and stopping a walker at them |
| `figure` | The player's body and which animation it plays |
| `models` | Reading shapes out of model files |

The models are free ones, recoloured in code; see [THIRD_PARTY.md](../../THIRD_PARTY.md). How it was put together and what it costs to draw is in [research/outpost-scene.md](../../research/outpost-scene.md).

What is placeholder: the figure is a mannequin from an animation library; the planet is the stand-in one, so it cannot be dug and the base stands on a foundation clear of it; collision is arithmetic (circles round trees and boulders, boxes round items, a grid for walls), not a physics engine, and bushes, small rocks and the figure's own limbs have none; you cannot stand on an item, and an item stood on another stays in the air if the one under it goes; there are no stairs, so upper storeys cannot be reached, and items go on the ground floor only; grass is planted once around the base and runs out about 100 m away; placed items and what you carry are not saved or part of the base's list of operations; and a cut tree never grows back; and none of the work has sound.

In a scripted run the game clock advances a sixtieth of a second per frame and real keyboard and mouse input is ignored, so the same script does the same thing on any machine:

```bash
cargo run -p terra_sandbox -- --experiment play --settle 480 --press w@20-215 --press b@232-246 --press 8@240 --press e@270-450 --press i@460 --shot shots/a-couch-and-the-backpack.png
cargo run -p terra_sandbox -- --experiment play --vantage 0,2.4,24,9,1.6,0 --shot shots/outpost.png
```

## The look experiment

```bash
cargo run -p terra_sandbox -- --experiment look
```

An early test of the art direction in [docs/art.md](../../docs/art.md): faceted ground, generated boulders and shrubs, a warm sun with cool shadows, haze, a gradient sky with clouds, and bloom. The play experiment has since gone further.

## The forest experiment

```bash
cargo run -p terra_sandbox -- --experiment look --experiment forest
```

About a million generated pines over the whole planet. Left click or F chops the one you are pointing at; K chops every tree in view. None of the trees is stored: each cell's trees are computed from the seed when the camera comes near, and a chopped tree is one bit. Measurements are in [research/forest-scale-test.md](../../research/forest-scale-test.md).

## The build experiment

```bash
cargo run -p terra_sandbox -- --experiment look --experiment build
```

A base you shape, inspired by Subnautica 2. It stands on the most level ground near the start, on a foundation, with steps up to its door. Aim with the mark in the middle of the view: G pushes the wall you are looking at out by a tile, H pulls it in, V cycles the tile through solid, three sizes of window and a door, and B adds a room on open ground. The rules live in `terra_build`; what it does and does not do is in [research/subnautica-2-building.md](../../research/subnautica-2-building.md).

## Adding an experiment

An experiment is a small plugin that nothing else depends on.

1. Add a module in `src/experiments/` with a `pub fn plugin(app: &mut App)`.
2. List it in `ALL` in `src/experiments/mod.rs`.

It can use the `Planet` resource to ask where the ground is, and anything in `terra_core` and `terra_gen`. `scatter.rs` is the example: it places rocks from a seed and lets you harvest them.

## Known limits

- The planet's mesh is fine, about a metre between vertices, only within 250 m of the start. Beyond that it is coarse, about 16 m, and cuts across hollows and humps by metres. Ask the planet for `drawn` rather than `surface` to put something on the ground as it is seen out there.
- No sky by default; the background is flat. The `look` and `play` experiments add one.
- Outside the `play` experiment there is no player, gravity or collision, and the camera flies freely.
- The play experiment is not tuned for weak graphics hardware: every tree is drawn at full detail at any distance.
