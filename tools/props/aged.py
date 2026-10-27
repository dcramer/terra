"""Aged bone's and old stone's grime, as a vertex colour (Blender): the barrow kit's built pieces
(barrow.py) and refined models that ask for it (refine.py --grime) darken in their crevices
and where they lie on the floor, with blotches of stain over the rest. Imported, never run."""

import math

from mathutils import Vector, noise
from mathutils.bvhtree import BVHTree

# How far a vertex looks for what shades it, metres (about a skull's radius); the dirt's colour
# in the crevices and where it lay on the earth, against the bone's own (as a multiply of its
# texture and tint); and the stain's blotches, a metre across so many. Old bone is never the
# white of a museum's cast: ivory to tobacco brown, darker and earthier wherever it touched the
# ground or another bone (barrow round 12: "a smooth white egg of a skull, the brightest thing
# in frame").
GRIME_REACH = 0.12
GRIME = (0.22, 0.17, 0.11)
STAIN_SCALE = 14.0
GRIME_RAYS = 64


def hemisphere(n):
    """`n` directions spread evenly over the sphere (a Fibonacci spiral), the same every run."""
    golden = math.pi * (3.0 - math.sqrt(5.0))
    out = []
    for i in range(n):
        z = 1.0 - (i + 0.5) / n * 2.0
        r = math.sqrt(max(0.0, 1.0 - z * z))
        out.append(Vector((math.cos(golden * i) * r, math.sin(golden * i) * r, z)))
    return out


def grime(bm, me, tag="BARROW"):
    """Each vertex's colour, the dirt of old bone: its ambient occlusion over `GRIME_REACH`
    (rays against the mesh itself and the floor at z 0, cosine weighted, as a baker's AO), so
    the crevices between bones, the skull's underside and the sockets' rims go earth brown, and
    blotches of stain over the rest. `bm` is `me`'s mesh, vertex for vertex. Written to the
    mesh's colour attribute, which the glTF carries as COLOR_0 (export_vertex_color "ACTIVE")
    and the game multiplies into the colour."""
    bm.verts.ensure_lookup_table()
    bm.normal_update()
    tree = BVHTree.FromBMesh(bm)
    dirs = hemisphere(GRIME_RAYS * 2)
    shades = []
    for v in bm.verts:
        n = v.normal
        p = v.co + n * 0.002
        hit = total = 0.0
        for d in dirs:
            w = d.dot(n)
            if w <= 0.0:
                continue
            total += w
            near = GRIME_REACH
            loc, _, _, dist = tree.ray_cast(p, d, GRIME_REACH)
            if loc is not None:
                near = min(near, dist)
            if d.z < 0.0:
                near = min(near, -p.z / d.z)
            hit += w * (1.0 - near / GRIME_REACH)
        open_ = 1.0 - hit / max(total, 1e-6)
        stain = 0.5 + 0.5 * noise.noise(v.co * STAIN_SCALE)
        # Darker in the crevices (open_ under 1), a little everywhere by the stain: a bone's
        # side over the floor, a third shut in, takes some 0.4 of the dirt, a crevice all.
        dirt = min(1.0, (1.0 - open_) * 1.4) ** 1.2
        bone = 0.78 + 0.22 * stain
        shades.append([bone * (1.0 - dirt) + g * dirt for g in GRIME])
    layer = me.color_attributes.new("Grime", "FLOAT_COLOR", "POINT")
    for i, c in enumerate(shades):
        layer.data[i].color = (*c, 1.0)
    me.color_attributes.active_color = layer
    lumas = sorted(0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2] for c in shades)
    print(f"{tag} grime: {len(lumas)} vertices, luma median {lumas[len(lumas) // 2]:.2f}, "
          f"tenth {lumas[len(lumas) // 10]:.2f}, least {lumas[0]:.2f}")
