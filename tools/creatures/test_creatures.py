"""Tests of the creature check's measures and the gait and weight rules (measures.py), on
made-up motion and skin whose answer is known another way. Blender's Python has numpy:

    tools/blender/run.sh tools/creatures/test_creatures.py
"""

import math
import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import measures as ms  # noqa: E402


def stride_feet(frames=60, pace=1.0, duty=0.6, drift=0.0, peel=0.0, skid=0.0):
    """Four feet treading in place at `pace` m/s, a frame 1/60 s, each a toe and a heel 5 cm
    behind it; one foot drifts `drift` metres sideways over its stance, or `skid` metres forward
    over its first tenth (a touch-down skid); every heel rises `peel` degrees about its toe over
    the last fifth of the stance. Returns each foot's (F, 2, 3) skin over one loop (last frame =
    first)."""
    feet = []
    cycle = frames / 60.0
    sweep = pace * duty * cycle
    for k, phase in enumerate((0.0, 0.25, 0.5, 0.75)):
        foot = []
        for f in range(frames + 1):
            p = ((f / frames) - phase) % 1.0
            th = 0.0
            if p < duty:
                u = p / duty
                y, z = -sweep / 2 + sweep * u, 0.0
                x = drift * u if k == 0 else 0.0
                y -= skid * min(u / 0.1, 1.0) if k == 0 else 0.0
                th = math.radians(peel) * min(max((u - 0.8) / 0.2, 0.0), 1.0)
            else:
                v = (p - duty) / (1 - duty)
                y, z, x = ms.swing_y(v, sweep, duty), 0.05 * math.sin(math.pi * v), 0.0
                y -= skid if k == 0 else 0.0
            toe = np.array((x + k, y, z))
            foot.append((toe, toe + 0.05 * np.array((0.0, math.cos(th), math.sin(th)))))
        feet.append(np.array(foot))
    return feet


def test_foot_slide_finds_pace_and_slip():
    mid, whole, pace, steps = ms.foot_slide(stride_feet(), 0.002, True)
    assert abs(pace - 1.0) < 0.02, pace
    assert whole < 0.002, whole
    assert steps == 4, steps
    mid, whole, _, _ = ms.foot_slide(stride_feet(drift=0.03), 0.002, True)
    assert 0.02 < mid < 0.04 and whole < 0.04, (mid, whole)
    # A heel peeling 40 degrees about a held toe is not a slide (the foot's mean moves ~1 cm).
    mid, whole, _, _ = ms.foot_slide(stride_feet(peel=40), 0.002, True)
    assert whole < 0.002, whole
    # A skid as the foot lands is not mid-stance, but the whole stance has it.
    mid, whole, _, _ = ms.foot_slide(stride_feet(skid=0.02), 0.002, True)
    assert mid < 0.003 and 0.015 < whole < 0.03, (mid, whole)


def test_swing_has_no_velocity_jump():
    sweep, duty, h = 0.3, 0.6, 1e-5
    carry = sweep * (1 - duty) / duty  # the stance's velocity per unit of swing
    for v in (0.0, 1.0 - h):
        dv = (ms.swing_y(v + h, sweep, duty) - ms.swing_y(v, sweep, duty)) / h
        assert abs(dv - carry) < 1e-3, (v, dv, carry)
    assert abs(ms.swing_y(0.0, sweep, duty) - sweep / 2) < 1e-12
    assert abs(ms.swing_y(1.0, sweep, duty) + sweep / 2) < 1e-12
    # A gallop's short stance: the foot still leaves and meets the ground at its speed (four
    # sweeps a swing), yet goes little past either end of the stance (a cubic with those
    # tangents flings it 0.38 sweeps out).
    carry = sweep * 0.8 / 0.2
    for v in (0.0, 1.0 - h):
        dv = (ms.swing_y(v + h, sweep, 0.2) - ms.swing_y(v, sweep, 0.2)) / h
        assert abs(dv - carry) < 1e-3, (v, dv, carry)
    ys = [ms.swing_y(i / 400, sweep, 0.2) for i in range(401)]
    assert max(ys) < 0.75 * sweep and min(ys) > -0.75 * sweep, (max(ys), min(ys))


