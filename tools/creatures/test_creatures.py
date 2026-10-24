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


def stride_tracks(frames=60, pace=1.0, duty=0.6, drift=0.0):
    """Four feet treading in place at `pace` m/s, a frame 1/60 s; one foot drifts `drift` metres
    sideways over its stance. Returns tracks and lows over one loop (last frame = first)."""
    tracks, lows = [], []
    cycle = frames / 60.0
    sweep = pace * duty * cycle
    for k, phase in enumerate((0.0, 0.25, 0.5, 0.75)):
        tr, lo = [], []
        for f in range(frames + 1):
            p = ((f / frames) - phase) % 1.0
            if p < duty:
                u = p / duty
                y, z = -sweep / 2 + sweep * u, 0.0
                x = drift * u if k == 0 else 0.0
            else:
                v = (p - duty) / (1 - duty)
                y, z, x = ms.swing_y(v, sweep, duty), 0.05 * math.sin(math.pi * v) ** 2, 0.0
            tr.append((x + k, y, z))
            lo.append(z)
        tracks.append(np.array(tr))
        lows.append(np.array(lo))
    return tracks, lows


def test_foot_slide_finds_pace_and_drift():
    tracks, lows = stride_tracks()
    slide, pace, steps = ms.foot_slide(tracks, lows, 0.002, True)
    assert abs(pace - 1.0) < 0.02, pace
    assert slide < 0.002, slide
    assert steps == 4, steps
    tracks, lows = stride_tracks(drift=0.03)
    slide, _, _ = ms.foot_slide(tracks, lows, 0.002, True)
    assert 0.02 < slide < 0.04, slide


def test_swing_has_no_velocity_jump():
    sweep, duty, h = 0.3, 0.6, 1e-5
    carry = sweep * (1 - duty) / duty  # the stance's velocity per unit of swing
    for v in (0.0, 1.0 - h):
        dv = (ms.swing_y(v + h, sweep, duty) - ms.swing_y(v, sweep, duty)) / h
        assert abs(dv - carry) < 1e-3, (v, dv, carry)
    assert abs(ms.swing_y(0.0, sweep, duty) - sweep / 2) < 1e-12
    assert abs(ms.swing_y(1.0, sweep, duty) + sweep / 2) < 1e-12
    # A gallop's short stance: capped, still far closer to the carry than the cosine ease's 0.
    dv = (ms.swing_y(h, sweep, 0.2) - ms.swing_y(0.0, sweep, 0.2)) / h
    assert abs(dv - 2.5 * sweep) < 1e-3


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


def test_twist_counts_turn_about_the_bone_not_bend():
    ring = np.array([[math.cos(a), 0.0, math.sin(a)] for a in np.linspace(0, 2 * math.pi, 12, endpoint=False)])

    def rot(axis, deg):
        c, s = math.cos(math.radians(deg)), math.sin(math.radians(deg))
        return {"x": np.array([[1, 0, 0], [0, c, -s], [0, s, c]]), "y": np.array([[c, 0, s], [0, 1, 0], [-s, 0, c]])}[axis]

    assert ms.twist(ring, ring @ rot("x", 70).T) < 1e-6  # a bend: no twist
    assert abs(ms.twist(ring, ring @ rot("y", 25).T) - 25) < 1e-6
    assert abs(ms.twist(ring, ring @ (rot("x", 60) @ rot("y", 20)).T) - 20) < 1e-6  # twist, then bend


tests = [f for n, f in sorted(globals().items()) if n.startswith("test_")]
for f in tests:
    f()
    print("ok", f.__name__)
print(f"creature tests: {len(tests)} passed")
