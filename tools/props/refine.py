"""Makes a generated prop ours: welded, cut to a triangle budget, one palette colour per face
read from its texture, levelled and squared up, at its size, checked, and rendered in the game's light beside the
figure for scale.

    tools/blender/run.sh tools/props/refine.py MODEL OUT.glb SHEET.png SIZE BUDGET TURN ROLE...

MODEL is Tripo's textured model (facing +X, as Studio keeps them); OUT the file the game
loads; SIZE its longest side in metres; BUDGET the triangles it may have; TURN degrees about
the up axis after it is turned to face the standard way; then the palette roles its faces may
take, each named in look.PALETTE, or as ROLE=r,g,b to match it to that sRGB colour in the
texture instead of its own; and --flat for a thing that lies flat, such as a rug. Each face gets the role nearest its texture's colour there,
then nearly flat regions and stray faces are evened out, so colours sit in broad areas as in
the concept. Materials are named by role: the game colours a prop by those names (props.rs),
and a part called Fire glows. Run by props.ts, which reads these from art/props/<prop>/prop.json.
"""

import json
import math
import os
import sys
from collections import Counter

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "characters"))

import bmesh
import bpy
import numpy as np
from mathutils import Matrix, Vector

import look

# --flat: a thing that lies flat, such as a rug, laid on its plane and pressed thin.
args = look.args()
flat = "--flat" in args
src, out, sheet, size, budget, turn, *given = [a for a in args if a != "--flat"]
size, budget, turn = float(size), int(budget), float(turn)
# ROLE, or ROLE=r,g,b for a role that the texture paints in another colour than its own.
roles = [role.split("=")[0] for role in given]
palette = np.array(
    [[float(c) for c in role.split("=")[1].split(",")] if "=" in role else look.PALETTE[role] for role in given]
)
# The most a model's feet may lean, in degrees, for level() to stand it straight.
LEVEL_MAX = 8.0
# How thick a flat thing is pressed, in metres.
FLAT_M = 0.02
# How much lightness counts against hue and saturation when a face's colour is matched: a
# texture's painted light and shade change lightness far more than hue, so matching by raw
# RGB turns a shaded blue blanket into dark wood.
LIGHTNESS_WEIGHT = 0.5


def lab(srgb):
    """CIE L*a*b* of sRGB colours (rows), lightness scaled by LIGHTNESS_WEIGHT."""
    c = np.asarray(srgb, dtype=np.float64)
    c = np.where(c <= 0.04045, c / 12.92, ((c + 0.055) / 1.055) ** 2.4)
    xyz = c @ np.array([[0.4124, 0.2126, 0.0193], [0.3576, 0.7152, 0.1192], [0.1805, 0.0722, 0.9505]])
    xyz = xyz / np.array([0.9505, 1.0, 1.089])
    f = np.where(xyz > 0.008856, np.cbrt(xyz), 7.787 * xyz + 16 / 116)
    return np.stack([(116 * f[..., 1] - 16) * LIGHTNESS_WEIGHT, 500 * (f[..., 0] - f[..., 1]), 200 * (f[..., 1] - f[..., 2])], axis=-1)


# The sun, relative to each camera, so no view on the sheet is all shadow.
SUN_YAW, SUN_PITCH = 40.0, 45.0


def texture_of(mat):
    if not mat or not mat.use_nodes:
        return None
    for node in mat.node_tree.nodes:
        if node.type == "TEX_IMAGE" and node.image:
            w, h = node.image.size
            return np.array(node.image.pixels[:], dtype=np.float32).reshape(h, w, 4)
    return None


def select_only(ob):
    for o in bpy.context.scene.objects:
        o.select_set(o is ob)
    bpy.context.view_layer.objects.active = ob


def load():
    """The model as one object with its transforms applied, without the generator's smoothed
    normals, which would otherwise spoil flat shading here and in the game."""
    bpy.ops.import_scene.gltf(filepath=look.path(src))
    meshes = [o for o in bpy.context.scene.objects if o.type == "MESH"]
    for o in meshes:
        o.data.transform(o.matrix_world)
        o.parent = None
        o.matrix_world = Matrix.Identity(4)
    for o in list(bpy.context.scene.objects):
        if o.type != "MESH":
            bpy.data.objects.remove(o)
    for o in bpy.context.scene.objects:
        o.select_set(True)
    bpy.context.view_layer.objects.active = meshes[0]
    if len(meshes) > 1:
        bpy.ops.object.join()
    ob = bpy.context.view_layer.objects.active
    select_only(ob)
    if ob.data.has_custom_normals:
        bpy.ops.mesh.customdata_custom_splitnormals_clear()
    return ob


