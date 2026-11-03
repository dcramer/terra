"""Tests of laying straps over a figure dressed in pieces (bands.py, look.worn_over), on shapes
whose answer is known by construction, in Blender's Python:

    tools/blender/run.sh tools/characters/test_bands.py
"""

import os
import sys

import bpy
from mathutils import Vector

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import bands  # noqa: E402


def quad(y):
    """A square two metres across, upright, facing -Y, at `y`."""
    return [(-1, y, -1), (1, y, -1), (1, y, 1), (-1, y, 1)]


def figure(whole):
    """A figure whose kept faces are a back wall at y = 0.5; with `whole`, every face it had
    also holds a front wall at y = 0, left out of the kept ones (a slit's floor, say)."""
    kept = bpy.data.meshes.new("kept")
    kept.from_pydata(quad(0.5), [], [(0, 1, 2, 3)])
    body = bpy.data.objects.new("Figure", kept)
    bpy.context.scene.collection.objects.link(body)
    if whole:
        every = bpy.data.meshes.new("every")
        every.from_pydata(quad(0.0) + quad(0.5), [], [(0, 1, 2, 3), (4, 5, 6, 7)])
        body["whole"] = every.name
    return body


def test_a_ray_from_outside_lands_on_the_first_face_of_the_whole_figure_not_through_a_slit():
    ray = [((0.0, -1.0, 0.0), (0.0, 1.0, 0.0))]
    assert abs(bands.path(figure(whole=False), ray)[0].y - 0.5) < 1e-6
    assert abs(bands.path(figure(whole=True), ray)[0].y) < 1e-6


def test_a_face_turned_in_is_turned_out_from_the_nearest_bone():
    bones = [(Vector((0, 0, 0)), Vector((0, 0, 1))), (Vector((0, 0, 1)), Vector((0.5, 0, 1)))]
    side = Vector((0.15, 0, 0.5))
    assert bands.outward(Vector((-1, 0, 0)), side, bones) == Vector((1, 0, 0))
    assert bands.outward(Vector((1, 0, 0)), side, bones) == Vector((1, 0, 0))
    # Over the arm's bone, out is up from it, not away from the spine.
    over_arm = Vector((0.4, 0, 1.1))
    assert bands.outward(Vector((0, 0, -1)), over_arm, bones) == Vector((0, 0, 1))


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
