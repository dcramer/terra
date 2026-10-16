"""Pieces made from the body's own surface (D128, docs/characters.md): a garment cut from the
skin (shell_garment.py), a strap laid on it (bands.py). What they share:

  line_up  a model of the base person wearing the piece, lined up with the body
  cut      cut a mesh along a smooth field through its triangles, not along their edges, as
           sculpting tools extract a garment from a body; no stepped, sawtooth edge
  relax    even the cut edge out along itself, kept on the skin
  bake     the piece's colour, surface (normals, a normal map included) and roughness, baked
           from a model of the base person wearing it onto a new unwrap
  rim      a rim turned in from the edge to just off the skin, so the edge reads as the
           material's thickness and nothing shows into the gap between piece and skin

The new vertices a cut makes take their weights and every float layer from the two ends of
their edge, so a piece keeps moving as the skin under it does.
"""

import math
import os

import bmesh
import bpy
import numpy as np
from mathutils import Matrix, Vector
from mathutils.bvhtree import BVHTree
from mathutils.kdtree import KDTree

import look


def flags(argv):
    """A script's `--name=value` options, by name, and its other arguments in order."""
    named = {a[2:].split("=", 1)[0]: (a.split("=", 1)[1] if "=" in a else "") for a in argv if a.startswith("--")}
    return named, [a for a in argv if not a.startswith("--")]


def kd_of(points):
    tree = KDTree(len(points))
    for i, p in enumerate(points):
        tree.insert(Vector(p), i)
    tree.balance()
    return tree


def morph_delta(name, points):
    """The morph `name` (tools/characters/out/NAME.npz, make_morph.sh) at each of `points`, the
    base body's skin in world space, matched by position as carry_gear.py matches it; nothing
    moves for no morph. A piece is drawn for a morph (a skirt on the female) by building it on
    the base's skin moved by this, then taking it back off (`follow`)."""
    points = np.asarray(points, dtype=np.float64)
    if not name:
        return np.zeros_like(points)
    with np.load(os.path.join(look.OUT, f"{name}.npz")) as data:
        at, delta = data["points"], data["delta"]
    tree = kd_of(at)
    return delta[[tree.find(Vector(p))[1] for p in points]]


def follow(P, V, delta, edges=None, near=12, rounds=8):
    """How far each of the points `P` moves when the skin `V` moves by `delta`: as the skin near
    it moves, the `near` nearest skin points weighted by closeness, then evened out along the
    piece's `edges`. For a piece that hangs off the skin, such as a skirt, whose hem is nearest
    now one shin and now the other: taken vertex by vertex from the one skin point nearest, the
    hem would tear between them."""
    P = np.asarray(P, dtype=np.float64)
    tree = kd_of(V)
    moved = np.zeros_like(P)
    for i, p in enumerate(P):
        found = tree.find_n(Vector(p), near)
        w = np.array([1.0 / max(d, 1e-3) for _, _, d in found])
        moved[i] = (delta[[j for _, j, _ in found]] * w[:, None]).sum(axis=0) / w.sum()
    if edges is not None and len(edges):
        edges = np.asarray(edges)
        for _ in range(rounds):
            total, count = moved.copy(), np.ones(len(P))
            np.add.at(total, edges[:, 0], moved[edges[:, 1]])
            np.add.at(total, edges[:, 1], moved[edges[:, 0]])
            np.add.at(count, edges[:, 0], 1)
            np.add.at(count, edges[:, 1], 1)
            moved = total / count[:, None]
    return moved


def made(body_name, item, V=None, delta=None):
    """A piece made on this body before (tools/characters/out/gear/BODY/ITEM.glb) as a BVH tree
    and its points, world space, for what is made over or under it; moved with the skin `V` by a
    morph's `delta` when given. Nothing of it is left in the scene."""
    before = set(bpy.context.scene.objects)
    bpy.ops.import_scene.gltf(filepath=os.path.join(look.GEAR_MADE, body_name, f"{item}.glb"))
    new = [o for o in bpy.context.scene.objects if o not in before]
    pts, faces = [], []
    for o in new:
        # Its skinned meshes: the importer also makes shapes to draw the bones with.
        if o.type != "MESH" or not any(m.type == "ARMATURE" for m in o.modifiers):
            continue
        base = len(pts)
        pts += [tuple(o.matrix_world @ v.co) for v in o.data.vertices]
        faces += [tuple(base + i for i in p.vertices) for p in o.data.polygons]
    for o in new:
        bpy.data.objects.remove(o)
    pts = np.array(pts)
    if delta is not None and delta.any():
        tree = kd_of(V)
        pts = pts + delta[[tree.find(Vector(p))[1] for p in pts]]
    return BVHTree.FromPolygons([Vector(p) for p in pts], faces), pts