def weld(me):
    """Generators split the mesh at every UV seam; weld it back into one surface. UVs live on
    face corners, so they survive."""
    bm = bmesh.new()
    bm.from_mesh(me)
    bmesh.ops.remove_doubles(bm, verts=bm.verts[:], dist=1e-4)
    bm.to_mesh(me)
    bm.free()


def reduce(ob):
    tris = sum(len(p.vertices) - 2 for p in ob.data.polygons)
    if tris <= budget:
        return
    cut = ob.modifiers.new("Reduce", "DECIMATE")
    cut.ratio = budget / tris
    cut.use_collapse_triangulate = True
    select_only(ob)
    bpy.ops.object.modifier_apply(modifier=cut.name)


def paint(ob):
    """Each face takes the palette role nearest its texture's colour."""
    me = ob.data
    palette_lab = lab(palette)
    textures = [texture_of(m) for m in me.materials]
    uv = me.uv_layers.active.data if me.uv_layers else None
    picked = []
    for poly in me.polygons:
        tex = textures[poly.material_index] if poly.material_index < len(textures) else None
        if tex is None or uv is None:
            picked.append(0)
            continue
        h, w = tex.shape[:2]
        corners = np.array([uv[i].uv for i in poly.loop_indices])
        centre = corners.mean(axis=0)
        # The centre, and points two thirds of the way to each corner.
        samples = np.vstack([centre, centre + (corners - centre) * 0.66])
        xs = np.clip((samples[:, 0] % 1.0) * (w - 1), 0, w - 1).astype(int)
        ys = np.clip((samples[:, 1] % 1.0) * (h - 1), 0, h - 1).astype(int)
        # An 8-bit sRGB texture's pixels come back as stored, already sRGB, as the palette is.
        colour = lab(tex[ys, xs, :3].mean(axis=0))
        picked.append(int(np.argmin(((palette_lab - colour) ** 2).sum(axis=1))))
    me.materials.clear()
    for role in roles:
        me.materials.append(look.material(role))
    # Fire glows in the game (props.rs), so it glows on the sheet too.
    if "Fire" in bpy.data.materials:
        bsdf = bpy.data.materials["Fire"].node_tree.nodes["Principled BSDF"]
        bsdf.inputs["Emission Color"].default_value = look.linear(look.PALETTE["Fire"]) + (1.0,)
        bsdf.inputs["Emission Strength"].default_value = 8.0
    for poly, i in zip(me.polygons, picked):
        poly.material_index = i
        poly.use_smooth = False
    while me.uv_layers:
        me.uv_layers.remove(me.uv_layers[0])


def flatten_regions(me, angle=12.0):
    """Faces lying nearly flat against each other, such as one plank's top, share one colour:
    whichever covers most of their area. The texture's shading otherwise splits them."""
    bm = bmesh.new()
    bm.from_mesh(me)
    bm.faces.ensure_lookup_table()
    limit = math.radians(angle)
    seen = set()
    for start in bm.faces:
        if start.index in seen:
            continue
        region, stack = [], [start]
        seen.add(start.index)
        while stack:
            f = stack.pop()
            region.append(f)
            for e in f.edges:
                for g in e.link_faces:
                    if g.index not in seen and f.normal.angle(g.normal, 0.0) < limit:
                        seen.add(g.index)
                        stack.append(g)
        if len(region) < 2:
            continue
        area = Counter()
        for f in region:
            area[f.material_index] += f.calc_area()
        colour = area.most_common(1)[0][0]
        for f in region:
            f.material_index = colour
    bm.to_mesh(me)
    bm.free()


def despeckle(me):
    """A face whose neighbours mostly share another colour takes theirs."""
    bm = bmesh.new()
    bm.from_mesh(me)
    bm.faces.ensure_lookup_table()
    for _ in range(2):
        current = [f.material_index for f in bm.faces]
        for f in bm.faces:
            around = Counter(current[g.index] for e in f.edges for g in e.link_faces if g is not f)
            if around:
                colour, count = around.most_common(1)[0]
                if colour != current[f.index] and count >= 2:
                    f.material_index = colour
    bm.to_mesh(me)
    bm.free()


def squaring(me):
    """Degrees about the up axis, within 45 either way, that give the model its smallest
    footprint: generators sometimes set a model a little or well askew, such as a
    pickaxe's head at 45 degrees, and squaring it puts its parts along the axes."""
    xy = np.array([(v.co.x, v.co.y) for v in me.vertices])

    def footprint(degrees):
        a = math.radians(degrees)
        turned = xy @ np.array([[math.cos(a), math.sin(a)], [-math.sin(a), math.cos(a)]])
        size = turned.max(axis=0) - turned.min(axis=0)
        return size[0] * size[1]

    return min(range(-45, 46), key=lambda degrees: (round(footprint(degrees), 6), abs(degrees)))


