"""Chars a refined fire's wood: a campfire's logs near-black where they burn, with thin glowing
cracks, its stones as they were (tools/props, `char` in prop.json, run after refine).

    tools/blender/run.sh tools/props/char.py SOURCE.glb OUT.glb

The generator paints a fire's logs as clean pale wood with broad flames drawn on them, which
in the game, where the fire's flames, coals and light are its own (D377), reads as birch with
orange stripes. Burnt wood is charcoal: near-black to dark brown, matte, cracked into blocks
whose cracks glow (alligatoring). So, in the textured material's maps:

- Each face is stone or wood by its texture: grey (unsaturated) faces in the outer ring are
  the stones and are left alone, as is the Fire part, which the game hides.
- Wood texels take a char colour, darkest nearest the fire's foot in the middle and a little
  browner toward the logs' upper ends, keeping some of the painted grain's light and dark.
- The painted orange keeps only its thin bright streaks, the cracks: a dull ember red in the
  colour map and an orange emissive map, a few percent of the wood, so the model never glows
  broadly beside the game's own fire.
- Wood is matte: roughness high, metal none.

Everything else of the file, its mesh, normals, normal map and the Fire material, is kept.
"""

import sys

import bpy
import numpy as np

# A face is stone where its texture is this grey (max minus min of sRGB) and it lies at least
# this far out from the middle (m): charred wood is grey too, but sits inside the ring.
STONE_GREY = 0.05
STONE_RING = 0.40
# Char colours in sRGB: at the fire's foot, and at the logs' far upper ends.
CHAR_LOW = np.array([0.075, 0.069, 0.064])
CHAR_HIGH = np.array([0.175, 0.13, 0.098])
# How much of the painted grain's light and dark the char keeps, low and high.
GRAIN_LOW, GRAIN_HIGH = 0.25, 0.55
# The char's blocks, how far apart (m), and how wide its cracks, a share of a block.
BLOCK = 0.045
CRACK_WIDTH = 0.14
# The glowing cracks: the emissive colour (sRGB, its brightest) and strength, and their dull
# red in the colour map.
CRACK_GLOW = np.array([1.0, 0.42, 0.10])
CRACK_STRENGTH = 3.0
CRACK_ALBEDO = np.array([0.55, 0.20, 0.06])
WOOD_ROUGHNESS = 0.93


def srgb_pixels(image):
    w, h = image.size
    px = np.empty(w * h * 4, dtype=np.float32)
    image.pixels.foreach_get(px)
    return px.reshape(h, w, 4)


def base_image(mat):
    bsdf = next(n for n in mat.node_tree.nodes if n.type == "BSDF_PRINCIPLED")
    link = bsdf.inputs["Base Color"].links[0]
    return bsdf, link.from_node.image


def face_samples(me, tex):
    """Each face's mean texture colour, at its centre and two thirds toward its corners."""
    h, w = tex.shape[:2]
    uv = me.uv_layers.active.data
    out = np.zeros((len(me.polygons), 3))
    for poly in me.polygons:
        corners = np.array([uv[i].uv for i in poly.loop_indices])
        centre = corners.mean(axis=0)
        s = np.vstack([centre, centre + (corners - centre) * 0.66])
        xs = np.clip((s[:, 0] % 1.0) * w, 0, w - 1).astype(int)
        ys = np.clip((s[:, 1] % 1.0) * h, 0, h - 1).astype(int)
        out[poly.index] = tex[ys, xs, :3].mean(axis=0)
    return out


def classify(me, tex, wood_material):
    """Per face: 0 another material (Fire), 1 stone, 2 wood."""
    colour = face_samples(me, tex)
    grey = colour.max(axis=1) - colour.min(axis=1) < STONE_GREY
    kind = np.zeros(len(me.polygons), dtype=int)
    for poly in me.polygons:
        if poly.material_index != wood_material:
            continue
        ring = np.hypot(poly.center.x, poly.center.y) > STONE_RING
        kind[poly.index] = 1 if grey[poly.index] and ring else 2
    return kind


