# Is building fluid? What the QA films found (2026-10-14)

The question: as a base is built, changed and knocked down, does everything on screen adjust when it should, with nothing popping, blinking, floating or hitching? Measured with the QA films (D308, `cargo xtask qa`, [tools/sandbox](../tools/sandbox/README.md#scripts)): a cabin built as a player builds one, a destruction scenario, materials, shapes and the castle, each in a set of kits, filmed frame by frame with every section laid before a frame is drawn, and the same calls run in real time on the RTX 5090 with every frame timed and each change's time to the screen.

## How the films were read

Each film was read frame by frame where things change (contact sheets, and runs of consecutive frames cut from the video), against the trace. A pixel measure of flicker, each frame against its neighbours as `pops.ts` does, was tried and dropped: with the camera always moving, grass in the wind and the lake's glitter scored as high as a planted 30-pixel blink (0.6% of the frame against 0.64%). Blinks are instead counted where they come from: the drawing says when it shows a change before all of it is back (`torn`), and the debris when it reaches the screen.

## Found and fixed

| What | Seen as | Cause | Fixed |
|---|---|---|---|
| A collapse left nothing behind | A tower knocked down vanished in a frame; no rubble anywhere | Since D290 debris drew in a kit's own `StandardMaterial`, no longer made, so every pile's meshes were dropped | Piles drawn in the texture arrays as sections are, their layers kept resident while they lie (D305) |
| Rubble fell as one sheet | Once drawn, a tower's rubble hung as a flat carpet at its top, then dropped together | Every pile started from the highest layer that fell into it | Each starts from a share of that height by where it lies, tumbling square as it lands: it rains down through the height that stood |
| Changes shown torn | 14 changes (log cabin) and 44 (stone house) shown with a section before the rest of their change, for a frame a piece twice or missing at a section edge | A build's sections were shown together only once nothing of it was on a worker; built a step every few frames, that never happened, and after 30 frames they were shown as they were | A change older than the newest is shown once it and every older one are back; held counts frames since a whole change was shown. 0 torn since in every set |
| A changed pile blinked out | A pile more fell onto vanished for the frames its new mesh took on a worker | Despawned when noticed, drawn again when made | The old pile stays until the new one is put; scripted runs make piles at once, as they lay sections |
| Stairs down castle walls | Every window in the castle's curtain and every hole carved in it planned a flight of ghost steps to the ground | A doorway was any opening with a floor beside its sill; a hole in a wall thicker than it has wall beside its sill | A doorway's sill leads, through the opening, into a room, not into the wall (D261) |
| The hammer aimed at the middle of the picture | A cyan cube outlined wherever the film's camera looked | The hammer took the camera held by a script for the player's view | A held camera aims no tool, as a fixed one already did |
| Doors and shutters short of their holes | A strip of light down a stone doorway's jambs and over its head, and over and under shutters | A leaf was sized to fit inside a log wall's frame boards, 5 cm in from each side and the head, whatever stood round the hole | Each leaf fills its opening and reaches 3 cm behind its jambs, head and sill, as a real one closes against its stop; its model, made for the clear opening, is stretched to it. A test measures no gap round any leaf of any hole (D225) |
| A placed claim stone could not be scripted | `storage.place` refused a claim stone outside a claim | The claim check did not know a claim stone makes one | A claim stone goes down as the hammer sets it, if the player has none and it keeps clear of others |

## Measured, not fixed (others' areas)

- **A castle edit takes 120 to 150 ms of the main thread** (`cargo xtask qa castle`): a profile (Bevy's `trace_chrome`) puts about 100 ms in `build::light::place`, which floods the whole base for its rooms (`Base::indoors`) after every change; 30 to 60 ms in finding what goes in next (`next_work`); 15 to 25 ms in `store::notice`. Each carve or block is a 100 ms hitch, and the tower brought down a 180 ms frame. The cabin's edits cost under 2 ms each.
- **Time from an edit to the screen** is 15 to 30 ms for a wall's course, but 50 to 200 ms in the storey and roof stages, where one section's near detail takes up to 70 ms to merge on a worker (`Build: change on screen` breaks it down); the castle's edits 60 to 90 ms, the tower's fall 500 to 740 ms.
- **Building everything at once** (`build.fill` of a few thousand steps in one call) takes several seconds on the main thread: what goes in next is found afresh each step.

## Partial states, frame by frame

The developers saw "some weird partial thing" in the first films. Each partial state was cut from the films and checked against a live run replaying the same calls (`build.state` beside the picture):

- **Walls rising** go in course by course across every wall, the lowest layer first (`next_work`), a stone wall's course a cube at a time from the nearest, so at most one course stands higher on the near side than the far while it is laid; a log wall's courses cross at the corners as they rise. As a real course is laid. Kept.
- **Roofs** go in a row at a time from the eaves, each row through its frame, battens, covering and trims before the next. Kept.
- **The loose blocks beside the footing** were the film's own: single cubes sunk a metre into falling ground and a cut block, which read as rubble. The film now lays a garden wall in metre boxes, a course below the ground and one over it, and a post at each end.
- **A wall changing material** came out course by course from the top, leaving the roof over ghosts, then went in again from the bottom. The film now changes to a material of the wall's own pattern, which repaints in place at once, as Enshrouded's swap does (D298); a change of pattern still rebuilds.
- **Door steps** leave a thin wedge of their last stair cube above falling ground in front of the bottom step: a stair cube's lower tread mostly buried. Not fixed (the kits' and stairs' drawing).

## Found, for the developers

- **A knocked-out corner of a log wall cannot be built again** while the logs over it stay up: "Something is built over it". Stone can; logs, as laid, go in from below.
- **A plan left in the air**: a planned cube never built stays as a ghost where it was after what held it collapses.
- **Damage reads as a hole**: a wall hit to a fifth of its health draws nearly black, as D305 says vertex colour does on a flat block surface.
- **A storey laid over a built roof** builds its walls above the roof, which stays inside the storey; the film takes the roof off first, as a player would.

## What did not need fixing

Neighbours (corners, quoins, copings, trims, eaves) change in the same frame as the piece that changes them, by construction: a change's sections are swapped in together. Ghosts sort and draw cleanly over built work; roofs build a row at a time through their four stages without a gap. Easing pieces in was not added: Valheim and Enshrouded put a piece in at once with a puff and a sound, and our pieces go in course by course already, so it would read as slower rather than smoother.
