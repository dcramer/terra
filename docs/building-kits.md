# Building kits: how construction looks, logs first

How each kind of construction looks and is laid, starting with logs. It fills in step 4 of [building-tech.md](building-tech.md#order). How pieces are made and drawn is technical (D177); the look of log walls (D178), posts and mixing kinds (D179) are Decided; what a wall costs need not match its pieces one for one.

![The log kit: a corner, how walls meet, a doorway, and the three states](images/log-kit.svg)

## What a kit has to do

A wall is drawn as a line on the grid (D154), and the game lays its pieces (D146). One layout, worked out by rule from the plan, serves everything:

- **The blueprint** (D139) is those pieces drawn as a ghost, every log separately, so you can count what is needed, as The Forest's blueprint shows each log.
- **Going up**, each piece is either in, drawn as it will stand, or still a ghost; the next one is lit.
- **Built**, the same pieces, merged into one mesh per building (building-tech.md).
- **While drawing**, the ghost under the cursor is the same layout, laid again each time the line grows by a cube.

So a corner looks the same in the blueprint as it does built, just ghosted. Nothing is modelled twice.

## How pieces are made (D177)

**Every piece is a model from the art pipeline; code only arranges them**. Logs drawn by code from numbers break the art direction (D14): their look was invented, not drawn to the style guide. Now a kit's look starts as a concept of a whole building drawn to the guide and approved by the developers, [art/props/log-cabin](../art/props/log-cabin/concept.png); each piece is modelled from a concept of its own, drawn the same way, by `tools/props` (Tripo, then a clean-up in Blender to palette roles). The layout (below) only places them: it repeats a piece, stretches a log along its length to fit its wall, cuts a piece by a plane where it must end, such as a split log or a shingle panel at a hip, and tiles panels over a roof. That is how shipped games build modular kits.

The log kit's pieces, from the approved cabin:

| Piece | Laid as |
|---|---|
| A straight log, a few versions | Every course, corner end, gable log and purlin, stretched along its length to fit; cut along its middle for a split log |
| A panel of shakes | Tiled over each face of a roof, cut at its edges, hips and valleys |
| A ridge cap | Along each ridge and hip |
| A timber board | Round every opening, stretched to its edges |
| A door, a shuttered window | Set into openings of their size |
| A footing of fieldstones | Under walls on the ground |

The stone kit's pieces (built 2026-10-12), rough fieldstone in lime mortar, all drawn after one concept ([art/props/kit-stone-wall-2](../art/props/kit-stone-wall-2/concept.png)) and matched to its grade:

| Piece | Laid as |
|---|---|
| A wall section a metre square, three versions | Each two cubes by two of a straight run, every other band starting a half block in so seams never line up; turned end for end by its seed |
| A half-metre block, two versions | Odd cubes, the ends of runs, and round holes; turned any of four ways |
| A column of cornerstones, half a metre square and a metre high | Up every corner and wall end, a half block for an odd cube |
| A timber lintel | Over every opening with wall above it |

A Tripo model's back and sides come out darker than its front, since it is textured from views lit differently; `tools/props` evens each side of a block to its front's colour, and every piece to the kit's grade, so a piece reads the same turned any way.

**A kit is a file** in `assets/data/kits/` (D136): its pattern (stacked, for logs; masonry, for stone), its sizes, its pieces' models, and its colours as palette roles (D14). A pine cabin and a dark-wood cabin are two files.

## Drawing them

| State | How | Why |
|---|---|---|
| Blueprint | Each piece in a pale, see-through blueprint material, edges brighter so each log reads apart; red where it cannot go. Drawn instanced, many pieces to one draw | Fifty cabins of blueprint are a few thousand pieces, which instancing makes cheap |
| Going up | The pieces in are drawn solid, the rest ghost, the next outlined and pulsing | Says what to bring and where it goes (building.md's prompt) |
| Built | One mesh per building and material, rebuilt in the background when a part changes | Never an entity per log (building-tech.md) |
| Far off | The plan's boxes in the wood's colour, faded in with `VisibilityRange` | As building-tech.md |

**A real preview, before a click** (asked for on 2026-10-12: "ideally it could be a real preview of whats happening? enshrouded is great at this"). Proposed: whatever the tool would do is applied to a copy of the base and laid again, and the difference drawn where it would be: pieces that would be added as the blueprint ghost, pieces that would be cut or taken away in red, the rest left as they are. A door aimed at a wall shows the logs it would shorten and the frame it would put in; a wall drawn across another shows its corner forming. It is laid again only when what the tool aims at changes, not every frame. This replaces today's outlines and films.

What a walker hits is the cube (D181), not the pieces drawn in it. Log ends sticking out at corners do not block anyone. A blueprint blocks nothing (building.md).

## Logs (D178)

Round logs, saddle-notched at the corners with their ends sticking out: the log cabin everyone pictures, and The Forest's. Squared logs dovetailed flush were the alternative: tidier, with less character.

**Not cylinders.** The art is real materials in detailed textures on clean shapes (D14, art-style.md). A log is made to look hewn and grown, not turned:

- **An uneven section**: seven to nine facets of different widths, never a regular polygon, slightly flattened top and bottom where it was scribed to sit on the log below, and turning a little along its length.
- **A tree's shape**: tapered, the thick end alternating course by course, with a gentle bend and a swelling here and there.
- **Knots**: now and then the stub of a cut branch.
- **Ends as cut by an axe**: a bark ring round a lighter face, a darker heart, the face a little off square.
- **Colour per face** from the palette's wood roles (Bark, Wood, WoodDark, WoodLight): bark in two tones in streaks along the log, weathered lighter on top.
- **Chinking**: a pale strip of daub in the groove between courses, for contrast.

Today the log is made in code from the kit's numbers. A kit can name a log model instead, placed for each log and stretched to fit (D177); the log kit's model is in [art/props/kit-log](../art/props/kit-log), not yet named by the kit.

**Courses.** Logs 0.3 m thick (D175), centred in a wall a cube thick and laid 0.25 m apart, so they sit into one another: two courses to a cube, 12 in a 3 m storey, the dozen logs building.md expects. Walls running east to west and walls running north to south are half a course apart, which is how logs interlock at a corner. Walls on one axis start with a split log at the foot and finish with one on top, as real cabins do. Because it goes by axis, every corner, T and cross interlocks the same way, with nothing to work out between walls.

**How walls meet.** A wall is a straight run of log cubes (D181). A cube with runs both ways is a corner, a T or a cross; a run that stops is a free end. Logs run through a corner to its cube's outer face and no further, a few centimetres short here and there, so nothing leaves the grid and anything can be built against them (2026-10-12; they ran 0.25 m past, which got in the way of whatever stood beside them). Each end of a wall is one of these:

| Where | Its logs | |
|---|---|---|
| Free end | Cut flush at the last cube | |
| Corner (L) | Both walls run through the corner cube to its outer face | The ends alternate, a course each |
| T | The through wall runs on; the stem runs into it and ends at its far face | |
| Cross | Both run through | |
| In line, at a post | The two meet end to end under an upright log covering the seam | As building.md's splice |
| Corner, the other wall built first | The built wall keeps its flush ends; the new wall runs through to the corner's face | A butt-and-pass corner, also real |

The last row is what freezing a wall's layout once a piece is in means (building-tech.md). Walls on the two axes never share a course, so a wall laid later never has to move a log already built. A corner drawn in one go, as a room or a loop of walls is, always gets the notched corner.

**Long walls.** Logs are as long as the wall, up to four metres, the longest a tree gives (D174). Longer walls are joined end to end, with the joins staggered from course to course so no seam runs up the wall.

**Openings.** An opening is cubes left empty, of any shape (D181); doors and windows are openings of a default size (D175): a door 1 m by 2 m, a window's sill at 1 m and its head at 2 m. The courses through an opening end at it, cut to a frame of boards along its own edges: jambs up its sides, a head board under the log over it, and a sill under it unless it is at the wall's foot. On the axis whose courses sit half a course up, the hole takes whole logs. An opening in the wall's middle therefore costs logs on both sides of it, as building round a doorway really does.

**Later, with roofs, floors and storeys:**

- **Gable ends**: courses carry on up, each log cut at the roof's slope, the cut made when the mesh is made (building-tech.md).
- **Floors**: split logs side by side on beams.
- **Roofs**: poles under a covering.
- **A second storey**: starts over a floor a cube thick (D181), so the split logs at the top of one storey and the foot of the next never show together.

Until floors and flat roofs have a kit of their own, those of log cubes are drawn as slabs in the kit's colours.

## Posts anywhere (D179)

Asked on 2026-10-12: "add a pillar to break up walls… arbitrarily? … giving folks some freedom is nice." Agreed: **a post is a column of cubes** (D181), which the kit draws as an upright log or timber; a column with nothing beside it either way is a post. Pick *Post*, click any cube, in a wall or in the open.

- **On a wall**, the wall's logs meet it as they meet the in-line post above: they stop at it, and the wall carries on beyond it. So a long wall broken by posts every few metres is something the player chooses, not something the kit forces.
- **At a corner**, a post takes the place of the notched corner, with flush log ends against it. That gives a second corner look for free: the corner-post cabin, as built in Canada.
- **In the open**, posts hold up a porch roof or a lean-to, or make a fence's ends.
- **Its kind can differ from the wall's**: a squared timber post or a stone pillar in a log wall. That is the next section.

Placing a post on a wall already built is like a late corner. The built logs stay as they are and the post stands over them, so a post can be added to a finished wall as a detail, without taking the wall down.

This is the first of what the developers called adding "a detail": a part placed on the skeleton that the kits lay themselves round. Beams, trims and corner stones could follow the same way, later.

## Mixing kinds (D179)

Asked on 2026-10-12: "how do you have one wall be log and one be stone". Agreed:

- **Every part has its kind.** A wall, post, floor or roof section is laid in the kind picked when it is drawn (Ctrl and the wheel, in building.md's controls). Two walls side by side can be different kinds, and so can the walls and the floor of one room. Each cube keeps its kind, so mixing is per cube.
- **Where kinds meet, the heavier one runs through.** Stone, then logs, then planks, then wattle. At a corner or a T of stone and logs, the stone wall's corner blocks run to its outer face, and the log wall ends flush against it, as a log cabin meets its stone chimney. In line, the log wall butts into the stone wall's end. Stone is thicker (0.5 m against 0.3), so the step between them shows, as it does on real buildings.
- **One wall over another.** Log cubes over stone cubes a metre high are how a cabin gets a stone footing, each laid by its own kit.
- **Changing a kind** is free on a plan nothing is in yet. Once pieces are in, the wall is frozen (building-tech.md), so it is taken down and laid again, as D146 says.
- **Drawing it** costs nothing extra: a finished building is one mesh per material already.

Built 2026-10-12: a base is laid by every kit in `assets/data/kits/` at once, each its own kind's cubes, so mixing is per cube. Logs end flush against stone, as they end at anything not open air. The kit sheet has a stone house under a hip roof and a cabin on a stone plinth a metre high.

## Checking it

The yard (building-tech.md) has a kit sheet: one of each way walls meet, a free end, a corner, a T, a cross, a long wall, posts, a wall carved a cube at a time and a small cabin with a door, windows and a gable roof, each in a row of three: built, going up and as a blueprint. Each row's vantage is written to the log, so a picture of it is one command (D69, D71):

```bash
cargo run -p terra_sandbox -- --experiment play --level yard --kit-sheet --time 16 --vantage 13,9,-6,13,0.5,-26.5 --shot out/corners.png
```

Everything built is laid in logs by default, the player's own building and the village's houses too; `--walls plain` lays it plain. The kit's file is read again when it is saved, so the look can be tuned with the sheet running. Tests in `terra_build` check the layout without a picture: the courses fill the storey, walls either way never share a course, corners interlock and nothing leaves its cubes, free ends are flush, a door cuts the courses it crosses and is framed, long walls stagger their seams, and gables shorten toward the ridge.

![The kit sheet's corners and T's: built, going up, and as a blueprint](images/log-kit-sheet-2026-10-12.png)

![A wall with a cross carved out of it, a cube at a time, and a window slot](images/log-carved-2026-10-12.png)

## What it costs

What a part costs is set apart from its pieces (2026-10-12): "we dont need 1:1 the raw materials to visually look the same in the blueprint … placing one log could place 2 or something if it made sense". So a wall's logs are counted by its length, about one log from a tree for each four metres of wall in a course, and setting one carried log may put in two short pieces at once. The numbers are a kit's, to tune.

## First build

Built:

1. **The stacked layout** in `terra_build` (`logs.rs`): each wall's logs, from the runs of log cubes and what meets their ends, in the order they go in; openings framed; gable ends in logs that shorten under the roof.
2. **The log made in code**, with the kit file for pine, `assets/data/kits/logs.ron`; a kit can name a log model instead (`terra_game`'s `build/pieces.rs`).
3. **In the game**: a build's walls drawn from the kit instead of the plain boxes, those in merged into one mesh, the rest a ghost with the next lit. Until plans come (step 3 of building-tech.md), the kit sheet sets how far each build is in.
4. **The kit sheet** on the yard.

![A log cabin from the kit sheet, late in the day](images/log-cabin-2026-10-12.png)

**Not yet:** the late corner, which needs parts with their own history (step 3); a tool for carving cubes by hand, though the operation is there and the sheet shows it; saddle notches cut into the logs, rather than logs meeting; roofs and floors with kits of their own; stone gables, so a stone wall under a gable carries boards above its eaves for now; stone cut to a roof's slope; seams between stone sections, which still show faintly.

**Doors and windows are not part of a plan** (the developers, 2026-10-12: "when you're building blueprints/etc, they dont have doors attached ... showing a partial ghost with a door does not reflect how the game should work"). A plan and a building going up show the opening and its frame only; the door or window is its own item, fitted once the building is up (construction.md). They are laid last.

**Logs stop at roofs.** A log that would pass through a roof or its eaves is cut back where the roof begins, as a builder saws the corner ends under a hip; one cut in the middle is left in two.