def test_kinks_tell_a_snap_from_a_sine():
    t = np.arange(60) / 60.0
    sine = np.stack([np.sin(2 * np.pi * t), np.zeros(60), np.zeros(60)], axis=1)[:, None, :] * 0.2
    n, worst = ms.kinks(sine)
    assert n == 0 and worst < 0.2, (n, worst)
    tri = np.stack([0.4 * np.abs((t * 2) % 2 - 1), np.zeros(60), np.zeros(60)], axis=1)[:, None, :]
    n, worst = ms.kinks(tri)
    assert n >= 2 and worst > 1.5, (n, worst)


def test_loop_gap_and_runs():
    a = np.zeros((5, 3))
    b = a + (0.1, 0.2, 0.0)  # root motion carries the whole skin: no gap
    assert ms.loop_gap(a, b) < 1e-9
    b[2, 2] += 0.005
    assert abs(ms.loop_gap(a, b) - 0.005) < 1e-9
    # A loop a frame short of closing: a point on a circle, 60 frames, the 60th never drawn,
    # wraps as smoothly as any step, and a jump at the wrap still shows.
    ring = [np.array([[math.cos(2 * math.pi * f / 60), math.sin(2 * math.pi * f / 60), 0.0], [0, 0, 0]]) for f in range(60)]
    hardest = max(ms.turn(ring[k - 1], ring[k], ring[k + 1]) for k in range(1, 59))
    assert ms.loop_gap(ring[0], ring[59]) > 0.05
    assert ms.loop_gap(ring[0], ring[59], ring[58], hardest) < 1e-6
    up = np.array([0.0, 0.0, 0.05])
    assert ms.loop_gap(ring[0], ring[59] + up, ring[58] + up, hardest) > 0.03
    assert ms.runs([1, 1, 0, 0, 1], True) == [(4, 7)]
    assert ms.runs([1, 1, 0, 0, 1], False) == [(0, 2), (4, 5)]


def test_hats_share_by_nearness():
    w = ms.hats(0.25, [0.0, 1.0, 2.0])
    assert np.allclose(w, [0.75, 0.25, 0.0])
    assert np.allclose(ms.hats(-5, [0.0, 1.0]), [1, 0]) and np.allclose(ms.hats(5, [1.0, 0.0]), [1, 0])


def test_feet_found_from_the_skin():
    rng = np.random.default_rng(1)
    pts = [rng.uniform((-0.3, -0.5, 0.2), (0.3, 0.5, 0.8), (400, 3))]
    for x, y in ((-0.2, -0.4), (0.2, -0.4), (-0.2, 0.4), (0.2, 0.4)):
        pts.append(np.array((x, y, 0.0)) + rng.uniform(-0.02, 0.02, (20, 3)) * (1, 1, 0.5))
    rest = np.concatenate(pts)
    feet = ms.foot_clusters(rest, 0.8, 1.0)
    assert len(feet) == 4 and all(len(f) == 20 for f in feet), [len(f) for f in feet]


def test_crease_reads_a_fold_not_a_turn():
    # Two triangles hinged on the edge 0-1 along x, flat at rest; then the second folded 90
    # degrees up about that edge; then the whole flat strip turned and moved, which folds nothing.
    rest = np.array([(0, 0, 0), (1, 0, 0), (0.5, -1, 0), (0.5, 1, 0)], float)
    tris = [(0, 1, 2), (1, 0, 3)]
    pairs, edges = ms.hinges(tris)
    assert pairs.tolist() == [[0, 1]] and edges.tolist() == [[0, 1]]
    r0 = ms.folds(rest, tris, pairs)
    folded = rest.copy()
    folded[3] = (0.5, 0, 1)
    assert abs(ms.crease(r0, ms.folds(folded, tris, pairs)) - 90.0) < 1e-6
    c, s = math.cos(0.7), math.sin(0.7)
    turned = rest @ np.array([[c, -s, 0], [s, c, 0], [0, 0, 1]]).T + (3, 1, 2)
    assert ms.crease(r0, ms.folds(turned, tris, pairs)) < 1e-6
    # Flipped back onto itself, a flap reads near 180.
    flap = rest.copy()
    flap[3] = (0.5, -1, 0.01)
    assert ms.crease(r0, ms.folds(flap, tris, pairs)) > 175.0


