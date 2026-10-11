"""Cut the player figure out of Quaternius's Universal Animation Library.

The library (standard edition, CC0) is a rigged mannequin with 46 animations in
one glTF: https://github.com/J-Ponzo/gltf-universal-animation-library. This
script keeps the mannequin, its rig and the animations listed in KEEP, drops
the rest, recolours the two materials (see MATERIALS), and writes a compact
figure.gltf + figure.bin.

Nothing else changes: nodes, skin, mesh and the kept animations' keyframes are
copied byte for byte. Accessors keep their original relative order and the
buffer is repacked with 4-byte alignment, so cutting the same clips twice gives
identical files.

Usage, from anywhere (standard library only):

    python3 cut_figure.py path/to/AnimationLibrary_Godot_Standard.gltf [out_dir]

out_dir defaults to this script's folder. The sandbox looks clips up by name,
so adding to KEEP never breaks the clips already used.
"""

import json
import os
import sys

# The animations to keep, by their names in the library. The library holds 46;
# the ones left out are the T-pose, swimming, driving, pushing, dancing, the
# formal walk, the torch idle, the pistol and spell clips, and the root-motion
# (_RM) duplicates of Roll and Sword_Attack. The sandbox moves the body itself,
# so it wants the in-place versions.
KEEP = {
    # Moving about.
    "Idle_Loop",
    "Walk_Loop",
    "Jog_Fwd_Loop",
    "Sprint_Loop",
    "Jump_Start",
    "Jump_Loop",
    "Jump_Land",
    "Crouch_Idle_Loop",
    "Crouch_Fwd_Loop",
    "Roll",
    # Working.
    "Interact",
    "PickUp_Table",
    "Fixing_Kneeling",
    # Fighting.
    "Sword_Idle",
    "Sword_Attack",
    "Punch_Enter",
    "Punch_Jab",
    "Punch_Cross",
    "Hit_Chest",
    "Hit_Head",
    "Death01",
    # People in a settlement.
    "Idle_Talking_Loop",
    "Sitting_Enter",
    "Sitting_Idle_Loop",
    "Sitting_Talking_Loop",
    "Sitting_Exit",
}

# The figure's look: a pale body and near-black joints, single-sided.
MATERIALS = {
    "M_Main": {"baseColorFactor": [0.83, 0.8, 0.74, 1], "roughnessFactor": 0.75},
    "M_Joints": {"baseColorFactor": [0.03, 0.032, 0.045, 1], "roughnessFactor": 0.6},
}

OUT_NAME = "figure"


def cut(src_path, out_dir):
    with open(src_path, "rb") as f:
        gltf = json.load(f)
    (buffer,) = gltf["buffers"]
    with open(os.path.join(os.path.dirname(src_path), buffer["uri"]), "rb") as f:
        data = f.read()

    names = {a["name"] for a in gltf["animations"]}
    missing = KEEP - names
    if missing:
        sys.exit(f"not in the library: {sorted(missing)}")
    gltf["animations"] = [a for a in gltf["animations"] if a["name"] in KEEP]

    # Every accessor still referenced, in the library's order.
    used = {gltf["skins"][0]["inverseBindMatrices"]}
    for mesh in gltf["meshes"]:
        for prim in mesh["primitives"]:
            used.update(prim["attributes"].values())
            used.add(prim["indices"])
    for anim in gltf["animations"]:
        for sampler in anim["samplers"]:
            used.update((sampler["input"], sampler["output"]))
    order = sorted(used)
    remap = {old: new for new, old in enumerate(order)}

    # One buffer view per accessor, packed in the same order.
    out = bytearray()
    views, accessors = [], []
    for old in order:
        acc = dict(gltf["accessors"][old])
        view = gltf["bufferViews"][acc["bufferView"]]
        assert "byteStride" not in view, "strided views are not handled"
        start = view.get("byteOffset", 0)
        out += b"\0" * (-len(out) % 4)
        new_view = {"buffer": 0, "byteLength": view["byteLength"], "byteOffset": len(out)}
        if "target" in view:
            new_view["target"] = view["target"]
        out += data[start : start + view["byteLength"]]
        acc["bufferView"] = len(views)
        views.append(new_view)
        accessors.append(acc)

    gltf["skins"][0]["inverseBindMatrices"] = remap[gltf["skins"][0]["inverseBindMatrices"]]
    for mesh in gltf["meshes"]:
        for prim in mesh["primitives"]:
            prim["attributes"] = {k: remap[v] for k, v in prim["attributes"].items()}
            prim["indices"] = remap[prim["indices"]]
    for anim in gltf["animations"]:
        for sampler in anim["samplers"]:
            sampler["input"] = remap[sampler["input"]]
            sampler["output"] = remap[sampler["output"]]

    for mat in gltf["materials"]:
        mat["doubleSided"] = False
        mat["pbrMetallicRoughness"].update(MATERIALS[mat["name"]])

    gltf["accessors"] = accessors
    gltf["bufferViews"] = views
    gltf["buffers"] = [{"byteLength": len(out), "uri": OUT_NAME + ".bin"}]

    with open(os.path.join(out_dir, OUT_NAME + ".gltf"), "w") as f:
        json.dump(gltf, f, separators=(",", ":"))
    with open(os.path.join(out_dir, OUT_NAME + ".bin"), "wb") as f:
        f.write(out)
    print(f"kept {len(gltf['animations'])} animations, {len(out)} bytes of data")


if __name__ == "__main__":
    if len(sys.argv) not in (2, 3):
        sys.exit(__doc__)
    cut(sys.argv[1], sys.argv[2] if len(sys.argv) == 3 else os.path.dirname(os.path.abspath(__file__)))