def rasterise(me, kind, w, h):
    """In texture space: which kind of face covers each texel (-1 none), and the model-space
    position drawn there."""
    owner = -np.ones((h, w), dtype=int)
    where = np.zeros((h, w, 3), dtype=np.float32)
    uv = me.uv_layers.active.data
    me.calc_loop_triangles()
    for tri in me.loop_triangles:
        k = kind[tri.polygon_index]
        p = np.array([[uv[l].uv[0] * w - 0.5, uv[l].uv[1] * h - 0.5] for l in tri.loops])
        v = np.array([me.vertices[i].co for i in tri.vertices])
        x0, y0 = np.floor(p.min(axis=0)).astype(int)
        x1, y1 = np.ceil(p.max(axis=0)).astype(int)
        x0, y0, x1, y1 = max(x0, 0), max(y0, 0), min(x1, w - 1), min(y1, h - 1)
        if x1 < x0 or y1 < y0:
            continue
        gx, gy = np.meshgrid(np.arange(x0, x1 + 1), np.arange(y0, y1 + 1))
        d = (p[1, 1] - p[2, 1]) * (p[0, 0] - p[2, 0]) + (p[2, 0] - p[1, 0]) * (p[0, 1] - p[2, 1])
        if abs(d) < 1e-9:
            continue
        a = ((p[1, 1] - p[2, 1]) * (gx - p[2, 0]) + (p[2, 0] - p[1, 0]) * (gy - p[2, 1])) / d
        b = ((p[2, 1] - p[0, 1]) * (gx - p[2, 0]) + (p[0, 0] - p[2, 0]) * (gy - p[2, 1])) / d
        c = 1 - a - b
        inside = (a >= -0.02) & (b >= -0.02) & (c >= -0.02)
        owner[gy[inside], gx[inside]] = k
        bary = np.stack([a[inside], b[inside], c[inside]], axis=1)
        where[gy[inside], gx[inside]] = bary @ v
    return owner, where


def grow(mask, free, steps):
    """The mask grown a texel a step into texels no face covers, so filtering at a face's
    edge takes its own colour."""
    for _ in range(steps):
        g = mask.copy()
        g[1:] |= mask[:-1]
        g[:-1] |= mask[1:]
        g[:, 1:] |= mask[:, :-1]
        g[:, :-1] |= mask[:, 1:]
        mask = mask | (g & free)
    return mask


def box_blur(a, r):
    """A box blur of radius r texels, by running sums."""
    k = 2 * r + 1
    p = np.pad(a, r, mode="edge")
    c = np.cumsum(np.cumsum(p, axis=0), axis=1)
    c = np.pad(c, ((1, 0), (1, 0)))
    return (c[k:, k:] - c[:-k, k:] - c[k:, :-k] + c[:-k, :-k]) / (k * k)


def fill(where, drawn, steps):
    """Positions for texels grown past a face's edge: their nearest drawn neighbour's."""
    where, drawn = where.copy(), drawn.copy()
    for _ in range(steps):
        for axis, step in ((0, 1), (0, -1), (1, 1), (1, -1)):
            near = np.roll(drawn, step, axis=axis)
            take = near & ~drawn
            where[take] = np.roll(where, step, axis=axis)[take]
            drawn = drawn | take
    return where


def cells(q):
    """Voronoi cells about a jittered point in each unit cube, for points q (rows): how near
    each is to its cell's edge (second-nearest distance less nearest), and its cell's own
    random value in 0..1. The hash is the shader one, a fraction of a large sine."""
    base = np.floor(q)
    near = np.full(len(q), np.inf)
    second = np.full(len(q), np.inf)
    value = np.zeros(len(q))
    hash3 = lambda c, k: np.modf(np.abs(np.sin(c @ k) * 43758.5453))[0]
    keys = [np.array(k) for k in ((127.1, 311.7, 74.7), (269.5, 183.3, 246.1), (113.5, 271.9, 124.6), (19.9, 47.3, 93.1))]
    for offset in np.array(np.meshgrid([-1, 0, 1], [-1, 0, 1], [-1, 0, 1])).T.reshape(-1, 3):
        c = base + offset
        point = c + np.stack([hash3(c, k) for k in keys[:3]], axis=1)
        d = np.sqrt(((q - point) ** 2).sum(axis=1))
        closer = d < near
        second = np.where(closer, near, np.minimum(second, d))
        value = np.where(closer, hash3(c, keys[3]), value)
        near = np.where(closer, d, near)
    return second - near, value


