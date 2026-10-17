# Large fliers: how games and films make a heavy flier move smoothly

2026-10-17. Question: how do shipped games and films, and the biology under them, move a large flier (our dragon, about 17 m long and 20 m span) so its flight reads as heavy and smooth, not wonky: bounded-curvature paths, banking, head-led bodies, flap against glide, speed limits, flare and the tell before a breath? Builds on section 3 of [2026-10-15-dragons-and-fire.md](2026-10-15-dragons-and-fire.md).

Tags: **[P]** primary (developer's code, talk, paper), **[S]** secondary (wikis, guides, press, blogs), **[I]** inferred or worked out here. Nothing was measured in our game.

## Answer

- **No studio publishes its flier tuning.** The primary sources that exist are Reynolds's steering code (MIT), bird and bat papers, and the animation principles. The rest is [S] or [I], and says so.
- **Turn from the bank, not the other way round.** In a coordinated turn the heading rate is `ω = g·tanφ / v`. If yaw is derived from the bank the dragon has actually reached, it rolls first and turns after, with no extra rule. The path's curvature must never ask for more than `κmax = g·tanφmax / v²`.
- **Heavy means slow to start and slow to stop, then smooth.** Bank, pitch and speed each move by a rate limit and a first-order smoothing of about half a second, never a snap.
- **The body trails the head's path.** Neck, body and tail sit where the head was `s/v` seconds ago. At 28 m/s the tail of a 17 m dragon lags about 0.6 s with no spring at all. A light spring adds the overshoot.
- **Flap when short of energy, glide when over.** One test on specific energy (`v²/2 + g·h`) makes it flap climbing and slowing, glide diving and cruising, and blend between.
- **A tell before every breath**, long enough to read at the range the player sees it from.

## 1. Paths with bounded curvature

- **[P] Dubins paths** are the shortest curve for a vehicle that only moves forward with a minimum turning radius: arcs and straights, always one of RSR, RSL, LSR, LSL, RLR, LRL. https://en.wikipedia.org/wiki/Dubins_path (read in full). The vehicle's maximum turn rate sets the minimum radius.
- **[I] The flaw for animation.** A Dubins path jumps in curvature at each arc and straight joint, so a bank that follows curvature snaps. Several UAV-planning papers in the search results say the same and blend curvature or use Bezier fillets; I did not read them. Our fix is cheap: round every waypoint corner with a fillet no tighter than `rmin`, and ease curvature between arc and straight over about one second of travel.
- **[P] Reynolds, steering behaviours** (GDC 1999). https://www.red3d.com/cwr/steer/gdc99/ (read)
  - Steering is a force clipped to a `max_force`, and speed is clipped by "kinematic truncation" of velocity. That is the simple vehicle: it cannot turn harder than its limit.
  - **Seek** aims velocity at the target. **Arrival** is seek that slows inside a slowing radius.
  - For flying vehicles roll is defined by banking: the up vector is a weighted sum of steering acceleration, gravity and the old up, the old up damping abrupt roll.
- **[P] OpenSteer** (Reynolds, Sony, MIT-style licence, so portable with its notice). https://raw.githubusercontent.com/meshula/OpenSteer/master/src/SimpleVehicle.cpp
  - Steering acceleration is smoothed each step at `clip(9·dt, 0.15, 0.4)`. Its experimental banking blends the up vector at `dt·3` (a time constant near 0.3 s) toward `0.05·smoothedAcceleration + (0, 0.2, 0)`.
- **[S] Keeper's bird** (Double Fine, James Collins, GDC 2026, "Fixed-wing flight in Keeper: pathfinding, locomotion and animation of a bird companion"). https://schedule.gdconf.com/session/fixed-wing-flight-in-keeper-pathfinding-locomotion-and-animation-of-a-bird-companion/913823
  - The page itself returned 403 to my fetch. All I have is search-engine snippets of the abstract: turning radius, speed, climb and avoidance must agree with animation timing, and fixed wing is harder than free flight. Collins also owned World of Warcraft's Skyriding. No numbers, no slides, no video seen.
- **[S] Game NPC pilots** (a patent, Unity and Three.js forum threads from the search; low authority). Real caps: 60° bank in one demo; the pilot adds back-pitch in proportion to how fast the bank changes, or the aircraft loses height in a turn. Forums say the problem is knowing when and how much to bank, and that games fake some of it.
- **[I] Radius `r = v²/(g·tanφ)`, load `1/cosφ`.** From the standard coordinated-turn relation (confirmed in the search results, e.g. https://aerospaceweb.org/question/performance/q0146.shtml ). For the dragon:

| Speed | 30° bank (1.15 g) | 45° (1.41 g) | 55° (1.74 g) |
|---|---|---|---|
| 22 m/s | 85 m | 49 m | 34 m |
| 28 m/s | 138 m | 80 m | 55 m |
| 35 m/s | 216 m | 125 m | 86 m |

## 2. Banking, rolling and pitching

- **[P] Real birds roll first, hold, then unroll.** Rose-breasted cockatoos began a roll in the second half of the downstroke before the turn, set the bank in one shallow wingbeat, held it through the turn and rolled level at the end. The peak was about 70° at a 0.9 m radius, in slow tight turns. Hedrick et al. 2007, J Exp Biol 210:1897. https://biewenerlab.oeb.harvard.edu/publications/low-speed-maneuvering-flight-rose-breasted-cockatoo-eolophus-roseicapillus-i (abstract only; the 403 stopped the page, the facts are from the search's abstracts).
- **[P] Large soarers bank 25 to 35°** low in a thermal, less higher up; Pennycuick's optimum for vultures is about 24° (Williams et al. 2018). https://hal.umontpellier.fr/hal-02393999 (abstract). A heavy animal circles at moderate bank, not 60°.
- **[S] HTTYD** (Simon Otto, DreamWorks). Animators went through a two-week "flight school": wingbeat, take-off and landing tests. A flap-cycle system drove the wings, and "weight, lift, drag, turbulence" were key. A wingsuit flier's footage was reference for the rider. https://www.artofvfx.com/how-to-train-your-dragon-2-simon-otto-head-of-character-animation-dreamworks/ (an interview about the sequel; it says nothing of banking, bats or speeds). An ornithologist advised on wing anatomy (Den of Geek, https://www.denofgeek.com/movies/revisiting-how-to-train-your-dragon, press only).
- **[I] Pitch follows the flight path.** Pitch = climb angle plus a few degrees of attack; do not pitch the nose by hand. Roll leads the turn by about one wingbeat (cockatoo). Add a little back-pitch in a held bank, since lift is split (the patent above says the same).
- **[I] Flare.** To brake into a hover or a landing: nose up while the speed falls, wings spread and beating deep, the body's pitch following the speed down.

## 3. Head leads, neck and tail lag

- **[P] Follow-through and overlapping action** (Thomas and Johnston, *The Illusion of Life*, 1981, pp. 59 to 62): loose parts keep moving after the body stops, then settle; parts move at different rates, so a tail lags the head. https://en.wikipedia.org/wiki/Twelve_basic_principles_of_animation
- **[P] RuneScape's Spirit Dragons** (Jagex dev blog). The neck was animated up and down, copied down the body with a one-frame offset per segment, so the wave travels head to tail; extra secondary motion was added. It used translation, not rotation. https://secure.runescape.com/m=news/dev-blog--animating-spirit-dragons (read). It says nothing about flight.
- **[S] Spring bones** are the usual tool for tails and necks (Blender add-ons, Polycount threads, Abratabia's overview), with stiffness stronger at the root. These are tooling notes, not tuning.
- **[S] Body rises on the downstroke** in a student's study of a dragon, albatross and condor (Uppsala, 2015). https://babel.speldesign.uu.se/?p=10823 A blog, not a paper.
- **[I] Physics of the bob.** Lift is higher on the downstroke, so the body accelerates up then. With a 0.8 Hz beat and about ±0.5 g over the stroke, amplitude is `a/ω² ≈ 0.2 m`, the speed peaks at the end of the downstroke and the height about a quarter beat later.
- **[I] Not found:** Monster Hunter's, Skyrim's, Spyro's and Dragon's Dogma's spine or tail lag. They are keyframed or physics-bone tuned, and no talk gives numbers.

## 4. Flap, glide and stroke rate

- **[P] Jensen, Hecksher and Dyre 2024** (PLOS ONE 19(6): e0303834, read in full): beat rate `f = 0.88·√m / A` (m in kg, A in m² of wing), on 414 animals from mosquitoes to a blue whale, correlation 0.95; Quetzalcoatlus (65 kg, 10 m²) gives 0.7 Hz. https://doi.org/10.1371/journal.pone.0303834 . For our dragon at 3 t and 60 m² of wing [I, the mass and area are guesses] it is 0.8 Hz; at 2 to 4 t, 0.66 to 0.93 Hz. That agrees with Pennycuick 1990 in the earlier note.
- **[P] Pennycuick's model** has a U-shaped power curve (induced, parasite, profile power), giving a minimum-power speed `Vmp` and a faster maximum-range speed `Vmr`; a jackdaw example runs 3.4 (min), 8.1 (`Vmp`) and 11.1 m/s (`Vmr`). https://cran.r-project.org/web/packages/afpt/vignettes/afpt-basic-usage.html (vignette, search snippet). Large birds fly slowly in flap mode near `Vmp` and cruise near `Vmr`.
- **[P] Witton and Habib 2010**: giant pterosaurs flapped "in short bursts" and flap-glided. https://pmc.ncbi.nlm.nih.gov/articles/PMC2981443 (through search summaries only). Condors flap 1% of the time (earlier note).
- **[I] The rule.** Keep a target energy `E* = v*²/2 + g·h*` for the cruise speed and altitude. Below `E*`, or climbing or slowing, flap; above it, or diving, glide. Hysteresis stops chatter.
- **[I] Glide.** A glide ratio near 10 (a gliding vulture's best is 11.6 in a wind-tunnel study, J Exp Biol 53:363, https://cob.silverchair.com/jeb/article/53/2/363/21469/Aerodynamics-of-Gliding-Flight-of-a-Black-Vulture , snippet only) means it sinks `v/10`, about 2.8 m/s at 28 m/s, with no flapping.

## 5. Speed, heaviness, flare and the tell

- **[I] Stall.** `Vs = √(2mg / (ρ·S·CLmax))`. At 3 t, 60 m², CL 1.2 and sea-level air it is 26 m/s; at 2 t, 21 m/s. Stall speed rises with the root of the load: 1.19× at 45° of bank. Cruise at 1.3 × Vs or more, so a banked strafe is still above stall.
- **[S] Skyrim** (earlier note): dragons choose hover, drive-by, perch or land from weighted odds; unridden dragons just circle. Forum threads: if a dragon finds no landing place in its cell it flies off. https://www.uesp.net/wiki/Skyrim_talk:Dragonrend
- **[S] Monster Hunter Rise's Rathalos.** In the air it "sharply inhales" and twirls its head up, flies backward, then fires, with a pause between fireballs; on the ground the same move starts with the head twirled up while it inhales. https://gamerguides.com/monster-hunter-rise/guide/monsters/large-monsters/rathalos (a fan guide). No frame data found.
- **[S] Elden Ring's Agheel**: it rises and flies backward, a glow gathers in its mouth, and the breath is long. Guides give no seconds. The only timing found, a 2 s windup, belongs to the player's own Agheel's Flame spell. https://eldenring.wiki.fextralife.com/Agheel's+Flame
- **[P] Anticipation** (Thomas and Johnston, pp. 51 to 52): prepare the audience for the action. Every tell above is a pull-back plus a glow.
- **[I] Accelerations.** No source gives a dragon's. A heavy flier reads heavy when changes of speed take seconds: faster by `3 m/s²`, slower by `4 m/s²` flaring, 22 to 30 m/s in 3 s.

## What we take

All numbers are [I] unless a source is named; they are starting values to tune against a film (`--vantage`, then the adversarial review).

1. **Speeds.** Stall 22 to 26 m/s at 2 to 3 t. Cruise 28 m/s. Strafe 30 to 35 m/s. Dive peak 40 m/s. Hover is a cheat (earlier note) and starts only below 6 m/s.
2. **Bank.** Cruise limit 35°, hard limit 45° (1.41 g), never past 55°. Bank rate 30°/s, and a first-order lag of 0.5 s on the wanted bank. The cockatoo set the bank inside one wingbeat; ours takes 1.0 to 1.5 s to reach 35°.
3. **Radius.** `rmin = v²/(g·tan φmax)`: 80 m at 28 m/s and 45°, 114 m at 35°. Every route is filleted at least that wide; asked for a tighter corner, fly a wider loop, not a snap. A 180° turn at 28 m/s and 35° takes about 12 s.
4. **Roll before yaw.** Heading rate is `g·tan(bank) / v` from the actual bank, not a separate clamp. Add a slight nose-up of 2 to 3° per 10° of bank.
5. **Pitch.** Climb angle within −45° (dive) to +25°, pitch rate under 25°/s, pitch = climb angle + 4° with 0.5 s smoothing. The big nose-up is kept for the flare (item 7).
6. **Acceleration.** Speed up by 3 m/s² (flapping), slow by 4 m/s² (flare and drag), and let a dive add `g·sin γ` capped at 40 m/s. Set the approach profile as `v = √(2·a·d)` to the hover point so it arrives with no speed left.
7. **Flare.** Over the last 1.5 s: pitch up 30°, wings spread, beat deep at twice the cruise rate (1.6 Hz); body held about 0.3 s after the speed hits zero before the next action.
8. **Body trail.** Head looks 0.3 s along the path ahead; each of about 8 body and tail points follows the head's path `s/v` behind (0.6 s for the tail tip at 28 m/s). On top, a damped spring for the neck and tail (damping ratio 0.5 to 0.7, natural frequency 1 to 1.5 Hz, stiffer at the root), and an extra 0.1 s lag on the wing tips.
9. **Flap and glide.** Cruise beat 0.8 Hz at 3 t; climb or slowing 1.1 Hz (up to about 1.3× cruise). Glide when over target energy or sinking more than 1 m/s; flap when under it or climbing above 1 m/s. Add 0.2 s of hysteresis. Crossfade between flap and glide over 0.5 to 0.8 s (ours is 0.45 s), starting the flap at the top of the upstroke.
10. **Bob.** About ±0.2 m over the beat at 0.8 Hz, the body rising through the downstroke, topping out around mid-upstroke. Scale with weight, 1/ω² (so slower beats bob more).
11. **Tell.** Before a breath: 1.2 s total. Head pulls back and up for 0.6 s with the wings holding, the mouth glow rises over the next 0.6 s, then the flame starts. For the strafe the tell begins no closer than 25 m/s × 1.2 s = 30 m before the flame's start. A hover breath finishes its flare before the tell starts. A breath holds 1.5 to 2.5 s, with a 0.6 s recovery (head forward, wings pump) before the next move.
12. **Check.** A film from the side and above shows the roll leading the heading, a tail lagging by 0.4 to 0.7 s on a turn, and a flap-to-glide switch without a pop.

## What was only read through an abstract or press

Keeper's talk (search snippets of its abstract; the page and gamedev.net mirror gave 403). HTTYD (an interview about the sequel, a review). The cockatoo and vulture papers (abstracts via search). Witton and Habib 2010, and the gliding-vulture paper (summaries). Pennycuick's models (a package vignette, not his books). Monster Hunter, Elden Ring and Skyrim behaviour comes from fan guides and forums. Nothing found gives a studio's dragon speed, bank limit, tail lag or windup in seconds; those in "What we take" are ours.
