# Kit review: the wood and stone kits at 2, 5 and 12 metres

2026-10-14. Question: what visual imperfections, of the kind the mesh checker does not catch, are left in the wood and stone kits (logs, hewn timber, fieldstone, shakes and slate, and where they meet), seen close (2 m), mid (5 m) and from the street (12 m) at noon and in the evening; which live in shared code and can be fixed there, and which are a kit's data. The developers: "theres still imperfections in some of the kits", then narrowed to "the wood, the rocks, the construction, the destroy". Earth, lime-washed earth, wattle and daub and thatch were left out as frozen.

The bar: [docs/art-style.md](../docs/art-style.md) (D14: real materials, lightly shaded, value before hue, one kit one grade), D211 (blended as Enshrouded's are) and D251 and D290 (one material system, no kit special-cased in code).

## How it was done

The kit sheet's rows (`--kit-sheet --level yard`, `--sheet-rows`) on the Windows GPU through the remote (`tools/sandbox/client.ts`), each built house shot from fixed views worked out from the row's logged vantage: close at its south-west corner, mid, the street, its roof, its eave, its gable straight on and close, and inside; at 12:00 and 18:00, the clock set over the remote. Rows: the log cabin; the stone house under a hip; each wall kind's gable end under shakes and slate; logs on a stone plinth; stone and logs in a T; a log cabin with a stone end wall; posts, shapes, a floor alone and decks. The same views again after each fix, and once at the low preset to tell relief mapping from mesh. Each fix measured too where a number could say it: `cargo xtask texels` (new, below) for texture stretch, and the mesh checker's ratchet for what the fixes might break. The contact sheets, before beside after, are in `shots/kits/` of the review's checkout: `logs.jpg`, `hewn.jpg`, `stone.jpg`, `shakes.jpg`, `slate.jpg` and `mixes.jpg`.

Real buildings compared against: Appalachian and Scandinavian log cabins (saddle-notched round logs, weathered silver-grey with darker ends; dovetailed hewn logs with white lime chinking), Cotswold and Pennine rubble houses with dressed quoins and lintels, cedar-shake and Welsh-slate roofs.

## A new measure: how far textures are stretched

`cargo xtask texels` (D320, `crates/terra_build/examples/texels.rs`) lays every roof over every plan in each kind and every two kinds mixed, as the mesh checker does, and for each triangle drawn near measures how much larger than made its texture is drawn along its most stretched way, against the metres its kit names; where a piece carries its own UVs it compares the two ways with each other. It prints each kit's and material's share of area past 1.5:1 and 2.5:1, where the worst is, and each modelled module as baked, so a stretch in a model's own UVs is told from one its laying makes. What it found is in the tables below.

## Logs

| # | What | Seen | Cause | Now |
|---|---|---|---|---|
| 1 | Every sawn end one tint, a bright orange disc against weathered grey-brown faces: read as a row of stickers at 5 and 12 m, worst in evening light | `logs.jpg`, close and mid | `timber::cap` coloured every end `end_tint`, unvaried; the tint itself warm and light | **Fixed in code** (D320): each end takes its log's shade and warmth and then its own, a tenth lighter or darker and greyed by up to six tenths. **Data to D290**: `end_tint` (0.72, 0.6, 0.47) to about (0.58, 0.5, 0.42), tried on the sheet: ends sit in the wall at every distance |
| 2 | Mud daub in the grooves reads as a saturated orange line in shade | close, mid | `clay_floor_001` under the pattern's `chinking` (0.92, 0.9, 0.86) | **Data to D290**: `colours.chinking` about (0.66, 0.62, 0.58), tried |
| 3 | A doorway's and window's head lit olive-yellow from below, brighter than the wall | mid, eave | The sky light lights an underside by the sunlit ground's whole bounce | **Fixed** (D320): faces looking down shaded as baked occlusion would, up to two thirds |
| 4 | A deck's joists, rim beams and stringers each one column of their plank smeared along their sides, 99:1 | `texels`: Floor 34% of area past 1.5:1, 10% past 2.5:1 | A plank drawn from one board of its texture (`strips`) mapped across by its section's x only | **Fixed** (D320): the plank runs round every face; 20% past 1.5:1 (a plank strip on a wider board), 0.1% past 2.5:1 |
| 5 | Indoors, the bark flecked bright white-blue, as if wet | inside | Sky reflections indoors; SSAO is all that occludes them | Remaining: lighting, not a kit's (indoor reflection occlusion or a room's probe) |
| 6 | Silvered tops catch a strong blue sky sheen at noon | close | `log_wood`'s roughness under the painted sky | Remaining, minor: **to D290** to check `log_wood`'s ARM roughness against `pine_log`'s |

## Hewn timber

