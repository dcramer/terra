"""A kit's dressed pieces made in Blender from their source (D215; tools/kits/README.md).

    tools/blender/run.sh tools/kits/pieces.py art/kits/stone/pieces.json assets/data/kits/pieces/stone.ron

Reads the source, a JSON file under art/kits/<kit>/: its profiles, each a polyline in
(out, up) metres from the edge it is swept along, the corners named rounded by Blender's
vertex bevel; and its corner stones, each a block at an edge's corner, standing out of both
faces, its two faces divided, pillowed and pitched as a mason dresses them, its arrises
bevelled, shaded smooth across curves and hard at creases with face-weighted normals.

Writes the game's file, RON as `terra_build::dress::Pieces` reads it: the profiles as points,
the corner stones as indexed triangles with their normals. Game space: Y up, a stone's
corner at the origin, its faces looking +X and +Z, the wall behind it in -X and -Z, from
y = 0 up a course. Nothing else is written; the file is committed with the kit, as the
props' models are.
"""

import bmesh
import bpy
import json
import math
import os
import random
import sys
import zlib

from mathutils import Vector, noise

args = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []
if len(args) != 2:
    raise SystemExit("usage: tools/blender/run.sh tools/kits/pieces.py <source.json> <out.ron>")
SOURCE, OUT = args


def clear():
    for ob in list(bpy.data.objects):
        bpy.data.objects.remove(ob)
    for mesh in list(bpy.data.meshes):
        bpy.data.meshes.remove(mesh)


def profile(spec):
    """A polyline's corners named in `round` rounded by Blender's vertex bevel; its points
    in order from the first."""
    points = spec["points"]
    bm = bmesh.new()
    verts = [bm.verts.new((o, u, 0.0)) for o, u in points]
    for a, b in zip(verts, verts[1:]):
        bm.edges.new((a, b))
    bm.verts.ensure_lookup_table()
    bmesh.ops.bevel(
        bm,
        geom=[verts[i] for i in spec.get("round", [])],
        offset=spec.get("radius", 0.01),
        segments=2,
        affect="VERTICES",
        profile=0.5,
        clamp_overlap=True,
    )
    ends = [v for v in bm.verts if len(v.link_edges) == 1]
    first = Vector((points[0][0], points[0][1], 0.0))
    start = min(ends, key=lambda v: (v.co - first).length)
    out, prev, here = [], None, start
    while here is not None:
        out.append((round(here.co.x, 4), round(here.co.y, 4)))
        nxt = None
        for e in here.link_edges:
            other = e.other_vert(here)
            if other is not prev:
                nxt = other
        prev, here = here, nxt
    bm.free()
    return out


def stone(name, spec, along_a, along_b, seed):
    """A corner stone: in game space (x, y up, z), from x = -along_a to proud along its +Z
    face, z = -along_b to proud along its +X face, a course high less its joints. Built in
    Blender's frame, Z up: game (x, y, z) is Blender (x, -z, y)."""
    rng = random.Random(seed)
    proud = spec["proud"]
    joint = spec["joint"]
    height = spec["height"] - 2 * joint
    bevel = spec["bevel"]
    cell = spec["cell"]
    # The box in Blender's frame: x from -a to proud, y (game -z) from -proud to b.
    x0, x1 = -along_a + joint, proud
    y0, y1 = -proud, along_b - joint
    z0, z1 = joint, joint + height
    bm = bmesh.new()
    bmesh.ops.create_cube(bm, size=1.0)
    bmesh.ops.scale(bm, vec=(x1 - x0, y1 - y0, z1 - z0), verts=bm.verts)
    bmesh.ops.translate(bm, vec=((x0 + x1) / 2, (y0 + y1) / 2, (z0 + z1) / 2), verts=bm.verts)
    # Its back, in the wall, is never seen.
    back = [f for f in bm.faces if f.normal.x < -0.99 or f.normal.y > 0.99]
    bmesh.ops.delete(bm, geom=back, context="FACES")
    # Its two faces divided, so they can be pillowed and pitched.
    faces = [f for f in bm.faces if f.normal.x > 0.99 or f.normal.y < -0.99]
    longest = max(x1 - x0, y1 - y0, z1 - z0)
    cuts = max(1, int(round(longest / cell)) - 1)
    bmesh.ops.subdivide_edges(
        bm, edges=list({e for f in faces for e in f.edges}), cuts=cuts, use_grid_fill=True
    )
    # Its arrises round its faces bevelled; not where it is cut open at the back.
    edges = []
    for e in bm.edges:
        if len(e.link_faces) != 2:
            continue
        a, b = e.link_faces
        if a.normal.dot(b.normal) > 0.5:
            continue
        edges.append(e)
    bmesh.ops.bevel(
        bm, geom=edges, offset=bevel, segments=2, affect="EDGES", profile=0.6, clamp_overlap=True
    )
    # Pillowed towards each face's middle and pitched by the chisel, never on the bevels.
    for v in bm.verts:
        on_x = v.co.x > proud - 1e-4
        on_y = v.co.y < -proud + 1e-4
        if not (on_x or on_y):
            continue
        if on_x:
            dy, size_a = min(v.co.y - y0, y1 - v.co.y), y1 - y0
        else:
            dy, size_a = min(v.co.x - x0, x1 - v.co.x), x1 - x0
        dz = min(v.co.z - z0, z1 - v.co.z)
        inner = min(dy, dz)
        if inner < bevel * 0.9:
            continue
        pillow = math.sin(min(1.0, dy / (0.5 * size_a)) * math.pi / 2) * math.sin(
            min(1.0, dz / (0.5 * height)) * math.pi / 2
        )
        at = Vector((v.co.x * 11.0 + seed * 0.37, v.co.y * 11.0, v.co.z * 11.0 + seed * 0.71))
        n = noise.noise(at) + 0.5 * noise.noise(at * 2.3 + Vector((3.0, 1.0, 2.0)))
        push = spec["pillow"] * pillow + spec["pitch"] * n * min(1.0, inner / 0.02)
        if on_x:
            v.co.x += push
        else:
            v.co.y -= push
    # A little out of true, as set by hand.
    tilt = rng.uniform(-0.006, 0.006)
    for v in bm.verts:
        if v.co.x > proud - 0.03:
            v.co.x += tilt * (v.co.z - (z0 + z1) / 2)
    bmesh.ops.triangulate(bm, faces=bm.faces)
    mesh = bpy.data.meshes.new(name)
    bm.to_mesh(mesh)
    bm.free()
    ob = bpy.data.objects.new(name, mesh)
    bpy.context.collection.objects.link(ob)
    bpy.context.view_layer.objects.active = ob
    ob.select_set(True)
    # Hard at creases over about 50 degrees, smooth across curves (docs/art-style.md); the
    # big faces keep their own normals and the bevels take the turn, as hard-surface game
    # art is weighted.
    bpy.ops.object.shade_smooth_by_angle(angle=math.radians(50))
    weighted = ob.modifiers.new("Weighted", "WEIGHTED_NORMAL")
    weighted.mode = "FACE_AREA"
    weighted.weight = 100
    weighted.keep_sharp = True
    ob.select_set(False)
    return ob


