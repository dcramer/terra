"""A frontier fence: split posts, two rails a span, rope lashed at the joints."""

import os
import random
import sys

sys.path[:0] = [os.path.dirname(os.path.abspath(__file__)), os.path.dirname(os.path.dirname(os.path.abspath(__file__)))]

import look
from parts import box, prism

look.reset()
rng = random.Random(5)
pieces = []
span = 2.2
posts = 4
for i in range(posts):
    x = i * span
    lean = (rng.uniform(-3, 3), rng.uniform(-3, 3), rng.uniform(0, 60))
    height = 1.25 + rng.uniform(-0.08, 0.08)
    pieces.append(prism(f"Post{i}", 0.09, height, "Wood", sides=6, at=(x, 0, 0), rot=lean, top=0.075, seed=i))
    # A pointed cap, cut at an angle.
    pieces.append(prism(f"Cap{i}", 0.075, 0.12, "WoodLight", sides=6, at=(x, 0, height), top=0.02, seed=i + 10))
    for z in (0.45, 0.95):
        pieces.append(prism(f"Lash{i}{z}", 0.105, 0.07, "Rope", sides=6, at=(x, 0, z - 0.035), seed=i + 20))
for i in range(posts - 1):
    x = i * span + span / 2
    for k, z in enumerate((0.45, 0.95)):
        sag = rng.uniform(-2.5, 2.5)
        pieces.append(box(f"Rail{i}{k}", (span + 0.25, 0.07, 0.11), "Wood" if k else "WoodDark", at=(x, 0.1, z), rot=(rng.uniform(-6, 6), sag, rng.uniform(-1.5, 1.5)), bevel=0.012))
fence = look.join(pieces, "Fence")
look.finish([fence], "code-fence", budget=1500)