def mirrored(source):
    """A copy of `source` turned front to back (its Y negated), its faces kept facing out: where
    a piece is covered by another made from the same dressed model (a shirt under a smith's
    apron), its own colour is taken from the other side of the body, where nothing covers it."""
    copy = source.copy()
    copy.data = source.data.copy()
    copy.name = source.name + "_mirrored"
    bpy.context.scene.collection.objects.link(copy)
    copy.data.transform(Matrix.Scale(-1, 4, (0, 1, 0)))
    copy.data.flip_normals()
    copy.data.update()
    return copy


def line_up(path, skin, top, match):
    """Imports a model of the base person wearing something (Tripo's, facing +X), turned to
    face -Y as ours do, scaled to the body's height `top`, and nudged until its bare skin meets
    the body's (`skin`, a BVH tree of it): skin within `match` of ours is what lines it up.
    Returns it, one mesh in world space."""
    before = set(bpy.context.scene.objects)
    bpy.ops.import_scene.gltf(filepath=path)
    new = [o for o in bpy.context.scene.objects if o not in before]
    dressed = max((o for o in new if o.type == "MESH"), key=lambda o: len(o.data.vertices))
    dmw = dressed.matrix_world.copy()
    dressed.parent = None
    dressed.modifiers.clear()
    dressed.data.transform(Matrix.Rotation(-math.pi / 2, 4, "Z") @ dmw)
    dressed.matrix_world = Matrix.Identity(4)
    for o in new:
        if o is not dressed:
            bpy.data.objects.remove(o)
    zs = [v.co.z for v in dressed.data.vertices]
    dressed.data.transform(Matrix.Scale(top / (max(zs) - min(zs)), 4) @ Matrix.Translation((0, 0, -min(zs))))
    for _ in range(6):
        sample = list(dressed.data.vertices)[::5]
        hits = [skin.find_nearest(v.co, match) for v in sample]
        offsets = [hit[0] - v.co for hit, v in zip(hits, sample) if hit[0] is not None]
        if offsets:
            dressed.data.transform(Matrix.Translation(sum(offsets, Vector()) / len(offsets)))
    return dressed


def cut(bm, field, level, clear):
    """Keeps the part of `bm` where the float vertex layer `field` is above `level`, cut where
    it crosses `level` through the faces. The field is first kept `clear` from `level` at the
    vertices, so the cut never passes a hair from one and leaves a sliver of a face. Returns the
    vertices the cut made."""
    for v in bm.verts:
        if abs(v[field] - level) < clear:
            v[field] = level + clear if v[field] >= level else level - clear
    made = set()
    for e in [e for e in bm.edges if (e.verts[0][field] - level) * (e.verts[1][field] - level) < 0]:
        a, b = e.verts
        _, v = bmesh.utils.edge_split(e, a, (level - a[field]) / (b[field] - a[field]))
        v[field] = level
        made.add(v)
    for f in list(bm.faces):
        on = [v for v in f.verts if v in made]
        if len(on) == 2:
            bmesh.ops.connect_verts(bm, verts=on)
    bmesh.ops.delete(bm, geom=[f for f in bm.faces if sum(v[field] for v in f.verts) / len(f.verts) < level], context="FACES")
    bmesh.ops.delete(bm, geom=[v for v in bm.verts if not v.link_faces], context="VERTS")
    bmesh.ops.dissolve_degenerate(bm, dist=1e-4, edges=bm.edges[:])
    return {v for v in made if v.is_valid}


