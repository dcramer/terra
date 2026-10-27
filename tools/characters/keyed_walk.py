"""A walk keyed from measured targets on a body's own skeleton, as an animator keys one from
its contact, down, passing and up poses (Williams, The Animator's Survival Kit, "walks"), for a
body no take fits: a 7 m walking tree's slow, heavy stride (art/animation/keyed/giant_walk.json,
research/2026-10-26-giant-walker-weight.md).

    tools/blender/run.sh tools/characters/keyed_walk.py BODY.glb CLIPS.glb SPEC.json

The feet are placed and the legs solved, not the legs turned: each frame the planted feet pass
back under the body at the walk's pace (in place, as every game clip runs), the swinging foot
lifts `lift_m` at its peak and lands ahead, toe up; the hips sink `drop_m` below where they
were at contact, `down_s` after it, held there, then rise past it at the passing pose; each
thigh and shin is the two-bone solution reaching that foot with the knee ahead, its hinge kept
across the leg's plane so no twist enters it. The trunk leans, rolls over the planted leg,
counter-turns the hips and folds a little after each contact; the crown (spine_03, neck, head)
nods `nod_lag_s` after the hips' lowest, as a mass on the trunk carries on after it stops; the
arms hang forward and swing against the legs, late.

Keyed on BODY's rest (its proportions, the clip's distances in its metres) and written into
CLIPS as an action of its own, one key a frame, a closed loop, its tracks laid as made_clips.py
lays them, so export_character.py --clips=CLIPS samples it on BODY unchanged. Prints a KEYED
line: the frames, the pace, and how far the hips were held down where a leg could not reach.
"""

import json
import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import bpy
from mathutils import Matrix, Quaternion, Vector

import look

REACH = 0.995  # a leg is never solved straighter than this share of its length


def rest_of(path):
    """BODY's bones at rest in the world: matrices, parents, and its world's up."""
    look.reset()
    bpy.ops.import_scene.gltf(filepath=path)
    arm = next(o for o in bpy.data.objects if o.type == "ARMATURE")
    rest = {b.name: arm.matrix_world @ b.matrix_local for b in arm.data.bones}
    parent = {b.name: (b.parent.name if b.parent else None) for b in arm.data.bones}
    order = [b.name for b in arm.data.bones]  # parents before children, as Blender keeps them
    return rest, parent, order


def hermite(keys, period):
    """A periodic curve through (time, value) keys, monotone between them (Fritsch-Carlson)."""
    n = len(keys)
    ts = [k[0] for k in keys]
    vs = [k[1] for k in keys]
    d = [(vs[(i + 1) % n] - vs[i]) / ((ts[(i + 1) % n] - ts[i]) % period or period) for i in range(n)]
    m = []
    for i in range(n):
        a, b = d[i - 1], d[i]
        m.append(0.0 if a * b <= 0 else (a + b) / 2 * min(1.0, 3 * min(abs(a), abs(b)) / max(abs(a + b) / 2, 1e-9)))

    def at(t):
        t %= period
        i = max(j for j in range(n) if ts[j] <= t) if t >= ts[0] else n - 1
        t0 = ts[i]
        h = (ts[(i + 1) % n] - t0) % period or period
        s = ((t - t0) % period) / h
        v0, v1 = vs[i], vs[(i + 1) % n]
        return ((2 * s**3 - 3 * s**2 + 1) * v0 + (s**3 - 2 * s**2 + s) * h * m[i]
                + (-2 * s**3 + 3 * s**2) * v1 + (s**3 - s**2) * h * m[(i + 1) % n])

    return at


def rot(axis, deg):
    return Quaternion(axis, math.radians(deg)).to_matrix().to_4x4()


def about(point, m):
    return Matrix.Translation(point) @ m @ Matrix.Translation(-point)


def frame_of(y, hinge):
    """A bone's frame from its length's way and the knee's hinge: hinge, length, their cross."""
    h = (hinge - hinge.dot(y) * y).normalized()
    return Matrix((h, y, h.cross(y))).transposed()


