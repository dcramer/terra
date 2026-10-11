"""The wolf modelled by the agent against its concept image
(out/tripo/wolf-concept-generated_image_url.png): a stick skeleton skinned and faceted,
proportions read from the concept, then rendered from the concept's camera beside it.

    run.sh image/wolf.py [round]
"""

import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path[:0] = [os.path.join(HERE, "..", "code"), os.path.join(HERE, "..")]

import bpy
from mathutils import Vector

import look
from parts import box, paint, prism, skinned

look.reset()
J = {}
pts, radii, edges = [], [], []


def joint(name, at, r, parent=None):
    J[name] = len(pts)
    pts.append(at)
    radii.append(r if isinstance(r, tuple) else (r, r))
    if parent is not None:
        edges.append((J[parent], J[name]))


# Spine, x forward, z up, metres.
joint("hip", (0.0, 0, 0.66), (0.15, 0.14))
joint("waist", (0.30, 0, 0.66), (0.13, 0.12), "hip")
joint("chest", (0.60, 0, 0.64), (0.30, 0.20), "waist")
joint("neck", (0.80, 0, 0.90), (0.32, 0.26), "chest")
joint("head", (0.94, 0, 1.08), (0.19, 0.17), "neck")
joint("snout", (1.13, 0, 1.03), (0.085, 0.075), "head")
joint("nose", (1.24, 0, 1.01), (0.045, 0.04), "snout")
# Legs: front from the chest, back from the hip, with the dog's backward hock.
for side, y in (("l", 0.11), ("r", -0.11)):
    joint(f"shoulder{side}", (0.64, y, 0.56), 0.09, "chest")
    joint(f"elbow{side}", (0.62, y * 1.1, 0.36), 0.055, f"shoulder{side}")
    joint(f"wrist{side}", (0.63, y * 1.1, 0.09), 0.04, f"elbow{side}")
    joint(f"paw{side}", (0.71, y * 1.1, 0.035), 0.055, f"wrist{side}")
    joint(f"thigh{side}", (0.04, y, 0.52), 0.13, "hip")
    joint(f"knee{side}", (0.12, y * 1.15, 0.34), 0.08, f"thigh{side}")
    joint(f"hock{side}", (-0.05, y * 1.15, 0.17), 0.05, f"knee{side}")
    joint(f"hpaw{side}", (-0.02, y * 1.15, 0.035), 0.05, f"hock{side}")
    joint(f"htoe{side}", (0.07, y * 1.15, 0.03), 0.045, f"hpaw{side}")
# A full tail, carried low.
joint("tail0", (-0.12, 0, 0.66), 0.05, "hip")
joint("tail1", (-0.26, 0, 0.50), 0.15, "tail0")
joint("tail2", (-0.38, 0, 0.30), 0.14, "tail1")
joint("tail3", (-0.44, 0, 0.13), 0.04, "tail2")

body = skinned("WolfBody", pts, edges, radii, "FurGrey", smooth=1, decimate=0.45)


def coat(c, n):
    if c.x > 1.21:
        return "Black"
    if n.z < -0.35 and c.z < 0.78 or (c.x > 1.0 and n.z < -0.2):
        return "FurLight"
    if c.z < 0.12:
        return "FurDark"
    if n.z > 0.55 and c.z > 0.66 or c.x < -0.5:
        return "FurDark"
    return None


paint(body, coat)
parts = [body]
for y in (0.06, -0.06):
    parts.append(prism("Ear", 0.08, 0.19, "FurDark", sides=4, at=(0.88, y * 1.6, 1.20), rot=(y * 200, -12, 45), top=0.004))
    parts.append(box("Eye", (0.03, 0.02, 0.022), "Black", at=(1.07, y * 1.45, 1.12)))
wolf = look.join(parts, "Wolf")
look.ground_to_origin(wolf)
print("REPORT", look.checks(wolf, 3000))
look.export(wolf, os.path.join(look.OUT, "image-Wolf.glb"))
round_name = (look.args() + ["1"])[0]
look.stage()
bpy.data.objects["Ground"].hide_render = True
scene = bpy.context.scene
scene.render.film_transparent = True
scene.render.resolution_x = scene.render.resolution_y = 1024
scene.view_settings.view_transform = "Standard"
cam = bpy.data.objects.new("Cam", bpy.data.cameras.new("Cam"))
cam.data.lens = 50
scene.collection.objects.link(cam)
scene.camera = cam
# The concept looks at the wolf's right side from ahead of its shoulder, a little above.
target = Vector((0.0, 0.0, 0.55))
cam.location = target + Vector((2.6, -1.9, 0.8))
cam.rotation_euler = (target - cam.location).to_track_quat("-Z", "Y").to_euler()
sun = bpy.data.objects["Sun"]
sun.rotation_euler = Vector((-0.4, 0.8, -0.6)).to_track_quat("-Z", "Y").to_euler()
sun.data.color, sun.data.energy = (1.0, 0.97, 0.92), 2.4
scene.world.node_tree.nodes["Background"].inputs["Color"].default_value = (0.75, 0.75, 0.78, 1)
scene.world.node_tree.nodes["Background"].inputs["Strength"].default_value = 0.8
scene.render.filepath = os.path.join(look.OUT, f"image-wolf-match-{round_name}.png")
bpy.ops.render.render(write_still=True)
