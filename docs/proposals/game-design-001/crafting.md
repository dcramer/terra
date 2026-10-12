# Resources, tools and crafting

Part of [game design 001](README.md). A proposal for the developers to react to; nothing here is decided unless it cites a decision.

## What the developers have said

- **No science-fiction toolset, and no single tool that harvests everything** (D96). Each job has its own tool.
- You carry raw items, and they let you build what you are able to build (D55).
- Resources are a reason to explore and settle (D91).
- A backpack where nothing stacks (D84).
- **Items wear and need repair**, as recommended below, for now (D99).

## Based on

**Valheim** for materials that belong to regions and gate what you can make, and for tools whose material decides what they can work. **Enshrouded** for crafting at stations, and for rescued people who run them.

## Resources

- **Belong to places** (D91): a wood to one forest, a stone to one terrain, the best ores deep or far or in higher-rank lands. Each world has its own (D59).
- **Come in tiers by rank**: an iron-rank land's materials make iron-rank things ([essences.md](essences.md)).
- **Some are touched by magic**, found where it pools, guarded by what forms there ([creatures.md](creatures.md)).
- **Are taken from the world itself** (D43): mined, chopped, dug, cut, hunted, gathered.

## Tools

One tool to a job, carried on the action bar (D66):

| Tool | Works |
|---|---|
| Axe | Trees, wood |
| Pickaxe | Rock, ore |
| Shovel | Earth, sand, digging and filling ground |
| Knife or sickle | Plants, fibres, hides |
| Hammer | Building and repair ([construction.md](../../construction.md)) |
| Rod, net, bow | Fish, game |

**A tool's material decides what it can work**: a stone pickaxe will not break iron ore. Better tools are made from what the last tool let you reach, which is the Valheim ladder.

## Crafting

- **By hand, anywhere**: the simplest things, a stone tool, a torch, a rough shelter.
- **At stations, in a claim** (D94): a workbench, a forge, a kiln, a loom, a tanning rack, an alchemist's table. Each makes a family of things.
- **Processing chains**: ore to ingots at a forge, hide to leather on a rack, sand to glass in a kiln.
- **Recruits run stations** (D90): a rescued smith makes the forge able to make more, as Enshrouded's survivors do.

## The station screen

Settled with the developers on 2026-10-11 (D109), after Enshrouded.

**Making takes time.** Click a recipe, without holding, and one joins the station's queue; click again to queue another. What it takes is paid when queued and given back if the job is cancelled.

**Two kinds of station.**

| Kind | Examples | How it works |
|---|---|---|
| Worked | Workbench, forge | Works through its queue only while you stay within reach. Leave and it waits, losing nothing. What it makes goes into your backpack. |
| Working on its own | Smelter, campfire | Works through its queue whether you are there or not, like a furnace. What it makes waits in it until you come back. Whether it needs fuel is open. |

**The screen is the build menu's**, with recipes for tiles:

| Part | What it shows |
|---|---|
| Tiles | One per recipe you have learned (D68, D92), in a grid: the name, and what it takes, each counted as have/need. Greyed when you cannot make it; absent until learned. Pictures later, as in the build menu. |
| Tabs | Only where a station makes several families, such as the workbench's tools and clothing. |
| The pointed tile | A strip saying what the thing does: a tool's work, a weapon's damage and reach, clothing's protection, a pack's size; and where anything missing comes from. |
| The queue | What is waiting, the first with its progress. Click one to cancel it. |
| The station | Its name, and who works it if anyone does ("Bren works this forge"), since a recruit is why some recipes are open (D90). |
| The backpack | Open beside it. |

F at the station opens it, F or Esc leaves; click a tile, or use the arrows and Enter. The world does not pause. The build menu and the station screens share one look and one set of controls in the code.

## Expanding a station

Asked for by the developers on 2026-10-12 (D151, Open): stations should be expandable, perhaps by slots, either Valheim's way, where an upgrade counts if it stands within a radius of the station, or Soulmask's, as the developers describe it, where a station has placeholder slots for its expansions. What they want different from both: **no fixed item for each expansion**; there should probably be variations, though they are not sure. Then (D151, Leaning): "crafting stations will vary in capabilities when they expand/upgrade, and we want things to visually look different but also organic … it might unlock new craftables, improve capacity or other attributes".

