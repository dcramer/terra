"""The creature check's measures as plain functions of point tracks, and the weight zones'
falloffs (D437, D441): no Blender here, so test_creatures.py proves each on made-up motion
where the answer is known another way. Lengths are in metres, a frame is 1/60 s."""

import numpy as np


def smooth(a, b, x):
    """Smoothstep: 0 at or below a, 1 at or above b, with no kink at either end."""
    t = np.clip((np.asarray(x, float) - a) / (b - a), 0.0, 1.0)
    return t * t * (3 - 2 * t)


def keep_near(d, near, far):
    """The share of a leg bone's weight a vertex keeps at distance d from that leg's bones: all
    of it within `near`, none past `far`, eased between, so skin round a leg moves with it and
    the belly and flank between the legs go to the trunk."""
    return 1.0 - smooth(near, far, d)


def hats(y, knots):
    """Linear hat weights of a position along a chain of knots (each bone's head, in order,
    then the last bone's tail): the two knots round y share it by nearness, ends clamped.
    Returns one weight a knot, summing to 1."""
    k = np.asarray(knots, float)
    order = np.argsort(k)
    ks = k[order]
    w = np.zeros(len(k))
    if y <= ks[0]:
        w[order[0]] = 1.0
    elif y >= ks[-1]:
        w[order[-1]] = 1.0
    else:
        i = int(np.searchsorted(ks, y)) - 1
        f = (y - ks[i]) / (ks[i + 1] - ks[i]) if ks[i + 1] > ks[i] else 0.0
        w[order[i]], w[order[i + 1]] = 1.0 - f, f
    return w


def segment_distance(p, a, b):
    """Distance of points p (n, 3) to the segment a-b."""
    ab = np.asarray(b, float) - np.asarray(a, float)
    t = np.clip(((p - a) @ ab) / max(float(ab @ ab), 1e-12), 0.0, 1.0)
    return np.linalg.norm(p - (np.asarray(a, float) + np.outer(t, ab)), axis=1)


def foot_clusters(rest, height, length, most=4):
    """The feet, found from the skin (a rig's feet may hang from IK bones, not the leg chain):
    vertices in the lowest 4% of the body's height, grouped where they lie within 6% of its
    length of each other on the ground plane, the `most` biggest groups."""
    low = np.nonzero(rest[:, 2] < rest[:, 2].min() + 0.04 * height)[0]
    xy = rest[low, :2]
    parent = list(range(len(low)))

    def find(i):
        while parent[i] != i:
            parent[i] = parent[parent[i]]
            i = parent[i]
        return i

    r2 = (0.06 * length) ** 2
    for i in range(len(low)):
        d = ((xy[i + 1:] - xy[i]) ** 2).sum(axis=1)
        for j in np.nonzero(d < r2)[0]:
            a, b = find(i), find(i + 1 + int(j))
            if a != b:
                parent[a] = b
    groups = {}
    for i in range(len(low)):
        groups.setdefault(find(i), []).append(int(low[i]))
    out = sorted((g for g in groups.values() if len(g) >= 3), key=len, reverse=True)[:most]
    return [np.array(g) for g in out]


def runs(mask, cyclic):
    """(start, end) index runs of True, end exclusive; a run wrapping a loop's seam is one run
    whose end passes len(mask)."""
    m = list(bool(x) for x in mask)
    n = len(m)
    out, i = [], 0
    while i < n:
        if m[i]:
            j = i
            while j < n and m[j]:
                j += 1
            out.append([i, j])
            i = j
        else:
            i += 1
    if cyclic and len(out) > 1 and out[0][0] == 0 and out[-1][1] == n:
        first = out.pop(0)
        out[-1][1] = n + first[1]
    elif cyclic and len(out) == 1 and out[0] == [0, n]:
        pass
    return [tuple(r) for r in out]


