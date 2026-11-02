# Moves keyed from a pose sheet

The developer, 2026-11-01: "I have found using chatgpt to create a pose sheet of the animation sequence, and then I get opus to animate against the pose sheet reasonably effective. lets try this", and then "lets start by documenting that process. we're not going to implement it all right now."

An image model draws a move as a strip of poses. An agent reads the strip and writes those poses on our own skeleton as a short text file. A tool turns the file into a clip. No motion generator sits in between, so the same file always gives the same clip.

Status: proposal (D588, Open). One blow has been made this way, the slash, as a spike ([the note](../research/2026-11-01-pose-sheet-blow.md)); nothing in the game uses it. This doc says how the process runs and what it still lacks. [animation.md](animation.md) stays the method every clip follows: the film card, a wrong clip replaced and never patched, the developer's eye as the judge.

## Where it sits in the pipeline

It is the proposed source for one row of [animation.md's table of where a clip comes from](animation.md#where-a-clip-comes-from). Every doc an agent follows to make a clip points here, so nobody starts a blow without seeing it.

If the developers take it up (D588 Decided), these change in one commit, the word "proposed" goes, and the row's Kimodo route is deleted (clean as you go):

- [animation.md](animation.md): the table's row and step 5 of the order of work
- [movement.md](movement.md): the pipeline's Clip step, the skeleton line and what a new move is
- [assembled-items.md](assembled-items.md): how a class's clips are made
- [tools/characters/README.md](../tools/characters/README.md): Made clips
- the `combat` and `character` skills, and D450, D532 and D565 where they name the source

If they don't, those lines and this doc are deleted, with the spike.

## What it is for

Moves with a clear start and end that no source does well: blows first, then a tool's swing, a shield bash, a drink, a way of holding. It is how game animators work: block the key poses, then time them. The sheet stands in for their thumbnail sketches.

It is not for walking, running or swimming. Those are long cycles that bought clips already do well, and a person keying them by hand is slow in any studio.

A small loop such as an idle goes through the same steps with less from the sheet: its panels barely differ, so the sheet gives the idea and the keys give the numbers ([the try](../research/2026-11-01-pose-sheet-blow.md#the-idle-one-flow)). It starts on the clip's own first pose, its last key equals its first, and its speed runs on through the seam (`loop`). Its empty arms hang from the chest (`hang`). `mesh.ts` is for blows; a loop's gates are the seam, the feet and `moves.ts`'s `hands` row, with `--also` naming the clip.

## Why it should hold up

- **It starts where the game is.** A move begins and ends on the guard exactly, because those two keys are the guard.
- **Feet can't slide.** A foot not told to move stays where it is, to the frame.
- **The hit is written down.** Its moment and place are numbers in the file, so nothing is measured afterwards and hoped for.
- **A fix is a changed number.** A wrong pose is one line in a short file, not a retake from a generator.

## The process

One move, one owner. Each step says what stops the job.

1. **Brief the move.** Say what it does in the fight: where it starts and ends, the hit's moment, how far the body travels, which foot steps, and which reference item it is made for (D565).
2. **Look at the start pose before anything else.** A keyed move inherits its start pose's faults: today's idle had its palms out and arms wide, and the keyed idle copied both until the developer saw them. Judge the start pose first, and name in the brief what must differ.

   **Render the start.** Our body in its start pose, from two sides, with the item in hand (`tools/characters/clip_sheet.ts`).
3. **Make the sheet.** `node tools/tripo/tripo.ts image "…" --ref guard.png --model gpt-image-2.5 --aspect 16:9`. The prompt asks for:
   - one strip of eight numbered panels, the first and last the start pose;
   - the same camera and scale in every panel, the reference's own view, the whole body and whole item always in frame;
   - a plain grey ground, clear outlines, no blur or effects;
   - each panel's pose in a sentence, and which feet stay planted.

   The brief decides how good the sheet is. On the idle, a vague prompt gave twelve near-identical panels; asking for one row of five, each panel's difference spelled out, and the hands by name gave poses worth keying. Fewer panels work for a small move.

   Reject a sheet whose body can't do what it shows, or whose item changes between panels. Draw a new one; don't argue with it. The image model may ignore the layout: read what it drew by the poses, not its panel numbers.
4. **Write the keys.** One key per panel, more where the hand travels a long arc. Copy the body from the panels. Timing is ours, since a sheet has none: a slow wind-up, a fast strike, a slow recovery, set against the move's row. Correct what the sheet gets wrong, such as a blade that points the wrong way in the wind-up. Read the sheet as an animator would, and don't trace it.
5. **Lay it under the sheet.** Render ours at the panels' moments from the same camera, stack it below the sheet, and correct by eye. Three rounds at most; past that, the sheet or the brief is wrong, so go back to step 1 or 3.
6. **Pass the gates**, before any film:
   - `tools/characters/mesh.ts`: the start, the feet, the speed and the hit's moment against the guard;
   - `tools/characters/moves.ts`: the wrist, the grip, the sword through its own shield, and hands into the body on every body template;
   - `spike/pose-sheet/rest.ts`, for empty hands: palms not forward, arms near the sides, against `Walk_Loop`.

   `mesh.ts` and the `held` rows are for blows; an idle doesn't use them.

   A clip put into a body file keeps the old clip's foot locks until they are baked again: `glb_clips.ts` doesn't carry them, so run `tools/characters/markers.ts` on the file.
7. **Film it twice.** The clip on our body from two sides, then the real game's duel (`cargo xtask gpu film -- cargo xtask duel --out DIR`). A clip that looks good alone has fooled us before.
8. **Put it on the desk.** The sheet above ours, and both films. The developer's eye decides. Nothing is tuned on a scorecard row to pass it.
9. **Keep it.** The sheet and the keys are the source, in the repository beside the set, and the clip is rebuilt from them.

## What exists

- `spike/pose-sheet/key.ts`: keys to clip, for the player's body and one guard. It sets the hips, chest, head, shoulders, sword hand and its blade, the shield's offset, and each foot and heel, and solves the arms and legs to reach them.
- `spike/pose-sheet/stills.py`: the stills for step 5.
- `spike/pose-sheet/slash.json` and `slash-sheet.jpg`: the worked example.
- `spike/pose-sheet/idle.json` and `idle-sheet.jpg`: a loop, with `loop` and `hang`.

## What it still lacks

In the order to build them, once the developers say go:

1. **The step.** The test blow stands still, so it lands less often. Travel has to be keyed: the lead foot steps, the body moves with it, and the game reads the travel from the clip instead of a hand-set `drive`. Getting close enough to hit, without the feet sliding, is the biggest open question: the bought blows slid because the game moved the body over feet the clip kept still.
2. **The gates.** Run `moves.ts` on a keyed clip and fix what it finds. The wrist and the sword through the shield have not been checked.
3. **The rest of the set.** The backhand, finisher and heavy, then a chain judged as a whole.
4. **A real tool.** `key.ts` moves to `tools/characters`, with a test, and joins the incremental build (`tools/characters/build.ts`) so a clip remakes itself when its keys change. It must take any start pose, both hands on one item, and any item class's reference.
5. **Other bodies.** Keys are places in metres, so a clip might be solved again for each body template instead of carried across. Untried.

## What could still go wrong

- The look may not hold up at game speed or in a long fight. Only the developer's eye can say.
- A second sheet of the same move will differ, so a set needs one style to read as one fighter. Kansei keeps a style prompt shared across a pack; do the same.
- Fine motion, such as a hand's small adjustments and the body settling, doesn't come from eight poses. A move may read stiff beside a bought clip, and it needs a look in a chain beside them.
- Reading depth from a flat picture is a judgement call, so a pose can look right from the sheet's camera and be wrong from another. The second camera in the film is there to catch that.
