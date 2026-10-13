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

import bmesh
import bpy
from mathutils import Matrix, Vector


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


def bake(piece, source, name, role, size, reach):
    """Gives `piece` a new unwrap and a material named `role`, its colour, normals and roughness
    baked from `source` (a model of the base person wearing it, lined up with the body) at
    `size` pixels, looking `reach` out and in from the piece. Returns the material."""
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
    colour = bpy.data.images.new(f"{name}_colour", size, size)
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
    scene.render.bake.use_selected_to_active = True
    scene.render.bake.cage_extrusion = 2 * reach
    scene.render.bake.max_ray_distance = 4 * reach
    scene.render.bake.margin = 16
    bpy.ops.object.select_all(action="DESELECT")
    source.select_set(True)
    piece.select_set(True)
    bpy.context.view_layer.objects.active = piece
    bpy.ops.object.bake(type="DIFFUSE")
    colour.pack()
    # Its surface as well as its colour, so leather reads as leather and not as a printed sheet.
    for kind in ("NORMAL", "ROUGHNESS"):
        image = bpy.data.images.new(f"{name}_{kind.lower()}", size, size, is_data=True)
        image.colorspace_settings.name = "Non-Color"
        node = nodes.new("ShaderNodeTexImage")
        node.image = image
        nodes.active = node
        bpy.ops.object.bake(type=kind, normal_space="TANGENT")
        image.pack()
        if kind == "NORMAL":
            mapping = nodes.new("ShaderNodeNormalMap")
            mat.node_tree.links.new(node.outputs["Color"], mapping.inputs["Color"])
            mat.node_tree.links.new(mapping.outputs["Normal"], bsdf.inputs["Normal"])
        else:
            mat.node_tree.links.new(node.outputs["Color"], bsdf.inputs["Roughness"])
    nodes.active = tex
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
