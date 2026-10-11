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
| `crates/terra_sandbox/assets/models/props/` | House Interior, Modular Sci-Fi and Survival packs: furniture, crates, tanks, a fire | None to the files. Recoloured the same way. |
| `crates/terra_sandbox/assets/models/character/` | Universal Animation Library, standard edition: a rigged figure and its animations | Cut down to eight animations, and the figure's two colours changed. |

The individual model files were fetched from [quaternius-showcase](https://github.com/trebeljahr/quaternius-showcase), which republishes the packs one model to a file, and the animation library from [gltf-universal-animation-library](https://github.com/J-Ponzo/gltf-universal-animation-library). The licence was checked against Quaternius's own [FAQ](https://quaternius.com/faq.html) on 2026-10-10.
