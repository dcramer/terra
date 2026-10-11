#!/usr/bin/env python3
"""Direction D beyond fantasy: tech subjects to make into models, and realm scenes to test
range. One image at a time through tripo.py, 10 credits each; skips any already made."""

import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
TRIPO = os.path.join(HERE, "..", "tripo.py")
D = ("Stylised 3D render with a graphic, illustrative look: bold simplified shapes, large flat "
     "areas of colour from a limited palette, a few crisp shadow shapes, no texture, no outlines; "
     "strong silhouettes; warm low sunlight and atmospheric depth.")
OBJECT = " Single subject, whole subject in frame and centred, on a plain light grey background, no ground, no text."
IMAGES = {
    "d-fabricator": ("A compact sci-fi fabricator crafting station: a squat machine with a work bed, a robotic arm, "
                     "vents and a small glowing screen, three-quarter view.", True),
    "d-drone": ("A small hovering scout drone creature: a rounded shell body, one large lens eye, two stubby "
                "manipulator arms and twin thrusters underneath, three-quarter side view.", True),
    "d-explorer": ("A human explorer in a light exosuit with shoulder plates, a backpack with a small antenna and "
                   "an open helmet visor, full body standing in a relaxed A-pose, front three-quarter view.", True),
    "d-flora": ("A strange alien plant: a tall curving stalk with fan-shaped translucent leaves and glowing bulbs "
                "at the tips, three-quarter view.", True),
    "d-realm-lush": ("A wide view of a lush starter realm: rolling green meadows, broadleaf groves, a winding "
                     "river, soft clouds and distant blue hills in haze.", False),
    "d-realm-ashen": ("A wide view of a dead, ashen realm: grey cracked plains, black petrified trees, smouldering "
                      "vents, a pale sun behind thin smoke, distant jagged ridges in haze. Bleak but beautiful.", False),
    "d-realm-crystal": ("A wide view of a strange crystal realm: violet and teal crystal spires rising from pale "
                        "sand dunes, two moons in a dusky sky, glowing fungi, distant mesas in haze.", False),
    "d-realm-tech": ("A wide view of a sci-fi frontier outpost on a red desert plateau: modular habitat domes, an "
                     "antenna mast, landing pads, a parked rover, distant mesas and a ringed planet in the sky.", False),
}
for name, (text, single) in IMAGES.items():
    if os.path.exists(os.path.join(HERE, "..", "out", "tripo", f"{name}-concept-generated_image_url.png")):
        continue
    prompt = f"{text} {D}" + (OBJECT if single else "")
    subprocess.run([sys.executable, "-I", TRIPO, "concept", f"{name}-concept", prompt, "--raw"], check=False)
