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


def test_a_winding_loop_flips_once_a_turn_and_owns_it():
    true = [math.radians(7 - 360 * i / 39) for i in range(40)]
    largest, flips, owned = twist.jumps(twist.unwrapped([short(a) for a in true]), 0.5, loop=True)
    assert flips == 1 and owned == 1, (flips, owned)
    # Its frames turn the twist bone half of 360/39 degrees each, the flip measured the short way.
    assert abs(largest - 0.5 * 360 / 39) < 1e-6, largest


def test_a_winding_loop_that_dithers_across_half_a_turn_flips_more_than_it_owns():
    # Once round, but swaying back and forth as it passes half a turn: each crossing a flip.
    true = [math.radians(-360 * i / 80 + 25 * math.sin(2 * math.pi * 4 * i / 80)) for i in range(81)]
    _, flips, owned = twist.jumps(twist.unwrapped([short(a) for a in true]), 0.5, loop=True)
    assert owned == 1 and flips > owned, (flips, owned)


def test_a_clip_that_is_no_loop_owns_no_flip():
    # A one-off roll out past half a turn (0 to 250 degrees) ends over half a turn from its
    # start, as a winding loop does: unwrapped takes it frame by frame, so its twist bone flips.
    true = [math.radians(250 * i / 30) for i in range(31)]
    _, flips, owned = twist.jumps(twist.unwrapped([short(a) for a in true]), 0.5, loop=False)
    assert flips == 1 and owned == 0, (flips, owned)


def test_a_flip_in_a_clips_data_is_a_big_jump():
    # One frame's roll 150 degrees off its neighbours', as a flipped key gives.
    true = [math.radians(20 + (150 if i == 12 else 0)) for i in range(25)]
    largest, flips, _ = twist.jumps(twist.unwrapped(list(true)), 0.5, loop=True)
    assert flips == 0 and abs(largest - 75) < 1e-6, (largest, flips)
    smooth = [math.radians(60 * math.sin(2 * math.pi * i / 30)) for i in range(31)]
    largest, flips, owned = twist.jumps(twist.unwrapped(smooth), 0.5, loop=True)
    assert largest < 7 and flips == 0 and owned == 0, (largest, flips, owned)


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
