# How games make customisable characters and gear that fits every body

2026-10-12. **Question:** at the level of mechanisms and data, how do shipped games and public tools let players shape a body and wear gear that fits every body, so we can rebuild the techniques on Bevy?

**Method:**
- Primary documents, source code and patents.
- Codes:
  - **[v]** verified in docs or source;
  - **[i]** inferred;
  - **[u]** unverified, from community sources.
- **Licences:**
  - UMA: MIT.
  - BodySlide/Outfit Studio and TexTools: GPL-3.0, read for ideas only, nothing copied.
  - Epic's weight-transfer code: MIT.
  - VRM: MIT.
  - Mutable: Epic's EULA; its docs are public.
  - Roblox: closed, read through its docs and patents.

## The techniques

| # | Technique | Solves | How it works | Shipped by |
|---|---|---|---|---|
| T1 | **Proportions in the bones** | Height, limb length, girth, head size; gear follows for free | Each body parameter sets bones' position, rotation and scale, added or multiplied and clamped, before skinning. Scale on leaf or helper bones, or parents shear their children | UMA `SkeletonModifier` [v]; Second Life's "collision volume" bones [v]; Final Fantasy XIV's per-race bone deformers [v structure, i runtime] |
| T2 | **Body morphs copied onto gear ("conform")** | Soft shape bones can't make: fat, muscle, bust, sex | For each garment vertex, the inverse-distance-weighted average of its 10 nearest body vertices' morph offsets, within a radius. Options: a mask, axis locks, "solid" parts moving as one, smoothing. The garment carries the same morphs, driven by the same weights | BodySlide/Outfit Studio [v]; UMA merges blendshapes by name [v]; Starfield's per-outfit `morph.dat` [v exists] |
| T3 | **Garments bound to a surface or cage** | One garment on any body, or on the garment beneath | Each garment vertex is bound to a face of the base body and placed on the same face of the target. Rigid clusters move as one; Laplacian smoothing after. Roblox fits a garment's inner cage to the outer cage beneath by radial basis functions, layer by layer, keeping its thickness | Mutable's Mesh Reshape [v]; Roblox Layered Clothing [v docs, patent US11615601B2]; MetaHuman's outfit sizes (editor only) [v] |
| T4 | **Skin weights copied from the body** | Rigging any garment with no hand work | Take the nearest surface point's weights where it is close (5% of the bounding diagonal) and the normals agree (30°). Solve the rest by Laplacian "inpainting", so a skirt between the legs gets smooth, not torn, weights | Epic's "Robust Skin Weights Transfer via Weight Inpainting" (SIGGRAPH Asia 2023, MIT code) and its Chaos Cloth node [v]; Roblox AutoSkin, nearest only, with documented failures [v] |
| T5 | **Layer order and tucking** | A shirt under a jacket, trousers into boots | Each item carries a "compressed" shape; a slot order decides which item is squashed under which | Cyberpunk's garment support [u]; Mutable's Clip Deform [v]; Roblox layer order [v] |
| T6 | **Hiding what gear covers** | Skin through tight gear | Per-triangle bit masks, unioned across what is worn (conservative: a triangle hidden if any vertex is). Or body regions chosen by variant, hair and ears hidden per helmet | UMA MeshHideAsset [v]; Mutable Remove Mesh and UV masks [v]; Roblox's hidden-surface removal, baked by casting rays from the cage [v patent]; WoW geosets [v]; Baldur's Gate 3 vertex-colour masks [u] |
| T7 | **Loose garments and hair** | Skirts and coats against thighs; hair through hats | Helper bones turned by the thighs; pose drivers; spring bones with capsule colliders (Verlet with stiffness, gravity and drag, kept to length, pushed out of colliders); hat-hair variants and hide flags | VRM's `VRMC_springBone` spec [v]; a skirt-helper patent [u]; WoW's helmet masks [v]; Baldur's Gate 3's physics paint [u] |
| T8 | **Skin colour and variety** | Many looks from one texture | A skin-tone mask and palette, or melanin and haemoglobin parameters, in the shader; tint and decal layers composited | Jimenez et al. 2010 skin appearance [v]; Skyrim's tint layers [i]; UMA overlays [v]; Mutable decals [v]; Red Dead Redemption 2 unpublished |
| T9 | **Many characters** | Draw and animation cost | Each character merged into one skinned mesh with an atlas (UMA's combiner, Mutable's block packing); LODs per part; animation at a lower rate by importance (Unreal's Animation Budget Allocator, 1 ms by default); pooled crowds | UMA [v]; Mutable [v]; Unreal [v]; Assassin's Creed Unity (GDC 2015) [v abstract] |

## How they fail, and how they are checked

| Technique | Fails by | Checked by |
|---|---|---|
| T1 | Non-uniform scale on a parent shears its children | Not stated |
| T2, T3 | Proximity grabs the wrong part where parts are close (armpit, crotch, between the legs); rigid details bend; far parts warp | Previews at slider extremes. Roblox validates cages on upload; Epic shows the chosen size |
| T4 | Nearest-only transfer tears skirts and gives blotchy weights | That is why Epic inpaints |
| T6 | Holes open where the garment pulls away in a deep pose or on a thin body | A conservative rule, with margins along the body's edge loops |

## The robust path for a few body parameters, skin colour, hair and slotted gear that fits every body

Ranked by the agent; [i]:
1. **One skeleton, with height and proportions in the bones (T1).**
2. **Build and sex as a few morphs, copied onto every item offline (T2).** Use inverse-distance or surface binding, with rigid clusters and smoothing.
3. **Garment weights copied from the body by inpainting (T4),** with thigh-driven helper bones and spring bones for hems (T7).
4. **Covered regions hidden per triangle (T6),** with hair and ear flags and hat-hair variants.
5. **Skin tone by mask and palette in the shader (T8).** Per-character mesh merging and animation-rate LOD for crowds (T9).
6. **Layering (T5) only if layers return:** D64 has none.

Unpublished, so not known:
- Roblox's exact kernel and runtime cost.
- How MetaHuman interpolates between outfit sizes.
- How Cyberpunk blends garment support at runtime.
- Red Dead Redemption 2's skin and variety pipeline.

## Sources

**Body proportions and morphs**
- [UMA](https://github.com/umasteeringgroup/UMA)
- [Second Life Fitted Mesh](https://wiki.secondlife.com/wiki/Mesh/Rigging_Fitted_Mesh)
- [BodySlide and Outfit Studio](https://github.com/ousnius/BodySlide-and-Outfit-Studio)
- [Starfield morphs](https://www.nexusmods.com/starfield/articles/268)

**Fitting garments to bodies**
- [Mutable docs](https://github.com/anticto/Mutable-Documentation/wiki)
- [Roblox caging](https://create.roblox.com/docs/art/accessories/caging-best-practices)
- [Roblox WrapLayer](https://create.roblox.com/docs/reference/engine/classes/WrapLayer.md)
- [Roblox patent US11615601B2](https://patents.google.com/patent/US11615601B2/en)
- [MetaHuman wardrobe](https://dev.epicgames.com/documentation/metahuman/tailoring-your-own-wardrobe-items)

**Skin weights**
- [Robust skin weights transfer](https://github.com/rin-23/RobustSkinWeightsTransferCode)
- [Roblox automatic skinning](https://create.roblox.com/docs/art/accessories/automatic-skinning-transfer)

**Layering and hiding**
- [Cyberpunk garment support](https://wiki.redmodding.org/cyberpunk-2077-modding/for-mod-creators-theory/3d-modelling/garment-support-how-does-it-work)
- [Roblox hidden-surface patent US12190427B2](https://patents.google.com/patent/US12190427B2/en)
- [WoW character customisation](https://wowdev.wiki/Character_Customization)

**Secondary motion, skin and crowds**
- [VRM spring bones](https://github.com/vrm-c/vrm-specification/blob/master/specification/VRMC_springBone-1.0/README.md)
- [Jimenez skin colour](https://www.iryoku.com/skincolor/)
- [Unreal Animation Budget Allocator](https://dev.epicgames.com/documentation/en-us/unreal-engine/animation-budget-allocator-in-unreal-engine)