def split_finely(ob, cuts=4):
    """Cuts every face into smaller ones, so a pattern painted across a few large faces,
    such as a rug's border, can be followed face by face."""
    bm = bmesh.new()
    bm.from_mesh(ob.data)
    bmesh.ops.subdivide_edges(bm, edges=bm.edges[:], cuts=cuts, use_grid_fill=True)
    bm.to_mesh(ob.data)
    bm.free()


def merge_colours(ob):
    """Joins neighbouring faces of one colour lying in one plane, undoing split_finely
    wherever the colour does not change."""
    select_only(ob)
    bpy.ops.object.mode_set(mode="EDIT")
    bpy.ops.mesh.select_all(action="SELECT")
    bpy.ops.mesh.dissolve_limited(angle_limit=math.radians(5), delimit={"MATERIAL"})
    bpy.ops.mesh.quads_convert_to_tris()
    bpy.ops.object.mode_set(mode="OBJECT")


def lay_flat(me):
    """Turns the model so the plane it best fits is level, then presses it thin: a rug the
    generator modelled tilted or curled lies flat. place() sets how thin, at its size."""
    pts = np.array([v.co for v in me.vertices])
    centred = pts - pts.mean(axis=0)
    _, vectors = np.linalg.eigh(centred.T @ centred)
    normal = Vector(vectors[:, 0])
    if normal.z < 0:
        normal = -normal
    me.transform(normal.rotation_difference(Vector((0, 0, 1))).to_matrix().to_4x4())
    zs = [v.co.z for v in me.vertices]
    me.transform(Matrix.Diagonal((1, 1, 0.01 / max(max(zs) - min(zs), 1e-6), 1)))


def level(me):
    """Stands the model level: the lowest point in each quarter of its footprint is where it
    rests, as a chair on its four legs, and a model a little askew on those is turned until
    they are level. Leaves alone anything leaning more than LEVEL_MAX, which is not a tilt
    to correct but its shape."""
    pts = np.array([v.co for v in me.vertices])
    middle = (pts.min(axis=0) + pts.max(axis=0)) / 2
    feet = []
    for east in (False, True):
        for north in (False, True):
            quarter = pts[((pts[:, 0] > middle[0]) == east) & ((pts[:, 1] > middle[1]) == north)]
            if len(quarter):
                feet.append(quarter[np.argmin(quarter[:, 2])])
    feet = np.array(feet)
    # Only what rests on its corners, as furniture does: a tool standing on the end of its
    # handle has nothing low in most quarters, and is left as it is.
    resting = feet[:, 2] - pts[:, 2].min() < (pts[:, 2].max() - pts[:, 2].min()) * 0.15
    if len(feet) < 3 or not resting.all():
        return 0.0
    # The plane z = a x + b y + c through the feet.
    (a, b, _), *_ = np.linalg.lstsq(np.c_[feet[:, :2], np.ones(len(feet))], feet[:, 2], rcond=None)
    normal = Vector((-a, -b, 1.0)).normalized()
    lean = math.degrees(normal.angle(Vector((0, 0, 1))))
    if 0.2 < lean <= LEVEL_MAX:
        me.transform(normal.rotation_difference(Vector((0, 0, 1))).to_matrix().to_4x4())
    return lean


def place(ob):
    """Squared up, facing the standard way (Studio's +X front to glTF's +Z, Blender's -Y),
    turned by TURN, its longest side SIZE, standing on the origin."""
    me = ob.data
    me.transform(Matrix.Rotation(math.radians(squaring(me) - 90.0 + turn), 4, "Z"))
    pts = np.array([v.co for v in me.vertices])
    lo, hi = pts.min(axis=0), pts.max(axis=0)
    me.transform(Matrix.Scale(size / float((hi - lo).max()), 4))
    if flat:
        zs = [v.co.z for v in me.vertices]
        me.transform(Matrix.Diagonal((1, 1, FLAT_M / max(max(zs) - min(zs), 1e-6), 1)))
    pts = np.array([v.co for v in me.vertices])
    lo, hi = pts.min(axis=0), pts.max(axis=0)
    me.transform(Matrix.Translation(Vector(((lo[0] + hi[0]) / -2, (lo[1] + hi[1]) / -2, -lo[2]))))
    me.update()


