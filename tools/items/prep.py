"""Prepares a modelled part of an assembled item once (D565 step 4, D566's convention): Tripo's
textured model of a part's cut-out welded closed, turned into its type's template frame, made its
real size, cut to its budget, its opening cut where its prep.json marks it by hand, its points
found from that cut and its maps kept at the part budget. Run from the repository root:

    tools/blender/run.sh tools/items/prep.py art/parts/<kind>/<part> assets/models/parts/<part>.glb OUT.json

The part's prep.json says what a person decided and nothing measured:
  type        its part type (types.json): axe_head, or a trim on the line (collar, cap)
  role        the material role its one material takes (types.json's roles), e.g. Metal
  turn_deg    degrees about up that bring the model's front into the frame (a head's bit to -Z)
  mass_kg     a working part: its mass in `material`, from which its size follows (D565: mass
              from shape and material); or height_m, a trim's real height along the line
  eye         a head's eye, marked by hand (research/2026-10-28-assembled-parts-art.md section 4:
              a tool cannot find it from the shape): {from_back_m, size_m}, the eye's centre that
              far in from the back of the poll, on the poll's middle across, cut round to size_m;
              seat_m, the underside round it cut flat at eye.lower that far out (default 2 cm), so
              a collar under the head meets it in one ring
  bore_m      a trim: turned as a closed lathe on its model's own outer profile (lathe.py), its bore
              exact at bore_m, its wall wall_m out from it (the relief kept in proportion), its
              look baked on from the model; Tripo's tubes are open, so they cannot be cut
  density_kg_m3  a trim's own material, {value, source}: its mass_kg is its volume in it
  edge_band_m a head's bit: how far back from its front-most point the cutting edge runs
  budget      a head: triangles before its cuts add about 340 (art-style.md: a tool's head, 800 at most)

OUT.json gets its points, a trim's cover and mass, in glTF's frame (+Y up, front +Z), copied by hand
into assets/data/parts/<part>.json; part.ts --write then measures its volume and bounds.
Blender's frame is glTF's turned: Blender (x, y, z) is glTF (x, z, -y).
"""

import json
import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "blender"))
sys.path.insert(0, HERE)

import bmesh  # noqa: E402
import bpy  # noqa: E402
from mathutils import Matrix, Vector  # noqa: E402
from mathutils.bvhtree import BVHTree  # noqa: E402

import lathe  # noqa: E402
import pieces  # noqa: E402

DENSITY = {"iron": 7750.0, "stone": 2600.0}  # assets/data/parts/materials.json, read below
TEXTURE_PX = 1024  # types.json's texture_px: colour and normal; roughness and metal at half
RAYS = 32
UP = [0.0, 1.0, 0.0]


def gltf(v):
    return [round(v[0], 5) or 0.0, round(v[2], 5) or 0.0, round(-v[1], 5) or 0.0]


def load(path):
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.import_scene.gltf(filepath=path)
    meshes = [o for o in bpy.context.scene.objects if o.type == "MESH"]
    for o in meshes:
        o.data.transform(o.matrix_world)
        o.parent = None
        o.matrix_world = Matrix.Identity(4)
    for o in list(bpy.context.scene.objects):
        if o.type != "MESH":
            bpy.data.objects.remove(o)
    pieces.only(meshes[0])
    for o in meshes:
        o.select_set(True)
    if len(meshes) > 1:
        bpy.ops.object.join()
    ob = bpy.context.view_layer.objects.active
    if ob.data.has_custom_normals:
        bpy.ops.mesh.customdata_custom_splitnormals_clear()
    return ob


def closed(ob):
    """Welded along its seams, its stray bits gone, its holes filled and its faces outward: a
    closed surface, which the boolean cut needs and part.ts --write measures."""
    bm = bmesh.new()
    bm.from_mesh(ob.data)
    bmesh.ops.remove_doubles(bm, verts=bm.verts, dist=1e-4)
    # Keep the largest piece: a fleck Tripo leaves floating is no part of the part.
    seen, groups = set(), []
    for v in bm.verts:
        if v in seen:
            continue
        group, todo = [], [v]
        seen.add(v)
        while todo:
            w = todo.pop()
            group.append(w)
            for e in w.link_edges:
                o = e.other_vert(w)
                if o not in seen:
                    seen.add(o)
                    todo.append(o)
        groups.append(group)
    groups.sort(key=len, reverse=True)
    stray = [v for g in groups[1:] for v in g]
    bmesh.ops.delete(bm, geom=stray, context="VERTS")
    bmesh.ops.holes_fill(bm, edges=bm.edges, sides=0)
    bmesh.ops.triangulate(bm, faces=bm.faces)
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    bm.to_mesh(ob.data)
    bm.free()
    return len(stray)


