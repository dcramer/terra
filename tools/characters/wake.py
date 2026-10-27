"""Gives a creature that wakes its face: a mouth that splits open and eyes that can glow.

    tools/blender/run.sh tools/characters/wake.py tools/characters/out/NAME.glb [--sheet=PNG]

Run after convert_rig.py and before export_character.py; it rewrites NAME.glb in place.
The face is found on the body, not given: the eyes are the darkest pair of spots on the
front of the upper trunk (the colour map's painted eye holes), and the mouth sits below
them by their spacing. As games give a face a mouth (a cut lip line, a mouth bag behind
it, teeth on the lips, opened by a blend shape) it adds:
  - a ragged horizontal cut, closed at rest (a frost crack), with a dark dish behind it
    (the mouth bag) so an open mouth shows no inside of the trunk, its throat an ember that
    lights with the eyes;
  - splinter teeth along both lips, grained as the bark and catching the throat's ember,
    tucked inside the trunk until the lips part;
  - a heavy bark brow over the eyes, which sit in sockets in its shadow;
  - a `Wake` shape key: the lips parted most in the middle, rolled out thick, raggedly;
  - an emissive map, amber with a hot middle where the eye holes are painted dark, black
    elsewhere. It ships
    dark (export_character.py exports an emissive map's factor as 0); the game lights it
    with staged.glow and opens the mouth with staged.morph (D536).
--sheet renders the face closed, open, and open from 35 degrees, to check it at a glance.
"""

import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import bmesh
import bpy
import numpy as np
from mathutils import Vector
from mathutils.interpolate import poly_3d_calc as poly_weights
from mathutils.bvhtree import BVHTree
from mathutils.kdtree import KDTree

import look

AMBER = (1.0, 0.52, 0.12)
CORE = (1.0, 0.86, 0.55)  # an eye's hot middle, near white, fading out to amber
# The throat's glow at its deepest, dark by the lips, so the mouth reads as a hollow lit from
# within, not a lit decal; at round 6's 0.07 it read as no hollow at all (film reviews).
EMBER = (0.45, 0.1, 0.014)
# The teeth's warm cast from it: barely any, so the fangs stand dark against the lit throat
# (round 8's 0.13 lit them as one orange sawtooth panel, a jack-o'-lantern's).
CAUGHT = (0.03, 0.009, 0.002)
src = look.path(look.args()[0])
SHEET = next((look.path(a.split("=", 1)[1]) for a in look.args() if a.startswith("--sheet=")), None)

look.reset()
bpy.ops.import_scene.gltf(filepath=src)
arm = next(o for o in bpy.context.scene.objects if o.type == "ARMATURE")
body = max((o for o in bpy.context.scene.objects if o.type == "MESH"), key=lambda o: len(o.data.vertices))
mw = body.matrix_world.copy()
joint = {b.name: arm.matrix_world @ b.head_local for b in arm.data.bones}
bark = body.data.materials[0]
tex = next(n for n in bark.node_tree.nodes if n.type == "TEX_IMAGE" and any(
    l.to_socket.name == "Base Color" for l in n.outputs[0].links))
W, H = tex.image.size
colour = np.empty(W * H * 4, dtype=np.float32)
tex.image.pixels.foreach_get(colour)
lum = (colour.reshape(H, W, 4)[..., :3] @ np.array([0.2126, 0.7152, 0.0722], dtype=np.float32))

bm = bmesh.new()
bm.from_mesh(body.data)
bm.transform(mw)  # worked on in the world: Z up, the body facing -Y
uv = bm.loops.layers.uv.active
deform = bm.verts.layers.deform.verify()


def front_faces(lo, hi, cx, half, facing=-0.3):
    return [f for f in bm.faces if f.normal.y < facing and lo < f.calc_center_median().z < hi
            and abs(f.calc_center_median().x - cx) < half]