def test_zones_give_the_belly_to_the_trunk():
    # A body along y from 0 to 1 at z 0.5, a leg down from (0.2, 0.2, 0.5) to the ground.
    co = [(0.0, 0.5, 0.3),   # belly, mid-way between the legs, far from the leg
          (0.2, 0.2, 0.25),  # on the leg
          (0.0, 0.2, 0.4)]  # 0.2 m off the leg, between near and far: some of each
    weights = {0: {"Thigh.L": 0.8, "Hips": 0.2}, 1: {"Thigh.L": 1.0}, 2: {"Thigh.L": 1.0}}
    legs = [(["Thigh.L"], [(0.2, 0.2, 0.5), (0.2, 0.2, 0.0)], 0.5)]
    moved = ms.zone_weights(co, weights, legs, ["Hips", "Spine"], [0.0, 0.5, 1.0])
    assert moved == 2, moved
    assert "Thigh.L" not in weights[0] and abs(sum(weights[0].values()) - 1.0) < 1e-9
    assert abs(weights[0]["Spine"] - 0.8) < 1e-9, weights[0]
    assert weights[1] == {"Thigh.L": 1.0}
    assert 0.0 < weights[2]["Thigh.L"] < 1.0 and abs(sum(weights[2].values()) - 1.0) < 1e-9
    # A band in metres widens a narrow ease (near 0.05, far 0.2 of a 0.5 m bone: 7.5 cm, a
    # step), so the flank beside the leg keeps some of it, and the far belly still none.
    def flank(band):
        w = {0: {"Thigh.L": 1.0}, 1: {"Thigh.L": 0.8, "Hips": 0.2}}
        ms.zone_weights([(0.0, 0.2, 0.42), (0.0, 0.5, 0.3)], w, legs, ["Hips", "Spine"], [0.0, 0.5, 1.0], 0.05, 0.2, band)
        return w[0].get("Thigh.L", 0.0), w[1].get("Thigh.L", 0.0)
    assert flank(0.0) == (0.0, 0.0), flank(0.0)
    assert 0.2 < flank(0.3)[0] < 1.0 and flank(0.3)[1] == 0.0, flank(0.3)
    # A broad paw's edge, far from the leg's bones but below its wrist (z 0.25), stays the leg's.
    paw = {0: {"Thigh.L": 1.0}}
    assert ms.zone_weights([(0.0, 0.2, 0.0)], paw, [legs[0] + (0.25,)], ["Hips", "Spine"], [0.0, 0.5, 1.0]) == 0
    assert paw[0] == {"Thigh.L": 1.0}


def test_twist_counts_turn_about_the_bone_not_bend():
    def rot(axis, deg):
        c, s = math.cos(math.radians(deg)), math.sin(math.radians(deg))
        return {"x": np.array([[1, 0, 0], [0, c, -s], [0, s, c]]), "y": np.array([[c, 0, s], [0, 1, 0], [-s, 0, c]]),
                "z": np.array([[c, -s, 0], [s, c, 0], [0, 0, 1]])}[axis]

    up = (0.0, 1.0, 0.0)
    for bend in (10, 90, 150, 179):  # a knee bent, however far: no twist
        assert ms.twist_about(rot("x", bend), up) < 1e-4
        assert ms.twist_about(rot("z", bend), (1.0, 0.0, 0.0)) < 1e-4
    assert abs(ms.twist_about(rot("y", 60), up) - 60) < 1e-4
    assert abs(ms.twist_about(rot("x", 60) @ rot("y", 20), up) - 20) < 2  # twist, then bend
    assert abs(ms.twist_about(rot("x", 170), (1.0, 0.0, 0.0)) - 170) < 1e-4  # a half turn reads right
    # Folded 175 degrees about an axis 5 degrees off square to the bone: a fold, not a twist.
    tilted = rot("y", 5) @ rot("x", 175) @ rot("y", -5)
    assert ms.twist_about(tilted, (0.0, 0.0, 1.0)) < 20


tests = [f for n, f in sorted(globals().items()) if n.startswith("test_")]
for f in tests:
    f()
    print("ok", f.__name__)
print(f"creature tests: {len(tests)} passed")