def relax(bm, mw, skin, rounds):
    """Evens the open edge of `bm` out along itself, `rounds` times, each round put back on the
    skin (`skin`, a BVH tree in world space; `mw`, the mesh's world matrix)."""
    inv = mw.inverted()
    for _ in range(rounds):
        moved = {}
        for v in [v for v in bm.verts if v.is_boundary]:
            along = [e.other_vert(v) for e in v.link_edges if e.is_boundary]
            if len(along) == 2:
                p = mw @ (v.co * 0.5 + (along[0].co + along[1].co) * 0.25)
                hit = skin.find_nearest(p)
                moved[v] = inv @ (hit[0] if hit[0] is not None else p)
        for v, co in moved.items():
            v.co = co


def blank(name, size, data=False):
    """A new image of `size` pixels, all clear, so what a bake writes shows by its alpha."""
    image = bpy.data.images.new(name, size, size, alpha=True, is_data=data)
    image.pixels.foreach_set(np.zeros(size * size * 4, np.float32))
    return image


def baked(image):
    """Which pixels of `image` a bake wrote, by its alpha, row by row as `pixels` holds them."""
    w, h = image.size
    px = np.empty(w * h * 4, np.float32)
    image.pixels.foreach_get(px)
    return px.reshape(h, w, 4)[..., 3] > 0.5


