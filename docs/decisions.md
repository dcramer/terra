# Decisions

The source of truth for what is settled; other docs must agree with it. Each decision is a file, [decisions/D198.md](decisions/D198.md) for D198, and this page is their index: a row each, its gist the decision's first sentence. Read a decision's file before building on it; `cargo xtask decision D28 D198` prints decisions in full with their lines in [the log](decisions-log.md).

**Who decides.** Design decisions (gameplay, art, scope, what the game is) belong to the developers. Technical decisions (engine, architecture, tooling, workflow) are made by the agent, recorded with the reason, and can be overruled. Owner says which kind: Design or Tech.

**Who made it.** By says who made the call. **Developers**: they said it, chose it or agreed to it; only they change it. **Agent**: an AI agent chose it, a technical question or a design question the developers left to it; it is soft. An agent may change a soft decision when it has a reason, recording the reason and a log line, without asking; anything the developers say overrules it; and when they confirm one, it becomes theirs. If the developers asked for something and the agent chose how, the how is the agent's. Open, superseded and rejected items have no one (—).

**Everything here is provisional.** This is pre-production: the record is our current thinking, written down so we can keep working from it, and any of it may change. A status says how firm an item is relative to the others, not that anything is locked: **Decided**, our current answer, not reopened without a reason; **Leaning**, a working assumption, untested or unconfirmed; **Open**, not chosen; **Rejected**, considered and dropped; **Superseded by Dn**, replaced by a later decision. Rejected and superseded items are listed last, so their numbers still resolve.

**Adding or changing one.** Edit the decision's file, never this page, then run `node tools/docs/docs.ts --render`, which writes this page; every change also gets a line at the end of the log. A new decision takes the next free number, which `cargo xtask decision` prints, as a new file `decisions/Dnnn.md` shaped like the others: `# Dnnn: Topic`; a blank line; `Area: … · Owner: … · By: … · Status: …`, its Area one of the headings below (— for a rejected or superseded one with none); a blank line; the decision; then `## Why` and the reason, in the developers' own words where they gave them. When two agents take one number at once, the rebase stops on the file both added, and whoever pushes second renumbers theirs. `cargo xtask check` checks every file's shape, that this page is their render, and that every decision cited exists.

