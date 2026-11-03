"""Bands: straps, belts and the like, made on the body (D128, docs/characters.md).

A band runs along a path over the skin, given by a few rays cast at the body (from its own
joints and measurements, so it fits any body): each lands where it first meets the skin, as a
strap is laid on (down onto the top of a shoulder, in onto the chest), never the nearest skin,
which for a point off the body may be a hand or the head. The path between them is filled in
on the skin; the band is the body's own skin within half its width of
the path, cut along its edges through the faces (skin_piece.py), pushed out a little, keeping
each vertex's skin weights. So a strap moves exactly as
the skin under it does, over a shoulder or round a waist, and cannot come through it.
"""

import bmesh
import bpy
from mathutils import Vector
from mathutils.bvhtree import BVHTree
from mathutils.geometry import intersect_point_line

import skin_piece

# How finely the path is filled in on the skin, in metres.
STEP_M = 0.01
CUT_CLEAR_M = 0.001     # the cut kept this far from the body's vertices (skin_piece.cut)
EDGE_RELAX_ROUNDS = 3   # rounds of evening its edges out


def tree_of(body, mesh=None):
    mw = body.matrix_world
    mesh = mesh or body.data
    return BVHTree.FromPolygons([mw @ v.co for v in mesh.vertices], [tuple(p.vertices) for p in mesh.polygons])


def outside_of(body):
    """The tree rays from outside are cast at: a dressed figure's every face (look.worn_over
    keeps them as its "whole"), since the first face such a ray meets is the outside, and the
    figure less the faces under a layer has slits where one piece tucks under another."""
    whole = body.get("whole")
    return tree_of(body, bpy.data.meshes[whole]) if whole else tree_of(body)


def on_skin(tree, p):
    hit = tree.find_nearest(p)
    return hit[0] if hit[0] is not None else p


def path(body, rays):
    """The path through where `rays` (origin, direction) first meet the body's skin, filled in
    along the skin between them."""
    tree = tree_of(body)
    outside = outside_of(body)
    anchors = []
    for origin, direction in rays:
        # A ray that passes beside a narrower body moves in toward its middle until it meets
        # skin, so the same route fits any body.
        origin = Vector(origin)
        for _ in range(8):
            hit = outside.ray_cast(origin, Vector(direction).normalized(), 3.0)
            if hit[0] is not None:
                break
            origin.x *= 0.85
        if hit[0] is None:
            raise ValueError(f"a band's ray from {tuple(origin)} meets no skin")
        anchors.append(hit[0])
    out = []
    for a, b in zip(anchors[:-1], anchors[1:]):
        steps = max(1, int((b - a).length / STEP_M))
        for k in range(steps):
            out.append(on_skin(tree, a.lerp(b, k / steps)))
    out.append(anchors[-1])
    return out


# Rounds of evening a ribbon's line out, ends held: enough that it turns no tighter than its
# width (some 5 cm round, from the chest down onto the side), where a ribbon folds on itself.
RIBBON_SMOOTH_ROUNDS = 60
RIBBON_ACROSS = 5          # points across a ribbon where the surface under it is found
RIBBON_RAMP_M = 0.06       # a ribbon rises to clear what stands under it over this length


def bone_lines(body):
    """Each bone of `body`'s skeleton at rest, as a line from its head to its tail in the world."""
    arm = body.find_armature()
    return [(arm.matrix_world @ b.head_local, arm.matrix_world @ b.tail_local) for b in arm.data.bones]


def outward(n, p, bones):
    """`n` turned, if need be, to face away from the bone nearest `p`."""
    best = None
    for a, b in bones:
        q, k = intersect_point_line(p, a, b)
        q = a if k < 0 else b if k > 1 else q
        if best is None or (p - q).length < (p - best).length:
            best = q
    return -n if n.dot(p - best) < 0 else n