def texels(faces):
    """Every texel of `faces`: its pixel, where it is on the body, how dark it is painted and
    how far its face turns from straight ahead (0 ahead, 1 sideways or up)."""
    px, pos, dark, turn = [], [], [], []
    for f in faces:
        loops = f.loops[:]
        for k in range(1, len(loops) - 1):
            tri = (loops[0], loops[k], loops[k + 1])
            t = np.array([[l[uv].uv.x * W, l[uv].uv.y * H] for l in tri])
            p = np.array([l.vert.co[:] for l in tri])
            x0, y0 = np.floor(t.min(0)).astype(int)
            x1, y1 = np.ceil(t.max(0)).astype(int)
            gx, gy = np.meshgrid(np.arange(x0, x1 + 1), np.arange(y0, y1 + 1))
            q = np.stack([gx.ravel() + 0.5, gy.ravel() + 0.5], 1)
            m = np.array([t[1] - t[0], t[2] - t[0]]).T
            if abs(np.linalg.det(m)) < 1e-9:
                continue
            b = np.linalg.solve(m, (q - t[0]).T).T
            inside = (b[:, 0] >= 0) & (b[:, 1] >= 0) & (b.sum(1) <= 1)
            b, q = b[inside], q[inside].astype(int) % [W, H]
            px.append(q)
            pos.append(p[0] + b[:, :1] * (p[1] - p[0]) + b[:, 1:] * (p[2] - p[0]))
            dark.append(lum[q[:, 1], q[:, 0]])
            turn.append(np.full(len(q), 1 + f.normal.y))
    return np.concatenate(px), np.concatenate(pos), np.concatenate(dark), np.concatenate(turn)


# 1. The eyes: the darkest pair of spots on the front of the upper trunk, level and either
# side of its middle, in hollows that face ahead: the dark inner faces of a fork in the crown
# turn up or aside. Spots are 3 cm cells of texels, each averaged with its neighbours.
band = np.array([v.co[:] for v in bm.verts if abs(v.co.z - joint["neck_01"].z) < 0.3 and abs(v.co.x) < 1])
cx, cy = (float(c) for c in np.median(band[:, :2], 0))
faces = front_faces(joint["spine_03"].z, joint["Head"].z + 0.5, cx, 0.45)
px, pos, dark, turn = texels(faces)
cell = 0.03
key = np.floor(pos[:, [0, 2]] / cell).astype(int)
cells = {}
for k, d in zip(map(tuple, key), dark + 0.3 * turn):
    cells.setdefault(k, []).append(d)
mean = {k: float(np.mean(v)) for k, v in cells.items()}
smooth = {k: np.mean([mean[n] for n in ((k[0] + i, k[1] + j) for i in (-1, 0, 1) for j in (-1, 0, 1)) if n in mean])
          for k in mean}
spots = sorted(smooth, key=smooth.get)[:400]
pairs = [(smooth[a] + smooth[b], a, b) for a in spots for b in spots
         if a[0] * cell < cx < b[0] * cell and abs(a[1] - b[1]) * cell < 0.12
         and 0.15 < (b[0] - a[0]) * cell < 0.5 and abs((a[0] + b[0] + 1) * cell / 2 - cx) < 0.12]
_, eye_l, eye_r = min(pairs)
eyes = [Vector(((k[0] + 0.5) * cell, 0, (k[1] + 0.5) * cell)) for k in (eye_l, eye_r)]
spacing = eyes[1].x - eyes[0].x
print("EYES", ", ".join(f"({e.x:.2f}, {e.z:.2f})" for e in eyes), f"{spacing:.2f} m apart, painted",
      f"{smooth[eye_l]:.3f}/{smooth[eye_r]:.3f} against the face's median {float(np.median(dark)):.3f}")

# 2. The glow: amber over each eye's dark paint, fading out over an eye-sized ellipse.
glow = np.zeros((H, W, 4), dtype=np.float32)
glow[..., 3] = 1
ref = float(np.percentile(dark, 40))
for e in eyes:
    r = np.hypot((pos[:, 0] - e.x) / (0.42 * spacing), (pos[:, 2] - e.z) / (0.25 * spacing))
    near = r < 1
    floor = float(np.percentile(dark[near], 3))
    w = np.clip((ref - dark[near]) / max(ref - floor, 1e-3), 0, 1) ** 0.7
    w *= 1 - np.clip((r[near] - 0.6) / 0.4, 0, 1)
    q = px[near]
    for c in range(3):
        hot = w * (AMBER[c] + (CORE[c] - AMBER[c]) * w ** 3)
        glow[q[:, 1], q[:, 0], c] = np.maximum(glow[q[:, 1], q[:, 0], c], hot)
    print("GLOW", f"{int((w > 0.5).sum())} texels lit")
mask = bpy.data.images.new(f"{body.name}_glow", W, H, alpha=False)
mask.pixels.foreach_set(glow.ravel())
mask.filepath_raw = os.path.join(look.OUT, f"{look.body_name(src)}-glow.png")
mask.file_format = "PNG"
mask.save()
nodes, links = bark.node_tree.nodes, bark.node_tree.links
bsdf = next(n for n in nodes if n.type == "BSDF_PRINCIPLED")
node = nodes.new("ShaderNodeTexImage")
node.image = mask
links.new(node.outputs["Color"], bsdf.inputs["Emission Color"])
bsdf.inputs["Emission Strength"].default_value = 1.0

