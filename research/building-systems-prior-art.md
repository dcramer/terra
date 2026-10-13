# Building in survival games

2026-10-12. Question: what do the building systems of Enshrouded, Rust, Ark, Subnautica and The Forest do, from the keys to claims and decay, so that ours leaves out nothing players expect?

Valheim and 7 Days to Die are added where they show something the others do not. Enshrouded and Subnautica 2 have notes of their own ([enshrouded.md](enshrouded.md), [subnautica-2-building.md](subnautica-2-building.md)); this note does not repeat them.

## How this was found

Everything here comes from wikis, patch notes, developer blogs, guides and forum threads, read on 2026-10-12. Nothing was checked in a game. The fan wikis on Fandom would not load, so several facts rest on one guide or forum post, and those are marked. Default keys are given only where a source states them; guides disagree in places, and games change their bindings. "Memory" marks something I believe but found no source for.

## Controls on mouse and keyboard

| Action | Enshrouded | Rust | Ark | Subnautica 1 / 2 | The Forest | Sons of the Forest | Valheim |
|---|---|---|---|---|---|---|---|
| The tool | Construction hammer | Building plan to place, hammer to change | None: a structure is an item, crafted first, placed from the hotbar | Habitat builder | Survival book | Guide book | Hammer |
| Choose a piece | Tab for the quick menu; Alt + wheel changes category | Hold right mouse: a wheel | Its hotbar slot | Right mouse: a tabbed menu | B, then a page | B; hold X to switch between blueprints and building by hand | Right mouse: a tabbed grid; Q and E change tab |
| Place | Left mouse | Left mouse (blue means allowed, red not) | Left mouse | Left mouse; in the first game held while it builds (memory) | E. Walls and floors you shape: left mouse sets each corner, right mouse takes the last one back | Left mouse | Left mouse |
| Rotate | Tap R for 90°; R + wheel | R turns a wall around | E while placing | Wheel in the first game; not found for the second | R | Q and R (one guide) | Wheel |
| Raise, lower, nudge | Q + wheel pushes it forward or back | No; neighbouring foundations may sit half a step apart | Q and Z cycle the snap points; Ascended raises and lowers a foundation as you move the mouse | Not found | Not found | Not found | Not found (mods add it) |
| Snapping off | X toggles it | Not possible | Ascended: a held key; reports disagree which | Not found | Not found | Logs go only where a white dashed outline shows | Hold Shift |
| Variant | Ctrl + wheel changes material | Chosen in the wheel; grade changed later | Ascended: a wall becomes a doorway or window | Not found for the second; see its note | Not found | Right mouse (ramp or stairs, floor direction) | Not found |
| Take apart | Right mouse | Hammer wheel, Demolish, only for about ten minutes after placing | Hold E for a wheel, Demolish | Q (first); Q, then hold left mouse (second) | C on a blueprint not yet finished | Hold C on a piece | Middle mouse |
| Repair | Not found | Hit it with the hammer | Hold E wheel (memory) | None needed | Repair tool | Repair tool | The hammer menu's repair tile, then left mouse |
| Upgrade | Not found | Hammer wheel, in place | Build the better piece over the old (foundations, forum reports) | None | None | None | None |
| Move | Not found | Turn only, in the same window | Not found | F in the second | None | None | None |
| Undo | Y | The demolish window does the job | None | None | None | None | None |

7 Days to Die adds two worth knowing: holding R opens a wheel with rotation modes and a **copy of the shape or rotation of any block you look at**, and the hammer's right mouse upgrades a block one material up or repairs it.

## Features by game

