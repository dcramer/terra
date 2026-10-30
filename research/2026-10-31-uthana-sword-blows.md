# Uthana sword blows

2026-10-31. Question: can Uthana make sword blows that mesh with our fight, from text or from a video of a real person? Measured with `tools/characters/mesh.ts` on our body against `Low_Guard`, clip only: nothing was eased, driven, warped or tried in the game. $0.80 spent on text takes, nothing on video.

## In plain words

- **Text gets the stance and the feet right.** "A fighting stance with the left foot forward", seed 101, began within 1.6-4.5 cm of our guard's feet on six takes of seven. None of the 10 steps in the three picks dragged; Quaternius's dragged 14 of 18.
- **Text gets the body turn, not the blow.** The hips turn 53-158 degrees against ours 1-2. No take is one-handed, fast and forward at once, and no take is a backhand.
- **The seed sets the performance more than the words.** Seed 101 made much the same lunge under new wording.
- **Timing is the worst miss.** Hits land at 0.9-1.3 s against ours 0.2-0.6 s, so a take would play 1.5 to 4 times too fast. Asking for a 2 s take did not bring the hit forward.
- **No free video gave a real cut.** The usable footage is posed demonstration at 2-4 m/s in the hand, so nothing was sent to Uthana. A real cut on our body remains untested.

## Numbers

Feet cm left / right. Hand is the sword hand's fastest, m/s. Hips / shoulders is degrees turned. Hands apart is at the hit, metres. Bearing is the hit's direction off forward, + left. Hit is seconds to the hit, ours at the game's rate.

| Blow | Take | Start feet | Steps / dragged | Hand | Hips / shoulders | Hands apart | Bearing | Hit, rate needed |
|---|---|---|---|---|---|---|---|---|
| Slash | Ours | 0.1 / 0.2 | 0 | 3.1 | 2 / 22 | 0.33 | -38 | 0.33 |
| | Quaternius | 3.5 / 2.9 | 4 / 3 | 16.9 | 84 / 193 | 0.76 | -28 | 0.28 |
| | slash-1, two-handed lunge | 2.6 / 2.4 | 5 / 0 | 7.2 | 157 / 162 | 0.10 | +36 | 0.88, x2.7 |
| | slash-2s, stitched | 2.5 / 2.4 | 3 / 0 | 4.1 | 55 / 79 | 1.01 | +14 | 1.32, x4.0 |
| Finisher | Ours | 0.1 / 0.2 | 0 | 4.1 | 1 / 22 | 0.33 | -40 | 0.23 |
| | Quaternius | 24.5 / 23.3 | 3 / 0 | 13.8 | 119 / 137 | 1.07 | +29 | 0.36 |
| | finisher-1 | 3.1 / 2.0 | 4 / 0 | 7.0 | 68 / 88 | 0.12 | +31 | 0.91, x3.9 |
| Heavy | Ours | 0.1 / 0.2 | 0 | 4.4 | 1 / 28 | 0.35 | -22 | 0.58 |
| | Quaternius | 30.7 / 38.6 | 4 / 0 | 16.3 | 96 / 115 | 1.17 | +55 | 0.58 |
| | heavy-3 | 3.1 / 2.1 | 3 / 0 | 6.3 | 150 / 159 | 0.10 | +37 | 0.88, x1.5 |

The picks' worst standing slip is 3.6 cm. Takes not picked reach 16.2 cm (heavy-1's right foot), so Uthana's right-foot slide is not gone, only absent from the picks.

## The video

Hand speed was read by eye off full-rate strips, scaled from an assumed 1.8 m body, about ±1 m/s, across the picture only.

| Clip | Licence | Cut | Hand m/s |
|---|---|---|---|
| [Man with sword in winter forest](https://www.pexels.com/video/man-with-sword-in-winter-forest-10653785/) | Pexels | One hand | about 2 |
| | | Two hands | about 4 |
| [Man practising martial arts in VR headset](https://www.pexels.com/video/a-man-practicing-martial-arts-while-wearing-his-virtual-reality-headset-8097548/) | Pexels | One hand | about 2-3 |

On the winter cut the tip moves about 4 times the hand: the blade turns about 450 degrees a second and the tip reaches about 8 m/s. Eight other clips failed on framing, licence or content. The Commons sweep was partial and Pixabay was judged by titles.

## What this does not show

- **The hand-speed bar is unproven for a sword.** The 8 m/s bar comes from a batter's hands, 9-11 m/s (research/2026-10-30-why-the-fight-clips-look-bad.md). No footage of a hard sword cut has been measured. These clips are slow because they are posed, so they neither confirm nor sink it, and Quaternius's 14-23 m/s may be faster than any person. A blade's speed may be the better measure.
- **No take has been in the game.** Quaternius's looked fine until it was.
- **Uthana cannot start from our guard clip.** `create_motion_from_gltf` takes it free, but Uthana's retarget fails on it (`u_naive_retarget`), also inside `stitch`. A stance made by text and stitched in does work.

## Bought packs, as a next source

Not bought. GameDev Hero's [Sword and Shield Animations V1](https://gamedev-hero.itch.io/sword-and-shield-animations-v1) lists 287 animations and [V3](https://gamedev-hero.itch.io/sword-and-shield-animation-v3-uefbx) 355, each $18.99 at the time read, counting in-place and root-motion copies, with FBX files. V1 lists stages for joining attacks into combos. Neither page says whether it is capture or hand-keyed, and neither states a licence, so nothing says its clips may sit in this public repository. Ask the seller first, and read a pack through mesh.ts before any tuning.
