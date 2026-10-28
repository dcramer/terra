"""Tests of the stand pass's geometry (stand.py) on legs whose answer is known another way, in
Blender's Python (mathutils):

    tools/blender/run.sh tools/characters/test_stand.py
"""

import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from mathutils import Quaternion, Vector  # noqa: E402

import stand  # noqa: E402


def bend(hip, knee, ankle):
    # mathutils works in single precision: answers are good to about 1e-6 m.
    return math.degrees((knee - hip).angle(ankle - knee))


def test_reach():
    # Straight, a leg is both bones long; folded flat, their difference; square, Pythagoras.
    assert abs(stand.reach(0.45, 0.43, 0.0) - 0.88) < 1e-5
    assert abs(stand.reach(0.45, 0.43, 180.0) - 0.02) < 1e-5
    assert abs(stand.reach(0.3, 0.4, 90.0) - 0.5) < 1e-5


def test_highest():
    # An ankle straight below: the hip rises to exactly `far` above it.
    hip, ankle = Vector((0, 0, 0.8)), Vector((0, 0, 0))
    assert abs(stand.highest(hip, ankle, 0.85) - 0.05) < 1e-5
    # Set 0.3 to the side, it rises until 0.3, h make the 3-4-5 triangle's 0.5.
    assert abs(stand.highest(Vector((0.3, 0, 0.2)), ankle, 0.5) - 0.2) < 1e-5
    # Already beyond reach: it must come down.
    assert stand.highest(hip, ankle, 0.7) < 0


def test_two_bone_reaches_and_keeps_its_plane():
    hip, ankle = Vector((0.1, 0, 0.84)), Vector((0.12, -0.05, 0.08))
    was_knee = Vector((0.11, -0.12, 0.45))   # bent forward (-Y)
    knee, reached = stand.two_bone(hip, ankle, was_knee, 0.45, 0.43)
    assert (reached - ankle).length < 1e-5
    assert abs((knee - hip).length - 0.45) < 1e-5 and abs((reached - knee).length - 0.43) < 1e-5
    # In the plane of hip, ankle and the old knee, on the old knee's side.
    normal = (ankle - hip).cross(was_knee - hip).normalized()
    assert abs((knee - hip).dot(normal)) < 1e-5
    axis = (ankle - hip).normalized()
    side = lambda p: (p - hip) - axis * (p - hip).dot(axis)
    assert side(knee).dot(side(was_knee)) > 0


def test_two_bone_never_locks_the_knee():
    # An ankle beyond reach: the knee stays bent the least, the ankle short on the line to it.
    hip, ankle = Vector((0, 0, 1.0)), Vector((0, -0.02, 0))
    knee, reached = stand.two_bone(hip, ankle, Vector((0, -0.1, 0.5)), 0.45, 0.43)
    assert abs(bend(hip, knee, reached) - stand.KNEE_MIN) < 1e-4
    assert ((reached - hip).normalized() - (ankle - hip).normalized()).length < 1e-5


def test_hips_raised_by_highest_bend_the_knee_the_least():
    # The two together: a hip raised as high as `highest` allows, then solved, bends KNEE_MIN.
    hip, ankle, l1, l2 = Vector((0.1, 0.02, 0.8)), Vector((0.12, -0.03, 0.06)), 0.45, 0.43
    up = stand.highest(hip, ankle, stand.reach(l1, l2, stand.KNEE_MIN))
    raised = hip + stand.UP * up
    knee, reached = stand.two_bone(raised, ankle, Vector((0.11, -0.1, 0.45)), l1, l2)
    assert (reached - ankle).length < 1e-5
    assert abs(bend(raised, knee, reached) - stand.KNEE_MIN) < 1e-4


def test_drawn_back():
    # The body faces -Y with its left at +X; a neck leaning forward 3 cm is drawn back 3 cm.
    left = Vector((1, 0, 0))
    assert (stand.ahead_of(left) - Vector((0, -1, 0))).length < 1e-5
    v = Vector((0, -0.03, 0.12))
    q = stand.drawn_back(v, left, 0.03)
    w = q @ v
    assert abs(w.dot(Vector((0, -1, 0)))) < 1e-4
    assert abs(w.length - v.length) < 1e-5 and abs(w.x) < 1e-5