def ribbon(body, rays, width, offset, name, material, inner):
    """A new object: a strap laid over `body` along `rays`, as a strap is laid over clothes,
    not cut from them: a solid ribbon `width` wide, from `inner` to `offset` off the highest
    surface under its width, rising over a belt or a shoulder flap within RIBBON_RAMP_M rather
    than following each layer's edge, its line evened out. Each vertex takes the skin weights
    of the surface beneath it. For a figure dressed in pieces (look.worn_over), whose low,
    layered outside a band cut from it would follow edge by edge."""
    tree = tree_of(body)
    mw = body.matrix_world
    line = [p.copy() for p in path(body, rays)]
    # Evened out and laid back on the surface in turn: evening alone cuts a corner round the
    # body through it.
    for _ in range(RIBBON_SMOOTH_ROUNDS):
        line = [line[0]] + [(line[i - 1] + 2 * line[i] + line[i + 1]) / 4 for i in range(1, len(line) - 1)] + [line[-1]]
        line = [on_skin(tree, p) for p in line]
    # The surface's facing, turned away from the bones (a face of a figure in pieces may face
    # in: a belt's inner side where it stands off the tunic, which turned the ribbon 5 cm into
    # him), and evened along the line, so the ribbon does not twist where it steps from one
    # layer onto another.
    bones = bone_lines(body)
    normals = [outward(tree.find_nearest(p)[1].normalized(), p, bones) for p in line]
    reach = max(1, int(RIBBON_RAMP_M / STEP_M))
    normals = [sum(normals[max(0, i - reach): i + reach + 1], Vector()).normalized() for i in range(len(line))]
    frames = []
    for i, p in enumerate(line):
        t = (line[min(i + 1, len(line) - 1)] - line[max(i - 1, 0)]).normalized()
        n = normals[i]
        s = t.cross(n).normalized()
        frames.append((p, t, s.cross(t).normalized(), s))
    # The highest surface under the ribbon's width at each point, along its normal.
    height = []
    for p, _, n, s in frames:
        h = 0.0
        for k in range(RIBBON_ACROSS):
            q = p + s * (width * (k / (RIBBON_ACROSS - 1) - 0.5))
            hit = tree.ray_cast(q + n * 0.08, -n, 0.16)
            if hit[0] is not None:
                h = max(h, (hit[0] - p).dot(n))
        height.append(h)
    height = [max(height[max(0, i - reach): i + reach + 1]) for i in range(len(height))]
    for _ in range(reach):
        height = [height[0]] + [max(height[i], (height[i - 1] + height[i] + height[i + 1]) / 3) for i in range(1, len(height) - 1)] + [height[-1]]
    # Its corners: four a point (outside left and right, inside right and left), a closed tube.
    bm = bmesh.new()
    rings = []
    for (p, _, n, s), h in zip(frames, height):
        ring = []
        for lift, side in ((offset, -0.5), (offset, 0.5), (inner, 0.5), (inner, -0.5)):
            ring.append(bm.verts.new(p + n * (h + lift) + s * (width * side)))
        rings.append(ring)
    uv_at = {}
    length = 0.0
    for i, ring in enumerate(rings):
        if i:
            length += (frames[i][0] - frames[i - 1][0]).length
        for k, v in enumerate(ring):
            uv_at[v] = (k in (1, 2), length / width)
    for a, b in zip(rings[:-1], rings[1:]):
        for k in range(4):
            bm.faces.new((a[k], a[(k + 1) % 4], b[(k + 1) % 4], b[k]))
    bm.faces.new(list(reversed(rings[0])))
    bm.faces.new(rings[-1])
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    bm.normal_update()
    uv = bm.loops.layers.uv.new("UVMap")
    for f in bm.faces:
        for loop in f.loops:
            u, v = uv_at[loop.vert]
            loop[uv].uv = (float(u), v)
    me = bpy.data.meshes.new(name)
    bm.to_mesh(me)
    bm.free()
    piece = bpy.data.objects.new(name, me)
    bpy.context.scene.collection.objects.link(piece)
    piece.matrix_world = mw.copy()
    inv = mw.inverted()
    for v in me.vertices:
        v.co = inv @ v.co
    # Each vertex weighted as the surface under it: the nearest face's corners, nearer more.
    names = {g.index: g.name for g in body.vertex_groups}
    for g in body.vertex_groups:
        piece.vertex_groups.new(name=g.name)
    for v in me.vertices:
        at, _, face, _ = tree.find_nearest(mw @ v.co)
        corners = body.data.polygons[face].vertices
        share = [1.0 / max((mw @ body.data.vertices[c].co - at).length, 1e-4) for c in corners]
        total = {}
        for c, w in zip(corners, share):
            for g in body.data.vertices[c].groups:
                total[g.group] = total.get(g.group, 0.0) + w * g.weight
        whole = sum(total.values()) or 1.0
        for g, w in total.items():
            piece.vertex_groups[names[g]].add([v.index], w / whole, "REPLACE")
    me.materials.append(material)
    return piece


def band(body, rays, width, offset, name, material, inner=None):
    """A new object: the band along `rays` over `body`, `width` wide, its outside `offset` off
    the skin. With `inner`, it is solid from `inner` off the skin up to `offset`: a strap worn
    over the chest is solid from the skin to above the thickest chest piece (docs/characters.md),
    so it rests on bare skin and shows over a coat alike."""
    line = path(body, rays)
    from mathutils.kdtree import KDTree
    kd = KDTree(len(line))
    for i, p in enumerate(line):
        kd.insert(p, i)
    kd.balance()
    mw = body.matrix_world
    piece = body.copy()
    piece.data = body.data.copy()
    piece.name = name
    bpy.context.scene.collection.objects.link(piece)
    piece.modifiers.clear()
    bm = bmesh.new()
    bm.from_mesh(piece.data)
    # The file splits vertices at seams; weld them so the band is one strip as it is pushed out.
    bmesh.ops.remove_doubles(bm, verts=bm.verts, dist=1e-5)
    # Only the skin near the path, then cut along its edges, half its width either side,
    # through the faces rather than along them, so its edges run straight (skin_piece.py).
    bmesh.ops.delete(bm, geom=[f for f in bm.faces if all(kd.find(mw @ v.co)[2] > width for v in f.verts)], context="FACES")
    bmesh.ops.delete(bm, geom=[v for v in bm.verts if not v.link_faces], context="VERTS")
    inside = bm.verts.layers.float.new("inside")
    for v in bm.verts:
        v[inside] = width / 2 - kd.find(mw @ v.co)[2]
    skin_piece.cut(bm, inside, 0.0, CUT_CLEAR_M)
    skin_piece.relax(bm, mw, tree_of(body), EDGE_RELAX_ROUNDS)
    bm.verts.layers.float.remove(inside)
    bm.normal_update()
    for v in bm.verts:
        v.co = v.co + v.normal * offset
    bm.to_mesh(piece.data)
    bm.free()
    piece.data.materials.clear()
    piece.data.materials.append(material)
    if inner is not None:
        solid = piece.modifiers.new("Solid", "SOLIDIFY")
        solid.thickness = offset - inner
        solid.offset = -1.0
        solid.use_rim = True
        bpy.ops.object.select_all(action="DESELECT")
        piece.select_set(True)
        bpy.context.view_layer.objects.active = piece
        bpy.ops.object.modifier_apply(modifier=solid.name)
    return piece
