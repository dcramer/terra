# How modern games vary bodies and fit gear to all of them

2026-10-12. Question: in games from about 2018 on, with deep character creators or large varied populations, how are body variety and worn gear combined so that any outfit works on any body, and what is authored per body versus once?

Read, not tried. Grades:
- **[P]** primary: a developer talk or documentation, a patent, or the shipped files as read by open-source modding tools.
- **[S]** secondary: press, wikis or players.
- **[I]** our inference.

Most GDC recordings are paywalled, so for several talks only the abstract was read. This note follows [one-mesh-for-every-body.md](one-mesh-for-every-body.md), which surveyed engines and fitting methods; this one is about what shipped games do.

## Answer

Modern games agree on a small, parametric body and gear that rides it:

1. **One skeleton per family of bodies, and shape driven mostly by the bones.** Height, limb length, shoulders, build and weight are bone scales or helper bones moved by parameters, so everything skinned to the skeleton follows with no work per item. Examples: Bannerlord, Red Dead Redemption 2's body "expressions", Final Fantasy XIV's per-race bone deformers, ARK's body regions, Funcom's sliders, Unity's UMA.
2. **A few body morphs for what bones cannot do** (soft volume: belly, chest, overall heaviness), **copied onto every garment automatically.** Starfield ships three on every outfit (Heavy, Muscular, Thin, the corners of its body triangle). The same idea appears in Sims 4 deformation maps, Mutable's reshape and BodySlide's conform.
3. **The body hides what is covered.** Every title does it, and we already do.
4. **Variety comes cheaply from texture and data, not geometry:**
   - tint masks and palettes;
   - dirt, wetness, wear and blood as per-character shader parameters;
   - curated looks per archetype, picked from at random.
   
   This is how Red Dead Redemption 2's world reads as lived in.
5. **Few discrete body types where gear is made per type** (Baldur's Gate 3, Cyberpunk, GTA, Watch Dogs: Legion, Dragon's Dogma 2). This works only while types stay very few, and it is what keeps their body variety low.

Bone-driven shape holds up only within modest ranges, roughly 15 to 40 percent either way, before clipping returns; past that, games add morphs or separate bodies. No shipped game was found that gives wide, arbitrary body variety with arbitrary clothing without per-garment morph work, done by a tool.

Templates and sliders are then points in a small space of parameters, not separate meshes. A garment is one mesh shared by every wearer; only per-character weights differ.

## Games

| Game | Bodies | Gear on them | Grade |
|---|---|---|---|
| **Final Fantasy XIV** | About 16 race and sex pairs from a few base models | `human.pbd` holds one 3×4 matrix per bone per race. Gear made for one race is carried to another by composing those matrices along a fallback tree, inside ordinary skinning, at no runtime cost. A per-item, per-race flag (EQDP) says when an item has its own model instead. Height and bust are bone scales. | P (Penumbra, TexTools parsers) |
| **Starfield** | Body type 1 or 2, plus a triangle of Heavy, Muscular and Thin | Every outfit has a `morph.dat` with those three morphs: named, sparse per-vertex deltas, up to 128 keys. The head has Hat, Mask and HideEar morphs, so the head gives way under headgear rather than each helmet fitting each head. | P (PyNifly, StarfieldMeshConverter) |
| **Red Dead Redemption 2** | One skeleton per type (male, female, teen, animal). Body presets and a 21-step weight ladder are "outfits" of body components. Continuous body sliders are "MetaPedExpressions" that "morph components"; how is not documented, likely helper bones. | Clothes made once per type follow the expressions. Conflicts players see often (cuffs and gloves, trousers and boots, shirt under vest or coat) have authored alternate "wearable states" for each item. Tints are three-channel masks indexing palettes. Dirt, wetness, wear and damage are per-ped shader parameters. Ambient NPCs are about 25 curated looks per archetype, chosen from. | P (RedM native and data documentation) |
| **Mount & Blade II: Bannerlord** | Weight and build are `bone_scale` lists per bone and axis (`skins.xml`) | Armour on the same skeleton scales with them automatically. `covers_body` and `body_mesh_type` swap in a partial body under armour. Extreme values clip; mods narrow the ranges. | P (shipped XML) |
| **Dragon's Dogma 2** | Sliders with dependencies and limits between them | Capcom rebuilt Maya's wrap deformer in RE Engine. Beards, brows and helmets are wrapped to the head at creation, then frozen and skinned, so nothing runs at play time. "A universal mesh was initially the goal", but men, women and beastren needed separate meshes. About 300 equipment variations. | P (Capcom on the Autodesk blog) |
| **Cyberpunk 2077** | Separate bodies and rigs; NPC deformation rigs | Garments are made per rig. Layers are handled by garment support: each garment carries a shrunk shape, two vertex attributes and a layer score, and outer layers squash inner ones toward that shape. Player only; NPC looks are pre-fitted. | P/S (REDmodding) |
| **Kingdom Come: Deliverance 1 and 2** | Not documented | An adaptive clothing system "procedurally adapts multiple layers of clothing so they do not overlap", at handmade quality, across 16 slots in about 4 layers. KCD2 adds hiding groups, material atlasing, UV-space decals, and blood, grime and damage. | P (GDC abstracts, 2015 and 2025) |
| **Baldur's Gate 3** | 4 body types × races, no sliders | A mesh per race, sex and body type for every item. Modders "refit" between them. | S |
| **Watch Dogs: Legion** | About two body types, mostly thin or average | Variety came from thousands of photogrammetry-blended heads, voices and data (Census), not from bodies. | P (abstract), S |
| **GTA Online** | One body per sex, no sliders; faces blended from parent heads | Twelve component slots; skin is itself a component, swapped to match a top's sleeves. Shop data forces or forbids pairings. | P |
| **The Sims 4** (2014, still current) | Body shape as deformation maps over the body's layout | Every garment samples them through a second layout. A vertex colour blends a tight map with a loose one for skirts. | P (patent, GDC 2015) |
| **inZOI, Dragon Age: The Veilguard, ARK, Palworld, Dune: Awakening** | Sliders | Not documented. inZOI's sliders are body morph targets, with height fixed for the animations; necklaces sink into large chests, so rigid parts do not follow. ARK's 22 body regions read as bone scales. | S/I |
| **MetaHuman outfits, Mutable** (engines) | Shapes of one topology | Mutable binds garments to the base body by proximity and moves them onto any morph. MetaHuman warps from the nearest of several authored sizes. Both bake when a character is made. | P |