| | Enshrouded | Rust | Ark | Subnautica 1 / 2 | The Forest | Sons of the Forest | Valheim |
|---|---|---|---|---|---|---|---|
| Pieces | Half-metre voxel blocks in many materials, and shapes (walls, floors, stairs, roofs) of 1, 2 and 4 metres | Square and triangle foundations; full, half and low walls, doorways, windows; floors, floor frames, roofs; U, L and spiral stairs, ramps, steps. No pillars. | Foundations, walls, ceilings, pillars, stairs, ramps, roofs, fences, gates for big tames | Fixed modules in the first; rooms and corridors you resize in the second | Whole buildings as blueprints; walls, floors and foundations whose corners you set | Logs placed one at a time: struts, beams, walls, floors of planks, roofs, stairs and ramps; doors and windows cut out of a wall; plus the old blueprints | Wood, core wood, stone, iron pieces; beams, poles, roofs at two pitches |
| Grid and snapping | Voxel grid; snapping toggled | Sockets only | Snap points, cycled | Grid of modules | Free; pieces placed in the world | Where the outline allows | Snap points; Shift for free |
| Slopes | Dig, fill and flatten the ground | Every corner of a foundation must touch the ground | Pillars in Evolved; height adjustment in Ascended | Not checked | Not checked | Struts of cut logs to level | A hoe levels or raises the ground, for stone |
| Stability | None found (one host says otherwise, unverified) | A percentage, read by looking at a piece with the hammer; 0% falls | Hidden support rules; pillars must be snapped; what falls is not refunded | Hull strength against water pressure, not weight | Not found | A piece holding up another cannot be removed; a broken-log icon warns | Colours while building: blue on the ground, green, yellow, red; wood stands about seven high |
| Materials and tiers | Material is chosen per block before placing | Twig, wood, stone, sheet metal, armoured; upgraded in place | Thatch, wood, adobe, stone, metal, greenhouse, Tek; each its own item | One | Logs, sticks, rocks | Logs, sticks, stones | Wood to black marble; no upgrade |
| Where the cost comes from | Blocks crafted at a workbench in hundreds, then from inventory | Inventory | Crafted into items beforehand | Inventory in the first; the second also takes from lockers within a generous range | Carried to the blueprint and put in by hand | The same; logs only in the hands | Inventory, with a workbench in range |
| Build time | Instant | Instant | Instant | A few seconds of the builder's beam | One log at a time, each with an animation | The same; a 2025 option skips the animation | Instant |
| Refund | Full with the hammer; none if broken with a pickaxe | Only in the demolish window | About half (one guide); none for what collapses | Full; the second can refund a whole base from the menu | Little when a blueprint is cancelled (player report) | The log comes back as a log | Full, except resin and fuel |
| Plans others fill | None | Twig frames serve | Ascended's Planning Mode (June 2026): free holograms, seen only by the tribe, that real pieces snap to and replace | None | The core of it: a ghost costs nothing, anyone fills it | Blueprints; the companion finishes them; 2025 added walls drawn as a path | None (mods) |
| Claims | Flame altar: 40 to 160 metres a side, 10 per world; changes outside revert after 30 minutes | Tool cupboard: privilege around the base, a list of who may build | None; tribes own pieces | None | None | None | Workbench to build; a ward (about 32 m) for permission |
| Doors and access | Server roles: edit a base, extend it, open chests, change the world | Key lock or code lock, with a guest code | PIN codes; a tribe rank per structure | None | Not found | A stick bars a door | The ward locks doors and chests |
| Decay | None | Upkeep paid into the cupboard, 10 to 33% of the cost a day; since September 2026 more for groups over four | Timers of 4 to 20 days by tier, reset by any tribe member logging in | None | None found | None found | Rain wears unroofed wood |
| Templates | None found | None (server mods) | Holograms saved as templates and placed again (Lost Colony's template hammer) | None | None | None | None (mods) |

Ark also clears trees and rocks under a structure as it is placed, and they do not grow back. Rust forbids building near monuments, roads and other people's foundations and says why in a line at the corner of the screen. 7 Days to Die's land claim block protects its area and makes blocks four times as hard while the owner is offline.

## The Forest: building as hauling

What makes it different from the rest. From guides and players' reports; the wiki would not load.

- **A blueprint is free; the work is filling it.** Pick a building in the survival book and place a see-through ghost of it, as many as you like, anywhere. It lists what it still needs. Walk up with materials and press E, and each one goes in with its own small animation, the building rising piece by piece. Guides advise laying out ghosts first to see the whole base and its total cost before gathering anything.
- **Logs are carried, not stored.** They never enter the inventory. You carry at most two on your shoulder, or two logs and a body; sticks and rocks go in the bag. So a cabin of a hundred logs is fifty walks unless you plan.
- **The rest is logistics.** A log sled you push (about a dozen logs, sources disagree), log holders to stock near the site, slopes to roll logs down, and later ziplines. Players remember the sled as the most useful thing in the game.
- **Anyone fills anyone's ghost.** In co-op the counter on screen totals every unfinished ghost in the world, and logs left on a sled or in a holder count for nothing until put into a ghost.
- **Shapes you draw.** Walls, floors and foundations can be set out corner by corner, so a wall follows the ground instead of a fixed piece.
- **Changing your mind costs.** A ghost can be cancelled with C before it is finished, but what went in mostly does not come back. Finished buildings are mended with a repair tool, which hammers a damage meter full; a hole-cutting blueprint removes part of a floor or wall.

**Sons of the Forest** kept the book of blueprints and added building by hand, log by log:

- **The log is the piece.** Hold a log and white dashed outlines show where it can go and what it would join. Lay logs on the ground for a floor's frame, stand them up for posts, lean them for stairs. An axe cuts a log into three-quarters, halves, quarters or planks along red dashed lines, or a full log carried to a piece that needs a shorter one is cut to fit by itself.
- **Openings are cut, not chosen.** Take five logs out of the bottom of a wall and add three planks to make a door; three higher up for a window.
- **Support is real, simply.** A log that holds up another cannot be taken out, so a tower comes down from the top.
- **Still two logs at a time.** Sleds carry 8 or 12. The companion, Kelvin, will fetch logs and finish any blueprint you place, gathering what it lacks himself; since 2025 you can draw a defensive wall's path and leave it to him.

Why it works: a building is made of things you went and got, so a big one is a project with a shape you can see growing, and in co-op one person chops while another carries and a third places. Its weaknesses are the same thing: trips, an animation per log (an option to skip it came in 2025), no way to try a whole base before paying for it, and taking things down being slower than putting them up.

## What players praise and complain about

**Enshrouded.** Praised as one of the best in the genre for freedom and looks. Complaints: the third-person camera hides what you are building and jumps between near and far; roofs are hard to line up, ending too high, too low or overlapping; only four directions, so no round or diagonal walls; tedium on big builds, with requests for a free camera and a tool to swap one block for another; the altar's radius feels limiting. The developers answered with a first-person building camera and props rotatable on every axis (January 2025), snapping that stays on when switching items, and favourite materials in the catalogue (April 2026).

**Rust.** Its pieces snap only one way and grades read at a glance (my reading; no review found saying so). Its founding blog (2014) said "Rust isn't a hammering simulator" and asked whether building should cost time as well as resources; it decided not. Complaints: the "too close to another building" refusal, added against double walls, that also blocks honest builds; that you can only fix a mistake for a few minutes; upkeep that taxes big bases and, from September 2026, big groups.

**Ark.** Ascended's adjustable foundation heights, mirrored pieces and walls that become doors were welcomed; Evolved needed pillar tricks for any slope. Long-standing complaints: snapping that will not take the point you want; support rules you cannot see until something falls; and, on servers for everyone, players fencing off land and resources with lone pillars and foundations, because nothing limits a claim.

**Subnautica.** No review of the first game's building was read; from memory it is easy, since a module snaps where it fits and taking it apart gives everything back. The second's first update (July 2026) was mostly about placement: clearer ghosts behind objects, and showing through walls what blocks a build.

**The Forest and Sons of the Forest.** The physical, filled-in blueprint is the memorable part, and a review calls the sequel's log-by-log building controllable rather than fiddly for cabins and bridges. Others call it clunky: blueprints for some things but not walls, two modes to switch between, tedious without a way to test a whole layout, and tearing down slow.

**Valheim.** Not surveyed for opinion. Its stability colours and full refunds are what guides lead with, and mods adding saved layouts, free cameras and finer placement are common, which suggests what players miss.

## What it suggests for Terra

Suggestions for the developers, not decisions. Where one touches a decision, it says which.

**A baseline that misses nothing.** Taken together, these are what a player arriving from these games will reach for. Ours has some already (see [construction.md](../docs/construction.md#the-baseline)):

1. Choose a piece from a menu (D67), and its variant from a wheel (as now).
2. A ghost that shows allowed or refused, and says why, with what is in the way outlined even through walls (Rust, Subnautica 2). Ours does most of this.
3. Rotate, snap with a switch to go free, nudge and raise or lower before placing. Ours has the first two; nudging is in Enshrouded, Ark and the grid's half-metre step (D57) would make it simple.
4. Take apart for a full refund, and undo for a short while (Enshrouded's Y, Rust's window). Full refund matches Subnautica and Valheim.
5. Move what is placed (Subnautica 2's F); pick another of what you look at (ours, and 7 Days to Die's copy of shape).
6. Repair, and change a piece's material in place (Rust, 7 Days to Die). With material a property of a block rather than a different buildable ([construction.md](../docs/construction.md#easy-to-extend)), upgrading is just changing that property and paying the difference.
7. Stability shown only while building, if there is stability at all (an open question in [construction.md](../docs/construction.md#stability)). If there is, Valheim's colours and 7 Days to Die's strain warning show it plainly; Ark's hidden rules are the ones players complain about.
8. A claim that is one placed object with a radius, a list of who may build and who may open what, and something that keeps it alive. Every game with many players has one; Ark's lack of one is the clearest warning for a thousand players. This is D94 and D104 as proposed.
9. Saved layouts. Ark added them in 2026 and the others have them as popular mods. Since a base is already a list of operations, a saved layout is that list, and D108's villages are the same thing.

**The Forest's blueprints fit "realistic but fun" (D122).** The developers want building to cost raw items you carry and to take time ([construction.md](../docs/construction.md#what-the-developers-want)). The Forest is the strongest version of that:

- **Place a ghost for nothing; it is built as materials go in.** Each thing added shows: a wall rising course by course. It doubles as Ark's planning mode, and as the way to see a whole base's cost before gathering. It is the same pay-as-you-go idea as crafting at stations (D109), where what is queued is paid and given back if cancelled; a ghost could keep what went in and give it back if taken down.
- **Big materials are carried.** Logs and stone on the shoulder, two at a time, with sleds and carts for more, makes a large build a project with logistics, and a natural job for a second player. Recruits who work at a base (D90) could fill ghosts as Kelvin does.
- **Where fun beats realism**, Subnautica 2's answer is that storage within reach of a ghost pays for it. That keeps the carrying for the first trip from the forest and removes it from the hundredth wall. Whether ghosts draw from nearby storage, and how far, is a design question.
- **What to avoid from it:** an animation per piece that cannot be sped up; losing what went into a cancelled ghost; and taking down being harder than putting up.

**Drawn shapes over many single pieces.** The complaints about tedium in Enshrouded, Rust and The Forest are about placing the same piece many times. The Forest's corner-by-corner walls, Sons of the Forest's drawn wall path and our shaped rooms (D48) answer it the same way: draw the outline, let the pieces fill it.

**Roofs and the camera are where Enshrouded hurts.** Our generated hull makes roofs (D48), which sidesteps lining them up by hand. A building camera that the body does not block was Enshrouded's fix and is worth having from the start.

**Trees under a building.** Ark clears them for you, permanently. Ours makes the player clear them first (D131, Leaning). Both are known to work; ours is the more physical.

## Earlier: rooms

Before walls, floors and roofs became the parts (D152) and everything built became half-metre cubes (D181), construction.md proposed rooms after Subnautica 2: rooms as plain boxes of any size, grown by placing one across another, with a hull generated round them; interior walls, windows and doors as panels on the edges between tiles; fixed-size buildables such as a round room joined by ports and connectors the game would build; a room's material as a skin; and corners that rounded by what they touched (D56, D57). Saves from then are still read and turned into cubes. The rest is in git history.

## Limits

- Nothing was checked in a game. Several keys come from one guide, and the Ark and Subnautica 2 rows have gaps that only playing would fill.
- Subnautica 2 is in early access and changes monthly; the others patch often too. Numbers (Rust's upkeep, Ark's timers, Enshrouded's altar count) are as the sources gave them in 2026.
- How each game feels to build in, which is what matters most, cannot be read from a wiki. A few hours in Enshrouded and Sons of the Forest would test the suggestions above better than this note.

## Sources

The Forest and Sons of the Forest:
- [Sons of the Forest wiki: Custom Buildings](https://sonsoftheforest.wiki.gg/wiki/Custom_Buildings), [Log](https://sonsoftheforest.wiki.gg/wiki/Log), [Log Sled](https://sonsoftheforest.wiki.gg/wiki/Log_Sled)
- [eip.gg: Sons of the Forest construction](https://eip.gg/sotf/guides/construction/)
- [GamerGuides: Sons of the Forest building](https://www.gamerguides.com/sons-of-the-forest/guide/crafting/building)
- [Patch notes, January 2025: rafts and defensive wall blueprints](https://patched.gg/games/sons-of-the-forest/rafts-defensive-wall-blueprints-fixes-improvements-and-more)
- [Corrosion Hour: Kelvin companion guide](https://www.corrosionhour.com/sons-of-the-forest-kelvin-companion-guide/)
- [Full Cleared: Sons of the Forest first impressions](https://fullcleared.com/features/sons-of-the-forest-first-impressions/)
- [Sportskeeda: can you carry more than two logs](https://www.sportskeeda.com/esports/fact-check-can-carry-two-logs-sons-forest)
- [Steam thread: The Forest's world-wide log counter](https://steamcommunity.com/app/242760/discussions/0/1635292137566388514)
- [Steam thread: log holders and sled](https://steamcommunity.com/app/242760/discussions/0/613936673556009855)
- [commands.gg: the log on your shoulder](https://commands.gg/theforest/blog/unlimited-logs)
- [ludo.guide: The Forest walkthrough](https://www.ludo.guide/guide/the-forest/the-forest-part-4)
- [Corrosion Hour: The Forest repair and remove structures](https://www.corrosionhour.com/the-forest-how-to-repair-remove-structures/)
- [Steam thread: Sons of the Forest building complaints](https://steamcommunity.com/app/1326470/discussions/0/598513524566674662)

Rust:
- [Corrosion Hour: how to build in Rust](https://www.corrosionhour.com/how-to-build-in-rust/)
- [XGamingServer: Rust base building](https://xgamingserver.com/blog/rust-base-building-guide/)
- [Facepunch Devblog 34 (2014)](https://rust.facepunch.com/news/devblog-34)
- [Facepunch: Breach and Clear (September 2026)](https://rust.facepunch.com/news/breach-and-clear)
- [Corrosion Hour: upkeep](https://www.corrosionhour.com/rust-upkeep-building-guide/)
- [uMod: the "too close to another building" error](https://umod.org/community/rust/28229-remove-the-to-close-to-another-building-error)
- [Prima Games: demolishing walls](https://primagames.com/tips/how-to-demolish-walls-in-rust)
- [Facepunch: building skins](https://rust.facepunch.com/skins)

Ark:
- [Ark wiki: Building](https://ark.wiki.gg/wiki/Building), [Foundation](https://ark.wiki.gg/wiki/Foundation), [patch 88.3, Planning Mode](https://ark.wiki.gg/wiki/88.3), [Tribes](https://ark.wiki.gg/wiki/Tribes)
- [GINX: raising and lowering foundations in Ascended](https://www.ginx.tv/en/raise-lower-foundations-guide)
- [Steam thread: Ascended building changes](https://steamcommunity.com/app/2399830/discussions/0/3941272762728198527)
- [Steam thread: snapping off in Ascended](https://steamcommunity.com/app/2399830/discussions/0/3881599433107261737)
- [Dododex: demolish refunds](https://www.dododex.com/tips/58/434179/downvtoe-all-tips-that-say-to-craft-vaults-using-these-then-destroy-it-to-get)
- [Steam threads on pillar spam](https://steamcommunity.com/app/346110/discussions/0/483366528906265954), [and](https://steamcommunity.com/app/346110/discussions/0/2251182852557223393)
- [Wikily: Ascended building templates](https://wikily.gg/es/ark-survival-ascended/building-templates/templates/2333/)

Enshrouded:
- [Enshrouded wiki: Flame Altar](https://enshrouded.wiki.gg/wiki/Alter), [Patch 7](https://enshrouded.wiki.gg/wiki/Patch_7)
- [holy.gg: Enshrouded keybinds](https://www.holy.gg/en/post/enshrouded-keybinds-and-controller-controls-guide)
- [GamingBolt: Pact of the Flame update](https://gamingbolt.com/enshrouded-pact-of-the-flame-update-revamps-locations-and-adds-first-person-building-camera)
- [patched.gg: patch 0.9.1](https://patched.gg/games/enshrouded/changelog-for-patch-14-v0911)
- [Sirus Gaming: block types](https://sirusgaming.com/enshrouded-block-types/)
- [GameServerKings: server roles](https://www.gameserverkings.com/knowledge-base/enshrouded/how-to-add-admins-and-commands.md)
- [Steam threads on building complaints](https://steamcommunity.com/app/1203620/discussions/0/4142816719800027023), [and](https://steamcommunity.com/app/1203620/discussions/0/4630359473422084556)

Subnautica:
- [Subnautica 2 wiki: Habitat Builder](https://wiki.subnautica.com/sn2/Habitat_Builder)
- [KeenGamer: Subnautica 2 base building](https://www.keengamer.com/articles/guides/subnautica-2-base-building-best-early-modules-and-safe-locations/)
- [GamerGuides: Subnautica 2 habitats](https://earth.gamerguides.com/subnautica-2/guide/habitats)
- [Pure Xbox: update 1.1 patch notes](https://www.purexbox.com/news/2026/07/subnautica-2-adaptive-measures-update-1-1-arrives-on-xbox-here-are-the-patch-notes)
- [Subnautica wiki: Seabases](https://subnautica.fandom.com/wiki/Seabases_(Subnautica)), [Reinforcement](https://subnautica.fandom.com/wiki/Reinforcement_(Subnautica))

Valheim and 7 Days to Die:
- [Gamepressure: Valheim building](https://www.gamepressure.com/valheim/building/zde2ae)
- [PC Gamer: repairing in Valheim](https://www.pcgamer.com/how-to-valheim-repair-tools-items-axe)
- [Freakhosting: Valheim wards](https://help.freakhosting.com/games/valheim/ward-settings-and-pvp)
- [7 Days to Die wiki: Structural Integrity](https://7daystodie.wiki.gg/wiki/Structural_Integrity), [Upgrading](https://7daystodie.wiki.gg/wiki/Upgrading)
- [7d2d.net: rotation, upgrade and pickup](https://wiki.7d2d.net/basics/block-rotation-upgrade-pickup.html)
- [Steam thread: 7 Days to Die shapes and copying](https://steamcommunity.com/app/251570/discussions/0/4517758984073774727)
- [Freakhosting: land claim hardening](https://help.freakhosting.com/games/7-days-to-die/land-claim-block-keystone-hardening)
