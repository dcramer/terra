"""A tool's haft thinned to a real thickness, inside Blender: a source whose handle is too thick
for its length (a chunky stylised tool, a pickaxe's 8.6 cm where a real one's is about 4) has it
thinned about its line to that thickness at the grip, swelling back to the source's own over
the HAFT_BLEND_M under its head, as a real haft swells into the eye. The held look
(fit_held.py, its data's "haft") and the world prop (tools/props refine.py, its prop.json's
"haft") are thinned by this one function, so the two agree."""

# A haft's thinning blends back to the source's thickness over this much under its head (m);
# the head starts where a section is HEAD_RATIO times the grip's across (detect.ts's HEAD_RATIO).
HAFT_BLEND_M = 0.15
HEAD_RATIO = 1.5


def cuts(ob, z):
    """Where the plane at height z cuts the mesh's edges."""
    vs = ob.data.vertices
    out = []
    for e in ob.data.edges:
        a, b = vs[e.vertices[0]].co, vs[e.vertices[1]].co
        if (a.z < z) == (b.z < z):
            continue
        out.append(a + (b - a) * ((a.z - z) / (a.z - b.z)))
    return out


def line_at(ob, z):
    """The handle's line at height z: the middle of its section there, (x, y)."""
    cs = cuts(ob, z)
    return ((min(c.x for c in cs) + max(c.x for c in cs)) / 2, (min(c.y for c in cs) + max(c.y for c in cs)) / 2)


def thin_haft(ob, haft, grip=0.0, about=(0.0, 0.0)):
    """The handle, running up +Z (Blender's), thinned about its line through `about` (x, y) to
    `haft` metres across its widest at the height `grip`; full below the blend, the source's at
    the head. Returns the width it found at the grip (m) and where its head starts."""
    vs = ob.data.vertices

    def width(z):
        cs = cuts(ob, z)
        return max(max(c.x for c in cs) - min(c.x for c in cs), max(c.y for c in cs) - min(c.y for c in cs)) if cs else 0

    across = width(grip)
    top = max(v.co.z for v in vs)
    start = int(round(grip * 100))
    head = next((z / 100 for z in range(start + 1, int(top * 100)) if width(z / 100) > HEAD_RATIO * across), top)
    k = haft / across
    ax, ay = about
    for v in vs:
        if v.co.z >= head:
            continue
        f = k if v.co.z <= head - HAFT_BLEND_M else k + (1 - k) * (v.co.z - head + HAFT_BLEND_M) / HAFT_BLEND_M
        if about == (0.0, 0.0):
            v.co.x *= f
            v.co.y *= f
        else:
            v.co.x = ax + (v.co.x - ax) * f
            v.co.y = ay + (v.co.y - ay) * f
    print("HAFT", ob.name, f"{across * 100:.1f} cm across at the grip -> {haft * 100:.1f}, its head from {head:.2f} m")
    return across, head
