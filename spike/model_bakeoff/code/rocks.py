"""Rocks, written as a generator: a seed and a few numbers give a faceted boulder."""

import math
import os
import random
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

import bmesh
import bpy
from mathutils import Vector

import look


def boulder(name, seed, size, squash=0.7, points=60, cuts=5, moss=False):
    rng = random.Random(seed)
    bm = bmesh.new()
    # Points on a lumpy ellipsoid; their hull is a faceted stone.
    for _ in range(points):
        u, v = rng.uniform(-1, 1), rng.uniform(0, math.tau)
        r = math.sqrt(1 - u * u)
        lump = 1.0 + rng.uniform(-0.18, 0.12)
        bm.verts.new(Vector((r * math.cos(v) * size.x, r * math.sin(v) * size.y, u * size.z * squash)) * lump)
    bmesh.ops.convex_hull(bm, input=bm.verts[:])
    # A few big cuts give it the broad flat faces of split stone.
    for _ in range(cuts):
        normal = Vector((rng.uniform(-1, 1), rng.uniform(-1, 1), rng.uniform(-0.2, 1))).normalized()
        depth = rng.uniform(0.72, 0.9)
        extent = max(abs(normal.x) * size.x, abs(normal.y) * size.y, abs(normal.z) * size.z * squash)
        geom = bm.verts[:] + bm.edges[:] + bm.faces[:]
        bmesh.ops.bisect_plane(bm, geom=geom, plane_co=normal * extent * depth, plane_no=normal, clear_outer=True)
        bmesh.ops.holes_fill(bm, edges=[e for e in bm.edges if e.is_boundary])
    # A flat underside, sunk a little into the ground.
    geom = bm.verts[:] + bm.edges[:] + bm.faces[:]
    bmesh.ops.bisect_plane(bm, geom=geom, plane_co=(0, 0, -size.z * squash * 0.55), plane_no=(0, 0, -1), clear_outer=True)
    bmesh.ops.holes_fill(bm, edges=[e for e in bm.edges if e.is_boundary])
    bmesh.ops.dissolve_limit(bm, angle_limit=math.radians(4), verts=bm.verts[:], edges=bm.edges[:])
    bmesh.ops.triangulate(bm, faces=bm.faces[:])
    me = bpy.data.meshes.new(name)
    bm.to_mesh(me)
    bm.free()
    ob = bpy.data.objects.new(name, me)
    bpy.context.scene.collection.objects.link(ob)
    me.materials.append(look.material("Rock"))
    me.materials.append(look.material("Moss"))
    if moss:
        for poly in me.polygons:
            if poly.normal.z > 0.75:
                poly.material_index = 1
    else:
        me.materials.pop(index=1)
    look.flat(ob)
    return ob


look.reset()
rocks = [
    boulder("BoulderLarge", 3, Vector((1.4, 1.1, 1.2)), moss=True),
    boulder("Boulder", 11, Vector((0.8, 0.65, 0.75)), cuts=6),
    boulder("Slab", 21, Vector((1.0, 0.7, 0.35)), squash=1.0, cuts=4),
    boulder("Stone", 34, Vector((0.35, 0.3, 0.3)), points=36, cuts=4),
]
look.finish(rocks, "code-rocks", budget=400)
