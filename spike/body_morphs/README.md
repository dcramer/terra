# Spike: body morphs on our own base

Question: can build and sex morphs be made from Tripo designs of the base person, registered onto our base body's mesh, and does gear made once on the base pass its clipping check on every morph at full weight? The plan it tests is [docs/character-system-plan.md](../../docs/character-system-plan.md); the findings are in [research/body-morphs.md](../../research/body-morphs.md).

| Script | Does |
|---|---|
| `morph.py BASE DESIGN NAME` | Registers the base onto a design's model (pose by the base's skeleton, shape by a distance-smoothed pull, hands kept, back to the T-pose); writes the morph and the base reshaped at full weight. A rigged design on our skeleton gives its own joints. |
| `gear_morph.py BASE MORPH LOOK OUT` | Carries a look made on the base onto a morph: skin-following parts per vertex, parts that stand off the body as one piece |
| `skin.py out\|in NAME` | A template's own skin: the morphed body bare for Tripo to paint, and the painting baked back onto the base's layout |
| `compare.py` | Bodies side by side, `a.glb+b.glb` to dress one |

Scratch output in `out/` (ignored); the designs that cost credits are kept in `art/characters/base-male/morphs/`.