# The bark graded to the forest's oaks' (D536): each channel's mean and spread, in sRGB, moved
# to those of the oaks' bark scans as the forest tints them (lush.rs BARKS), measured on the
# trunk's front, so asleep it is not the lighter, greener tree among them. The crown is bare
# limbs, graded with the rest; its leaves are the forest's cards (crown.py).
def encoded(rgb):
    return np.where(rgb <= 0.0031308, 12.92 * rgb, 1.055 * np.power(np.maximum(rgb, 0), 1 / 2.4) - 0.055)


def scan(name, tint):
    im = bpy.data.images.load(look.path(f"assets/materials/{name}/color.jpg"))
    a = np.empty(im.size[0] * im.size[1] * 4, dtype=np.float32)
    im.pixels.foreach_get(a)
    srgb = a.reshape(-1, 4)[::7, :3]
    lin = np.where(srgb <= 0.04045, srgb / 12.92, ((srgb + 0.055) / 1.055) ** 2.4)
    return encoded(lin * tint)


oaks = np.concatenate([scan("oak_bark", 0.65), scan("bark_brown_02", 0.82)])
# Darker and warmer again: in the film the scans' grade still stood pale grey among the
# forest's dark brown oaks (round-5 still reviews), whose shader darkens them in its relief.
oaks = oaks * np.array([0.84, 0.79, 0.72], dtype=np.float32)
rgb = colour.reshape(H, W, 4)[..., :3]
trunk_px = px[pos[:, 2] < joint["Head"].z]
own = rgb[trunk_px[:, 1], trunk_px[:, 0]]
gain = oaks.std(0) / np.maximum(own.std(0), 1e-3)
graded = np.clip((rgb - own.mean(0)) * gain + oaks.mean(0), 0, 1)
rgb[...] = graded
tex.image.pixels.foreach_set(colour)
tex.image.update()
tex.image.pack()
print("BARK", "mean", " ".join(f"{v:.3f}" for v in own.mean(0)), "->", " ".join(f"{v:.3f}" for v in oaks.mean(0)),
      "spread x", " ".join(f"{v:.2f}" for v in gain))

# The crown kept on the trunk: what the conversion bound to the arms well above them (the
# leaves and twigs over the shoulders) moved to the trunk's bone at its height, so an arm
# raised into the crown goes in among its leaves instead of dragging them with it.
arm_bones = {g.index for g in body.vertex_groups if g.name.split("_")[0] in (
    "clavicle", "upperarm", "lowerarm", "hand", "thumb", "index", "middle", "ring", "pinky")}
trunk = {n: body.vertex_groups[n].index for n in ("spine_03", "neck_01", "Head")}
above = max(joint["clavicle_l"].z, joint["clavicle_r"].z) + 0.6
moved = 0
for v in bm.verts:
    w = v[deform]
    off = sum(w[g] for g in list(w.keys()) if g in arm_bones)
    if v.co.z < above or off == 0:
        continue
    for g in [g for g in w.keys() if g in arm_bones]:
        del w[g]
    to = trunk["Head"] if v.co.z > joint["Head"].z else trunk["neck_01"] if v.co.z > joint["neck_01"].z else trunk["spine_03"]
    w[to] = w.get(to, 0) + off
    moved += 1
print("CROWN", f"{moved} vertices over {above:.2f} m moved off the arms")

# 3. The mouth's cut: below the eyes by their spacing, wider than they are apart, ragged
# along its length as bark splits on its furrows, not a carved zigzag (round-6 film reviews:
# a jack-o'-lantern decal). The faces around it are split once for a finer lip.
mx, mz = (eyes[0].x + eyes[1].x) / 2, eyes[0].z / 2 + eyes[1].z / 2 - 1.3 * spacing
half = 1.2 * spacing
eye_z = (eyes[0].z + eyes[1].z) / 2


def jag(x):
    return 0.07 * spacing * math.sin(9 * x + 1.3) + 0.035 * spacing * math.sin(29 * x + 0.4)


