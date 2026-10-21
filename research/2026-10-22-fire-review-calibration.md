# Fire review calibration: what AAA frames score from our harsh reviewer

2026-10-22. The question: our harsh reviewers score the campfire 5-5.5/10 and the dragon breath about 5/10 "against AAA game frames". What does a real AAA frame score from the same kind of reviewer, blind? Without that, 5.5 has no meaning.

## Answer

**On this reviewer's scale, the best AAA frame scores 8-8.5 and an average shipped-game screenshot scores 5.3-6. Our fire scores 4.3-4.7.** Our campfire is about 0.6 below the average game frame and 3.7 below the best one (RDR2). Our breath is about 1.7 below the average and 3.7 below the best (Elden Ring). Our single best frame scores 5.0 for the campfire and 5.3 for the breath. So 5.5 from a round review means "as good as a middling game screenshot". The reviewer's own anchor, 7.5 = shippable, is about right, because only one frame in each set clears it.

| Set | Game frames, mean | Ours, mean | Gap | Best game frame | Our best frame |
|---|---|---|---|---|---|
| Campfire | 5.29 | 4.67 | 0.63 | RDR2 8.33 | 5.00 (round 38, night, 2.5 m) |
| Breath | 5.96 | 4.29 | 1.67 | Elden Ring 8.00 | 5.33 (vol9, dusk three-quarter) |

The mean scores of each frame over three reviewers:

- **Campfire games:** RDR2 8.33, Witcher 3 pyre 5.33, Enshrouded 4.83, Sons of the Forest 2.67.
- **Campfire ours:** 5.00, 4.67, 4.50, 4.50.
- **Breath games:** Elden Ring 8.00, MHW palisade 6.33, MHW Fatalis 5.83, Skyrim 3.67.
- **Breath ours:** 5.33, 4.50, 4.00, 3.33.

The reviewers agreed closely: every reviewer put the same frame first in each set, and the reviewers' means for each group of four were within 1.0 of each other.

## Could they tell which were games?

No. No reviewer named a game, called a frame a real capture, or treated the game frames differently. They ranked by look, not by source:

- In the campfire set, two of the three reviewers ranked our 2.5 m frame above Enshrouded and Sons of the Forest, and one also ranked it above Witcher 3.
- Every reviewer ranked Sons of the Forest last.
- In the breath set, every reviewer ranked Elden Ring and both MHW frames above all of ours.
- All three ranked our best breath frame above Skyrim's.

They also gave shipped games the faults they give ours: "flame sprite cards" (Witcher 3), "salmon-pink" logs (Enshrouded), "blown-out mush" (MHW) and "grey haze" (Skyrim). A named fault is evidence about the picture, not proof that a technique is missing.

## What it means for the fire work

- Our reviews' "about 5" agrees with this blind test. The reviewers are not unfairly harsh on us.
- The target is the top frame, not the average. RDR2's campfire and Elden Ring's breath score about 8. Only those clear the 7.5 bar, and they set the gap: about 3 points for both fires.
- The breath has the bigger gap to ordinary games. The campfire is already level with the middle of the pack.
- Where the top frames score, by the reviewers' own words:
  - RDR2: layered tongues, a visible coal bed, and light on the people and props.
  - Elden Ring: a readable jet with lit water and embers.
- Our recurring faults are:
  - campfire: flat cards, tiled char, a single rope of smoke and a flat disc of light;
  - breath: no jet from the jaw, a soft blob, and a salmon plume behind it.

## Method and limits

- **Sets:** two blind sets of eight, four game frames and four of ours in each. Each frame was cropped to the fire with the HUD, labels and watermark removed, then resized to 1280x720 and shuffled with a seed.
  - Campfire games were Steam screenshots at 1920x1080: RDR2, Witcher 3 (a pyre, close), Enshrouded (dusk) and Sons of the Forest.
  - Campfire ours were round 38 night frames (main, M2 and the 2.5 m review view) and round 32 GFCS night close.
  - Breath games were 720p video captures: Elden Ring Agheel, MHW Iceborne Fatalis (two frames, one cutscene) and Skyrim. The Game of Thrones reference frames were left out, because they are not a game.
  - Breath ours were vol9 `lit` frames, after 55435ea2: dusk three-quarter, night side, night head-on and night front three-quarter.
- **Reviewers:** three fresh reviewers per set, each given the rounds' harsh brief: score absolutely out of 10 as AAA-game fire, 7.5 = shippable, and name the top fault. They were not told that some frames were games.
- **Limits:**
  - Four frames per group and one model are a small sample. Treat differences under about half a point as noise.
  - The game frames are compressed screenshots and video captures, some of weak moments: the Sons of the Forest fire is small and far, and Skyrim is from 2011. Their mean is therefore a floor for "AAA", not a typical value.
  - Our crops were enlarged from about 800-1400 px, and the game frames were enlarged from 720p. Neither side is at native resolution.
  - Stills only: flicker and motion, where real-time fire often wins or loses, are not judged.
- **Where the files are:** the sets, the script and the key are in the session scratchpad. The game frames stay in `~/.cache/terra-review/fire/`, never in the repository.