def foot_slide(tracks, lows, eps, cyclic):
    """How far a planted foot slides over the ground, in metres, and the pace the clip treads.
    tracks: per foot (F, 3) positions; lows: per foot (F,) its lowest point. A foot is down
    where its lowest point is within eps of its ground, the 10th percentile of its lowest (not
    the clip's minimum: a hoof dipping a centimetre at touch-down would leave the whole stance
    out). Locomotion is in place, so a planted foot is carried back at the clip's pace: the pace
    is the median velocity of every foot over every frame it is down, and a foot slides by how
    far it drifts from that carry over each time down. A loop's last frame is its first again,
    so it is dropped."""
    if cyclic:
        tracks = [t[:-1] for t in tracks]
        lows = [l[:-1] for l in lows]
    vels, spans = [], []
    for tr, lo in zip(tracks, lows):
        n = len(tr)
        for a, b in runs(lo < np.percentile(lo, 10) + eps, cyclic):
            # A tenth of each time down at either end is left out, at least a frame: a foot a
            # hair off the ground, still landing or already easing into its swing, is not a
            # planted foot sliding.
            k = max(1, int(round(0.1 * (b - a))))
            a, b = a + k, b - k
            if b - a < 3:
                continue
            idx = np.arange(a, b) % n
            p = tr[idx, :2]
            vels.append(np.diff(p, axis=0))
            spans.append(p)
    if not spans:
        return 0.0, 0.0, 0
    pace = np.median(np.concatenate(vels), axis=0)
    worst = 0.0
    for p in spans:
        steps = np.arange(len(p))[:, None]
        drift = p - p[0] - steps * pace
        worst = max(worst, float(np.linalg.norm(drift, axis=1).max()))
    return worst, float(np.linalg.norm(pace)) * 60.0, len(spans)


def loop_gap(first, last, prev=None, turn=0.0):
    """How far a loop jumps where it wraps, in metres. A loop either ends on its first frame
    again, so the gap is how far the skin's last frame is from its first; or it stops a frame
    short (Quaternius's clips do), so glTF's sampler steps from the last frame to the first as
    from any frame to the next, and the gap is how much harder that step turns (its second
    difference, from `prev`, the frame before the last) than the clip's own hardest turn,
    `turn`. The smaller of the two is the gap. A clip with root motion carries the whole skin
    over the ground, so the skin's mean move over the ground is taken off each step first."""

    def flat(d):
        d = np.array(d, float)
        d[:, :2] -= d[:, :2].mean(axis=0)
        return d

    closed = float(np.linalg.norm(flat(last - first), axis=1).max())
    if prev is None:
        return closed
    step = flat(first - last) - flat(last - prev)
    return min(closed, max(0.0, float(np.linalg.norm(step, axis=1).max()) - turn))


def turn(prev, cur, nxt):
    """The skin's hardest turn between three frames: the largest second difference of a vertex,
    which a constant root motion leaves out."""
    return float(np.linalg.norm(nxt - 2 * cur + prev, axis=1).max())


def kinks(tails, floor=0.01):
    """Velocity kinks of a clip's bone tails (F, B, 3), one frame apart: frames where a tail's
    velocity changes by over half its top speed in the clip and over `floor` metres a frame,
    as a limb snapping at lift-off or touch-down does; and the worst change over the top speed.
    A loop's tracks should be given with the last frame (its first again) dropped; they wrap."""
    v = np.roll(tails, -1, axis=0) - tails
    top = np.linalg.norm(v, axis=2).max(axis=0)
    dv = np.linalg.norm(v - np.roll(v, 1, axis=0), axis=2)
    bad = (dv > np.maximum(floor, 0.5 * top)[None, :])
    worst = float((dv / np.maximum(top, 1e-9)[None, :]).max()) if len(tails) > 2 else 0.0
    return int(bad.any(axis=1).sum()), worst


def stretch(rest_len, posed_len, q=95):
    """The q-th percentile of how far edges stretch or shrink from rest, as a share. Not the 99th:
    a low-poly barrel has a couple of hundred edges, so its 99th is its worst two edges."""
    if len(rest_len) == 0:
        return 0.0
    return float(np.percentile(np.abs(posed_len / rest_len - 1.0), q))


def polyline_distance(p, points):
    """Distance of points p (n, 3) to a polyline through `points`."""
    d = np.full(len(p), np.inf)
    for a, b in zip(points, points[1:]):
        d = np.minimum(d, segment_distance(p, a, b))
    return d