# The brow: a heavy ridge of bark over both eyes, thrust out so the sun leaves them in its
# shadow until they light, the eyes sunk into sockets under it (round-6 film reviews: the
# eyes slits painted on a flat face). Its faces split once for a ridge, not a dent.
brow_lo, brow_hi = mz + 0.5, eye_z + 1.6 * spacing
region = [f for f in front_faces(brow_lo, brow_hi, mx, 1.6 * spacing + 0.15)]
bmesh.ops.subdivide_edges(bm, edges=list({e for f in region for e in f.edges}), cuts=1, use_grid_fill=True)
bm.normal_update()
ridge = eye_z + 0.32 * spacing  # its crest, over the eyes
thrust = 0.45 * spacing
browed = 0
for v in bm.verts:
    if v.co.y > cy or not (eye_z - 0.6 * spacing < v.co.z < brow_hi) or abs(v.co.x - mx) > 1.6 * spacing:
        continue
    u = (v.co.x - mx) / (1.6 * spacing)
    # Lower over the nose than over the eyes' outer ends, as a scowl.
    crest = ridge - 0.12 * spacing * (1 - abs(u))
    across = max(0.0, 1 - u * u) ** 0.6
    dz = v.co.z - crest
    # Sharp under the crest, an overhang; long and easing out above it into the bark.
    rise = math.exp(-((dz / (0.14 * spacing)) ** 2)) if dz < 0 else math.exp(-((dz / (0.7 * spacing)) ** 2))
    push = thrust * across * rise
    # The sockets: each eye pressed back into the trunk under the ridge.
    for e in eyes:
        r = math.hypot((v.co.x - e.x) / (0.45 * spacing), (v.co.z - e.z) / (0.3 * spacing))
        if r < 1:
            push -= 0.18 * spacing * (1 - r * r)
    if abs(push) > 1e-4:
        v.co.y -= push
        browed += 1
bm.normal_update()
print("BROW", f"crest {ridge:.2f} m, {thrust:.2f} m out, {browed} vertices moved")


# The cut runs through the walls of the bole's furrows too, not only the faces turned ahead:
# kept to those, it left the furrow between the pressed legs whole across the mouth, a bark
# seam down its middle when it opened (round-8 film reviews).
def mouth_faces(lo, hi, wide):
    return [f for f in front_faces(lo, hi, mx, wide, facing=0.6) if f.calc_center_median().y < cy]


region = mouth_faces(mz - 0.5, mz + 0.5, half + 0.2)
edges = list({e for f in region for e in f.edges})
bmesh.ops.subdivide_edges(bm, edges=edges, cuts=1, use_grid_fill=True)
bm.normal_update()
region = mouth_faces(mz - 0.3, mz + 0.3, half + 0.1)
geom = list({g for f in region for g in (f, *f.edges, *f.verts)})
cut = bmesh.ops.bisect_plane(bm, geom=geom, plane_co=(0, 0, mz), plane_no=(0, 0, 1))["geom_cut"]
slit = [e for e in cut if isinstance(e, bmesh.types.BMEdge)
        and abs((e.verts[0].co.x + e.verts[1].co.x) / 2 - mx) < 0.95 * half]
bmesh.ops.split_edges(bm, edges=slit)
bm.normal_update()
on_slit = {v for e in slit for v in e.verts}
on_slit |= {v for v in bm.verts if abs(v.co.z - mz) < 1e-5 and abs(v.co.x - mx) < 0.95 * half and v.co.y < cy}
for v in on_slit:
    v.co.z += jag(v.co.x)

# 4. What the open mouth shows: a dark dish behind the cut, its rim just inside the bark, and
# splinter teeth on both lips, each skinned as the nearest bark is and grained as it.
bark_verts = list(bm.verts)
skin = KDTree(len(bark_verts))
for i, v in enumerate(bark_verts):
    skin.insert(v.co, i)
skin.balance()
tree = BVHTree.FromBMesh(bm)
tree_faces = list(bm.faces)  # the tree's face indices, kept as faces are added after it


def surface(x, z):
    hit = tree.ray_cast(Vector((x, -50, z)), Vector((0, 1, 0)))[0]
    return hit if hit is not None else Vector((x, joint["neck_01"].y - 0.5, z))


def bark_uv(co):
    """The bark's picture where `co` is seen from straight ahead: a splinter grained as the
    wood it is broken from, not one flat colour."""
    hit, _, i, _ = tree.ray_cast(Vector((co.x, -50, co.z)), Vector((0, 1, 0)))
    if hit is None:
        return None
    f = tree_faces[i]
    w = poly_weights([v.co for v in f.verts], hit)
    return sum((l[uv].uv * wi for l, wi in zip(f.loops, w)), Vector((0, 0)))


