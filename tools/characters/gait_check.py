"""How a body moves through the locomotion clips, against the mannequin the clips were made on
(research/2026-10-14-animation.md): the numbers a run is judged by, so a change to the clips or
to how they reach a body is measured, not only looked at.

    tools/blender/run.sh tools/characters/gait_check.py [assets/models/character/base.glb] [--clips=Walk_Loop,Jog_Fwd_Loop]

Each body's game file carries its own copy of the clips (export_character.py); the mannequin
plays them as made. For every clip and both bodies, over the clip's frames:

  arms in torso  the share of forearm and hand skin inside the torso (the trunk's skin, by
                 its strongest bone), worst frame; arms jammed against the chest
  hands          where the hands ride between the hips (0) and the base of the neck (1),
                 the middle of the clip; pumping at the chest is high
  flare          how far the elbows stand out from the shoulders, sideways, in arm lengths
  lean           the trunk's tilt forward from upright, pelvis to neck, in degrees
  shoulders      how far the shoulders rise above the clavicles' roots, in clavicle lengths
  slide          how far a planted foot strays from where the clip's own planted speed
                 carries it, in centimetres, worst moment of any contact; under 1 is planted.
                 A foot is planted near its lowest while moving back under the body

The base is right where it matches the mannequin's numbers, which are the clip's intent, except
slide, which should be under 1 cm on both.
"""

import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import bpy
from mathutils import Vector
from mathutils.bvhtree import BVHTree

import look
import pace

CLIPS = ("Walk_Loop", "Jog_Fwd_Loop", "Sprint_Loop")
ARM_BONES = {f"{b}_{s}" for b in ("lowerarm", "hand", "lowerarm_twist_01") for s in "lr"}
ARM_BONES |= {f"{f}_{n:02d}_{s}" for f in ("thumb", "index", "middle", "ring", "pinky") for n in (1, 2, 3) for s in "lr"}
TORSO_BONES = {"pelvis", "spine_01", "spine_02", "spine_03"}
INSIDE_M = 0.01     # skin this far behind the torso's surface is inside it
FOOT = ("ball_l", "ball_r")


def load(path):
    """The armature and skinned body in `path`, and its clips by name."""
    before, acts = set(bpy.data.objects), set(bpy.data.actions)
    bpy.ops.import_scene.gltf(filepath=look.path(path))
    new = [o for o in bpy.data.objects if o not in before]
    arm = next(o for o in new if o.type == "ARMATURE")
    body = max((o for o in new if o.type == "MESH"), key=lambda o: len(o.data.vertices))
    clips = {a.name.split(".")[0]: a for a in bpy.data.actions if a not in acts}
    return arm, body, clips


def play(arm, action):
    ad = arm.animation_data or arm.animation_data_create()
    ad.action = action
    if getattr(action, "slots", None) and len(action.slots):
        ad.action_slot = action.slots[0]


def parts(body):
    """Indices of the body's arm skin and of its trunk's faces, by each vertex's strongest bone."""
    names = {g.index: g.name for g in body.vertex_groups}
    strongest = {}
    for v in body.data.vertices:
        if v.groups:
            g = max(v.groups, key=lambda g: g.weight)
            strongest[v.index] = names.get(g.group, "")
    arms = [i for i, n in strongest.items() if n in ARM_BONES]
    trunk = [p.index for p in body.data.polygons if all(strongest.get(i) in TORSO_BONES for i in p.vertices)]
    return arms, trunk


def joint(arm, name):
    return arm.matrix_world @ arm.pose.bones[name].head