The two ways, as they would play here:

| | Within a radius (Valheim) | Slots on the station (Soulmask) |
|---|---|---|
| Placing | Anywhere near the station, so a workshop is laid out as you like | On marked places on the station, shown as ghosts, so there is no doubt it counts |
| Fits our building | Well: free placing, the grid, items on items (D144) | Needs sockets on every station model |
| Reads as real | A smithy is a forge with its tools around it | A bellows really is fixed to a forge |
| Which station it belongs to | Can be unclear with two stations close together | Always clear |

**Proposed: an expansion is a role, not an item.** A station names the roles it can use, such as the forge's bellows, quench trough, grindstone and tool rack, and anything built for a role fills it. Each role has variations: different looks, materials and makers, many found in other realms (D87), as Terraria's furniture is (D85), so two smithies look different. Stronger variations may do more, such as an iron anvil letting the forge make what a stone one cannot.

**Proposed: where by the role.** What is part of the station attaches to it, on a slot (a bellows on a forge, a vice on a bench); what is part of the workshop stands within a few metres (a tool rack, a grindstone). Each is then placed in the way that reads as real, and a slot is shown only when an expansion that fits it is in hand.

**What an expansion does, by its variation** (the developers' direction): each variation says, as data (D136), what it adds to its station, so two forges expanded differently can do different things:

| Adds | Such as |
|---|---|
| New things to make | An iron anvil opens iron tools a stone one cannot (D68, D92) |
| Capacity | A longer queue, a bigger batch, more fuel or stock held |
| Speed | A bellows makes the forge work faster |
| Quality and reach | Better things made, or higher tiers mended (D99) |
| Other attributes | Less fuel used, a recruit working it better (D90) |

Shown on the station's screen: what fills each role, what it adds, and what an empty role would add.

**Looking organic** (the developers' direction), proposed:

- **No visible slots or sockets.** A role's place is shown only while placing an expansion; once in, it looks set there by hand, with a little turn and offset of its own.
- **The station grows with what is added.** Its own model changes with what it has: soot on the forge's hood once a bellows works it, tools hung on the bench once a rack is by it, wear where it is used most.
- **Variations differ in form, not only colour**: a stone quench trough and a barrel one are different shapes, and those found in other realms look of their realm.
- **A workshop reads as a whole**: the expansions that stand nearby are where a worker would want them, and recruits use them where they stand (D90).

## Learning what to make

Proposed, replacing research as a tree of technology (D68): you learn a thing by finding what it is made of, as in Valheim; from people, recruits and cultures (D86); and by skill (D88), as a better smith knows more. Nothing is listed until you have learned it (D92).

## Wear and repair

Agreed by the developers for now, 2026-10-11 (D99), as recommended here, after Valheim: **tools, weapons and armour wear with use, a broken one stops working but is never lost, and repair is cheap at a station in a claim**, quicker with a recruit who knows the craft. It is a gentle reason to come home (D91), it makes smiths worth rescuing, and it stays clear of the chore of grinding repairs (D3). The other way, no wear at all, is simpler and loses those.

In the tech demo, proposed numbers: felling, breaking or cutting one thing, digging a scoop, landing a blow, loosing an arrow or building a part is a use; a stone tool lasts 60, an iron one 200, the hammer 300, the spear 100, the bow 120; leather wears a use for each blow taken, the cap lasting 60 and the coat 80; a torch burns 8 minutes. A thing is mended, free, at the station that makes it, when you open it in your claim: the workbench mends stone tools, leather and the hammer, the forge iron, a campfire torches. A recruit who knows the craft mending quicker is still to come.

## Questions

1. Answered: **wear and repair** as recommended, for now (D99).
2. **Quality**: does skill make better things (a master smith's sword is better than a novice's)?
3. **Stations moving**: can a station be packed up and moved, or is it built in place?
4. **Recipes**: can a player teach another, or must each learn for themselves?
5. **Expanding a station** (D151): roles filled by any of several variations, attached or nearby by the role, as proposed above? Answered in part: variations differ in what they do as well as how they look, and look organic.