def material(name, rgb, rough):
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    p = m.node_tree.nodes["Principled BSDF"]
    p.inputs["Base Color"].default_value = (*rgb, 1)
    p.inputs["Roughness"].default_value = rough
    p.inputs["Specular IOR Level"].default_value = 0.1
    body.data.materials.append(m)
    return len(body.data.materials) - 1


added = set()


def new_vert(co):
    v = bm.verts.new(co)
    added.add(v)
    near = bark_verts[skin.find(co)[1]]
    v[deform].clear()
    for g, w in near[deform].items():
        v[deform][g] = w
    return v


mouth_bag = material(f"{body.name}_mouth", (0.004, 0.003, 0.002), 1.0)
# A throat, not a dish: 1 m deep in the middle (round 8's 0.55 read as a flat lit panel).
gw, gh, bag_w, bag_h, bag_d = 16, 10, 1.3 * half, 0.6, 1.0
grid = [[new_vert(surface(mx + u * bag_w, mz + v * bag_h)
                  + Vector((0, 0.02 + bag_d * (1 - u * u) * (1 - v * v), 0)))
         for u in np.linspace(-1, 1, gw)] for v in np.linspace(-1, 1, gh)]
for j in range(gh - 1):
    for i in range(gw - 1):
        f = bm.faces.new((grid[j][i], grid[j][i + 1], grid[j + 1][i + 1], grid[j + 1][i]))
        f.material_index = mouth_bag
        for l, (a, b) in zip(f.loops, ((i, j), (i + 1, j), (i + 1, j + 1), (i, j + 1))):
            l[uv].uv = (a / (gw - 1), b / (gh - 1))
# The throat lit faintly from deep inside, an ember hottest at the back of the bag, so the
# open mouth reads in the shade; it ships dark and lights with the eyes (staged.glow).
throat_px = 32
gu, gv = np.meshgrid(np.linspace(-1, 1, throat_px), np.linspace(-1, 1, throat_px))
# Black at the rim and hot only at the back, so the opening reads as a cavity going in.
heat = np.clip(1 - (gu ** 2 + gv ** 2), 0, 1) ** 9
throat = np.ones((throat_px, throat_px, 4), dtype=np.float32)
throat[..., :3] = heat[..., None] * np.array(EMBER, dtype=np.float32)
ember = bpy.data.images.new(f"{body.name}_throat", throat_px, throat_px, alpha=False)
ember.pixels.foreach_set(throat.ravel())
ember.pack()
bag = body.data.materials[mouth_bag]
bag_bsdf = bag.node_tree.nodes["Principled BSDF"]
bag_tex = bag.node_tree.nodes.new("ShaderNodeTexImage")
bag_tex.image = ember
bag.node_tree.links.new(bag_tex.outputs["Color"], bag_bsdf.inputs["Emission Color"])
bag_bsdf.inputs["Emission Strength"].default_value = 1.0
# The teeth: the bark's own material, but catching the throat's ember (an emissive map of its
# warm cast, so staged.glow lights them with the eyes); splinters a reviewer sees from 8 m.
caught = bark.copy()
caught.name = f"{body.name}_teeth"
body.data.materials.append(caught)
teeth_mat = len(body.data.materials) - 1
cast = bpy.data.images.new(f"{body.name}_caught", 4, 4, alpha=False)
cast.pixels.foreach_set(np.tile(np.array([*CAUGHT, 1], dtype=np.float32), 16))
cast.pack()
caught_bsdf = next(n for n in caught.node_tree.nodes if n.type == "BSDF_PRINCIPLED")
caught_tex = caught.node_tree.nodes.new("ShaderNodeTexImage")
caught_tex.image = cast
caught.node_tree.links.new(caught_tex.outputs["Color"], caught_bsdf.inputs["Emission Color"])
teeth = []  # (vertices, the lip it hangs from: +1 upper, -1 lower, its x)
# Fewer and longer than round 8's 9 and 8 (a flat sawtooth): uneven splinters, the upper lip's
# second from each corner a fang that crosses the open gap, each raked back into the throat.
for lip, count in ((1, 7), (-1, 6)):
    for i in range(count):
        x = mx + (-0.8 + 1.6 * (i + 0.5 * (lip < 0) + 0.5) / (count + 0.5)) * 0.95 * half
        h = (math.sin(91.7 * x + 7 * lip) + 1) / 2  # a fixed scatter: each tooth its own length
        fang = lip > 0 and i in (1, count - 2)
        root = surface(x, mz + jag(x)) + Vector((0, 0.045, 0))
        b = (0.11 + 0.07 * h + 0.05 * fang) * spacing
        long = (0.4 + 0.45 * h) * (1.6 if fang else 1) * spacing
        a, c, d = (new_vert(root + o) for o in (Vector((-b, 0, 0)), Vector((b, 0, 0)), Vector((0, b, 0))))
        tip = new_vert(root + Vector((0.3 * b * math.sin(13 * x), 0.2 * long, -lip * long)))
        for tri in ((a, c, d), (a, tip, c), (c, tip, d), (d, tip, a)):
            f = bm.faces.new(tri if lip > 0 else tri[::-1])
            f.material_index = teeth_mat
            for l in f.loops:
                grain = bark_uv(l.vert.co)
                if grain is not None:
                    l[uv].uv = grain
        teeth.append(((a, c, d, tip), lip, x))
