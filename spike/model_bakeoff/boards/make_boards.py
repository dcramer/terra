#!/usr/bin/env python3
"""Mood boards: the same five subjects drawn in each style direction, so styles compare
like for like. Calls tripo.py once per image (10 credits each), one at a time."""

import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
TRIPO = os.path.join(HERE, "..", "tripo.py")

STYLES = {
    "a-facets": "Stylised low-poly 3D render: simple forms cut into many small flat planes like folded paper, each plane one flat colour, no textures, no outlines; a small, soft, harmonious palette; warm sunlight with cool blue-violet shadows.",
    "b-soft": "Stylised 3D render: simple rounded, softly modelled forms with smooth shading, flat colours with gentle gradients, no surface texture, no outlines; friendly, slightly exaggerated proportions; soft warm light and pastel atmosphere.",
    "c-painted": "Stylised 3D game render: chunky, readable forms with hand-painted textures, visible brush strokes, painted highlights and soft colour variation on every surface; rich warm colours; soft fantasy light.",
    "d-graphic": "Stylised 3D render with a graphic, illustrative look: bold simplified shapes, large flat areas of colour from a limited palette, a few crisp shadow shapes, no texture, no outlines; strong silhouettes; warm low sunlight and atmospheric depth.",
}
OBJECT = " Single subject, whole subject in frame and centred, on a plain light grey background, no ground, no text."
SUBJECTS = {
    "wolf": "A grey wolf standing calmly, full body, three-quarter side view, all four legs visible.",
    "villager": "A frontier villager man in a simple red tunic with a leather belt and pouch, trousers and boots, full body, standing, front three-quarter view.",
    "workbench": "A rustic carpenter's workbench with a wooden vice, a hammer on top and a small crate on the lower shelf, three-quarter view.",
    "tree": "A broadleaf tree with a short forked trunk and a broad, full crown.",
    "scene": "A wide view of a small timber frontier outpost on a grassy hillside in late golden-hour light: a campfire, a few broadleaf trees, a fence, a wolf watching from the edge of a forest, distant hills fading into haze.",
}

only = sys.argv[1:]
for style, look in STYLES.items():
    for subject, text in SUBJECTS.items():
        name = f"board-{style}-{subject}"
        if only and style not in only:
            continue
        if os.path.exists(os.path.join(HERE, "..", "out", "tripo", f"{name}-generated_image_url.png")):
            continue
        prompt = f"{text} {look}" + ("" if subject == "scene" else OBJECT)
        subprocess.run([sys.executable, "-I", TRIPO, "concept", name, prompt, "--raw"], check=False)
