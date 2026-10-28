# Assembled parts as real art: can we make D565's parts beautiful and join them without it looking bad?

2026-10-28, characters and items. Question: D565 ([docs/assembled-items.md](../docs/assembled-items.md), [the prior-art note](2026-10-28-assembled-equipment.md)), as corrected by the developer: every part (head, blade, guard, handle) is textured art to [docs/art-style.md](../docs/art-style.md), made by Tripo or modelled; only the assembly is procedural; each part is prepared once into its template with its points marked. The developer: "I still want the game to look beautiful ... I'm worried you're gonna make the game look dumb ... research every part of the design you're suggesting to make sure we can actually pull this off". Feasibility only: nothing built, no Tripo credits spent.

*Read* marks a claim checked in a source this session, *recalled* one from memory, *measured* one measured on our files with the read-only scripts named.

## Answer

| # | Question | Verdict | Confidence |
|---|---|---|---|
| 1 | Isolated parts from Tripo, in our style | **Works with** a style reference image and a kind's parts drawn together in one concept sheet; consistency across separate runs is unproven by us or anyone we found | Medium on quality and budgets, low on consistency until the proof |
| 2 | Style coherence across parts | **Works with** a prep step that grades each part to its role (the `grade` tool we have) and one texel density; role tint alone does not carry it | Medium-high |
| 3 | Hiding the joins | **Works**: every join is covered by the head's own eye or a trim part (collar, wrap, ferrule, guard, pommel), as real tools are made; the engine needs nothing new | High |
| 4 | Finding each part's points | **Handles work** automatically; **a head's eye does not** from its shape alone: take it from the generation (the handle's line inside the whole model) or a modelled hole, else a person marks it (minutes a part) | High on handles, high that eyes need another way |
| 5 | Drawing assembled items | **Works**: separate parts at run time, shared and batched across every item and player; never a merged mesh per item | High |
| 6 | Icons per assembled item | **Works with** a render per recipe, cached by its hash; painting each item is out of reach | Medium (the look must match the painted icons) |
| 7 | A good-looking proof | One one-handed axe kind: 3 heads, 3 handles, 2 trims, made two ways and compared; about 600 to 900 credits of 19,225 | Medium on the estimate |

The risk that decides it is 1 and 2 together: parts from separate generations that do not look like one tool. Everything else is known engineering. The proof (7) exists to answer that with pictures before anything is built on it.

## What we get today (measured)

`target/job/inspect.ts` and `texel.ts` (read-only, uncommitted), on every committed held look and its source:

- **Thirteen held looks:** five are Tripo-textured (arming sword, round and heater shields, torch, basket); seven are a free pack's flat colours (axe 236 triangles, knife 450, bow 728, spear 1,202, hammer 1,474; stone and iron variants share the axe and pick), and the pickaxe is Tripo's model painted to flat roles (599). art-style.md lists tools as still flat colour ("Where the pipeline stands"), so today's axes are not the bar; the textured five are.
- **Tripo's raw output:** 4,545 to 4,777 triangles and 2K maps (4K for the h3.1 torch), cut by `refine` to 1,000 to 1,200 triangles and 1K maps in the game, inside the guide's budgets (a tool's head 300 to 800, a prop 600 to 1,500). Bannerlord's sword parts are 120 to 1,302 triangles a blade and 96 to 928 a handle (developer Q&A, in a search summary, not read).
- **Texture use:** Tripo's own unwrap fills only 34 to 43% of the map. In the game that gives 16.5 pixels a centimetre on the sword, 19.6 on the torch, 5.8 to 8.7 on the shields and basket. A part alone gets its own map, so a head at 512 square (about 15 px/cm on an 18 by 12 cm head) and a handle at 512 to 1K matches the sword.
- **Concepts across runs:** the five tool concepts in `art/props` (sword, torch, round shield, pickaxe, spade), each its own run from the same style paragraph, side by side: four read as one style; the spade drifted, its wood saturated orange and its shapes chunky and cartoonish. There is no approved style reference yet (`art/style/reference.png` is absent), which is the strongest lever on this.
- **Tripo's handles run thick:** the pickaxe's came out 8.6 cm across at the grip against a real 4, which is why `haft.py` exists.

## 1. Isolated parts from Tripo

- **Part-only generation:** nothing in Tripo's docs or others' accounts tests prompting a head or handle alone (searched; none found). Tripo models what the image shows, and a lone axe head is an ordinary object, so the shape is not the doubt. The doubts are an eye modelled solid (no hole, see 4) and a concept that adds a stub of handle.
- **Tripo's own parts:** `model --parts simple|balanced|detailed` (our CLI, H models) generates a model "pre-segmented"; `segment` (40 credits) splits an existing one, v2 semantic, "a weapon versus the hand holding it" in its examples; part completion rebuilds the hidden geometry of a split part, from VAST's HoloPart (Tripo's and Scenario's pages in search summaries; Tripo's blog refused our fetch; the paper's abstract *read*). Scenario's guide: "models with distinct parts segment more cleanly than smooth, fused shapes" (*read*). Whether `--parts` gives each part its own texture is untested.
- **Consistency across runs:** no source measures it for any generator (searched; reviews are vendor-adjacent). Our own evidence is the 4-of-5 above. The usual lever is the same reference image in every run (Tripo's image step takes up to ten `--ref`).
- **So the proof makes parts two ways:** (A) whole tools, generated in parts or split, each tool coherent in itself, mixed across tools; (B) a kind's heads drawn together on one concept sheet (one image, one style), each cropped and modelled alone, the handles likewise. Both graded at prep (2).

## 2. Style coherence across parts

- **How games do it:** a shared material library and one texel density (Polycount: "texture density ... consistency is nearly always important" for base content, *read* in a search summary); Warframe's tint masks with tint channels that "match up" between a helmet and its body (Warframe's guidelines, *read* in a search summary), and DE: "there's really no way to 'standardize' the process" per design. Unique maps for props, tileables and trims for world geometry (same thread).
- **What we have:** `tools/props`'s `reference` (a concept drawn after another, so pieces share one material) and `grade` (a texture matched to another's mean and spread on its surface); `block` evens Tripo's baked light ("Tripo lights a model's back darker than its front", props README); `tools/kits` bakes a kit's pieces into one atlas (D251, D258). These are the prep steps a part needs.
- **Role tint** (`Tint`, `crates/terra_walker/src/character.rs`): over a texture it multiplies by the colour scaled to a full brightest channel (`dye`). It can turn a hue and darken; it cannot lighten, desaturate or even out contrast. Each part is naturally one role (a head Metal, a handle Wood, a wrap Leather), so tinting by part works, but only on a neutrally graded texture (grey iron, pale wood). It carries a material tier (iron, steel, bronze), not coherence between two parts drawn differently.
- **Verdict:** coherence comes from making (one reference, one sheet a kind) and prep (grade, one density, baked light evened), with tint for tiers on top.

## 3. Joins

- **Real tools hide their own joins:** an axe's handle passes through the eye and is wedged; a sword's guard covers the blade's shoulder and the pommel its tang; wraps, collars and ferrules sit where wood meets iron. Every join in the kinds we have is covered by the head itself or a trim, which D565 already allows (at most two trims a kind).
- **Where shipped systems fail:** Bannerlord players report gaps "between pommel/handle and handle/hilt", mostly the handles (Steam, 2020, *read* in a search summary); a mod author added invisible guard and pommel pieces where a separate one "would just mess up the model" (Nexus, same). Polycount: overlap is fine placed deliberately; coplanar surfaces z-fight (*read* in a search summary).
- **What it needs of us:** a join is a slot with one inner diameter, a handle's top is made to it (the `haft` step, applied to every handle), and the cover's inner surface sits inside the handle and its outer over the cut. The part checker's "no gap, nothing buried, no two surfaces in one plane" holds it. Seen close, the cover must be big enough to hide the handle's top: a collar 1 to 2 cm, as real ones are.
- **Engine:** overlapping meshes depth-sort correctly; nothing new. Bevy 0.20 has forward and clustered decals (`bevy_pbr-0.20.0/src/decal/`, *read*; clustered need bindless, fine on our native targets) for wear across a seam, if ever wanted.

## 4. Preparing parts: finding the points (measured)

`target/job/parts.ts` splits each held look (grip at the origin, handle along +Y, D546) into wood and metal by material, measures each part alone, and compares with the truth the whole look gives:

| Part | Axis off +Y | Line found within | Across (median) | Eye found within, from the head alone |
|---|---|---|---|---|
| Axe | 0.1° | 0.6 cm | 3.5 cm | 2.6 cm |
| Pickaxe | 0.5° | 0.8 cm | 4.0 cm | 3.2 cm |
| Hammer | 0.0° | 0.2 cm | 2.5 cm | 0.1 cm |
| Spear | 0.5° | 1.7 cm | 4.0 cm | 0.8 cm |
| Knife | 4.0° (10 cm handle) | 0.4 cm | 1.9 cm | 0.2 cm |

- **Handles:** the principal axis and sections give the axis, line and thickness reliably; ends flare (the pick's 10 cm under its head), so a slot reads its diameter at its own place, not the maximum. The existing detector (`tools/characters/detect.ts`) on the whole sources finds turns within 1° of the hand-set data on all six hafted and blade items it measures, the grip within 2.1 cm (the sword's points within 0.2 cm; the spear's rule takes mid-shaft: 68 against 75 cm), and on the fused Tripo sword the handle (5 cm across) and the guard where the head begins.
- **Heads:** with the eye's axis given (the head made to its template, eye up), the eye from the head's shape alone (its thickest, tallest place along it) is 2.6 and 3.2 cm out on the axe and pick, as far as the handle is thick: the handle would come out of the head's side. The axis itself is ambiguous by shape: the axe head's two largest extents are 26.2 and 23.8 cm, the pick's 17.7 and 14.2. The hammer's metal runs 43 cm down its handle as straps, so a split by material is not always a head.
- **So the eye comes from how the head is made:** from the handle's line in a whole generation (exact by construction, route A), or a modelled through-hole found as an inner loop (route B, if Tripo models one). A blade's tang is where the detector finds the guard, its edge the wide axis, its flat the thin one (the knife's blade alone: 15.9 by 4.4 by 1.3 cm, unambiguous).
- **The manual fallback:** a person or agent sets the point in the part's json, as three held looks already have hand-set points (`fit: points`: the sword and both shields), checked on a still: minutes a part, done once, and refused by the part checker if wrong.

## 5. Drawing assembled items

- **Separate parts at run time.** A held look is already its own glTF scene on the hand socket (`crates/terra_walker/src/character/held.rs`). Parts as separate meshes share one mesh and material each wherever they are used, so every player's copy of a head batches: Bevy 0.20's StandardMaterial is bindless (`#[bindless]`, `pbr_material.rs`), its mesh allocator packs meshes into shared buffers "so that multiple meshes can be drawn back-to-back without any rebinding" (`mesh/allocator.rs`), and GPU preprocessing draws them by multi-draw indirect (`batching/gpu_preprocessing.rs`; all *read*). Three or four parts an item, a thousand players with two items each, is about 8,000 instances in a handful of draws.
- **Not merged per item:** a merged mesh is unique per recipe, so nothing instances, and its textures would have to be baked per item, so memory grows with items rather than parts.
- **Texture memory scales with the parts vocabulary, never the items:** cooked (BC7 colour, BC5 normal, one byte a pixel, D188), a part at 512 with roughness-metal at 256 is about 0.75 MB with mips, at 1K about 3 MB. Seven kinds of about 30 parts each, 210 parts, is 160 to 630 MB if every one were resident at once; only the ones in view are needed.
- **LOD:** held items are small; Bevy's `VisibilityRange` drops trims first and swaps a far item to fewer triangles (`bevy_camera-0.20.0/src/visibility/range.rs`, *read*). No new engine work.

## 6. Icons

- Icons today are rendered from the look, then painted by the image model (D129, D130; `tools/icons/paint.ts`), 100 free images a month. Thousands of recipes cannot each be painted.
- **A render per recipe**, in the existing house rig (`crates/terra_survival/src/icons.rs`, which names no item), cached by the recipe's hash: one GPU render of seconds each, the way games with crafted items do it (render the item to a texture; Unreal forum, *read* in a search summary; Bannerlord's crafted-weapon thumbnails *recalled*, not confirmed). The catch: renders beside painted icons do not match. Since every icon is to be repainted for style D anyway (art-style.md), the choice of renders or paintings for style D is the developers' (D130).
- **Painted parts composited** (the render is orthographic, so a painted part's image lies exactly where the part does) is the alternative if paintings are kept; untested.

## 7. The proof

- **One kind**, the one-handed axe: 3 heads (broad, bearded, hatchet), 3 handles (straight ash, curved, short), 2 trims (leather wrap, iron collar), giving 9 pairings to a sheet, at the envelope's extremes as D565's order of work says.
- **Both routes:** A, three whole axes with `--parts` at h3.1 (about 55 each, 165), mixed across; B, a concept sheet a part type, each part modelled alone (about 50 to 55 each, 8 parts, 440). With one remake in three, about 600 to 900 credits. Concepts free (about 15 of the 100 images a month). Balance on 2026-10-28: 19,225. Over 100 credits, so it needs the developers' go-ahead.
- **Judged on** the game's own render: the nine held, carried and stowed, close and at play distance, beside the textured sword and torch, through the art-style checklist, and the developer's eye on whether they read as one tool.

## Limits

- Nothing was generated; part-only quality and cross-run consistency are judged from five concepts and no outside measurement.
- The eye test used flat-colour pack heads split by material, which have no modelled eye hole; a Tripo head made alone might have one.
- Several sources were read only through search summaries (marked); Tripo's own blog refused the fetch.
- Texture memory is arithmetic, not measured in the game; no bench was run.

## Sources

- Tripo: [Scenario, Tripo segmentation essentials](https://help.scenario.com/articles/9861107995-tripo-segmentation-the-essentials) (*read*); [Tripo, segmentation v2](https://www.tripo3d.ai/blog/tripo-segmentation-v2) and [splitting a model into parts](https://www.tripo3d.ai/blog/split-3d-model-into-parts) (search summaries); [HoloPart, arXiv 2504.07943](https://arxiv.org/abs/2504.07943); `tools/tripo/README.md`, `tools/tripo/commands/make.ts` (`--parts`).
- Consistency: [Building game-ready 3D assets fast, 2026](https://levelup.gitconnected.com/building-game-ready-3d-assets-fast-the-best-ai-generators-for-production-workflows-in-2026-f1f725ff5df0) (search summary; no consistency data).
- Coherence: [Polycount, trim sheets and texel density](https://polycount.com/discussion/comment/2592840); [Warframe, technical guidelines](https://www.warframe.com/steamworkshop/content-files-and-examples), [basic art guide](https://www.warframe.com/en/steamworkshop/basic-art-guide), [forum, tint order](https://forums.warframe.com/topic/630951-tennogen-skins-all-sticking-to-a-common-coloring-practice/).
- Joins: [Steam, Bannerlord pieces misaligned](https://steamcommunity.com/app/261550/discussions/0/2279330983544366958); [Nexus, invisible guard and pommel](https://www.nexusmods.com/mountandblade2bannerlord/mods/393/); [Bannerlord dev Q&A, part budgets](https://www.gamebanshee.com/k4a8q); [Polycount, overlap and z-fighting](https://polycount.com/discussion/comment/2773261).
- Icons: [Unreal forum, icons for customisable weapons](https://forums.unrealengine.com/t/custom-icon-for-customisable-weapon/1693727).
- Engine: Bevy 0.20's `bevy_pbr/src/pbr_material.rs`, `bevy_pbr/src/decal/`, `bevy_render/src/mesh/allocator.rs`, `bevy_render/src/batching/gpu_preprocessing.rs`.
- Ours: `assets/models/held`, `assets/models/tools`, `art/held`, `art/props/{arming_sword,pickaxe,shovel,torch,round_shield,heater_shield}`, `tools/characters/detect.ts`, `tools/props/README.md`, `crates/terra_walker/src/character.rs` (`Tint`, `dye`).
