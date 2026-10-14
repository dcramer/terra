"""Where a modelled tree's leaves go: the clumps along its outer branches, for a crown of
leaf cards to be grown on a modelled trunk (D235). A modelled trunk carries no leaves, so
the game needs to know where its branches run out: this finds them by how far each point
of the mesh is from the foot along its surface, as the sap would go. Points far along are
the outer branches; they are gathered into clumps about a metre apart, each with the way
its branch runs (from nearer the foot to farther), and grouped into masses by which way
round the trunk and how high they are.

    tools/blender/run.sh tools/blender/tips.py MODEL.glb OUT.ron [--from 0.5] [--clump 1.0]

Writes a RON list the game reads (`assets/data/trees/*.ron`): each clump's place and
direction in the model's own metres, glTF's axes (y up), its size and its mass.
"""

import heapq
import math
import sys

import bmesh
import bpy


def main():
    args = sys.argv[sys.argv.index("--") + 1:]
    src, out = args[0], args[1]
    share = float(args[args.index("--from") + 1]) if "--from" in args else 0.5
    clump = float(args[args.index("--clump") + 1]) if "--clump" in args else 1.0

    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.import_scene.gltf(filepath=src)
    meshes = [ob for ob in bpy.context.scene.objects if ob.type == "MESH"]
    bm = bmesh.new()
    for ob in meshes:
        part = bmesh.new()
        part.from_mesh(ob.data)
        part.transform(ob.matrix_world)
        data = bpy.data.meshes.new("part")
        part.to_mesh(data)
        bm.from_mesh(data)
        part.free()
    bmesh.ops.remove_doubles(bm, verts=bm.verts, dist=0.01)
    bm.verts.ensure_lookup_table()
    verts = bm.verts
    height = max(v.co.z for v in verts)

    # How far each point is from the foot along the surface.
    foot = min(verts, key=lambda v: v.co.z + 0.2 * math.hypot(v.co.x, v.co.y))
    far = {foot.index: 0.0}
    parent = {}
    queue = [(0.0, foot.index)]
    while queue:
        d, i = heapq.heappop(queue)
        if d > far.get(i, math.inf):
            continue
        for edge in verts[i].link_edges:
            j = edge.other_vert(verts[i]).index
            nd = d + edge.calc_length()
            if nd < far.get(j, math.inf):
                far[j] = nd
                parent[j] = i
                heapq.heappush(queue, (nd, j))
    longest = max(far.values())

    # The outer branches, gathered into cells a clump across.
    cells = {}
    for i, d in far.items():
        v = verts[i]
        if d < share * longest or v.co.z < 0.25 * height or i not in parent:
            continue
        key = tuple(math.floor(c / clump) for c in v.co)
        p = verts[parent[i]].co
        cell = cells.setdefault(key, [0, [0.0] * 3, [0.0] * 3, 0.0])
        cell[0] += 1
        for k in range(3):
            cell[1][k] += v.co[k]
            cell[2][k] += v.co[k] - p[k]
        cell[3] = max(cell[3], d)

    tips = []
    for count, at, along, d in cells.values():
        if count < 3:
            continue
        at = [c / count for c in at]
        length = math.sqrt(sum(c * c for c in along)) or 1.0
        along = [c / length for c in along]
        # Farther along, the clump is larger: the branch's ends carry the most leaves.
        size = clump * (0.9 + 0.5 * (d / longest - share) / (1.0 - share))
        turn = math.atan2(at[1], at[0])
        sector = int((turn + math.pi) / (2 * math.pi) * 6) % 6
        band = min(int(at[2] / height * 2.0), 1)
        # glTF's axes: x, z up, -y.
        tips.append((at[0], at[2], -at[1], along[0], along[2], -along[1], size, sector * 2 + band))

    with open(out, "w") as f:
        f.write("// Where a modelled tree's leaf clumps go, from tools/blender/tips.py; don't edit by hand.\n")
        f.write("// Each: place (x, y up, z), the way its branch runs, its size, its mass.\n[\n")
        for t in tips:
            f.write("    ((%.3f, %.3f, %.3f), (%.3f, %.3f, %.3f), %.2f, %d),\n" % t)
        f.write("]\n")
    print("TIPS %d clumps from %d points, the longest path %.1f m, %.1f m tall" % (len(tips), len(far), longest, height))


if __name__ == "__main__":
    main()