def zone_weights(co, weights, legs, trunk, knots, near=0.25, far=0.6):
    """Zoned torso and belly weights (D441), as a rigger paints a runner: bone heat hands the
    belly and flanks between the legs to the leg bones, so each stride drags the belly into a
    pouch. Each leg's bones keep their weight on a vertex by `keep_near` of its distance to that
    leg's bone polyline, `near` and `far` being fractions of the leg's upper bone length; what
    they lose goes to the trunk chain (`trunk`, rump to chest) by linear hats along it, at the
    knots' y (each trunk bone's head, then the last one's tail).
    co: (n, 3) vertices; weights: {vertex: {bone: w}}, changed in place;
    legs: [(bones, polyline points, upper bone length)]. Returns how many vertices gave weight."""
    co = np.asarray(co, float)
    owner = list(trunk) + [trunk[-1]]
    loss = np.zeros(len(co))
    for leg_bones, points, upper in legs:
        keep = keep_near(polyline_distance(co, np.asarray(points, float)), near * upper, far * upper)
        for v in np.nonzero(keep < 1.0)[0]:
            ws = weights.get(int(v))
            if not ws:
                continue
            for b in leg_bones:
                if b in ws:
                    loss[v] += ws[b] * (1.0 - keep[v])
                    ws[b] *= keep[v]
                    if ws[b] <= 1e-4:
                        del ws[b]
    moved = np.nonzero(loss > 1e-6)[0]
    for v in moved:
        ws = weights[int(v)]
        for b, h in zip(owner, hats(co[v, 1], knots)):
            if h > 0:
                ws[b] = ws.get(b, 0.0) + loss[v] * h
    return len(moved)


def swing_y(v, sweep, duty, cap=2.5):
    """Where a swinging foot is along the body (+y back) at v of its swing, for a stance that
    carries it from -sweep/2 to +sweep/2 over `duty` of the stride: a cubic Hermite from the
    lift-off to the touch-down whose end tangents are the stance's carry, capped at `cap` sweeps,
    so the foot's velocity has no jump at either end."""
    p0, p1 = sweep / 2, -sweep / 2
    m = min(sweep * (1 - duty) / duty, cap * sweep)
    return (p0 * (2 * v ** 3 - 3 * v ** 2 + 1) + p1 * (-2 * v ** 3 + 3 * v ** 2)
            + m * (v ** 3 - 2 * v ** 2 + v) + m * (v ** 3 - v ** 2))


def twist_about(r, axis):
    """How far a joint's turn r (3x3) goes about the unit `axis`, its bone, in degrees, 0 to
    180: the turn's rotation vector (its angle along its axis, from the quaternion) taken along
    the bone. Bending is about an axis square to the bone and reads 0, a turn about the bone
    reads whole. Not the swing-twist split, which near a full fold (a wing or hock folded 175
    degrees) gives an axis a few degrees off square up to 180 degrees of twist."""
    r = np.asarray(r, float)
    a = np.asarray(axis, float) / np.linalg.norm(axis)
    t = np.trace(r)
    i = int(np.argmax([t, r[0, 0], r[1, 1], r[2, 2]]))
    if i == 0:
        w = np.sqrt(1.0 + t) / 2
        v = np.array([r[2, 1] - r[1, 2], r[0, 2] - r[2, 0], r[1, 0] - r[0, 1]]) / (4 * w)
    else:
        j, k = [(1, 2), (2, 0), (0, 1)][i - 1]
        i -= 1
        v = np.zeros(3)
        v[i] = np.sqrt(max(0.0, 1.0 + 2 * r[i, i] - t)) / 2
        w = (r[k, j] - r[j, k]) / (4 * v[i])
        v[j] = (r[j, i] + r[i, j]) / (4 * v[i])
        v[k] = (r[k, i] + r[i, k]) / (4 * v[i])
    if w < 0:
        w, v = -w, -v
    n = np.linalg.norm(v)
    if n < 1e-12:
        return 0.0
    angle = 2 * np.degrees(np.arctan2(n, w))
    return float(abs(angle * (v / n) @ a))
