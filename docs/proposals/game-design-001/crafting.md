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

## Learning what to make

Proposed, replacing research as a tree of technology (D68): you learn a thing by finding what it is made of, as in Valheim; from people, recruits and cultures (D86); and by skill (D88), as a better smith knows more. Nothing is listed until you have learned it (D92).

## Wear and repair

Agreed by the developers for now, 2026-10-11 (D99), as recommended here, after Valheim: **tools, weapons and armour wear with use, a broken one stops working but is never lost, and repair is cheap at a station in a claim**, quicker with a recruit who knows the craft. It is a gentle reason to come home (D91), it makes smiths worth rescuing, and it stays clear of the chore of grinding repairs (D3). The other way, no wear at all, is simpler and loses those.

## Questions

1. Answered: **wear and repair** as recommended, for now (D99).
2. **Quality**: does skill make better things (a master smith's sword is better than a novice's)?
3. **Stations moving**: can a station be packed up and moved, or is it built in place?
4. **Recipes**: can a player teach another, or must each learn for themselves?
