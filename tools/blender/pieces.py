"""What the scripts that make models from scans share: a mesh's triangles, welding it along
its seams, and cutting it to a triangle budget (Blender's collapse decimation, which keeps its
texture places, so its scanned maps still fit). Imported by scan.py and campfire.py."""

import bpy


def triangles(ob):
    return sum(len(p.vertices) - 2 for p in ob.data.polygons)


def only(ob):
    """Makes `ob` the one selected object and the active one, as operators want."""
    bpy.ops.object.select_all(action="DESELECT")
    ob.select_set(True)
    bpy.context.view_layer.objects.active = ob


def weld(ob, threshold=0.0005):
    """Scans are often split along their texture seams; welded, a part is cut as one."""
    only(ob)
    bpy.ops.object.mode_set(mode="EDIT")
    bpy.ops.mesh.select_all(action="SELECT")
    bpy.ops.mesh.remove_doubles(threshold=threshold)
    bpy.ops.object.mode_set(mode="OBJECT")


def cut(ob, budget):
    """Cuts `ob` to about `budget` triangles. One pass can stop short on a scan with many seams
    and loose pieces; a few more get it there."""
    for _ in range(6):
        now = triangles(ob)
        if now <= budget * 1.05:
            break
        modifier = ob.modifiers.new("cut", "DECIMATE")
        modifier.decimate_type = "COLLAPSE"
        modifier.ratio = budget / now
        modifier.use_collapse_triangulate = True
        only(ob)
        bpy.ops.object.modifier_apply(modifier=modifier.name)