bm.normal_update()

# 5. The Wake shape: each lip moved off the cut, most in the middle and raggedly, rolled out.
wf = 0.95 * half
# Each lip's move eased out over a long reach (the lower, which falls further, the longer), so
# the bark's picture is drawn out by a quarter at most, not by half just off the cut; the
# upper's reach stops short of the eyes.
# Opened wide enough to read from 8 m (round-6 film reviews: it never opens): the lower lip
# drops as a jaw would, the upper lifts under the brow.
up, down = 0.5 * spacing, 1.15 * spacing
reach_up, reach_down = 1.2 * spacing, 3.2 * spacing


def along(x):
    return max(0.0, 1 - ((x - mx) / wf) ** 2) * (1 + 0.25 * math.sin(23 * x))


delta = {}
for v in bm.verts:
    dz = v.co.z - (mz + jag(v.co.x))
    if v in added or v.co.y > cy or abs(dz) > max(reach_up, reach_down) or abs(v.co.x - mx) > wf:
        continue
    if v in on_slit:
        side = 1 if sum(f.calc_center_median().z for f in v.link_faces) / max(1, len(v.link_faces)) > v.co.z else -1
    else:
        side = 1 if dz > 0 else -1
    g = max(0.0, 1 - abs(dz) / (reach_up if side > 0 else reach_down)) ** 1.2
    f = along(v.co.x) * g
    # The lips rolled out into a thick rim, the bark nearest the cut pushed forward most
    # (round 8's curled-in lips read as the edge of a hole cut in a flat face).
    delta[v] = Vector(((v.co.x - mx) * 0.12 * f, -0.22 * spacing * f * g ** 4, side * (up if side > 0 else down) * f))
for verts, lip, x in teeth:
    d = Vector((0, 0, lip * (up if lip > 0 else down) * along(x)))
    for v in verts:
        delta[v] = d
bm.verts.index_update()
delta = {v.index: d for v, d in delta.items()}
print("MOUTH", f"at ({mx:.2f}, {mz:.2f}), {2 * half:.2f} m wide, {len(slit)} cut edges,",
      f"{len(delta)} vertices move, {len(teeth)} teeth")

bm.transform(mw.inverted())
inv = mw.inverted().to_3x3()
bm.to_mesh(body.data)
bm.free()
body.shape_key_add(name="Basis", from_mix=False)
wake = body.shape_key_add(name="Wake", from_mix=False)
for i, d in delta.items():
    wake.data[i].co += inv @ d

for o in bpy.context.scene.objects:
    o.select_set(o in (arm, body))
bpy.context.view_layer.objects.active = arm
bpy.ops.export_scene.gltf(filepath=src, export_format="GLB", use_selection=True, export_animations=False)
print("EXPORTED", src)

if SHEET:
    look.stage()
    tiles = []
    for weight, glow_on, side in ((0, 0, 0), (1, 6, 0), (1, 6, 35)):
        wake.value = weight
        bsdf.inputs["Emission Strength"].default_value = glow_on
        bag_bsdf.inputs["Emission Strength"].default_value = glow_on
        caught_bsdf.inputs["Emission Strength"].default_value = glow_on
        # close_up names its pictures by their place in one call: each kept under its own.
        shot = look.close_up(arm, "neck_01", 4 * spacing, sides=(side,), size=600)[0]
        tiles.append(shot.replace(".png", f"-{len(tiles)}.png"))
        os.replace(shot, tiles[-1])
    look.stitch(tiles, SHEET)
    print("SHEET", SHEET)
