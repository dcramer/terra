"""Grows a crown of the forest's oak leaf cards on a tree creature's bare crown (D536).

    tools/blender/run.sh tools/characters/crown.py tools/characters/out/NAME.glb [--sheet=PNG]

Run after wake.py and before export_character.py; it rewrites NAME.glb in place. A model's
own baked leaves read as dark flat cardboard beside the forest's oaks, so the creature is
modelled with bare limbs and dressed as the forest's veteran oaks are (D235): clumps found
where its crown's branches run out, each hung with six or seven cards of the foliage atlas's
`oakbranch` picture, placed, leaned and shaded as lush/species.rs `crown_cards` places an
oak's (stalk on the twig, turned mostly out of its mass, normals leaning out of the mass and
the crown, darker deep in the crown and low in it). Each card is skinned rigidly to the
nearest point of the crown, so it moves with the limb it grows on, and joins the body as a
second material (alpha-cut, two-sided) so export_character.py carries it.
"""

import math
import os
import random
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import bmesh
import bpy
import numpy as np
from mathutils import Vector
from mathutils.kdtree import KDTree

import look

CELL = (0.5, 0.5)  # assets/data/foliage.ron "oakbranch": its square's top-left, a 4th across
CLUMP_M = 0.55  # clumps this far apart along the outer branches
# A clump's size: the veteran oaks' tips are 0.95 m (assets/data/trees/oak_bough.ron) on limbs
# that run on far past the leaves; a creature's crown limbs are short, so its clumps are larger
# and carried out past the twigs' ends, for a crown as full as the forest's oaks'.
SIZE_M = 1.3
# lush.rs CROWN_TINTS[0] (0.92, 0.95, 0.85), raised and yellowed: the forest's leaves are lit
# through as well (their shader's transmission), which a plain material is not, and the first
# crown read dark blue-green beside them (round-5 still reviews).
TINT = Vector((1.2, 1.25, 0.82))
ARM = ("clavicle", "upperarm", "lowerarm", "hand", "thumb", "index", "middle", "ring", "pinky")

src = look.path(look.args()[0])
SHEET = next((look.path(a.split("=", 1)[1]) for a in look.args() if a.startswith("--sheet=")), None)
dice = random.Random(534)

look.reset()
bpy.ops.import_scene.gltf(filepath=src)
arm = next(o for o in bpy.context.scene.objects if o.type == "ARMATURE")
body = max((o for o in bpy.context.scene.objects if o.type == "MESH"), key=lambda o: len(o.data.vertices))
mw = body.matrix_world.copy()
joint = {b.name: arm.matrix_world @ b.head_local for b in arm.data.bones}
names = {g.index: g.name for g in body.vertex_groups}


def unit():
    while True:
        v = Vector((dice.uniform(-1, 1), dice.uniform(-1, 1), dice.uniform(-1, 1)))
        if 0.01 < v.length_squared <= 1:
            return v.normalized()


# 1. The crown's points: from the neck up, where its limbs fork off, on the trunk's bones,
# not the arms'.
head = joint["Head"]
points, weights = [], []
for v in body.data.vertices:
    co = mw @ v.co
    if co.z < joint["neck_01"].z - 0.1 or not v.groups:
        continue
    top = max(v.groups, key=lambda g: g.weight)
    if names[top.group].split("_")[0] in ARM:
        continue
    points.append(co)
    weights.append([(names[g.group], g.weight) for g in v.groups if g.weight > 0])
if len(points) < 50:
    sys.exit(f"CROWN only {len(points)} points above the head: no crown to dress")
pts = np.array([p[:] for p in points])
base = Vector((head.x, head.y, head.z))
reach = np.linalg.norm(pts - np.array(base[:]), axis=1)

# 2. Clumps where the branches run out: the points farther from the crown's foot than a third
# of its reach and off the bole, gathered CLUMP_M apart; each leans the way its branch runs,
# out from the foot.
off_bole = np.hypot(pts[:, 0] - head.x, pts[:, 1] - head.y) > 0.5
outer = pts[(reach > 0.3 * reach.max()) & (off_bole | (pts[:, 2] > head.z + 0.8))]
cells = {}
for p in outer:
    cells.setdefault(tuple(np.floor(p / CLUMP_M).astype(int)), []).append(p)