def open_edges(ob):
    bm = bmesh.new()
    bm.from_mesh(ob.data)
    n = sum(1 for e in bm.edges if not e.is_manifold)
    vol = bm.calc_volume(signed=True)
    bm.free()
    return n, vol


def cut_round(ob, centre, diameter, below=None, sides=40):
    """Cuts a round opening straight up through the part at `centre` (x, y), `diameter` across:
    the eye or bore is made, so it is exactly where and as wide as its points say; or, `below` a
    height, only beneath it (a seat)."""
    radius = diameter / 2 / math.cos(math.pi / sides)  # its flats touch the circle, never inside it
    lo = min(v.co.z for v in ob.data.vertices) - 0.05
    hi = max(v.co.z for v in ob.data.vertices) + 0.05 if below is None else below
    bpy.ops.mesh.primitive_cylinder_add(vertices=sides, radius=radius, depth=hi - lo, location=(centre[0], centre[1], (lo + hi) / 2))
    cutter = bpy.context.active_object
    mod = ob.modifiers.new("eye", "BOOLEAN")
    mod.operation = "DIFFERENCE"
    mod.solver = "EXACT"
    mod.object = cutter
    pieces.only(ob)
    print("boolean:", bpy.ops.object.modifier_apply(modifier=mod.name))
    bpy.data.objects.remove(cutter)


def walled(tree, centre, z, r):
    """Whether a ring at height z round the line meets a wall within 1 mm past r all round."""
    for i in range(RAYS):
        a = 2 * math.pi * i / RAYS
        d = Vector((math.cos(a), math.sin(a), 0.0))
        hit = tree.ray_cast(Vector((centre[0], centre[1], z)), d, r + 0.001)
        if hit[0] is None:
            return False
    return True


def opening_span(ob, centre, r):
    """The lowest and highest heights at which the cut opening is walled all round, 0.5 mm in."""
    tree = BVHTree.FromObject(ob, bpy.context.evaluated_depsgraph_get())
    lo = min(v.co.z for v in ob.data.vertices)
    hi = max(v.co.z for v in ob.data.vertices)
    steps = int((hi - lo) / 0.00025)
    full = [lo + k * 0.00025 for k in range(steps + 1) if walled(tree, centre, lo + k * 0.00025, r)]
    if not full:
        raise SystemExit(f"prep: no height is walled all round the cut at {centre}: the mark is off the part, or the part is thinner than its opening; move the mark (prep.json)")
    # One walled run: the opening is the longest run of walled heights.
    runs, start = [], full[0]
    for a, b in zip(full, full[1:] + [None]):
        if b is None or b - a > 0.0006:
            runs.append((start, a))
            start = b
    lo_z, hi_z = max(runs, key=lambda s: s[1] - s[0])
    return lo_z + 0.0005, hi_z - 0.0005


def poll_middle(ob, y, band=0.005):
    """The middle across (x) of the part near y: an eye is cut on the poll's middle."""
    xs = [v.co.x for v in ob.data.vertices if abs(v.co.y - y) < band]
    return (min(xs) + max(xs)) / 2, max(xs) - min(xs)


def edge_points(ob, band):
    """A head's cutting edge, its bit's front (-Z in glTF, +y here): the front-most point of each
    5 mm of height, kept where within `band` of the front-most of all; its lowest and highest."""
    front = {}
    for v in ob.data.vertices:
        k = round(v.co.z / 0.005)
        if k not in front or v.co.y > front[k].y:
            front[k] = v.co.copy()
    most = max(p.y for p in front.values())
    keep = sorted((p for p in front.values() if p.y > most - band), key=lambda p: p.z)
    return keep[0], keep[-1]


def look(ob, path):
    """The part from its side (glTF +X), from above and from its front (glTF -Z), in its own
    colours, in one row: where the eye was marked and cut shows, so the mark is checked by eye."""
    scene = bpy.context.scene
    scene.render.engine = "BLENDER_WORKBENCH"
    scene.display.shading.color_type = "TEXTURE"
    scene.display.shading.light = "STUDIO"
    scene.render.resolution_x = scene.render.resolution_y = 512
    scene.render.film_transparent = False
    corners = [ob.matrix_world @ Vector(c) for c in ob.bound_box]
    mid = sum(corners, Vector()) / 8
    size = max((max(c[k] for c in corners) - min(c[k] for c in corners)) for k in range(3)) * 1.15
    cam = bpy.data.objects.new("look", bpy.data.cameras.new("look"))
    scene.collection.objects.link(cam)
    cam.data.type = "ORTHO"
    cam.data.ortho_scale = size
    cam.data.clip_start = size / 100
    scene.camera = cam
    shots = []
    for i, (where, rot) in enumerate(((Vector((1, 0, 0)), (math.pi / 2, 0, math.pi / 2)), (Vector((0, 0, 1)), (0, 0, 0)), (Vector((0, 1, 0)), (math.pi / 2, 0, math.pi)))):
        cam.location = mid + where * size * 2
        cam.rotation_euler = rot
        scene.render.filepath = f"{path}-{i}.png"
        bpy.ops.render.render(write_still=True)
        shots.append(scene.render.filepath)
    bpy.data.objects.remove(cam)
    return shots


