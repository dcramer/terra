# Uthana CLI

Makes animation clips from text with [Uthana](https://uthana.com), retargeted onto our body, for the clips Kimodo makes badly: idles, sprint, swimming, reactions, climbing (D532). TypeScript, run directly by Node 22.18 or later, with no dependencies.

## Setup

1. Make an account at <https://uthana.com> and add credits (pay as you go; turn auto top-up off).
2. Copy the API key from the account settings.
3. Put it in `.env` at the repository root (worktrees read the main checkout's): `UTHANA_API_KEY=<key>`.

Check it with `node tools/uthana/uthana.ts org`.

## Use

```bash
node tools/uthana/uthana.ts motion "a man stands at ease, weight shifting, arms loose at his sides" -o idle.glb --length 6
```

The first `motion` uploads `assets/models/character/base.glb` (or `--body`) as a character, without its animations (its id kept in `~/.cache/terra/uthana.json`, uploaded again when the body changes), so every clip comes on our skeleton. The clip is saved motion only, at 30 fps. A clip is judged by the character tools' checks ([tools/characters](../characters/README.md)) and the workbench, as a Kimodo take is. Each `motion` prints one line of JSON with the motion, the character, the dollars spent and the file.

| Model | `--model` | Cost (2026-10-26) | Notes |
|---|---|---|---|
| Text-to-motion 1.0 | `1` | $0.02 a second | |
| Text-to-motion 2.0 | `2` (default) | $0.03 a second | `--seed`, `--foot-ik` |
| Text-to-motion 3.0 | `3` | $0.10 a second | 4–10 s, a job polled every 5 s; unlocked by asking Uthana's support |

`video clip.mp4 -o clip.glb` makes a clip from a video of one person (video-to-motion 2.0, $0.05 a second of video sent, 2-60 s, .mp4/.mov/.avi), on our body and checked as `motion`'s are. Trim the video to the stretch wanted first (`ffmpeg -ss -t`): it refuses one costing over `--most` dollars (0.5). Uthana asks for a still camera, the whole body in frame and one person. `--playback 2` downloads a video filmed slowed twice (50 a second shown at 25) at its real speed; the take's speed comes from the video's clock, so know it. A file sent again within a day returns the same job, uncharged.
A walk or run comes from Uthana's locomotion instead ($0.02 a second, 2026-10-29): `loco --speed 4.3 --strides 5 --style neutral_male_a -o jog.glb` makes 1 to 5 stride pairs straight ahead at that speed, its last frame its first; `styles` lists the styles (the names of the 100STYLE capture set: `neutral_male_a`, `neutral_female_a`, `proud`, `elderly`...). In `art/animation/made.json` it is an entry with `"model": "loco"`, its `speed`, `strides` and `style` (or a list, one a take), made by `motion.ts` as any Uthana entry; `made_clips.py` cuts one stride from it.

`--in-place` takes out the root's horizontal motion. `--loop 1` makes the clip a closed loop, its end eased into its start over the last second (Uthana's looping, no charge seen in the trial); `--from <motion>` loops one already made. A text-to-motion clip is not a loop until then: the trial's ended 19–67 cm off their start. `schema` lists the arguments the live API takes, for when the docs and the API disagree.

`stitch <id[:from-to]> <id[:from-to]> --over s -o clip.glb` joins two motions by Uthana's transition model ($0.01 a second of what comes out, `stitch`, transition 0.1-3 s), checked as `motion`'s are, the way to start a take from another take's stance. The motions must be Uthana's own: a clip of ours baked in by `create_motion_from_gltf` is taken, free, and downloads on our body, but Uthana cannot retarget it, to its reference or into a stitch (`u_naive_retarget failed`, 2026-10-30, in our file's shape and in a downloaded take's).

In text-to-motion 2.0 the `--seed` sets the performance more than the words do: on sword blows the same seed with other words gave the same lunge, so vary the seed as well as the wording.

## Rights

Clips made with paid credits are ours ("you own all right, title, and interest in and to the Customer Outputs generated under such Paid Plan", uthana.com/terms), so they may sit in the repository and steer Kimodo. Free-plan clips are Uthana's under CC BY 4.0: don't use the free plan. Uthana may train on what we make.

## On our body

A take is carried onto our body inside `download` (`carry.ts`, D533), so `motion` and `motion.ts <clip>` give a take ready for `made_clips.py`. The body goes up without its animations: with them, Uthana read our skeleton wrongly (the trial idle 14.5 times too low and 30° over). Each take gets the body's skin (`withSkin`, so Blender reads it as our armature) and is raised by our rest forefoot height less Manny's (`lift`, 3.1 cm: Uthana keeps the forefeet at Manny's height). Then it is downloaded again on Uthana's Manny (free) and refused unless it stands as Manny does (`against`: hips, trunk, feet), kept beside the output as `.refused.glb`. Each call's cost is the fall in the account's credit (`org` prints `payg_total_usd`); looping is charged too, $0.01 a second.
