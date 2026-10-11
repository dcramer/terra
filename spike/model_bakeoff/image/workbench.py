"""The workbench modelled by the agent against its concept image
(out/tripo/workbench-concept-generated_image_url.png), then rendered from the concept's
camera beside it, and from the review angles.

    run.sh image/workbench.py [round]
"""

import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path[:0] = [os.path.join(HERE, "..", "code"), os.path.join(HERE, "..")]

import bpy
from mathutils import Vector

import look
from parts import box, prism

look.PALETTE.update({
    "BenchTop": (0.78, 0.52, 0.30), "BenchLeg": (0.48, 0.31, 0.20), "BenchVice": (0.74, 0.50, 0.28),
    "Crate": (0.68, 0.56, 0.38), "HammerHead": (0.36, 0.38, 0.42), "Handle": (0.52, 0.32, 0.20),
})

look.reset()
p = []
L, D, H = 1.45, 0.70, 1.0          # length, depth, height to the top's surface
top_t = 0.11                        # the top's thickness
z_top = H - top_t

# The top: a narrow front plank, a groove, then the main slab of two planks.
p.append(box("FrontPlank", (L, 0.16, top_t), "BenchTop", at=(0, -D / 2 + 0.08, z_top + top_t / 2), bevel=0.012))
p.append(box("Slab", (L, D - 0.18, top_t), "BenchTop", at=(0, 0.09, z_top + top_t / 2), bevel=0.012))
p.append(box("SlabSeam", (L - 0.6, 0.012, 0.01), "BenchLeg", at=(0.3, 0.12, H + 0.002)))
# Square dog holes along the front and the back.
for x, y in ((-0.56, 0.20), (-0.42, -0.27), (0.50, 0.24), (0.58, 0.02), (0.64, 0.16)):
    p.append(box(f"Hole{x}{y}", (0.07, 0.07, 0.012), "BenchLeg", at=(x, y, H - 0.004)))

# The deep rail under the top's front, set back a little.
p.append(box("FrontRail", (L - 0.25, 0.10, 0.12), "BenchLeg", at=(0.05, -D / 2 + 0.12, z_top - 0.06), bevel=0.01))
p.append(box("BackRail", (L - 0.25, 0.10, 0.16), "BenchLeg", at=(0.05, D / 2 - 0.12, z_top - 0.08), bevel=0.01))

# Legs: chunky, tapering toward the floor, splayed outward along the length.
for sx in (-1, 1):
    for sy in (-1, 1):
        x, y = sx * (L / 2 - 0.22), sy * (D / 2 - 0.12)
        p.append(box(f"Leg{sx}{sy}", (0.16, 0.15, z_top + 0.04), "BenchLeg", at=(x + sx * 0.12, y, z_top / 2 - 0.01),
                     rot=(0, sx * -11, 0), taper=0.85, bevel=0.01))
    # A stretcher joining each end's legs at the shelf.
    p.append(box(f"EndStretcher{sx}", (0.10, D - 0.10, 0.10), "BenchLeg", at=(sx * (L / 2 - 0.12), 0, 0.22)))

# The low shelf: two planks with a seam, on rails front and back.
p.append(box("ShelfFront", (L - 0.20, 0.30, 0.06), "BenchLeg", at=(0.0, -0.15, 0.25), bevel=0.008))
p.append(box("ShelfBack", (L - 0.20, 0.30, 0.06), "BenchLeg", at=(0.0, 0.16, 0.25), bevel=0.008))
p.append(box("ShelfRail", (L - 0.12, 0.08, 0.12), "BenchLeg", at=(0.0, -D / 2 + 0.06, 0.19), bevel=0.008))

# The leg vice at the front left: a tall chop over the leg, layered blocks, a hex hub and a
# turned handle hanging through it.
vx = -0.59
p.append(box("ViceBack", (0.36, 0.08, 0.26), "BenchVice", at=(vx + 0.02, -D / 2 - 0.03, z_top - 0.05), bevel=0.012))
p.append(box("ViceChop", (0.30, 0.10, 0.34), "BenchVice", at=(vx + 0.04, -D / 2 - 0.10, z_top - 0.22), bevel=0.014))
p.append(box("ViceFace", (0.18, 0.06, 0.24), "BenchVice", at=(vx + 0.07, -D / 2 - 0.17, z_top - 0.20), bevel=0.01))
p.append(prism("ViceHub", 0.06, 0.09, "BenchVice", sides=6, at=(vx + 0.07, -D / 2 - 0.20, z_top - 0.11), rot=(90, 0, 0)))
p.append(prism("ViceHandle", 0.026, 0.36, "BenchVice", sides=8, at=(vx + 0.07, -D / 2 - 0.27, z_top - 0.48)))
for z in (z_top - 0.50, z_top - 0.13):
    p.append(prism(f"HandleKnob{z:.2f}", 0.042, 0.06, "BenchVice", sides=8, at=(vx + 0.07, -D / 2 - 0.27, z)))
