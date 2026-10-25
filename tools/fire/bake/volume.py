# Reads a Mantaflow gas sim's cache (OpenVDB, as Blender bakes it) and writes the cells of one box
# of it, frame by frame, for volume.ts to pack into a looping 3D volume (docs/fire-module.md, the
# last section; D493), run by bake.ts through tools/blender/run.sh:
#
#   tools/blender/run.sh tools/fire/bake/volume.py <recipe.json> <out dir>
#
# The recipe's `volume` says the cache, the first frame, how many, and the box in the sim's own
# cells, counted in the domain's whole grid. Blender's adaptive domain stores each frame's grids
# over only the cells holding gas, from `res_min`, which the frame's config file holds (Blender's
# MANTA::writeConfiguration: base_res at byte 148, res_min at 160, gzipped little-endian ints);
# the cell size is the domain's height over its base cells (the VDB's own voxel size follows the
# adaptive grid and is not it). For each frame it writes the box's flame then its temperature, float32, x fastest, then
# y, then z (up), to raw.bin; and to meta.json the cell size, the domain's cells, the temperature's
# largest value, and how much flame each frame has outside the box (its share of the frame's
# flame, and the largest flame there), so a box that cuts the flame shows. Blender's Python is
# used only for its OpenVDB module (D121). Our own script: nothing of Blender's is copied in.

import gzip
import json
import os
import struct
import sys
import time

import numpy as np
import openvdb as vdb

recipe_path, out = sys.argv[sys.argv.index("--") + 1:][:2]
with open(recipe_path) as f:
    recipe = json.load(f)
v = recipe["volume"]
cache = os.path.expanduser(v["cache"])
first, count = v["first"], v["extract"]
x0, y0, z0 = v["box_min"]
nx, ny, nz = v["cells"]
os.makedirs(out, exist_ok=True)


def grid(path, name, base, rmin):
    """A frame's grid over the domain's whole base grid, 0 where the adaptive domain held none."""
    a = np.zeros(base, np.float32)
    vdb.read(path, name).copyToArray(a, ijk=tuple(-x for x in rmin))
    return a


def config(f):
    with gzip.open(os.path.join(cache, "config", "config_%04d.uni" % f)) as c:
        b = c.read()
    return struct.unpack_from("<3i", b, 148), struct.unpack_from("<3i", b, 160)


start = time.time()
outside, outside_max, t_max, res = [], [], 0.0, None
with open(os.path.join(out, "raw.bin"), "wb") as raw:
    for f in range(first, first + count):
        path = os.path.join(cache, "data", "fluid_data_%04d.vdb" % f)
        if not os.path.exists(path):
            sys.exit("no frame %d in the cache: %s" % (f, path))
        res, rmin = config(f)
        flame = grid(path, "flame", res, rmin)
        temp = grid(path, "temperature", res, rmin)
        box = (slice(x0, x0 + nx), slice(y0, y0 + ny), slice(z0, z0 + nz))
        fb, tb = flame[box], temp[box]
        if fb.shape != (nx, ny, nz):
            sys.exit("the box %s+%s leaves the domain's %s cells" % (v["box_min"], v["cells"], res))
        total = float(flame.sum())
        rest = flame.copy()
        rest[box] = 0
        outside.append(float(rest.sum()) / total if total > 0 else 0.0)
        outside_max.append(float(rest.max()))
        t_max = max(t_max, float(tb.max()))
        # x fastest: (x, y, z) -> (z, y, x) in C order.
        raw.write(np.ascontiguousarray(fb.transpose(2, 1, 0)).tobytes())
        raw.write(np.ascontiguousarray(tb.transpose(2, 1, 0)).tobytes())

meta = {
    "frames": count,
    "cells": [nx, ny, nz],
    "cell": v["domain_size"][2] / res[2],
    "domain_cells": list(res),
    "temperature_max": t_max,
    "outside_share_max": max(outside),
    "outside_share_mean": sum(outside) / len(outside),
    "outside_flame_max": max(outside_max),
    "outside_frames_over_0.3": sum(1 for m in outside_max if m > 0.3),
    "read_s": round(time.time() - start, 1),
}
with open(os.path.join(out, "meta.json"), "w") as f:
    json.dump(meta, f, indent=1)
print("volume", json.dumps(meta))