def textures(ob, role):
    mat = ob.material_slots[0].material
    mat.name = role
    for slot in ob.material_slots[1:]:
        slot.material = mat
    for image in bpy.data.images:
        w, h = image.size
        if not w:
            continue
        colour = any(
            l.to_node.type == "NORMAL_MAP" or (l.to_node.type == "BSDF_PRINCIPLED" and l.to_socket.name == "Base Color")
            for n in mat.node_tree.nodes
            if n.type == "TEX_IMAGE" and n.image == image
            for l in n.outputs["Color"].links
        )
        most = TEXTURE_PX if colour else TEXTURE_PX // 2
        if max(w, h) > most:
            image.scale(w * most // max(w, h), h * most // max(w, h))
        # The glTF exporter's AUTO keeps a packed image's own format, Tripo's JPEG, read from its
        # bytes when it has no path (io_scene_gltf2 image.py, __is_blender_image_a_jpeg); a .png path
        # makes it encode PNG, which part.ts reads the size of.
        image.file_format = "PNG"
        image.filepath_raw = f"{bpy.path.clean_name(image.name)}.png"


def main():
    args = sys.argv[sys.argv.index("--") + 1 :]
    folder, out_glb, out_json = args
    with open(os.path.join(folder, "prep.json")) as f:
        prep = json.load(f)
    with open(os.path.join(HERE, "..", "..", "assets", "data", "parts", "materials.json")) as f:
        for name, m in json.load(f)["materials"].items():
            DENSITY[name] = m["density_kg_m3"]["value"]
    ob = load(os.path.join(folder, "model.glb"))
    stray = closed(ob)
    raw = [max(c) - min(c) for c in zip(*[v.co for v in ob.data.vertices])]
    print(f"prep: as made {raw[0]:.3f} x {raw[2]:.3f} x {raw[1]:.3f} (glTF x, y up, z)")
    for name, axis in (("x", 0), ("z", 1)):
        cs = [v.co[axis] for v in ob.data.vertices]
        for end, at in (("-", min(cs)), ("+", max(cs))):
            near = [v.co for v in ob.data.vertices if abs(v.co[axis] - at) < 0.1 * (max(cs) - min(cs))]
            other = 1 - axis
            print(f"prep: at glTF {end}{name}, {max(p[other] for p in near) - min(p[other] for p in near):.3f} thick")
    ob.data.transform(Matrix.Rotation(math.radians(prep.get("turn_deg", 0)), 4, "Z"))
    _, vol = open_edges(ob)
    if "mass_kg" in prep:
        want = prep["mass_kg"] / DENSITY[prep["material"]]
        scale = (want / abs(vol)) ** (1 / 3)
    else:
        scale = prep["height_m"] / (max(v.co.z for v in ob.data.vertices) - min(v.co.z for v in ob.data.vertices))
    ob.data.transform(Matrix.Scale(scale, 4))
    if "bore_m" in prep:
        return turned_trim(ob, prep, folder, out_glb, out_json)
    # The cut's walls and rims take about 300 triangles of the budget.
    pieces.cut(ob, prep["budget"] - 340)
    closed(ob)
    xs = [v.co.x for v in ob.data.vertices]
    ys = [v.co.y for v in ob.data.vertices]
    zs = [v.co.z for v in ob.data.vertices]
    print(f"prep: made {(max(xs) - min(xs)) * 100:.1f} x {(max(zs) - min(zs)) * 100:.1f} x {(max(ys) - min(ys)) * 100:.1f} cm (across, up, along)")
    if "eye" in prep:
        # What the hand marks from: across and up, every centimetre in from the back.
        row = []
        for cm in range(1, 9):
            at = min(ys) + cm / 100
            near = [v.co for v in ob.data.vertices if abs(v.co.y - at) < 0.004]
            if near:
                row.append(f"{cm}: {(max(p.x for p in near) - min(p.x for p in near)) * 100:.1f}/{(max(p.z for p in near) - min(p.z for p in near)) * 100:.1f}")
        print("prep: cm from the back: across/up cm, " + ", ".join(row))
        y = min(ys) + prep["eye"]["from_back_m"]  # the back of the poll is glTF +Z, Blender -y
        x, across = poll_middle(ob, y)
        size = prep["eye"]["size_m"]
        want = prep["eye"].get("across_m")
        if want and want > across:
            # The generator draws a cheek's thickness, not a poll's: swell the poll round the eye
            # to `across_m`, as a real head swells round its eye, easing back to the cheek.
            # Full across the eye and its walls, easing to nothing over `reach_m` past them.
            reach = prep["eye"].get("reach_m", 0.03)
            # Each 3 mm along swells by what brings its own thickness there to `across_m`, at most
            # threefold, so the eye has its walls all along its length.
            span = size / 2 + 0.006
            tree = BVHTree.FromObject(ob, bpy.context.evaluated_depsgraph_get())
            def thickness(at):
                best = 0.0
                for k in range(41):
                    z = min(zs) + (max(zs) - min(zs)) * k / 40
                    a = tree.ray_cast(Vector((x - 1, at, z)), Vector((1, 0, 0)), 2.0)[0]
                    b = tree.ray_cast(Vector((x + 1, at, z)), Vector((-1, 0, 0)), 2.0)[0]
                    if a is not None and b is not None:
                        best = max(best, b.x - a.x)
                return best
            steps = {k: thickness(y + k * 0.003) for k in range(-int(span / 0.003) - 1, int(span / 0.003) + 2)}
            def factor(at):
                t = steps.get(round((at - y) / 0.003), 0.0)
                return min(3.0, max(1.0, want / t)) if t > 0.002 else 1.0
            for v in ob.data.vertices:
                t = min(1.0, max(0.0, 1.0 - (abs(v.co.y - y) - span) / reach))
                inside = y + max(-span, min(span, v.co.y - y))
                v.co.x = x + (v.co.x - x) * (1.0 + (factor(inside) - 1.0) * t * t * (3 - 2 * t))
            x, across = poll_middle(ob, y)
        print(f"prep: eye {size * 100:.1f} cm at {prep['eye']['from_back_m'] * 100:.1f} cm from the back, the poll {across * 100:.1f} cm across there")
    shots = os.path.join(os.path.dirname(os.path.abspath(out_json)), os.path.basename(folder.rstrip("/")))
    look(ob, shots + "-before")
    cut_round(ob, (x, y), size)
    look(ob, shots + "-after")
    bm = bmesh.new()
    bm.from_mesh(ob.data)
    bmesh.ops.triangulate(bm, faces=bm.faces)
    bm.to_mesh(ob.data)
    bm.free()
    lo, hi = opening_span(ob, (x, y), size / 2)
    # The seat: the underside round the eye made flat at eye.lower, as a forged head's is, so a
    # collar under it meets it in one ring and nothing of the head hangs into the collar (D566).
    seat = prep["eye"].get("seat_m", 0.02)
    cut_round(ob, (x, y), 2 * seat, below=lo, sides=12)
    bm = bmesh.new()
    bm.from_mesh(ob.data)
    # The cuts split the faces round them into slivers: the flat ones (the seat, each facet of the
    # eye) merge back into one face each, UV islands kept, then the part is triangles again.
    bmesh.ops.dissolve_limit(bm, angle_limit=math.radians(0.5), verts=bm.verts, edges=bm.edges, delimit={"UV"})
    bmesh.ops.triangulate(bm, faces=bm.faces, quad_method="BEAUTY", ngon_method="BEAUTY")
    bm.to_mesh(ob.data)
    bm.free()
    ob.data.transform(Matrix.Translation(-Vector((x, y, lo))))
    a, b = edge_points(ob, prep.get("edge_band_m", 0.03))
    points = [
        {"name": "eye.lower", "type": "eye", "at": [0.0, 0.0, 0.0], "dir": UP, "size": size},
        {"name": "eye.upper", "type": "eye", "at": [0.0, round(hi - lo, 5), 0.0], "dir": UP, "size": size},
        {"name": "edge.lower", "type": "edge", "at": gltf(a), "dir": [0.0, 0.0, -1.0]},
        {"name": "edge.upper", "type": "edge", "at": gltf(b), "dir": [0.0, 0.0, -1.0]},
    ]
    pieces.only(ob)
    bpy.ops.object.shade_smooth_by_angle(angle=math.radians(50.0), keep_sharp_edges=True)
    textures(ob, prep["role"])
    export(ob, folder, out_glb, out_json, {"points": points}, f"{stray} stray vertices dropped")


def turned_trim(raw, prep, folder, out_glb, out_json):
    """A trim on the line, Tripo's tube with its own hole: turned as a closed lathe (lathe.py) on
    the tube's own outer profile, its bore exact at bore_m and its wall wall_m thick, the relief
    kept in proportion; the model, moved to the same radii, gives only its look, baked on."""
    zs = [v.co.z for v in raw.data.vertices]
    centre = ((min(v.co.x for v in raw.data.vertices) + max(v.co.x for v in raw.data.vertices)) / 2, (min(v.co.y for v in raw.data.vertices) + max(v.co.y for v in raw.data.vertices)) / 2)
    bore, wall, sides = prep["bore_m"], prep["wall_m"], prep.get("sides", 24)
    r_in = lathe.inner_radius(raw, centre)
    profile = lathe.outer_profile(raw, centre, prep.get("rings", 12))
    r_mid = sorted(r for _, r in profile)[len(profile) // 2]
    k = wall / (r_mid - r_in)

    def radius(r):
        return bore / 2 + (r - r_in) * k if r >= r_in else r * (bore / 2) / r_in

    print(f"prep: tube as made {r_in * 2000:.1f} mm hole, {r_mid * 2000:.1f} mm across; made {bore * 1000:.1f} mm bore, {(bore / 2 + wall) * 2000:.1f} mm across, {(max(zs) - min(zs)) * 1000:.1f} mm high")
    lathe.remap(raw, centre, radius)
    raw.data.transform(Matrix.Translation(-Vector((centre[0], centre[1], 0.0))))
    ob = lathe.lathe(os.path.basename(folder.rstrip("/")), [(z, radius(r)) for z, r in profile], bore, sides)
    shots = os.path.join(os.path.dirname(os.path.abspath(out_json)), os.path.basename(folder.rstrip("/")))
    lathe.bake(raw, ob, prep["role"], prep.get("texture_px", 512), 1.0 if prep["role"] == "Metal" else 0.0, wall)
    bpy.data.objects.remove(raw)
    opening_span(ob, (0.0, 0.0), bore / 2)  # refuses a bore not walled all round
    mid = (min(zs) + max(zs)) / 2
    ob.data.transform(Matrix.Translation(-Vector((0.0, 0.0, mid))))
    pieces.only(ob)
    bpy.ops.object.shade_smooth_by_angle(angle=math.radians(50.0), keep_sharp_edges=True)
    look(ob, shots + "-after")
    _, vol = open_edges(ob)
    d = prep["density_kg_m3"]
    export(ob, folder, out_glb, out_json, {
        "points": [{"name": "bore", "type": "bore", "at": [0.0, 0.0, 0.0], "dir": UP, "size": bore}],
        # The bore is exact its whole height, so the trim covers from end to end.
        "cover_m": [round(min(zs) - mid, 5), round(max(zs) - mid, 5)],
        # A trim keeps its own material, so it gives its mass (part.schema.json): its closed
        # volume in its own density, the density's source carried with it.
        "mass_kg": {"value": round(abs(vol) * d["value"], 4), "source": f"{abs(vol) * 1e6:.2f} cm^3 measured by tools/items/prep.py, at {d['value']} kg/m^3: {d['source']}"},
    }, "turned")


def export(ob, folder, out_glb, out_json, result, note):
    n_open, vol = open_edges(ob)
    dims = [max(c) - min(c) for c in zip(*[v.co for v in ob.data.vertices])]
    tris = pieces.triangles(ob)
    print(f"prep: {tris} triangles, {n_open} open edges, {note}, {vol * 1e6:.1f} cm^3, {dims[0] * 100:.1f} x {dims[2] * 100:.1f} x {dims[1] * 100:.1f} cm (x, up, along)")
    if n_open:
        raise SystemExit(f"prep: {n_open} open edges after the cut: the part is not closed; look at {out_glb} in Blender (Select All by Trait > Non Manifold)")
    os.makedirs(os.path.dirname(os.path.abspath(out_glb)), exist_ok=True)
    pieces.only(ob)
    ob.name = os.path.basename(folder.rstrip("/"))
    bpy.ops.export_scene.gltf(filepath=os.path.abspath(out_glb), export_format="GLB", use_selection=True, export_tangents=True, export_image_format="AUTO")
    with open(out_json, "w") as f:
        json.dump({**result, "triangles": tris}, f, indent=1)
    print(f"prep: {out_glb}, {out_json}")


main()