def checks(ob):
    """What a script can say about a model: size, triangles, loose parts, colours."""
    bm = bmesh.new()
    bm.from_mesh(ob.data)
    tris = sum(len(f.verts) - 2 for f in bm.faces)
    seen, islands = set(), 0
    for f in bm.faces:
        if f.index in seen:
            continue
        islands += 1
        stack = [f]
        while stack:
            g = stack.pop()
            if g.index in seen:
                continue
            seen.add(g.index)
            for e in g.edges:
                stack.extend(h for h in e.link_faces if h.index not in seen)
    area = Counter()
    for f in bm.faces:
        area[roles[f.material_index]] += f.calc_area()
    bm.free()
    total = sum(area.values()) or 1.0
    return {
        "triangles": tris,
        "budget": budget,
        "size_m": [round(d, 2) for d in ob.dimensions],
        "islands": islands,
        "colours": {role: round(a / total, 3) for role, a in area.most_common()},
    }


def figure():
    """The 1.8 m figure standing beside the model, for scale, in one plain colour."""
    before = set(bpy.context.scene.objects)
    bpy.ops.import_scene.gltf(filepath=look.FIGURE)
    new = [o for o in bpy.context.scene.objects if o not in before]
    for o in new:
        if o.type == "ARMATURE":
            if o.animation_data:
                o.animation_data.action = None
            for bone in o.pose.bones:
                bone.matrix_basis.identity()
        if o.type == "MESH":
            for slot in o.material_slots:
                slot.material = look.material("Cloth")
    return new, [o for o in new if o.parent is None]


def render(ob, path):
    """Four views in a strip, the figure beside the model in the first only."""
    scene = bpy.context.scene
    scene.render.resolution_x = scene.render.resolution_y = 480
    cam = bpy.data.objects.new("Cam", bpy.data.cameras.new("Cam"))
    cam.data.lens = 50
    scene.collection.objects.link(cam)
    scene.camera = cam
    people, roots = figure()
    beside = Vector((ob.dimensions.x / 2 + 0.5, ob.dimensions.y / 4 + 0.2, 0))
    for r in roots:
        r.location = beside
    half = Vector(ob.dimensions) / 2
    sun = bpy.data.objects["Sun"]
    tiles = []
    for i, (yaw, pitch) in enumerate([(30, 15), (120, 15), (210, 15), (300, 45)]):
        with_figure = i == 0
        for o in people:
            o.hide_render = not with_figure
        lo = Vector((-half.x, -half.y, 0))
        hi = Vector((half.x, half.y, ob.dimensions.z))
        if with_figure:
            lo = Vector((min(lo.x, beside.x - 0.4), min(lo.y, beside.y - 0.3), 0))
            hi = Vector((max(hi.x, beside.x + 0.4), max(hi.y, beside.y + 0.3), max(hi.z, 1.85)))
        centre = (lo + hi) / 2
        dist = max((hi - lo).length / 2, 0.3) / math.sin(cam.data.angle / 2) * 0.9
        y, p = math.radians(yaw), math.radians(pitch)
        cam.location = centre + Vector((math.sin(y) * math.cos(p), -math.cos(y) * math.cos(p), math.sin(p))) * dist
        cam.rotation_euler = (centre - cam.location).to_track_quat("-Z", "Y").to_euler()
        ly, lp = y + math.radians(SUN_YAW), math.radians(SUN_PITCH)
        towards = Vector((math.sin(ly) * math.cos(lp), -math.cos(ly) * math.cos(lp), math.sin(lp)))
        sun.rotation_euler = (-towards).to_track_quat("-Z", "Y").to_euler()
        tile = f"{path}.view{i}.png"
        scene.render.filepath = tile
        bpy.ops.render.render(write_still=True)
        tiles.append(tile)
    look.stitch(tiles, path)
    for t in tiles:
        os.remove(t)


look.reset()
ob = load()
ob.name = os.path.splitext(os.path.basename(out))[0]
weld(ob.data)
reduce(ob)
if flat:
    split_finely(ob)
paint(ob)
# A flat thing is one plane, so evening out its planes would leave it one colour.
if not flat:
    flatten_regions(ob.data)
despeckle(ob.data)
if flat:
    merge_colours(ob)
    lay_flat(ob.data)
else:
    leaned = level(ob.data)
place(ob)
bpy.context.view_layer.update()
report = checks(ob)
report["leaned_degrees"] = 0.0 if flat else round(leaned, 1)
select_only(ob)
bpy.ops.export_scene.gltf(filepath=look.path(out), export_format="GLB", use_selection=True)
look.stage()
render(ob, look.path(sheet))
with open(os.path.splitext(look.path(sheet))[0] + ".json", "w") as f:
    json.dump(report, f, indent=2)
print("REPORT", json.dumps(report))
