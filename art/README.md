# Source art

The inputs the asset pipeline makes the game's files from (D133): approved design sheets, and the models Tripo made from them. What the game loads is built from these, not kept here. Everything here cost credits or a developer's approval to make, so it is committed; anything that can be remade for free from it is not.

| Folder | What it is | Made into |
|---|---|---|
| `characters/<body>/` | `sheet.png`, the approved T-pose design sheet; `front.png`, its front view as sent to Tripo; `model.glb`, Tripo's textured model; `rigged.glb`, Tripo's auto-rig of it | The game's body, `models/character/<body>.glb`, by the character pipeline (the `character` skill) |
| `characters/base-male/morphs/<morph>/` | A morph of the base: a design of the base person in another build (`sheet.png` with its prompt, `front.png`, `model.glb`), and for a template with its own skin, Tripo's painting of it on the base's mesh (`skin.glb`) | The morph, by `tools/characters/make_morph.sh`; the skin by `paint_skin.sh` |
| `characters/rig-baseline.json` | The rig checker's scores for the hand-rigged mannequin, the bar every body is checked against | — |
| `gear/dressed-<body>-<item>/` | A base body wearing the item, drawn and modelled by Tripo, from which the garment is cut as a shell | `models/gear/<body>/<item>.glb` |
| `gear/<item>/` | A worn thing modelled on its own from a concept, fitted to each body | `models/gear/<body>/<item>.glb` |
| `props/<prop>/` | `prop.json`, what the prop is and how it is made ours; `concept.png`, its concept; `model.glb`, Tripo's model of it | `models/props/<file>.glb`, by `tools/props` (D147) |
| `style/` | The mood board image every sheet is drawn in the style of (`--ref`) | — |

A `*.prompt.txt` beside an image is the prompt that made it. Tripo's own records of each task are not kept: they name the account.

Generated on the team's Tripo Studio subscription. Copied on 2026-10-12 from the scratch folder the pipeline wrote to at the time; the pipeline still writes there until it moves to `tools/characters/`.
