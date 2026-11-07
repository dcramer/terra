"""Gathers a modular kit's parts into the modules a layout places, such as Poly Haven's scanned
building facades into one-storey bays: each module its parts joined into one mesh, cut to a
triangle budget (pieces.py's collapse decimation, which keeps its texture places), its foot on
the ground and its middle over the origin across, written as a scene of one glTF binary, so
every module shares the kit's textures. Then a preview of them all side by side.

    tools/blender/run.sh tools/blender/kit.py SPEC.json OUT.glb [PREVIEW.png]

SPEC is JSON: `source` a Poly Haven asset's id (fetched from api.polyhaven.com at
`resolution`, glTF with its textures, into a folder beside OUT) or a local .gltf path;
`budget` a module's triangles; `modules` a list of `{name, parts: [[node, height], ...]}` in
the order of the file's scenes. Each part is the source's node by name, its origin put at the
module's (a modular kit's pieces share a corner as their origin), raised by `height` metres;
`alpha` an optional `{material: alpha}` set on the output's materials (glass over an opaque map).
Prints KIT with each module's triangles before and after and its size."""

import json
import os
import sys
import urllib.request

import bpy
from mathutils import Matrix, Vector
from pieces import cut, only, triangles


def get(url):
    """A request Poly Haven answers: it refuses Python's own User-Agent (403)."""
    return urllib.request.urlopen(urllib.request.Request(url, headers={"User-Agent": "terra-tools/1 (tools/blender/kit.py)"}))


def fetch(asset, resolution, into):
    """Poly Haven's glTF of `asset` and every file it includes, into `into`; its path."""
    with get(f"https://api.polyhaven.com/files/{asset}") as r:
        files = json.load(r)
    entry = files["gltf"][resolution]["gltf"]
    wanted = {os.path.basename(entry["url"]): entry["url"]}
    wanted.update({rel: inc["url"] for rel, inc in entry.get("include", {}).items()})
    for rel, url in wanted.items():
        path = os.path.join(into, rel)
        if os.path.exists(path):
            continue
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with get(url) as r, open(path, "wb") as f:
            f.write(r.read())
    return os.path.join(into, os.path.basename(entry["url"]))


def finish(path, alpha):
    """Rewrites the GLB at `path` without its scenes that hold no nodes (the emptied work
    scene), scene 0 its default: Bevy's glTF loader refuses the whole file over a scene
    without `nodes` ("missing field `nodes`", iso-kit j26). Each material named in `alpha`
    gets that base colour alpha: Poly Haven's facade glass blends on an opaque JPEG, so it hid
    the rooms behind it. The binary chunk is kept as is."""
    with open(path, "rb") as f:
        data = f.read()
    size = int.from_bytes(data[12:16], "little")
    doc = json.loads(data[20 : 20 + size])
    doc["scenes"] = [s for s in doc["scenes"] if s.get("nodes")]
    doc["scene"] = 0
    for m in doc.get("materials", []):
        if m.get("name") in alpha:
            pbr = m.setdefault("pbrMetallicRoughness", {})
            pbr["baseColorFactor"] = pbr.get("baseColorFactor", [1, 1, 1, 1])[:3] + [alpha[m["name"]]]
            m["alphaMode"] = "BLEND"
    text = json.dumps(doc, separators=(",", ":")).encode()
    text += b" " * (-len(text) % 4)
    rest = data[20 + size :]
    head = b"glTF" + (2).to_bytes(4, "little") + (20 + len(text) + len(rest)).to_bytes(4, "little")
    with open(path, "wb") as f:
        f.write(head + len(text).to_bytes(4, "little") + b"JSON" + text + rest)


