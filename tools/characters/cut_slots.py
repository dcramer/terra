"""The bare slot pieces for D600's trial, cut from a copy of Quaternius's full body: a torso, legs
and feet, each an exact cut (every triangle, vertex, normal, UV and weight theirs, nothing
moved or added), as their own UpperBody is a cut of their FullBody. With their UpperBody (head,
shoulders, arms and hands) the four make the whole body again, triangle for triangle. Their
files stay as shipped; the cuts are separate outputs, and `intact.ts` names each as a cut.

    tools/blender/run.sh tools/characters/cut_slots.py [male|female]

Their pack ships an UpperBody for the man alone. Where a body has none (the woman), its arms
are cut here too: the faces mostly weighted to the arm's bones from the upper arm down, an
exact cut as the others are, written as quaternius-SEX-bare-arms.glb; her head is their
OnlyHead, and her neck and collar stay with the torso.

Writes tools/characters/out/quaternius-SEX-bare-{torso,legs,feet}.glb, and
quaternius-SEX-patched.glb: their full body whole, with its skin patches (patches.py) in a
third UV set (TEXCOORD_2: their body has two already, so the game, which reads the patch from
the second, would need it moved; D600), as the game would hide it under worn pieces. Its
patches are made within each slot's region, the faces the slot's bare piece holds (the arms:
their UpperBody less their OnlyHead), so the hidden way hides a worn slot's region whole.

A face goes to a slot by its centre's height, so a cut runs along their edges: the faces of
their UpperBody are left to it; below, feet under FEET_M, legs under WAIST_M, the torso above.
The heights sit where the man's Ranger pieces both cover (measured on their rims): the legs'
hem at 0.42-0.43 m and the boots' top at 0.56 m, so a bare foot reaches up under the legs and a
bare shin down into the boots; the tunic's hem from 0.92 m and the legs' top at 1.05 m. The
woman's pieces cover the same heights (her legs' hems 0.41-0.44 m, her shoes' tops from 0.46 m,
her tunics' hems to 0.99 m, her legs' tops from 1.06 m).
"""

import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import bmesh
import bpy
from mathutils.kdtree import KDTree

import look
import patches

FEET_M = 0.44
WAIST_M = 1.03
SEX = (look.args() or ["male"])[0].capitalize()
UBC = os.path.join(look.ART, "characters", "quaternius", "universal-base-characters")
FULL = os.path.join(UBC, f"Regular_{SEX}_FullBody.gltf")
UPPER = os.path.join(UBC, f"Regular_{SEX}_UpperBody.gltf")
ONLY_HEAD = os.path.join(UBC, f"Regular_{SEX}_OnlyHead.gltf")
OUT = look.OUT


def load(path):
    look.reset()
    bpy.ops.import_scene.gltf(filepath=path)
    arm = next(o for o in bpy.context.scene.objects if o.type == "ARMATURE")
    body = max((o for o in bpy.context.scene.objects if o.type == "MESH"), key=lambda o: len(o.data.polygons))
    return arm, body


def export(path, objects):
    for o in bpy.context.scene.objects:
        o.select_set(o in objects)
    bpy.ops.export_scene.gltf(filepath=path, export_format="GLB", use_selection=True, export_animations=False,
                              export_all_influences=True, export_extras=True, export_morph=False)
    print("WROTE", os.path.relpath(path, look.ROOT))


def faces(path):
    """A file's skin faces by centre, so the cuts leave them to it."""
    _, skin = load(path)
    cs = [skin.matrix_world @ p.center for p in skin.data.polygons]
    tree = KDTree(len(cs))
    for i, c in enumerate(cs):
        tree.insert(c, i)
    tree.balance()
    return tree, len(cs)


# The bones an arm's faces are weighted to, from the upper arm down.
ARM_BONES = ("upperarm_", "lowerarm_", "hand_", "index_", "middle_", "pinky_", "ring_", "thumb_")