def triangles(ob):
    """An object's triangles as shared corners in game space: positions, normals, indices."""
    depsgraph = bpy.context.evaluated_depsgraph_get()
    evaluated = ob.evaluated_get(depsgraph)
    mesh = evaluated.to_mesh()
    mesh.calc_loop_triangles()
    normals = mesh.corner_normals
    index, positions, out_normals, indices = {}, [], [], []
    for tri in mesh.loop_triangles:
        for loop in tri.loops:
            p = mesh.vertices[mesh.loops[loop].vertex_index].co
            n = normals[loop].vector
            game_p = (round(p.x, 4), round(p.z, 4), round(-p.y, 4))
            game_n = (round(n.x, 3), round(n.z, 3), round(-n.y, 3))
            key = (game_p, game_n)
            if key not in index:
                index[key] = len(positions)
                positions.append(game_p)
                out_normals.append(game_n)
            indices.append(index[key])
    evaluated.to_mesh_clear()
    return positions, out_normals, indices


def ron_floats(values):
    return "(" + ", ".join(f"{v:g}" for v in values) + ")"


def ron_mesh(positions, normals, indices):
    tris = [indices[i : i + 3] for i in range(0, len(indices), 3)]
    return (
        "(\n                positions: ["
        + ", ".join(ron_floats(p) for p in positions)
        + "],\n                normals: ["
        + ", ".join(ron_floats(n) for n in normals)
        + "],\n                triangles: ["
        + ", ".join("(%d, %d, %d)" % tuple(t) for t in tris)
        + "],\n            )"
    )


def main():
    with open(SOURCE, encoding="utf-8") as f:
        source = json.load(f)
    clear()
    lines = ["// Made by tools/kits/pieces.py from " + SOURCE.replace("\\", "/") + "; do not edit.", "("]
    lines.append("    profiles: {")
    for name, spec in source["profiles"].items():
        points = profile(spec)
        lines.append(f'        "{name}": [' + ", ".join(ron_floats(p) for p in points) + "],")
        print(f"profile {name}: {len(points)} points")
    lines.append("    },")
    lines.append("    corners: {")
    for name, spec in source["corners"].items():
        lines.append(f'        "{name}": (')
        for which in ("long", "short"):
            meshes = []
            for variant, (a, b) in enumerate(spec.get(which, [])):
                seed = zlib.crc32(f"{name}{which}{variant}".encode()) % 1000
                ob = stone(f"{name}_{which}_{variant}", spec, a, b, seed)
                positions, normals, indices = triangles(ob)
                print(f"corner {name} {which} {variant}: {len(indices) // 3} triangles")
                meshes.append(ron_mesh(positions, normals, indices))
            lines.append(f"            {which}: [" + ", ".join(meshes) + "],")
        lines.append("        ),")
    lines.append("    },")
    lines.append(")")
    os.makedirs(os.path.dirname(os.path.abspath(OUT)), exist_ok=True)
    with open(OUT, "w", encoding="utf-8", newline="\n") as f:
        f.write("\n".join(lines) + "\n")
    print("wrote", OUT)


main()
