# Making our own models with AI

2026-10-11. Question: we have no artists, and want great art. Can an agent, given the right tools, make the game's models, perhaps from models we choose as a base or from a generated model it redesigns? And what other tools, free or paid, make good 3D art today?

This is a survey of what sources say, made through web research on the day. Nothing here was tried. Where a claim is a vendor's own, it says so.

## Answer

Yes, mostly, but not by one route. Which route works depends on the kind of thing:

| Kind of model | Best route today | Confidence |
|---|---|---|
| Rocks, crystals, ruins, fences, building parts, furniture, tools and weapons | The agent writes it as code: a generator with a seed, checked by rendering it in the game's light | Good. This is where code is strongest, and our look (flat colour, visible facets, no textures) suits it |
| Simple trees, shrubs and ground plants | The same, as generators | Fair. Already done crudely in the `look` and `forest` tests |
| Creatures, NPCs, complex foliage, anything that must match a concept | A concept image, then a 3D generator, then the agent cleans the result up and colours it in our palette | Fair. The generators are good at organic shapes and bad at our style; the clean-up is what makes them ours |
| Rigging and animation | A generator's auto-rig (Tripo's covers the most body plans), or an open rigging model, with clips baked onto one skeleton per body plan | Fair for humanoids, weaker for creatures |

The developers' idea of redesigning a base model fits each route: a reference model can be rebuilt as code the agent can change, or rendered and used as the input to a generator. What may be used as a reference is limited by licence (below).

**Consistency is our job, not the tools'.** Every source that discusses a set of generated models says the same thing: each one comes out in its own style ("5 swords from 5 different games"). What makes ours one game is what we already do with the Quaternius packs: our palette, our shader, our light. Proportions and facet size need a written style guide the agent checks each model against.

**Our look removes the hardest part.** Every commercial and open generator textures its output by default, and the textures and UVs are where most of the clean-up effort goes in the reports. We use neither. We need clean low-poly geometry split into parts, which is exactly what `models.rs` already reads: a glTF whose triangles are grouped by named material, one colour each.

## The routes in detail

### 1. The agent as modeller, in code

The agent writes geometry as a program, in Blender's Python or as a Rust generator in the sandbox, renders it, looks at it and corrects it.

What the benchmarks say:

