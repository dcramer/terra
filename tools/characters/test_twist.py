"""Tests of how the twist step follows a clip's roll (twist.unwrapped) on rolls whose answer is
known another way, in Blender's Python:

    tools/blender/run.sh tools/characters/test_twist.py
"""

import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import twist  # noqa: E402


def short(a):
    """An angle the short way round, in (-pi, pi], as split() gives it."""
    return math.atan2(math.sin(a), math.cos(a))


def test_a_roll_past_half_a_turn_and_back_is_followed():
    # Out to 200 degrees and back: each frame alone reads -160 at the top, the frames round it
    # do not, so the twist bone turns smoothly through it.
    true = [math.radians(200 * math.sin(math.pi * i / 40)) for i in range(41)]
    got = twist.unwrapped([short(a) for a in true])
    assert max(abs(g - t) for g, t in zip(got, true)) < 1e-9


def test_a_loop_that_winds_a_whole_turn_is_taken_frame_by_frame():
    # A crawl stroke's arm: the roll goes once round while the pose comes back to its start.
    # Followed, the twist bone would end half a turn from where it began; frame by frame no
    # frame rolls past half a turn, and the turn back falls where the roll passes it.
    true = [math.radians(7 - 360 * i / 39) for i in range(40)]
    got = twist.unwrapped([short(a) for a in true])
    assert max(abs(a) for a in got) <= math.pi
    assert abs(got[-1] - got[0]) < math.radians(1)
    jumps = [i for i in range(1, len(got)) if abs(got[i] - got[i - 1]) > math.pi]
    assert len(jumps) == 1 and abs(math.degrees(true[jumps[0]])) > 175


def test_a_roll_that_stays_within_half_a_turn_is_unchanged():
    true = [math.radians(60 * math.sin(2 * math.pi * i / 30)) for i in range(31)]
    assert twist.unwrapped(list(true)) == true


failed = 0
for name, test in sorted(globals().copy().items()):
    if name.startswith("test_") and callable(test):
        try:
            test()
            print("PASS", name)
        except AssertionError as e:
            failed += 1
            print("FAIL", name, e)
print("TWIST TESTS", "pass" if not failed else f"{failed} failed")
if failed:
    sys.exit(1)
