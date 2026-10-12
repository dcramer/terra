# One mesh for every body: how gear fits many bodies without a model per pair

2026-10-12. Question: how do we make every worn item fit every body, every body template and later any body shape, without a model for each item on each body (items × bodies), while the characters still look good and the system stays easy to add to?

The first approach, each piece made on each base body from a dressed model of that body (D128 as it stood), works. Every piece passed the clipping check on both bodies (see [docs/characters.md](../docs/characters.md)). But the cost grows as items × bodies: each new piece needs a dressed Tripo model per body, and a new body needs every piece made again. This note surveys how shipped games and open tools avoid that. It is read, not tried; the spike that tries it is linked at the end.

> **Since:** on 2026-10-12 the developers chose morphs on a base body of our own and ruled out MakeHuman. What follows on MakeHuman is kept as what was found; see [modern-character-variety.md](modern-character-variety.md) and [docs/character-system-plan.md](../docs/character-system-plan.md).

## Answer

**All bodies share one mesh, and a garment is bound to that mesh once.** Every body, and every template of it, is the same mesh with the same vertices, order and texture layout, moved by stored shapes (morph targets). A garment records, once, where each of its vertices sits on that mesh. Placed on any body, it moves as that body's surface does. A new item is then one model made once, and a new body is one fit of the shared mesh. That costs items + bodies, not items × bodies.

The shared mesh to use is **MakeHuman's** (hm08), which is CC0. It is the only open body that is free for commercial use and already has everything this needs:

- a clean all-quad body of 13,380 vertices;
- interpretable shape targets (gender, age, weight, muscle, height, proportions, and hundreds of detail targets);
- a skeleton computed from the mesh, so every shape gets its joints;
- skin weights for a 53-bone game rig;
- a clothing format that binds a garment once and fits it to any shape;
- skirt and tights helpers for loose clothes.

We keep Tripo for the look. A Tripo body becomes one more shape of the shared mesh, with its painted texture moved onto the shared layout. A garment's colour and surface detail are baked once, from one Tripo model of the base wearing it.

## How shipped games do it

There are four strategies, often used together:

| Strategy | Who | Made per body? | Fails where |
|---|---|---|---|
| Make each item for each body, and keep bodies few | Baldur's Gate 3 (four body types), Cyberpunk 2077 (two), Monster Hunter, WoW helmets, Skyrim's thin and heavy pair | Yes | Every new body: Cyberpunk and Skyrim mods need a "refit" of every garment for every body mod |
| Bend everything with the bones | Second Life Fitted Mesh, Conan Exiles | No | Only stretches: no local shapes (chest, belly), rigid plates stretch |
| Carry the body's shapes onto the garment | The Sims 4, BodySlide, Unreal Mutable, MetaHuman outfits | No | Far from the body it was made on it warps; loose cloth needs its own rule; rigid parts must be left out |
| Hide the body under what covers it | WoW, Bethesda, Sims, Second Life, Mutable, MetaHuman | — | Partly covered skin |