**By area:** [The game as a whole](#the-game-as-a-whole) · [Worlds and realms](#worlds-and-realms) · [The player and survival](#the-player-and-survival) · [Items and crafting](#items-and-crafting) · [Lore, magic, combat and growth](#lore-magic-combat-and-growth) · [People and the tribe](#people-and-the-tribe) · [Building](#building) · [Art, characters and sound](#art-characters-and-sound) · [Engine and code](#engine-and-code) · [Tools and workflow](#tools-and-workflow) · [Superseded and rejected](#superseded-and-rejected) · [Log](decisions-log.md)

<!-- rendered from docs/decisions/ by `node tools/docs/docs.ts --render`: edit that, not this -->

## The game as a whole

What we are building, its tenets, scope and players.

| # | Topic | Owner | By | Status | Gist |
|---|---|---|---|---|---|
| [D1](decisions/D1.md) | Team and assets | Design | Developers | Decided | Two developers. |
| [D2](decisions/D2.md) | Multiplayer | Design | Developers | Decided | Multiplayer, with no PvP. |
| [D3](decisions/D3.md) | Tone | Design | Developers | Decided | Challenging, with goals to prepare for rather than… |
| [D7](decisions/D7.md) | Platforms | Design | Developers | Decided | Cross-platform, and above all cross-platform to develop… |
| [D8](decisions/D8.md) | Procedural content | Design | Developers | Decided | Procedural generation is the default for terrain, flora… |
| [D10](decisions/D10.md) | Browser | Design | Developers | Decided | Nice to have, not a requirement. |
| [D15](decisions/D15.md) | Astroneer | Design | Developers | Decided | Not a model for gameplay, and not the art target. |
| [D18](decisions/D18.md) | Fail state | Design | Developers | Decided | None. Setbacks only. |
| [D38](decisions/D38.md) | Licence | Design | Developers | Decided | MIT, for everything in this repository. |
| [D49](decisions/D49.md) | How we work from here | Design | Developers | Leaning | Larger design docs come before implementation. |
| [D51](decisions/D51.md) | Server-first multiplayer | Design | Developers | Decided | The game always talks to a server, which decides… |
| [D52](decisions/D52.md) | How many players a world holds | Design | Developers | Decided | A thousand concurrent players on one persistent server… |
| [D85](decisions/D85.md) | Our inspirations | Design | Developers | Decided | Kenshi, Enshrouded, Subnautica 2, *He Who Fights With*… |
| [D89](decisions/D89.md) | Direction: a living world | Design | Developers | Decided | The game's design is gameplay.md and the system pages it… |
| [D92](decisions/D92.md) | No hand-holding | Design | Developers | Decided | A principle of the game: it does not hold your hand. |
| [D93](decisions/D93.md) | Rich systems, little on screen | Design | Developers | Decided | A principle: the game is rich with systems but somewhat… |
| [D105](decisions/D105.md) | The MVP | Design | Agent | Leaning | A first playable, as a prototype of the core loop in… |
| [D107](decisions/D107.md) | Disclosing AI | Design | Developers | Decided | The game says plainly that its assets are made with AI… |
| [D122](decisions/D122.md) | Realistic but fun | Design | Developers | Decided | A core tenet of the game: realistic but fun. |
| [D125](decisions/D125.md) | Our own design, from many sources | Design | Developers | Decided | The core mechanics of *He Who Fights With Monsters* and… |
| [D127](decisions/D127.md) | Many of everything | Design | Developers | Decided | A core principle: assume a large quantity of every kind… |
| [D182](decisions/D182.md) | Hundreds of variants, not millions | Design | Developers | Decided | A core tenet: almost everything is modular and… |
| [D187](decisions/D187.md) | What we are building | Design | Developers | Decided | First and foremost, an extremely ambitious sandbox… |

## Worlds and realms

Ground, water, weather, seasons, life and how realms join.

| # | Topic | Owner | By | Status | Gist |
|---|---|---|---|---|---|
| [D4](decisions/D4.md) | World paradigm | Design | Developers | Decided | Many realms (D17, D87), each a round world… |
| [D13](decisions/D13.md) | World scale | Design | Agent | Leaning | Always round, fully procedural, and large but finite… |
| [D17](decisions/D17.md) | Worlds | Design | Developers | Decided | Many worlds, each different from the others… |
| [D20](decisions/D20.md) | Only players reshape the ground | Design | Developers | Decided | The solid ground changes only when players dig. |
| [D43](decisions/D43.md) | What can be harvested | Design | Agent | Leaning | Possibly rock and ore by mining, trees by chopping… |
| [D59](decisions/D59.md) | World variety | Design | Developers | Decided | Every world is thematic and varied: its own terrain… |
| [D62](decisions/D62.md) | The starter world | Design | Developers | Decided | Breathable air and the like: the easiest world… |
| [D63](decisions/D63.md) | Gravity | Design | Developers | Decided | Worlds have gravity, toward their centre. |
| [D72](decisions/D72.md) | Getting between worlds | Design | Developers | Leaning | Settled on 2026-10-12: there is no fast travel between… |
| [D75](decisions/D75.md) | Sea level | Design | Developers | Decided | Every world with water has a sea level… |
| [D78](decisions/D78.md) | Weather | Design | Developers | Decided | There are weather systems, and weather differs by world. |
| [D86](decisions/D86.md) | Across realms | Design | Developers | Decided | One art style across the whole game (D14)… |
| [D87](decisions/D87.md) | Realms | Design | Developers | Decided | Realms, not a set number of planets… |
| [D91](decisions/D91.md) | Reasons to explore and settle | Design | Developers | Decided | The player must have reasons to explore… |
| [D110](decisions/D110.md) | A life-sized world | Design | Developers | Decided | The world feels life-sized: things are at real-world… |
| [D116](decisions/D116.md) | Settings and generated realms | Design | Developers | Leaning | Every realm's theme fills the same gameplay roles… |
| [D134](decisions/D134.md) | Day and night | Design | — | Open | Recommended: a whole day lasts half an hour of play… |
| [D148](decisions/D148.md) | How a world fills with life | Design | Developers | Decided | The goals in biomes.md, with the developers' answers of… |
| [D149](decisions/D149.md) | Seasons | Design | Developers | Decided | Worlds have seasons, and they change play… |
| [D150](decisions/D150.md) | Regrowth | Design | Developers | Decided | What is cut or harvested grows back… |
| [D174](decisions/D174.md) | Felling trees | Design | Developers | Decided | Trees behave as in The Forest: chopped down… |
| [D301](decisions/D301.md) | The lush forest a tall conifer stand, looming | Design | Developers | Decided | The lush forest is a tall, open conifer stand as Sons of… |
| [D321](decisions/D321.md) | Biome demos: a mountain lake, a tundra, a living town | Design | Agent | Leaning | The next demos after the lush forest, as demos.md says… |
| [D350](decisions/D350.md) | The market town paved, with shops, women and a horizon | Design | Developers | Decided | The town's streets and square are paved… |
| [D365](decisions/D365.md) | Townsfolk dressed by role, sex and seed | Tech | Agent | Decided | Serves D350's varied people, women among them. |
| [D419](decisions/D419.md) | Winter strips the plants; falling snow grades cool; trails are shared and fade | Design | Developers | Decided | In winter the land is in its winter state. |
| [D425](decisions/D425.md) | Mountains: horns and Witcher-like ranges first, two thirds rock, sites re-found once | Design | Developers | Decided | The three questions of the mountain look note: |
| [D444](decisions/D444.md) | A horn stands in a massif of lesser tops, kept off the valley pins | Tech | Agent | Decided | Each horn (D425) sends ridges out along two or three of… |
| [D473](decisions/D473.md) | Dungeons are made in stages: a mission graph, placed on 3 m cells and 4 m levels | Tech | Agent | Leaning | A dungeon is a theme and a seed, made in stages… |
| [D474](decisions/D474.md) | Dungeon cells are dressed by wave function collapse over a theme's socketed modules | Tech | Agent | Leaning | Every placed cell takes a module from its theme's set… |

## The player and survival

Needs, death, the body, the map and options.

| # | Topic | Owner | By | Status | Gist |
|---|---|---|---|---|---|
| [D61](decisions/D61.md) | Survival attributes | Design | Developers | Decided | Health, stamina, breath (D74), hunger and thirst… |
| [D74](decisions/D74.md) | Swimming and breathing | Design | Developers | Decided | The character can swim, using stamina for now. |
| [D76](decisions/D76.md) | A map | Design | Developers | Decided | There is a map system. |
| [D80](decisions/D80.md) | Options menu | Design | Developers | Decided | An options menu with the settings players expect… |
| [D101](decisions/D101.md) | Food and death | Design | Developers | Decided | Food is essential: going without weakens you and in the… |
| [D111](decisions/D111.md) | Character creator | Design | Developers | Decided | A creator of a few sliders, not templates: sex, build… |
| [D153](decisions/D153.md) | Body temperature | Design | Developers | Decided | A survival attribute of its own, for players and… |
| [D171](decisions/D171.md) | Coming back from death | Design | Agent | Decided | A soul that dies falls into the astral and is lost… |
| [D312](decisions/D312.md) | Climate shared where it changes play, each machine's where it is only seen | Design | Developers | Decided | What a season or the weather does to play is worked out… |
| [D313](decisions/D313.md) | What grows where and when: species' niches and years, the world's climate | Design | Agent | Decided | A species carries, as data, its niche… |
| [D314](decisions/D314.md) | Climate in code: fields, an integrated past, samples and maps | Tech | Agent | Decided | Temperature, daylight, precipitation… |
| [D315](decisions/D315.md) | Climate's crate, clock, weather and cells | Tech | Agent | Decided | The world's clock is days (`WorldTime`, a 64-bit count… |
| [D327](decisions/D327.md) | Climate samples, maps and plants' years in code | Tech | Agent | Decided | A `ClimateSample` on what cares (the walker's player… |
| [D328](decisions/D328.md) | Body temperature and the going in code | Tech | Agent | Decided | D153's first effects, as climate.md orders them. |

## Items and crafting

Equipment, packs, tools, stations and icons.

| # | Topic | Owner | By | Status | Gist |
|---|---|---|---|---|---|
| [D64](decisions/D64.md) | Equipment | Design | Developers | Decided | An equipment grid, a character sheet… |
| [D66](decisions/D66.md) | Action bar | Design | Developers | Decided | Items are put on an action bar to be used… |
| [D83](decisions/D83.md) | Items in code | Tech | Agent | Decided | The rules for items and containers live in an… |
| [D84](decisions/D84.md) | Inventory baseline | Design | Developers | Decided | Subnautica, for now: a backpack grid where items take up… |
| [D96](decisions/D96.md) | Tools | Design | Developers | Decided | No science-fiction toolset, and no single tool that… |
| [D99](decisions/D99.md) | Wear and repair | Design | Developers | Decided | For now: tools, weapons and armour wear with use… |
| [D106](decisions/D106.md) | Packs | Design | Developers | Decided | A pack is a container: an item worn in the pack slot… |
| [D109](decisions/D109.md) | Crafting at stations | Design | Developers | Decided | Making takes time. |
| [D118](decisions/D118.md) | Ammunition in a quiver | Design | Developers | Decided | A quiver is a container holding a stack of arrows… |
| [D129](decisions/D129.md) | How item icons are made | Tech | Agent | Decided | Icons are image files baked during development and… |
| [D130](decisions/D130.md) | What item icons look like | Design | Agent | Leaning | In the art style (D14, now direction D, art-style.md)… |
| [D151](decisions/D151.md) | Expanding a station | Design | Developers | Decided | Stations are expanded and upgraded… |
| [D460](decisions/D460.md) | Where the off hand's item comes from | Design | — | Open | How a shield, a torch or a second blade gets into the… |

## Lore, magic, combat and growth

Essences, ranks, skills, fighting, creatures and threats.

| # | Topic | Owner | By | Status | Gist |
|---|---|---|---|---|---|
| [D68](decisions/D68.md) | Research | Design | Developers | Decided | A research system unlocks technology and things to craft… |
| [D88](decisions/D88.md) | Player skills | Design | Developers | Decided | There is a player progress system with a lot of skills… |
| [D97](decisions/D97.md) | Combat and view | Design | Developers | Decided | Combat is important. |
| [D98](decisions/D98.md) | Essences | Design | Developers | Decided | The books' system of essences, abilities and ranks is… |
| [D100](decisions/D100.md) | Taming and mounts | Design | Developers | Leaning | Animals can probably be tamed, and ridden as mounts. |
| [D102](decisions/D102.md) | Mana | Design | Developers | Decided | There is mana as well as stamina: a pool of its own for… |
| [D103](decisions/D103.md) | Where monsters come from | Design | Developers | Decided | Monsters form from pooled magic; there is ordinary… |
| [D117](decisions/D117.md) | Archery | Design | Developers | Decided | Archery is a skill of its own, grown by shooting (D88). |
| [D119](decisions/D119.md) | Hitboxes and critical hits | Design | Developers | Decided | Combat has hitboxes: where a blow lands matters… |
| [D124](decisions/D124.md) | Where growth comes from | Design | Developers | Decided | Magic and special abilities come from essences (D98). |
| [D126](decisions/D126.md) | Crossing a rank | Design | Developers | Leaning | Advancing to the next rank may need more than practice… |
| [D132](decisions/D132.md) | How damage shows | Design | Developers | Leaning | Damage shows on what was hit, as wounds and how it… |
| [D156](decisions/D156.md) | Gods | Design | Developers | Decided | There are gods, because they enrich the world. |
| [D157](decisions/D157.md) | Beings of the gap between realms | Design | Developers | Leaning | Something older and larger than gods lives in the gap… |
| [D158](decisions/D158.md) | Names and generated lore | Design | Agent | Leaning | Proposed in naming.md: the books' names for the core… |
| [D159](decisions/D159.md) | Rank names | Design | Developers | Decided | The ranks are the books' own, low to high: iron, bronze… |
| [D163](decisions/D163.md) | The magic system | Design | — | Open | Proposed in magic.md, except the parts marked decided. |
| [D164](decisions/D164.md) | Attributes | Design | Developers | Decided | Four attributes, as in the books: Power, Speed… |
| [D165](decisions/D165.md) | Monster cores | Design | Developers | Decided | Monsters leave cores. |
| [D167](decisions/D167.md) | Money is magic | Design | Developers | Decided | Money works as in the books: the currency is magic… |
| [D168](decisions/D168.md) | Fragments of essences | Design | Developers | Decided | Small fragments of one essence's kind… |
| [D172](decisions/D172.md) | Threats, and an organisation across realms | Design | Developers | Decided | No single long threat: each realm's threats vary… |
| [D173](decisions/D173.md) | Knowledge of essences, and essence users among people | Design | Developers | Decided | What a world knows of what stones give grows as anyone… |
| [D180](decisions/D180.md) | Combat's baseline | Design | Developers | Decided | Enshrouded is the baseline for combat: fast-paced, fun… |
| [D250](decisions/D250.md) | Worman, Drillhog and Seawish | Design | Developers | Decided | Monsters drawn by a developer's daughter… |
| [D380](decisions/D380.md) | Small animals in every biome | Design | Developers | Decided | Every biome has a few small animals that belong there… |
| [D382](decisions/D382.md) | Fliers, and the dragon | Tech | Agent | Decided | Creatures that fly are a body plan of their own… |
| [D402](decisions/D402.md) | Staged figures, told over the remote | Tech | Agent | Decided | A film's set piece on the ground is played by staged… |
| [D405](decisions/D405.md) | A playable sword-and-shield prototype comes before the movement system's layers | Design | Developers | Decided | Before the layers of docs/movement.md are built… |
| [D447](decisions/D447.md) | The fight's rules in a plain crate, its clips checked by tools that build nothing | Tech | Agent | Decided | The move runner's rules leave `terra_creatures::melee`… |
| [D449](decisions/D449.md) | Out of stamina in a fight | Design | Developers | Decided | What a body with too little stamina may still do with a… |
| [D451](decisions/D451.md) | The fight camera | Tech | Agent | Decided | While a weapon is drawn the player's camera frames the… |
| [D452](decisions/D452.md) | Combat is held by a scorecard of criteria and a ratchet on their numbers | Tech | Agent | Decided | What combat must get right is data… |
| [D453](decisions/D453.md) | The locked-on fight camera stands off the line to the target | Design | Developers | Decided | Locked on to an enemy, the fight camera… |
| [D455](decisions/D455.md) | A fighter's planted feet are locked to the ground and step when the body leaves them | Tech | Agent | Decided | After the clip has posed a fighter and before the… |
| [D456](decisions/D456.md) | A landed blow flashes and kicks the camera, it does not freeze the fight | Design | Developers | Decided | A blow that lands holds the fighters for at most 0.05 s… |
| [D459](decisions/D459.md) | What each hand holds, and how its animation is chosen, by rule from data | Tech | Agent | Leaning | The technical parts of holding.md. |
| [D461](decisions/D461.md) | Which stances come first, after sword and shield | Design | — | Open | Which of holding.md's stances are built… |
| [D462](decisions/D462.md) | Dual wielding | Design | — | Open | Whether a body may hold a one-hander in each hand… |
| [D464](decisions/D464.md) | Holding a one-handed weapon in two hands | Design | — | Open | Whether a one-hander can be held in both hands for other… |
| [D465](decisions/D465.md) | How many variants of a move | Design | — | Open | How many takes of each move are kept and drawn without… |

## People and the tribe

NPCs, factions, claims, recruits and outposts.

| # | Topic | Owner | By | Status | Gist |
|---|---|---|---|---|---|
| [D70](decisions/D70.md) | NPCs | Design | Developers | Decided | There are NPCs. What they are and do is open. |
| [D73](decisions/D73.md) | Agents as players | Design | Developers | Decided | The game can be played headless, by an agent alone… |
| [D90](decisions/D90.md) | Answering threats | Design | Developers | Decided | Part of survival is establishing claims and outposts and… |
| [D94](decisions/D94.md) | Claims | Design | Developers | Decided | You build within areas you claim, so the world is not… |
| [D95](decisions/D95.md) | Factions and settlements | Design | Developers | Decided | The world needs systems like Kenshi's for how it exists… |
| [D104](decisions/D104.md) | Claim size | Design | Developers | Leaning | A claim starts at a reasonable size and can be upgraded… |
| [D123](decisions/D123.md) | Recruits progress | Design | Developers | Decided | People who join you have their own progression… |
| [D161](decisions/D161.md) | The tribe: recruits who look after themselves | Design | Developers | Decided | You recruit people, bring them back safely… |
| [D162](decisions/D162.md) | How recruits take work | Design | Developers | Decided | Recruits take part on their own, in whatever way the… |
| [D166](decisions/D166.md) | Tribes and outposts | Design | Developers | Decided | A tribe is its players and recruits… |
| [D169](decisions/D169.md) | Recruits' bodies, needs and death | Design | Developers | Decided | Recruits are generally much like a player: health… |
| [D170](decisions/D170.md) | Handing a plan to the tribe | Design | Developers | Leaning | Agreed for now, perhaps to be revisited… |
| [D297](decisions/D297.md) | Recruits at a build | Design | — | Open | D161 says recruits help by carrying to a build… |
| [D324](decisions/D324.md) | Town life in code | Tech | Agent | Decided | A person's day is a schedule of activities by role in… |
| [D330](decisions/D330.md) | A market town's layout | Tech | Agent | Decided | `terra_town::town` lays a market town from its seed and… |
| [D344](decisions/D344.md) | A town's ground worn, its smoke grey, its people carrying their tools and going round visitors | Tech | Agent | Decided | The ground cover says how trodden a place is… |
| [D346](decisions/D346.md) | A town's furnishing by rules | Tech | Agent | Decided | `terra_town::town` lays, after the yards and by its… |
| [D352](decisions/D352.md) | A market's stalls and their goods: CC0 scans by recipe, a trade a stall by the seed | Tech | Agent | Decided | For D350's stalls with varied goods… |

## Building

How structures are planned, built, drawn and coded.

| # | Topic | Owner | By | Status | Gist |
|---|---|---|---|---|---|
| [D48](decisions/D48.md) | Construction | Design | Developers | Decided | Building rests on each base's voxel grid… |
| [D67](decisions/D67.md) | Build menu | Design | Developers | Decided | Like Subnautica: you equip a building tool… |
| [D108](decisions/D108.md) | One system for everything built | Design | Developers | Decided | Everything built in a world, the player's and the… |
| [D131](decisions/D131.md) | Building on uneven ground | Design | Developers | Decided | A structure set into the ground cuts the ground away… |
| [D139](decisions/D139.md) | Structures are planned, then constructed | Design | Developers | Decided | A structure is laid out as a plan that costs nothing… |
| [D140](decisions/D140.md) | Large materials, carried | Design | Developers | Decided | Large structures need large materials, such as logs… |
| [D141](decisions/D141.md) | Building draws on storage | Design | Developers | Decided | Building takes what it needs from storage nearby… |
| [D142](decisions/D142.md) | Stability | Design | Developers | Decided | No load or span in the MVP, wanted later… |
| [D143](decisions/D143.md) | Rooms below ground | Design | Developers | Decided | Rooms can be built underground, cutting the ground away… |
| [D144](decisions/D144.md) | Building controls | Design | Developers | Decided | The building controls and snapping in building.md… |
| [D146](decisions/D146.md) | What a construction's pieces are | Design | Developers | Decided | Each kind of construction is a kit of pieces sized to… |
| [D152](decisions/D152.md) | Walls, not rooms | Design | Developers | Decided | Blueprints are walls, floors and roofs… |
| [D154](decisions/D154.md) | How placing walls, floors and roofs works | Design | Developers | Decided | The Sims' way with our materials: the player draws the… |
| [D155](decisions/D155.md) | What a building is | Design | Developers | Decided | The game knows its buildings: the walls… |
| [D160](decisions/D160.md) | How structures work in code | Tech | Agent | Decided | What stands is kept by `terra_build` as half-metre cubes… |
| [D175](decisions/D175.md) | Sizes of structural parts | Design | Agent | Leaning | Walls, floors, storeys, doors and windows are D181's. |
| [D176](decisions/D176.md) | A plan view from above, and heights not storeys | Design | — | Open | An idea for later, not to be built yet: Plan mode… |
| [D177](decisions/D177.md) | How kit pieces are made and drawn | Tech | Agent | Decided | One layout per wall, by its kit's rule… |
| [D178](decisions/D178.md) | How a log wall looks | Design | Agent | Decided | Round logs, made to look hewn and grown rather than… |
| [D179](decisions/D179.md) | Posts, and mixing kinds of construction | Design | Developers | Decided | As building-kits.md says: a post is a part on any grid… |
| [D181](decisions/D181.md) | Structures strictly on the voxel grid | Design | Developers | Decided | Everything built is half-metre cubes of a kind of… |
| [D192](decisions/D192.md) | Rules for construction pieces | Design | Developers | Decided | construction-001.md: Enshrouded's way on our cubes… |
| [D193](decisions/D193.md) | What holds, what falls, and plans' limits | Design | Developers | Decided | Nothing is taken away while anything is attached to it… |
| [D194](decisions/D194.md) | Construction defaults | Design | Agent | Decided | Footings fill down to the ground and stop there… |
| [D195](decisions/D195.md) | How the construction system is built | Tech | Agent | Decided | As building-plan.md says: `terra_build` split into… |
| [D197](decisions/D197.md) | How block kinds are drawn | Design | Developers | Decided | Stone, brick, plaster and earth are drawn as their… |
| [D199](decisions/D199.md) | The construction model's rules in code | Tech | Agent | Decided | As `terra_build` holds them (step 1 of… |
| [D200](decisions/D200.md) | How the timber kind is drawn | Design | Developers | Decided | Timber is drawn as generated long members… |
| [D201](decisions/D201.md) | Plans and building in the game | Tech | Agent | Decided | As step 4 of building-plan.md builds it… |
| [D206](decisions/D206.md) | Footings, floors and steps in code | Tech | Agent | Decided | As `terra_build` lays them (D194): footings are cubes of… |
| [D207](decisions/D207.md) | Openings cut into what is built, and doors that swing | Tech | Agent | Decided | An opening asked for where a wall is built marks its… |
| [D208](decisions/D208.md) | Pieces placed in grid units | Tech | Agent | Decided | A layout's pieces are placed in the base's grid in ticks… |
| [D210](decisions/D210.md) | Trims where openings and kinds meet, and the kit checker | Tech | Agent | Decided | Round every opening in a block kind's wall… |
| [D211](decisions/D211.md) | The look of building: on par with Enshrouded | Design | Developers | Decided | Building takes an approach to voxels like Enshrouded's… |
| [D213](decisions/D213.md) | How bases are drawn | Tech | Agent | Decided | As step 3 of building-plan.md builds it… |
| [D215](decisions/D215.md) | Dressing a kit's edges by their role | Design | Developers | Decided | Each exposed face, edge and corner of a kit's cubes has… |
| [D220](decisions/D220.md) | Cubes cut to shapes, posts, and timber masses | Tech | Agent | Decided | As step 5 of building-plan.md builds D193's shapes… |
| [D225](decisions/D225.md) | Doors and shutters as props; windows open | Design | Developers | Decided | Doors and windows' shutters are props fitted into a hole… |
| [D226](decisions/D226.md) | Building by hand, roofs too | Design | Developers | Decided | Players build by hand as in Enshrouded: brushes of 1… |
| [D227](decisions/D227.md) | The next materials: earth, wattle and daub | Design | Developers | Decided | After logs, stone and hewn timber, the next kits are… |
| [D228](decisions/D228.md) | Decay and overgrowth | Design | Developers | Decided | Moss, ivy, weathering and decay on built cubes are the… |
| [D229](decisions/D229.md) | The village laid out from its seed | Design | Agent | Decided | The MVP's settlement (D108) is a street village made by… |
| [D231](decisions/D231.md) | More shapes, and a cube's shape in five bits | Tech | Agent | Decided | Amends D199's cube layout and adds to D220's shapes… |
| [D232](decisions/D232.md) | The material in hand, changing it in place, and how kinds meet | Tech | Agent | Decided | The hammer holds a material (`Builder::material`)… |
| [D233](decisions/D233.md) | Brushes, carving a box and taking out the cube aimed at | Design | Agent | Decided | As D226 asks, in the hammer, on the operations there… |
| [D236](decisions/D236.md) | Earth, and wattle and daub as a framed kind | Tech | Agent | Decided | D227's kits, as data (D136): **earth**… |
| [D238](decisions/D238.md) | Laying a base once a change, and making only the detail that is seen | Tech | Agent | Decided | A base is laid whole once, on a worker… |
| [D241](decisions/D241.md) | Roofs as cubes, built by hand | Tech | Agent | Decided | As D226 asks, in place of D209's roof sections… |
| [D251](decisions/D251.md) | One material system for walls, floors and roofs, of hand-made pieces | Design | Developers | Decided | Every kind, wall, floor or roof, is drawn by one system… |
| [D252](decisions/D252.md) | Whether the terrain shares building's materials | Design | Agent | Decided | Yes: the terrain and building share one library of… |
| [D253](decisions/D253.md) | The hammer leads with building by hand | Design | Agent | Decided | The hammer's menu starts with building by hand… |
| [D257](decisions/D257.md) | Auditing building, and the holes it closed | Tech | Agent | Decided | `cargo xtask rules` audits building end to end… |
| [D258](decisions/D258.md) | One material system, built: surfaces, modules and stones | Tech | Agent | Decided | D251 built as building-plan.md says… |
| [D260](decisions/D260.md) | Parts that touch and agree are one | Tech | Agent | Decided | Amends D199's parts: a part groups cubes for the… |
| [D261](decisions/D261.md) | Rooms, doorways and door steps from the cubes alone | Tech | Agent | Decided | Rooms (D155) are the space the built cubes close in all… |
| [D262](decisions/D262.md) | What makes a generated log read as real | Tech | Agent | Decided | Within D200, a log's face is one material made of two… |
| [D263](decisions/D263.md) | Thick timber walls laid as skins along their faces | Tech | Agent | Decided | A cube of a timber kind whose run across a wall is two… |
| [D265](decisions/D265.md) | Claims keep ten metres apart, and clear of villages | Tech | Agent | Decided | D193's "claims do not overlap, keep a margin" enforced… |
| [D266](decisions/D266.md) | The haze under trees | Tech | Agent | Decided | The volumetric haze (D217, D244) takes its sky light at… |
| [D277](decisions/D277.md) | Wall or floor, read from the cubes | Tech | Agent | Decided | Whether a cube is in a wall is read from the cubes round… |
| [D278](decisions/D278.md) | Bases far off: a hull, merged by cell to the horizon, and streamed by distance | Tech | Agent | Decided | What is drawn of a base by distance… |
| [D279](decisions/D279.md) | No open sky through the leaves of a forest, and its haze green in a glade | Tech | Agent | Decided | Leaves take 0.05 of Bevy's own diffuse transmission… |
| [D280](decisions/D280.md) | Villages and scale scenes put up by the operations | Tech | Agent | Decided | As D108 asks, a village's buildings… |
| [D281](decisions/D281.md) | How far an edit lays a base again | Tech | Agent | Decided | An edit lays again only the sections its pieces can… |
| [D282](decisions/D282.md) | A calmer wind: gusts to one and a half times the steady wind, leaf clusters that rustle as one | Tech | Agent | Decided | Gusts run from under a half to one and a half times the… |
| [D284](decisions/D284.md) | A block surface's triangles only where its shape bends | Tech | Agent | Decided | How D197's surface is meshed, as terrain and voxel… |
| [D287](decisions/D287.md) | Building's junk checked by meshes and by frames, and ratcheted | Tech | Agent | Decided | What a renderer would draw as junk is checked without a… |
| [D290](decisions/D290.md) | One way to draw every construction material, for hundreds of materials | Design | Developers | Decided | As construction-002.md says. |
| [D295](decisions/D295.md) | Storage, and building from it | Tech | Agent | Decided | As step 7 of building-plan.md builds D141 on D84's… |
| [D296](decisions/D296.md) | How big materials are carried and set | Design | — | Open | D140 says logs and other big materials are carried to… |
| [D298](decisions/D298.md) | A change of material within a pattern is a repaint at once | Design | Agent | Decided | Change material between two kinds of one family… |
| [D305](decisions/D305.md) | Loss and repair: damage, collapse, fragments and mending | Tech | Agent | Decided | As `terra_build::loss` holds them… |
| [D306](decisions/D306.md) | Decks framed on their posts, and stairs that bear | Tech | Agent | Decided | A timber deck is framed within its layer of cubes as a… |
| [D320](decisions/D320.md) | What every kit's surfaces are shaded and mapped by, beyond its own data | Tech | Agent | Decided | Three rules in shared code, so no kit needs its own… |
| [D333](decisions/D333.md) | The building baseline: wood and stone, built and destroyed | Design | Developers | Decided | Construction work is held to a minimal baseline until it… |
| [D339](decisions/D339.md) | Acts on a base worked out off the main thread and swapped in | Tech | Agent | Decided | An act on the player's base (D21's operations: a fill, a… |
| [D343](decisions/D343.md) | Walls on roofs, the roof whole round them | Design | Developers | Decided | Walls, chimneys and anything else may stand on a roof's… |
| [D348](decisions/D348.md) | The voxel contract: who draws what between cubes, and every junction proved | Tech | Agent | Decided | building-voxels.md is the contract for construction… |
| [D349](decisions/D349.md) | A material's pack checked as it is added | Tech | Agent | Decided | D290's known failure modes, the inputs a material made… |
| [D351](decisions/D351.md) | Building's code a module a concern, and what nothing uses deleted | Tech | Agent | Decided | `terra_build` keeps one concern a module… |
| [D353](decisions/D353.md) | No old saves | Tech | Developers | Decided | Only the save format of now is read: a game save… |
| [D355](decisions/D355.md) | Destruction leaves a blueprint to rebuild | Design | Developers | Decided | Later, not in the current baseline (D333)… |
| [D358](decisions/D358.md) | A building going up drawn from what is built | Design | Developers | Decided | Every role of a kind's cubes that are in… |
| [D362](decisions/D362.md) | Construction materials' texture detail near | Tech | Agent | Decided | How D290's texture arrays carry detail a metre off… |
| [D367](decisions/D367.md) | One ground under a column, its own once built on | Tech | Agent | Decided | `Ground::height_cm` is the one answer to how high the… |
| [D373](decisions/D373.md) | Shade laid on a building's colours as light, and a log's underside shaded once | Tech | Agent | Decided | A building's colours are as on screen… |
| [D377](decisions/D377.md) | Fire in what is built | Tech | Agent | Decided | A cellular fire on a base's cubes, as Far Cry 2's and… |

## Art, characters and sound

The look, models, bodies and gear, textures and audio.

| # | Topic | Owner | By | Status | Gist |
|---|---|---|---|---|---|
| [D14](decisions/D14.md) | Art direction | Design | Developers | Decided | Textured, lightly stylised realism… |
| [D54](decisions/D54.md) | Models, for now | Tech | Agent | Leaning | Free CC0 low-polygon packs (Quaternius)… |
| [D112](decisions/D112.md) | How body templates are made | Tech | Agent | Leaning | One base body, made once and checked: one mesh… |
| [D113](decisions/D113.md) | How sound is put together | Tech | Agent | Decided | Gameplay says what happened as a `Noise` (what, where)… |
| [D114](decisions/D114.md) | What the game sounds like, for now | Design | Agent | Leaning | Sound carries information; music is sparse and follows… |
| [D115](decisions/D115.md) | How characters are made and rigged | Tech | Agent | Leaning | A character's base body is drawn as a design sheet in a… |
| [D120](decisions/D120.md) | Scripting Tripo | Tech | Agent | Decided | One command-line tool, tools/tripo… |
| [D128](decisions/D128.md) | How equipment fits every body | Tech | Agent | Leaning | Every item fits every body by contract… |
| [D147](decisions/D147.md) | Props with no free model | Tech | Agent | Leaning | A prop no free pack covers is made by tools/props… |
| [D186](decisions/D186.md) | What the base body wears | Design | Developers | Decided | The base bodies wear plain, close-fitting underwear… |
| [D188](decisions/D188.md) | Texture budgets for characters and gear | Tech | Agent | Decided | Source art keeps Tripo's maps as made… |
| [D189](decisions/D189.md) | Modular packs and belts | Tech | Agent | Decided | A pack is a bag and a harness. |
| [D190](decisions/D190.md) | How the ground and the light are drawn | Tech | Agent | Decided | Covered ground is drawn from real textures, soil, grass… |
| [D191](decisions/D191.md) | Where materials come from | Tech | Agent | Decided | Tileable materials (ground, plaster, planks, stone) come… |
| [D202](decisions/D202.md) | How ground cover is drawn | Tech | Agent | Decided | The cover tier of biomes.md is `terra_world::cover`… |
| [D203](decisions/D203.md) | How trees change detail | Tech | Agent | Decided | A tree's levels of detail switch at distances that grow… |
| [D204](decisions/D204.md) | How sun shadows are kept steady and let through | Tech | Agent | Decided | The sun's shadow direction moves in steps of a twentieth… |
| [D212](decisions/D212.md) | How the air, the exposure and plants' materials are drawn | Tech | Agent | Decided | The sky's colour, the sun's colour through the air and… |
| [D214](decisions/D214.md) | Temporal anti-aliasing, eye adaptation and bark that reads | Tech | Agent | Decided | The camera resolves with Bevy's temporal anti-aliasing… |
| [D216](decisions/D216.md) | Light under the canopy, and leaves that give way to the camera | Tech | Agent | Decided | The sky lights shade 1.4 times as brightly as D212's… |
| [D217](decisions/D217.md) | Haze and sun shafts | Tech | Agent | Decided | A thin haze round the camera that the sun lights in… |
| [D218](decisions/D218.md) | Bark that reads as a real trunk | Tech | Agent | Decided | A grown trunk is no longer a smooth tube… |
| [D219](decisions/D219.md) | What lies on the forest floor | Tech | Agent | Decided | The forest floor has its fourth storey from biomes.md… |
| [D221](decisions/D221.md) | Trees and grass that do not pop while moving | Tech | Agent | Decided | A crown's levels of detail are one set of cards… |
| [D222](decisions/D222.md) | How trees, bushes, small plants and rocks are made | Tech | Agent | Decided | Trees, bushes, herbs and flowers are grown by rules… |
| [D223](decisions/D223.md) | Crowns that thin card by card | Tech | Agent | Decided | A crown near the camera is one mesh with all its cards… |
| [D224](decisions/D224.md) | Haze between the trees | Tech | Agent | Decided | A distance haze at the forest's own scale… |
| [D230](decisions/D230.md) | How far detail reaches, and checking that nothing pops | Tech | Agent | Decided | Every distance at which something changes its level of… |
| [D234](decisions/D234.md) | Shadows that hand over without popping, and a guard on Bevy's range limit | Tech | Agent | Decided | Bevy 0.20's shadow passes never dither a crossfade… |
| [D235](decisions/D235.md) | The lush forest's kinds of tree | Tech | Agent | Decided | The lush forest grows three kinds… |
| [D240](decisions/D240.md) | Crossfades TAA resolves on every material, and eyes that open indoors | Tech | Agent | Decided | Bevy's standard material dithers its crossfades between… |
| [D242](decisions/D242.md) | The lush forest on rolling ground, its floor fuller | Tech | Agent | Decided | The lush forest's world stands on gently rolling ground… |
| [D243](decisions/D243.md) | What stops a walker in the forest, and checking it | Tech | Agent | Decided | Every rock, boulder, stump and fallen log in the lush… |
| [D244](decisions/D244.md) | Mist, and the hour in the air | Tech | Agent | Decided | The haze volume (D217) carries a density picture… |
| [D245](decisions/D245.md) | How closed the canopy is, measured | Tech | Agent | Decided | The vantage sheet also looks straight up from eye… |
| [D246](decisions/D246.md) | The lush forest's floor follows the light | Tech | Agent | Decided | What grows on the lush forest's floor is worked out from… |
| [D247](decisions/D247.md) | The lush forest's trees: a closed canopy, leaves lit through, bark in proportion | Tech | Agent | Decided | **Canopy:** grown oaks stouter (trunks 3.8 to 4.6% of… |
| [D248](decisions/D248.md) | Spruces green, and a skirt that stops a walker | Tech | Agent | Decided | The spruce's needle picture is recoloured in the atlas… |
| [D264](decisions/D264.md) | Trees under and close: leaves lit through every layer, bark at its real albedo, roots and modelled trunks in the grown bark | Tech | Agent | Decided | **Leaves:** a crown's cards all have normals leaning out… |
| [D267](decisions/D267.md) | Solid things fitted to their meshes, and a step onto low ones | Tech | Agent | Decided | Every rock, boulder, stump, log and tree stops a walker… |
| [D268](decisions/D268.md) | The lush forest's middle layer, and thin grass in tussocks | Tech | Agent | Decided | Between the floor and the canopy stands a middle layer… |
| [D269](decisions/D269.md) | Leaf cards composed from scanned leaves, with a normal map | Tech | Agent | Decided | A broadleaf card is composed, as studios build their… |
| [D270](decisions/D270.md) | Sky light under the canopy from a volume baked from the crowns | Tech | Agent | Decided | Everything under and among a forest's crowns takes its… |
| [D271](decisions/D271.md) | Plants make way for the camera and the player | Tech | Agent | Decided | Amends D216 and D264 for a camera that follows a figure. |
| [D272](decisions/D272.md) | Leaves cut by their picture alone, and lit through by the sky above them | Tech | Agent | Decided | A leaf's alpha in the main pass is its picture's… |
| [D273](decisions/D273.md) | The forest carried to the horizon | Tech | Agent | Decided | As shipped open worlds draw what lies past the trees… |
| [D274](decisions/D274.md) | The third-person camera on a boom, and plants that fade whole | Tech | Agent | Decided | As shipped third-person games do… |
| [D275](decisions/D275.md) | The forest floor in layers that settle into each other, with litter lying on it | Tech | Agent | Decided | **The ground's layers** (`assets/shaders/ground.wesl`)… |
| [D276](decisions/D276.md) | The lush forest's leaves at their own size, its pines each their own, spruce cards drawn needle by needle | Tech | Agent | Decided | **Leaves:** a composed card says how long its leaves are… |
| [D285](decisions/D285.md) | A third-person camera that keeps calm among trunks | Tech | Agent | Decided | As shipped cameras keep collision apart from occlusion… |
| [D286](decisions/D286.md) | A forest's shade as dark as Sons of the Forest's, measured | Tech | Agent | Decided | Light is tuned against Sons of the Forest's own daytime… |
| [D289](decisions/D289.md) | Trees that hold up from below, and a pine stand that reads as pines | Tech | Agent | Decided | Amends D235, D247, D264, D267, D268 and D271. |
| [D291](decisions/D291.md) | Leaves true to life, not flat | Design | Developers | Decided | A forest's leaves are drawn as real leaves seen in a… |
| [D292](decisions/D292.md) | Leaf cards graded to living leaves, each leaf bent, worn and glossy as its own | Tech | Agent | Decided | For D291. |
| [D293](decisions/D293.md) | The lush forest's floor green under its trees, its litter umber | Tech | Agent | Decided | Amends D246 and D275, as… |
| [D294](decisions/D294.md) | Bark, moss and the floor's surface without repeats or felt | Tech | Agent | Decided | **Bark**: a veteran oak's triplanar bark is hex-tiled on… |
| [D299](decisions/D299.md) | Under the trees the air's distances shorter | Tech | Agent | Decided | The air is drawn as an Earth-sized one with its… |
| [D300](decisions/D300.md) | Leaves lit through mostly toward the sun | Tech | Agent | Decided | A leaf passes the sun on mostly looking toward it… |
| [D302](decisions/D302.md) | Night graded as moonlight | Tech | Agent | Decided | The day's warm grade goes with the sun… |
| [D303](decisions/D303.md) | The model contract | Tech | Agent | Decided | Every model the game places obeys one checked contract… |
| [D304](decisions/D304.md) | Props drawn in their own textures | Tech | Agent | Decided | A prop keeps the textures Tripo made, by default… |
| [D309](decisions/D309.md) | Crowns that read from below, a Scots pine's stubbed bole, bark with depth and things growing on it | Tech | Agent | Decided | **Oaks** carry their twigs evenly along each branch's… |
| [D310](decisions/D310.md) | Night lit by a high moon, its sky a glow | Tech | Agent | Decided | By night most light is the moon's: 700 lux to the sky's… |
| [D311](decisions/D311.md) | A place's look measured against references kept as data | Tech | Agent | Decided | A biome's look is aimed at a reference… |
| [D316](decisions/D316.md) | Locomotion clips made, not taken | Tech | Agent | Decided | The idle, walk, jog and sprint the game plays are made… |
| [D317](decisions/D317.md) | Under the trees shiny things mirror leaves, not the open sky | Tech | Agent | Decided | The sky's reflection is scaled by how much sky is open… |
| [D318](decisions/D318.md) | The lush forest a tall Scots pine stand, each pine's crown pads of needle tufts | Tech | Agent | Decided | Amends D223, D235, D247, D273, D276, D289 and D309… |
| [D329](decisions/D329.md) | What the player wears | Design | Developers | Decided | A player starts in a leather coat with long sleeves… |
| [D334](decisions/D334.md) | The air's scale is the world's | Tech | Agent | Decided | Its dust is as much of that haze as the day's warmth… |
| [D336](decisions/D336.md) | Under the trees the low sky is hidden by stems, and reflections follow the sky open round the eye | Tech | Agent | Decided | The sky-occlusion bake counts a trunk under every crown… |
| [D347](decisions/D347.md) | What people do, made, not taken | Tech | Agent | Decided | The clips a town's people play at their work and talk… |
| [D356](decisions/D356.md) | The environment's style written down and checked | Tech | Agent | Decided | The art guide's Places section (docs/art-style.md)… |
| [D359](decisions/D359.md) | Canopy shade dappled, pines' stubs snapped, a thicker middle layer under the pines | Tech | Agent | Decided | A crown's solid shadow stand-in casts a crown's shade… |
| [D360](decisions/D360.md) | A stride carried by aim, its arms layered | Tech | Agent | Decided | A made stride (`art/animation/made.json`) is fixed over… |
| [D368](decisions/D368.md) | Leaves checked to read as leaves, card by card and in the picture | Tech | Agent | Decided | Serves D291 and D292, and D356's rules… |
| [D370](decisions/D370.md) | A neutral grade, the colour from the light | Design | Developers | Decided | The colour grade is neutral everywhere and at every… |
| [D383](decisions/D383.md) | How ore shows in a rock | Design | Developers | Decided | A rock with ore in it is the plain rock… |
| [D392](decisions/D392.md) | Skylight is daylight, and the ground bounces the sun that reaches it | Tech | Agent | Decided | The painted sky the shade is lit by… |
| [D423](decisions/D423.md) | Fire is lightly stylised, through the neutral camera | Design | Developers | Decided | Fire is drawn in the art style's lightly stylised… |
| [D427](decisions/D427.md) | A hero prop's triangle budget | Tech | Agent | Decided | A prop the player sits beside or works at for minutes… |
| [D429](decisions/D429.md) | The looks check knows water by what draws it | Tech | Agent | Decided | The looks check's debug-tint rule… |
| [D432](decisions/D432.md) | The dusk sky: an amber afterglow toward the set sun, the earth's shadow and the Belt of Venus opposite | Tech | Agent | Decided | After sunset the sky low toward the set sun glows amber… |
| [D436](decisions/D436.md) | Creature body plans, and what is made another way | Design | — | Open | Recommended: every creature with a skeleton is one of… |
| [D437](decisions/D437.md) | What a generated creature promises, and who gets hand work | Design | Developers | Decided | There is no hand-finished tier: every creature… |
| [D438](decisions/D438.md) | Monsters move by real biomechanics, or by rules of their own | Design | — | Open | Recommended: monsters move as the wildlife does… |
| [D439](decisions/D439.md) | A body plan is a data file | Tech | Agent | Leaning | A creature's body plan is a file… |
| [D440](decisions/D440.md) | A creature's skeleton is fitted to its mesh by its plan | Tech | Agent | Leaning | The skeleton is fitted to the mesh, not typed in… |
| [D441](decisions/D441.md) | Creature weights from the plan's template body, or zoned heat, whichever measures better | Tech | Agent | Leaning | A creature's weights are made two ways and the creature… |
| [D442](decisions/D442.md) | The creature check gates a creature by measured deformation and gait | Tech | Agent | Leaning | `tools/creatures/creature_check.py <kind>` gates a… |
| [D446](decisions/D446.md) | The dusk lake film faces the set sun's side across the water | Design | Developers | Decided | The lake's dusk film looks across the water toward the… |
| [D450](decisions/D450.md) | Combat clips made by Kimodo, keyed to the guard pose | Tech | Agent | Decided | A move's clip is made with Kimodo's latest model… |
| [D454](decisions/D454.md) | The fight's roll is the library's with the arms held out of the tuck | Tech | Agent | Decided | The sword-and-shield set's roll plays `Fight_Roll`… |
| [D458](decisions/D458.md) | A drawn body walks with the guard's arms laid over the library's walk | Tech | Agent | Decided | A body with the sword and shield drawn walks forward and… |
| [D463](decisions/D463.md) | Left-handed bodies | Design | — | Open | Whether a body's main hand may be the left (holding.md). |

## Engine and code

Rust and Bevy, terrain, shared state, networking and the game's model.

| # | Topic | Owner | By | Status | Gist |
|---|---|---|---|---|---|
| [D9](decisions/D9.md) | Terrain representation | Tech | Developers | Decided | Smooth voxels (a density field) on level-of-detail… |
| [D11](decisions/D11.md) | Engine selection criteria | Design | Developers | Decided | Everything in Rust. |
| [D12](decisions/D12.md) | Engine | Tech | Agent | Decided | Bevy (0.20, D31) as the framework, with our own voxel… |
| [D21](decisions/D21.md) | How terrain edits are stored and synced | Tech | Agent | Decided | As operations (shape, position, size) applied in the… |
| [D25](decisions/D25.md) | Performance target | Tech | Developers | Decided | 75 fps, comfortably and always: a frame within 13.3 ms… |
| [D26](decisions/D26.md) | Where the simulation runs | Tech | Agent | Decided | The threaded world simulation, `terra_sim`… |
| [D28](decisions/D28.md) | How the code is split | Tech | Agent | Decided | Anything that must be the same for every player lives in… |
| [D29](decisions/D29.md) | How state is shared | Tech | Agent | Decided | World changes are operations applied in the server's… |
| [D30](decisions/D30.md) | Maths in shared state | Tech | Agent | Decided | The `libm` crate for sine and similar functions… |
| [D31](decisions/D31.md) | Bevy version policy | Tech | Agent | Decided | Pinned, now to 0.20. |
| [D32](decisions/D32.md) | Terrain meshing library | Tech | Agent | Decided | The `transvoxel` crate, in `terra_voxel`… |
| [D33](decisions/D33.md) | Physics | Tech | Agent | Leaning | Avian for rigid bodies: loose objects… |
| [D34](decisions/D34.md) | Networking library | Tech | Agent | Leaning | Our own messages over `renet`, with a Steam transport… |
| [D35](decisions/D35.md) | UI | Tech | Agent | Leaning | Bevy's own UI for the game; `bevy_egui` for developer… |
| [D36](decisions/D36.md) | Coordinates | Tech | Agent | Leaning | One coordinate frame per world, with positions relative… |
| [D40](decisions/D40.md) | Materials in voxels | Tech | Agent | Decided | Voxels carry a material as well as a shape… |
| [D41](decisions/D41.md) | Generation toolkit | Tech | Agent | Decided | One shared crate, `terra_gen`, for seeds… |
| [D42](decisions/D42.md) | Engine build order | Tech | Agent | Leaning | A small foundation (shared contracts, then the… |
| [D45](decisions/D45.md) | Contracts for harvest and water | Tech | Agent | Decided | Every harvest is an operation that returns a yield. |
| [D47](decisions/D47.md) | Numerous world objects | Tech | Agent | Leaning | Objects such as trees and rocks are computed from the… |
| [D53](decisions/D53.md) | Collision | Tech | Agent | Decided | Every body is moved by one kinematic character… |
| [D58](decisions/D58.md) | How the game is modelled in code | Tech | Developers | Decided | In ECS. |
| [D135](decisions/D135.md) | Loading screen | Tech | Agent | Decided | Every build starts on a loading screen that covers the… |
| [D136](decisions/D136.md) | Kinds of thing as data | Tech | Agent | Decided | Kinds of item, recipe, station and creature are entries… |
| [D137](decisions/D137.md) | How a player's actions reach the game | Tech | Agent | Decided | Each player has an `Intent` component… |
| [D138](decisions/D138.md) | The game's tick | Tech | Agent | Decided | Gameplay runs at a fixed rate, 64 ticks a second… |
| [D145](decisions/D145.md) | Simulation level of detail | Tech | Agent | Decided | Creatures are simulated by how near the nearest player… |
| [D184](decisions/D184.md) | How far we change Bevy | Tech | Agent | Decided | Extend first: our own systems, components… |
| [D196](decisions/D196.md) | When Bevy falls short | Tech | Developers | Decided | Find the engine or public tool that does it well and… |
| [D198](decisions/D198.md) | The game in layers | Tech | Agent | Decided | `terra_game` is three plugin layers… |
| [D239](decisions/D239.md) | Quality presets, and what the integrated GPU must hold | Tech | Agent | Decided | Three presets, low, medium and high… |
| [D288](decisions/D288.md) | Foliage shaded only where it is in front | Tech | Agent | Decided | Leaves, grass, litter and the plants that sway… |
| [D307](decisions/D307.md) | The low preset drawn as integrated GPUs need | Tech | Agent | Decided | What the low preset draws is cut where an integrated GPU… |
| [D322](decisions/D322.md) | Mountains, lakes and water | Tech | Agent | Decided | The world's ground gains a mountain term set by a… |
| [D323](decisions/D323.md) | The ground bends the climate | Tech | Agent | Decided | `terra_climate::Place` carries how the ground faces the… |
| [D325](decisions/D325.md) | Smoke, sparks and blown snow | Tech | Agent | Decided | Drawn as instanced soft cards facing the camera… |
| [D331](decisions/D331.md) | The land's rules in one engine-free crate, and a place's latitude its own | Tech | Agent | Decided | `terra_land`, free of Bevy (D28): the world's ground as… |
| [D332](decisions/D332.md) | A frame-rate limit, and one frame budget | Tech | Agent | Decided | The options menu has a frame-rate limit beside vsync… |
| [D335](decisions/D335.md) | Woods and snow by the land's rules, carried on the ground | Tech | Agent | Decided | On a world with mountains the land round the start is… |
| [D337](decisions/D337.md) | The tundra found on the land, and the snow the ground catches | Tech | Agent | Decided | The tundra is found by a query on the land's rules… |
| [D338](decisions/D338.md) | The mountain lake's open air: the eye opened, the air's sky in the water, trees off the shore | Tech | Agent | Decided | On a world whose views reach kilometres (D334)… |
| [D340](decisions/D340.md) | mimalloc as the global allocator | Tech | Agent | Decided | The sandbox, which is the game's program… |
| [D341](decisions/D341.md) | The heath, patterned ground and the climate's wind | Tech | Agent | Decided | Open ground past the trees is a heath of five… |
| [D342](decisions/D342.md) | Trees at their limit stand in islands | Tech | Agent | Decided | A tree's growing season is warmed or chilled for it by… |
| [D345](decisions/D345.md) | The heath's dwarf shrubs, and blown snow | Tech | Agent | Decided | Willow scrub, knee- to thigh-high on the birch's year… |
| [D354](decisions/D354.md) | The town's place found on the land, its fields cleared of the forest | Tech | Agent | Decided | The market town is found by a query on the land's rules… |
| [D361](decisions/D361.md) | Bevy's glTF loader behind a gate, its IO pool Bevy's own | Tech | Agent | Decided | Under D184, Bevy's `GltfPlugin` is replaced by… |
| [D363](decisions/D363.md) | The time of day's look by the place's own sun | Tech | Agent | Decided | What goes with the hour, the morning's mist and the… |
| [D364](decisions/D364.md) | Frozen ground rolls: hummocky moraine and kettles | Design | Agent | Decided | Where the ground is frozen at depth… |
| [D366](decisions/D366.md) | The woods' conifers are the lush forest's, asked for by the world and given by the layer above | Tech | Agent | Decided | `terra_world::woods` places the conifers and asks for… |
| [D371](decisions/D371.md) | A lake's morning mist, and the haze lit by the open sky | Tech | Agent | Decided | Lakes of 5 ha and more steam on a calm morning… |
| [D372](decisions/D372.md) | A mountain meadow's straw | Tech | Agent | Decided | On a world with mountains a meadow's grass is less lush… |
| [D374](decisions/D374.md) | What changes with the world reaches plant shaders in one shared buffer, not their materials | Tech | Agent | Decided | The world's wind (D255) and where the climate's maps lie… |
| [D378](decisions/D378.md) | Wildlife by niche, found as players go | Tech | Agent | Decided | A kind of creature that lives about the land says its… |
| [D379](decisions/D379.md) | Ambient life round the eye | Tech | Agent | Decided | What lives in the air and on the water at a few metres… |
| [D384](decisions/D384.md) | Ambient encounters staged to be seen | Tech | Agent | Decided | A meeting ahead of a player on the move… |
| [D394](decisions/D394.md) | Far mountains drawn from their own cells, not the planet's centre | Tech | Agent | Decided | The ground shader's far ribs are laid cell by cell… |
| [D395](decisions/D395.md) | Far walls' gullies run long, their snow lies in the mesh's own gullies, and pinned places keep their ground | Tech | Agent | Decided | A range's gullies are laid with each cell of their… |
| [D401](decisions/D401.md) | A range breaks into ranks of separate summits, its foothills in front | Tech | Agent | Decided | A range is no longer one wall round its edge. |
| [D403](decisions/D403.md) | A mountain's face is drawn in tiers, weathering and skirts, its gullies shaped by face | Tech | Agent | Decided | A far wall's face gets variety at several sizes… |
| [D404](decisions/D404.md) | A face's gullies come in several sizes, its tiers stand in the light, and talus aprons lie under its walls | Tech | Agent | Decided | A far wall's face varies in its gullies' sizes and its… |
| [D407](decisions/D407.md) | How the crates are cut | Tech | Developers | Decided | `terra_build` (71k lines) and `terra_game`… |
| [D415](decisions/D415.md) | One Fire module, built by the rules shipped games follow | Tech | Agent | Leaning | Every fire in the game, from a candle to a burning… |
| [D416](decisions/D416.md) | The ground shades and mirrors what the camera does not see from a horizon map of the land, not from what is drawn | Tech | Agent | Decided | What lies off screen already casts… |
| [D418](decisions/D418.md) | The weather at the eye drives the sky, the light, the air and what falls | Tech | Agent | Decided | One `Weather` resource holds the climate's cloud… |
| [D420](decisions/D420.md) | GPU memory in pages, made once, uploaded over frames | Tech | Agent | Decided | What the GPU holds is made at its size once and in… |
| [D424](decisions/D424.md) | A creature's clip plays within 0.8-1.2 of its own rate | Tech | Agent | Decided | Every creature on foot plays its gait's clip at its… |
| [D426](decisions/D426.md) | A mountain's walls stand in stepped faces by range kind, in the height | Tech | Agent | Decided | Plan step 2 of the mountain look note, amends D403… |
| [D428](decisions/D428.md) | An ultra preset for films, spent on distance | Tech | Agent | Decided | A fourth quality preset, ultra (`terra_world::quality`)… |
| [D430](decisions/D430.md) | A film's breath is marched as a volume, play's stays cards | Tech | Agent | Decided | The cinematic tier of D428 draws a dragon's breath as… |
| [D433](decisions/D433.md) | The fire sheet measures the flame on the renderer's ID matte | Tech | Agent | Decided | The fire sheet (D415, `tools/fire/sheet.ts`) measures a… |
| [D434](decisions/D434.md) | A far wall keeps a third of its own light before it gets more detail | Tech | Agent | Decided | Work on far and middle-distance mountains… |
| [D435](decisions/D435.md) | Vertex buffers packed in ordinary memory before the upload | Tech | Agent | Decided | A mesh's vertex buffer is packed in ordinary memory… |
| [D443](decisions/D443.md) | A creature's paces come from its size | Tech | Agent | Leaning | A kind with no measured speeds takes its walk… |
| [D457](decisions/D457.md) | The character controller is held by a scorecard of criteria and a ratchet, measured on a plain-Rust course | Tech | Agent | Decided | What the controller must get right is data… |
| [D466](decisions/D466.md) | Fire is one self-contained module, and every kind's flame body is one marched flame field | Tech | Agent | Leaning | The technical design of fire-module.md, extending D415. |
| [D468](decisions/D468.md) | A world instance is the unit loaded in and out, one to a Bevy World | Tech | Agent | Leaning | A world instance is one running copy of a world… |
| [D469](decisions/D469.md) | World content is scoped by default, and a module never names its instance | Tech | Agent | Leaning | On the client, what belongs to the current world is… |
| [D470](decisions/D470.md) | Crossing to another world is a hand-off under a cover, with a return stack | Tech | Agent | Leaning | A door or portal is an entity with `Door { to, arrival`… |
| [D471](decisions/D471.md) | Asset sets by theme, a record per instance, replication by instance | Tech | Agent | Leaning | How an instance's files, saves and messages are bounded… |

## Tools and workflow

Checks, developer tools, languages and the repository.

| # | Topic | Owner | By | Status | Gist |
|---|---|---|---|---|---|
| [D23](decisions/D23.md) | Development workflow | Tech | Agent | Decided | One Cargo workspace. |
| [D37](decisions/D37.md) | Repository tooling | Tech | Developers | Decided | One Cargo workspace at the root with a pinned compiler. |
| [D46](decisions/D46.md) | A sandbox for experiments | Tech | Developers | Decided | `terra_sandbox` is the test environment… |
| [D69](decisions/D69.md) | QA for agents | Tech | Agent | Decided | The game's QA is reachable by our coding agents… |
| [D71](decisions/D71.md) | Developer tools | Tech | Developers | Decided | Tools that help refine each system… |
| [D77](decisions/D77.md) | Logs and frame-time readout | Tech | Developers | Decided | Every build has a frame-time readout on screen at all… |
| [D81](decisions/D81.md) | Debug menu | Tech | Developers | Decided | An in-game debug menu, switched on by a flag at launch… |
| [D82](decisions/D82.md) | Token stats | Tech | Agent | Decided | Each developer's Claude token use on this repository is… |
| [D121](decisions/D121.md) | Languages | Tech | Developers | Decided | Code is Rust or TypeScript. |
| [D133](decisions/D133.md) | Repository layout | Tech | Developers | Decided | Each kind of thing has one home. |
| [D183](decisions/D183.md) | Builds in worktrees | Tech | Agent | Decided | A new worktree's `target/` is seeded before it first… |
| [D185](decisions/D185.md) | How performance is measured | Tech | Agent | Decided | `cargo xtask bench` runs named scenes… |
| [D249](decisions/D249.md) | No options for one-offs | Tech | Developers | Decided | A command-line option, flag, experiment or tool is added… |
| [D254](decisions/D254.md) | Tone mapping for a forest's range | Tech | Agent | Decided | The camera tone maps with Khronos PBR Neutral… |
| [D255](decisions/D255.md) | Gusts | Tech | Agent | Decided | One wind for the grass, the bushes and the trees… |
| [D256](decisions/D256.md) | Dusk's shade lit by the sky above | Tech | Agent | Decided | The sky light (D190) takes its hue half from the drawn… |
| [D259](decisions/D259.md) | Clearing away worktrees | Tech | Agent | Decided | `cargo xtask worktrees` lists every worktree and its… |
| [D308](decisions/D308.md) | QA films | Tech | Agent | Decided | `cargo xtask qa <scenario> --set <kit>\|mixes\|all` films… |
| [D319](decisions/D319.md) | Benchmarks have the GPU to themselves | Tech | Developers | Decided | Only a benchmark needs the GPU alone. |
| [D326](decisions/D326.md) | Checks for places | Tech | Agent | Decided | Every place a demo makes is checked by… |
| [D357](decisions/D357.md) | Trailer films | Tech | Agent | Decided | The trailer is one run through each place… |
| [D375](decisions/D375.md) | The agents' routine | Tech | Developers | Decided | Every agent and lead session follows the routine in… |
| [D376](decisions/D376.md) | Recording decisions without conflicts | Tech | Agent | Decided | The log of decisions changed is its own file… |
| [D381](decisions/D381.md) | A settlement's time-lapse, built by the building system's own steps | Tech | Agent | Decided | `--timelapse <file>[@frame]` with the town experiment… |
| [D385](decisions/D385.md) | Building's debug views | Tech | Agent | Decided | One key, F4 (Shift+F4 back), and the remote's… |
| [D386](decisions/D386.md) | The material bench | Tech | Agent | Decided | `cargo xtask material <material>…\|--all`… |
| [D387](decisions/D387.md) | A Makefile as the front door | Tech | Developers | Decided | A `Makefile` at the root gets a new machine ready… |
| [D388](decisions/D388.md) | Quiet gates | Tech | Agent | Decided | `cargo xtask check` and `--quick` print a line a stage… |
| [D389](decisions/D389.md) | A file per decision | Tech | Agent | Decided | Each decision is its own file, `docs/decisions/Dnnn.md`… |
| [D390](decisions/D390.md) | Where an agent's context goes, and the nudge to wrap up | Tech | Agent | Decided | `node tools/token-stats/context.mjs` reads the… |
| [D391](decisions/D391.md) | Cheaper models for mechanical subagent work | Tech | Agent | Decided | Two subagents in `.claude/agents/`: `finder`… |
| [D393](decisions/D393.md) | A demo's place pinned where its query found it | Tech | Agent | Decided | A place the land's rules make is found by its query once… |
| [D396](decisions/D396.md) | Code health is gated, not reviewed | Tech | Agent | Decided | The classes of code-health fault the reviews kept… |
| [D397](decisions/D397.md) | Subagents keep their prompt cache an hour | Tech | Developers | Decided | `.claude/settings.json` sets `subagentPromptCacheTtl` to… |
| [D398](decisions/D398.md) | The gates scale with the change | Tech | Developers | Decided | Since 2026-10-19 (D37) the hooks only format and lint… |
| [D399](decisions/D399.md) | A crash leaves a capsule; state is snapped and diffed | Tech | Agent | Decided | A panic in the sandbox, and so in the play experiment… |
| [D400](decisions/D400.md) | Agents' tasks are bounded | Tech | Developers | Decided | Every agent's brief says when it stops: a score bar… |
| [D406](decisions/D406.md) | The slow sweeps run on every core, optimised, and an asset is read only by the crates that read it | Tech | Agent | Decided | A test that sweeps many independent cases runs them on… |
| [D408](decisions/D408.md) | Release builds our crates incrementally; bench builds them whole | Tech | Agent | Decided | Our own crates build incrementally in the release… |
| [D409](decisions/D409.md) | Rendered indexes merge row by row | Tech | Agent | Decided | An index of files that live on their own is rendered… |
| [D410](decisions/D410.md) | Tasks register from their own modules | Tech | Agent | Decided | Each xtask module that owns tasks lists them in its own… |
| [D411](decisions/D411.md) | A full run on main, on one box, lowers the baselines and bisects what breaks | Tech | Agent | Decided | `cargo xtask nightly` (`xtask/src/nightly.rs`) runs what… |
| [D412](decisions/D412.md) | A design call is settled by the developers | Tech | Agent | Decided | The docs check fails a decision with Owner Design and… |
| [D414](decisions/D414.md) | A ratchet's baseline is raised by a change with its why, and lowered only by the full run on main | Tech | Agent | Decided | A ratchet's baseline (the artifacts and fuzz counts… |
| [D417](decisions/D417.md) | Offscreen runs open no window | Tech | Agent | Decided | An unattended run drawing into a picture… |
| [D421](decisions/D421.md) | Demo and film places leave out the combat lair | Design | Developers | Decided | Play keeps its lair of combat creatures… |
| [D422](decisions/D422.md) | The review desk, one per developer | Tech | Agent | Decided | Each developer runs their own review desk… |
| [D431](decisions/D431.md) | A push warns when it silently undoes another's recent commit | Tech | Agent | Decided | Work lands by `git pull --rebase origin main` onto one's… |
| [D445](decisions/D445.md) | Renders run on a GPU box that takes jobs over SSH | Tech | Agent | Leaning | An agent's heavy render (a trailer cut or its stills, a… |
| [D448](decisions/D448.md) | Every run writes its log to a file; a played session's log says what shared the GPU | Tech | Agent | Decided | Every run of the game writes each line its log keeps to… |
| [D467](decisions/D467.md) | A demo is an experiment that stands for others, its keys moving the world's own clock | Tech | Agent | Leaning | A playable demo for the developers is a sandbox… |
| [D472](decisions/D472.md) | Visual A/B judging scores one build per judge, against fixed anchors | Tech | Agent | Decided | A blind visual comparison by subagent judges… |

## Superseded and rejected

Kept so their numbers still resolve. They no longer bind anything.

| # | Topic | Owner | By | Status | Gist |
|---|---|---|---|---|---|
| [D5](decisions/D5.md) | Genre | Design | — | Superseded by D187 | Was "sandbox plus simulation, with more depth than… |
| [D6](decisions/D6.md) | Terraforming model | Design | — | Superseded by D89 | A planet divided into cells of local and global values… |
| [D16](decisions/D16.md) | Order of hostile life | Design | — | Superseded by D103 | Plants, then insect swarms, then animals. |
| [D19](decisions/D19.md) | Three.js as primary stack | Tech | — | Rejected | Three.js in the browser as the primary stack. |
| [D22](decisions/D22.md) | Languages | Tech | — | Superseded by D121 | Rust for everything, with no scripting language. |
| [D24](decisions/D24.md) | Water in the first version | Design | — | Superseded by D75 | One global sea level that rises as ice melts… |
| [D27](decisions/D27.md) | Build in Rust instead of Godot | Design | — | Superseded by D11, D12 | Rebuild the game in Rust rather than carry on in Godot. |
| [D39](decisions/D39.md) | What the engine must do regardless of gameplay | Design | — | Superseded by D8, D9, D40, D187 | Was: voxel worlds, resources harvested organically… |
| [D44](decisions/D44.md) | Melting into pools of water | Design | — | Superseded by D75, D89 | Ice melting into local pools of water. |
| [D50](decisions/D50.md) | Style of play | Design | — | Superseded by D89, D97, D180 | Broadly like No Man's Sky or Subnautica… |
| [D55](decisions/D55.md) | Carrying and cost | Design | — | Superseded by D139, D141, D106 | Was: the player carries raw items, and those let them… |
| [D56](decisions/D56.md) | How construction is put together | Design | — | Superseded by D152, D154, D181 | Buildables as list entries with named placement rules… |
| [D57](decisions/D57.md) | The building grid | Design | — | Superseded by D152, D154, D181 | One half-metre grid for everything… |
| [D60](decisions/D60.md) | Terraforming | Design | — | Superseded by D89 | Probably no terraforming, with the notes kept. |
| [D65](decisions/D65.md) | Inventory and backpack | Design | — | Superseded by D106 | Was: the inventory is the backpack, small at the start… |
| [D79](decisions/D79.md) | Terraform rating | Design | — | Superseded by D89 | A terraformable planet's rating of how far along it is. |
| [D209](decisions/D209.md) | How roofs are laid and drawn | Tech | — | Superseded by D241 | Was: a kit laid pitched roofs from roof sections… |
| [D283](decisions/D283.md) | Mesh slabs start at 64 MiB and double | Tech | — | Superseded by D420 | Was: Bevy keeps meshes in shared slabs of GPU memory… |
| [D369](decisions/D369.md) | A grey card grey at noon in the open | Tech | — | Superseded by D370 | Was: the day grade warmed only under a closed canopy… |
| [D413](decisions/D413.md) | Landing takes turns: a push's check runs again only what a rebase changed, and at most two build and test at once | Tech | — | Superseded by D37 | Was: a push's check that passed stamped its tree… |

<!-- end of rendered -->
