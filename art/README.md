# Source art

The inputs the asset pipeline makes the game's files from (D133): approved design sheets, and the models Tripo made from them. What the game loads is built from these, not kept here. Everything here cost credits or a developer's approval to make, so it is committed; anything that can be remade for free from it is not.

| Folder | What it is | Made into |
|---|---|---|
| `characters/<body>/` | `sheet.png`, the approved T-pose design sheet; `front.png`, its front view as sent to Tripo; `model.glb`, Tripo's textured model; `rigged.glb`, Tripo's auto-rig of it | The base body, `assets/models/character/base.glb`, from `base-male`; another body, such as `base-female`, becomes a morph of it. By the character pipeline (`cargo xtask bodies`, the `character` skill) |
| `characters/base-male/morphs/<morph>/` | A morph of the base: a design of the base person in another build (`sheet.png` with its prompt, `front.png`, `model.glb`), and for a template with its own skin, Tripo's painting of it on the base's mesh (`skin.glb`) | The morph, by `tools/characters/make_morph.sh`; the skin by `paint_skin.sh` |
| `characters/rig-baseline.json` | The rig checker's scores for the hand-rigged mannequin, the bar every body is checked against | — |
| `gear/dressed-<body>-<item>/` | A base body wearing the item, drawn and modelled by Tripo, from which the garment is cut as a shell | `assets/models/gear/<item>.glb`, one for every body |
| `gear/<item>/` | A worn thing modelled on its own from a concept, fitted to the base body | `assets/models/gear/<item>.glb` |
| `props/<prop>/` | `prop.json`, what the prop is and how it is made ours; `concept.png`, its concept; `model.glb`, Tripo's model of it | `assets/models/props/<file>.glb`, by `tools/props` (D147) |
| `animation/` | `clips.txt`, the clips the game plays. `library/` holds Quaternius's Universal Animation Library 1 (Pro, `UAL1.glb`) and 2 (Source, `UAL2.glb`) as bought from [quaternius.itch.io](https://quaternius.itch.io/universal-animation-library); it is not committed, as they are paid editions: each developer puts their own copy there | `assets/models/character/mannequin.glb`, by `tools/characters/clips.py` |
| `style/` | Style references given to the image model (`--ref`). `mood-medieval-town.png` is the earlier painted mood board (direction C), kept as research; the style is now D ([docs/art-style.md](../docs/art-style.md)), whose approved reference image goes at `style/reference.png` once there is one | — |

A `*.prompt.txt` beside an image is the prompt that made it. Tripo's own records of each task are not kept: they name the account.

Generated on the team's Tripo Studio subscription. The pipeline is in `tools/characters/` and `tools/props/`.