def char(ob):
    me = ob.data
    wood_material = next(i for i, m in enumerate(me.materials) if m.name != "Fire")
    mat = me.materials[wood_material]
    bsdf, image = base_image(mat)
    px = srgb_pixels(image)
    h, w = px.shape[:2]
    kind = classify(me, px, wood_material)
    owner, where = rasterise(me, kind, w, h)
    wood = grow(owner == 2, owner == -1, 3)
    rgb = px[..., :3].astype(np.float64)

    # How burnt: 1 at the fire's foot in the middle, 0 at the logs' far upper ends.
    fire = [p.center for p in me.polygons if me.materials[p.material_index].name == "Fire"]
    foot = np.array([np.mean([c.x for c in fire]), np.mean([c.y for c in fire]), min(c.z for c in fire)]) if fire else np.zeros(3)
    filled = fill(where, owner >= 0, 4)
    dist = np.sqrt(((filled - foot) ** 2).sum(axis=2))
    far = np.quantile(dist[owner == 2], 0.97)
    burnt = np.clip(1.0 - dist / far, 0.0, 1.0) ** 0.5

    # The painted grain: lightness about its local mean, so broad painted light and the
    # painted flames' brightness go and the fine light and dark stay.
    light = rgb.mean(axis=2)
    grain = light - box_blur(light, 6)
    grain = np.clip(grain / (grain[wood].std() + 1e-6), -2.5, 2.5)
    keep = GRAIN_HIGH + (GRAIN_LOW - GRAIN_HIGH) * burnt

    # Alligatored char: blocks a few centimetres across, laid out in the model's own space
    # so they run on across the texture's seams, deeper cut and more of them the more burnt.
    line, block = cells(filled[wood] / BLOCK)
    cracks = np.zeros((h, w))
    cracks[wood] = np.clip(1.0 - line / CRACK_WIDTH, 0.0, 1.0)
    cut = cracks * np.clip((burnt - 0.05) / 0.4, 0.0, 1.0)
    shade = np.ones((h, w))
    shade[wood] = 0.7 + 0.6 * block
    base = CHAR_HIGH + (CHAR_LOW - CHAR_HIGH) * burnt[..., None]
    charred = base * shade[..., None] * (1.0 + 0.22 * keep[..., None] * grain[..., None])
    charred = np.clip(charred * (1.0 - 0.75 * cut[..., None]), 0.0, 1.0)

    # The glowing cracks: the char's cracks where the generator painted the wood burning
    # orange, and only where it burns, so its painted flames become a few thin lines.
    orange = np.clip((rgb[..., 0] - rgb[..., 2]) * 1.4, 0, 1) * np.clip(rgb[..., 0] * 1.3 - 0.2, 0, 1)
    painted = np.clip((box_blur(orange, 3) - 0.15) / 0.3, 0.0, 1.0)
    crack = cut * painted * np.clip((burnt - 0.2) / 0.4, 0.0, 1.0) * wood
    # It glows along the crack's middle only, half its width, so a crack is a line of light.
    core = np.zeros((h, w))
    core[wood] = np.clip(1.0 - line / (CRACK_WIDTH * 0.5), 0.0, 1.0)
    lit = crack * core

    out = rgb.copy()
    out[wood] = charred[wood]
    out = out + (CRACK_ALBEDO - out) * crack[..., None]
    px[..., :3] = out
    image.pixels.foreach_set(px.astype(np.float32).ravel())
    image.update()

    # The glow at the colour's size: at half, a crack a texel or two wide smears to a haze.
    glow = bpy.data.images.new(f"{image.name}_emissive", w, h, alpha=False)
    glow.colorspace_settings.name = "sRGB"
    e = np.zeros((h, w, 4), dtype=np.float32)
    e[..., :3] = CRACK_GLOW * lit[..., None]
    e[..., 3] = 1.0
    glow.pixels.foreach_set(e.ravel())
    glow.update()
    node = mat.node_tree.nodes.new("ShaderNodeTexImage")
    node.image = glow
    mat.node_tree.links.new(node.outputs["Color"], bsdf.inputs["Emission Color"])
    bsdf.inputs["Emission Strength"].default_value = CRACK_STRENGTH

    # Matte wood: the metal-roughness map (glTF's: roughness green, metal blue) on the wood.
    rough_link = bsdf.inputs["Roughness"].links
    if rough_link:
        sep = rough_link[0].from_node
        mr = sep.inputs[0].links[0].from_node.image
        mp = srgb_pixels(mr)
        mh, mw = mp.shape[:2]
        ys = (np.arange(mh) * h // mh)[:, None]
        xs = (np.arange(mw) * w // mw)[None, :]
        small = wood[ys, xs]
        mp[small, 1] = WOOD_ROUGHNESS
        mp[small, 2] = 0.0
        mr.pixels.foreach_set(mp.ravel())
        mr.update()

    stone = owner == 1
    print(
        "CHAR",
        f"faces wood {int((kind == 2).sum())} stone {int((kind == 1).sum())} other {int((kind == 0).sum())};",
        f"wood texels {int(wood.sum())}, cracks {crack[wood].mean() * 100:.1f}%, glowing {lit[wood].mean() * 100:.1f}%;",
        f"wood mean sRGB {np.round(out[wood].mean(axis=0) * 255).astype(int).tolist()},",
        f"stone {np.round(out[stone].mean(axis=0) * 255).astype(int).tolist()}",
    )
    return kind


if __name__ == "__main__":
    source, out = sys.argv[sys.argv.index("--") + 1 :][:2]
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.import_scene.gltf(filepath=source)
    ob = next(o for o in bpy.data.objects if o.type == "MESH")
    char(ob)
    bpy.ops.export_scene.gltf(filepath=out, export_format="GLB", export_tangents=True, export_image_format="JPEG",
                              export_jpeg_quality=90, export_extras=True)