def legs_at(bob, l1=0.45, l2=0.43):
    # A standing pair of legs whose hips bob `bob` m below straight-and-a-bit, the knees bent
    # forward (-Y) to reach: each side (hip, knee, ankle).
    frame = {}
    for s, x in (("l", 0.1), ("r", -0.1)):
        hip, ankle = Vector((x, 0, 0.86 - bob)), Vector((x, 0.01, 0.0))
        knee, _ = stand.two_bone(hip, ankle, Vector((x, -0.2, 0.45)), l1, l2, 0.0)
        frame[s] = (hip, knee, ankle)
    return frame


def test_softened():
    # Over the least bend, a share of the bend over it; under it, untouched; a share of 1 none.
    assert abs(stand.softened(28.0, 0.5) - (stand.KNEE_MIN + 10.0)) < 1e-9
    assert stand.softened(5.0, 0.5) == 5.0
    assert stand.softened(28.0, 1.0) == 28.0


def test_lift_bends_the_limiting_knee_softened():
    # Raised by lift_at and solved, the knee bends exactly the softened bend of the take's.
    legs = {"l": (0.45, 0.43), "r": (0.45, 0.43)}
    frame = legs_at(0.03)
    up = stand.lift_at(frame, legs, 0.4)
    hip, knee, ankle = frame["l"]
    was = bend(hip, knee, ankle)
    k, reached = stand.two_bone(hip + stand.UP * up, ankle, knee, 0.45, 0.43)
    assert up > 0 and (reached - ankle).length < 1e-5
    assert abs(bend(hip + stand.UP * up, k, reached) - stand.softened(was, 0.4)) < 1e-2


def test_softness_raises_the_hips_as_asked_with_smaller_steps():
    # Hips bobbing through a loop: the share found raises them the asked mean, and each frame's
    # knee step is at most that share of the take's (the knee near straight turns most per mm).
    legs = {"l": (0.45, 0.43), "r": (0.45, 0.43)}
    frames = [legs_at(0.03 + 0.004 * math.sin(i / 8.0) + (0.003 if i == 20 else 0.0)) for i in range(60)]
    c = stand.softness(frames, legs, 0.012)
    assert 0.0 < c < 1.0
    lifts = [stand.lift_at(f, legs, c) for f in frames]
    assert abs(sum(lifts) / len(lifts) - 0.012) < 1e-4
    was, now = [], []
    for f, up in zip(frames, lifts):
        hip, knee, ankle = f["l"]
        k, reached = stand.two_bone(hip + stand.UP * up, ankle, knee, 0.45, 0.43)
        was.append(bend(hip, knee, ankle))
        now.append(bend(hip + stand.UP * up, k, reached))
    step = lambda xs: max(abs(a - b) for a, b in zip(xs, xs[1:]))
    assert step(now) <= c * step(was) + 1e-2
    assert min(now) >= stand.KNEE_MIN - 1e-3
    # Asked for nothing, nothing changes; asked for more than straight knees give, they are straight.
    assert stand.softness(frames, legs, 0.0) == 1.0 and stand.softness(frames, legs, 0.5) == 0.0


def test_held_bend_is_taken_out_and_the_sway_about_it_kept():
    # A lower back held 12 degrees back over its rest, swaying 2 either way about the body's
    # left line and 3 about up: straightened, the frames sway about rest by as much.
    rest = Quaternion(Vector((0.0, 0.0, 1.0)), math.radians(5.0))
    left = Vector((1.0, 0.0, 0.0))
    sways = [Quaternion(left, math.radians(a)) @ Quaternion(Vector((0.0, 0.0, 1.0)), math.radians(b))
             for a, b in ((2.0, 0.0), (-2.0, 0.0), (0.0, 3.0), (0.0, -3.0))]
    hold = Quaternion(left, math.radians(-12.0))
    rels = [hold @ s @ rest for s in sways]
    held = stand.held_bend(rels, rest)
    assert abs(math.degrees(held.angle) - 12.0) < 0.05, math.degrees(held.angle)
    for s, r in zip(sways, rels):
        off = stand.straightened(r, held) @ rest.inverted()
        assert abs(math.degrees(off.rotation_difference(s).angle)) < 0.05
    # A back already at rest is left as it is, whichever sign its quaternions carry.
    assert stand.held_bend([rest, -rest], rest).angle < 1e-3


TESTS = [v for k, v in sorted(globals().items()) if k.startswith("test_")]
failed = 0
for t in TESTS:
    try:
        t()
        print(f"ok {t.__name__}")
    except AssertionError as e:
        failed += 1
        print(f"FAIL {t.__name__} {e}")
print(f"STANDTEST {'pass' if not failed else 'fail'} {len(TESTS) - failed}/{len(TESTS)}")
