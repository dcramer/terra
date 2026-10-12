"""Body patches: small pieces of a body's skin that what is worn can hide (docs/characters.md, D128).

The export divides each body into PATCHES patches, about two centimetres each, by clustering its
faces by place and facing, and writes each face's patch into the body's second set of texture
coordinates (u = patch + 0.5). The body stays one mesh. A worn look lists the patches it
covers in its glTF extras (`fit_gear.py` works them out), and the game's body shader collapses
those patches while it is worn (character.rs, body_mask.wgsl), so skin cannot poke through.
Patches are faces, so a vertex on a border is split there, keeping its weights: no cut opens.
"""

import bpy
import numpy as np

# How many patches a body has: what the game's mask holds (1024 bits). Finer patches leave a
# thinner strip of skin unhidden at a garment's edge, where a patch is only partly under it.
PATCHES = 1024
LAYER = "patch"


def mark(body, seed=7):
    """Writes each face's patch into a second texture coordinate layer of `body`."""
    mesh = body.data
    centres = np.array([tuple(body.matrix_world @ p.center) for p in mesh.polygons])
    normals = np.array([tuple((body.matrix_world.to_3x3() @ p.normal).normalized()) for p in mesh.polygons])
    # Facing counts a little, so the two sides of a thin gap (between the legs, under an
    # arm) fall in different patches.
    points = np.hstack([centres, normals * 0.04])
    rng = np.random.default_rng(seed)
    centroids = points[rng.choice(len(points), PATCHES, replace=False)]
    def nearest_of(points, centroids):
        out = np.empty(len(points), dtype=int)
        for start in range(0, len(points), 4096):
            chunk = points[start:start + 4096]
            out[start:start + 4096] = np.argmin(((chunk[:, None, :] - centroids[None, :, :]) ** 2).sum(-1), axis=1)
        return out

    for _ in range(12):
        nearest = nearest_of(points, centroids)
        for k in range(PATCHES):
            members = points[nearest == k]
            if len(members):
                centroids[k] = members.mean(axis=0)
    layer = mesh.uv_layers.get(LAYER) or mesh.uv_layers.new(name=LAYER)
    for poly in mesh.polygons:
        for li in poly.loop_indices:
            layer.data[li].uv = (nearest[poly.index] + 0.5, 0.5)
    return nearest


def of(body):
    """Each vertex's patch, read back from a body exported with them (its second layer)."""
    mesh = body.data
    if len(mesh.uv_layers) < 2:
        return None
    layer = mesh.uv_layers[1]
    patch = {}
    for poly in mesh.polygons:
        for li, vi in zip(poly.loop_indices, poly.vertices):
            patch[vi] = int(layer.data[li].uv[0])
    return patch