| # | What | Seen | Cause | Now |
|---|---|---|---|---|
| 1 | Dovetailed ends bright, flat orange-tan blocks beside dark adzed faces: read as plywood at 2 and 5 m | `hewn.jpg`, close, eave | As logs 1, `end_tint` (0.9, 0.8, 0.7) | **Fixed in code** as logs 1. **Data to D290**: `end_tint` about (0.66, 0.58, 0.5), tried |
| 2 | The window head's underside a bright yellow band | eave | As logs 3 | **Fixed** (D320) |
| 3 | The stone footing's top edge smeared into vertical streaks at 2 m | close | Relief mapping (parallax) on the rounded edge: gone at the low preset, which draws no relief. The planar texture's tangent frame turns on the bevel and the parallax steps follow it | **To D290** (the shader is theirs): fade the parallax depth where the surface turns away from its texture's plane, as Unreal's and Unity's POM fade by view and by vertex weight |
| 4 | White lime chinking between every course reads as stripes at 12 m | street | As built: real dovetailed cabins are chinked white | Kept |

## Fieldstone

| # | What | Seen | Cause | Now |
|---|---|---|---|---|
| 1 | Every lintel's soffit and every quoin's and jamb stone's underside lit olive-green, brighter than the wall's shaded face | `stone.jpg`, mid | As logs 3 | **Fixed** (D320) |
| 2 | Under a bedded sill, the stones slant into fins and hang like teeth at 2 m | close | As hewn 3: relief mapping on the face built up round the sill, gone at the low preset | **To D290**, with hewn 3 |
| 3 | The plinth and copings read smooth, like cast concrete, at 2 m | close | `rock_boulder_dry` at 1.4 m a copy on long smooth profiles | **Data to D290**: the block pattern's `Dressed` at about 0.9 m, tried: more grain, still reads dressed |
| 4 | Jamb stones and quoins stand out from the rubble as boxes with flat sides, close | close | The jamb and quoin sets' depth in `pieces.json` against the rubble's 5 cm relief | **To D290** (pieces): set their faces at the rubble's proud face, not past it |
| 5 | A wall's flagstone top carried round its rounded edge, stretched up to 3.7:1 | `texels`: stone Floor 2.5% past 2.5:1, at a wall's top under the eaves | The top's planar mapping on the steeper part of the bevel | Remaining, minor: mostly under the eaves |

## Shakes

| # | What | Seen | Cause | Now |
|---|---|---|---|---|
| 1 | The ridge capping near white against the dark shakes: a pale line along every ridge from the street | `shakes.jpg`, roof | The ridge's part takes no tint (eaves and verges do) | **Data to D290**: the roof pattern's `Ridge` and `Hip` parts a tint about (0.66, 0.62, 0.58), tried: grey-brown, of the roof |
| 2 | The ridge's copies meet with a visible step, and its texture is stretched up to 7 to 9:1 | roof; `texels`: shakes Piece 30% past 2.5:1, the module as baked 0% | `modules::seat` drapes a module modelled flat over a crease, bending its halves about the line | Remaining: a ridge modelled for a pitch and laid unbent, or drape only the part beyond `blend`; goals review 3 finding 2 |
| 3 | Bargeboards sheared: their nail holes and knots lean | gable close; `texels`: shakes Trim 46% past 2.5:1, as baked 0% | A verge's board is stood plumb (`Seat::Plane { up: Y / n.y }`): on a 45-degree roof its texture is sheared 2.6:1 | Remaining: square to the slope in the gable's plane would be unsheared, but changes its ends at the eaves, which the mesh checker holds; to do with the next roof-edge change |

## Slate

| # | What | Seen | Cause | Now |
|---|---|---|---|---|
| 1 | From the street the slate reads brown-grey, like weathered boards, not blue-grey slate | `slate.jpg`, street, evening | `grey_roof_tiles_02` under the pack's tint (0.6, 0.62, 0.66) | **Data to D290**: tint about (0.5, 0.53, 0.58), tried |
| 2 | The hip and ridge stones pale against it | street | As shakes 1 | **Data to D290**, as shakes 1 |
| 3 | The ridge's stone copies stretched where draped, 20% past 2.5:1 | `texels` | As shakes 2 | Remaining, as shakes 2 |

## Where they meet

| # | What | Seen | Now |
|---|---|---|---|
| 1 | Logs on a stone plinth: the stone's cap under the logs' sill reads as built, at every distance | `mixes.jpg` | Good |
| 2 | Stone running through logs at a T: logs end flush against it, quoins up the joint | T | Good |
| 3 | A log cabin with a stone end wall: the stone carried up to the verge, the logs' ends cut to it | stone end | Good |
| 4 | Every underside where kinds meet (a cap's, a sill's) was lit by the ground's bounce | | Fixed with the rest (D320) |

## Summary

Fixed in shared code (D320): undersides shaded as baked occlusion would be, every kit; sawn ends varied and greyed, every coursed kit; a plank's texture run round its faces, every deck, joist, rim and stair. Sent to D290 as data: the logs' and hewn ends' tints, the logs' daub, the dressed stone's scale, the jamb and quoin depth, the slate's tint, and a tint for the ridges and hips of both roofs; and the relief mapping at rounded edges and beds, which is the shader's. Remaining: the ridge's stretch over a crease and the bargeboards' shear (modules and roofing, with the mesh checker), sky reflections indoors (lighting), and a wall top's flagstones round its bevel (minor).
