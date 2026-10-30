# How a body is animated: the studio method, on our budget

The developer, 2026-10-30: "stop fucking it up and get it right. we need to do this like the big boy game studios do things, but like.. without their equipment and budget and only using llms and cheap shit".

This is the method every job that changes how a body moves follows. D584 holds the rule for layers; this doc holds the method and the order of work. Status: proposal, for the developers to read.

## What went wrong

- **Numbers instead of eyes.** We judged motion by scorecard rows. Each fix passed its row while the look got worse. Nobody put a known-good animation beside ours until 2026-10-29.
- **Patching instead of replacing.** When a clip looked wrong, a layer was added to twist joints over it. Twelve layers built up and fought each other (research/2026-10-29-why-the-skeleton-moves-wrong.md).
- **Speeds never matched.** The sprint clip was made at 3.9 m/s and the game runs at 7.4 m/s, so the game sped it up and stretched the stride, and the legs bent.
- **Clips made for something else.** A swim was mimed standing and then tipped over.
- **Many narrow jobs, no owner.** Agents worked in parallel, each tuning one number, and none of them owned the whole look.

## How studios do it, and what we use instead

| Studio | Us |
|---|---|
| Motion-capture stage, actors | Quaternius's packs we bought (UAL1 Pro, UAL2 Source) first; Uthana's text-to-motion (trained on capture) for what they lack; Kimodo only for what neither has (D532) |
| Animators cleaning up capture | Agents with Blender scripts (tools/characters), retaking rather than patching |
| One skeleton, retargeting | Ours: carried onto every body by D542, measured faithful to 0.0° |
| Animation director, daily reviews | The developer, by eye, on the review desk: a before/after film from the game's own camera for every change |
| Animation graph with blend spaces | figure.rs's states, blending clips by speed and direction |
| Motion matching (Ubisoft, Unreal 5) | Not first: blend spaces need a handful of clips and come first. Motion matching is possible later; Kansei ran it in Rust on 7-25 minutes of generated clips (research/2026-10-30-agent-driven-animation-deep.md). |
| Post-process: foot IK, look-at, hand IK | The same three layers and nothing else (D584) |

## The method

1. **The speed comes from the clip** (D585, held by `terra_walker::pace::matched` and its tests). Every moving clip is made at the speed the game moves the body, or the game moves the body at the clip's own speed.
   - Small mismatches are covered by playing a little faster or slower, at most 10%. Stretching the stride, at most 10% (Unreal's stride warping keeps to this range).
   - A gait that needs more gets its own clip: walk, jog, sprint, each at its own speed, blended by speed.
   - A clip plays at its take's own frame rate (`tools/characters/clip_rate.test.ts`): Kimodo's 30 fps strides keyed as Blender's 24 fps frames played at 0.8 of their speed until 2026-10-29.
   - Each clip's speed is measured from its planted foot. Where the clamp can't reach the game's speed, change the game's speed or make a new clip; never stretch past it (others did the same: project-whiskeyjack moved 4.0 to 4.6 m/s rather than stretch).
   - All gaits share one stride clock (sync groups, phase-locked at the same foot's touchdown), so blending two gaits never crosses the feet.
   - Which speeds the game uses is a design call. A film shows what each choice looks like.
2. **A clip is made for what it shows.** A swim is made lying in the water; a carry made carrying; a blow made striking. Nothing stands in for something else.
3. **A wrong clip is replaced or retaken, never patched.** No layer, offline script or fix-up bends a clip to hide a fault. Wrist limits, reach solvers and formula arms are retired as their clips are replaced.
4. **Holding is an overlay from a clip.** Each way of holding (carry a tool, carry a long tool, guard with sword and shield) is a pose for the upper body, taken from a clip. It is laid over the locomotion from the spine up by a bone mask (holding.md section 2), as Lyra and ALS do.
   - The item sits in the hand by its grip point (D566), placed once per kind of item, never per item.
   - The hand stays on a two-handed handle by hand IK. Fingers close on it by a grip pose.
5. **Only three layers after the clip:** feet on uneven ground, the head looking at the aim, and hands on what they hold (D584). The bone audit holds them to that (`node tools/controller/bones.ts`).
6. **Every change is reviewed by eye.** A change to how a body moves lands only with a before/after film from the game's own camera (`tools/controller/layers.ts` is the pattern). It goes on the desk if it changes the look.
   - Numbers are guard rails (foot sliding, the bone audit, nothing through the body), not the goal. A row passing never stands in for a look approved.
7. **One owner at a time.** One job at a time changes how bodies move, working through the order below. Jobs on unrelated things run alongside it, but not on the same bodies.

## Order of work

Each step ends with a film on the desk and the developer's yes before the next starts.

1. **Stock against ours.** The film is being made now: our locomotion as shipped beside the bought clips played as made, each at its own speed, with only the three layers. The developer picks.
2. **The locomotion set at matched speeds:** idle, walk, jog, sprint, from the pick in step 1.
   - The game's speeds are set to the clips', or clips are retaken at the game's speeds (a design call, from the film).
   - Then starts, stops and turns, from the same source.
3. **Carry overlays:** a pose from a clip for each way of holding. They replace carry_hand and arms (the per-frame twists), which are deleted.
4. **Swims:** done 2026-10-30 (Uthana, lying in the water). Open faults: the quiver shows skin in the dive, and the tread reads as a squat.
5. **The fight set:** each blow made striking, from the bought packs where they have it, Uthana otherwise. Hand IK only, by the same ladder (raw, carried, in game).
6. **Then attachments and assembled items resume** (D565), on bodies that move right.

## What we stop doing

- Tuning a scorecard row to judge a look.
- Adding a layer to cover a clip.
- Several jobs tuning the same bodies at once.
- Claiming a fix without a film from the game.