def arm_faces(body):
    """Which of a body's faces are an arm's: over half their weight on the arm's bones."""
    names = {g.index: g.name for g in body.vertex_groups}
    share = []
    for v in body.data.vertices:
        total = sum(g.weight for g in v.groups) or 1.0
        share.append(sum(g.weight for g in v.groups if names[g.group].startswith(ARM_BONES)) / total)
    return [sum(share[i] for i in p.vertices) / len(p.vertices) > 0.5 for p in body.data.polygons]


HAS_UPPER = os.path.exists(UPPER)
tree, n_upper = faces(UPPER) if HAS_UPPER else (None, 0)
head_tree, n_head = faces(ONLY_HEAD)

arm, body = load(FULL)
centres = [body.matrix_world @ p.center for p in body.data.polygons]
slot = []
head = []
arms = None if HAS_UPPER else arm_faces(body)
for i, c in enumerate(centres):
    head.append(head_tree.find(c)[2] < 1e-5)
    if (tree.find(c)[2] < 1e-5) if HAS_UPPER else (head[i] or arms[i]):
        slot.append("upper")
    else:
        slot.append("feet" if c.z < FEET_M else "legs" if c.z < WAIST_M else "torso")
counts = {s: slot.count(s) for s in ("upper", "torso", "legs", "feet")}
print("SLOTS", ", ".join(f"{s} {n}" for s, n in counts.items()), f"of {len(slot)} faces; UpperBody has {n_upper if HAS_UPPER else 'no file'}, OnlyHead {n_head}")
if HAS_UPPER and counts["upper"] != n_upper:
    raise SystemExit(f"cut_slots: {counts['upper']} of the full body's faces match their UpperBody's {n_upper}; the two files no longer agree, so the cuts would not make the body whole again")
if sum(head) != n_head or any(h and s != "upper" for h, s in zip(head, slot)):
    raise SystemExit(f"cut_slots: {sum(head)} of the full body's faces match their OnlyHead's {n_head}, or some lie outside UpperBody; the head is no longer a cut of the body, so the arms slot's region (UpperBody less OnlyHead) is not defined")

# The whole body with its patches, for the hidden way; the eyes and brows go with it. Each
# patch lies within one slot's region (the head, the arms: UpperBody less OnlyHead, the torso,
# the legs, the feet), so a worn piece hides the region it stands in for, whole (D600).
region = ["head" if h else "arms" if s == "upper" else s for h, s in zip(head, slot)]
patches.mark(body, group=[("head", "arms", "torso", "legs", "feet").index(r) for r in region])
export(os.path.join(OUT, f"quaternius-{SEX.lower()}-patched.glb"), [arm, *[o for o in bpy.context.scene.objects if o.type == "MESH"]])
body.data.uv_layers.remove(body.data.uv_layers[patches.LAYER])

# Each cut is the faces of its region; a body with no UpperBody of theirs has its arms cut too.
for name in ("torso", "legs", "feet") + (() if HAS_UPPER else ("arms",)):
    arm, body = load(FULL)
    for o in [o for o in bpy.context.scene.objects if o.type == "MESH" and o is not body]:
        bpy.data.objects.remove(o)
    # Their normals, corner by corner, kept: Blender holds custom normals relative to each
    # corner's fan of faces, so taking faces away would turn the border's normals (up to 22
    # degrees at the ankle) unless they are set again as they were.
    me = body.data
    kept = [p.index for p in me.polygons if region[p.index] == name]
    normals = [tuple(me.corner_normals[li].vector) for i in kept for li in me.polygons[i].loop_indices]
    bm = bmesh.new()
    bm.from_mesh(me)
    bm.faces.ensure_lookup_table()
    bmesh.ops.delete(bm, geom=[f for f in bm.faces if region[f.index] != name], context="FACES_ONLY")
    bmesh.ops.delete(bm, geom=[v for v in bm.verts if not v.link_faces], context="VERTS")
    bm.to_mesh(me)
    bm.free()
    if len(me.loops) != len(normals):
        raise SystemExit(f"cut_slots: the {name} cut has {len(me.loops)} corners, {len(normals)} expected")
    me.normals_split_custom_set(normals)
    body.name = body.data.name = f"Regular{SEX}_Bare_{name.capitalize()}"
    export(os.path.join(OUT, f"quaternius-{SEX.lower()}-bare-{name}.glb"), [arm, body])
