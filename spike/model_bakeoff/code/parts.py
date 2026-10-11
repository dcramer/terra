"""Small building blocks for models written as code: boxes, prisms, blobs and skinned limbs."""

import math
import os
import random
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

import bmesh
import bpy
from mathutils import Euler, Matrix, Vector

import look


def _object(name, bm, mat):
    me = bpy.data.meshes.new(name)
    bm.to_mesh(me)
    bm.free()
    ob = bpy.data.objects.new(name, me)
    bpy.context.scene.collection.objects.link(ob)
    return look.assign(ob, mat)


def place(ob, at=(0, 0, 0), rot=(0, 0, 0), scale=(1, 1, 1)):
    ob.matrix_world = Matrix.LocRotScale(Vector(at), Euler([math.radians(r) for r in rot]), Vector(scale))
    return ob


def box(name, size, mat, at=(0, 0, 0), rot=(0, 0, 0), taper=1.0, bevel=0.0):
    """A box `size` across, centred on `at`; `taper` shrinks its top."""
    bm = bmesh.new()
    bmesh.ops.create_cube(bm, size=1.0)
    for v in bm.verts:
        if v.co.z > 0:
            v.co.x *= taper
            v.co.y *= taper
        v.co = Vector((v.co.x * size[0], v.co.y * size[1], v.co.z * size[2]))
    if bevel:
        bmesh.ops.bevel(bm, geom=bm.edges[:], offset=bevel, segments=1, affect="EDGES")
    return place(_object(name, bm, mat), at, rot)


def prism(name, radius, height, mat, sides=6, at=(0, 0, 0), rot=(0, 0, 0), top=None, seed=None):
    """A many-sided column standing on `at`, narrowing to `top` radius."""
    rng = random.Random(seed)
    bm = bmesh.new()
    top = radius if top is None else top
    ring = []
    for z, r in ((0, radius), (height, top)):
        layer = []
        for i in range(sides):
            a = i / sides * math.tau
            j = 1 + (rng.uniform(-0.12, 0.12) if seed is not None else 0)
            layer.append(bm.verts.new((math.cos(a) * r * j, math.sin(a) * r * j, z)))
        ring.append(layer)
    for i in range(sides):
        k = (i + 1) % sides
        bm.faces.new((ring[0][i], ring[0][k], ring[1][k], ring[1][i]))
    bm.faces.new(ring[1])
    bm.faces.new(ring[0][::-1])
    return place(_object(name, bm, mat), at, rot)


def blob(name, radius, mat, at=(0, 0, 0), squash=(1, 1, 1), seed=0, detail=1, rough=0.15):
    """A faceted lump: a coarse sphere pushed in and out at random."""
    rng = random.Random(seed)
    bm = bmesh.new()
    bmesh.ops.create_icosphere(bm, subdivisions=detail, radius=radius)
    for v in bm.verts:
        v.co *= 1 + rng.uniform(-rough, rough)
        v.co = Vector((v.co.x * squash[0], v.co.y * squash[1], v.co.z * squash[2]))
    return place(_object(name, bm, mat), at)


def skinned(name, points, edges, radii, mat, smooth=0, decimate=None):
    """A body grown around a stick skeleton by Blender's skin modifier, then made faceted.

    `points` are joints, `edges` join them, and `radii` gives each joint's (x, y) thickness."""
    me = bpy.data.meshes.new(name)
    me.from_pydata([tuple(p) for p in points], edges, [])
    ob = bpy.data.objects.new(name, me)
    bpy.context.scene.collection.objects.link(ob)
    skin = ob.modifiers.new("Skin", "SKIN")
    skin.use_smooth_shade = False
    skin.branch_smoothing = 0.5
    # Smoothing pulls a skinned body in by about a third; make it up so radii mean radii.
    grow = 1.4 if smooth else 1.0
    for sv, r in zip(me.skin_vertices[0].data, radii):
        sv.radius = (r[0] * grow, r[1] * grow)
    me.skin_vertices[0].data[0].use_root = True
    if smooth:
        sub = ob.modifiers.new("Smooth", "SUBSURF")
        sub.levels = smooth
        sub.render_levels = smooth
    if decimate:
        dec = ob.modifiers.new("Decimate", "DECIMATE")
        dec.ratio = decimate
    bpy.context.view_layer.objects.active = ob
    for o in bpy.context.scene.objects:
        o.select_set(o is ob)
    for m in list(ob.modifiers):
        bpy.ops.object.modifier_apply(modifier=m.name)
    return look.assign(ob, mat)


def paint(ob, rule):
    """Gives faces palette materials by a rule on each face's centre and normal."""
    names = [m.name for m in ob.data.materials]
    for poly in ob.data.polygons:
        want = rule(ob.matrix_world @ poly.center, poly.normal)
        if want is None:
            continue
        if want not in names:
            ob.data.materials.append(look.material(want))
            names.append(want)
        poly.material_index = names.index(want)
