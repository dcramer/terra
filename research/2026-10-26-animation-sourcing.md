# Animation sourcing: a library, a service or a suit for every clip, under a licence a public repository can hold

2026-10-26. Question: is there something we could buy that supplies the whole move list (idles and fidgets; walk, jog and sprint with starts, stops, turns and strafes; one-handed, two-handed, dual, shielded and unarmed sets; blocks, parries, hit reactions and deaths; rolls; jumps, falls and landings; swimming; free climbing and mantles; carrying and tools), well enough that the idle stops looking wrong, and keeps us able to make new moves later? The developer, 2026-10-26: "if there were some solution that we could purchase that would help us solve all of these (not just swimming) let me know / because things like the idle animation also still suck / but we have to be able to future proof". Earlier: "Find CC0 first, else buy". D526 sets how a paid option is proposed.

What we need from it: about 210 moves and 450 clips, 900 with the mirrored sets (holding.md), on our skeleton (Unreal's bone names, Quaternius's rest pose, twist bones, D115), carried by aim (D360), checked by `moves.ts` and `gait_check.py`. Kimodo makes locomotion (D316) and combat keyed to the guard (D450), but it cannot make a 7.4 m/s sprint, swimming or an idle that reads as a person standing. Climbing is still an Open design (controller.md). The repository is public and MIT (D38).

## Answer

No purchase covers the whole list well under a licence our repository can hold. The licences split the sources in two:

- **Public**: files may sit in a public repository, and the game may ship them, on any engine. These are CC0 and similar (Quaternius, CMU), CC BY (100STYLE), output we own (Kimodo, Uthana on a paid plan), and capture we make or commission with the rights assigned to us.
- **Private only**: every store pack (Fab, the Unity Asset Store, MoCap Online, Kevin Iglesias, ActorCore, Mixamo) allows use in a game on any engine. None allows the files, or clips baked from them, to sit where anyone can take them. Several also forbid feeding them to an AI model, which would rule out using them as Kimodo constraints. Move.ai keeps ownership of its output and forbids it as an AI input.

The research datasets with the best motion (LAFAN1, Bandai Namco, BONES-SEED) are non-commercial or bar public copies. Epic's Game Animation Sample is licensed for Unreal Engine only.

**Recommendation: own the motion.** Kimodo stays the generator: it makes new moves unattended, and it is the only source that makes the guard-keyed blows (D450). Capture we own becomes its reference and its constraints. That means an inertial suit, the developers as performers, and agents doing the clean-up and the checks.

- It captures what Kimodo does badly: idles and fidgets, a real sprint outdoors, hit reactions and deaths, carrying and tools, climbing on a wall, and swimming mimed on a bench.
- The data is ours, so it goes in the repository under MIT. It can be fed to Kimodo as keys, and a new move costs another session, not another purchase.

The gaps are filled in this order:

1. Quaternius's CC0 clips, for what is not yet captured (swimming today).
2. A small trial of Uthana's text-to-motion, whose paid output we own, wherever Kimodo fails.
3. A day at a capture studio, with the rights bought out, only for moves our performers cannot do (acrobatic rolls, skilled weapon play).

A private store library is the fallback if nobody will perform. It is the fastest way to cover the list, but it splits the build in two and cannot grow.

## The sources

Checked on each source's own site on 2026-10-26. Licence texts were read in full where the site served them. Two exceptions: Fab's full EULA sits behind a Cloudflare check, so its own licence summary was read instead; and the Mixamo FAQ refuses scripted fetches, so it is quoted from Adobe's community copy. Prices are in USD as listed today, several of them sale prices.

