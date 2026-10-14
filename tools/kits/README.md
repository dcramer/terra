# Kits

Makes a building kit's dressed pieces (D215, [docs/building-kits.md](../../docs/building-kits.md)): the sections a layout sweeps along each exposed edge by its role (a coping, a plinth, a sill, a lintel) and the corner stones laid up an edge a course at a time (quoins, jamb stones), from their source under `art/kits/<kit>/`, in Blender.

```bash
tools/blender/run.sh tools/kits/pieces.py art/kits/stone/pieces.json assets/data/kits/pieces/stone.ron
```

The source, `pieces.json`, says each piece in numbers a person can read and change:

- **Profiles**: a polyline of points (out, up) in metres from the edge it is swept along, out from the cube's face and up from the edge's line, nothing past an eighth of a cube out (D192); `round` names the points Blender's vertex bevel rounds, by `radius`. The two ends of the line lie inside the wall: the layout closes and mitres them.
- **Corner sets**: stones at an edge's corner, their two faces standing `proud` of the wall's, a course `height` less their `joint`; `long` gives each variant's length along its first face and its second, and `short`, if any, the variants laid by turns with them (jamb stones, long and short along the wall); a set with no `short` turns its stones each course instead (quoins). Each stone's faces are divided about every `cell`, pillowed to their middle by `pillow` and pitched by the chisel by `pitch`, their arrises bevelled by `bevel`, shaded hard at creases and smooth across curves with face-weighted normals.

- **Modules** (D258): a length of mesh along +X, `length` long, Y up from what it sits on and Z across: a section of a `kind` swept along it and shaped as it goes. `timber` is squared timber or a board between `y` and `z`, its corners rounded by `round` and wandering, its faces scalloped by the `adze` every `period`; `pole` a round pole of `radius`; `mound` a capping crowned across `half` either side and `height` over `base`, its edge cut in `points` every `period`; `bundle` an oval about `y`, `z` of `radii`, `pinch`ed every `period`. `rods` lays thin rods along it (a ridge's liggers); `parts` joins several sections in one module; `joint` presses a gap at each end, as between ridge stones; `open` leaves its ends open so copies run on as one member. `cap` (`flat`, or a `dome` drawn in over `reach` to a `pivot`) closes a line where it stops; `blend` is how far either side of a crease a draped module turns from one slope to the other. Each of its `variants` is written near and at a far budget (`far`, a fraction).

`pieces.py` writes the game's file, `assets/data/kits/pieces/<kit>.ron` (`terra_build::dress::Pieces`), which the kit names (`dress: (pieces: Some(…))`) and which is committed with it, as the props' models are. A new look is a source here and a kit file; no code. `cargo xtask kits` checks that every role a kit dresses has its piece and that each keeps to its budget.
