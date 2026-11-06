# iso-art

The iso experiment's art pipeline (iso-art): a hero, creature, NPC or prop from a description to a checked GLB with its clips, through [Tripo](../tripo/README.md), one command a stage. Tripo's own rig and preset clips are played as they are, one mesh per character with its gear: no conversion onto our skeleton, which is what made main Terra's people hard ([character skill](../../.agents/skills/character/SKILL.md), its traps). TypeScript on Node 22.18+, no dependencies, no build step; `node --test tools/iso-art/iso-art.test.ts`.

```bash
iso=tools/iso-art/iso-art.ts
node $iso concept ranger --kind hero --theme "a dark-fantasy ruin at night" "a hooded ranger in worn leather and a mail shirt"
node $iso model ranger          # 55 credits (--standard 25); refuses if the balance is short
node $iso rig ranger            # 20; humanoid for hero and npc, as its shape reads for a creature (--body)
node $iso animate ranger        # free: the kind's presets
node $iso export ranger         # free: assets/models/iso/ranger.glb
node $iso check ranger          # exits 1 if a check fails
node $iso adopt ogre a661c0b1 --kind creature --height 2.8   # an existing project, at its step
node $iso status                # every asset's stages, credits and minutes
```

Each stage prints a line; Tripo's progress goes to `target/iso-art/NAME/<stage>.log`, with its drafts. Each records its credits (the Tripo CLI's own charge) and minutes in the ledger `art/iso/NAME/asset.json`, with the concept, its prompt and the description. Credits are read from tools/tripo/README.md's table, the one place they are written; a spending stage checks the balance against them first.

- **concept**: the description and theme, an iso part (the whole subject from the front; big shapes that read from above; gear part of the one figure; a T-pose for a biped, a square stance for another body), then docs/art-style.md's concept paragraph word for word. Free, 100 images a month. Look at `art/iso/NAME/concept.png` before `model`.
- **clips** by the rig's type, Tripo's V3 presets: a biped gets `idle`, `walk`, `run`, `attack` (preset `slash`), `hit` (`hurt`), `death` (`fall`, the nearest: there is no death preset). Every other body Tripo rigs (quadruped, hexapod, octopod, serpentine, aquatic) has one preset, a walk (`march` for the last two): no idle, attack or death, which would need `--prompt` motions, humanoid only, so not for them.
- **export** asks for the clips in place at the kind's texture size, names them ours, and adds a root node named for the asset that makes it its height (`--height`, life size, D110), turns it to face +Z by the way its walk goes, and stands it on y=0 with the body over the origin (the model contract's base anchor). The game moves the body at the speed `check` records.

## The checks

| Check | For |
|---|---|
| each clip by name, its length | the game plays it by name; a length far off is the wrong preset or a broken export |
| loops close (idle, walk, run) | the last frame meets the first, or the body jumps each time round |
| walk, run in place | the game moves the body; root motion would carry it twice |
| walk, run speed (recorded) | the planted foot's speed against the body, so the game moves it without the feet sliding; run over 1.3x walk |
| facing | the walk goes toward +Z, glTF's front, within 25 degrees |
| death ends down | the body ends under 60% of its start height; the nearest preset may not |
| triangles, textures | the kind's budget (`kinds.ts`): a dozen on screen from 15 to 20 m up |
| height, on the ground | life size within 5%; the lowest point of the idle's first frame on y=0 |

Read from the GLB in Node (`gltf.ts`: accessors, the node hierarchy posed by a clip, the skin's points), so a check needs no GPU. Looking at it in the iso spike is that spike's own command: this tool depends on no spike (D133).