# A small end vice block and its peg on the left end, a hex peg on the right front leg.
p.append(box("EndBlock", (0.08, 0.14, 0.14), "BenchLeg", at=(-L / 2 - 0.02, -0.18, z_top - 0.16), bevel=0.008))
p.append(box("EndPeg", (0.10, 0.07, 0.07), "BenchLeg", at=(-L / 2 - 0.09, -0.18, z_top - 0.12), bevel=0.006))
p.append(prism("LegPeg", 0.035, 0.06, "BenchLeg", sides=6, at=(L / 2 - 0.08, -D / 2 + 0.02, z_top - 0.12), rot=(90, 0, 0)))

# A hammer lying on the top, at an angle.
ang = -38
p.append(prism("HammerHandle", 0.032, 0.50, "Handle", sides=6, at=(0.02, 0.18, H + 0.035), rot=(0, 90, ang)))
p.append(box("HammerHead", (0.22, 0.09, 0.09), "HammerHead", at=(0.02, 0.18, H), rot=(0, 0, ang + 90), bevel=0.008))

# An open slatted crate on the shelf.
cx, cy, cz = 0.10, 0.0, 0.28
cw, cd, ch = 0.54, 0.40, 0.28
for sx in (-1, 1):
    for sy in (-1, 1):
        p.append(box(f"CratePost{sx}{sy}", (0.05, 0.05, ch), "Crate", at=(cx + sx * (cw / 2 - 0.025), cy + sy * (cd / 2 - 0.025), cz + ch / 2), bevel=0.004))
for zz in (0.05, 0.17):
    for sy in (-1, 1):
        p.append(box(f"CrateSlatX{zz}{sy}", (cw - 0.04, 0.025, 0.08), "Crate", at=(cx, cy + sy * (cd / 2 - 0.012), cz + zz), bevel=0.004))
    for sx in (-1, 1):
        p.append(box(f"CrateSlatY{zz}{sx}", (0.025, cd - 0.04, 0.08), "Crate", at=(cx + sx * (cw / 2 - 0.012), cy, cz + zz), bevel=0.004))
p.append(box("CrateFloor", (cw - 0.04, cd - 0.04, 0.02), "Crate", at=(cx, cy, cz + 0.01)))

bench = look.join(p, "Workbench")
look.ground_to_origin(bench)
report = look.checks(bench, 2500)
print("REPORT", report)
look.export(bench, os.path.join(look.OUT, "image-Workbench.glb"))

# Render from the concept's camera (front left, a little above), on a plain grey ground,
# to sit beside the concept.
round_name = (look.args() + ["1"])[0]
look.stage()
bpy.data.objects["Ground"].hide_render = True
bpy.context.scene.render.film_transparent = True
scene = bpy.context.scene
scene.render.resolution_x = scene.render.resolution_y = 1024
cam = bpy.data.objects.new("Cam", bpy.data.cameras.new("Cam"))
cam.data.lens = 50
scene.collection.objects.link(cam)
scene.camera = cam
target = Vector((0.0, 0.0, 0.50))
yaw, pitch, dist = math.radians(-22), math.radians(33), 3.7
cam.location = target + Vector((math.sin(yaw) * math.cos(pitch), -math.cos(yaw) * math.cos(pitch), math.sin(pitch))) * dist
cam.rotation_euler = (target - cam.location).to_track_quat("-Z", "Y").to_euler()
sun = bpy.data.objects["Sun"]
sun.rotation_euler = Vector((0.5, 0.8, -0.7)).to_track_quat("-Z", "Y").to_euler()
# Soft, neutral studio light like the concept's, not the game's warm sun.
sun.data.color = (1.0, 0.96, 0.9)
sun.data.energy = 2.2
scene.world.node_tree.nodes["Background"].inputs["Color"].default_value = (0.75, 0.75, 0.78, 1)
scene.world.node_tree.nodes["Background"].inputs["Strength"].default_value = 0.7
scene.view_settings.view_transform = "Standard"
scene.render.filepath = os.path.join(look.OUT, f"image-workbench-match-{round_name}.png")
bpy.ops.render.render(write_still=True)
print("MATCH", scene.render.filepath)
