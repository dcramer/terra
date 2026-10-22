# Creature rigs: how shipped games make many creatures move well

2026-10-22. Question: how do shipped games and engines make many creatures' skeletons, weights and motion reliably, so that a generated animal or monster is generally good without an artist on each; and where does Terra's animal pipeline stand against them? For [docs/creature-rigs.md](../docs/creature-rigs.md).

Tags: **[P]** primary (paper, talk, the engine's own docs or code), **[S]** secondary (articles, wikis, forums), **[M]** measured here.

## Answer

- **Shipped games split three ways.** Humanoids are motion matched from long captures (For Honor, The Last of Us Part II, Far Cry 6) **[P/S]**. AAA quadrupeds are captured clips with additive layers, ground probes, IK and physics on top (Far Cry 4, Red Dead Redemption 2); The Last of Us Part II's dogs are the one confirmed motion-matched quadruped **[S]**. Games with many or player-made creatures make motion by rule over labelled parts (Spore, Rain World, Overgrowth, The Flame in the Flood) **[P]**.
- **Spore is the precedent for "generally good".** Bones tagged by capability, motion authored once against the tags, legs grouped by length with a duty factor and phase each, a particle IK; tested against several hundred players' creatures on a validation grid, about 90% passing **[P]**.
- **Nobody publishes a deformation bar.** Rigging studios run range-of-motion passes and judge by eye; the closest documented practice is Unreal's ML Deformer, trained on 5,000 to 25,000 random poses **[P]**. Metrics exist in papers (local volume, edge length, weight variation) **[P]**; a gate made of them is ours to calibrate.
- **Automatic weights:** bone heat (Pinocchio, Blender's since 2007) gives skin to the nearest bone; geodesic voxel binding (Maya) handles broken meshes; bounded biharmonic weights (libigl, MPL-2.0) need a tetrahedral mesh; Epic's robust weight transfer (MIT) copies a good body's weights by closest point and normal and fills the rest **[P]**. Dual quaternions remove the candy-wrapper twist and bulge at bends **[P]**.
- **Dynamic similarity holds for running mammals**: at equal Froude number v²/(g·h) relative stride, duty factor and phase match, and horses of every size change gait at the same number **[P]**. The quadruped transition values (walk to trot about 0.5, trot to gallop 2 to 3) were not confirmed at a primary source here; a robotics summary gives 0.3 to 0.6 and 1.1 to 1.7 **[S]**, so they are to be calibrated on our own kinds.
- **Quadruped capture we could ship: none free.** AI4Animation's dog data is CC BY-NC, LaFAN1 CC BY-NC-ND and human only **[P]**; Truebones' zoo is royalty-free with credit, files not redistributable, about $100 **[P]**; Unity store packs run under its standard EULA **[P]**.
- **For Terra [M]**: our animals' gaits are already made the Spore way (D424); the weak stages are hand-placed joints, bone heat with no zones, and no check. The stag's belly takes half its weight from the legs and stretches 118% in the gallop; a hand-rigged deer, 7% and 10.5%. Proposal: [docs/creature-rigs.md](../docs/creature-rigs.md).

## Terra today [M]

- **People**: the `character` skill (D115): Tripo's V3 auto-rig in Unreal's skeleton converted onto ours, clips made with Kimodo (D316, D347, D360), checked by `rig_check.py`, `deform_check.py` and `gait_check.py`. None of the three runs on a creature.
- **Animals**: `tools/creatures/animal.py`, two plans (`quadruped`, 10 kinds; `bird`, 6), plus `hen.py` and `dragon.py`. Every joint's head and tail is typed into the kind's `creature.json`; weights are Blender's bone heat on a copy scaled to 3 m, redone on a voxel-remeshed closed copy when more than 0.2% of the skin is left bare, then hand fixes per region (`haunch`, D424 amended); gaits are made in place from footfall tables and Alexander's relative stride, legs by two-bone IK, baked at 60 fps. Creatures skin with Bevy's linear blend (only people get `terra_walker::skinning`'s dual quaternions), and the game has no creature foot IK.
- **The belly, measured** with a throwaway Blender script (not committed): the skin under the trunk between 15% and 85% of the way from the fore to the hind leg roots and in the lowest 35% of the trunk's depth, its weights summed by bone, and its edges' length against rest through every clip, every second frame, the worst frame's 99th percentile and mean.

| Model | Belly vertices | Weight on leg bones | Gallop p99 stretch | Walk p99 stretch |
|---|---|---|---|---|
| Red deer stag (ours, bone heat) | 161 | 52% (upper arms 35, thighs 16, shins 8) | 118% (trot 114%) | 77% |
| Wild boar (ours, bone heat) | 91 | ~100% | 194% (trot 184%) | 96% |
| Farm dog (ours) | 54 | 68%, 37% the hind shins | 38% | 35% |
| Quaternius's deer (hand-rigged, CC0, 4,276 vertices) | 12 | 7% | 10.5% | 9.6% |

  The stag's and boar's bellies swing with their legs: bone heat gives each vertex to the bones nearest it, and under a deep chest the nearest are the tops of the legs, not the spine 40 cm above. That is the "behaving very weirdly" the developer saw. The box is crude: on the fox and the hind it caught only 17 to 24 vertices near the elbow, and on the hare, sat low on its flat hind feet, the feet; a real check needs the region from the fitted skeleton's landmarks, not a box.

## Sources

Gathered 2026-10-22. GDC Vault was in maintenance and chrishecker.com down, so a talk known only from its listing says so.

**Spore**
- Hecker, Raabe, Enslow, DeWeese, Maynard, van Prooijen, "Real-time Motion Retargeting to Highly Varied User-Created Morphologies", SIGGRAPH 2008, TOG 27(3) 27 **[P]**, https://chrishecker.com/Real-time_Motion_Retargeting_to_Highly_Varied_User-Created_Morphologies. Bones carry capabilities (grasper, mouth, foot, spine, root) and standard deforms; animators key in Spasm on one creature, each channel selecting bones by a query (capability, front or back, left or right, top or bottom); poses stored in a body-independent space and specialised at runtime; branch predicates (HasGraspers, IsUprightSpine) choose variants. Particle IK, joints as particles held by lengths (Jakobsen 2001), spine first then limbs, 5 iterations. Legs grouped by length, group frequencies as small ratios, a duty factor (Alexander 2003) and phase per foot; 7 or more feet procedural; no feet, float or inch-worm. Tested on several hundred creatures and about 1,000 animations, about 90% passing. 0.2 ms a 25-bone creature. Failed: no volume or self-collision, twisting at singularities, weeks for animators to learn what generalises, the result "not yet competitive" with a hand-animated character.
- Hecker, "How To Animate a Character You've Never Seen Before", GDC 2007 **[P, listing only]**.

**Unreal Engine 5**
- IK Rig and IK Retargeter **[P]**, https://dev.epicgames.com/documentation/unreal-engine/ik-rig-animation-retargeting-in-unreal-engine: motion carried by chains, matched by fuzzy name (arm, leg, spine, tail, tentacle, jaw), sides by X; IK goals with speed planting and stride warping. Auto retarget only for predefined skeleton types, https://dev.epicgames.com/documentation/unreal-engine/auto-retargeting-in-unreal-engine. No official quadruped Control Rig or animal motion-matching sample found.
- Motion matching (Pose Search) **[P]**, https://dev.epicgames.com/documentation/unreal-engine/motion-matching-in-unreal-engine; the Game Animation Sample, 500+ clips, Unreal only, commercial use allowed **[S]**, https://www.cgchannel.com/2024/06/get-500-free-game-ready-animations-for-use-in-unreal-engine
- ML Deformer: 5,000 to 25,000 random poses, 15,000 typical **[P]**, https://dev.epicgames.com/documentation/unreal-engine/how-to-use-the-machine-learning-deformer-in-unreal-engine

**Motion matching and shipped quadrupeds**
- Clavet, "Motion Matching and The Road to Next-Gen Animation", GDC 2016 (For Honor) **[P]**, https://gdcvault.com/play/1023280/Motion-Matching-and-The-Road: a few bones matched (feet, weapon) with velocity and trajectory; needs very large capture sessions.
- Konieczny and Pelletier, "Grounding Wildlife in the Mountains of Far Cry 4", GDC 2015 **[P]**, https://www.gdcvault.com/play/1022027/Grounding-Wildlife-in-the-Mountains: additive clips, environment probes, IK and physics.
- The Last of Us Part II: motion matching applied to the dogs; dogs and horses captured **[S]**, https://wccftech.com/the-last-of-us-part-ii-motion-matching-tech/amp/
- Red Dead Redemption 2's horses: capture, Euphoria, "stop matching" near marked edges **[S]**, https://www.pcgamer.com/uk/the-mind-boggling-work-that-went-into-red-dead-2s-incredibly-lifelike-horses/
- Frey, "Animating Quadruped Characters in The Flame in the Flood", GDC 2016 **[P]**, https://gdcvault.com/play/1023209/Animating-Quadruped-Characters-in-The
- Holden, Kanoun, Perepichka, Popa, "Learned Motion Matching", SIGGRAPH 2020 **[P]**, https://www.theorangeduck.com/media/uploads/other_stuff/Learned_Motion_Matching.pdf: a 590 MB database becomes 17 MB of networks. Code MIT, https://github.com/orangeduck/Motion-Matching; its data, LaFAN1, CC BY-NC-ND 4.0, https://github.com/ubisoft/ubisoft-laforge-animation-dataset

**Gaits**
- Alexander and Jayes 1983, J. Zool., dynamic similarity **[P, abstract]**, https://zslpublications.onlinelibrary.wiley.com/doi/10.1111/j.1469-7998.1983.tb04266.x; holds mainly for cursorial mammals **[P]**, https://www.ncbi.nlm.nih.gov/pmc/articles/PMC3798186/
- Bullimore and Burn 2006, JEB 209:455, horses dynamically similar **[P]**, https://journals.biologists.com/jeb/article/209/3/455/16439/Dynamically-similar-locomotion-in-horses
- Transition values from a robotics paper's summary, not opened **[S]**, https://arxiv.org/pdf/2111.01674
- Rosen, "An Indie Approach to Procedural Animation", GDC 2014 **[P]**, https://gamedeveloper.com/design/video-an-indie-approach-to-procedural-animation
- Zhang, Starke, Komura, Saito, "Mode-Adaptive Neural Networks for Quadruped Motion Control", SIGGRAPH 2018 **[P]**: learned from unlabelled dog capture (about 30 minutes, **[S]**), walk to canter, jump, sit.

**Skinning and deformation**
- Baran and Popović, Pinocchio, SIGGRAPH 2007 **[P]**, https://people.csail.mit.edu/ibaran/papers/2007-SIGGRAPH-Pinocchio.pdf; Blender's bone heat from its section 4 (2007) **[P]**, https://lists.blender.org/pipermail/bf-blender-cvs/2007-July/010641.html
- Jacobson, Baran, Popović, Sorkine, Bounded Biharmonic Weights, SIGGRAPH 2011 **[P]**, https://homes.cs.washington.edu/~jovan/papers/jacobson-2011-bbw.pdf; libigl `bbw.h`, MPL-2.0, https://github.com/libigl/libigl/blob/main/include/igl/bbw.h
- Dionne and de Lasa, Geodesic Voxel Binding, SCA 2013 **[P]**, https://diglib.eg.org/handle/10.2312/15001: robust to non-manifold, open, self-intersecting and many-part meshes; Maya's bind, joints inside the volume.
- Abdrashitov, Raichstat, Monsen, Hill (Epic), Robust Skin Weights Transfer, SIGGRAPH Asia 2023 **[P]**, https://github.com/rin-23/RobustSkinWeightsTransferCode (MIT): copy where the closest point is within 0.05 of the bounding diagonal and the normal within 35°, inpaint the rest (weights may go negative, so clamp).
- Kavan, Collins, Žára, O'Sullivan, dual quaternion skinning, I3D 2007 **[P]**, https://users.cs.utah.edu/~ladislav/dq/; optimised centres of rotation, Le and Hodgins 2016 **[P]**.
- Rohmer, Hahmann, Cani 2008, local volume preservation **[P]**, https://hal.archives-ouvertes.fr/hal-00319640; Lewis, Cordner, Fong, pose-space deformation, SIGGRAPH 2000 **[P]**.
- Learned rigging: RigNet (2020, GPL-3.0) **[P]**, https://github.com/zhan-xu/RigNet; UniRig (SIGGRAPH 2025, MIT code, weights' licence unstated, degrades with a wrong skeleton) **[P]**, https://github.com/VAST-AI-Research/UniRig; Tripo's rig claims quadruped, hexapod, octopod, avian, serpentine and aquatic **[S, vendor]**, https://developers.tripo3d.com/en/models/rig; Mixamo bipeds only **[P]**, https://helpx.adobe.com/creative-cloud/faq/mixamo-faq.html

**Quadruped capture and licences**
- AI4Animation (MANN's dog data): capture CC BY-NC 4.0, code research only **[P]**, https://github.com/sebastianstarke/AI4Animation
- Animal Kingdom (CVPR 2022): video and keypoints, not capture, licence unstated **[P]**, https://github.com/sutdcv/Animal-Kingdom
- Truebones: royalty-free including games, credit asked, no redistribution of the files **[P]**, https://truebones.gumroad.com/p/truebones-terms-of-use-for-truebones-motions-t-o-s; the full zoo about $100, https://truebones.gumroad.com/l/skZMC; whether captured or keyed unknown.
- Unity Asset Store assets usable in other engines under its EULA, not redistributable raw **[P]**, https://support.unity.com/hc/en-us/articles/34387186019988-Can-I-use-assets-from-the-Asset-Store-with-other-engines
