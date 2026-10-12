"""Takes Tripo's painting of a fitted body (fit.py's NAME-paint.glb, painted by `tripo.ts
texture`) back onto the shared mesh's own layout, and writes the rigged body with it.

    tools/blender/run.sh spike/shared_mesh/paint.py base-male

Tripo returns the mesh a unit tall and centred, shaded flat; its colour is baked onto our own
mesh, which keeps its smooth shading, layout and vertex order.
"""

import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "..", "tools", "characters"))

import bpy
from mathutils import Matrix, Vector

import look

OUT = os.path.join(HERE, "out")
name = look.args()[0]
look.reset()


def load(path):
    before = set(bpy.context.scene.objects)
    bpy.ops.import_scene.gltf(filepath=path)
    return [o for o in bpy.context.scene.objects if o not in before]


def box(objs):
    pts = [o.matrix_world @ v.co for o in objs if o.type == "MESH" for v in o.data.vertices]
    lo = Vector([min(p[i] for p in pts) for i in range(3)])
    hi = Vector([max(p[i] for p in pts) for i in range(3)])
    return lo, hi


ours = load(os.path.join(OUT, f"{name}-rigged.glb"))
body = next(o for o in ours if o.type == "MESH")
arm = next(o for o in ours if o.type == "ARMATURE")
painted = load(os.path.join(OUT, f"{name}-painted.glb"))
lo, hi = box([body])
plo, phi = box(painted)
k = (hi.z - lo.z) / (phi.z - plo.z)
centre, pcentre = (lo + hi) / 2, (plo + phi) / 2
fit = Matrix.Translation(centre) @ Matrix.Scale(k, 4) @ Matrix.Translation(-pcentre)
for o in painted:
    if o.parent is None:
        o.matrix_world = fit @ o.matrix_world
bpy.context.view_layer.update()
plo, phi = box(painted)
print("PAINT size ours", tuple(round(x, 3) for x in hi - lo), "theirs", tuple(round(x, 3) for x in phi - plo))

image = body.data.materials[0].node_tree.nodes["Image Texture"].image if body.data.materials else None
mat = body.data.materials[0]
tex = next(n for n in mat.node_tree.nodes if n.type == "TEX_IMAGE")
baked = bpy.data.images.new(f"{name}_skin", 2048, 2048)
tex.image = baked
mat.node_tree.nodes.active = tex
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
body.modifiers.clear()
bpy.ops.object.select_all(action="DESELECT")
for o in painted:
    if o.type == "MESH":
        o.select_set(True)
body.select_set(True)
bpy.context.view_layer.objects.active = body
bpy.ops.object.bake(type="DIFFUSE")
baked.pack()
for o in painted:
    bpy.data.objects.remove(o)
mod = body.modifiers.new("Armature", "ARMATURE")
mod.object = arm
bpy.ops.object.select_all(action="DESELECT")
body.select_set(True)
arm.select_set(True)
path = os.path.join(OUT, f"{name}-rigged.glb")
bpy.ops.export_scene.gltf(filepath=path, export_format="GLB", use_selection=True, export_animations=False)
print("PAINTED", path)
