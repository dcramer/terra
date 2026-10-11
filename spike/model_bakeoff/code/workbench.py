"""A workbench: a plank top on splayed legs, a vice, a shelf, and tools left on it."""

import os
import sys

sys.path[:0] = [os.path.dirname(os.path.abspath(__file__)), os.path.dirname(os.path.dirname(os.path.abspath(__file__)))]

import look
from parts import box, prism

look.reset()
p = []
top_z = 0.86
# The top: three thick planks with small gaps, slightly uneven.
for i, (y, mat, dz) in enumerate(((-0.24, "WoodLight", 0.0), (0.0, "Wood", 0.006), (0.24, "WoodLight", -0.004))):
    p.append(box(f"Plank{i}", (1.7, 0.225, 0.07), mat, at=(0, y, top_z + dz), bevel=0.01))
# Legs, splayed outward, and the frame under the top.
for sx in (-1, 1):
    for sy in (-1, 1):
        p.append(box(f"Leg{sx}{sy}", (0.09, 0.09, 0.88), "Wood", at=(sx * 0.7, sy * 0.26, 0.42), rot=(-sy * 4, sx * 4, 0), bevel=0.008))
for sy in (-1, 1):
    p.append(box(f"Apron{sy}", (1.5, 0.05, 0.12), "WoodDark", at=(0, sy * 0.27, top_z - 0.1)))
for sx in (-1, 1):
    p.append(box(f"Stretcher{sx}", (0.05, 0.5, 0.07), "WoodDark", at=(sx * 0.71, 0, 0.22)))
p.append(box("Shelf", (1.45, 0.5, 0.04), "Wood", at=(0, 0, 0.24), bevel=0.006))
# A wooden vice at one end: jaw, screw and handle.
p.append(box("Jaw", (0.08, 0.28, 0.2), "WoodDark", at=(0.9, -0.12, top_z - 0.06), bevel=0.01))
p.append(prism("Screw", 0.022, 0.22, "MetalDark", sides=6, at=(0.88, -0.12, top_z - 0.06), rot=(0, 90, 0)))
p.append(prism("Handle", 0.014, 0.24, "Wood", sides=6, at=(1.1, 0.0, top_z - 0.06), rot=(90, 0, 0)))
# Tools: a hammer, a chisel, and a crate of offcuts on the shelf.
p.append(prism("HammerHandle", 0.016, 0.34, "WoodLight", sides=6, at=(-0.45, -0.05, top_z + 0.055), rot=(0, 90, 25)))
p.append(box("HammerHead", (0.05, 0.14, 0.05), "MetalDark", at=(-0.13, 0.11, top_z + 0.06), rot=(0, 0, 25), bevel=0.006))
p.append(prism("Chisel", 0.012, 0.2, "WoodDark", sides=6, at=(0.2, 0.18, top_z + 0.05), rot=(0, 90, -10)))
p.append(box("ChiselBlade", (0.09, 0.025, 0.01), "Metal", at=(0.45, 0.14, top_z + 0.05), rot=(0, 0, -10)))
p.append(box("Crate", (0.42, 0.32, 0.22), "WoodDark", at=(-0.35, 0, 0.37), bevel=0.01))
for i, (x, rz) in enumerate(((-0.45, 8), (-0.3, -12), (-0.22, 30))):
    p.append(box(f"Offcut{i}", (0.07, 0.07, 0.3), "WoodLight", at=(x, 0.02, 0.5), rot=(10, 12, rz)))
bench = look.join(p, "Workbench")
look.finish([bench], "code-workbench", budget=1500)
