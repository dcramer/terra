# How do others get good character animation when AI agents do the work?

2026-10-30. The developer: "look up how others are doing this shit using agents". This is a web survey only, and it informs docs/animation.md. Many sources are vendor pages or READMEs; where a claim is marketing, it says so.

## What others do

- **Nobody shows an agent making good-looking animation on its own.** The credible pattern is a split of the work:
  - real human motion is the source;
  - the agent does the plumbing (import, retarget, controller setup);
  - quality is judged by renders of the motion playing, plus numbers.
  - One practitioner (a Blender forum post, a single anecdote) found that sending a render back after every operation was the biggest quality gain.
  - A MindStudio guide says Claude with Blender MCP manages keyframes but fails at rigging and at timing.
- **The best agent-run pipeline found:** [mixamo-llm-mocap](https://skillsllm.com/skill/mixamo-llm-mocap) (MIT, README "written for agents").
  - Its steps: video, then a GVHMR pose estimate, then a retarget, then headless Blender, then a rendered review video with checks (limb angles against the video, foot slide, jitter).
  - The models it uses (GVHMR, SMPL-X) are research-licensed, so it is not usable for a shipped game. The lesson is its loop, not its parts.
- **"Fine alone, wrong in game" is a speed mismatch.** The standard fix is Epic's [pose warping](https://dev.epicgames.com/documentation/unreal-engine/pose-warping-in-unreal-engine):
  - the stride is scaled by the ratio of the movement speed to the clip's root speed;
  - the legs are turned toward the way the body travels;
  - foot IK runs after;
  - warping is skipped at low speed.
  That is the normal amount of procedural layering: a few whole-body nodes, not per-joint fix-ups. We had a clip made at 3.9 m/s played at 7.4.
- **Stills hide motion faults.** Several write-ups on AI sprite animation agree ([Gamelabs](https://gamelabstudio.co/blog/why-ai-cant-make-game-ready-spritesheets), [DEV](https://dev.to/gamelabstudio/ai-can-code-your-game-why-are-ai-characters-and-animations-still-such-a-mess-19ad)). Judge motion from the film playing and from frame strips, not single stills.

## Sources an agent can use

- **Uthana** ([pricing](https://uthana.com/pricing)):
  - pay as you go: text-to-motion $0.02-0.10/s, locomotion $0.02/s, video-to-motion $0.05/s;
  - GraphQL API and a Python client;
  - commercial use of paid output per its page; its Terms are not yet read.
  - We use it (D533). Our swims, made lying down, worked on it (2026-10-30).
- **Kimodo:**
  - trained on 700 h of mocap; the SOMA checkpoints are under NVIDIA's Open Model License (commercial use allowed);
  - foot skating is a listed limitation, cleaned up by default. Check that we never pass `--no-postprocess`.
- **Mixamo:** free for games, but its raw files can't be redistributed, which rules out our public repo. No API.
- **Cascadeur:** $19/month Indie. Physics-assisted keying, but no confirmed headless API, so it isn't agent-drivable.
- **Meshy and Tripo:** rig and animate APIs, with no independent quality tests found.
- **Not checked:** DeepMotion, Move.ai, Rokoko, Kinetix, Plask, Anything World.

## Motion-capture libraries we may ship

| Library | Licence | For us |
|---|---|---|
| [100STYLE](https://ianxmason.github.io/100style) | CC BY 4.0 (author's page; Zenodo field not confirmed) | Yes: captured locomotion in 100 styles, with credit |
| Quaternius UAL | CC0 (we bought Pro and Source) | Yes, but hand-keyed |
| CMU | Free, commercial products allowed, no resale | Redistribution in a public repo unclear |
| LAFAN1 (Ubisoft) | CC BY-NC-ND | No |
| Bandai Namco Research | CC BY-NC | No |
| Motorica | Research only | No |

## Read

- The fix is the method, not one more source:
  - clips from real or capture-trained motion;
  - the game's speeds matched to them, with one standard warp and foot IK;
  - review from the motion playing;
  - no per-joint fix-ups.
  docs/animation.md already says this. This survey adds the warp's standard form and the need to judge from motion, not stills.
- **100STYLE** is the one free, shippable source of captured locomotion found. It is worth a column in the stock-against-ours film once its licence is confirmed at the source.
