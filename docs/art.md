# Art direction

Status: Open (D14). Art style is a top priority. No Man's Sky is the benchmark. The beauty corner in [prototypes.md](prototypes.md) builds two treatments and we pick from screenshots.

## Constraints that shape the style

- Full 3D.
- No artists: every asset is free, bought or LLM-generated.
- Procedural wherever possible.
- Smooth voxel terrain has no UV mapping and is naturally rounded.
- Generated 3D meshes are acceptable for static props and weak for clean topology, rigging and animation.
- Assets from mixed sources will not match each other.

## Benchmark: No Man's Sky

It has the best art of our references and it is procedural throughout. What makes it work:

- **A bold, limited palette per planet.** Sky, fog, ground and foliage colours are generated together so they harmonise.
- **Heavy atmosphere.** Fog and scattering do much of the work and hide distant level-of-detail changes.
- **Dense scattered flora and strong silhouettes.**
- **Variation from rules, not from unique assets**: kits of parts recombined, recoloured and placed procedurally.

One caution: its kits of parts were made by artists. Ours come from bought or generated packs, so the palette and shader have to do more of the unifying.

## Principles that hold for either treatment

- **The art is the simulation rendered.** Sky colour from pressure, terrain tint from moisture and biomass, plant density from cell values. The sky going from black to dusty to blue is the terraforming feedback.
- **Palette per biome and phase**, generated from simulation values. This is our version of palette per planet.
- **One material pipeline for everything.** Every asset, whatever its source, goes through the same palette and shader so the game reads as one thing.
- **Procedural plants** grown from parameters, with growth stages, so each biome gets its own flora without per-asset work.
- **Effort goes into light and atmosphere**, which is shader code and therefore procedural and LLM-writable.

## The two treatments to try

| | A: painterly and detailed | B: simple shapes, rich light |
|---|---|---|
| Reference | No Man's Sky | Valheim |
| Geometry and surfaces | Detailed meshes, textured terrain | Simple, flat-coloured |
| Strength | The look we most admire | Forgiving of generated and mismatched assets |
| Risk | Exposes weak assets; more per-asset work | May look generic |

Astroneer is not a target (D15).

## How we decide

A beauty corner: one fixed scene, rendered at three phases (dead, thawing, alive), in both treatments. Pick from screenshots. No content is built before one passes.

## Open questions

- Player character: visible suit in third person, or first person only? First person removes a large animation cost.
- Look of machines and structures.
- UI style.
