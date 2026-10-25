# The shield arm in a swing: what shipped sword-and-shield sets do with the shield while the sword cuts

2026-10-25, combat round 15. Question: in a one-handed blow with a shield on the other arm, where do shipped games put the shield in the wind-up, active and recovery frames, and when is it back in the guard? The developer, on the duel film: "they're poking through the shield to stab (instead of the shield arm moving the shield out of the way as part of it...)" and "you cant really hold a shield to your face AND swing... i think swinging probably needs to drop guard?" Measured by D481 (`move.<move>.sword_in_shield`, `shield_in_arm`).

## Answer

Every set looked at drops the guard to swing: the shield leaves the line of the body as the blow winds up, stays out of the sword's path through the active frames and the follow-through, and comes back to the guard only in the recovery, after the sword is back on its own side. None swings the sword across a shield held in front of the chest. Blocking and attacking are exclusive states in all of them: the guard is a stance the attack leaves, not a layer held over it.

- **Enshrouded** (Update 8, light chain, read frame by frame from the warp comparison, `ftp` 244.9-246.0 s, pictures in `~/.cache/terra-review/refs/enshrouded-combat/ftp/warp/`): in the **wind-up** (245.03) the torso turns right and the shield arm opens to the left side. Through the **active frames** (245.17-245.30) the chest counter-rotates left with the cut and the shield goes out to the left and back, its face toward the camera behind: a **counterweight** to the sword arm, the chest open, the sword passing in front of and below it. In the **follow-through** (245.47) the shield is held out at the left side at chest height, its face outward, the sword low ahead of the body; it does not return across the chest between blows of the chain (the next blow starts from there). Back on guard 1.05 s after the chain's last hit.
- **Elden Ring / Dark Souls** (recalled from play and footage, not measured): a one-handed light attack plays the right arm's moveset with the shield arm pulled down and in to the left hip or out to the side; the shield is up only while the guard button is held (a separate stance, L1), and pressing attack leaves it. Guard counters are the one attack from behind a raised shield, and they too swing the shield aside first.
- **Mordhau** (mechanics read: [Ludo guide](https://www.ludo.guide/guide/mordhau/items-equipment/shields)): a shield block is held, and you cannot attack while blocking; an attack lowers it. The swing's **windup, release and recovery** play with the shield arm low at the side; the shield is raised again only by blocking after the recovery.
- **Kingdom Come: Deliverance** (recalled from footage, low confidence; its swordplay was motion-captured with historical fencers, [Radio Prague](https://english.radio.cz/what-were-czech-middle-ages-find-out-kingdom-come-ii-8839276)): the shield is held forward in the guard and brought aside, outward, as the cut is thrown, then back in front in the recovery; the strike never passes through the board.

## What it means for our set

The light slash (Keyed_Slash: a right-to-left diagonal cut ending low left) should take the shield out to the left and a little back, the face turned outward, from the wind-up through the follow-through, and bring it back to the guard only once the sword has come back to the right side, inside the clip's recovery (D450 keeps the guard at both ends). Industry terms: the guard drops in the **anticipation** (wind-up), the shield arm **counter-rotates** as a **counterweight** through the **active frames**, and returns in the **recovery**. How far the guard drops (how much the chest is open in a light chain) is a feel call; this note gives the shipped answer, Enshrouded's out-and-back.

## Tried in round 15

Six left-hand keys on Keyed_Slash, the fist out to the left and a little back at chest height (grip [-0.38 to -0.42, 0 to 0.08, 0.28] right, ahead, up from the hips; the hand turned 70-80 degrees outward, the face to the left) from 0.3 s to 1.05 s, back to the guard by 1.7 s: every one of 32 takes (seeds 2 and 3) cleared the clash, the sword 0.5 to 7 cm clear of the board and the board 4 to 11 cm clear of the sword hand. None passed the move check, on the right arm: Kimodo cut the follow-through short (the fist back by the guard at 0.6 s, not low left) and the sword went into the right thigh (0.67 s, 1.27-1.53 s) or the spine (the wind-up, 0.27-0.33 s), deepest on the female template. A shield held at the waist (0.15 m up) put its rim in the left thigh at 0.2 s; chest height cleared it.

## Sources and confidence

- Enshrouded: the official "Forging the Path | Combat" video ([RDQ0_R4AR-A](https://www.youtube.com/watch?v=RDQ0_R4AR-A)), frames read by eye; high confidence on where the shield is, the poses being clear at 60 fps.
- Elden Ring and Kingdom Come: recalled, not read from frames here; medium and low confidence.
- Mordhau: mechanics read from guides; the arm's pose recalled.
