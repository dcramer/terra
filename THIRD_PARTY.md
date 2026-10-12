# Third-party code and assets

What in this repository comes from other projects, and the terms it comes under.

## Code

| Where | From | Licence |
|---|---|---|

`terra_voxel` will be a port of parts of Voxel Tools. Ported files carry a comment naming the original file, and this table gets a row for the crate.

Dependencies fetched by Cargo are not listed here; their licences are in their own packages.

## Models and animation

All by [Quaternius](https://quaternius.com), who releases everything under [CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/): free for any use, no attribution required. We credit them anyway.

| Where | From | Changes |
|---|---|---|
| `assets/models/nature/` | Ultimate Nature Pack: trees, rocks, bushes, plants, flowers | None to the files. The sandbox reads their shapes and applies its own colours. |
| `assets/models/props/` | Survival pack: a fire and a log | None to the files. Recoloured the same way. |
| `assets/models/character/` | Universal Animation Library, standard edition: a rigged figure and its animations | Cut down to 26 of its 46 animations, and the figure's two colours changed, by a script since retired (D121), which copied the kept nodes, skin, mesh and keyframes unchanged. |
| `assets/models/creatures/` | Ultimate Animated Animals pack: `Deer.glb`. Easy Enemies pack: `Spider.glb`. Both rigged and animated | None to the files. |
| `assets/models/village/` | Medieval Village pack: a well and a market stall | None to the files. Its houses and blacksmith's, and a palisade from the Fantasy RTS pack, were removed: buildings are built from parts (D108). |
| `assets/models/tools/` | Survival pack: `Axe.glb`, `Knife.glb`. Medieval Weapons pack: hammer, spear, bow, arrow | None to the files. Made into the held looks in `models/held/` by `tools/characters/fit_held.py`: scaled, set at their grip, and stone and iron heads coloured. |

The individual model files were fetched from [quaternius-showcase](https://github.com/trebeljahr/quaternius-showcase), which republishes the packs one model to a file, and the animation library from [gltf-universal-animation-library](https://github.com/J-Ponzo/gltf-universal-animation-library). The licence was checked against Quaternius's own [FAQ](https://quaternius.com/faq.html) on 2026-10-10. The creatures, village pieces, tools and workbench were fetched from quaternius-showcase on 2026-10-11.

## Made with AI

Made by us with generators on paid plans, whose terms leave what they make with the subscriber. We say so openly (D107).

| Where | Made with | How |
|---|---|---|
| `assets/models/character/base.glb` | [Tripo](https://www.tripo3d.ai) (the base male's model and rig, on our Studio subscription; design sheet on its API), animated with the Quaternius clips above. Its morphs from Tripo models of designs of the same man (heavy, thin, strong; image model and Studio) and from the female body (Tripo design, model and rig) | `.claude/skills/character/SKILL.md` and `tools/characters/build_bodies.sh`, from the source art in `art/characters/`; D112, D115 |
| `assets/models/character/skins/` | Tripo (texture, on our Studio subscription): a template's skin painted on the base's mesh from its design | `tools/characters/paint_skin.sh` |
| `assets/models/gear/` | Tripo (concept images and models, on our Studio subscription): a pack in two sizes, a cap, a coat and a quiver | Made once on the base body and given its morphs, by `tools/characters/build_bodies.sh` |
| `assets/models/props/Forge.glb`, `Smelter.glb`, `Workbench.glb`, `ClaimStone.glb`, `Sack.glb`, and the furniture (`Chair.glb`, `Stool.glb`, `RoundTable.glb`, `NightStand.glb`, `Bed.glb`, `Bookshelf.glb`, `RugRound.glb`, `RugLong.glb`, `Spade.glb`); `assets/models/tools/Pickaxe.glb`, `Torch.glb` | Tripo (concept images and models, on our Studio subscription) | `tools/props`, from the source art in `art/props/`: cleaned up and given our palette in Blender; D147 |

## Sound

By [Kenney](https://kenney.nl), under [CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/): free for any use, no attribution required. We credit them anyway.

| Where | From | Changes |
|---|---|---|
| `assets/sounds/` | [Impact Sounds](https://kenney.nl/assets/impact-sounds) and [RPG Audio](https://kenney.nl/assets/rpg-audio): footsteps, impacts, chopping, creaks, clicks | None to the files; 88 of the packs' 180 sounds. Fetched from kenney.nl on 2026-10-11, licence checked in each pack's `License.txt`. |

The rest of the game's sound, air, water, fire, birds and music, is made in code (`crates/terra_game/src/audio/`).
