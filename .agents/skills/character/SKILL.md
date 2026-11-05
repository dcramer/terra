---
name: character
description: Make a game character for Terra (a base body, an NPC, a body template or morph) from a design sheet to a rigged, animated, checked model in the game: concept image, Tripo model and auto-rig on the Studio subscription, conversion onto our skeleton, the rig checker, export, and a look in the sandbox's workbench. Starts from the status, cargo xtask characters (D625). Use when asked to make, generate, rig or fix a character or humanoid model.
---

# Making a character

**Start every job with `cargo xtask characters`** (D625, a second): each criterion against its bar, how stale each check's record is and its refresh command, the tasks in order with the files (and lines) that own them, and the next thing to run. Read that before the log, handoffs, decisions or code, and only the parts it points to. Refresh a stale record with the command it names; a job that finishes a task takes its row out of `tools/characters/criteria.json`, one that finds work adds a row. Measure a job's cost with `node tools/token-stats/job.mjs --job '<its name>'`.

From a design to a model that plays every animation in the game, with Tripo doing the hard parts and scripts doing the rest. Worked out on 2026-10-11 ([research](../../../research/ai-models-for-art.md#characters-made-and-rigged), D115). The male base body was made this way.

**The base bodies are moving to Quaternius's** (D599): their Regular man and woman and their fantasy outfits, CC0, in `art/characters/quaternius/`, repainted in our materials, by the jobs in [the research note](../../../research/2026-11-02-character-base.md#the-switch-in-jobs). Make no new base body or base garment with Tripo; until the switch lands, the steps below keep today's base and its gear building. Tripo still makes held things and packs' bags. Never change their parts (D599): no push, stretch, scaling, retopology or cutting of their meshes, weights or bones, and their files stay as shipped; paint and new pieces of ours only, and a new piece that clips is fixed, never theirs. `node tools/characters/intact.ts OUTPUT.glb` proves it on every output built from their files; a Blender export keeps their weights only with `export_all_influences` (the woman's Ranger body and legs carry eight). Their man through our pipeline: `tools/characters/finish_character.sh quaternius-male 1.80 art/characters/quaternius/universal-base-characters/Regular_Male_FullBody.gltf` (no conversion, no twist bones, our clips retargeted onto his skeleton, sockets, `intact.ts` on the export). Dressed: give `HEAD.gltf+PIECE.gltf+...` (their `OnlyHead` and outfit pieces) in place of the body; `intact.ts` names every piece.

**Adding one of their outfits** (D611; char-pipeline j3; the costs in research/2026-11-05-agent-pipeline-tokens.md):
- `node tools/characters/vendor.ts --list Male_` names what their CC0 packs ship (`*` vendored); `node tools/characters/vendor.ts Male_<Outfit>_ --colourways` copies its pieces byte for byte with their `.bin`, textures and colourway maps and adds the `SHA256SUMS` lines; name the outfit in `art/characters/quaternius/README.md`'s list (the pack's licence rows are there already).
- A row a piece in `wardrobe.json` (copy a neighbour's one-line row; the schema is the file's `about`, read no source), headwear in `optional`; a look per outfit.
- `tools/characters/build_bodies.sh --step wardrobe:male` (3 min; ends on the known Sprint_Loop skate; then `git checkout -- assets/models/character/fight.glb`), `node tools/characters/pairings.ts --new` (only the new pieces' pairs), `node tools/characters/wardrobe.ts`; read only `✗` lines. `cargo xtask characters` then shows the build and the check recorded.
- Their parts are never reshaped: `wardrobe.ts`'s vendored, private and reshape stages fail on a changed byte or a reshaping script given their files (D599); a pairing that pokes is refused by the matrix, never fixed on theirs.

Everything runs from the repository root under WSL, with the Windows Blender (`tools/blender/run.sh`, which finds it) for the steps that must run inside Blender, and the Tripo CLI (`tools/tripo/tripo.ts`, the `tripo` skill) for everything Tripo does, logged in with `TRIPO_STUDIO_COOKIE` in `.env`. Check it first with `node tools/tripo/tripo.ts balance`.

Costs per character, on the Studio subscription: design sheets are free (100 images a month); the model and the rig cost what [tools/tripo/README.md](../../../tools/tripo/README.md) lists.

## The steps, in references/ (read only the one the task needs)

- [tripo-pipeline.md](references/tripo-pipeline.md): today's base by Tripo: 1. the design sheet a person approves, 2. model, rig, convert, check, 3. when the checker fails, clips and twist (when a limb twists or squeezes), 4. into the game.
- [gear.md](references/gear.md): 5. gear, what a body wears and holds.
- [body-templates.md](references/body-templates.md): 6. body templates, morphs of the base.
- [rebuilding.md](references/rebuilding.md): 7. rebuilding everything and into the game, gear on every body.

## Traps already found

- Blender's glTF importer leaves the skeleton posed in the first frame of some animation. Clear every pose bone's transform before using its rest pose, or that pose becomes the rest pose.
- The importer turns the skeleton to make Z up, so its own axes are not the world's. Compute directions in the skeleton's space and check them by measuring, never by assuming a sign.
- Scale the skeleton as an object; never apply the scale. Applying it changes the units of the animations' hip movements and the hips sink.
- Blender's automatic weights fail on generated meshes unless they are given a watertight copy (a voxel remesh), and the weights copied back.
- Copying the mannequin's weights onto a generated body did twice as badly as Tripo's rig. Don't.
- Tripo models face +X; ours face -Y. Every script turns them.
- glTF splits a vertex wherever the texture has a seam. Weld the copies before touching weights (`convert_rig.py` does), or each copy gets different weights and the seams open into light lines as the body moves. The checker's `seams` check catches it.
- Bevy needs its `jpeg` feature for Tripo's textures; without it the model fails to load.
- Tripo's own (V1) rig and its `actorcore` skeleton have twist and share bones with no skin on them; the `unreal` skeleton's carry skin. Check with a weight count before trusting a rig's bones.
- A third-party clip library may be a stale copy: our first was a 2025 mirror of Quaternius's, which missed his fix for the sword swing's elbow twist. Take it from the author, at the current version, and read the changelog.
- glTF does not keep Blender's constraints, and the game plays clips: the twist step is baked into every clip by `export_character.py`, and any script that plays the mannequin's clips on a body bakes them too (`twist.bake`). A body file's clips are baked already.
- Two files loaded in one Blender scene each bring their own copy of a clip, the second named `Name.001`: give each body its own (`compare.py` does), or one plays the other's.
- A roll past half a turn is ambiguous in any one frame: unwrap it over the whole clip (`twist.unwrapped`), never from the first frame.
- A loop whose arm circles (a crawl stroke) gains a whole turn of roll each time round (Codman's paradox), which no twist bone can follow and close the loop: `twist.unwrapped` takes such a clip frame by frame, as engines' twist correctives do, so the twist bone turns back once a loop where the roll passes half a turn (`test_twist.py`).
- The edge check cannot see a pinch: a ring of skin shrinking changes no edge's length much. The `pinched` measure watches the rings at elbows, knees and wrists.
- Scripts that `cd` lose relative paths; resolve them first (`realpath`).
- The body object carries its skeleton's scale (1.117 on the base): a piece worked out in the world goes through the object's inverse matrix before it becomes the mesh's vertices, or it comes out that much bigger (`hung_garment.py` built every skirt and apron 9% too big, D593).
- Current Blender keeps vertex group names on the mesh: a piece built on a new mesh loses them, and the exporter writes it without a skin. Build on a copy of the body's mesh.
- A body file splits vertices at texture seams and patch borders; weld before working on its surface as one, or every patch looks like a separate speck.
- A sheet drawn at 1,024 pixels and padded to 2,048 without scaling leaves a small figure in a pale box, and Tripo models the box. Both `make_character.sh` and `make_morph.sh` scale the front view and pad it with the sheet's own background.
- A generated model can carry stray pieces (a slab of backdrop); `morph.py` keeps only its largest connected part, after joining points split at texture seams.
- The clipping check once counted skin that had passed into a closed piece (a heavy back inside a pack's bag), which no one can see. It now counts only skin with nothing of the piece covering it from outside. Packs also hide the patches wholly under them, as garments do.
- In zsh, `r pose $c` passes "Idle_Loop 0.3" as one word; use `${=c}`.
