# How do major games theme and scale their dungeons, and which themes should Terra's have?

2026-10-26. The developer's desk answer on r-dungeons asked for these comparisons. He wants the dungeons "more spacious", and some of them "cavernous". The question has three parts. How big are other games' dungeons, in rooms, corridors and ceilings? How are they built: from kits, by hand, or as sculpted meshes or voxels? Which few themes do the games share? The themes proposed from this note, decided as D527, are in [dungeons.md](../docs/dungeons.md#themes-d527); like all design, they are the developers' call (D412).

## Answer

- **Every one of these games builds its dungeons on a grid about twice the size of ours, and builds its caves differently from its halls.** Skyrim's kits use a 512-unit footprint, about 7.3 m, with halls on a 256-unit sub-grid, about 3.7 m. Valheim's rooms are 8 m in 4 m steps. Our cell is 3 m, with 2 m of clear corridor and 3.5 m of headroom, which is the Level Design Book's bare minimum for a hall.
- **Five themes recur**:
  1. Built burial halls: a crypt or barrow.
  2. Natural caves.
  3. Mines and tunnels.
  4. Ancient engineered halls.
  5. A vast underground cavern, the Underdark.

  Each game reuses a few kits across hundreds of dungeons. Skyrim built 7 kits for 300+ dungeons, Diablo IV built tile sets for 150+, and Valheim has a 94-room pool. The variety comes from props, light and enemies.
- **Caves are not built from the halls' kit in any of these games.** Skyrim builds its caves from shells: the room's whole skin is one piece, joined by flanges and arches. Deep Rock Galactic carves its rooms out of intersecting spheres and planes, then crumples them with noise. Astroneer and Enshrouded dig them out of voxel terrain.
- **Our terrain is already smooth voxels at 0.5 m (D9, `terra_voxel::mesh::VOXEL_M`), and caverns cost its shape nothing ([worlds.md](../docs/worlds.md)).** The dungeon stamp already digs terrain boxes. So a cavern theme can be carved terrain rather than cubes.

## The games

### Skyrim (Bethesda, 2011): Nordic ruins, caves, Dwemer ruins

The best source is Joel Burgess and Nate Purkeypile's GDC 2013 talk, *Skyrim's Modular Approach to Level Design*: [slides](https://www.slideshare.net/slideshow/gdc2013-kit-buildingfinal/17728576) and a [write-up](https://gamedeveloper.com/design/skyrim-s-modular-approach-to-level-design).

- **Scale**: 128 units is about 6 ft, roughly 1.8–2 m. That makes one unit about 1.43 cm.
- **Footprints**:
  - The base kit footprint is 512×512×512, about 7.3 m on every side.
  - The other footprints are 256×256×768 (about 3.7 by 11 m) and 256×256×1024 (about 3.7 by 14.6 m).
  - Sub-kit footprints are multiples of each other, and designers snap to half a footprint.
  - From those footprints, a hall is about 3.7 m wide and a room 7.3 m, with 7–15 m vertical pieces. The talk gives no exact corridor or ceiling sizes; these are derived from the grid.
- **Seven kits** cover 300+ dungeons and 400+ cells of play: cave, mines, ice cave, Nordic, fort, Dwarven and Ratway. Two kit artists and eight level designers made them over 30 months. The cave kit was used about 200 times, Nordic about 150, Ratway twice.
- **Sub-kits**: the cave kit has 7 sub-kits, and its small hall alone has about 50 pieces. Sub-kits include small, large and "epic" rooms; small, large, round and square halls; temple rooms, stairwells, shafts and balconies.
- **Caves**: the talk calls caves organic and hard to build from modules, so the cave kit is shells. The [Level Design Book](https://book.leveldesignbook.com/process/blockout/metrics/modular) names flanges and arches as the joining pieces, and calls a "gap test" essential for tunnel and cave kits.
- **Rules**: a passage at least two characters wide, so two can pass. Slopes of 30–45°.
- **Themes as kits**:
  - Nordic ruins: hewn and dressed stone, round arches, burial niches with draugr.
  - Dwemer ruins: brass, pipes, steam, and monumental halls with machines.
  - Caves and ice caves: natural rock.
  - Mines: timber sets.

### The Witcher 3 (CD Projekt Red, 2015)

- **Built by hand.** Procedural generation is limited to a base layer of grass. Level designer Miles Tost spoke of "really large dungeons", some taking an hour or more. Some are horror, some "mystical", and caves are dark unless you bring a light ([Wccftech](https://wccftech.com/witcher-3-wild-hunt-features-handcrafted-dungeons-offer-1hour-long-game-time/amp/)).
- **Pipeline**: level designers block out each place, then environment artists dress it in several passes ([Mapcore interview via GameBanshee](https://www.gamebanshee.com/kwaax)).
- **Reuse**: a report says the Novigrad temple-island dungeon reused rooms from the early elven ruins, rearranged and redressed ([GameStar](https://www.gamestar.de/artikel/versteckte-geheimnisse-witcher-3,3356666,seite3.html); seen only as a search summary). Its themes are elven ruins (marble, round arches, water), crypts, and natural caves.
- **Doors**: what we found does not confirm that its doors open by walking into them. Guides describe an interact prompt, with a tutorial for it in the Kaer Morhen prologue ([GamerGuides](https://gamerguides.com/the-witcher-3-wild-hunt/guide/walkthrough/tutorial/kaer-morhen-prologue)). D512 stands as the developers stated it. The reference may be to the feel, not a mechanic, so it is worth a word on the desk.

### Elden Ring (FromSoftware, 2022): catacombs, caves, tunnels

- **Minor dungeons** come in three kinds, plus 5 Hero's Graves ([PC Gamer](https://pcgamer.com/elden-ring-dungeons-locations-guide), [Guides4Gamers](https://guides4gamers.com/elden-ring/poi/heros-graves)):
  - Catacombs: built, with skeletons and traps.
  - Caves: natural, with demi-humans and animals.
  - Tunnels: mines, with miners and smithing stones.
- **Caves**: there are 22, 20 of them in the Lands Between. They are described as worn by water ([wiki.gg](https://eldenring.wiki.gg/wiki/Caves)).
- **Reuse**: players note that catacombs in different regions share textures, corridors and boss arenas ([Steam discussion](https://steamcommunity.com/app/1245620/discussions/0/3183486955461681051)). FromSoftware has published nothing on its tile sets.
- **Scale** (unsourced, judged from play footage against a 1.8 m figure): catacomb halls are tall and vaulted, about 5–8 m to the crown, with corridors about 3 m wide. Caves open into chambers tens of metres across.
- **Openness**: Miyazaki says its dungeons are less tightly interconnected than in earlier games because the world is larger and more open ([Gematsu](https://gematsu.com/2019/06/elden-ring-is-an-evolution-of-dark-souls-says-director-hidetaka-miyazaki)).

### Valheim (Iron Gate, 2021): burial chambers, troll caves, frost caves, infested mines

- **Generation**: a dungeon is an outside location plus an inside of room prefabs, joined at their connection sockets ([modding wiki](https://github-wiki-see.page/m/Valheim-Modding/Wiki/wiki/Creating-Dungeons)).
  - A generator's theme admits only that theme's rooms.
  - Each socket tries a room, until a maximum count is reached or a room would leave the 64 m zone.
  - Room sizes are multiples of 4 m, and a typical room is 8×8 m. End caps are size 0.
- **Frost Caves** draw from a pool of 94 rooms and place 3–64 per run ([Metabot](https://metabot.gg/en/valheim/dungeons/frost-caves)).
- **Kinds** ([summary](https://gamever.io/knowledge-base/every-dungeon-type-in-valheim-seven-kinds-and-what-each-one-is-actually-for)):
  - Burial Chambers: a stone maze, sometimes changing level.
  - Troll Caves: a single large natural chamber.
  - Sunken Crypts: brick, in the swamp.
  - Frost Caves and Infested Mines: under black marble.
- **Its burial chambers are about our scale**: stone corridors one 4 m module wide, low rooms. Its caves read as rooms of natural rock, not cubes.

### Enshrouded (Keen Games, 2024)

- **Engine**: Keen's own Holistic engine is built for voxels. Nearly all terrain can be dug or blasted ([PC Games Hardware](https://www.pcgameshardware.de/Enshrouded-Spiel-74267/Specials/1-0-Technik-Test-Benchmarks-Release-1556566/)).
- **Making the world**: it is hand-made, with procedural help ([Keen's behind-the-scenes video, via RPG Site](https://rpgsite.net/news/15193-enshrouded-creating-world-enshrouded-behind-scenes-video)).
- **Its dungeons**: Ancient Spires (climbs of traps and puzzles), Ancient Vaults, and Elixir Wells inside the Shroud ([GamerGuides](https://gamerguides.com/enshrouded/guide/ancient-spires), [TechRaptor](https://techraptor.net/gaming/guides/enshrouded-map-all-flame-shrine-and-shroud-root-locations)). Their voxel size and how they are authored are unsourced.
- **Closest to us**: built structures and carved ground in one voxel world, which is our D497 (a dungeon dug into its realm).

### Diablo IV (Blizzard, 2023)

- **Assembly**: tile sets are combined with props, interactive objects and lighting into dungeons that are "handcrafted yet procedurally created". There are 150+ of them, and the assets were made flexible so they can be reused ([Screen Rant, from the dungeon-design update](https://screenrant.com/diablo-4-quarterly-update-dungeons-tile-set/)).
- **Similarity is on purpose**: dungeons in the same place are meant to resemble each other ([Gamepressure](https://www.gamepressure.com/newsroom/diablo-4s-repetitive-dungeons-were-designed-like-this/ze52eb)). Its tile sizes are unsourced.
- **Spacing**: the camera is isometric, so its spaces are wider than a third-person game needs.

### Baldur's Gate 3 (Larian, 2023): the Underdark

- **Larian**: built from modular environment pieces made in Maya, ZBrush and Substance ([80.lv](https://80.lv/articles/a-closer-look-at-modular-assets-created-for-baldur-s-gate-3)). A "curated open world" that leans into verticality ([PCWorld](https://www.pcworld.com/article/3563515/new-baldurs-gate-iii-gameplay-shows-off-the-underdark-and-a-mushroom-man.html)).
- **The setting**: the Underdark is not one cavern. It is "many huge networks of caverns and caves", with few passages between regions ([Wikipedia](https://en.wikipedia.org/wiki/Underdark)).
- **Look** (unsourced, from play): one great cavern several hundred metres across, its roof lost in the dark. It is lit by bioluminescent fungi and crystals rather than torches, with giant mushrooms, chasms, bridges and a lake.

### Caves in other games and the metrics

- **Deep Rock Galactic**: a cave is rooms strung "like pearls on a string" by narrow tunnels. Each of its 119 hand-made room templates is intersecting spheres, half-spheres and planes, crumpled by noise. Ten biomes set the noise and the dressing ([developer post](https://devtrackers.gg/deep-rock-galactic/p/8cd5ca9c-below-decks-at-ghost-ship-cave-generation-in-deep-rock-galactic)).
- **Astroneer**: caves come from layers of noise in a voxel engine and reach down to a planet's core ([Unreal spotlight](https://www.unrealengine.com/spotlights/how-system-era-softworks-leveraged-ue4-to-create-astroneer-s-wonderful-universe)).
- **Metrics** ([Level Design Book](https://book.leveldesignbook.com/process/blockout/metrics)): a 1.8 m player, a minimum hall of 1.5–2 m, a door 1.1–1.25 by 2.2–2.5 m, and interior walls of 3 m.
  - Those are first-person minimums. A third-person camera, as ours is, sits 3–4 m behind the body and needs more room. The camera brushing walls was a fault in rounds 20–26.

## The themes they share

| Theme | Games | Scale they use | How built |
|---|---|---|---|
| Built burial halls | Skyrim Nordic ruins, Elden Ring catacombs, Valheim burial chambers, Diablo IV crypts, Witcher crypts | halls about 3–4 m wide, vaulted 4–8 m; chambers 7–15 m | modular kit on a grid |
| Natural caves | Skyrim caves and ice caves, Elden Ring caves, Valheim troll and frost caves, Witcher caves | passages 3–6 m, chambers 10–40 m | shells, sculpted meshes, voxels |
| Mines and tunnels | Skyrim mines, Elden Ring tunnels, Valheim infested mines | tight drifts 2.5–3 m, opening onto stopes and shafts | kit of timber sets in hewn rock |
| Ancient engineered halls | Skyrim Dwemer ruins, Witcher elven ruins, Enshrouded vaults and spires | monumental: 10–20 m halls, several storeys, balconies | larger-grid kit |
| A vast cavern | Baldur's Gate 3 Underdark, Elden Ring's underground rivers | hundreds of metres, its roof unseen | sculpted, bespoke |

Scales are taken from Skyrim's and Valheim's grids where sourced, and estimated from play elsewhere.

## What this means for Terra

- **More spacious.** Our 3 m cell and 4 m level sit at the first-person minimum.
  - The built themes would take a 4 m cell: a 3 m corridor between walls a cube thick.
  - Their levels would be 6 m apart, which leaves room for a 2.5–3 m springing and a 4.5–5 m crown. That is the "taller levels" round 26's spike asked for.
  - Halls would be 2–4 cells (8–16 m), with piers carrying the vaults.
  - Rooms would never span less than 2 cells.
- **Cavernous.** At least one theme should be natural rock, carved as terrain:
  - Dig operations shaped by noise, in the way Deep Rock Galactic builds its rooms, on the same mission graph and A* routing over a coarser grid.
  - Props from the realm's own rocks, where the halls' kit uses cubes.
  - The rock stays undiggable (D502).
- **The reviewers' faults are a kit's missing pieces, not the cubes' fault**:
  - Vault and arch pieces, piers, niches with remains, and a second stone. Round 26 showed cubes cannot fake them.
  - Mines and caves need timber sets and hewn or natural rock, not cubes.
- **A few kits, many dungeons.** Four or five kits, each with sub-kits by size (small, large, "epic"), and props and light to vary them. That matches D182's "hundreds of variants".

## Limits

- **Few published numbers.** Skyrim's grid and Valheim's room module are the only sourced dimensions. The other scales are estimates from play.
- **Some games are thinly sourced.** Little was found on Enshrouded's or Baldur's Gate 3's methods, and no source confirms the Witcher 3's doors open by walking into them.
- **Desk research only.** No reference frames were measured. A look round would measure the reference games' frames, as the environment skill does.