def measure(arm, body, action):
    play(arm, action)
    scene = bpy.context.scene
    fps = scene.render.fps / scene.render.fps_base
    start, end = (int(round(x)) for x in action.frame_range)
    arms, trunk = parts(body)
    clip_pace = pace.pace(arm, action) or 0.0
    play(arm, action)
    inside, hands, flare, lean, shoulders, feet = [], [], [], [], [], []
    for f in range(start, end + 1):
        scene.frame_set(f)
        dg = bpy.context.evaluated_depsgraph_get()
        mesh = body.evaluated_get(dg).to_mesh()
        mw = body.matrix_world
        pts = [mw @ v.co for v in mesh.vertices]
        tri = [[mesh.polygons[i].vertices[k] for k in range(len(mesh.polygons[i].vertices))] for i in trunk]
        tree = BVHTree.FromPolygons(pts, tri)
        n_in = 0
        for i in arms:
            hit, normal, _, _ = tree.find_nearest(pts[i])
            if hit is not None and (pts[i] - hit).dot(normal) < -INSIDE_M:
                n_in += 1
        inside.append(n_in / max(1, len(arms)))
        body.evaluated_get(dg).to_mesh_clear()
        hips, neck = joint(arm, "pelvis"), joint(arm, "neck_01")
        span = (neck - hips).length
        for s in "lr":
            hand, elbow, shoulder = joint(arm, f"hand_{s}"), joint(arm, f"lowerarm_{s}"), joint(arm, f"upperarm_{s}")
            root = joint(arm, f"clavicle_{s}")
            arm_len = (elbow - shoulder).length + (hand - elbow).length
            hands.append((hand.z - hips.z) / span)
            flare.append((abs(elbow.x - hips.x) - abs(shoulder.x - hips.x)) / arm_len)
            shoulders.append((shoulder.z - root.z) / max(1e-6, (shoulder - root).length))
        trunk_up = (neck - hips).normalized()
        lean.append(math.degrees(math.atan2(-trunk_up.y, trunk_up.z)))
        feet.append([joint(arm, b) for b in FOOT])
    # A contact: a run of frames with the foot near its lowest and moving back under the body
    # (the body faces -Y), as a planted foot does in a clip that runs in place; a foot swung low
    # past the other is near its lowest too, but moving forward. Planted, it moves back at the
    # clip's own speed, the middle of its planted feet's; what it strays from that is slide.
    n = len(feet)
    planted = []
    for side in range(len(FOOT)):
        low = min(at[side].z for at in feet)
        back = [(feet[(k + 1) % n][side].y - feet[k][side].y) * fps for k in range(n)]
        planted.append([feet[k][side].z < low + pace.LOW_M and back[k] > 0 for k in range(n)])
    speeds = sorted((feet[(k + 1) % n][s].y - feet[k][s].y) * fps for s in range(len(FOOT)) for k in range(n) if planted[s][k])
    speed = speeds[len(speeds) // 2] if speeds else clip_pace
    slides = []
    for side in range(len(FOOT)):
        # Runs of planted frames, joined round the loop's seam.
        start = next((k for k in range(n) if not planted[side][k]), None)
        if start is None:
            continue
        run = []
        for i in range(1, n + 1):
            k = (start + i) % n
            if planted[side][k]:
                run.append(k)
                continue
            if len(run) >= 3:
                a = feet[run[0]][side]
                for j, f in enumerate(run[1:], 1):
                    expected = a + Vector((0.0, speed * j / fps, 0.0))
                    slides.append((feet[f][side] - expected).length * 100)
            run = []
    mid = lambda xs: sorted(xs)[len(xs) // 2]
    return {
        "arms in torso": f"{max(inside) * 100:5.1f}%",
        "hands": f"{mid(hands):5.2f}",
        "flare": f"{mid(flare):5.2f}",
        "lean": f"{mid(lean):5.1f} deg",
        "shoulders": f"{mid(shoulders):5.2f}",
        "slide": f"{max(slides) if slides else 0.0:5.1f} cm",
    }


def main():
    given = [a for a in look.args() if not a.startswith("--")]
    body_path = given[0] if given else os.path.join(look.ASSETS, "models", "character", "base.glb")
    wanted = next((a.split("=", 1)[1].split(",") for a in look.args() if a.startswith("--clips=")), CLIPS)
    look.reset()
    bodies = [("mannequin", *load(look.FIGURE)), (look.body_name(body_path), *load(body_path))]
    # Side by side, so neither stands in the other's skin.
    bodies[1][1].location.x += 3.0
    look.skin_as_game()
    for clip in wanted:
        for name, arm, body, clips in bodies:
            if clip not in clips:
                print(f"GAIT {clip:14s} {name:12s} no such clip")
                continue
            row = measure(arm, body, clips[clip])
            print(f"GAIT {clip:14s} {name:12s} " + "  ".join(f"{k} {v}" for k, v in row.items()))


main()