def solve(hip, ankle, l1, l2, ahead):
    """The knee of a two-bone leg from `hip` reaching `ankle`, bent toward `ahead`."""
    u = ankle - hip
    d = min(u.length, (l1 + l2) * REACH)
    u.normalize()
    a = (l1 * l1 - l2 * l2 + d * d) / (2 * d)
    h = math.sqrt(max(0.0, l1 * l1 - a * a))
    v = (ahead - ahead.dot(u) * u).normalized()
    return hip + u * a + v * h, hip + u * d


def main():
    body, clips, spec_path = (look.path(a) for a in look.args()[:3])
    with open(spec_path) as f:
        sp = json.load(f)
    rest, parent, order = rest_of(body)
    pos = {b: m.translation.copy() for b, m in rest.items()}
    up = Vector((0, 0, 1))
    # Ahead is where the toes point, not where a side's name says: the Cetri's thigh_l stands
    # on its right (round 10: a walk keyed from the names walked backward, knees bent back).
    ahead = sum((pos[f"ball_{s}"] - pos[f"foot_{s}"] for s in "lr"), Vector())
    ahead = (ahead - ahead.dot(up) * up).normalized()
    left = up.cross(ahead)
    side = {s: math.copysign(1.0, (pos[f"thigh_{s}"] - pos["pelvis"]).dot(left)) for s in "lr"}
    ground = min(pos[b].z for b in ("ball_l", "ball_r", "foot_l", "foot_r"))
    legs = {}
    for s in "lr":
        hip, knee, ankle, ball = (pos[f"{b}_{s}"] for b in ("thigh", "calf", "foot", "ball"))
        legs[s] = {"l1": (knee - hip).length, "l2": (ankle - knee).length, "ankle": ankle, "ball": ball}

    ts, tc = sp["step_s"], 2 * sp["step_s"]
    fps = bpy.context.scene.render.fps / bpy.context.scene.render.fps_base
    frames = round(tc * fps)
    tc = frames / fps
    ts = tc / 2
    stance = sp["stance"]
    pace = sp["step_m"] / ts
    travel = pace * stance * tc  # how far a planted foot passes under the body
    half = travel / 2
    hips = hermite([(0.0, 0.0), (sp["down_s"], -sp["drop_m"]), (sp["down_s"] + sp["hold_s"], -sp["drop_m"] * 0.92),
                    (sp["passing_s"], sp["rise_m"]), (ts - 0.05, 0.01)], ts)
    # The hips' sink after a contact, 0 to 1, which the trunk's fold and the crown's nod follow late.
    sink = lambda t: max(0.0, -hips(t) / sp["drop_m"])
    contact_h = pos["pelvis"].z - sp["crouch_m"]
    cyc = lambda t: math.cos(2 * math.pi * t / tc)

    def foot(s, t):
        """Where side `s`'s ankle is, its pitch (toe up, degrees), and its toes' bend, at `t`."""
        leg = legs[s]
        p = ((t - (0.0 if s == "l" else ts)) % tc) / tc
        rest_a, rest_b = leg["ankle"], leg["ball"]
        if p < stance:
            off = half - pace * p * tc
            base_a = rest_a + ahead * off
            base_b = rest_b + ahead * off
            k = p * tc
            pitch = sp["heel_deg"] * max(0.0, 1 - k / sp["flat_s"]) ** 2
            heel = max(0.0, (p - (stance - sp["heel_off"])) / sp["heel_off"])
            if heel > 0:
                up_deg = sp["toe_off_deg"] * heel * heel
                turned = rot(left, up_deg).to_3x3() @ (rest_a - rest_b)
                return base_b + turned, -up_deg, up_deg
            return base_a, pitch, 0.0
        w = (p - stance) / (1 - stance)
        start = rest_b - ahead * half + rot(left, sp["toe_off_deg"]).to_3x3() @ (rest_a - rest_b)
        end = rest_a + ahead * half
        e = w * w * (3 - 2 * w)
        e = 0.5 * e + 0.5 * (1 - (1 - w) ** 2.2)  # moves out early, eases in to the contact
        at = start.lerp(end, e)
        k = w / sp["lift_at"] if w < sp["lift_at"] else 1 + (w - sp["lift_at"]) / (1 - sp["lift_at"])
        at = at + up * sp["lift_m"] * math.sin(math.pi * k / 2) ** 2  # its peak at lift_at
        at = at + left * side[s] * sp["out_m"] * math.sin(math.pi * w)
        pitch = -sp["toe_off_deg"] * (1 - w) ** 2 + sp["heel_deg"] * w ** 3
        return at, pitch, sp["toe_off_deg"] * (1 - w) ** 2

    poses = []
    held = 0.0
    for f in range(frames + 1):
        t = f / fps
        # Over the l leg as it is planted, its hip up, the r hip forward as the r foot lands.
        sway = sp["sway_m"] * cyc(t - stance * ts) * side["l"]
        roll = sp["roll_deg"] * cyc(t - stance * ts) * side["l"]
        yaw = sp["yaw_deg"] * cyc(t - ts) * side["l"]
        feet = {s: foot(s, t) for s in "lr"}
        # The hips: sunk after each contact, swayed over the planted leg, rolled and turned.
        turn = rot(up, yaw) @ rot(ahead, roll) @ rot(left, sp["lean_deg"] * 0.3)
        z = contact_h + hips(t)
        pose = {}

        def place(z):
            m = Matrix.Translation(left * sway + up * (z - pos["pelvis"].z)) @ about(pos["pelvis"], turn) @ rest["pelvis"]
            return m

        # Lowered where a leg could not reach its foot: a planted leg never straighter than REACH.
        drop = 0.0
        while True:
            pm = place(z - drop)
            ok = all(((pm @ rest["pelvis"].inverted() @ rest[f"thigh_{s}"]).translation - feet[s][0]).length
                     <= (legs[s]["l1"] + legs[s]["l2"]) * REACH for s in "lr")
            if ok or drop > 0.5:
                break
            drop += 0.005
        held = max(held, drop)
        pose["pelvis"] = pm
        poses.append((t, pose, feet, sway, roll, yaw))

    # The bones' world poses, a frame at a time, parents first; the rest carried by their parents.
    def carried(pose, b):
        p = parent[b]
        if p is None:
            return rest[b]
        if p not in pose:
            pose[p] = carried(pose, p)
        return pose[p] @ rest[p].inverted() @ rest[b]

    def delta(pose, b, m):
        c = carried(pose, b)
        pose[b] = about(c.translation, m) @ c

    def absolute(pose, b, r):
        c = carried(pose, b)
        _, _, scale = c.decompose()
        pose[b] = Matrix.LocRotScale(c.translation, r.to_quaternion(), scale)

    child = {f"{b}_{s}": f"{c}_{s}" for s in "lr" for b, c in (("thigh", "calf"), ("calf", "foot"))}
    keyed = []
    for t, pose, feet, sway, roll, yaw in poses:
        bob = sp["bob_deg"] * sink(t - sp["bob_lag_s"])
        nod = sp["nod_deg"] * sink(t - sp["nod_lag_s"])
        lean = sp["lean_deg"]
        delta(pose, "spine_01", rot(left, lean * 0.4 + bob * 0.6) @ rot(ahead, -roll * 1.2 - sp["over_deg"] * math.copysign(1, roll)) @ rot(up, -yaw * 0.3))
        delta(pose, "spine_02", rot(left, lean * 0.3 + bob * 0.4) @ rot(up, -yaw * 0.3))
        delta(pose, "spine_03", rot(left, nod * 0.35) @ rot(up, -yaw * 0.3))
        delta(pose, "neck_01", rot(left, nod * 0.35 - lean * sp["neck_up"]))
        delta(pose, "Head", rot(left, nod * 0.3))
        for s in "lr":
            hip = carried(pose, f"thigh_{s}").translation
            ankle, pitch, toes = feet[s]
            leg = legs[s]
            knee, reach = solve(hip, ankle, leg["l1"], leg["l2"], ahead + 0.15 * side[s] * left)
            n = (reach - hip).cross(knee - hip)
            n = n if n.dot(left) >= 0 else -n
            if n.length < 1e-6:
                n = left
            n.normalize()
            for bone, a, b in ((f"thigh_{s}", hip, knee), (f"calf_{s}", knee, reach)):
                r0 = rest[bone].to_3x3().normalized()
                y0 = (pos[child[bone]] - pos[bone]).normalized()
                q = frame_of((b - a).normalized(), n) @ frame_of(y0, left).inverted()
                absolute(pose, bone, q @ r0)
            absolute(pose, f"foot_{s}", rot(left, -pitch).to_3x3() @ rest[f"foot_{s}"].to_3x3().normalized())
            delta(pose, f"ball_{s}", rot(left, -toes))
            # The arm on this side swings forward as the other leg comes forward, late.
            other = ts if s == "l" else 0.0
            swing = sp["arm_swing_deg"] * cyc(t - other - sp["arm_lag_s"])
            delta(pose, f"upperarm_{s}", rot(left, -sp["arm_hang_deg"] - swing))
            delta(pose, f"lowerarm_{s}", rot(left, -sp["elbow_deg"] - 0.4 * swing))
        keyed.append(pose)

    # Into the clips' file: the bases each pose asks of its bone, a key a frame.
    look.reset()
    bpy.ops.import_scene.gltf(filepath=clips)
    arm = next(o for o in bpy.data.objects if o.type == "ARMATURE")
    body_mesh = next(o for o in bpy.data.objects if o.type == "MESH" and any(m.type == "ARMATURE" for m in o.modifiers))
    ad = arm.animation_data or arm.animation_data_create()
    ad.action = None
    names = [tr.name for tr in ad.nla_tracks]
    name = sp["name"]
    old = bpy.data.actions.get(name)
    if old is not None:
        bpy.data.actions.remove(old)
    action = bpy.data.actions.new(name)
    ad.action = action
    driven = [b for b in order if b in keyed[0] and b in arm.pose.bones]
    last = {}
    for f, pose in enumerate(keyed):
        for b in driven:
            p = parent[b]
            local = rest[b] if p is None else rest[p].inverted() @ rest[b]
            if p is not None and p not in pose:
                pose[p] = carried(pose, p)
            basis = rest[b].inverted() @ pose[b] if p is None else local.inverted() @ pose[p].inverted() @ pose[b]
            pb = arm.pose.bones[b]
            pb.rotation_mode = "QUATERNION"
            loc, q, _ = basis.decompose()
            if b in last and last[b].dot(q) < 0:
                q.negate()
            last[b] = q
            pb.location = loc
            pb.rotation_quaternion = q
            pb.keyframe_insert("location", frame=f, group=b)
            pb.keyframe_insert("rotation_quaternion", frame=f, group=b)
    ad.action = None
    if name not in names:
        names.append(name)
    for tr in list(ad.nla_tracks):
        ad.nla_tracks.remove(tr)
    for n in names:
        a = bpy.data.actions[n]
        track = ad.nla_tracks.new()
        track.name = n
        strip = track.strips.new(n, int(a.frame_range[0]), a)
        if getattr(a, "slots", None) and len(a.slots):
            strip.action_slot = a.slots[0]
        track.mute = True
    for pb in arm.pose.bones:
        pb.matrix_basis.identity()
    for o in bpy.context.scene.objects:
        o.select_set(o in (arm, body_mesh))
    bpy.context.view_layer.objects.active = arm
    bpy.ops.export_scene.gltf(
        filepath=clips, export_format="GLB", use_selection=True,
        export_animations=True, export_animation_mode="NLA_TRACKS", export_force_sampling=True,
        export_influence_nb=4,
    )
    print(f"KEYED {name} on {look.body_name(body)} into {os.path.relpath(clips, look.ROOT)}: {frames} frames at {fps:g} fps, "
          f"step {ts:.2f} s, pace {pace:.2f} m/s, planted feet pass {travel:.2f} m; hips held down up to {held * 100:.1f} cm "
          f"where a leg could not reach; {len(driven)} bones keyed")


main()
