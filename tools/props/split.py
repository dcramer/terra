"""Cuts a door in its frame into the frame, which stays where it is fitted, and the leaf,
which swings on its hinges (docs/building-kits.md): two meshes, Frame and Leaf, in one
file, so the game turns only the leaf about its hinge. Run by props.ts after refine.py for a
prop whose prop.json gives its leaf.

    tools/blender/run.sh tools/props/split.py MODEL.glb LEFT RIGHT TOP

MODEL is the refined model, written over; LEFT and RIGHT are where the leaf's edges are across
it, TOP how high its top is, in metres, the model standing as refine.py leaves it: its front
to +Z in glTF, its foot on the ground. The mesh is cut along those three planes and every
face inside them is the leaf's; each part's cut is closed with a cap, so the leaf's edges
and the frame's rebates show solid when the door stands open.
"""

import math
import sys

import bmesh
import bpy
from mathutils import Vector

args = sys.argv[sys.argv.index("--") + 1 :]
path, left, right, top = args[0], float(args[1]), float(args[2]), float(args[3])

bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=path)
meshes = [ob for ob in bpy.context.scene.objects if ob.type == "MESH"]
if len(meshes) != 1:
    raise SystemExit(f"{path}: expected one mesh to cut, found {len(meshes)}")
door = meshes[0]
bpy.ops.object.select_all(action="DESELECT")
door.select_set(True)
bpy.context.view_layer.objects.active = door
bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)

bm = bmesh.new()
bm.from_mesh(door.data)
bmesh.ops.remove_doubles(bm, verts=bm.verts, dist=1e-4)
# Blender's own axes: X across the door, Z up.
for point, normal in [
    (Vector((left, 0, 0)), Vector((1, 0, 0))),
    (Vector((right, 0, 0)), Vector((1, 0, 0))),
    (Vector((0, 0, top)), Vector((0, 0, 1))),
]:
    geom = bm.verts[:] + bm.edges[:] + bm.faces[:]
    bmesh.ops.bisect_plane(bm, geom=geom, plane_co=point, plane_no=normal)


def is_leaf(face):
    c = face.calc_center_median()
    return left < c.x < right and c.z < top


def part(name, keep):
    """A copy of the door with only the faces `keep` says, its cut capped."""
    copy = bm.copy()
    gone = [f for f in copy.faces if not keep(f)]
    bmesh.ops.delete(copy, geom=gone, context="FACES")
    edges = [e for e in copy.edges if e.is_boundary]
    capped = bmesh.ops.holes_fill(copy, edges=edges, sides=0)
    me = bpy.data.meshes.new(name)
    copy.to_mesh(me)
    copy.free()
    for material in door.data.materials:
        me.materials.append(material)
    ob = bpy.data.objects.new(name, me)
    bpy.context.scene.collection.objects.link(ob)
    print(f"{name}: {len(me.polygons)} faces, {len(capped['faces'])} capping its cut")
    return ob


leaf = part("Leaf", is_leaf)
frame = part("Frame", lambda f: not is_leaf(f))
bm.free()
# Normals as refine.py leaves them: smooth across a curve, split at a crease (art-style.md).
for ob in (leaf, frame):
    bpy.ops.object.select_all(action="DESELECT")
    ob.select_set(True)
    bpy.context.view_layer.objects.active = ob
    bpy.ops.object.shade_smooth_by_angle(angle=math.radians(50.0), keep_sharp_edges=True)
if not leaf.data.polygons or not frame.data.polygons:
    raise SystemExit("the cut left the leaf or the frame empty: check LEFT, RIGHT and TOP")
bpy.data.objects.remove(door)

bpy.ops.object.select_all(action="DESELECT")
for ob in (frame, leaf):
    ob.select_set(True)
bpy.ops.export_scene.gltf(filepath=path, export_format="GLB", use_selection=True)
print(f"wrote {path}: Frame and Leaf")
