# Kits

Makes a building kit's dressed pieces (D215, [docs/building-kits.md](../../docs/building-kits.md)): the sections a layout sweeps along each exposed edge by its role (a coping, a plinth, a sill, a lintel) and the corner stones laid up an edge a course at a time (quoins, jamb stones), from their source under `art/kits/<kit>/`, in Blender.

```bash
tools/blender/run.sh tools/kits/pieces.py art/kits/stone/pieces.json assets/data/kits/pieces/stone.ron
```

The source, `pieces.json`, says each piece in numbers a person can read and change:

- **Profiles**: a polyline of points (out, up) in metres from the edge it is swept along, out from the cube's face and up from the edge's line, nothing past an eighth of a cube out (D192); `round` names the points Blender's vertex bevel rounds, by `radius`. The two ends of the line lie inside the wall: the layout closes and mitres them.
- **Corner sets**: stones at an edge's corner, their two faces standing `proud` of the wall's, a course `height` less their `joint`; `long` gives each variant's length along its first face and its second, and `short`, if any, the variants laid by turns with them (jamb stones, long and short along the wall); a set with no `short` turns its stones each course instead (quoins). Each stone's faces are divided about every `cell`, pillowed to their middle by `pillow` and pitched by the chisel by `pitch`, their arrises bevelled by `bevel`, shaded hard at creases and smooth across curves with face-weighted normals.

- **Modules** (D258): a length of mesh along +X, `length` long, Y up from what it sits on and Z across: a section of a `kind` swept along it and shaped as it goes. `timber` is squared timber or a board between `y` and `z`, its corners rounded by `round` and wandering, its faces scalloped by the `adze` every `period`; `pole` a round pole of `radius`; `mound` a capping crowned across `half` either side and `height` over `base`, its edge cut in `points` every `period`; `bundle` an oval about `y`, `z` of `radii`, `pinch`ed every `period`; `log` a log in its bark of `radius`, `uneven` and `lumps`y, `flat`ter where it bears, with `stubs` (fewest, most a copy) of branches sawn off `stub_length` proud and `stub_radius` round on its two faces. `rods` lays thin rods along it (a ridge's liggers); `parts` joins several sections in one module; `joint` presses a gap at each end, as between ridge stones; `open` leaves its ends open so copies run on as one member. `cap` (`flat`, or a `dome` drawn in over `reach` to a `pivot`) closes a line where it stops; `blend` is how far either side of a crease a draped module turns from one slope to the other. Each of its `variants` is written near and at a far budget (`far`, a fraction).

- **Modelled modules** (D251, D258): a module with a `model`, a model kept as source art (a Tripo model of a concept, pending the developers' approval, `art/kits/<kit>/<piece>/model.glb`), is read in place of numbers: laid along +X by its longest side (`lies`: `board`, `log`, or `capping` and `channel`, which are unfolded flat so the game can drape them over any angle a crease makes), fitted into its box (`length`, `y`, `z`) and cut to its `budget`. The kit's modelled modules have their own textures baked into one atlas, `atlas` under `assets/` (`color.jpg`, with a flat `normal.jpg` and a plain `arm.jpg`), which the kit names as its `Piece` material; each corner keeps its place in it. This is the path the developers asked for in D251; the numbered kinds above are the script-made pieces it replaces.

`pieces.py` writes the game's file, `assets/data/kits/pieces/<kit>.ron` (`terra_build::dress::Pieces`), which the kit names (`dress: (pieces: Some(…))`) and which is committed with it, as the props' models are. A new look is a source here and a kit file; no code. `cargo xtask kits` checks that every role a kit dresses has its piece and that each keeps to its budget.

## A new material in one command (D290)

```bash
cargo xtask pack river_cobble --from polyhaven:river_small_rocks --title "River cobble" --pattern block
cargo xtask pack cut_sandstone --from ambientcg:Bricks076C --metres 2 --pattern block
cargo xtask pack plastered_timber --from image:art/textures/plaster.png --pattern frame
cargo xtask pack oak_shingles --from model:art/kits/shingles/patch/model.glb --metres 1.5 --pattern roof
```

`pack.ts` makes a construction material from a source: a CC0 scan (`polyhaven:`, `ambientcg:`, through `tools/textures`), a generated picture (`image:`), a hand-made reference model baked from above onto a square of the material (`model:`, `bake.py` in Blender: colour, normal, occlusion and roughness, height), or a variant of a texture set already made (`pack:`, its colour multiplied by `--tint`). It fits every map to the texture budget (1,024 texels), checks the set (its maps there, its colour not black), credits it in `assets/materials/README.md`, writes its pack (`assets/data/packs/<name>.ron`: its texture sets for its pattern's slots, its size in metres, its parallax depth, its mean colours for far off) and its material file (`assets/data/kits/<name>.ron`: the next free id, its pattern, pack, cost, weight and roof, as its pattern's defaults unless told), and runs the kit checker. The hammer's picker and the kit sheet read every material file, so nothing else is needed; tune it by editing either file while the game runs. `node --test tools/kits/pack.test.ts` tests it.
