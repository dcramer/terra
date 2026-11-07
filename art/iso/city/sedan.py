"""The neon street's parked car (iso-kit j77): Daniel Zhabotinsky's "Kiri '86 - Low poly model"
(a made-up compact Japanese sedan of the 1980s, no maker's marks; CC BY 4.0,
https://sketchfab.com/3d-models/5c17c582ef9549798f694b660feea42a), from its Objaverse mirror on
Hugging Face (allenai/objaverse, Sketchfab's own GLB of it, downloadable without an account),
made into one prop as the city's others are: its hidden engine and suspension dropped, its parts
joined, its length along Z, its width along X, its middle over the origin and its tyres on y=0,
its textures written as JPEG (PNG where they carry alpha) and the interior's, badges' and plates'
halved to 512 px.

  tools/blender/run.sh art/iso/city/sedan.py OUT.glb

Prints the triangles, the size in metres and the file's size."""

import os
import sys
import tempfile
import urllib.request

import bpy
from mathutils import Matrix, Vector

SOURCE = (
    "https://huggingface.co/datasets/allenai/objaverse/resolve/main/"
    "glbs/000-084/5c17c582ef9549798f694b660feea42a.glb"
)
# Meshes under the closed hood and the floor, never seen from the ARPG camera.
DROP = ("Engine", "Suspension_front", "Suspension_rear")
# Materials whose textures are seen only small (through the glass, a badge, a plate).
SMALL = ("GEN4DR_INTERIOR", "GENERIC_BADGES", "Numberplates_dif")


def triangles(ob):
    return sum(len(p.vertices) - 2 for p in ob.data.polygons)


def main(args):
    out = args[0]
    bpy.ops.wm.read_factory_settings(use_empty=True)
    src = os.path.join(tempfile.mkdtemp(), "kiri86.glb")
    req = urllib.request.Request(SOURCE, headers={"User-Agent": "terra-tools/1 (art/iso/city/sedan.py)"})
    with urllib.request.urlopen(req) as r, open(src, "wb") as f:
        f.write(r.read())
    bpy.ops.import_scene.gltf(filepath=src)
    scene = bpy.context.scene
    for ob in list(scene.objects):
        if ob.type == "MESH" and ob.data.name.startswith(DROP):
            bpy.data.objects.remove(ob)
    meshes = [ob for ob in scene.objects if ob.type == "MESH"]
    for ob in meshes:
        world = ob.matrix_world.copy()
        ob.parent = None
        ob.matrix_world = world
    for ob in list(scene.objects):
        if ob.type != "MESH":
            bpy.data.objects.remove(ob)
    bpy.ops.object.select_all(action="DESELECT")
    for ob in meshes:
        ob.select_set(True)
    bpy.context.view_layer.objects.active = meshes[0]
    bpy.ops.object.make_single_user(object=True, obdata=True)
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    bpy.ops.object.join()
    car = bpy.context.view_layer.objects.active

    pts = [v.co for v in car.data.vertices]
    lo = Vector((min(p.x for p in pts), min(p.y for p in pts), min(p.z for p in pts)))
    hi = Vector((max(p.x for p in pts), max(p.y for p in pts), max(p.z for p in pts)))
    size = hi - lo
    print(f"sedan: source size {size.x:.3f} x {size.y:.3f} x {size.z:.3f} (Blender x y z)")
    fix = Matrix.Translation(-Vector(((lo.x + hi.x) / 2, (lo.y + hi.y) / 2, lo.z)))
    if size.x > size.y:
        # Length along Blender's Y, which the exporter writes as glTF's Z.
        fix = Matrix.Rotation(1.5707963, 4, "Z") @ fix
    length = max(size.x, size.y)
    if not 3.8 <= length <= 5.0:
        fix = Matrix.Scale(4.35 / length, 4) @ fix
        print(f"sedan: length {length:.3f} is no car's, scaled to 4.35 m")
    car.data.transform(fix)

    for mat in car.data.materials:
        if mat and mat.name.startswith(SMALL) and mat.node_tree:
            for node in mat.node_tree.nodes:
                if node.type == "TEX_IMAGE" and node.image and node.image.size[0] > 512:
                    node.image.scale(512, 512)

    pts = [v.co for v in car.data.vertices]
    dims = [max(p[i] for p in pts) - min(p[i] for p in pts) for i in range(3)]
    bpy.ops.export_scene.gltf(
        filepath=out,
        export_format="GLB",
        export_animations=False,
        export_yup=True,
        export_image_format="JPEG",
    )
    print(
        f"sedan: {triangles(car)} triangles, {dims[0]:.2f} x {dims[2]:.2f} x {dims[1]:.2f} m "
        f"(glTF x y z), {os.path.getsize(out) / 1e6:.2f} MB, to {out}"
    )


if __name__ == "__main__":
    main(sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else sys.argv[1:])
