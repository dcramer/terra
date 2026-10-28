"""A code-made haft's skin (D565 step 4): the picture its turned handle is drawn in, so a handle is
real wood, not a flat colour. The handle stays code's (types.json, D566); only its look is made.
Run from the repository root:

    tools/blender/run.sh tools/items/haft.py blank OUT.glb
        the straight rod, as code turns it, for `tripo import` and `tripo texture --image` from
        the haft's cut-out (art/parts/<kind>/<haft>/model.glb is what Tripo paints)
    tools/blender/run.sh tools/items/haft.py skin art/parts/<kind>/<haft> assets/models/parts/<haft>_skin.glb
        Tripo's painted rod fitted back onto the rod and its colour, normal and roughness baked
        onto the turned handle's own UVs (lathe.py's rod: u round, v butt to top), one material
        named Wood; terra_bake's item draws the handle in it (the part's `skin`)

The rod is the reference haft's: 0.68 m, 35 mm across; a shorter or longer haft stretches the
picture along it by its length (a tenth either way, the kind's envelope).
"""

import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "blender"))
sys.path.insert(0, HERE)

import bpy  # noqa: E402
from mathutils import Matrix  # noqa: E402

import lathe  # noqa: E402
import pieces  # noqa: E402

LENGTH, DIAMETER = 0.68, 0.035
SIDES = 16  # crates/terra_bake/src/item.rs SIDES
SKIN_PX = (256, 1024)  # round by along: the grain runs along


def export(ob, path):
    pieces.only(ob)
    os.makedirs(os.path.dirname(os.path.abspath(path)), exist_ok=True)
    bpy.ops.export_scene.gltf(filepath=os.path.abspath(path), export_format="GLB", use_selection=True, export_tangents=True, export_image_format="AUTO")
    print(f"haft: {path}")


def fitted(path):
    """Tripo's painted rod, back on the line: its longest axis up, its middle on the line, from 0 to
    LENGTH, as wide as the rod."""
    bpy.ops.import_scene.gltf(filepath=path)
    meshes = [o for o in bpy.context.scene.objects if o.type == "MESH"]
    for o in meshes:
        o.data.transform(o.matrix_world)
        o.parent = None
        o.matrix_world = Matrix.Identity(4)
    ob = meshes[0]
    vs = [v.co for v in ob.data.vertices]
    span = [max(v[k] for v in vs) - min(v[k] for v in vs) for k in range(3)]
    axis = span.index(max(span))
    if axis != 2:
        ob.data.transform(Matrix.Rotation(math.pi / 2, 4, "X" if axis == 1 else "Y"))
    vs = [v.co for v in ob.data.vertices]
    lo = [min(v[k] for v in vs) for k in range(3)]
    hi = [max(v[k] for v in vs) for k in range(3)]
    ob.data.transform(Matrix.Translation((-(lo[0] + hi[0]) / 2, -(lo[1] + hi[1]) / 2, -lo[2])))
    across = ((hi[0] - lo[0]) + (hi[1] - lo[1])) / 2
    ob.data.transform(Matrix.Diagonal((DIAMETER / across, DIAMETER / across, LENGTH / (hi[2] - lo[2]), 1.0)))
    print(f"haft: Tripo's rod {span[axis]:.3f} long, {across:.3f} across, fitted to {LENGTH} x {DIAMETER} m")
    return ob


def main():
    args = sys.argv[sys.argv.index("--") + 1 :]
    bpy.ops.wm.read_factory_settings(use_empty=True)
    if args[0] == "blank":
        export(lathe.rod("haft", LENGTH, DIAMETER, 64), args[1])
        return
    folder, out = args[1], args[2]
    src = fitted(os.path.join(folder, "model.glb"))
    skin = lathe.rod(os.path.basename(folder.rstrip("/")) + "_skin", LENGTH, DIAMETER, SIDES)
    lathe.bake(src, skin, "Wood", SKIN_PX, 0.0, 0.004)
    bpy.data.objects.remove(src)
    export(skin, out)


main()
