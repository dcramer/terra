"""Bands: straps, belts and the like, made on the body (D128, docs/characters.md).

A band runs along a path over the skin, given by a few rays cast at the body (from its own
joints and measurements, so it fits any body): each lands where it first meets the skin, as a
strap is laid on (down onto the top of a shoulder, in onto the chest), never the nearest skin,
which for a point off the body may be a hand or the head. The path between them is filled in
on the skin; the band is the body's own faces within half its width of
the path, pushed out a little, keeping each vertex's skin weights. So a strap moves exactly as
the skin under it does, over a shoulder or round a waist, and cannot come through it.
"""

import bmesh
import bpy
from mathutils import Vector
from mathutils.bvhtree import BVHTree

# How finely the path is filled in on the skin, in metres.
STEP_M = 0.01


def on_skin(tree, p):
    hit = tree.find_nearest(p)
    return hit[0] if hit[0] is not None else p


def path(body, rays):
    """The path through where `rays` (origin, direction) first meet the body's skin, filled in
    along the skin between them."""
    mw = body.matrix_world
    pts = [mw @ v.co for v in body.data.vertices]
    tree = BVHTree.FromPolygons(pts, [tuple(p.vertices) for p in body.data.polygons])
    anchors = []
    for origin, direction in rays:
        # A ray that passes beside a narrower body moves in toward its middle until it meets
        # skin, so the same route fits any body.
        origin = Vector(origin)
        for _ in range(8):
            hit = tree.ray_cast(origin, Vector(direction).normalized(), 3.0)
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
    keep = [f for f in bm.faces if kd.find(mw @ f.calc_center_median())[2] <= width / 2]
    bmesh.ops.delete(bm, geom=[f for f in bm.faces if f not in set(keep)], context="FACES")
    bmesh.ops.delete(bm, geom=[v for v in bm.verts if not v.link_faces], context="VERTS")
    # The file splits vertices at seams; weld them so the band is one strip as it is pushed out.
    bmesh.ops.remove_doubles(bm, verts=bm.verts, dist=1e-5)
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
