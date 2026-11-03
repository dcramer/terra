"""Writes a wardrobe body's pieces as the game loads them (D611): each piece of the catalogue
(assets/models/character/wardrobe.json) made on BODY, alone with its copy of the skeleton, as
its own glTF, `assets/<pieces>/<piece>.gltf` with its `.bin`, its textures in `textures/`
beside them, shared: a texture two pieces use (their Ranger maps, their skin) is one file, so
the game loads it once however many pieces and people use it.

    tools/blender/run.sh tools/characters/export_pieces.py BODY [--textures=2048]

A piece's source is one of Quaternius's files as shipped, or, with `cut`, the bare piece
cut_slots.py cut from it (tools/characters/out/quaternius-SEX-bare-REGION.glb). Nothing of
theirs is changed (D599): their meshes, normals, UVs and weights go through as made, which
`intact.ts` proves on every file written; only the maps are scaled to the body's texture
budget (D188), and the images' names made one (their full body names its maps `..._png`).
"""

import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import json
import re

import bpy

import look

BODY = look.args()[0]
with open(os.path.join(look.ASSETS, "models", "character", "wardrobe.json")) as f:
    WARDROBE = json.load(f)
body = WARDROBE["bodies"][BODY]
folder = os.path.join(look.ASSETS, body["pieces"])
os.makedirs(folder, exist_ok=True)
pieces = sorted((name, p) for name, p in WARDROBE["pieces"].items() if p["body"] == BODY)
for name, piece in pieces:
    look.reset()
    if piece.get("cut"):
        sex = "female" if "Female" in os.path.basename(piece["source"]) else "male"
        source = os.path.join(look.OUT, f"quaternius-{sex}-bare-{piece['cut']}.glb")
    else:
        source = os.path.join(look.ROOT, piece["source"])
    arm, skinned = look.join(source)
    for pb in arm.pose.bones:
        pb.matrix_basis.identity()
    # One name a map: their full body's are T_..._png, the same pictures as their head's.
    for image in bpy.data.images:
        image.name = re.sub(r"_png$", "", os.path.splitext(image.name)[0])
    look.fit_textures(sys.argv)
    for o in bpy.context.scene.objects:
        o.select_set(o is arm or o in skinned)
    bpy.context.view_layer.objects.active = arm
    dst = os.path.join(folder, f"{name}.gltf")
    bpy.ops.export_scene.gltf(
        filepath=dst, export_format="GLTF_SEPARATE", export_texture_dir="textures", use_selection=True,
        export_animations=False, export_all_influences=True, export_extras=True, export_morph=False,
    )
    print("PIECE", name, os.path.relpath(dst, look.ROOT), ", ".join(o.name for o in skinned))