def module(work, spec, by_name, budget):
    """One module of the spec as one mesh object in `work`, its origin at its foot's middle."""
    parts = []
    for node, height in spec["parts"]:
        src = by_name.get(node)
        if src is None:
            raise SystemExit(f"kit: module {spec['name']} names part {node}, which the source has not: fix the spec's name (the source's nodes are its objects)")
        ob = src.copy()
        ob.data = src.data.copy()
        work.collection.objects.link(ob)
        ob.location = Vector((0.0, 0.0, float(height)))  # Blender is z up: glTF's y
        parts.append(ob)
    bpy.ops.object.select_all(action="DESELECT")
    for ob in parts:
        ob.select_set(True)
    bpy.context.view_layer.objects.active = parts[0]
    if len(parts) > 1:
        bpy.ops.object.join()
    ob = parts[0]
    only(ob)
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    xs = [v.co.x for v in ob.data.vertices]
    ob.data.transform(Matrix.Translation((-(min(xs) + max(xs)) / 2.0, 0.0, 0.0)))
    before = triangles(ob)
    cut(ob, budget)
    ob.name = ob.data.name = spec["name"]
    lo = [min(v.co[i] for v in ob.data.vertices) for i in range(3)]
    hi = [max(v.co[i] for v in ob.data.vertices) for i in range(3)]
    # Reported as glTF's x (across), y (up), z (out toward the street).
    size = (hi[0] - lo[0], hi[2] - lo[2], hi[1] - lo[1])
    print(f"KIT {spec['name']}: {before} to {triangles(ob)} triangles, {size[0]:.2f} x {size[1]:.2f} x {size[2]:.2f} m")
    return ob


def preview(work, modules, out):
    """The modules side by side from the front, rendered by Cycles on the CPU."""
    x = 0.0
    for ob in modules:
        work.collection.objects.link(ob)
        w = ob.dimensions.x
        ob.location = Vector((x + w / 2.0, 0.0, 0.0))
        x += w + 0.6
    cam = bpy.data.objects.new("front", bpy.data.cameras.new("front"))
    cam.data.type = "ORTHO"
    cam.data.ortho_scale = x + 0.6
    cam.location = Vector((x / 2.0 - 0.3, -30.0, 1.6))
    cam.rotation_euler = (1.5708, 0.0, 0.0)
    work.collection.objects.link(cam)
    work.camera = cam
    sun = bpy.data.objects.new("sun", bpy.data.lights.new("sun", "SUN"))
    sun.data.energy = 3.0
    sun.rotation_euler = (1.1, 0.0, 0.5)
    work.collection.objects.link(sun)
    if work.world is None:
        work.world = bpy.data.worlds.new("sky")
    work.world.color = (0.25, 0.27, 0.3)
    work.render.engine = "CYCLES"
    work.cycles.device = "CPU"
    work.cycles.samples = 24
    work.render.resolution_x = 1920
    work.render.resolution_y = int(1920 * 4.0 / (x + 0.6))
    work.render.filepath = out
    bpy.ops.render.render(write_still=True, scene=work.name)


def main(argv):
    spec_path, out = os.path.abspath(argv[0]), os.path.abspath(argv[1])
    shot = os.path.abspath(argv[2]) if len(argv) > 2 else None
    with open(spec_path) as f:
        spec = json.load(f)
    src = spec["source"]
    if not src.endswith(".gltf"):
        src = fetch(src, spec.get("resolution", "2k"), os.path.join(os.path.dirname(out), "source", src))
    for ob in list(bpy.data.objects):  # the factory scene's cube, camera and light
        bpy.data.objects.remove(ob)
    work = bpy.context.scene
    work.name = "zz work"  # scenes export in name order: the work scene last, emptied first
    bpy.ops.import_scene.gltf(filepath=src)
    sources = [ob for ob in work.objects if ob.type == "MESH"]
    by_name = {ob.name: ob for ob in sources}
    made = [module(work, m, by_name, spec.get("budget", 20000)) for m in spec["modules"]]
    for ob in sources:
        bpy.data.objects.remove(ob)
    for i, ob in enumerate(made):
        scene = bpy.data.scenes.new(f"{i:02d} {ob.name}")
        scene.collection.objects.link(ob)
        work.collection.objects.unlink(ob)
    bpy.ops.export_scene.gltf(
        filepath=out,
        export_format="GLB",
        use_active_scene=False,
        export_animations=False,
        export_yup=True,
        export_image_format="AUTO",
        export_jpeg_quality=90,
    )
    finish(out, spec.get("alpha", {}))
    print(f"KIT {len(made)} modules to {out}, {os.path.getsize(out) / 1e6:.1f} MB")
    if shot:
        preview(work, made, shot)
        print(f"KIT preview {shot}")


if __name__ == "__main__":
    main(sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else sys.argv[1:])