def fill_gaps(image, mask):
    """Edge padding: every pixel a bake left clear given the colour of the baked pixels nearest
    it, by pull-push (each smaller copy the mean of the baked pixels under it, then each gap filled
    from the smallest copy that has something there). Left black, the gaps darken a texture's
    smaller copies, which a game reads at a distance, and its roughness's, which were 0, glossed
    the leather until it mirrored the sky. `mask` is where the bake wrote: a colour bake's alpha
    shows it; a normal or roughness bake writes every pixel opaque, so takes the colour's."""
    w, h = image.size
    px = np.empty(w * h * 4, np.float32)
    image.pixels.foreach_get(px)
    px = px.reshape(h, w, 4)
    weight = mask.astype(np.float32)
    if weight.all() or not weight.any():
        return
    levels = [(px[..., :3] * weight[..., None], weight)]
    while levels[-1][1].shape[0] > 1 and levels[-1][1].shape[1] > 1:
        total, count = levels[-1]
        hh, ww = count.shape[0] // 2 * 2, count.shape[1] // 2 * 2
        total = total[:hh, :ww].reshape(hh // 2, 2, ww // 2, 2, 3).sum(axis=(1, 3))
        count = count[:hh, :ww].reshape(hh // 2, 2, ww // 2, 2).sum(axis=(1, 3))
        levels.append((total, count))
    colour = levels[-1][0] / np.maximum(levels[-1][1], 1e-6)[..., None]
    for total, count in reversed(levels[:-1]):
        up = np.repeat(np.repeat(colour, 2, axis=0), 2, axis=1)
        up = np.pad(up, ((0, count.shape[0] - up.shape[0]), (0, count.shape[1] - up.shape[1]), (0, 0)), mode="edge")
        here = total / np.maximum(count, 1e-6)[..., None]
        colour = np.where(count[..., None] > 0, here, up)
    px[..., :3] = np.where(weight[..., None] > 0, px[..., :3], colour)
    px[..., 3] = 1.0
    image.pixels.foreach_set(px.ravel())


# How rough each material is at least, on average over its piece: generated models make leather
# and cloth glossier than worn ones, and a glossy dark garment mirrors the sky (blue, then purple
# under the game's grade) wherever it is seen. Each is lifted toward 1 as a whole, keeping its grain.
ROUGHNESS = {"Leather": 0.7, "Cloth": 0.85, "Harness": 0.7, "Linen": 0.85, "Wool": 0.9}
# The colour each material is graded to on average, in sRGB, its variation kept: leather a dark,
# muted brown, little saturated, which stays brown in the forest's shade, where a redder or more
# saturated one turned mauve and an ochre one read as suede in the sun (D329).
GRADE = {"Leather": (0.30, 0.24, 0.19)}
# A plain cloth's texture is its own within this of its hue, and this share of its lightness (`bake`, plain).
PLAIN = (0.06, 0.5, 1.6)


def grade(image, mask, target):
    """Scales each channel of `image` so its mean where `mask` says it was baked is `target`.
    A baked colour image holds sRGB values, as its pixels give them."""
    w, h = image.size
    px = np.empty(w * h * 4, np.float32)
    image.pixels.foreach_get(px)
    px = px.reshape(h, w, 4)
    was = px[..., :3][mask].mean(axis=0)
    px[..., :3] = np.clip(px[..., :3] * (np.array(target) / np.maximum(was, 1e-4)), 0, 1)
    image.pixels.foreach_set(px.ravel())
    return tuple(round(float(c), 3) for c in was)


def roughen(image, mask, least):
    """Lifts `image`'s roughness, where `mask` says it was baked, so it averages at least `least`:
    each pixel moved the same share of the way to 1."""
    w, h = image.size
    px = np.empty(w * h * 4, np.float32)
    image.pixels.foreach_get(px)
    px = px.reshape(h, w, 4)
    mean = float(px[..., 0][mask].mean()) if mask.any() else 1.0
    if mean >= least:
        return mean, mean
    t = (least - mean) / (1.0 - mean)
    px[..., :3] += (1.0 - px[..., :3]) * t
    image.pixels.foreach_set(px.ravel())
    return mean, least


def _pixels(image):
    w, h = image.size
    px = np.empty(w * h * 4, np.float32)
    image.pixels.foreach_get(px)
    return px.reshape(h, w, 4)


def bake(piece, source, name, role, size, reach, under=None, plain=False, unwrap=True, behind=None):
    """Gives `piece` a new unwrap and a material named `role`, its colour, normals and roughness
    baked from `source` (a model of the base person wearing it, lined up with the body) at
    `size` pixels, looking `reach` out and in from the piece. Returns the material. With
    `unwrap` false it keeps the unwrap the piece has (a hung skirt's, laid out as its grid);
    `behind`, for such an unwrap, gives the texture column on the other side of the body from
    a column (0 to 1), where a plain cloth's covered part is taken from before any patch.

    `under` names a float attribute on the piece's vertices, 1 where something else made from
    the same model covers it (a skirt under an apron): there each map is baked from the model
    turned front to back (`mirrored`), so the piece shows its own cloth there and not the
    other piece's, blended across the attribute's edge. `plain`: the piece is one plain cloth,
    so what the model paints on it far from the colour of most of it is refilled (PLAIN)."""
    if unwrap:
        for layer in list(piece.data.uv_layers):
            piece.data.uv_layers.remove(layer)
        piece.data.uv_layers.new(name="UVMap")
        bpy.ops.object.select_all(action="DESELECT")
        piece.select_set(True)
        bpy.context.view_layer.objects.active = piece
        bpy.ops.object.mode_set(mode="EDIT")
        bpy.ops.mesh.select_all(action="SELECT")
        with bpy.context.temp_override(active_object=piece, object=piece, edit_object=piece):
            bpy.ops.uv.smart_project(island_margin=0.01)
        bpy.ops.object.mode_set(mode="OBJECT")
    colour = blank(f"{name}_colour", size)
    mat = bpy.data.materials.new(role)
    mat.use_nodes = True
    nodes = mat.node_tree.nodes
    bsdf = nodes["Principled BSDF"]
    tex = nodes.new("ShaderNodeTexImage")
    tex.image = colour
    mat.node_tree.links.new(tex.outputs["Color"], bsdf.inputs["Base Color"])
    bsdf.inputs["Roughness"].default_value = 0.8
    nodes.active = tex
    piece.data.materials.clear()
    piece.data.materials.append(mat)
    scene = bpy.context.scene
    scene.render.engine = "CYCLES"
    scene.cycles.device = "CPU"
    scene.cycles.samples = 1
    scene.render.bake.use_pass_direct = False
    scene.render.bake.use_pass_indirect = False
    scene.render.bake.cage_extrusion = 2 * reach
    scene.render.bake.max_ray_distance = 4 * reach
    scene.render.bake.margin = 16
    mirror = mirrored(source) if under else None
    share = None
    if under:
        # Where the piece is covered, as an image on its unwrap: the attribute baked as light.
        mask = blank(f"{name}_under", size, data=True)
        node = nodes.new("ShaderNodeTexImage")
        node.image = mask
        nodes.active = node
        attr = nodes.new("ShaderNodeAttribute")
        attr.attribute_name = under
        link = mat.node_tree.links.new(attr.outputs["Fac"], bsdf.inputs["Emission Color"])
        bsdf.inputs["Emission Strength"].default_value = 1.0
        scene.render.bake.use_selected_to_active = False
        bpy.ops.object.select_all(action="DESELECT")
        piece.select_set(True)
        bpy.context.view_layer.objects.active = piece
        bpy.ops.object.bake(type="EMIT")
        share = np.clip(_pixels(mask)[..., 0], 0.0, 1.0)
        mat.node_tree.links.remove(link)
        bsdf.inputs["Emission Strength"].default_value = 0.0
        nodes.remove(attr)
        nodes.remove(node)
        bpy.data.images.remove(mask)
        print(f"BAKED {name}: {float((share > 0.5).mean()):.0%} of its texture is under another piece, taken from the other side")
    scene.render.bake.use_selected_to_active = True

    def from_sources(kind, image):
        """Bakes `kind` into `image` from the model, and where the piece is covered, from the
        model turned about. Returns where the bake wrote."""
        bpy.ops.object.select_all(action="DESELECT")
        source.select_set(True)
        piece.select_set(True)
        bpy.context.view_layer.objects.active = piece
        bpy.ops.object.bake(type=kind, normal_space="TANGENT")
        if mirror is None:
            return baked(image)
        first = _pixels(image).copy()
        image.pixels.foreach_set(np.zeros(first.size, np.float32))
        bpy.ops.object.select_all(action="DESELECT")
        mirror.select_set(True)
        piece.select_set(True)
        bpy.context.view_layer.objects.active = piece
        bpy.ops.object.bake(type=kind, normal_space="TANGENT")
        second = _pixels(image)
        # The other side where the piece is covered and the turned model was hit; the first
        # bake elsewhere, and wherever the turned one found nothing.
        t = share * (second[..., 3] > 0.5)
        mixed = first * (1.0 - t[..., None]) + second * t[..., None]
        mixed[..., 3] = np.maximum(first[..., 3], second[..., 3] * (share > 0.5))
        image.pixels.foreach_set(mixed.ravel())
        return mixed[..., 3] > 0.5

    written = from_sources("DIFFUSE", colour)
    print(f"BAKED {name} covers {written.mean():.0%} of its texture; the rest padded")
    patch = None
    if plain:
        # One plain cloth: what is far from the colour of most of it is another piece painted
        # on it by the dressed model (a strap, a tie, an apron's edge). Its own cloth's colour is
        # the one most of it is near (a mode, not a mean: half a shirt can be painted with an
        # apron), told apart by hue more than lightness, which a fold's shade changes; what is
        # not its own is laid over, in every map, with a patch of its own cloth repeated.
        rgb = _pixels(colour)[..., :3]

        def other(colours, c):
            size = np.linalg.norm(colours, axis=-1) + 1e-4
            hue = np.linalg.norm(colours / size[..., None] - c / np.linalg.norm(c), axis=-1)
            light = size / np.linalg.norm(c)
            return (hue > PLAIN[0]) | (light < PLAIN[1]) | (light > PLAIN[2])

        found = rgb[written]
        sample = found[np.random.default_rng(0).choice(len(found), min(400, len(found)), replace=False)]
        few = found[:: max(1, len(found) // 20000)]
        middle = sample[int(np.argmin([other(few, c).sum() for c in sample]))]
        foreign = written & other(rgb, middle)
        own = written & ~foreign
        # The patch: the square of its own cloth, wholly its own, nearest its colour.
        side = size // 32
        best = None
        for y in range(0, size - side, side // 2):
            for x in range(0, size - side, side // 2):
                if own[y:y + side, x:x + side].all():
                    off = float(np.linalg.norm(rgb[y:y + side, x:x + side].mean(axis=(0, 1)) - middle))
                    if best is None or off < best[0]:
                        best = (off, y, x)
        patch = (best[1], best[2], side, foreign, own) if best is not None else (None, None, side, foreign, own)
        print(f"BAKED {name} plain: colour {tuple(round(float(c), 2) for c in middle)}, {foreign.sum() / max(1, written.sum()):.1%} of it another piece's paint, "
              + (f"laid over with its own cloth from {best[1]},{best[2]}" if best else "no clean patch of its own to lay over it"))

    def lay_over(image):
        """Where the piece is not its own cloth, a patch of its own repeated (`plain`)."""
        if patch is None:
            return
        y0, x0, side, where, own = patch
        px = _pixels(image)
        where = where.copy()
        if behind is not None:
            # The same cloth on the other side of the body, where that is its own.
            other = np.array([min(size - 1, max(0, int(round(behind((x + 0.5) / size) * size - 0.5)))) for x in range(size)])
            take = where & own[:, other]
            px[take] = px[:, other][take]
            where &= ~take
        if y0 is not None:
            tile = px[y0:y0 + side, x0:x0 + side]
            reps = (size + side - 1) // side
            tiled = np.tile(tile, (reps, reps, 1))[:size, :size]
            px[where] = tiled[where]
        image.pixels.foreach_set(px.ravel())

    lay_over(colour)
    if role in GRADE:
        print(f"BAKED {name} colour {grade(colour, written, GRADE[role])} graded to {GRADE[role]} for {role}")
    fill_gaps(colour, written)
    colour.pack()
    # Its surface as well as its colour, so leather reads as leather and not as a printed sheet.
    for kind in ("NORMAL", "ROUGHNESS"):
        image = blank(f"{name}_{kind.lower()}", size, data=True)
        image.colorspace_settings.name = "Non-Color"
        node = nodes.new("ShaderNodeTexImage")
        node.image = image
        nodes.active = node
        from_sources(kind, image)
        lay_over(image)
        if kind == "ROUGHNESS" and role in ROUGHNESS:
            was, now = roughen(image, written, ROUGHNESS[role])
            print(f"BAKED {name} roughness {was:.2f}, at least {now:.2f} for {role}")
        fill_gaps(image, written)
        image.pack()
        if kind == "NORMAL":
            mapping = nodes.new("ShaderNodeNormalMap")
            mat.node_tree.links.new(node.outputs["Color"], mapping.inputs["Color"])
            mat.node_tree.links.new(mapping.outputs["Normal"], bsdf.inputs["Normal"])
        else:
            mat.node_tree.links.new(node.outputs["Color"], bsdf.inputs["Roughness"])
    nodes.active = tex
    if mirror is not None:
        bpy.data.objects.remove(mirror)
    return mat


def rim(piece, inward):
    """Turns a rim in from `piece`'s open edge, each new vertex at `inward(i)`, the place (in
    the piece's own space) for the rim under the edge vertex `i`. Its faces take the colour
    along the edge (their texture coordinates are the edge's), and the edge is kept sharp."""
    bm = bmesh.new()
    bm.from_mesh(piece.data)
    bm.verts.ensure_lookup_table()
    uv = bm.loops.layers.uv.active
    edge = [e for e in bm.edges if e.is_boundary]
    # Keyed by the elements themselves: the extrusion leaves new elements' indices unset.
    edge_uv = {}
    for e in edge:
        face = e.link_faces[0]
        edge_uv[e] = {l.vert: l[uv].uv.copy() for l in face.loops if l.vert in e.verts} if uv else {}
        e.smooth = False
    old = set(bm.verts)
    index = {v: v.index for v in bm.verts}
    grown = bmesh.ops.extrude_edge_only(bm, edges=edge)
    source = {}
    for v in [g for g in grown["geom"] if isinstance(g, bmesh.types.BMVert)]:
        source[v] = next(e.other_vert(v) for e in v.link_edges if e.other_vert(v) in old)
        v.co = inward(index[source[v]])
    if uv:
        for f in [g for g in grown["geom"] if isinstance(g, bmesh.types.BMFace)]:
            e = next(e for e in f.edges if e in edge_uv)
            for l in f.loops:
                l[uv].uv = edge_uv[e][l.vert if l.vert in old else source[l.vert]]
    bm.normal_update()
    bm.to_mesh(piece.data)
    bm.free()
    piece.data.update()
