"""Cuts the clips the game plays out of Quaternius's animation libraries into the game's
mannequin file: the libraries' mannequin and skeleton, with only the clips listed.

    tools/blender/run.sh tools/characters/clips.py [LIST] [OUT]

LIST defaults to art/animation/clips.txt, one clip name a line; OUT to the game's
assets/models/character/mannequin.glb. The libraries (Universal Animation Library 1 and 2,
version 3.0 and 2.2, both on one skeleton with Unreal's bone names) are read from
art/animation/library/, where each developer keeps the copies bought from Quaternius
(art/README.md); they are not committed. A clip in both is taken from the first library. A
clip listed that no library has must be one we made (art/animation/made.json), which
made_clips.py adds after this.
"""

import glob
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import bpy

import look

LIBRARY = os.path.join(look.ART, "animation", "library")
args = look.args()
listed = look.path(args[0]) if args else os.path.join(look.ART, "animation", "clips.txt")
out = look.path(args[1]) if len(args) > 1 else look.FIGURE
with open(listed) as f:
    wanted = [line.strip() for line in f if line.strip() and not line.startswith("#")]
# A clip we made that the libraries do not have (made.json) is added by made_clips.py after.
with open(os.path.join(look.ART, "animation", "made.json")) as f:
    made = json.load(f)

look.reset()
libraries = sorted(glob.glob(os.path.join(LIBRARY, "*.glb")))
if not libraries:
    raise SystemExit(f"no libraries in {LIBRARY}: see art/README.md")
found = {}
arm = body = None
for path in libraries:
    before = set(bpy.data.objects)
    had = set(bpy.data.actions)
    bpy.ops.import_scene.gltf(filepath=path)
    new = [o for o in bpy.data.objects if o not in before]
    for a in bpy.data.actions:
        if a not in had:
            # A name already taken comes in as "Name.001".
            found.setdefault(a.name.split(".")[0], a)
    if arm is None:
        arm = next(o for o in new if o.type == "ARMATURE")
        body = next(o for o in new if o.type == "MESH" and any(m.type == "ARMATURE" for m in o.modifiers))
        new = [o for o in new if o not in (arm, body)]
    for o in new:
        bpy.data.objects.remove(o)
missing = [w for w in wanted if w not in found and w not in made]
if missing:
    raise SystemExit(f"not in the libraries: {', '.join(missing)}")
wanted = [w for w in wanted if w in found]

# Only the clips listed go into the file: the exporter writes every action it can bind.
for a in list(bpy.data.actions):
    if found.get(a.name.split(".")[0]) is not a or a.name.split(".")[0] not in wanted:
        bpy.data.actions.remove(a)
ad = arm.animation_data or arm.animation_data_create()
ad.action = None
# The importer leaves a track per clip of the first library; only ours are exported.
for t in list(ad.nla_tracks):
    ad.nla_tracks.remove(t)
for name in wanted:
    a = found[name]
    a.name = name
    track = ad.nla_tracks.new()
    track.name = name
    strip = track.strips.new(name, int(a.frame_range[0]), a)
    if getattr(a, "slots", None) and len(a.slots):
        strip.action_slot = a.slots[0]
    track.mute = True
for pb in arm.pose.bones:
    pb.matrix_basis.identity()
for o in bpy.context.scene.objects:
    o.select_set(o in (arm, body))
bpy.context.view_layer.objects.active = arm
bpy.ops.export_scene.gltf(
    filepath=out, export_format="GLB", use_selection=True,
    export_animations=True, export_animation_mode="NLA_TRACKS", export_force_sampling=True,
    export_influence_nb=4,
)
print("CLIPS", len(wanted), "from", ", ".join(os.path.basename(p) for p in libraries), "to", os.path.relpath(out, look.ROOT))
