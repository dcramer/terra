# Spike: a raymarched campfire flame (D133)

One question: does a raymarched volumetric flame make the campfire read AAA, where its cards have stalled at 3-4/10?

Not the game's campfire, and nothing may depend on it. It is a shader made from main's `fire.wesl` and needs no Rust change:

- `volume.wesl`: a hearth's first flame column drawn as a camera-facing quad over the bounding sphere of a 1 x 1.5 x 1 m box over the logs, and its fragment marched front to back through that box, ended at the depth prepass, emissive-absorbing (the breath spike's march, 6db86f20). The gas: temperature rising off the coal bed in tongues, a three-octave value noise carried up with the gas and swept by a divergence-free field, coloured through the campfire's ramp row. The hearths' mask pass draws the same quad, so the flame goes through the film tonemap as the cards do.
- `assemble.ts <main fire.wesl> <out>`: main's shader with `volume.wesl` put in, the hearth's other flame cards hidden, and the quad's fragment marched. Logs, coals, ground card, light, smoke, sparks and torches stay main's.

The answer is in research/2026-10-19-raymarched-breath.md, "The campfire".
