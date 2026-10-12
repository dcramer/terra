# Third-party code and assets

What in this repository comes from other projects, and the terms it comes under.

## Code

| Where | From | Licence |
|---|---|---|
| `demo/ground.gdshader`, the seam-stitching vertex function | [Voxel Tools for Godot](https://github.com/Zylann/godot_voxel) | MIT, copyright Marc Gilleron |
| `spike/demo.patch` | A patch against the [Voxel Tools solar-system demo](https://github.com/Zylann/solar_system_demo) | MIT, copyright Marc Gilleron |

`terra_voxel` will be a port of parts of Voxel Tools. Ported files carry a comment naming the original file, and this table gets a row for the crate.

Dependencies fetched by Cargo are not listed here; their licences are in their own packages.

## Models and animation

All by [Quaternius](https://quaternius.com), who releases everything under [CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/): free for any use, no attribution required. We credit them anyway.

| Where | From | Changes |
|---|---|---|
| `crates/terra_sandbox/assets/models/nature/` | Ultimate Nature Pack: trees, rocks, bushes, plants, flowers | None to the files. The sandbox reads their shapes and applies its own colours. |
| `crates/terra_sandbox/assets/models/props/` | House Interior, Modular Sci-Fi and Survival packs: furniture, crates, tanks, a fire. Modular Dungeon pack: `Table_Small.glb`, a workbench | None to the files. Recoloured the same way. |
| `crates/terra_sandbox/assets/models/character/` | Universal Animation Library, standard edition: a rigged figure and its animations | Cut down to 26 of its 46 animations, and the figure's two colours changed, by a script since retired (D121), which copied the kept nodes, skin, mesh and keyframes unchanged. |
| `crates/terra_sandbox/assets/models/creatures/` | Ultimate Animated Animals pack: `Deer.glb`. Easy Enemies pack: `Spider.glb`. Both rigged and animated | None to the files. |
| `crates/terra_sandbox/assets/models/village/` | Medieval Village pack: a fence, a well, a market stall | None to the files. Its houses and blacksmith's, and a palisade from the Fantasy RTS pack, were removed: buildings are built from parts (D108). |
| `crates/terra_sandbox/assets/models/tools/` | Survival pack: `Axe.glb`, `Knife.glb`. Medieval Weapons pack: hammer, sword, spear, bow, arrow | None to the files. |

The individual model files were fetched from [quaternius-showcase](https://github.com/trebeljahr/quaternius-showcase), which republishes the packs one model to a file, and the animation library from [gltf-universal-animation-library](https://github.com/J-Ponzo/gltf-universal-animation-library). The licence was checked against Quaternius's own [FAQ](https://quaternius.com/faq.html) on 2026-10-10. The creatures, village pieces, tools and workbench were fetched from quaternius-showcase on 2026-10-11.

## Made with AI

Made by us with generators on paid plans, whose terms leave what they make with the subscriber. We say so openly (D107).

| Where | Made with | How |
|---|---|---|
| `crates/terra_sandbox/assets/models/character/base-male.glb` | [Tripo](https://www.tripo3d.ai) (model and rig, on our Studio subscription; design sheet on its API), animated with the Quaternius clips above | `.claude/skills/character/SKILL.md` and `spike/character_gear/`; D115 |
| `crates/terra_sandbox/assets/models/character/base-female.glb` | Tripo (design sheet, model and rig, on our Studio subscription), animated with the Quaternius clips above | The same |
| `crates/terra_sandbox/assets/models/gear/` | Tripo (concept images and models, on our Studio subscription): a pack in two sizes, a cap, a coat and a quiver | Fitted to each base body, one folder each, by `spike/character_gear/fit_gear.py` |

## Sound

By [Kenney](https://kenney.nl), under [CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/): free for any use, no attribution required. We credit them anyway.

| Where | From | Changes |
|---|---|---|
| `crates/terra_sandbox/assets/sounds/` | [Impact Sounds](https://kenney.nl/assets/impact-sounds) and [RPG Audio](https://kenney.nl/assets/rpg-audio): footsteps, impacts, chopping, creaks, clicks | None to the files; 88 of the packs' 180 sounds. Fetched from kenney.nl on 2026-10-11, licence checked in each pack's `License.txt`. |

The rest of the game's sound, air, water, fire, birds and music, is made in code (`crates/terra_sandbox/src/audio/`).
