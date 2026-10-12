# Kits: how construction looks, logs first

Status: a draft for the developers, 2026-10-12. Asked for that day: "figure out the new models … have them be modular (how does a corner render vs a long wall, both in blueprint and non blueprint mode)", starting with logs. It fills in step 4 of [structures.md](structures.md#order) for the first kind of construction. How pieces are made and drawn is technical and mine (D177). The developers read it the same day and left the look of log walls to me, on theme, with visual interest and organic where possible (D178); agreed posts on grid points and mixing kinds (D179); and said what a wall costs need not match its pieces one for one.

![The log kit: a corner, how walls meet, a doorway, and the three states](images/log-kit.svg)

## What a kit has to do

A wall is drawn as a line on the grid (D154), and the game lays its pieces (D146). One layout, worked out by rule from the plan, serves everything:

- **The blueprint** (D139) is those pieces drawn as a ghost, every log separately, so you can count what is needed, as The Forest's blueprint shows each log.
- **Going up**, each piece is either in, drawn as it will stand, or still a ghost; the next one is lit.
- **Built**, the same pieces, merged into one mesh per building (structures.md).
- **While drawing**, the ghost under the cursor is the same layout, laid again each time the line grows by a cell.

So a corner looks the same in the blueprint as it does built, just ghosted. Nothing is modelled twice.

## How pieces are made (D177)

**Every piece is a model from the art pipeline; code only arranges them** (revised 2026-10-12). Logs drawn by code from numbers broke the art direction (D14): their look was invented, not drawn to the style guide. Now a kit's look starts as a concept of a whole building drawn to the guide and approved by the developers, [art/props/log-cabin](../art/props/log-cabin/concept.png); each piece is modelled from a concept of its own, drawn the same way, by `tools/props` (Tripo, then a clean-up in Blender to palette roles). The layout (below) only places them: it repeats a piece, stretches a log along its length to fit its wall, cuts a piece by a plane where it must end, such as a split log or a shingle panel at a hip, and tiles panels over a roof. That is how shipped games build modular kits.

The log kit's pieces, from the approved cabin:

| Piece | Laid as |
|---|---|
| A straight log, a few versions | Every course, corner end, gable log and purlin, stretched along its length to fit; cut along its middle for a split log |
| A panel of shakes | Tiled over each face of a roof, cut at its edges, hips and valleys |
| A ridge cap | Along each ridge and hip |
| A timber board | Round every opening, stretched to its edges |
| A door, a shuttered window | Set into openings of their size |
| A footing of fieldstones | Under walls on the ground |

**A kit is a file** in `assets/data/kits/` (D136): its pattern (stacked, for logs), its sizes, its pieces' models, and its colours as palette roles (D14). A pine cabin and a dark-wood cabin are two files.

## Drawing them

| State | How | Why |
|---|---|---|
| Blueprint | Each piece in a pale, see-through blueprint material, edges brighter so each log reads apart; red where it cannot go. Drawn instanced, many pieces to one draw | Fifty cabins of blueprint are a few thousand pieces, which instancing makes cheap |
| Going up | The pieces in are drawn solid, the rest ghost, the next outlined and pulsing | Says what to bring and where it goes (construction.md's prompt) |
| Built | One mesh per building and material, rebuilt in the background when a part changes | Never an entity per log (structures.md) |
| Far off | The plan's boxes in the wood's colour, faded in with `VisibilityRange` | As structures.md |

**A real preview, before a click** (asked for on 2026-10-12: "ideally it could be a real preview of whats happening? enshrouded is great at this"). Proposed: whatever the tool would do is applied to a copy of the base and laid again, and the difference drawn where it would be: pieces that would be added as the blueprint ghost, pieces that would be cut or taken away in red, the rest left as they are. A door aimed at a wall shows the logs it would shorten and the frame it would put in; a wall drawn across another shows its corner forming. It is laid again only when what the tool aims at changes, not every frame. This replaces today's outlines and films.

What a walker hits stays the grid's: the wall as a box its kit's thickness, 0.3 m for logs (D175). Log ends sticking out at corners do not block anyone. A blueprint blocks nothing (construction.md).

## Logs (D178)

Round logs, saddle-notched at the corners with their ends sticking out: the log cabin everyone pictures, and The Forest's. Squared logs dovetailed flush were the alternative: tidier, with less character.

**Not cylinders.** The art is shape, a palette colour per face, and light, with no textures (art.md). A log is made to look hewn and grown, not turned:

- **An uneven section**: seven to nine facets of different widths, never a regular polygon, slightly flattened top and bottom where it was scribed to sit on the log below, and turning a little along its length.
- **A tree's shape**: tapered, the thick end alternating course by course, with a gentle bend and a swelling here and there.
- **Knots**: now and then the stub of a cut branch.
- **Ends as cut by an axe**: a bark ring round a lighter face, a darker heart, the face a little off square.
- **Colour per face** from the palette's wood roles (Bark, Wood, WoodDark, WoodLight): bark in two tones in streaks along the log, weathered lighter on top.
- **Chinking**: a pale strip of daub in the groove between courses, for contrast.

If generated logs fall short, the fallback is a few logs modelled by the props pipeline (tools/props), cut to length in code and capped with end grain.

**Courses.** Logs 0.3 m thick (D175), laid 0.25 m apart, so they sit into one another. That makes 12 courses in a 3 m storey, the dozen logs construction.md expects. Walls running east to west and walls running north to south are half a course apart, which is how logs interlock at a corner. Walls on one axis start with a split log at the foot and finish with one on top, as real cabins do. Because it goes by axis, every corner, T and cross interlocks the same way, with nothing to work out between walls.

**How walls meet.** Each end of a wall is one of these, from what it meets at that grid point:

| Where | Its logs | |
|---|---|---|
| Free end | Cut flush, half the wall's thickness past the point | |
| Corner (L) | Both walls run 0.25 m past the corner | The notched ends alternate, a course each |
| T | The through wall runs on; the stem runs through it and sticks out the far side | |
| Cross | Both run through | |
| In line | The two meet end to end under an upright log, a post, covering the seam | As construction.md's splice |
| Corner, the other wall built first | The built wall keeps its flush ends; the new wall runs past | A butt-and-pass corner, also real |

The last row is what freezing a wall's layout once a piece is in means (structures.md). Walls on the two axes never share a course, so a wall laid later never has to move a log already built. A corner drawn in one go, as a room or a loop of walls is, always gets the notched corner.

**Long walls.** Logs are as long as the wall, up to four metres, the longest a tree gives (D174). Longer walls are joined end to end, with the joins staggered from course to course so no seam runs up the wall.

**Openings.** Doors and windows take the sizes of D175: a door 1 m by 2.1 m, a window's sill at 0.9 m and its head level with the door's. Logs in the courses an opening crosses are cut to a frame of boards, the jambs, so those courses have two logs. Above the opening, the first log runs over it on a head board, flattened underneath; below a window, logs run under it to a sill. An opening in the wall's middle therefore costs logs on both sides of it, as building round a doorway really does.

**Later, with roofs, floors and storeys:**

- **Gable ends**: courses carry on up, each log cut at the roof's slope, the cut made when the mesh is made (structures.md).
- **Floors**: split logs side by side on beams.
- **Roofs**: poles under a covering.
- **A second storey**: starts over a floor 0.2 m thick, so the split logs at the top of one storey and the foot of the next never show together.

## Posts anywhere (D179)

Asked on 2026-10-12: "add a pillar to break up walls… arbitrarily? … giving folks some freedom is nice." Agreed: **a post is a part of its own, on the half-metre grid**: on cubes (D181), a column of cubes the kit draws as an upright log or timber. Pick *Post*, click any cube, in a wall or in the open.

- **On a wall**, the wall's logs meet it as they meet the in-line post above: they stop at it, and the wall carries on beyond it. So a long wall broken by posts every few metres is something the player chooses, not something the kit forces.
- **At a corner**, a post takes the place of the notched corner, with flush log ends against it. That gives a second corner look for free: the corner-post cabin, as built in Canada.
- **In the open**, posts hold up a porch roof or a lean-to, or make a fence's ends.
- **Its kind can differ from the wall's**: a squared timber post or a stone pillar in a log wall. That is the next section.

Placing a post on a wall already built is like a late corner. The built logs stay as they are and the post stands over them, so a post can be added to a finished wall as a detail, without taking the wall down.

This is the first of what the developers called adding "a detail": a part placed on the skeleton that the kits lay themselves round. Beams, trims and corner stones could follow the same way, later.

## Mixing kinds (D179)

Asked on 2026-10-12: "how do you have one wall be log and one be stone". Agreed:

- **Every part has its kind.** A wall, post, floor or roof section is laid in the kind picked when it is drawn (Ctrl and the wheel, in construction.md's controls). Two walls side by side can be different kinds, and so can the walls and the floor of one room. The grid keeps the kind with each wall, as it keeps what fills it now.
- **Where kinds meet, the heavier one runs through.** Stone, then logs, then planks, then wattle. At a corner or a T of stone and logs, the stone wall's corner blocks run to its outer face, and the log wall ends flush against it, as a log cabin meets its stone chimney. In line, the log wall butts into the stone wall's end. Stone is thicker (0.5 m against 0.3), so the step between them shows, as it does on real buildings.
- **One wall over another.** A wall drawn on top of a lower one, such as logs over a stone wall a metre high, is how a cabin gets a stone footing. Walls already take a height (construction.md), so this is walls stacked by height, laid by their own kits.
- **Changing a kind** is free on a plan nothing is in yet. Once pieces are in, the wall is frozen (structures.md), so it is taken down and laid again, as D146 says.
- **Drawing it** costs nothing extra: a finished building is one mesh per material already.

## Checking it

The yard (structures.md) has a kit sheet: one of each way walls meet, a free end, a corner, a T, a cross, a long wall, posts, a wall carved a cube at a time and a small cabin with a door, windows and a gable roof, each in a row of three: built, going up and as a blueprint. Each row's vantage is written to the log, so a picture of it is one command (D69, D71):

```bash
cargo run -p terra_sandbox -- --experiment play --level yard --kit-sheet --time 16 --vantage 13,9,-6,13,0.5,-26.5 --shot out/corners.png
```

Everything built is laid in logs by default, the player's own building and the village's houses too; `--walls plain` lays it plain. The kit's file is read again when it is saved, so the look can be tuned with the sheet running. Tests in `terra_build` check the layout without a picture: the courses fill the storey, walls either way never share a course, corners run past and free ends are flush, a door cuts the courses it crosses and is framed, long walls stagger their seams, and gables shorten toward the ridge.

![The kit sheet's corners and T's: built, going up, and as a blueprint](images/log-kit-sheet-2026-10-12.png)

## On cubes (D181)

Structures are strictly on the voxel grid: a wall is a row of half-metre cubes (structures.md). Nothing above changes in how logs look; the layout reads the cubes (built 2026-10-12):

- **A wall is a straight run of log cubes**, its logs centred in it, two courses to a cube.
- **Corners, T's and crosses** are cubes with runs both ways, and **free ends** where a run stops, as above. Logs run past a corner only into the open air, never into a room.
- **Carving a cube** cuts the courses through it, which end at the hole; on the axis whose courses sit half a course up, the hole takes whole logs. Any shape of hole is framed along its own edges: jambs up its sides, a head reaching into the log over it, a sill under it unless it is at the wall's foot, as a doorway is. Doors and windows are holes of a default size.
- **A column of cubes alone is a post**: an upright log.
- **Floors and flat roofs** of log cubes are drawn as slabs in the kit's colours, until floors have a kit of their own.
- **Mixing kinds** is per cube: log cubes over stone cubes are a cabin on a stone footing, once there is a stone kit.

![A wall with a cross carved out of it, a cube at a time, and a window slot](images/log-carved-2026-10-12.png)

## What it costs

What a part costs is set apart from its pieces (2026-10-12): "we dont need 1:1 the raw materials to visually look the same in the blueprint … placing one log could place 2 or something if it made sense". So a wall's logs are counted by its length, about one log from a tree for each four metres of wall in a course, and setting one carried log may put in two short pieces at once. The numbers are a kit's, to tune.

## First build

Built 2026-10-12:

1. **The stacked layout** in `terra_build` (`logs.rs`): each wall's logs, from the grid's walls and what meets their ends, in the order they go in; openings framed; gable ends in logs that shorten under the roof.
2. **The log made in code**, with the kit file for pine, `assets/data/kits/logs.ron`.
3. **In the game**: a build's walls drawn from the kit instead of the plain boxes, those in merged into one mesh, the rest a ghost with the next lit. Until plans come (step 3 of structures.md), the kit sheet sets how far each build is in.
4. **The kit sheet** on the yard.

![A log cabin from the kit sheet, late in the day](images/log-cabin-2026-10-12.png)

**Not yet:** the late corner, which needs parts with their own history (step 3); a second kit, so log and stone can mix (D179); a tool for carving cubes by hand, though the operation is there and the sheet shows it; saddle notches cut into the logs, rather than logs meeting; roofs and floors with kits of their own; eaves over the walls (D175); doors in the frames.
