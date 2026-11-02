"""Tests of the body's stretch (stretch.py) and the proportion check (proportions.py, D593) on a
stick figure whose measures are known by construction, in Blender's Python (for numpy):

    tools/blender/run.sh tools/characters/test_stretch.py
"""

import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import proportions  # noqa: E402
import stretch  # noqa: E402

JOINTS = {
    "thigh_l": (0.10, 0.0, 0.93), "thigh_r": (-0.10, 0.0, 0.93),
    "foot_l": (0.12, 0.0, 0.09), "foot_r": (-0.12, 0.0, 0.09),
    "upperarm_l": (0.17, 0.0, 1.45), "upperarm_r": (-0.17, 0.0, 1.45),
    "hand_l": (0.62, 0.0, 1.45), "hand_r": (-0.62, 0.0, 1.45),
}


def figure():
    """Skin points and edges: the head's top, soles on the floor, fingertips 22 cm past the
    wrists, the crotch an edge across the middle at 0.82 m, and each leg's line sampled."""
    pts = [(0, 0, 1.80), (0.12, 0.05, 0.0), (-0.12, 0.05, 0.0), (0.84, 0, 1.45), (-0.84, 0, 1.45), (0.02, 0, 0.82), (-0.02, 0, 0.82)]
    for s in (1, -1):
        for t in np.linspace(0, 1, 50):
            pts.append((s * (0.10 + 0.02 * t), 0.0, 0.93 - 0.84 * t))
    return np.array(pts, dtype=float), np.array([[5, 6]])


def test_the_stretch_reaches_its_span_at_its_height():
    P, E = figure()
    r = stretch.plan(P, E, JOINTS, 1.80)
    Q = stretch.apply(r, P)
    assert abs((Q[:, 0].max() - Q[:, 0].min()) - stretch.REACH * 1.80) < 1e-9
    assert abs((Q[:, 2].max() - Q[:, 2].min()) - 1.80) < 1e-9
    assert abs(Q[:, 2].min() - P[:, 2].min()) < 1e-9


def test_each_leg_grows_by_the_stretch_along_its_own_line():
    P, E = figure()
    r = stretch.plan(P, E, JOINTS, 1.80)
    for s in "lr":
        hip, ankle = np.array(JOINTS[f"thigh_{s}"]), np.array(JOINTS[f"foot_{s}"])
        h, a = stretch.apply(r, hip), stretch.apply(r, ankle)
        assert abs(np.linalg.norm(a - h) - stretch.LEGS * r["scale"] * np.linalg.norm(ankle - hip)) < 1e-9
        d0, d1 = (ankle - hip) / np.linalg.norm(ankle - hip), (a - h) / np.linalg.norm(a - h)
        assert d0 @ d1 > 1 - 1e-12


def test_the_warp_has_no_seam_across_the_middle_or_down_the_leg():
    P, E = figure()
    r = stretch.plan(P, E, JOINTS, 1.80)
    rng = np.random.default_rng(1)
    A = rng.uniform((-0.2, -0.1, 0.0), (0.2, 0.1, 1.0), (4000, 3))
    B = A + rng.normal(0, 1, A.shape) * 1e-4
    moved = np.linalg.norm(stretch.apply(r, A) - stretch.apply(r, B), axis=1) / np.linalg.norm(A - B, axis=1)
    assert moved.max() < 1.5  # a seam would tear two points 0.1 mm apart by centimetres


def test_what_lies_above_the_hips_and_inside_the_shoulders_only_rises_and_scales():
    P, E = figure()
    r = stretch.plan(P, E, JOINTS, 1.80)
    p = np.array([0.05, 0.03, 1.30])
    q = stretch.apply(r, p)
    c = np.array([*r["centre"], r["floor"]])
    assert np.allclose(q, c + (p + (0, 0, r["lift"]) - c) * r["scale"])


def test_a_model_of_the_same_person_elsewhere_lines_up_where_the_body_went():
    P, E = figure()
    r = stretch.plan(P, E, JOINTS, 1.80)
    assert np.allclose(stretch.line_up(r, P + (0.3, -0.2, 0.5)), stretch.apply(r, P))


def test_the_check_measures_the_figure_and_fails_its_short_arms():
    P, E = figure()
    m = proportions.measure(P, E, JOINTS)
    want = {"height": 1.80, "reach": 1.68, "fingertip": 1.45 - 0.45 - 0.22, "wrist": 1.00, "crotch": 0.82}
    assert all(abs(m[k] - v) < 1e-9 for k, v in want.items())
    _, faults = proportions.judge(m)
    assert [f.split()[0] for f in faults] == ["reach", "fingertip"]


def test_the_stretched_figure_has_a_persons_reach():
    P, E = figure()
    r = stretch.plan(P, E, JOINTS, 1.80)
    J = {k: stretch.apply(r, v) for k, v in JOINTS.items()}
    _, faults = proportions.judge(proportions.measure(stretch.apply(r, P), E, J))
    assert not any(f.startswith("reach") for f in faults)


def test_a_body_of_quaternius_is_known_by_its_mesh_name_and_ours_is_not():
    # Their man's body node and mesh as Blender names them on import (D599: reported, not failed).
    assert proportions.theirs("RegularMale", "RegularMale.001")
    assert proportions.theirs("Body", "Female_Regular.001")
    assert not proportions.theirs("Body", "Body.001")


if __name__ == "__main__":
    tests = [(k, v) for k, v in sorted(globals().items()) if k.startswith("test_")]
    failed = 0
    for name, fn in tests:
        try:
            fn()
            print("ok", name)
        except AssertionError as e:
            failed += 1
            print("FAIL", name, e)
    print(f"TESTS {len(tests) - failed}/{len(tests)} pass")
    sys.exit(1 if failed else 0)