- **The Sims 4** is the closest to "nothing made per body". Each body shape is stored once as an image over a flattened body layout, and every garment vertex carries where it sits on that layout, so one map moves every item. A second, looser map, blended per vertex, keeps skirts and baggy clothes from clinging ([GDC 2015](https://gdcvault.com/play/1022085/Innovations-in-The-Sims-4), [patent US9626808B2](https://patents.google.com/patent/US9626808B2/en)). Its failure is that one bad map breaks every item at once ([forum summary](https://www.patreon.com/posts/118466613)).
- **BodySlide** (Skyrim and Fallout 4) copies each slider of a reference body onto nearby outfit vertices ("Conform All"), then bakes meshes and morph files. Rigid plates are left out ([wiki](https://github.com/ousnius/BodySlide-and-Outfit-Studio/wiki/Creating-a-BodySlide-project)).
- **MetaHuman outfits** (Unreal 5.6) warp a garment from the body it was made on to any other by RBF interpolation. The meshes must share vertices. Warping grows with distance, so Epic advises several source sizes ([docs](https://dev.epicgames.com/documentation/metahuman/getting-started-for-creating-parametric-clothing-in-metahuman), [API](https://dev.epicgames.com/documentation/unreal-engine/API/Plugins/ChaosClothAssetDataflowNodes/FChaosClothAssetApplyResizingNod-)).
- **Mutable** reshapes a garment by the body's morphs, and breaks when topologies differ ([API](https://dev.epicgames.com/documentation/en-us/unreal-engine/python-api/class/CustomizableObjectNodeMeshReshape), [UE-372131](https://issues.unrealengine.com/issue/UE-372131)).
- **Cyberpunk's Garment Support** is a stored "squashed" shape per garment plus an order, so an outer garment flattens the inner one. It is how layers avoid clipping, and needs no runtime collision ([RED wiki](https://wiki.redmodding.org/cyberpunk-2077-modding/for-mod-creators-theory/3d-modelling/garment-support-how-does-it-work)).
- **WoW** makes many items as textures over the body's layout plus a few shared body-part variants (geosets) such as flared robes and bulky boots ([wiki](https://warcraft.wiki.gg/wiki/Character_model)). A few shared shapes plus textures scale to thousands of items.

Everyone also hides the body under what covers it, which we already do.

## How a garment follows a body

- **MakeHuman's binding** (`.mhclo`). Each garment vertex stores three body vertices, weights summing to one, and an offset: `p = Σ wₖ·v[rₖ] + S·d`. The offset is scaled per axis by how much a body measurement changed. Skin weights are blended from the same three vertices. Each vertex binds only to body faces in the same named region, so a sleeve never binds to the chest. Loose skirts bind to the skirt helper, which carries every body target, instead of the legs ([format](https://github.com/makehumancommunity/mpfb2/blob/master/docs/fileformats/mhclo.md), [matching](https://github.com/makehumancommunity/mpfb2/blob/master/docs/entities/clothes/vertexmatch.md)).
- **Blender's Surface Deform** works the same way but keeps the offset along the deformed face's normal, which holds up better under large shape changes ([design](https://archive.blender.org/wiki/2015/index.php/User:Lucarood/SurfaceDeform/Technical)).
- **RBF warps** need no shared mesh, but smooth away the body's shape, give no guarantee against clipping, and let regions pull on each other (legs on a skirt). With a shared mesh, binding is strictly better. RBF earns its place only for bodies that do not share it.
- **Getting a generated body onto the shared mesh** is a registration problem.
  - First fit the mesh's own shape targets to the body: a small least-squares problem.
  - Then fit the residual with a smoothness-regularised non-rigid fit. Optimal Step NICP (Amberg et al., CVPR 2007) is the standard, and has an MIT implementation in trimesh.
  - Finally bake the generated texture onto the shared layout.
- **Skin weights for parts that stand off the body** (coat tails) come from Epic's weight inpainting: keep only reliable nearest matches and fill the rest smoothly ([paper and MIT code](https://www.dgp.toronto.edu/~rinat/projects/RobustSkinWeightsTransfer)).
- **A skeleton for every shape.** MakeHuman places each joint at the centre of a small helper cube that every target moves, so joint positions are a linear function of the shape weights. Animations then play on any proportions if clips carry rotations only, with the hips' height scaled. That is the "strip the clips' joint translations" item already open in D115.

## What the shared mesh gives us

Measured from MPFB 2.0.18 and the CC0 system assets pack:

- **Body:** 13,380 vertices, about 26.8k triangles. Units are decimetres, in an A-pose. The helpers (tights, skirt, hair, eyes, teeth, joint cubes) come after the body's vertices and are dropped for the game.
- **Targets:** 1,258. Of these, 348 are macro targets (gender × age × muscle × weight, height, proportions); the rest are detail targets for body, face and expressions.
- **Rigs:** default (163 bones), game_engine (53, mannequin names, close to ours), mixamo, and others. Each has CC0 weights over every vertex, and joints placed from the helper cubes. There is a T-pose for the game_engine rig.
- **Parts:** proxy topologies down to 741 vertices, sharing the body's texture layout (useful for distant levels of detail; a middle level must be made), eyes, teeth, brows, 14 garments, 10 hairstyles and 24 skins.
- **Licence:** the assets are CC0, including mesh, targets, rigs, weights and clothes (MPFB [LICENSE.md](https://github.com/makehumancommunity/mpfb2/blob/master/LICENSE.md), section C). The code is GPL-3 (MPFB) and AGPL-3 (MakeHuman), so we reimplement from the formats and never copy code. The binding is about twenty lines.

The alternatives:

- SMPL, SMPL-X, STAR and SKEL are non-commercial.
- MB-Lab makes everything it generates AGPL.
- Reallusion's and Daz's licences rule them out.
- MetaHuman is authored only in Unreal.
- Meta's MHR (Apache-2.0) has real levels of detail, but only abstract shape parameters and no clothing.
- NAVER's Anny (Apache-2.0) is MakeHuman's data with a calibrated, differentiable model. It could serve as an offline fitter, but avoid its non-commercial SMPL-X extra.
- No notable shipped game is built on MakeHuman, but Humanizer (Godot, AGPL) shows a runtime creator on its targets and clothing.

## What it means for Terra

The design is in [docs/characters.md](../docs/characters.md) (D112, D128). In short:

- **One base mesh** (hm08's body) for everyone. Male, female and every template are its shapes, our Tripo bodies included, each with its texture on the shared layout.
- **Our skeleton** placed on each shape by rule, from the shape's own joints. Clips carry rotations, so they play on any proportions.
- **Gear made once:**
  - A garment is the base mesh's faces it covers, pushed out, with colour and surface detail baked once from one dressed model.
  - Thick or loose parts are bound to the base mesh's surface or its skirt helper, MakeHuman's way.
  - Rigid things hang on sockets, as now.
- **A body is built when a character appears or changes clothes**, from a few bytes of appearance: the shape's vertices, its joints and each piece fitted to it, cached per appearance and item. The binding and fitting are reimplemented in a Rust crate that does not depend on Bevy, shared by the offline tools and the game.

## Limits

- Read, not tried, apart from the shell garment on two separate bodies. Untried: fitting the shared mesh to our stylised Tripo bodies well enough to look as good as they do, and one garment passing the clipping check on every shape.
- The body is about 27k triangles before eyes and teeth. Thousands of players on screen need lighter versions (D127).
- Little is public on how Black Desert, Elden Ring, Dragon's Dogma 2 or Enshrouded deform armour; several details above come from modding communities rather than the developers.
- Roblox's cage-to-cage layered clothing is patented ([US11615601](https://image-ppubs.uspto.gov/dirsearch-public/print/downloadPdf/11615601)); we do not copy it.