tips = [(Vector(np.mean(c, 0)), len(c)) for c in cells.values() if len(c) >= 2]
centre = sum((t for t, _ in tips), Vector()) / len(tips)
crown_reach = max(1.0, max(math.hypot(t.x - centre.x, t.y - centre.y) for t, _ in tips))
top_z = max(t.z for t, _ in tips)
# Masses: the clumps grouped by which way round the crown and how high, each a little other green.
masses = {}
for i, (t, _) in enumerate(tips):
    a = int((math.atan2(t.y - centre.y, t.x - centre.x) + math.pi) / (2 * math.pi) * 5) % 5
    masses.setdefault((a, t.z > centre.z), []).append(i)
mass_of, mass_at, hue_of = {}, {}, {}
for k, idx in masses.items():
    m = sum((tips[i][0] for i in idx), Vector()) / len(idx)
    h = dice.uniform(-1, 1)
    for i in idx:
        mass_of[i], mass_at[i], hue_of[i] = k, m, Vector((1 + 0.05 * h, 1 + 0.02 * h, 1 - 0.08 * h))

# 3. The cards, as crown_cards hangs an oak's.
tree = KDTree(len(points))
for i, p in enumerate(points):
    tree.insert(p, i)
tree.balance()
bm = bmesh.new()
uv = bm.loops.layers.uv.new("UVMap")
col = bm.loops.layers.float_color.new("Leaf")
normals, cards = [], []
for i, (tip, _) in enumerate(tips):
    along = (tip - base).normalized()
    mass = mass_at[i]
    out_crown = (tip - centre).normalized() if (tip - centre).length > 1e-3 else Vector((0, 0, 1))
    for k in range(7 + dice.randrange(2)):
        stalk = tip + along * SIZE_M * 0.05 + unit() * SIZE_M * dice.uniform(0.03, 0.15)
        out_mass = (stalk - mass).normalized() if (stalk - mass).length > 1e-3 else out_crown
        up = (along * 0.75 + unit() * 0.7 + out_mass * 0.45).normalized()
        wanted = out_mass * 0.6 + Vector((0, 0, 0.35)) + unit() * 0.55
        facing = wanted - up * wanted.dot(up)
        facing = facing.normalized() if facing.length > 1e-3 else up.orthogonal().normalized()
        half = SIZE_M * dice.uniform(0.48, 0.62)
        middle = stalk + up * half * 0.75
        # The face kept clear: no spray hangs before the eyes and mouth (the front is -y), and
        # none below the neck, so awake the face is seen and asleep the bole stands clear.
        reach_down = middle.z - half * 0.9
        if reach_down < joint["neck_01"].z or (
                middle.y - half < head.y + 0.3 and reach_down < head.z + 0.45 and abs(middle.x - head.x) < 1.5):
            continue
        right = up.cross(facing)
        out_m = (middle - mass).normalized() if (middle - mass).length > 1e-3 else out_crown
        normal = (out_m * 0.45 + out_crown * 0.35 + facing * 0.2).normalized()
        flat = math.hypot(middle.x - centre.x, middle.y - centre.y)
        rise = min(1, max(0, (middle.z - joint["pelvis"].z) / max(1e-3, top_z - joint["pelvis"].z)))
        out = min(1, (middle - centre).length / (crown_reach * 1.1))
        in_mass = min(1, (middle - mass).length / (SIZE_M * 3))
        light = 0.44 + 0.22 * out + 0.16 * in_mass + 0.16 * rise + dice.uniform(-0.13, 0.09)
        h = hue_of[i]
        tint = Vector((light * dice.uniform(0.95, 1.05) * h.x, light * h.y, light * dice.uniform(0.9, 1.0) * h.z))
        tint = Vector((tint.x * TINT.x, tint.y * TINT.y, tint.z * TINT.z))
        # The picture's stalk is its bottom-left corner, its leaves out to the top right: the
        # square laid so its diagonal runs along `up` from the stalk.
        side = 2 * half / math.sqrt(2)
        du, dv = (up + right).normalized() * side, (up - right).normalized() * side
        corners = [stalk, stalk + du, stalk + du + dv, stalk + dv]
        vs = [bm.verts.new(c) for c in corners]
        f = bm.faces.new(vs)
        for loop, (a, b) in zip(f.loops, ((0, 0), (1, 0), (1, 1), (0, 1))):
            loop[uv].uv = (a, b)
            loop[col] = (*tint, 1.0)
        cards.append((range(4 * len(normals), 4 * len(normals) + 4), tree.find(stalk)[1]))
        normals.append(normal)
bm.verts.index_update()
leaves_me = bpy.data.meshes.new("CrownLeaves")
bm.to_mesh(leaves_me)
bm.free()
leaves = bpy.data.objects.new("CrownLeaves", leaves_me)
bpy.context.scene.collection.objects.link(leaves)
for p in leaves_me.polygons:
    p.use_smooth = True
