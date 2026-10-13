# Building kits: how construction looks, logs first

How each kind of construction looks and is laid, starting with logs. It fills in step 4 of [building-tech.md](building-tech.md#order). How pieces are made and drawn is technical (D177), with block kinds (D197) and timber (D200) drawn from real materials by code; the look of log walls (D178), posts and mixing kinds (D179) are Decided; what a wall costs need not match its pieces one for one.

**The bar is Enshrouded's approach, not its look (D211):** cubes that touch blend into one structure, and each cube's look changes with what it touches and where it sits, as optimised cubes. Every kit here is judged side by side with Enshrouded against that, and changed where it falls short.

![The log kit: a corner, how walls meet, a doorway, and the three states](images/log-kit.svg)

## What a kit has to do

A wall is drawn as a line on the grid (D154), and the game lays its pieces (D146). One layout, worked out by rule from the plan, serves everything:

- **The blueprint** (D139) is those pieces drawn as a ghost, every log separately, so you can count what is needed, as The Forest's blueprint shows each log.
- **Going up**, each piece is either in, drawn as it will stand, or still a ghost; the next one is lit.
- **Built**, the same pieces, merged into one mesh per building (building-tech.md).
- **While drawing**, the ghost under the cursor is the same layout, laid again each time the line grows by a cube.

So a corner looks the same in the blueprint as it does built, just ghosted. Nothing is modelled twice.

## How pieces are made (D177, D197, D200)

**Props and trims are models from the art pipeline; code only arranges them** (D177). A kit's look starts as a concept of a whole building drawn to the guide and approved by the developers, [art/props/log-cabin](../art/props/log-cabin/concept.png); each prop is modelled from a concept of its own, drawn the same way, by `tools/props` (Tripo, then a clean-up in Blender to palette roles). There are none for now: the door and the shuttered window were taken out (D225, the developers on 2026-10-13: "we have some of these ugly ass door and window props we can probably just remove for now"), so every opening stands as a framed hole until better props are made.

**Long members are generated** (D200): logs, beams and planks are a profile swept along each piece the layout gives (`terra_build::timber`), textured and displaced by real materials from the pipeline, so their shape fits their wall exactly and every one differs. The timber spike compared this with Tripo's log models and with timber as a surface, and the generated logs read most as a log cabin ([research/timber-walls.md](../research/timber-walls.md)). Two profiles from the same code:

| Profile | What it is | Its ends |
|---|---|---|
| Round (the log kit) | A log in its bark: tapered from its butt, bowed, its section a little uneven and swelling, scribed flatter top and bottom; the bark stands out by its own height map (Poly Haven's `pine_bark`, CC0) | Sawn and chamfered by the axe, showing their grain (`log_end`), stacked at corners a little ragged (a saddle notch) |
| Hewn | A beam squared by the axe: faces gently waved and scalloped by the adze, edges rounded and wandering, a little less tall than a course so daub shows between | Sawn square, flush at corners (a dovetail) |

Each log differs from the next by its seed, from its place, by as much as the kit says: thicker or thinner, more or less tapered and bowed, its section turned, its texture placed anew, a little lighter or darker and warmer or cooler. Lightly stylised (D14): the variation is a tree's, not a cartoon's. A pale lime daub (Poly Haven's `white_stucco`) is recessed in the groove under each log, for the value contrast the style guide asks for.

**Block kinds are not pieces** (D197): stone, brick, plaster and earth are drawn as their cubes' own surface, each face a fine grid, rounded along every outside edge and pushed out by its material's height, so a wall of any shape, carved anywhere, is one wall with no seams (`terra_build::surface`; [research/block-modules.md](../research/block-modules.md)). The stone kit is rough fieldstone, Poly Haven's `stone_wall` (CC0); its file gives the bevel (4 cm) and how far the stone stands out (5 cm). A plan's progress counts its cubes from the bottom up, and the cubes in, the next and the rest are drawn as three surfaces, so the ghost and a wall going up work as for logs. Its edges are dressed by their role (below, D215); an end board stands where logs end against it (D210); doors and windows are fitted props (D193), none fitted for now (D225).

**Every exposed edge is dressed by its role** (D215, [research/role-pieces.md](../research/role-pieces.md)), as Enshrouded's blocks change with what they touch. When a section is laid, each edge of a kind's cubes is given a role from what is round it and the ground's samples (`layout::roles`), never stored; each belongs to one cube, so a piece laid for it is in, planned or carved with that cube, and a part-built wall wears its dress on what is up. The kit's file says what each role takes:

| Role | Where | Stone (`stone.ron`) | Logs (`logs.ron`) | Hewn (`hewn.ron`) |
|---|---|---|---|---|
| Crest | Along a top open to the sky | A crowned coping with a drip | A hewn plate as the top course | A heavier hewn plate |
| Foot | Where a wall stands on the ground or another kind, or the ground passes it | A chamfered plinth course, stepping with the ground a course at a time and run on down into it | A hewn sill beam as the first course | A heavier sill |
| Arris | Up an outside corner, or beside a thinner kind | Quoins a course each, long and short by turns | Log ends run past the corner, an eighth of a cube | A squared corner post |
| Jamb, Head, Sill | Round an opening | Jamb stones long and short, a lintel bearing 20 cm either side, a sloped sill with horns | Hewn posts up its sides | Hewn posts |
| Meet | Where a thinner kind stands on it | A weathered cap under the logs | | |
| Soffit | Under an overhang | A fascia | | |
| Nose | Along a step | A tread with a rounded nose | A flight laid as steps: logs across it, a log along each nose, board treads, a stringer up each side | The same, hewn |
| Border | Along a floor a wall stands on | A border flag | | |
| Inside | Up an inside corner | (nothing) | | |

Profiles and corner stones are drawn in Blender from a source in numbers, `art/kits/<kit>/pieces.json`, into a file the kit names (`tools/kits`); a profile is swept a cube at a time, mitred out and back round corners, with a chamfered joint where it runs on and its ends closed; each piece stands out up to an eighth of a cube (D192) and casts the shadow line that makes a wall read as built. Masks shade what a kit draws: dirt rising unevenly from the ground, worn outside edges, timber greyed higher up; and a roof's covering greened by moss and lichen in blotches toward its eaves, greyed toward its ridge and worn lighter along its eaves and verges (D220). Dressed stone is Poly Haven's `rock_boulder_dry` (CC0), tinted toward the rubble; hewn members `rough_wood`. Far off the bands stay and the corner stones drop out. A kit's dress costs about 140 triangles a cube near for stone, 60 for logs, 90 for hewn, averaged over the checker's cabin; `cargo xtask kits` checks every role a kit dresses has its piece, each within an eighth of a cube, and their budgets (600 triangles a metre of profile, 320 a corner stone).

**A kit is a file** in `assets/data/kits/` (D136): its pattern (stacked, for timber; masonry, for block kinds drawn as their surface), its sizes, its timber (profile, notch, sides, taper, bend, unevenness, bark relief, daub, tints and how much each log varies), its materials, its props' models, its cost and its dress. A pine cabin, an oak one and a hewn-timber one are three files; nothing about a kind of timber is code. The log kit is `logs.ron`, under shakes; `hewn.ron` is hewn beams under thatch; `stone.ron` is fieldstone under slate.

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

**Not cylinders.** The art is real materials in detailed textures on clean shapes (D14, art-style.md). A log is made to look grown and cut, not turned: an uneven section, scribed flatter where it sits on the log below; a tree's taper, the butt alternating by seed, a gentle bend and a swelling here and there; bark that stands out from it; sawn ends with a bark ring, lighter sapwood and a darker heart; and a pale strip of daub in the groove between courses, for contrast.

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
- **Roofs**: poles under a covering.
- **A second storey**: starts over a floor a cube thick (D181), so the split logs at the top of one storey and the foot of the next never show together.

Floors and flat roofs of log cubes are drawn as decks (D206).

## Posts anywhere (D179)

Asked on 2026-10-12: "add a pillar to break up walls… arbitrarily? … giving folks some freedom is nice." Agreed: **a post is a column of cubes** (D181), which the kit draws as an upright log or timber; a column with nothing beside it either way is a post. Pick *Post*, click any cube, in a wall or in the open.

- **On a wall**, the wall's logs meet it as they meet the in-line post above: they stop at it, and the wall carries on beyond it. So a long wall broken by posts every few metres is something the player chooses, not something the kit forces.
- **At a corner**, a post takes the place of the notched corner, with flush log ends against it. That gives a second corner look for free: the corner-post cabin, as built in Canada.
- **In the open**, posts hold up a porch roof or a lean-to, or make a fence's ends.
- **Its kind can differ from the wall's**: a squared timber post or a stone pillar in a log wall. That is the next section.

**Posts cut as posts** (D193, D220): the hammer's *Post* lays a column of cubes cut as posts, a storey high to the floor over it, on a footing on the ground. Each kit draws it as its file dresses `Post`: the log kit a round log a little thinner than its wall's, the hewn kit a squared beam, each on a pad of boards where it stands on something and under a short bolster across its top where something rests on it; the stone kit a pillar 36 cm square of its rubble, a drum a cube, with its plinth round the foot and its coping round the cap. A post collides and holds as a whole cube: a deck or a beam laid on it is joined through it, and nothing of it comes out while they rest on it.

Placing a post on a wall already built is like a late corner. The built logs stay as they are and the post stands over them, so a post can be added to a finished wall as a detail, without taking the wall down.

This is the first of what the developers called adding "a detail": a part placed on the skeleton that the kits lay themselves round. Beams, trims and corner stones could follow the same way, later.

## Mixing kinds (D179)

Asked on 2026-10-12: "how do you have one wall be log and one be stone". Agreed:

- **Every part has its kind.** A wall, post, floor or roof section is laid in the kind picked when it is drawn (Ctrl and the wheel, in building.md's controls). Two walls side by side can be different kinds, and so can the walls and the floor of one room. Each cube keeps its kind, so mixing is per cube.
- **Where kinds meet, the heavier one runs through.** Stone, then logs, then planks, then wattle. At a corner or a T of stone and logs, the stone wall's corner blocks run to its outer face, and the log wall ends flush against it, as a log cabin meets its stone chimney. In line, the log wall butts into the stone wall's end. Stone is thicker (0.5 m against 0.3), so the step between them shows, as it does on real buildings.
- **One wall over another.** Log cubes over stone cubes a metre high are how a cabin gets a stone footing, each laid by its own kit.
- **Changing a kind** is free on a plan nothing is in yet. Once pieces are in, the wall is frozen (building-tech.md), so it is taken down and laid again, as D146 says.
- **Drawing it** costs nothing extra: a finished building is one mesh per material already.

Built 2026-10-12: a base is laid by every kit in `assets/data/kits/` at once, each its own kind's cubes, so mixing is per cube. Logs end flush against stone, as they end at anything not open air; stone runs through corners and T's. The kit sheet has stone and logs meeting every way walls meet: a stone house under a hip roof, a log cabin on a stone plinth a metre high, an L, a T each way round, in line, stone pillars in a log wall, and a log cabin with a stone end wall.

## Stairs, slopes and halves (D193, D220)

A cube can be cut to a shape: a half (its lower half), a stair (two steps, each a quarter of a metre high and deep, rising toward a side) or a slope (a ramp up to a side). The hammer lays a *Stairs* flight, stairs or a ramp a metre or a metre and a half wide, up a storey to the floor above, each column whole cubes under its cut top; or one *Block* of any shape against the face aimed at, joining the plan of its kind beside it. The wheel turns which way it rises. Walked, each shape holds a walker at its top under their feet: a stair's two steps, a slope's ramp, a half's top.

| | Stone | Logs and hewn |
|---|---|---|
| Stair | A stone a tread, rounded and displaced like the wall, a tread with a rounded nose on each | Logs across the flight, a log along each step's nose, a board tread on each step, a stringer up each open side |
| Slope | The stone's surface cut to the ramp, flagstones on it, a coping raking up each open side | Boards up the fall of the ramp, a cleat across them, a stringer up each open side |
| Half | The stone's surface cut to half height, flagstones on it, a coping round its open top | A low deck: boards on a rim of beams, joists where it is in the air |

**Slopes are not roofs.** A roof is laid by its kit from roof sections over walls (D209), a height field of courses, ridges and eaves; a slope is a ramp of its own kind's material, and under or beside a roof it is a cube like any other.

**A solid mass of timber** (a platform, a slab: whole cubes of a timber kind side by side two by two on a layer) is laid as a framed deck, not as walls crossing: boards on top on joists where it is in the air, a rim of sawn beams stacked a course deep round its open sides, nothing inside it, and its sides below its top as walls.

## Between a building and the ground, and its floors (D206)

**Footings** are stone cubes, the stone kit's, laid into every plan on the ground as a part of their own and built first: under each wall from the floor's layer, under a floor's edge from the layer below, down to the cube the ground passes through, so they are set into it all round and no gap shows on a slope. They are drawn as the stone's surface, one with any stone wall or floor they meet. A log cabin stands on a course of stone that rises to its floor on level ground and steps down the hill on a slope. The ground outside is never changed.

**Floors** are drawn by their kind. A stone floor is the stone's surface, its top in flagstones (Poly Haven's `grey_stone_path`, CC0), flatter than its walls since it is walked on; a stone wall's top is capped in them too. A timber floor is a deck, made by the timber code: sawn boards two to a cube, as long as a tree gives, their joints staggered; each board drawn from one plank of `weathered_planks`, so no seam runs down a board; where the floor meets a wall standing on a footing, the boards run on to the logs' faces; a rim beam along any side left open, and joists under it where it is in the air.

**A floor drawn alone**, with the hammer's *Floor* on any storey, with no walls on it, is finished at its edges: a timber deck with its rim of beams and its joists, a stone floor paved in its flagstones, its edges rounded into the ground.

**Door steps** are stone slabs from each ground-floor door in an outside wall down to the ground, a riser of no more than 18 cm each, set into the ground; none where the threshold is less than a step up.

**Doors** are fitted props (D207); none is fitted or drawn for now (D225), so a doorway is its frame and steps.
## Roofs (D209)

A kit lays its own roofs, from the shape the roof's sections make over the walls (`terra_build::layout::roofs`, drawn by `roofing`): a gable, a hip or a lean-to over any plan, an L, T or U getting its valleys and its ridges dying into one another from the sections alone. In the order a roof goes up, a stage of its cells at a time:

| Stage | Pieces |
|---|---|
| Frame | A rafter up the slope at every cube, in the kit's timber (round poles for logs and thatch, hewn for stone), and a beam under every ridge, hip and valley |
| Battens | One under the foot of each course, along the face |
| Covering | The covering a square at a time, from the eaves up; boards along the eaves (fascia) and the verges (bargeboards); then caps over ridges and hips (two boards, or a roll of the covering), valley boards, and a mortar fillet where a wall stands up through the roof |

The covering is courses, level up the slope so they run round a hip, each stepping out at its butt by the kit's amount, wandering a little along it; the texture is laid so its rows sit on the steps, each course shifted along so no two rows repeat. Shakes over pine logs, slates (Poly Haven's `grey_roof_tiles_02`) over stone, a foot of thatch (`riet_01`) over hewn timber: each kit's `roof` numbers and its `Covering` material, so a roof is data too.

**Gables close to the roof.** Over a timber wall the courses carry on up, each log's ends cut to the slope over it; over a block kind's wall its cubes carry on up as its own surface, cut off at the roof's underside, so a stone end wall is stone to the ridge.

A ghost's covering is a plain sheet, without its courses, which costs a tenth as much to draw.

## Checking it

The yard (building-tech.md) has a kit sheet: one of each way walls meet, a free end, a corner, a T, a cross, a long wall, posts, a wall carved a cube at a time and a small cabin with a doorway, windows and a gable roof, every shape in each kit (a platform on posts reached by stairs and a ramp, a step of halves, a post alone) and a floor alone, each in a row of three: built, going up and as a blueprint. Each row's vantage is written to the log, so a picture of it is one command (D69, D71): To the east, every roof style (gable, hip, lean-to) over every plan (a rectangle, an L, a T, a U) in each kit and mixed, built, part-built and as a ghost; `--sheet-rows TEXT` lays only the rows whose names hold it, as `--sheet-rows "stone: L"`, since the whole sheet is heavy to lay.

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

1. **The stacked layout** in `terra_build::layout` (`runs`, `trims`, `ground`, `floors`, `blocks`): each wall's logs, from the runs of log cubes and what meets their ends, in the order they go in; openings framed; gable ends in logs that shorten under the roof.
2. **Generated logs** (D200), `terra_build::timber`, from the kit file for pine, `assets/data/kits/logs.ron`.
3. **In the game**: a build's walls drawn from the kit instead of the plain boxes, those in merged into one mesh, the rest a ghost with the next lit. Until plans come (step 3 of building-tech.md), the kit sheet sets how far each build is in.
4. **The kit sheet** on the yard.

![A log cabin from the kit sheet, late in the day](images/log-cabin-2026-10-12.png)

**Not yet:** the late corner, which needs parts with their own history (step 3); a tool for carving cubes by hand, though the operation is there and the sheet shows it; saddle notches cut into the logs, rather than logs meeting; floors with kits of their own; seams between stone sections, which still show faintly.

**Doors and windows are not part of a plan** (the developers, 2026-10-12: "when you're building blueprints/etc, they dont have doors attached ... showing a partial ghost with a door does not reflect how the game should work"). A plan and a building going up show the opening and its frame only; the door or window is its own item, fitted once the building is up (construction.md). None is fitted for now (D225).

**Logs stop at roofs.** A log that would pass through a roof or its eaves is cut back where the roof begins, as a builder saws the corner ends under a hip; one cut in the middle is left in two.
