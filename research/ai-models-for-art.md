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

The developers have tried Meshy and found it underwhelming (2026-10-11), and Reddit's reports agree (below). Tripo looks the best fit: parts, low poly, the widest rigging, and first on the one 2026 blind-vote ranking found ([as reported by Sloyd](https://www.sloyd.ai/blog/ai-3d-model-generator-rankings), August 2026). A studio's informal test found Tripo best at stylised work and Rodin's cartoon still realistic ([StraySpark](https://www.strayspark.studio/blog/generative-3d-tools-comparison-meshy-rodin-tripo-csm-2026), April 2026). One independent review found Meshy's Smart Topology missed its own face target ([GlobalGPT](https://www.glbgpt.com/hub/meshy-v7-review/)).

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

## What developers on Reddit say

Added the same day, after the developers asked for Reddit's view. Reddit blocks direct searches, so these threads were found through the [Arctic Shift](https://arctic-shift.photon-reddit.com) archive, mostly in r/aigamedev, r/StableDiffusion and r/blender, from 2025-08 to 2026-10. They are individuals' experiences, several from people promoting tools, and r/gamedev removes or attacks such posts, so this is a skewed sample.

- **Tripo is the generator people use for game assets.** Pipelines built on Tripo, then Blender, then an engine recur ([1](https://reddit.com/r/aigamedev/comments/1vgx9wu/my_entire_indie_game_pipeline_is_basically_ai_now/), [2](https://reddit.com/r/aigamedev/comments/1wb8vzb/ai_blender_mcp_is_getting_pretty_wild/)). One developer built a ruined arena in 72 hours from Tripo P2 pieces, choosing quad output and a triangle budget per piece by how much it mattered, and assembled them in Blender with array and curve modifiers ([3](https://reddit.com/r/aigamedev/comments/1w0t180/i_built_this_dark_fantasy_environment_with_3d_ai/)). When someone spent $37.50 of GPT usage having it model a character in Blender, the top reply was that $8 of Tripo would have made 200; others answered that Tripo's newer low-poly model has "pretty clean topology" ([4](https://reddit.com/r/aigamedev/comments/1wetkmm/results_of_burning_150_of_a_5x_plans_weekly_usage/)).
- **Meshy is spoken of poorly for game work**: "everything I've experienced with Meshy has been pretty garbage. The vertex count of models is through the roof", and "Meshy sucks particularly" ([5](https://reddit.com/r/aigamedev/comments/1vq8thi/rigging_faces_generated_by_meshy_or_tripo/)). One test found Meshy and Tripo more reliable than local Hunyuan3D: high 70s against about 60% of results usable after clean-up ([6](https://reddit.com/r/StableDiffusion/comments/1uciv8k/open_source_vs_commercial_ai_3d_generators_a/)).
- **Others rate Hitem3D and Rodin higher on shape**, especially for organic things such as monsters, with Tripo and Meshy below ([7](https://reddit.com/r/StableDiffusion/comments/1o6eett/what_are_the_best_tools_for_3d_gen/), [8](https://reddit.com/r/StableDiffusion/comments/1mg1kx9/genai_3d_asset_platforms_compared_tripo_vs_meshy/)). A developer who sketches characters by hand found Rodin's meshes "highly accurate", with good topology when symmetry is on ([9](https://reddit.com/r/gamedev/comments/1uoc3oh/3d_gen_ai_to_speed_up_asset_creation/)). Neither was in this note's shortlist; Hitem3D only as a vendor claim.
- **Locally, TRELLIS.2 and Pixal3D are what people run**, now built into ComfyUI with nothing non-commercial in the chain ([10](https://reddit.com/r/StableDiffusion/comments/1w3znex/trellis2_and_pixal3d_are_now_native_in_comfyui/)). One developer's open pipeline goes from an image to a model of about 5,000 faces ([11](https://reddit.com/r/StableDiffusion/comments/1wucthp/local_image_to_3d_high_quality_low_poly_preserve/)). Opinions split between "pretty good, very little clean-up" with several views of the subject ([12](https://reddit.com/r/aigamedev/comments/1x02gfs/how_are_you_creating_3d_models/)) and "nowhere near" the paid services ([13](https://reddit.com/r/StableDiffusion/comments/1v0yw3s/text_to_3d_vs_image_to_3d_in_2026_which_modality/)). Several views beat one, for every tool.
- **Claude with Blender works for simple, stylised models.** In the same thread, "Opus 5.5 -> blender" is a common answer. One developer runs headless Blender, MCP and Opus 5.5 unattended to build assets from written specifications, and has it write scripts that make whole kits ([12](https://reddit.com/r/aigamedev/comments/1x02gfs/how_are_you_creating_3d_models/)). The consensus: it "does well with low poly, basic models at large scale" and is not ready for complex ones ([4](https://reddit.com/r/aigamedev/comments/1wetkmm/results_of_burning_150_of_a_5x_plans_weekly_usage/)). One suggestion from that thread: generate a detailed mesh first and give it to the agent to rebuild in parts from simple shapes. That is route 2 with MeshCoder's idea in it.
- **Parts beat one solid mesh.** Asking for a model as separate parts, or making it part by part and assembling it, comes up again and again ([5](https://reddit.com/r/aigamedev/comments/1vq8thi/rigging_faces_generated_by_meshy_or_tripo/), [8](https://reddit.com/r/StableDiffusion/comments/1mg1kx9/genai_3d_asset_platforms_compared_tripo_vs_meshy/), [14](https://reddit.com/r/aigamedev/comments/1wiwsvf/i_made_an_8legged_spider_for_my_game_using_ai_and/)). An eight-legged creature was made that way, with Tripo P2 for the parts, but its rig, weights and walk were done by hand ([14](https://reddit.com/r/aigamedev/comments/1wiwsvf/i_made_an_8legged_spider_for_my_game_using_ai_and/)).
- **Faces and animation are where it breaks.** Generated faces cannot be rigged to deform; the advice for stylised games is to swap between a set of expressions instead ([5](https://reddit.com/r/aigamedev/comments/1vq8thi/rigging_faces_generated_by_meshy_or_tripo/)). Animation by LLM is "crude"; mocap libraries such as Mixamo, video to pose, or Cascadeur are what people recommend ([15](https://reddit.com/r/aigamedev/comments/1wzl26v/i_want_to_make_animations_of_my_characters_what/)). One developer had Claude animate a Tripo dog's idle through Blender MCP with good results, by iterating ([2](https://reddit.com/r/aigamedev/comments/1wb8vzb/ai_blender_mcp_is_getting_pretty_wild/)).
- **Taste stays human.** On Anthropic's Blender connector (April 2026): "a very fast, very obedient junior artist who can execute but can't art direct" ([16](https://reddit.com/r/blender/comments/1sy6yt9/anthropic_just_became_an_official_blender/)). A developer who asked for "AAA" got a Roblox look until the lighting and post-processing changed; that developer also built a viewer where you point at a part of the model and leave Claude a note for its next pass ([17](https://reddit.com/r/aigamedev/comments/1vkg7jr/letting_ai_create_graphics_and_3d/)). Another turns a feeling into rules the agent checks every model against ("soft, never sharp") ([18](https://reddit.com/r/aigamedev/comments/1wxc583/your_ai_games_look_generic_because_of_one_mistake/)). That is the style guide above.

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

- **Steam requires disclosure** of generated content that ships and that players see, made before release ("pre-generated"). Its January 2026 rewrite says efficiency gains in making the game are not the point. Inferred: meshes from a generator need disclosing even after clean-up. Whether geometry the agent writes as code counts too is not clear; the safe reading is yes. D1 already allows generated assets, and the developers have since said they will disclose that everything is AI-made (D107).
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

A bake-off, not started. Six models chosen to cover the categories: a set of rocks, a fence or ruin, a workbench, a tree, a wolf-sized creature and an NPC. Each made four ways:

1. **Tripo**, untextured and low-poly, in parts.
2. **The agent in Blender**, from scratch, through Blender's MCP server or headless scripts.
3. **The agent in Blender, from a Tripo model**: given the generated mesh as a reference, rebuild it in parts in our style.
4. **Locally**: TRELLIS.2 or Pixal3D, then decimation.

Every result is coloured in one palette and rendered in the sandbox's light on one sheet for the developers to judge. Meshy is left out: the developers found it underwhelming. Rodin or Hitem3D could be added for the creature if Tripo's is weak. Cost: a month of Tripo and well under $50 of API use. It would show where each route's ceiling is for our look, and whether the clean-up can be fully scripted.

## The bake-off, first round

Run on 2026-10-11 in [spike/model_bakeoff](../spike/model_bakeoff/README.md). Six models: rocks, a fence, a workbench, a tree, a wolf, a villager.

**The agent in Blender, from scratch: not good enough.** The developers called the results "pretty garbage", and they were: readable blockouts. The fence and workbench were passable props, the rocks and tree generic, the wolf and villager crude. This matches the benchmarks' ceiling for organic shapes.

**Image first, then a generator, then the agent: good.** All six concepts came out usable at the first attempt, and the wolf, villager, workbench, tree and fence came out close to their concepts. The rock lost its concept's broad planes and came out lumpy; it is the one case where a generator written as code does as well. The steps (`spike/model_bakeoff/make.sh`):

1. **Concept**: gpt-image-2 through Tripo's API, with a fixed brief: one subject, three-quarter view, plain background, finely cut flat planes, flat colours. Checked by eye before anything else is spent. 10 credits ($0.10).
2. **Four views** of the concept, made by Tripo from the one image. 10 credits.
3. **A low-poly model from the four views** (Tripo P1), with a texture. 50 credits, not the 30 the price list suggests.
4. **Refinement in Blender**, free:
   - clear the generator's smoothed normals, which otherwise spoil flat shading here and in the game;
   - weld the seams;
   - cut to any number of triangle budgets;
   - give each face the palette colour nearest its texture there, then make flat regions and speckle agree;
   - set the size;
   - check it, export glTF, and render a sheet in our light.

**About $0.70 a model.** The round cost 470 credits ($4.70), including an extra wolf at a higher face cap. Tripo runs one task at a time on this account.

**Budgets.** One model generated at a generous cap and reduced in Blender gives every level of detail for one price. The wolf held its shape from 7,700 triangles down to 1,500; the lower budgets have bolder facets, closer to the concept. The villager at 2,500, the workbench, tree and fence at 1,500, and the rock at 300 still read well.

**What it does not yet do:** split a model into parts, rig or animate, or make sets of pieces that fit together. Faces come out rough (eyes as dark marks).

## The bake-off, second round: style

Run on 2026-10-11, the same day. Pages: [Terra Style Directions](https://claude.ai/artifact/AWgMLGJmS5GR7hvjJMbKq4).

- **Mood boards of four directions** (A facets, B soft, C painted, D graphic), each with the same five subjects. The developers were first drawn to D, then said C's lighting and look were better, provided it can work with many realms.
- **D can be a shader.** Our existing models, rendered with light cut into three crisp bands, a cool shadow band and distance haze, read like the D concepts. The same files recoloured by palette role made convincing starter, cold, gothic, ashen and crystal realms.
- **C can be made to work with realms** by splitting it: colour stays a palette role per part, and the painted look moves into shared material detail (brush-stroke variation, painted top light, rims) and the lighting (soft sun, bounce, occlusion, haze, bloom). Only detail painted uniquely onto one model is lost.
- **Life size (D110)** changes how models must be made: detail scales with the world, not the object. A tree designed at 5.5 m and scaled to 20 m keeps clumps the size of cars and looks like a toy.
- **The mood board** is four C-style concepts: a medieval town and a techno-punk street, fantasy wilds and a gothic graveyard. Built for real in Blender from the pipeline's models and buildings made from parts in code, the same four worlds came out structurally right and far plainer than the concepts. The gap is set dressing, ground cover, density and lighting work, not the models.
- **Costs.** A model made straight from one image costs 50 credits, not the 30 the price list suggests, so a prop is 60 credits ($0.60) with its concept. The whole day spent 1,770 credits ($17.70) for about 30 models and 50 images. Tripo runs one task at a time on this account, and its image queue took 5 to 10 minutes an image in the afternoon.

## Image to Blender

Tried on 2026-10-11 at the developers' request ([spike/model_bakeoff/image](../spike/model_bakeoff/image)): the agent modelled the workbench and the wolf in Blender against their concept images, rendering from the concept's camera beside it and correcting each round. No credits.

- **The workbench, after four rounds:** recognisably the concept's bench, with every part (plank top and holes, leg vice with hanging handle, splayed legs, shelf, crate, hammer) as clean separate pieces in palette roles, about 1,500 triangles, and parametric. But less faithful than Tripo's from one image: the proportions and the legs' splay never quite matched, and judging by eye, fixing one thing broke another more than once.
- **The wolf, after two rounds:** poor, as the benchmarks predicted. A stick skeleton skinned in code reads as a wolf-like animal, not the concept's wolf.
- **So:** code with an image is a fair way to make props and kit pieces when clean parts or variants matter more than fidelity, and no way to make creatures or characters. Tripo stays the route for organic shapes and for any prop that must match its concept closely.

## Characters made and rigged

Worked out on 2026-10-11 ([spike/character_gear](../spike/character_gear/README.md)). The male base body, from an approved design sheet, now plays every sandbox animation.

- **Generate in a T-pose.** Our skeleton and its animations are in a T-pose. A body generated with its arms down (an A-pose) and moved into the T-pose crushed its shoulders every time the arms came down, in every animation. The design sheet is redrawn in a T-pose (one image edit, keeping the design) before the model is made.
- **Let Tripo rig it, then convert.** Studio's auto-rig (about 20 credits) puts a 78-bone humanoid skeleton in the body with good weights. A script moves our skeleton's joints to Tripo's joints without turning any bone, renames Tripo's weights onto our bones (folding its twist and helper bones into the bones they help), smooths them lightly, and keeps four bones a vertex.
- **Judge with a checker, against a known-good rig.** `rig_check.py` compares a rig with our skeleton and with the hand-rigged mannequin on the same animations, measuring how many edges crush or over-stretch. Results, as the share of such edges in the worst frame:

| Move | Mannequin, rigged by hand | Tripo's rig, converted | Blender's own weights |
|---|---|---|---|
| Idle | 0.16% | 0.16% | 0.32% |
| Walk | 0.17% | 0.29% | 0.50% |
| Jog | 0.45% | 1.1% | 1.5% |
| Crouch | 0.55% | 1.8% | 1.6% |
| Roll | 1.0% | 2.4% | 1.9% |

Copying the mannequin's own weights onto the body did twice as badly as either.
- **Mistakes this caught or would have:** the importer leaves the skeleton posed mid-animation, which silently became the rest pose; the skeleton's axes are turned by the importer, so "forward" in code was sideways; scaling the skeleton's data instead of its object broke the animations' hip movements; Blender's weighting fails on generated meshes unless given a watertight copy.
- **Not yet done:** Bevy applies every joint position stored in the clips, which would undo the moved joints in the game; the clips need their unchanging joint positions stripped, keeping only the hips. The deepest bends stretch the texture's seams into thin light lines on the shorts. The female base, templates, hair and gear are next.

## Limits

- **Nothing was tried.** Every quality claim here is a paper's, a vendor's or a commenter's.
- **Benchmarks don't match our look.** None studies flat-shaded low-poly styles, and none splits organic from man-made objects.
- **Vendors change monthly.** Model names, prices and features are as of 2026-10-11. Meshy's older `lowpoly` mode, for one, retires on 2026-10-30.
- **Unverified:** who owns output from Hunyuan's hosted service; the current text of Tripo's free-tier terms; how Tripo and Meshy lay out parts inside a glTF file (separate nodes or separate materials); Procedura's numbers; and the dates on some forum posts.
- **No shipped indie game** with an account of how it made AI-generated 3D models was found.
