# The shield in a blow, frame by frame: what floats in ours, and where shipped games and historical fighters carry it

2026-10-25, combat round 19. Question: the developers, on round 16's layered hold (the shield arm held at Keyed_Guard's pose with `upperarm_l` turned +90° through each blow): "it looks better but the shield is floating in some and theres still odd behaviors. you probably need to look up and research more prior art here". Which faults are they, what does prior art do instead, and how is each measured? Builds on [the shield arm in a swing](2026-10-25-shield-arm-in-a-swing.md) (round 15), and corrects its reading of Enshrouded.

## Answer

Four faults, each now a row of `tools/combat/criteria.json` (`?` until `moves.ts` measures it):

1. **The chain pop** (`move.chain.shield_round_trip`): as the next blow of the light chain starts, the shield swings back in toward the body and out again in about 0.13 s. The outgoing blow holds it out. The incoming clip's hold starts at weight 0 (`in` from clip time 0), so the 110 ms move blend (`melee::show`) carries the board toward the keyed clip's own guard arm and the `in` ease carries it back out.
2. **The sail** (`move.blows.shield_high`, `shield_elbow`, `shield_face`): Keyed_Guard's raised forearm turned 90° at the shoulder points the forearm straight up. The board then stands above the shoulder, its top rim over the head. The upper arm is level and the shoulder is at the end of its outward turn (a waiter's tray), and from behind the board shows its painted face. No reference holds the shield above the shoulder unless it is blocking.
3. **The grip** (`held.shield.grip_hand`, `grip_across_palm`): the round shield is a centre-grip shield (a boss, a bar behind it), but it rides `lowerarm_l` like a strapped shield. Its grip bar runs along the forearm, where a fist cannot close on it. The board stands off the forearm and nothing visibly holds it. That is the "floating" read whenever the forearm hides behind the board.
4. **The jump at other move changes** (`film.shield_round_trip`): after the parried heavy, the stagger takes the shield from held out to down by the hip in about 0.1 s (record frames 458-464).

