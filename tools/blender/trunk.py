"""Makes a modelled trunk ready to be drawn in the game's own bark, as grown trunks are
(D235, D264): the game lays its bark scans over it by where each point is (triplanar), so
the model's own texture is dropped, and what it needs instead is a smooth surface and how
much of the sky each point sees.

    tools/blender/run.sh tools/blender/trunk.py MODEL.glb NEAR.glb FAR.glb [--near 20000] [--far 5000]

MODEL is the trunk as the props pipeline left it (tools/props, its foot at the origin).
NEAR is it subdivided once, so no limb's outline shows facets, then cut to `--near`
triangles; FAR is it cut to `--far`, for beyond the first level of detail. Both are smooth
shaded everywhere, carry tangents (which the game's bark needs, whatever their direction),
and a colour per vertex: how open it is to the sky round it, from ambient occlusion
cast against the model itself, in its colour (crevices and forks darker), and in its alpha that
times how high it stands, as the grown trunks' vertices carry (the foot damp and dark, the
crown open). No textures or materials are written.
"""

import math
import os
import sys

import bpy
from mathutils.bvhtree import BVHTree

# How far round a point its occlusion is looked for, in metres: its forks, burrs and furrows,
# not the whole crown; and how many rays look.
REACH = 1.2
RAYS = 32


def option(args, name, fallback):
    return int(args[args.index(name) + 1]) if name in args else fallback


def triangles(ob):
    return sum(len(p.vertices) - 2 for p in ob.data.polygons)


def smoothstep(a, b, x):
    t = min(max((x - a) / (b - a), 0.0), 1.0)
    return t * t * (3.0 - 2.0 * t)


def only(ob):
    bpy.ops.object.select_all(action="DESELECT")
    ob.select_set(True)
    bpy.context.view_layer.objects.active = ob


def cut(ob, budget):
    for _ in range(6):
        now = triangles(ob)
        if now <= budget * 1.05:
            break
        mod = ob.modifiers.new("cut", "DECIMATE")
        mod.decimate_type = "COLLAPSE"
        mod.ratio = budget / now
        mod.use_collapse_triangulate = True
        only(ob)
        print("cut", bpy.ops.object.modifier_apply(modifier=mod.name))


def sky(ob, height):
    """Measures how open each vertex is to the sky round it, by rays cast against the model
    itself within `REACH`, then writes the game's colour and sky from it."""
    tree = BVHTree.FromObject(ob, bpy.context.evaluated_depsgraph_get())
    # Rays spread over the half of the sky a point faces, more toward its normal (cosine
    # weighted), the same for every point so the result is the same every run.
    golden = math.pi * (3.0 - math.sqrt(5.0))
    rays = []
    for k in range(RAYS):
        r = math.sqrt((k + 0.5) / RAYS)
        a = k * golden
        rays.append((r * math.cos(a), r * math.sin(a), math.sqrt(max(0.0, 1.0 - r * r))))
    open_ = []
    for v in ob.data.vertices:
        n = v.normal.normalized()
        t = n.orthogonal().normalized()
        b = n.cross(t)
        start = v.co + n * 0.005
        free = 0
        for x, y, z in rays:
            hit = tree.ray_cast(start, t * x + b * y + n * z, REACH)
            if hit[0] is None:
                free += 1
        open_.append(free / RAYS)
    colour = ob.data.color_attributes.new("Col", "FLOAT_COLOR", "POINT")
    ob.data.color_attributes.active_color = colour
    lowest = min(v.co.z for v in ob.data.vertices)
    for i, v in enumerate(ob.data.vertices):
        ao = open_[i]
        y = v.co.z - lowest
        # As lush.rs's tubes: least at the foot, more up the bole, most in the crown.
        seen = 0.3 + 0.2 * smoothstep(0.0, 1.4, y) + 0.5 * smoothstep(0.0, 0.8, y / height)
        shade = 0.7 + 0.3 * ao
        colour.data[i].color = (shade, shade, shade, seen * (0.55 + 0.45 * ao))
    mean = sum(open_) / max(len(open_), 1)
    dark = sum(1 for a in open_ if a < 0.5) / max(len(open_), 1)
    print(f"TRUNK sky: {mean:.2f} open on average, {dark:.0%} of points under half")


def export(ob, path):
    only(ob)
    os.makedirs(os.path.dirname(os.path.abspath(path)), exist_ok=True)
    print(
        "export",
        bpy.ops.export_scene.gltf(
            filepath=os.path.abspath(path),
            export_format="GLB",
            use_selection=True,
            export_yup=True,
            export_apply=True,
            export_tangents=True,
            export_vertex_color="ACTIVE",
            export_materials="NONE",
        ),
    )
    print(f"TRUNK {path}: {triangles(ob)} triangles")


def main():
    args = sys.argv[sys.argv.index("--") + 1 :]
    src, near_out, far_out = args[0], args[1], args[2]
    near_budget = option(args, "--near", 20000)
    far_budget = option(args, "--far", 5000)

    bpy.ops.wm.read_factory_settings(use_empty=True)
    print("import", bpy.ops.import_scene.gltf(filepath=os.path.abspath(src)))
    meshes = [ob for ob in bpy.context.scene.objects if ob.type == "MESH"]
    for ob in meshes:
        ob.select_set(True)
    bpy.context.view_layer.objects.active = meshes[0]
    bpy.ops.object.parent_clear(type="CLEAR_KEEP_TRANSFORM")
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    if len(meshes) > 1:
        bpy.ops.object.join()
    trunk = bpy.context.view_layer.objects.active
    for name in [a.name for a in trunk.data.color_attributes]:
        trunk.data.color_attributes.remove(trunk.data.color_attributes[name])
    only(trunk)
    bpy.ops.object.mode_set(mode="EDIT")
    bpy.ops.mesh.select_all(action="SELECT")
    print("weld", bpy.ops.mesh.remove_doubles(threshold=0.0001))
    # Tripo's faces do not all face out (its material is drawn from both sides), which lit
    # parts of the trunk from inside, black.
    print("outward", bpy.ops.mesh.normals_make_consistent(inside=False))
    bpy.ops.object.mode_set(mode="OBJECT")
    height = trunk.dimensions.z
    before = triangles(trunk)

    far = trunk.copy()
    far.data = trunk.data.copy()
    bpy.context.scene.collection.objects.link(far)
    cut(far, far_budget)

    # Once subdivided, its outline is round, then cut back to what it may cost.
    sub = trunk.modifiers.new("round", "SUBSURF")
    sub.levels = 1
    sub.render_levels = 1
    only(trunk)
    print("round", bpy.ops.object.modifier_apply(modifier=sub.name))
    cut(trunk, near_budget)

    for ob in (trunk, far):
        for poly in ob.data.polygons:
            poly.use_smooth = True
        if hasattr(ob.data, "set_sharp_from_angle"):
            ob.data.set_sharp_from_angle(angle=math.pi)
        sky(ob, height)
    print(f"TRUNK {src}: {before} triangles, {height:.1f} m tall")
    export(trunk, near_out)
    export(far, far_out)


main()
