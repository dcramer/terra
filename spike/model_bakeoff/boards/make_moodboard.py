#!/usr/bin/env python3
"""The mood board: four scenes in direction C's painted, richly lit look. Two cities, two
landscapes, across settings. Waits for other Tripo jobs; 10 credits each."""

import os
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
TRIPO = os.path.join(HERE, "..", "tripo.py")
OUT = os.path.join(HERE, "..", "out", "tripo")
C = ("Stylised 3D game render, eye-level view at true life size: chunky readable forms with hand-painted "
     "textures, visible brush strokes, painted highlights and soft colour variation on every surface; "
     "rich colour; soft cinematic light with warm bounce light, ambient occlusion and atmospheric depth.")
SCENES = {
    "medieval-town": "A medieval market town street in warm afternoon light: timber-framed houses with plaster walls and steep tiled roofs leaning over a cobbled street, a market stall with a striped awning, barrels, a stone well, lanterns on posts, townsfolk in tunics.",
    "techno-street": "A techno-punk city street at dusk: tall modular concrete blocks with balconies, pipes and air-conditioning units, glowing neon signs and window strips, overhead cables, vending machines, a parked hover bike, a small drone, a person in a light exosuit, wet pavement reflecting the neon.",
    "fantasy-wilds": "A vast fantasy wilderness at real scale in bright daylight: giant mushrooms taller than houses, clusters of glowing crystals, an ancient standing-stone arch, towering trees, a river, a lone traveller and a wolf for scale, distant mountains in haze.",
    "gothic-graveyard": "A gothic graveyard on a hill by moonlight: a ruined stone chapel with pointed-arch windows, crooked gravestones, dead twisted trees, a wrought-iron fence, warm lanterns glowing in the mist, a huge pale moon.",
}


def busy():
    return subprocess.run(["pgrep", "-f", "tripo.py (views|multiview|concept|image)"], capture_output=True).returncode == 0


for name, text in SCENES.items():
    if os.path.exists(os.path.join(OUT, f"mood-{name}-generated_image_url.png")):
        continue
    while busy():
        time.sleep(15)
    subprocess.run([sys.executable, "-I", TRIPO, "concept", f"mood-{name}", f"{text} {C}", "--raw"], check=False)