- **Tools matter as much as the model.** In [3DHarnessBench](https://arxiv.org/html/2609.06535) (September 2026), agents rebuilt target objects through Blender with four levels of access. Being able to query sizes and bounding boxes gave every agent its best result. Opus 5, run through Claude Code, was the strongest overall when it could interact. When it could choose its own camera views it improved about 16% over four fixed views; Gemini 3.1 Pro got worse and often stopped early.
- **One-shot code is weaker.** On [3DCodeBench](https://arxiv.org/html/2606.01057v1) (May 2026), with human pairwise votes on Blender scripts for 212 categories, GPT-5.5 and Gemini 3.1 Pro led. Claude Opus 4.7 was about 160 points lower, though it had nearly the highest share of scripts that ran. Feeding Blender's errors back raised the share that ran from 70% to 97%. The main failure the paper names is shapes that are not physically plausible: parts floating apart, or "simplistic, floating primitives".
- **Agents are still far from skilled people.** [BlenderGym](https://arxiv.org/html/2504.01786v1) (2025) found every model tested several times worse than human Blender users. Spending effort on checking candidates paid more than spending it on generating them.
- **Checking each part's connections helps.** [Procedura](https://www.emergentmind.com/papers/2608.26238) (August 2026) builds an object as named parts joined by checkable connections, and accepts a part only when it compiles, connects and touches what it should. It claims to beat 3D generators on hard-surface objects; we could not see its numbers.

What developers report ([a Hacker News thread](https://hn.svelte.dev/item/49541458), about September 2026; [MindStudio](https://www.mindstudio.ai/blog/claude-blender-mcp-real-world-performance), May 2026): good at props, hard-surface pieces and arranging scenes; poor at organic shapes and foliage. One commenter: "There's a threshold slightly over 'low poly' that Opus cannot get over." Another: geometry written as code is "clean by construction". One studio's workaround is for the model to make the low-poly silhouette and a generator to add detail.

The connection to Blender exists. Blender's own [MCP server](https://www.blender.org/lab/mcp-server/) (Blender 5.1 and later) runs Python the agent writes. The older community [MCP for Blender](https://github.com/ahujasid/mcp-for-blender) adds a `look` tool that renders from chosen angles, searches of free model libraries, and calls to Tripo, Hunyuan and Rodin. For a pipeline, headless `blender --background --python` is enough and easier to repeat.

Inferred for us: the agent should write **generators, not models**. A rock or a fence as a function of a seed and a few parameters gives variety without more work, is checkable, and fits planets as data (D59), where a theme picks parameters rather than new models.

### 2. A generator underneath

For organic shapes, a 3D generator makes the shape and the agent makes it ours:

1. **A concept image** in the house style: one subject, even light, plain background, flat colours. gpt-image-2 (April 2026) and Google's Nano Banana Pro both take several reference images to hold a style; FLUX.2 takes exact hex colours; Recraft keeps a style as an object in its API. Midjourney has no official API.
2. **A low-poly mesh in parts, untextured.** See the services below.
3. **Clean-up by the agent, in scripts:** merge, flatten nearly flat regions, cut to a triangle budget, check nothing floats, set size and origin against a reference figure, name each part's material and give it a palette colour (for example the concept image's average colour over that part), and export glTF.
4. **Judged in the game.** Rendered in the sandbox's light from set angles, next to models already accepted.

A model can also be turned into editable code: [MeshCoder](https://arxiv.org/html/2508.14879v1) (2025) converts a shape into Blender code part by part, and 3DHarnessBench measures frontier agents doing the same. This is the most direct form of "redesign this model": rebuild it as parameters, then change them.

#### Paid services

All three below have an API an agent can drive, and can return untextured low-poly meshes. Prices are the vendors' own, fetched on the day.

| Service | Low-poly and parts | Rigging | Cost per model | Output rights |
|---|---|---|---|---|
| [Tripo](https://developers.tripo3d.com/en/docs/generation-image-to-model/standard.md) | P1 model 50 to 20,000 faces; P2 (September 2026) up to 25,000 quads; split into parts (untextured only); a segmentation call for any mesh | Humanoid and creature: biped, quadruped, hexapod, bird, serpent, fish and more | About $0.10 to $0.50 with parts; rig $0.25; $0.10 per animation | Paid plans yours; free tier not for commercial use |
| [Meshy](https://docs.meshy.ai/en/api/image-to-3d) | "Smart Topology", 100 to 15,000 polygons, split into parts (July 2026); remesh to a target count | Humanoid only through the API, and it wants a textured model | A few cents to $0.40 on a $20/month plan | Paid plans yours; free tier CC BY, owned by Meshy |
| [Hunyuan 3D global](https://intl.cloud.tencent.com/document/product/1284/75281) (Tencent) | Untextured, low-poly and part modes; quad remesh | Yes | $0.30 to $1.00 | Not found for the hosted service |

Tripo looks the best fit: parts, low poly, the widest rigging, and first on the one 2026 blind-vote ranking found ([as reported by Sloyd](https://www.sloyd.ai/blog/ai-3d-model-generator-rankings), August 2026). A studio's informal test found Tripo best at stylised work and Rodin's cartoon still realistic ([StraySpark](https://www.strayspark.studio/blog/generative-3d-tools-comparison-meshy-rodin-tripo-csm-2026), April 2026). One independent review found Meshy's Smart Topology missed its own face target ([GlobalGPT](https://www.glbgpt.com/hub/meshy-v7-review/)).

Looked at and not recommended: Rodin (good, but its API needs the $120/month plan), Sloyd (cheap, templated props only), Kaedim (artists finish each model, about $400 a month and up), CSM (bought by Google DeepMind in January 2026, gone), Autodesk Wonder 3D (no API found), Adobe (no 3D generation now), Roblox Cube (research licence).

#### Locally, on the developer's GPU

The RTX 5090's 32 GB runs all of these. None gives our look from an image directly; the route is a dense mesh, then reduction, then our palette.

| Step | Model | Licence |
|---|---|---|
| Shape | [TRELLIS.2](https://github.com/microsoft/TRELLIS.2) (Microsoft, December 2025), or [Pixal3D](https://github.com/TencentARC/Pixal3D) built on it | MIT |
| Shape, fallbacks | [Step1X-3D](https://github.com/stepfun-ai/Step1X-3D); [TripoSG](https://github.com/VAST-AI-Research/TripoSG), light and quick | Apache 2.0; MIT |
| Shape in parts | [PartCrafter](https://github.com/wgsxm/PartCrafter), up to 16 parts from one image | MIT |
| Low poly | Plain decimation first; [TreeMeshGPT](https://github.com/sail-sg/TreeMeshGPT) (up to 5,500 or 11,000 faces) or [DeepMesh](https://github.com/zhaorw02/DeepMesh) to compare | Not needed; MIT |
| Rigging | [UniRig](https://github.com/VAST-AI-Research/UniRig) (people, animals and objects); [Puppeteer](https://github.com/Seed3D/Puppeteer) | MIT; Apache 2.0 |
| Motion from text | [Kimodo](https://github.com/nv-tlabs/kimodo) (NVIDIA, 2026), its SOMA weights | NVIDIA Open Model Licence, commercial use allowed |

Licences to avoid:

- **Every Tencent community licence**: the open Hunyuan3D 2.x models, Hunyuan3D-Part and -Omni, and HY-Motion. They do not apply in the EU, the UK or South Korea, and forbid using even their outputs there. That rules them out for a game sold in Europe.
- **Non-commercial**: MeshAnything v1 and v2, Meta's MeshFlow, NVIDIA's PartPacker.
- **Capped**: Stability's SF3D and SPAR3D, free only below $1M a year in revenue.

The public leaderboard, [3D Arena](https://arxiv.org/html/2506.18787v1), rewards textured renders (textured models score 144 points higher) and has no measure of topology, so its rankings say little about our use.

Inferred: the paid services are cheap enough that cost does not decide this. A bake-off of 20 models per service is under $50. Local models matter if we want no per-model cost, no dependence on a vendor, or many variations, and they are slower to set up.

### 3. Rigging and animation

- **Bevy plays a clip on any skeleton whose bone names and hierarchy match** ([AnimationTargetId](https://dev-docs.bevyengine.org/bevy/animation/struct.AnimationTargetId.html)); it has no humanoid bone map. Inferred: retarget offline, in Blender, onto one skeleton per body plan (biped, quadruped, bird), and every creature of that plan shares its clips. The Quaternius animation library (CC0) we use already is the natural biped.
- **Humanoids are well served.** Mixamo still works but is unmaintained. AccuRIG is free. Text-to-motion services (Uthana, SayMotion) and Kimodo are humanoid only.
- **Creatures are the hard part.** Tripo's auto-rig covers the most body plans; Meshy's web app does quadrupeds and winged bodies. [Cascadeur](https://www.cgchannel.com/2026/08/nekki-releases-cascadeur-2026-2-with-animation-layers/) ($96 a year for indies) is AI-assisted hand animation, the likely tool for a creature that matters, though it still needs someone to animate.

Mounts (D100), combat (D97) and third person mean characters and creatures will be seen closely and often. They are likely the largest art cost in the game, and the area where generated work is weakest.

## What can be used as a base

| Source | Can it be fed to a generator or used as a reference? |
|---|---|
| Quaternius, KayKit, Kenney | Yes: CC0 |
| Our own models and generators | Yes |
| Synty | **No.** Its licence (July 2026) forbids uploading its models to services for 3D generation, and generating 3D models from them. Using Synty models alongside generated ones appears allowed |
| Fab (Epic) and Sketchfab | Not if the listing is tagged NoAI; check each one |
| Poly Pizza | About two thirds are CC BY: usable, with credit |
| itch.io packs | Each page must say if it was made with AI; using an AI-made pack means disclosing it ourselves |

## Rights and reception

- **Steam requires disclosure** of generated content that ships and that players see, made before release ("pre-generated"). Its January 2026 rewrite says efficiency gains in making the game are not the point. Inferred: meshes from a generator need disclosing even after clean-up. Whether geometry the agent writes as code counts too is not clear; the safe reading is yes. D1 already allows generated assets.
- **Raw generated output has little copyright protection.** The US Copyright Office's 2025 report says prompts alone are not enough; human selection, arrangement and changes can be protected. The Supreme Court declined *Thaler v. Perlmutter* in March 2026. Inferred: keep a record of what we changed in each model.
- **Use paid plans.** Free tiers leave the rights with the vendor, or license them to us only with credit.
- **Reception is a real risk.** In the GDC 2026 survey 52% of developers saw generative AI as harmful, up from 30% a year earlier. The Indie Game Awards withdrew *Clair Obscur*'s awards in December 2025 over AI placeholder textures that were patched out within days. About a fifth of Steam games now carry the disclosure. Inferred: what hurts most is being caught not disclosing, and visible "slop". A consistent, finished look, honest wording, and no AI in key art or the store capsule reduce it. Whether the developers accept this is theirs to decide.
- **The EU AI Act's labelling duty** (from August 2026) is aimed at deepfakes and at providers of models. Inferred: stylised game art is unlikely to be affected unless the game generates content live.

## The tools the agent would need

Inferred from what made the difference in the benchmarks and the reports:

1. **Blender, run headless**, for clean-up and building from code.
2. **The model workbench** (D71): load a model on simple ground in the game's own light and shader, render it from set angles and from angles the agent chooses, and lay it out next to accepted models and a reference figure. A model is judged where it will be seen.
3. **Measurements and checks a script can run**: triangle count, size, parts that float or do not touch, and every material named and in the palette.
4. **A style guide** the agent can check against: proportions, facet size by object size, silhouette, palette rules per place.
5. **API keys** for one 3D service and one image model, on paid plans.
6. **A reference library** of models we may feed to generators, each tagged with its licence.
7. **A record per model**: where it came from, which tool and plan, the prompt, and what was changed.

The developers judge taste; the agent does the iterations. Contact sheets of candidates, rendered in the game, are what to bring them.

## A proposed next step

A bake-off, not started. Six models chosen to cover the categories: a set of rocks, a fence or ruin, a workbench, a tree, a wolf-sized creature and an NPC. Each made by the agent in code, by Tripo, by Meshy and by TRELLIS.2 locally with decimation; all coloured in one palette and rendered in the sandbox's light on one sheet for the developers to judge. Cost: a month of one or two paid plans and under $50 of API use. It would show where each route's ceiling is for our look, and whether the clean-up can be fully scripted.

## Limits

- **Nothing was tried.** Every quality claim here is a paper's, a vendor's or a commenter's.
- **Benchmarks don't match our look.** None studies flat-shaded low-poly styles, and none splits organic from man-made objects.
- **Vendors change monthly.** Model names, prices and features are as of 2026-10-11. Meshy's older `lowpoly` mode, for one, retires on 2026-10-30.
- **Unverified:** who owns output from Hunyuan's hosted service; the current text of Tripo's free-tier terms; how Tripo and Meshy lay out parts inside a glTF file (separate nodes or separate materials); Procedura's numbers; and the dates on some forum posts.
- **No shipped indie game** with an account of how it made AI-generated 3D models was found.
