# Dragons and fire: how shipped games spread, draw and breathe it

2026-10-15. Question: how do shipped games spread fire through grass, wood and buildings, how does real fire behave on timber and stone, how do games fly a dragon and draw its breath, and how are flames, embers, smoke and their light drawn and capped? For a cellular fire on half-metre building cubes and a dragon that sets a base alight.

Tags: **[P]** primary (developer's docs, code, interview, paper), **[S]** secondary (wikis, guides, press), **[I]** inferred or worked out here. Nothing was measured in our game.

## Answer

- **Every shipped fire is a few simple rules, tuned hard.** Far Cry 2's programmer: the propagation was simple; balancing realism against play took most of eighteen months. Fire spreads by heat to neighbours, faster upward and downwind, slower when wet, and burns out when its fuel is gone.
- **On a grid it is a cellular automaton.** Minecraft and Teardown each step a cell (a block, a voxel) on a tick, with odds or heat per material. The known flaw of cells is a square or diamond front; per-cell jitter in the threshold hides it.
- **Real timber burns slowly through and fast across its surface.** Softwood chars at about 0.65 to 0.8 mm a minute, so a 250 mm log wall takes hours to burn through. Flame climbs a wall in seconds and crawls sideways in minutes. Game time must be compressed: fun wins (D122).
- **Stone does not burn, but a burnt castle is a shell.** Roofs and floors go, and the walls stand sooted and reddened, with empty joist sockets. Lime calcines past about 600 °C. Timber in or on walls (lacing, hoardings, mine props) is how fire brings stone down.
- **A dragon is a fixed-wing flyer that cheats only to hover.** Something with a 15 to 20 m span beats its wings about once a second, stalls near 20 m/s, and turns on a radius of 40 to 70 m at 30 to 45° of bank. Games give it a few states: circle, strafe (fly-by breath), hover breath, land, ground fight, take off.
- **Flames are few large flipbook cards, emissive and soft.** Embers are velocity-stretched sparks, smoke is lit cards above, and light comes from one flickering, mostly shadowless light per cluster of fire. Every game caps the count. Ours is D325's instanced cards, with no particle dependency.

## 1. Fire spread in shipped games

- **[P] Far Cry 2** (Lévesque, interview, 2008). https://www.gamedeveloper.com/game-platforms/interview-how-i-far-cry-2-i-s-fire-fuels-spreads
  - Wind sets the direction; the flames' hot gases make it run faster uphill than down. The jungle is hard to light and spreads little, against the savanna, and recent rain slows ignition until things dry.
  - Capped: "the fire doesn't propagate indefinitely", and particles are limited for frame time. In one test a barrel set miles alight in two minutes and the frame rate fell. Fire scares creatures, an area of fear.
- **[S] Real slope and wind rules.** These are what Far Cry approximates.
  - **Slope:** spread doubles for each 10° of upslope and halves for each 10° down, `R = R0·2^(θ/10)`, valid to about ±20° (McArthur, used by CSIRO). https://research.csiro.au/spark/resources/model-library/slope-effects
  - **Wind:** a dry forest or shrub fire's front runs at about 10% of the open wind speed (Cruz and Alexander 2019, not for grass). https://doi.org/10.1007/s13595-019-0829-8
- **[S] Breath of the Wild and Tears of the Kingdom** (GDC 2017, Fujibayashi, Dohta, Takizawa: "Breaking Conventions"). https://gamesbeat.com/the-legend-of-zelda-breath-of-the-wild-makes-chemistry-just-as-important-as-physics/
  - The chemistry engine has three rules. Elements (fire, water, ice, wind, electricity) change materials' states. Elements change each other. Materials do not change materials.
  - So fire, not a burning tree, lights the next tree: fire is an element that spreads.
  - Grass burns in a running front, and burning grass makes an updraft you can glide on. Wind pushes fire.
  - Wooden things catch and burn away, and wooden gear catches in volcanic heat. Metal conducts electricity and draws lightning. Water puts fire out. https://zeldawiki.wiki/wiki/Shields_in_Breath_of_the_Wild
- **[P] Minecraft (Java)**, a cellular fire with numbers. https://minecraft.wiki/w/Fire
  - **The tick:** a fire block updates every 30 to 40 game ticks (1.5 to 2 s). Its age runs 0 to 15 and rises by about a third of a step a tick.
  - **Fuel by block:** each block has *encouragement* (ignite odds) and *flammability* (burn odds). Logs are 5/5, planks 5/20, leaves and wool 30/60, grass and flowers 60/100, hay 60/20.
  - **Burning away:** each tick, each of the six neighbours is consumed with chance flammability/300 sideways and /250 up and down. Planks go about 7% a tick a face. The consumed block becomes fire or air.
  - **Spreading** [I, from the game's code]: fire reaches empty cells 1 below to 4 above in a 3×3 column. The odds are (encouragement + 40 + 7·difficulty)/(age + 30) against 100, plus 100 for each level past one up. Young fire spreads most, and humid biomes halve it.
  - **Rain:** rain on a fire or its four sides puts it out with chance 20% + 3% × age.
  - **Going out:** fire with nothing flammable beside it goes out after age 3. At age 15 it goes out a quarter of ticks. Fire only spreads within 8 chunks of a player, its performance cap.
- **[P] Teardown**, voxel heat. https://teardowngame.com/modding/api.html
  - `AddHeat` heats a voxel. Past a critical heat the voxel is destroyed and can light the voxels round it, so a fire eats holes.
  - Fires are counted and queryable (`GetFireCount`, `QueryAabbFireCount`, `RemoveAabbFires`). Flames are emissive particles from a 16-tile atlas, and extinguishing particles are flagged so water finds fire.
- **[S] Caps and design levers** (realtimevfx thread, on Far Cry 2 and Divinity: Original Sin 2): fire dies down over time, burnt ground does not burn again, an attack makes at most so much fire a second against how fast fire dies, nearby fires merge into one, Divinity's surfaces burn a set number of turns. https://realtimevfx.com/t/too-much-fire/8529
- **[P] The flaw of cells.** Cell fires distort the front into the grid's shape (Johnston, Kelso and Milne 2008). https://www.publish.csiro.au/WF/WF06147

## 2. Real fire on timber buildings and stone

**Timber and roofs**

- **[P] Charring** (EN 1995-1-2).
  - One-dimensional β0 is 0.65 mm/min for softwood of 290 kg/m³ or more. The notional βn, with corners and cracks, is 0.8 for solid softwood and 0.7 for glulam. Dense hardwood (450 kg/m³ or more) is 0.5 and 0.55.
  - Tests on glulam and CLT give 0.65 ± 0.1. So a 50 mm board burns through in about an hour from one side, a 250 mm log in 4 to 6 hours. Big timbers keep their core and strength long after their surface is black.
  - https://www.academia.edu/7616333/Assessment_of_Eurocode_5_Charring_Rate_Calculation_Methods , https://link.springer.com/article/10.1007/s10694-024-01593-x
- **[I] Flame spread.** The terms are fire science's; the speeds are typical, not from one test.
  - Upward spread is *concurrent*: flame heats the wood above it, so it runs up a wall in seconds to tens of seconds.
  - Sideways and downward spread is *opposed*: it creeps, typically at millimetres a second on wood. https://publications.iafss.org/publications/fss/1/97
  - The standard fire (ISO 834, `T = 20 + 345·log10(8t+1)`) reaches 576 °C in 5 minutes, 842 °C in 30 and 945 °C in 60. A room flashes over within minutes once its hot layer passes about 500 to 600 °C.
- **[S] Thatch and shingles.**
  - Thatch is lit mostly by embers from the chimney, not by heat through the stack.
  - It burns deep and is "difficult to detect and once started is difficult to control". One in Devon took 100 firefighters ten hours (https://www.ifsecglobal.com/fire-news/spate-of-serious-thatched-roof-fires-prompts-safety-warning/).
  - Its surface flares fast and its depth smoulders for hours. Shingles light from embers the same way. https://www.cheshirefire.gov.uk/your-safety/home-safety/thatched-buildings
- **[S] Embers spread towns.** London in 1666 burned 13,200 timber houses in four days, driven by an east wind across jettied streets. https://en.wikipedia.org/wiki/Great_Fire_of_London
- **[P] Flame size** (fire engineering's correlations, worked out here).
  - **Height:** Heskestad's `L = 0.235·Q^0.4 − 1.02·D` (L and D in m, Q in kW).
  - **Flicker:** a fire puffs at `f ≈ 1.5/√D` Hz, whatever its fuel. https://arxiv.org/pdf/1803.10400
  - **[I] A cube's face** at about 250 kW/m²: a flame about 0.65 m tall, flickering at 2 Hz.
  - **[I] A fully burning roof** of 6 × 6 m: a flame about 5 m tall, flickering at 0.6 Hz.
  - **Colour:** flame is yellow-orange from glowing soot at about 1000 to 1200 °C. Blue shows only where gas burns premixed, at the base.

**Stone and masonry**

- **[S] Heat in stone.** At 200 to 300 °C iron-bearing stone turns pink to red, for good. Above about 600 °C limestone (and lime mortar, the same carbonate) calcines to quicklime, and stone that looks whole can crumble later as it takes up water. At 600 to 800 °C stone loses much of its strength, and spalls and cracks under thermal shock, worst when hoses hit it hot. https://www.fireriskheritage.net/?p=7902
- **[S] Notre-Dame, 2019.** The oak roof, "the forest", was almost all lost. The stone vaults caught the falling timber and failed in only three bays, under the spire's fall. The stone was stained red, black and white, burst at its face and damaged to 20 cm deep. https://www.stone-ideas.com/86373/notre-dame-replacement-stones/ , https://www.istructe.org/getmedia/3db76999-b62b-4116-bfc7-5b75b118ac27/Viewpoint-The-fire-at-Notre-Dame-cathedral.pdf
- **[I] How a burnt castle looks.** A roofless shell with its floors gone, rows of empty joist sockets, soot plumes above every window, and reddened stone round the fiercest rooms.

**Timber in stone defences**

- **[P] Timber-laced ramparts.** These Iron Age walls burned so hot they fused to glass, the *vitrified forts*.
  - Childe and Thorneycroft's 1937 test wall burned three hours and collapsed vitrified. The timber charred first, an endothermic step, and then the charcoal burned hot. https://journals.socantscot.org/index.php/psas/article/view/8085
  - Dun Deardail's core reached 850 to 1100 °C, likely from a burning timber-and-thatch superstructure. https://archaeology.co.uk/articles/burning-questions-at-dun-deardail.htm
- **[S] Hoardings** (hourds). Timber galleries hung on a wall's top for a siege, cheap but burnt by fire arrows and pots. Stone machicolations replaced them for that reason. https://en.wikipedia.org/wiki/Hoarding_(castle)
- **[P] Mining by fire.** At Rochester in 1215, King John's miners propped a tunnel under the keep's corner with timber, filled it with brushwood and the fat of forty pigs, and burned it. The props failed and the corner fell. https://www.english-heritage.org.uk/visit/places/rochester-castle/history/

## 3. Dragons in shipped games and in physics

**How games fly them**

- **[P] Skyrim** (Creation Kit, combat style's Flight tab). https://en.uesp.net/wiki/Tes5Mod:Mod_File_Format/CSTY , https://ck.uesp.net/wiki/Combat_Style
  - Weighted choices, each with a default: Hover Chance 0.5, Dive Bomb (pick up and drop) 1.0, Ground Attack Chance 0.5 with a Ground Attack Time, Perch Attack (land on a perch furniture: roofs, word walls, towers) 0.5 with a time, Flying Attack (drive-by shout) 0.75.
  - Dragons circle, breathe on a fly-by, hover and breathe, perch, and land to bite and tail-swipe. At half health they crash-land and stay down. Dragonrend forces a landing. https://en.uesp.net/wiki/Skyrim:Dragon
  - Takeoff puts the creature in flight mode, where only its flap and glide clips play (modders).
- **[P] Minecraft's Ender Dragon** names its phases: CIRCLING, STRAFING (fly at the target, fire within 64 blocks), CHARGE_PLAYER, SEARCH_FOR_BREATH_ATTACK_TARGET, ROAR_BEFORE_ATTACK, BREATH_ATTACK, HOVER, LAND_ON_PORTAL, LEAVE_PORTAL. https://jd.papermc.io/paper/1.14.4/org/bukkit/entity/EnderDragon.Phase.html
- **[S] Elden Ring's flying dragons** (Agheel and kin).
  - **Fly-by breath:** it rises up and back as its mouth glows (the tell), then flies forward breathing down and to one side in a line. **Hover breath:** it rises, then breathes down in a wide cone.
  - **Ground breath:** a long sweeping cone whose aim follows the target for a couple of seconds. **Dive:** it falls on you and lies stunned after, the window to strike. https://gamerguides.com/elden-ring/guide/limgrave/bosses/how-to-beat-flying-dragon-agheel
- **[S] Dragon's Dogma.** Drakes breathe fire while pacing forward or backing off, and spew a horizontal arc. Wounding the wings brings one down. https://dragonsdogma2.wiki.fextralife.com/Drake , https://primagames.com/gaming/how-to-beat-a-drake-in-dragons-dogma-2
- **[P] Fixed-wing flight in Keeper** (Double Fine, GDC 2026). A bird flies by pathfinding, splines fitted to its turning radius, speed and climb, and animation timed to them. Fixed-wing flight is harder than free flight, because it cannot stop or turn in place. https://schedule.gdconf.com/session/fixed-wing-flight-in-keeper-pathfinding-locomotion-and-animation-of-a-bird-companion/913823

**What the physics says**

- **[P] Quetzalcoatlus** (Witton and Habib 2010): 10 to 11 m of span, 200 to 250 kg, 9.5 to 11.4 m² of wing.
  - Best glide at 13 to 25 m/s, minimum sink at 9 to 16 m/s.
  - It launched from all fours and flapped in about a minute's anaerobic burst, then soared. https://pmc.ncbi.nlm.nih.gov/articles/PMC2981443
- **[P] The heaviest soaring bird flaps almost never.** Andean condors flap 1% of the time, over 70% of that at take-off. One went five hours and 172 km without a beat. https://www.sciencedaily.com/releases/2020/07/200714111730.htm
- **[P] Bank angles.** Vultures circle at 25 to 35° of bank low in a thermal, less higher up (Williams et al. 2018). https://hal.umontpellier.fr/hal-02393999
  - A coordinated turn's radius is `r = v²/(g·tanφ)`. At 20 m/s that is 71 m at 30° and 41 m at 45°. At 30 m/s it is 159 m and 92 m.
  - The load is 1/cosφ: 1.4 g at 45°.
- **[I] Wingbeat rate.** Pennycuick's bird formula is `f = 1.08·m^(1/3)·g^(1/2)·b^(−1)·S^(−1/4)·ρ^(−1/3)`. It gives a mute swan 3.3 Hz (measured about 3) and Quetzalcoatlus 1.1 Hz.
  - **Our dragon:** a 15 to 20 m span at 1 to 4 t is about 0.8 to 0.9 Hz in cruise.
  - **Its stall:** with a lift coefficient of 1.2 it stalls near 20 to 30 m/s.
  - **Hovering** is beyond any animal this size: a paper finds Monster Hunter's Rathalos, at 1.71 Hz, below its 1.81 Hz hover minimum. A game's hover is a cheat, so make it short, with deep strokes at twice the cruise rate and a downwash that throws dust and fans fire. https://journals.le.ac.uk/index.php/pst/article/view/4547
- **[I] Believable flight.** Cruise is gliding with a few beats now and then; climbing and take-off are hard, continuous beating. It banks into every turn, rolling before it yaws, and never turns tighter than its radius at that speed. It pitches nose-up and spreads its wings to brake before landing or hovering. It strafes in a straight line low over the target and climbs away.

**How the breath is drawn** [I, from the games above and VFX practice]

- A cone or sweeping line of emissive flame cards from the jaw, after a glow in the mouth (the tell), with one moving light at the mouth and one where the flame lands. Hits are a capsule or cone test along the breath, not per particle.
- Scorch decals and burning cells where its line meets the ground or a building: the breath adds heat to cells and the fire model does the rest, rather than setting them burning outright.

## 4. Drawing flames, embers, smoke and light

- **[S] Flames** (realtimevfx practice). https://realtimevfx.com/t/looking-for-some-tips-for-creating-realistic-fire-effects-in-real-time-vfx/28143
  - A large rolling flipbook in the middle, smaller faster ones at the edges. Fewer cards with more variety beat many alike.
  - Flipbooks come from simulations (EmberGen, Houdini), with motion-vector textures to blend frames.
  - Soft particles fade cards where they meet geometry.
  - Many artists avoid pure additive blending, which washes reds out. They use alpha blending with emissive HDR raised.
  - Cards are pivoted at their base so flames grow up from the fuel.
- **[S] Embers and sparks** are small emissive points as stretched billboards, aligned with their velocity and lengthened by speed. They rise on the plume, drift with the wind and fall.
- **[P] Smoke** is dark cards above the flames, lit from below by the fire. Unity's six-way lighting bakes a smoke flipbook's light from six directions into two RGBA textures and blends them by the scene's lights at about a lit sprite's cost (EmberGen exports it). https://unity.com/blog/engine-platform/realistic-smoke-with-6-way-lighting-in-vfx-graph
- **[I] Heat haze** is a screen-space distortion over the flames, sampling the opaque scene's colour.
- **[I] Lights.** One flickering point light per cluster of fire, merged on a grid of about 4 m, its intensity by the fire's size and its flicker at the puffing frequency above. Shadowless but for the nearest one or two, since a point light's shadow is six views.
- **[S] Caps.** Every game caps fire: Far Cry 2 its propagation and particles, Minecraft by ticking fire only near players, Teardown by a count of fires removed by region, Divinity by burn time. **[I] For us:** a budget of burning cells drawn in full, distant fires as fewer, larger cards, and the spread itself bounded.

## Limits

GDC Vault's talks (Breath of the Wild's, Keeper's) were read only through their abstracts and press write-ups. Minecraft's spread odds come from the wiki and our reading of its code. No game publishes its dragon speeds or breath ranges. The wingbeat rate and stall speed come from bird formulas stretched far past their data. Heat release per area of a burning cube is assumed, not measured. Nothing here was tried in our game.