loop_normals = [None] * len(leaves_me.loops)
for p, n in zip(leaves_me.polygons, normals):
    for li in p.loop_indices:
        loop_normals[li] = n[:]
leaves_me.normals_split_custom_set(loop_normals)
for vs, near in cards:
    for name, w in weights[near]:
        g = leaves.vertex_groups.get(name) or leaves.vertex_groups.new(name=name)
        g.add(list(vs), w, "REPLACE")

# 4. Its material: the atlas's oakbranch square and its normals, cut at the atlas's alpha,
# both sides drawn, tinted by each card's shade.
def cell_of(file, name, colour):
    atlas = bpy.data.images.load(look.path(f"assets/materials/foliage/{file}"))
    w, h = atlas.size
    a = np.empty(w * h * 4, dtype=np.float32)
    atlas.pixels.foreach_get(a)
    a = a.reshape(h, w, 4)
    n = w // 4
    x0, y_top = int(CELL[0] * w), int(CELL[1] * h)
    # Blender's rows run from the bottom.
    part = np.ascontiguousarray(a[h - y_top - n:h - y_top, x0:x0 + n])
    im = bpy.data.images.new(name, n, n, alpha=True)
    im.colorspace_settings.name = "sRGB" if colour else "Non-Color"
    im.pixels.foreach_set(part.ravel())
    im.pack()
    bpy.data.images.remove(atlas)
    return im


mat = bpy.data.materials.new("CrownLeaves")
mat.use_nodes = True
mat.use_backface_culling = False
nodes, links = mat.node_tree.nodes, mat.node_tree.links
bsdf = next(n for n in nodes if n.type == "BSDF_PRINCIPLED")
pic = nodes.new("ShaderNodeTexImage")
pic.image = cell_of("cards.png", "crown_leaves", True)
shade = nodes.new("ShaderNodeVertexColor")
shade.layer_name = "Leaf"
mix = nodes.new("ShaderNodeMix")
mix.data_type = "RGBA"
mix.blend_type = "MULTIPLY"
mix.inputs["Factor"].default_value = 1.0
links.new(pic.outputs["Color"], mix.inputs[6])
links.new(shade.outputs["Color"], mix.inputs[7])
links.new(mix.outputs[2], bsdf.inputs["Base Color"])
cut = nodes.new("ShaderNodeMath")
cut.operation = "ROUND"  # the glTF exporter's sign for alpha MASK at 0.5
links.new(pic.outputs["Alpha"], cut.inputs[0])
links.new(cut.outputs[0], bsdf.inputs["Alpha"])
nmap_pic = nodes.new("ShaderNodeTexImage")
nmap_pic.image = cell_of("normal.png", "crown_leaves_normal", False)
nmap = nodes.new("ShaderNodeNormalMap")
links.new(nmap_pic.outputs["Color"], nmap.inputs["Color"])
links.new(nmap.outputs["Normal"], bsdf.inputs["Normal"])
bsdf.inputs["Roughness"].default_value = 0.7
leaves_me.materials.append(mat)

# 5. Joined to the body, so the export's one skinned mesh carries it; the body's own corners
# take a white shade so the bark is untouched if the shade is exported for every part.
bpy.ops.object.select_all(action="DESELECT")
leaves.select_set(True)
body.select_set(True)
bpy.context.view_layer.objects.active = body
bpy.ops.object.join()  # its points are the world's; the join takes them into the body's space
shade_attr = body.data.color_attributes.get("Leaf")
first_leaf = len(body.data.polygons) - len(normals)
for p in body.data.polygons[:first_leaf]:
    for li in p.loop_indices:
        shade_attr.data[li].color = (1, 1, 1, 1)
print("CROWN", f"{len(tips)} clumps, {len(normals)} cards over {len(points)} crown points,",
      f"reach {crown_reach:.2f} m, top {top_z:.2f} m, head {head.z:.2f} m, neck {joint['neck_01'].z:.2f} m")

for o in bpy.context.scene.objects:
    o.select_set(o in (arm, body))
bpy.context.view_layer.objects.active = arm
bpy.ops.export_scene.gltf(filepath=src, export_format="GLB", use_selection=True, export_animations=False)
print("EXPORTED", src)

if SHEET:
    look.stage()
    shots = look.close_up(arm, "Head", 8.0, sides=(0, 90), size=600)
    look.stitch(shots, SHEET)
    print("SHEET", SHEET)