Prior art agrees on one carriage. When not blocking, the shield arm stays low (the board's middle at the hip to the chest) and close (the elbow by the side), on its own side, through the wind-up, contact, recovery and between chained blows. It rises only to block. A centre-grip shield or buckler is held in the fist, its bar across the palm. A heater rides the forearm on its straps, the forearm against the board.

## Our film, named

The duel film (record frames at 60 a second, film time = (frame − 20)/60 s), round 18's set: `~/.cache/terra-review/duel/duel-r18-burst-shipped.mp4`. The moves are unchanged since round 16, so round 17's film shows the same poses. Cameras: side (square to the line) and three-quarter (behind the player's right shoulder).

| Move | Record frames | Camera | What is wrong |
|---|---|---|---|
| Slash to backhand (chain) | 296 → 300 → 304-306 | three-quarter | The shield is out at the left, edge-on (296). It swings in behind the shoulder, face to the camera (300), and out again (304-306). The finisher's start is worse: the board swings in behind the back (340-346) and is out again by 350-356, about 0.2 s. |
| Partner's slash | 200-258 (wind-up to recovery) | side | The sail: the forearm points up and the board stands over the left shoulder, its top rim above the head, edge toward the foe. The upper arm is level, and the hand is hidden behind the board. |
| Player's heavy | 436-454 | three-quarter | The same sail: the board goes up and out to the upper left, over the shoulder line. |
| Player's slash | 272-296 | three-quarter | The arm is held out like a wing, the board at arm's length to the left. Its painted front faces back toward the camera, so the player sees the shield's face from behind. |
| Parried heavy, stagger | 458 → 464 | three-quarter | The board jumps from up-left to down by the hip, face down, in 6 frames. |
| Partner's guard after its slash | 274-290 | side | The board stands face-on to the side camera, side-on to the foe. This is the existing `move.guard.shield_face` (`?`). |
| Block | 189-261 | both | Raised across the face as specified. The sword through it is the existing `move.block.sword_in_shield` (19.1 cm). |

The torso does turn with the cut (Kimodo's keyed body), so "the torso not turning" is not a fault seen here.

### How the shield is attached

`models/held/round_shield.glb` is fitted by `tools/characters/fit_held.py` as a **centre-grip** shield (TEXTURED: "held in the left hand across its planks just behind the board, the knuckles up into the boss ... the forearm square to the board"). Its file also says `strap`: `{"bone": "lowerarm_l", "at": [-0.02, 0.34, -0.065], "face": [0, 0, -1], "grip": [0, 1, 0]}` (STRAPS). `terra_walker::strap` and `moves.ts` hang it from the forearm bone, not the hand. As a result:

- the board's plane runs along the forearm, as an enarmes shield's does, but there are no straps on the model;
- the grip bar (the look's +Y) runs along the bone, so a fist cannot close on it: a fist closes on a bar across the palm;
- the hand is not tied to the grip. The wrist's own rotation moves the fist off the boss, and the hand socket's pose does not move the board;
- the grip sits 6.8 cm off the bone's line and the board's back 2.5 cm further out, so the forearm (3-4.5 cm radius) clears the board by roughly 4-6 cm along its length (from the strap's numbers, not measured).

So "floating" is the socket and the pose together, not the model. The model is a sound centre-grip shield. The socket wears it as a strapped one, and the hold's sail pose hides the forearm, which is all that ties the board to the body. The strap is D459's (Agent, Leaning, technical): this note is the evidence to revisit it (Targets).

## Prior art, phase by phase

| Source | Grip | Guard | Wind-up | Contact | Recovery and chained blows | Block | Confidence |
|---|---|---|---|---|---|---|---|
| **Enshrouded**, Update 8 light chain (FtP 244.90-247.20 s, read frame by frame from `~/.cache/terra-review/refs/enshrouded-combat/ftp/warp/`) | A small heater-shaped shield on the forearm | At the left hip, the forearm forward (244.90, from behind) | The sword up at the right; the board at the left side at the waist, a hand from the hip, face forward and out (245.80) | Unchanged at the left hip, the sword out level (246.20) | Stays at the left side at waist to lower-chest height through every chained blow (245.40-247.20). It moves tens of centimetres with the torso's turn, never above the chest and never behind the body | 2023 build: a rectangular shield raised before the face, the forearm upright behind it (cave 22.50 s) | High on where the board is (60 fps, clear poses); medium on the grip |
| **Viking round shield** ([Hurstwic](https://www.hurstwic.org/history/articles/manufacturing/text/viking_shields.htm), sourced; reenactment photo, [Commons](https://commons.wikimedia.org/wiki/File:Vikings_(6096147677).jpg), CC BY-SA 2.0) | A centre grip: the fist on a wooden bar behind the boss, "the arm did not slip through any straps", so the board turns freely in the hand | "Held forward, but not in front of, the combatant", at an angle, touching the hand, arm and shoulder; a high ward with the hand by the face | Not described | Not described; a flat-of-the-shield parry, then the thrust | Moved by "small motions of the arm and hip"; binds sweep it across the front | The flat or the high ward; edge punches | Sourced for grip and carriage; strike phases not described |
| **I.33**, sword and buckler, c.1300 ([ARMA guards essay](https://thearma.org/essays/I33-guards-footwork.html); folios 1r and 5r, [Commons](https://commons.wikimedia.org/wiki/Category:Royal_Armouries_Ms._I.33), public domain) | A fist-held buckler | Forward, the arm extended, beside or over the sword hand ("held extended to the front", "covering the hilt") | Forward at the chest, the sword up behind (fol. 1r) | "The left wrist is pulled back ... to flatten the buckler along the left arm ... the right hand ... pass[es] directly underneath the buckler, which protects the sword hand during the cut" | With the hilt; "moved up and down and back and forth across the body"; never out to the side away from the sword | The buckler is the guard of the hand | Sourced (a modern reading of the manuscript) |
| **Heater with enarmes** ([Strapped shield](https://en.wikipedia.org/wiki/Strapped_shield), [Heater shield](https://en.wikipedia.org/wiki/Heater_shield); Morgan Bible c.1250, [Commons](https://commons.wikimedia.org/wiki/File:Leaf_from_the_Morgan_Picture_Bible_-_Battle_detail.jpg), public domain) | Straps on the forearm plus a hand strap; a guige sling round the neck. Wikipedia disputes arm-strapping for heaters (hand strap and guige only) | Close on the forearm before the left side | Riders cut overhead with the shield held close at the left front (the Morgan Bible) | The same | The same | The board before the body | Medium: the sources disagree on the strap |
| **Elden Ring / Dark Souls** | In the fist | The shield low at the left hip or thigh, face forward-left | The arm stays low and a little out at the side, almost still (one-handed attacks are the same with or without a shield) | The same | The same | A held stance (L1) raising it before the chest | Recalled from play, medium |
| **Mordhau** | Strap and hand | Down at the left side | Down at the side | Down at the side | Down at the side | Raised; blocking with a shield is held, and attacking and blocking are exclusive ([guide](https://www.ludo.guide/guide/mordhau/items-equipment/shields)) | Mechanics sourced; pose recalled, medium |
| **Skyrim** | In the fist | Forward-left at the chest | A per-arm layer over the one-handed attacks: the shield arm almost still, forward-left | The same; the blade often clips the board | The same | Raised before the face | Recalled, medium |
| **For Honor** | On the forearm | Before the left side of the chest, close | The same, the board edge-forward | The same | The same | Raised in its stance | Recalled, low; motion-matched mocap layered with combat moves ([masterclass coverage](https://www.trueachievements.com/n24310/for-honor-masterclass-video)) |
| **Kingdom Come: Deliverance 1 and 2** | Forearm straps and hand | Before the chest | Taken a little out as the cut is thrown | The same | Back before the chest in the recovery | Raised | Recalled, low; motion-captured with historical fencers ([Radio Prague](https://english.radio.cz/what-were-czech-middle-ages-find-out-kingdom-come-ii-8839276)) |
| **Witcher 3** | — | Geralt carries no shield; shielded enemies hold the board before the body | — | — | — | — | Recalled, high: nothing to read for a player's shield |
| **Talhoffer, Meyer** | — | Not used: Talhoffer's judicial-duel shields are full-height pavises held in two hands, and Meyer teaches no shield | — | — | — | — | — |

### Round 15 corrected

Round 15 read FtP 245.17-245.30 s as the shield "out to the left and back, its face toward the camera behind", and round 16 built its hold on that reading (90° out at the shoulder). Read again at every frame, those are frames of the warp's spin. From behind at 245.13 s, the body has turned to face the camera by 245.40 s, so the face seen at 245.23-245.30 is the board's front, seen as the body turns. Once the spin ends (245.40-247.20), the shield rides low at the left side at the waist, through four blows. The counterweight is real but small: tens of centimetres at the hip, not the arm thrown out and up.

## Targets

In order: the cheapest fix of the most noticed fault first, and each from the technique the evidence supports.

1. **`move.chain.shield_round_trip`, at most 0.1 m** (D447's CHAIN_M; Enshrouded keeps the shield out between chained blows). Technique: a **layered hold** (hold.py), data only. A blow entered only from a chain (the backhand; a finisher clip of its own, now Open_Slash's) holds from clip time 0, with no `in` ease, because the shield arrives already out. Do this first: one key in `fight.json`.
2. **`move.blows.shield_high` at most 0 cm, `shield_elbow` at least 0.15 m, `shield_face` at most 90°** (Enshrouded 245.40-247.20 s; Mordhau, Elden Ring and Skyrim recalled). Technique: the **layered hold re-keyed to a new held pose**, not Keyed_Guard's pose turned 90°. The pose: the upper arm down and 20-30° out from the side, the elbow bent about 90°, the forearm forward and out, the board's middle at belly height 10-20 cm out from the hip, its face forward and out about 45°. Read the pose off the Enshrouded frames into the hold. New Kimodo takes keyed from it only if the held arm fights the torso's counter-rotation (a clash or `hands` fault).
3. **`held.shield.grip_hand` at most 1 cm, `grip_across_palm` at most 15°** (holding.md section 3; Hurstwic; I.33). Technique: **the socket first, then IK**. For the round shield, hang the centre-grip board from the fist (the hand socket), its bar across the palm, as fit_held.py's TEXTURED already describes it. The strap is right only for a shield with enarmes (a heater or kite; Enshrouded's). Revisit D459's strap clause with this note; which shield the first set carries, round or heater, is an art and design call for the developers. Two-bone IK on the shield arm only after that, to hold the fist on the grip where a clip drifts more than 1 cm.
4. **`film.shield_round_trip`, 0** (the duel film). Technique: the fight record carries each fighter's shield middle. Then any move change still popping (a stagger, the block) gets an **inertialized** transition (Unreal's Inertialization, after Bollo's "Inertialization: High-Performance Animation Transitions in Gears of War", GDC 2018; recalled) instead of the 110 ms crossfade.

The block's own sword through its shield (`move.block.sword_in_shield`, 19.1 cm) stays the task list's first open fault. It is not a shield-arm fault and is not repeated here.

## Sources

- Enshrouded: "Forging the Path | Combat" ([RDQ0_R4AR-A](https://www.youtube.com/watch?v=RDQ0_R4AR-A)), frames at 60 fps in `~/.cache/terra-review/refs/enshrouded-combat/ftp/warp/` (244.90-247.20 s); "Combat Gameplay" ([EuViyK4qLQg](https://www.youtube.com/watch?v=EuViyK4qLQg)), `cg/cave/` 22.50 s. Read by eye, as [Enshrouded's melee, measured](2026-10-24-enshrouded-combat-measured.md) describes.
- Hurstwic, [Viking shields](https://www.hurstwic.org/history/articles/manufacturing/text/viking_shields.htm).
- ARMA, [The guards of I.33 and their footwork and cuts](https://thearma.org/essays/I33-guards-footwork.html); Royal Armouries Ms. I.33 fol. 1r and 5r ([Commons](https://commons.wikimedia.org/wiki/Category:Royal_Armouries_Ms._I.33)).
- Wikipedia, [Strapped shield](https://en.wikipedia.org/wiki/Strapped_shield) and [Heater shield](https://en.wikipedia.org/wiki/Heater_shield); the Morgan Picture Bible battle leaf ([Commons](https://commons.wikimedia.org/wiki/File:Leaf_from_the_Morgan_Picture_Bible_-_Battle_detail.jpg)).
- Viking reenactors, [Commons](https://commons.wikimedia.org/wiki/File:Vikings_(6096147677).jpg) (CC BY-SA 2.0): the fist at the boss, the board out at the side.
- Elden Ring, Dark Souls, Skyrim, For Honor, Kingdom Come, Witcher 3: recalled from play and footage, not read from frames this round; the confidence is in each row. No video downloader was available, so their frames are not on the sheet.
- The reference sheet (local, web images not committed): `~/.cache/terra-review/duel/shield-prior-art-sheet.jpg`.