| Source | Kind | Covers of our list | Quality | Onto our skeleton | Price | Licence | Public repo, and ship on any engine? | Feed to Kimodo? | New moves later |
|---|---|---|---|---|---|---|---|---|---|
| **Own inertial suit** (Rokoko Smartsuit Pro II + Smartgloves; Xsens for more) | Capture | Everything a performer can do: idles, sprint outdoors, reactions, carry, tools, climbing on a wall; swimming only mimed | Real human motion; inertial drift and foot slide need clean-up (our foot locks and IK already exist) | Rokoko Studio exports FBX/BVH to many skeletons; then `aim.py` | Rokoko $2,068 (list $3,390), 30-day refund, unlimited FBX on the free Studio plan; Xsens by quote, about $4k–12k | Ours | **Yes** | Yes | **Yes**, a session each |
| **Kimodo** (v1.2, 2026-10-15; today's source) | Generator | Locomotion and combat; no fast sprint, no swimming, weak idles | Good gait; idles wooden | In use (`made_clips.py`, `aim.py`) | Free, runs here | NVIDIA Open Model, output unclaimed | **Yes** | It is Kimodo | **Yes**, unattended |
| **Quaternius UAL1 Pro + UAL2 Source** (v3.0, v2.2; owned) | Hand-keyed library | ~250 clips: idles, 8-way jog and sprint, swimming (tread, forward, dive), climbing and parkour, sword and shield fragments, punches, hits, deaths, roll, carry, farming | Cartoon: 28–41° lean in jog and sprint (D316) | Our skeleton is theirs | Paid $9.99 + $14.99 | CC0 1.0 | **Yes** | Yes | No, only what the pack has |
| **CMU Graphics Lab** (2003–) | Optical capture, 2,600 takes | Walks, runs, jumps, some fighting, ladders; no weapon sets or swimming | Noisy, old; no fingers | ASF/AMC/BVH; a bone map to ours | Free | "may be copied, modified, or redistributed without permission"; may go in commercial products, not resold; acknowledgement asked | **Yes** | Yes | No |
| **100STYLE** | Xsens capture, 4M frames at 60 fps | Locomotion in 100 styles: forward, back, sidestep, idle, transitions | Good locomotion; no fingers, 28 bones | BVH; a bone map | Free | CC BY 4.0 | **Yes**, credited | Yes | No |
| **Uthana** (paid plan) | Text and video to motion, plus a locomotion model | Unproven against our list; worth a bake-off for sprint, swimming and idle | Unmeasured | FBX/GLB; retargeting service | $0.02–0.10 per generated second | Paid plan: "you own all right, title, and interest in and to the Customer Outputs"; free plan CC BY 4.0, Uthana-owned | **Yes** | Yes | Yes |
| **Commissioned capture shoot** (optical studio, rights assigned) | Capture | Whatever the shot list says, acrobatics included | Best (optical, a professional performer) | FBX on a skeleton we name | $1,500–3,000 a day typical (vendor survey), plus performer and clean-up; ~3–5 days for ~450 clips | Ours if the contract assigns it | **Yes** | Yes | Another booking each time |
| **Fab packs** (Kubold's animsets, Motionbeats' 153-clip ledge climbing, "Swim Techniques" 70 clips, many sword and shield sets) | Capture or keyed packs | All of it, across many vendors and styles | Varies by vendor; mostly game-ready mocap, root motion and in place | UE4/UE5 mannequin: closest to ours | ~$30–150 a pack (ledge climbing from $64.99); ~$500–1,500 for the list | Fab Standard: "any compatible tools (usage is not limited to Unreal Engine)"; may share "via a private repository"; may not "redistribute the asset ... on a standalone basis" | **No**: private repo only, game ships binary | Per listing (sellers may mark no-AI) | No |
| **Unity Asset Store packs** | Same, Unity side | Same | Same | Unity Humanoid FBX, mostly fine | Similar | Unity: other engines allowed, but "Users of your project cannot access or extract the raw asset files" | **No** | Not addressed | No |
| **MoCap Online** (22 packs on its own store) | Capture | Locomotion, punches, deaths, ladder, ninja; **no sword and shield, no swimming** on its store today | Studio mocap | UE, Unity, Blender, FBX/BVH | $10–75 a pack, bundles $70–150 | Standard: "any single computer", under 1M users or $1M revenue, "Distribution is allowed only as binary-only", AI use needs written permission | **No** | **No**, without permission | No |
| **Kevin Iglesias** (Human Animations) | Hand-keyed packs | Basic motions, melee, archer, crafting, throwing, spellcasting; no swimming or climbing | Stylised, clean | Unity Humanoid / FBX | ~$9–49 each, Mega pack $130 | Own licence: not in "libraries, templates ... where the original files are accessible or extractable" | **No** | Not addressed | No |
| **ActorCore** (Reallusion) | Capture library | Large and cinematic: crowds, fights, a 5-clip swim pack; few game loops | Studio mocap | FBX via AccuRIG and iClone | $1.50–6.75 a clip, packs $49–149, 20 clips for $29 a month | Standard: export to any engine, use in games; redistribution and "AI training" need Enterprise | **No** | **No** | No |
| **Mixamo** (Adobe) | Capture library | Wide: sword and shield, great sword, swimming, ledge climbing, idles | Mixed; dated; inconsistent loops | Mixamo skeleton (other names), fingers | Free with an Adobe account | FAQ: may not "distribute the raw character and animation files"; no ML training | **No** | **No** (ML) | No |
| **Move.ai Move One** | Video capture (phone) | What we film | Good for markerless; one camera | FBX | $18–490 a month by credits (1–2 credits a second) | App EULA: Move AI owns the Output and licenses it to us; sublicensing needs a written agreement on the same terms; "Prohibited Use" includes "an input for any AI System" | **No** | **No** | Yes, but not ours |
| DeepMotion, Plask, Rokoko Vision | Monocular video capture | What we film | Below a suit; jitter and foot slide | FBX/BVH/GLB | Free tiers; DeepMotion's free tier is non-commercial | Not read in full; outputs' terms vary | Unconfirmed | Unconfirmed | Not ours unless the terms say so |
| RADiCAL | Video capture | — | — | — | — | Closed: bought by Autodesk, portal shut on 2026-07-06 | — | — | — |
| LAFAN1 (Ubisoft) | Optical, 4.6 h | Locomotion, falls, fights, obstacles | Very good | BVH | Free | CC BY-NC-ND 4.0 | **No** (non-commercial) | — | — |
| Bandai Namco Research Motion Dataset | Optical, 2 sets | Styled daily actions, fighting, locomotion | Very good | BVH | Free | CC BY-NC 4.0 | **No** | — | — |
| BONES-SEED (Bones Studio) | Optical, 142k motions, 288 h | Everyday motion (Kimodo's training data) | Very good | SOMA/BVH | Free to startups under $1M revenue | No public copies; no "generative model" that substitutes for it; ends at $1M revenue | **No** | **No** | — |
| Epic Game Animation Sample | Capture, 500+ clips | Locomotion with starts, stops, turns, pivots | Excellent | UE5 mannequin | Free | "UE-Only Content" | **No** (Unreal only) | — | — |

## What a private library would cost the workflow

There is a precedent. Quaternius's paid editions are kept out of git, each developer putting a copy in `art/animation/library/`, and only the clips cut from them reach `assets/models/character/mannequin.glb` (art/README.md). That works because the clips are CC0. A store pack's clips could not follow them into the repository, so a private library means:

- **A second store.** A private repository (Fab names it as allowed) mounted as a submodule or fetched by the seed. It holds the raw packs and the clips baked from them, mirrored ones included, since a retarget or a mirror is still the pack's motion. Every developer machine and every agent worktree needs the credentials.
- **Two builds.** The public repository has to build and play without it, on open fallback clips (Kimodo and Quaternius). Clips named in data (D136) have to resolve to whichever is present, and the clip checks (`moves.ts`, `gait_check.py`, `clip_check.py`) only judge the licensed clips where the private store is mounted.
- **Packed shipping.** Unity's "cannot access or extract the raw asset files" and MoCap Online's "binary-only" rule out the loose `.glb` files Bevy loads today. A shipped build would pack its assets into an archive.
- **No AI use.** MoCap Online, ActorCore, Mixamo and Move.ai forbid or gate it, so these clips cannot be Kimodo's constraints. Fab's terms depend on the listing.
- **No growth.** A new weapon class or move means finding another vendor's pack in another style, or going without.

The first two already bite: the Quaternius library is on no disk of this machine, so `clips.py` cannot be re-run here.

## Scaling and future-proofing

- **450–900 clips.** The generators scale: Kimodo already runs unattended, a minute a take (D450), and Uthana runs through an API. Capture costs performer time. With a suit, a session gives a few dozen good takes; the 450 are mostly combat, which Kimodo keeps making. A suit is needed only for the clips Kimodo does badly: idles, reactions, deaths, sprint, swimming, climbing, carrying and tools. That is perhaps 60–120 clips, a few sessions.
- **New moves.** Only a generator or our own capture makes a move that no pack has. Owning the capture also leaves one style across the whole set: the same performers and the same skeleton.
- **Kimodo constrained by capture.** A captured pose or path as full-body or end-effector keys works the way D450's guard keys do. Doing that with a purchased clip needs the vendor's AI permission, which most do not give. With our own capture it needs nobody's.

## Caveats

- The suit's quality on our checks (`gait_check.py`, foot slide, an idle a developer judges) is unmeasured. The 30-day refund is the trial: capture an idle, a sprint and a hit reaction, carry them by aim, measure them, and keep or return the suit.
- Inertial suits drift near steel, and a climbing wall's frame may be steel. Swimming in a suit is a dry-land mime. Neither is verified.
- Uthana's quality is unmeasured. A bake-off on sprint, swimming and idle against Kimodo costs a few dollars of credits.
- The studio day rates come from a mocap vendor's survey (MoCap Online's cost guide). Real quotes may differ.
- Fab's full EULA could not be fetched (Cloudflare), so the Fab row rests on Fab's own licence summary, which says it is not binding. Rokoko's Motion Library licence is not published outside its app. Treat both as private only until read.

## Sources

- Quaternius: quaternius.itch.io/universal-animation-library (v3.0, 2026-06-16; Standard free, Pro $9.99, Source $14.99; CC0 1.0) and /universal-animation-library-2 (v2.2, 2026-09-28; Source $14.99; CC0).
- CMU: mocap.cs.cmu.edu (front page and FAQ).
- 100STYLE: ianxmason.com/100style (CC BY 4.0). LAFAN1: github.com/ubisoft/ubisoft-laforge-animation-dataset, license.txt (CC BY-NC-ND 4.0). Bandai Namco: github.com/BandaiNamcoResearchInc/Bandai-Namco-Research-Motiondataset, dataset LICENSE (CC BY-NC 4.0). BONES-SEED: huggingface.co/datasets/bones-studio/seed, LICENSE.md; bones.studio/licensing.
- Fab: fab.com/eula (the Standard License summary; full text behind Cloudflare). Unity: support.unity.com, "Can I use assets from the Asset Store with other engines?", and unity.com/legal/as-terms. Epic Game Animation Sample: its Fab listing ("UE-Only Content") and CG Channel, 2024-06-12.
- MoCap Online: mocaponline.com/pages/license (Standard License) and the store's product list. Kevin Iglesias: keviniglesias.com (licence, updated 2026-10-15) and kevdev.itch.io. ActorCore: actorcore.reallusion.com and reallusion.com/license/content.html. Mixamo: Adobe's Mixamo FAQ, as posted on community.adobe.com.
- Uthana: uthana.com/pricing and uthana.com/terms. Move.ai: help.move.ai (Move One pricing) and move.ai/move-one-app-eula. RADiCAL: CG Channel, 2026-04-09. Rokoko: store.rokoko.com (Creator Bundle 2026). Xsens prices: CG Channel 2021 and MoCap Online's Xsens review (estimates, not quotes).
- Kimodo: github.com/nv-tlabs/kimodo CHANGELOG (2026-10-15, v1.2).

FOR REVIEW: question | Art, characters and sound | Where should the clips Kimodo makes badly (idle, sprint, swimming, reactions, climbing) come from? | (a) **Buy an inertial suit and capture our own**: Rokoko Smartsuit Pro II with Smartgloves, $2,068 (list $3,390), 30-day refund. A developer performs a few sessions, the agents clean and check the takes, the data is ours in the public repo, it feeds Kimodo as keys, and new moves cost only time. (b) **Commission a capture studio** with the rights assigned to us: about $1,500–3,000 a day, ~3–5 days for the list ($5k–15k with performer and clean-up). Best quality and ours, but each new move is another booking. (c) **Buy store packs** (Fab, Unity, MoCap Online): ~$500–1,500 for the whole list, the fastest. They can never sit in the public repo: a private repo, a second build with fallback clips, packed shipping, no Kimodo constraints for most, mixed styles, no new moves. (d) **Generators only**: add Uthana's paid text-to-motion beside Kimodo, ~$20–100 of credits, output ours, quality unproven. Recommended: (a), after a $20 bake-off of (d) on sprint, swimming and idle, with Quaternius's CC0 clips filling in meanwhile. | files: none
