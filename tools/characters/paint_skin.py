"""A template's own skin, painted by Tripo on the base body's mesh in the template's shape and
baked onto the base's texture layout (D112; research/body-morphs.md).

    tools/blender/run.sh tools/characters/paint_skin.py out NAME
    tools/blender/run.sh tools/characters/paint_skin.py in NAME

`out` writes tools/characters/out/NAME-bare.glb: the body of out/NAME-game.glb, bare, facing +X
as Tripo's models do. Tripo then paints it from the design's front view (paint_skin.sh does
both). `in` bakes out/NAME-painted.glb, which Tripo returns a unit tall and centred, back onto
the body's own layout and writes out/NAME-game.glb with it.
"""

import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import bpy
from mathutils import Matrix, Vector

import look

OUT = look.OUT
step, name = look.args()[:2]
look.reset()


def load(path):
    before = set(bpy.context.scene.objects)
    bpy.ops.import_scene.gltf(filepath=path)
    return [o for o in bpy.context.scene.objects if o not in before]


def box(objs):
    pts = [o.matrix_world @ v.co for o in objs if o.type == "MESH" for v in o.data.vertices]
    return Vector([min(p[i] for p in pts) for i in range(3)]), Vector([max(p[i] for p in pts) for i in range(3)])


objs = load(os.path.join(OUT, f"{name}-game.glb"))
arm = next(o for o in objs if o.type == "ARMATURE")
if arm.animation_data:
    arm.animation_data.action = None
for pb in arm.pose.bones:
    pb.matrix_basis.identity()
bpy.context.view_layer.update()
body = max((o for o in objs if o.type == "MESH"), key=lambda o: len(o.data.vertices))

if step == "out":
    bare = bpy.data.objects.new(f"{name}-bare", body.data.copy())
    bpy.context.scene.collection.objects.link(bare)
    bare.matrix_world = Matrix.Rotation(math.pi / 2, 4, "Z") @ body.matrix_world
    bare.data.materials.clear()
    while len(bare.data.uv_layers) > 1:
        bare.data.uv_layers.remove(bare.data.uv_layers[-1])
    bpy.ops.object.select_all(action="DESELECT")
    bare.select_set(True)
    bpy.ops.export_scene.gltf(filepath=os.path.join(OUT, f"{name}-bare.glb"), export_format="GLB", use_selection=True)
    print("BARE", name)
else:
    painted = [o for o in load(os.path.join(OUT, f"{name}-painted.glb")) if o.type == "MESH"]
    lo, hi = box([body])
    turn = Matrix.Rotation(-math.pi / 2, 4, "Z")
    for o in painted:
        o.matrix_world = turn @ o.matrix_world
    bpy.context.view_layer.update()
    plo, phi = box(painted)
    k = (hi.z - lo.z) / (phi.z - plo.z)
    fit = Matrix.Translation((lo + hi) / 2) @ Matrix.Scale(k, 4) @ Matrix.Translation(-(plo + phi) / 2)
    for o in painted:
        o.matrix_world = fit @ o.matrix_world
    mat = body.data.materials[0].copy()
    body.data.materials[0] = mat
    tex = next(n for n in mat.node_tree.nodes if n.type == "TEX_IMAGE")
    size = tex.image.size[0] if tex.image else 2048
    baked = bpy.data.images.new(f"{name}_skin", size, size)
    tex.image = baked
    mat.node_tree.nodes.active = tex
    body.data.uv_layers.active_index = 0
    scene = bpy.context.scene
    scene.render.engine = "CYCLES"
    scene.cycles.device = "CPU"
    scene.cycles.samples = 1
    scene.render.bake.use_pass_direct = False
    scene.render.bake.use_pass_indirect = False
    scene.render.bake.use_selected_to_active = True
    scene.render.bake.cage_extrusion = 0.01
    scene.render.bake.max_ray_distance = 0.02
    scene.render.bake.margin = 16
    mods = [(m.name, m.object) for m in body.modifiers if m.type == "ARMATURE"]
    body.modifiers.clear()
    bpy.ops.object.select_all(action="DESELECT")
    for o in painted:
        o.select_set(True)
    body.select_set(True)
    bpy.context.view_layer.objects.active = body
    bpy.ops.object.bake(type="DIFFUSE")
    baked.pack()
    for o in painted:
        bpy.data.objects.remove(o)
    for mname, mobj in mods:
        body.modifiers.new(mname, "ARMATURE").object = mobj
    bpy.ops.object.select_all(action="SELECT")
    bpy.ops.export_scene.gltf(filepath=os.path.join(OUT, f"{name}-game.glb"), export_format="GLB", use_selection=True, export_animations=True, export_extras=True)
    print("PAINTED", name)
