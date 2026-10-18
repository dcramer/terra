# How shipped melee makes a blow feel heavy

2026-10-17. Question: why did the troll fight's first film read as "tons of delays between moves, no fluidity, no juiciness", and what do shipped melee games do that it did not? Asked for the troll trailer (docs/troll-fight.md).

## Answer

A blow reads as heavy from its timing, not its clip alone: a held wind-up, a fast strike, a short freeze on contact, a follow-through played out, and the next move starting inside the last one's recovery. Our first film sent an idle before each blow had landed and started each beat from a standstill, so every blow was cut short and followed by a gap.

## What the games do

- **Anticipation, action, follow-through** (the animator's rule of three). Elden Ring's giants and God of War's trolls telegraph with a long wind-up, often held at its top (the "delayed attack"), then strike fast, then stay bent over the blow: the recovery is the player's window to punish.
- **Hit-stop (hit-lag).** On contact both attacker and target freeze for a few frames. Sakurai's column and video on it: freeze both, shake the one hit more than the attacker, keep the attacker moving a little rather than still, shake sideways on the ground and less over time, scale the freeze with the blow. Fighting games hold a light hit about 6 frames at 60 a second and a heavy one 10 to 14. Monster Hunter holds the end of a heavy weapon's swing for several frames, strongest on the Great Sword and Hammer; Capcom lightened it for Wilds' beta and players complained the blows felt weightless, so the release made it heavier again.
- **Cancels.** Action games (Devil May Cry, Ninja Gaiden, God of War) let the next attack start in a window of the last one's follow-through, so a combination flows without returning to idle.
- **Camera and effects.** A short, quickly fading shake on heavy contacts, and a burst at the impact point (dust and debris, a splash), framed side on so the point of contact is visible.

## What the troll film takes

- Each blow's moments (its start, wind-up top, contact, recovery) are measured from its clip's take, and the scene times everything from them (`perform` in tools/sandbox/trailer/scenes/troll.ts).
- A staged figure's own time can be slowed and sped (`staged.rate`): the troll's wind-up is held about 0.4 s at its top, its strike played about 1.5 times as fast, and every contact freezes both fighters at 4% speed for 6 (a cut), 10 (the sweep on the shield) or 12 frames (a smash).
- The troll stays bent over a smash at half speed, the window the man cuts in; each next move starts in the last one's recovery, never from an idle.
- Bigger bursts (dust and stones, a water sheet), a jolt-and-fade camera shake, and contacts framed side on.

## Sources

- Masahiro Sakurai, Famitsu column vol. 490 on hit-stop, translated by Source Gaming (2015): https://sourcegaming.info/2015/11/11/thoughts-on-hitstop-sakurais-famitsu-column-vol-490-1/
- Nintendo Wire's summary of Sakurai's video on fine-tuning hit-stop (2022): https://nintendowire.com/news/2022/12/12/this-week-in-sakurai-12-5-12-11-fine-tuning-hit-stop-and-cheating-the-system/
- Monster Hunter Wilds' hit-stop, explained: https://deltiasgaming.com/?p=113987 and https://mp1st.com/news/monster-hunter-wilds-hitstops-final-game-closer-to-people-are-looking-forward
- "Combat Canceled: God of War and action game design", Game Developer: https://www.gamedeveloper.com/game-platforms/combat-canceled-i-god-of-war-i-action-game-design
- Jason McDonald, "Taking an Axe to God of War Gameplay", GDC 2019: https://gdcvault.com/play/1026023/Taking-an-Axe-to-God
