"""The hips lifted by body size (docs/characters.md, phase 4).

The clips are made on the mannequin, and in some (a roll, a kneel, a fall) it touches the
floor. A thicker body playing them goes into the floor by however much thicker it is. Games
offset the pelvis by body size; here, for each clip and each frame:

  1. the mannequin's lowest point is measured once, at the start of a build (this file run
     on its own: `tools/blender/run.sh tools/characters/lift.py`);
  2. each body's lowest point is measured as the game skins it, and the lift it needs is how
     much deeper it goes than the mannequin, smoothed so it never pops;
  3. the base's own lift is baked into its clips' hips, as the twist step is (`bake`);
  4. each morph's is kept as a curve of how much more (or less) than the base it needs
     (`morph_lifts`), which the game mixes by the body's sliders and adds to the hips
     after the animation (crates/terra_game/src/lift.rs).

A body never goes deeper than the mannequin in any clip, so the rig check's floor test (no
deeper than the mannequin plus 2 cm) holds for every shape.
"""

import json
import os
import sys

import bpy
import numpy as np
from mathutils import Vector

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import look  # noqa: E402

HIPS = "pelvis"
SMOOTH_FRAMES = 3   # a lift is held this many frames either side of where it is needed, then eased


def frames_of(action):
    start, end = action.frame_range
    return list(range(int(round(start)), int(round(end)) + 1))


def lowest(arm, body, action, shape=None):
    """The world height of `body`'s lowest point at every frame of `action` on `arm`, skinned
    as it is set up (call look.skin_as_game first). `shape`: a shape key held at full weight."""
    ad = arm.animation_data or arm.animation_data_create()
    ad.action = action
    if getattr(action, "slots", None) and len(action.slots):
        ad.action_slot = action.slots[0]
    keys = body.data.shape_keys.key_blocks if body.data.shape_keys else []
    for k in keys:
        k.value = 1.0 if k.name == shape else 0.0
    depsgraph = bpy.context.evaluated_depsgraph_get()
    out = []
    for f in frames_of(action):
        bpy.context.scene.frame_set(f)
        ev = body.evaluated_get(depsgraph)
        me = ev.to_mesh()
        co = np.empty(len(me.vertices) * 3)
        me.vertices.foreach_get("co", co)
        co = co.reshape(-1, 3)
        mw = np.array(ev.matrix_world)
        z = co @ mw[2, :3] + mw[2, 3]
        out.append(float(z.min()))
        ev.to_mesh_clear()
    for k in keys:
        k.value = 0.0
    return np.array(out)


CACHE = os.path.join(look.OUT, "lift-mannequin.json")


def stamp():
    return f"{os.path.getsize(look.FIGURE)}-{int(os.path.getmtime(look.FIGURE))}"


def measure_mannequin():
    """Measures the mannequin's lowest point at every frame of every game clip, in a scene of
    its own, and keeps it (CACHE) beside its file's size and time. Run as this file's main,
    once at the start of a build."""
    look.reset()
    bpy.ops.import_scene.gltf(filepath=look.FIGURE)
    arm = next(o for o in bpy.context.scene.objects if o.type == "ARMATURE")
    body = max((o for o in bpy.context.scene.objects if o.type == "MESH"), key=lambda o: len(o.data.vertices))
    look.skin_as_game()
    actions = {a.name.split(".")[0]: a for a in bpy.data.actions}
    clips = {name: lowest(arm, body, actions[name]).tolist() for name in look.game_clips() if name in actions}
    with open(CACHE, "w") as f:
        json.dump({"stamp": stamp(), "clips": clips}, f)
    print("LIFT mannequin measured,", len(clips), "clips")


def reference():
    """The mannequin's lowest point at every frame of each game clip (measure_mannequin)."""
    if not os.path.exists(CACHE):
        raise SystemExit(f"lift: no {CACHE}; run tools/blender/run.sh tools/characters/lift.py")
    with open(CACHE) as f:
        kept = json.load(f)
    if kept.get("stamp") != stamp():
        raise SystemExit(f"lift: {CACHE} is of an older mannequin; run tools/blender/run.sh tools/characters/lift.py")
    return {a: np.array(z) for a, z in kept["clips"].items()}


def smoothed(need):
    """`need` held SMOOTH_FRAMES either side, then eased over as many: never less than it was
    where it is needed, and no pop as it comes and goes."""
    n = len(need)
    held = np.array([need[max(0, i - SMOOTH_FRAMES): i + SMOOTH_FRAMES + 1].max() for i in range(n)])
    kernel = np.ones(2 * SMOOTH_FRAMES + 1) / (2 * SMOOTH_FRAMES + 1)
    padded = np.pad(held, SMOOTH_FRAMES, mode="edge")
    return np.maximum(need, np.convolve(padded, kernel, mode="valid"))


def needed(arm, body, actions, ref):
    """The lift each of `actions` needs on `body`, per frame: how much deeper than the
    mannequin it goes, never below nothing, smoothed."""
    out = {}
    for action in actions:
        name = action.name.split(".")[0]
        if name not in ref:
            continue
        low = lowest(arm, body, action)
        if len(low) != len(ref[name]):
            continue
        out[name] = smoothed(np.maximum(0.0, ref[name] - low))
    return out


def bake(arm, actions, lifts):
    """Raises the hips of each of `actions` by its lift, every frame, along the world's up."""
    pb = arm.pose.bones[HIPS]
    # The world's up in the hips' own rest space, where their location is keyed.
    up = (pb.bone.matrix_local.to_3x3().inverted() @ (arm.matrix_world.to_3x3().inverted() @ Vector((0, 0, 1)))).normalized()
    ad = arm.animation_data or arm.animation_data_create()
    keep = (ad.action, getattr(ad, "action_slot", None))
    for action in actions:
        name = action.name.split(".")[0]
        lift = lifts.get(name)
        if lift is None or not lift.any():
            continue
        ad.action = action
        if getattr(action, "slots", None) and len(action.slots):
            ad.action_slot = action.slots[0]
        frames = frames_of(action)
        # Everything read before anything is keyed, so a key never changes what is read.
        at = []
        for f in frames:
            bpy.context.scene.frame_set(f)
            at.append(pb.location.copy())
        for f, loc, h in zip(frames, at, lift):
            pb.location = loc + up * float(h)
            pb.keyframe_insert("location", frame=f, group=HIPS)
        print("LIFT", name, f"up to {lift.max() * 100:.1f} cm")
    ad.action, slot = keep
    if slot is not None:
        ad.action_slot = slot


def morph_lifts(arm, body, actions, ref, morphs):
    """For each of `morphs` (shape keys of `body`) and each of `actions`, already lifted for
    the base: how much more each frame (less, if negative) the morph at full weight needs to
    meet the floor as the mannequin does, smoothed. Rounded to the millimetre; clips that
    need nothing are left out."""
    out = {}
    for action in actions:
        name = action.name.split(".")[0]
        if name not in ref:
            continue
        base = lowest(arm, body, action)
        for m in morphs:
            low = lowest(arm, body, action, shape=m)
            if len(low) != len(ref[name]):
                continue
            # Where the base touches the floor as the mannequin does, the morph is held to
            # the same; elsewhere only lifted, never lowered into the floor.
            touching = base <= ref[name] + 0.005
            extra = np.where(touching, base - low, np.maximum(0.0, ref[name] - low))
            extra = np.sign(extra) * smoothed(np.abs(extra))
            if np.abs(extra).max() >= 0.002:
                out.setdefault(name, {})[m] = [round(float(x), 3) for x in extra]
    return out


if __name__ == "__main__":
    measure_mannequin()