## What each game authors once and what per body

- **Once:**
  - the skeleton;
  - the shape parameters (bone scales, a few morphs, head blends);
  - palettes;
  - dirt, wetness and wear shaders;
  - decals and overlays.
- **Per garment, by a tool:** the body's morphs copied onto it, its skin weights copied from the body, its layer data, and which body parts it hides.
- **Per garment, by hand, sparingly:** alternate states for the few conflicts players see constantly (Red Dead Redemption 2), and a model of its own for a body type where deforming fails (Final Fantasy XIV's per-race flag).
- **Per archetype:** curated looks to pick from (Red Dead Redemption 2).

## Failure modes reported

- **Extreme slider values clip.** Bannerlord's mods narrow the ranges. EA's own test criteria have creators push bodies to their extremes. Dragon's Dogma 2 sets limits between sliders.
- **Rigid parts bend when they follow soft morphs.** inZOI's necklaces sink into chests; MetaHuman's buckles and armpits fail. Fix: mark rigid parts to ride one bone.
- **Loose cloth clings to the legs.** The Sims 4 adds a loose map; MakeHuman adds a skirt helper.
- **One bad body shape breaks every item at once.** A 2024 Sims patch broke 18 maps.
- **Copying morphs onto generated gear must go by position**, not vertex order: a generated garment shares no vertices with the body. Mutable's reshape breaks when topologies differ (UE-372131).
- **Crowds are pooled:** Assassin's Creed Unity drew 10,000 people with 120 full-detail models swapped in near the player (GDC 2015).
- **Fixed body types make bodies homogeneous.** Watch Dogs: Legion was criticised for it; every body type costs every item a mesh.

## Limits

- No developer source found for Veilguard, inZOI, Monster Hunter Wilds, Black Desert, Palworld, Dune: Awakening, Hitman or the recent Assassin's Creed games.
- Whether Red Dead Redemption 2's expressions move helper bones, blend shapes or both is not documented; its shipped files would settle it.
- Sources: [Penumbra deformer hook](https://github.com/xivdev/Penumbra/blob/master/Penumbra/Interop/Hooks/PostProcessing/PreBoneDeformerReplacer.cs), [Penumbra.GameData PbdFile](https://github.com/Ottermandias/Penumbra.GameData), [TexTools framework](https://github.com/TexTools/xivModdingFramework), [PyNifly Starfield morphs](https://github.com/BadDogSkyrim/PyNifly), [Capcom RE Engine tools](https://blogs.autodesk.com/media-and-entertainment/2025/04/10/ideas-and-ingenuity-behind-capcoms-in-house-re-engine-tools-for-dragons-dogma-2/), [RDR3 discoveries](https://github.com/femga/rdr3_discoveries), [RDR3 native DB](https://github.com/alloc8or/rdr3-nativedb-data), [Cyberpunk garment support](https://wiki.redmodding.org/cyberpunk-2077-modding/for-mod-creators-theory/3d-modelling/garment-support-how-does-it-work), [KCD adaptive clothing](https://gdcvault.com/play/1022822/Adaptive-Clothing-System-in-Kingdom), [KCD2 Dressed to Quest](https://gdcvault.com/play/1035519/Dressed-to-Quest-Achieving-Dynamic), [Census](https://gdcvault.com/play/1027018/Census-The-Systemic-Backbone-Behind), [Sims patent](https://patents.google.com/patent/US9626808), [EA creator test criteria](https://media.contentapi.ea.com/content/dam/eacom/SIMS/common/create-a-sim-technical-test-criteria.pdf), [Mutable Mesh Reshape](https://github-wiki-see.page/m/anticto/Mutable-Documentation/wiki/Node-Mesh-Reshape-(v5.5)), [MetaHuman parametric clothing](https://dev.epicgames.com/documentation/metahuman/getting-started-for-creating-parametric-clothing-in-metahuman), [Funcom armour primer](https://conanexiles.com/wp-content/wiki/2693791979.html), [BG3 outfit refits](https://bg3.wiki/wiki/Modding:Use_Outfit_Builder_To_Refit_Outfits).
