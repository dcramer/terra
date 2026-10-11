#!/usr/bin/env python3
"""Props for the four scene tests, in direction D: a concept, then a low-poly model straight
from it (props are simple enough to skip the four views). 40 credits each. Waits for any
other Tripo job first, since the account runs one task at a time; skips finished ones."""

import os
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
TRIPO = os.path.join(HERE, "..", "tripo.py")
OUT = os.path.join(HERE, "..", "out", "tripo")
D = ("Stylised 3D render with a graphic, illustrative look: bold simplified shapes, large flat areas of "
     "colour from a limited palette, a few crisp shadow shapes, no texture, no outlines; strong silhouette. "
     "Single subject, whole subject in frame and centred, on a plain light grey background, no ground, no text.")
PROPS = {
    "stall": ("A medieval market stall: a wooden counter with a striped cloth awning on poles, baskets of produce and crates.", 2500),
    "barrel": ("A wooden barrel with iron hoops, standing upright.", 800),
    "well": ("A medieval stone well with a wooden roof, a crank and a hanging bucket.", 2500),
    "lantern": ("A medieval wrought-iron street lantern on a tall wooden post.", 1200),
    "vending": ("A chunky cyberpunk vending machine with a glowing screen, buttons, stickers and a dented side panel.", 2000),
    "streetlamp": ("A futuristic street lamp: a tall angular metal pole with a glowing light bar and a small sign.", 1200),
    "hoverbike": ("A sleek hover bike with no wheels, glowing thrusters underneath, handlebars and a long seat, side view.", 3000),
    "mushroom": ("A giant fantasy mushroom with a broad spotted cap and a thick curved stem with glowing gills.", 2000),
    "crystals": ("A cluster of tall fantasy crystals growing from a rock, sharp faceted spires of different heights.", 1500),
    "runearch": ("An ancient standing-stone arch: two tall weathered stones with a lintel across them, carved with runes and patched with moss.", 1500),
    "gravestone": ("A weathered gothic gravestone with a pointed arch top and a small cross, slightly tilted.", 800),
    "deadtree": ("A dead gnarled tree with twisted bare branches and no leaves.", 2500),
}


def busy():
    return subprocess.run(["pgrep", "-f", "tripo.py (views|multiview|concept|image)"], capture_output=True).returncode == 0


for name, (text, faces) in PROPS.items():
    concept = os.path.join(OUT, f"prop-{name}-concept-generated_image_url.png")
    model = os.path.join(OUT, f"prop-{name}-p1.glb")
    if os.path.exists(model):
        continue
    while busy():
        time.sleep(20)
    if not os.path.exists(concept):
        subprocess.run([sys.executable, "-I", TRIPO, "concept", f"prop-{name}-concept", f"{text} {D}", "--raw"], check=False)
    if os.path.exists(concept):
        subprocess.run([sys.executable, "-I", TRIPO, "image", f"prop-{name}-p1", concept,
                        "--model", "P1-20260311", "--faces", str(faces), "--texture"], check=False)
subprocess.run([sys.executable, "-I", TRIPO, "balance"])
