# Creature rigs: from a generated mesh to a creature that moves well

Status: **proposal, 2026-10-22**, for the developers to read before anything is built (D49). The pipeline is technical and the agents' to propose (D439 to D443, Leaning); the calls on scope and look are design, Open (D436 to D438), and listed at the end. The research behind it is [research/2026-10-22-creature-rig-pipeline.md](../research/2026-10-22-creature-rig-pipeline.md). How a creature moves once it is in the game (intent, mover, fix-up) is [movement.md](movement.md); this doc is how its body, skeleton, weights and clips are made, and how a machine tells a good one from a bad one.

The developer, 2026-10-22: "i want to have stable, very good predictrable pipeline that if you start generating some animals/monsters, we know they will generally be good."

## Today

- **People** have that pipeline ([characters.md](characters.md), the `character` skill, D115): a design sheet, a Tripo model and auto-rig in Unreal's skeleton, converted onto ours, and clips made with Kimodo (D316, D347, D360). It is checked by numbers: `rig_check.py` (bones, weights, seams, reach, symmetry, stretch, pinch at the joints), `deform_check.py` (squeeze, flatten and twist of skin rings along every limb through every clip) and `gait_check.py` (foot slide under 1 cm, lean, hands). A bad body fails a check before anyone looks.
- **Animals** have half of one. `tools/creatures/animal.py` makes 16 kinds from Tripo meshes by two body plans, a four-legged runner and a bird; `dragon.py` and `hen.py` are scripts of their own; the fliers fly by D382. Gaits are made well, from measured footfalls and a real animal's stride at the speed asked (D424). But:
  - **The skeleton is placed by hand**: each kind's `creature.json` lists every joint's head and tail in model units, typed in by an agent.
  - **The weights are Blender's bone heat** on a scaled copy, a voxel copy when heat leaves skin bare, with hand fixes per region added after a failure (`haunch` for the deer's rump).
  - **Nothing measures the result.** None of the three checks above runs on a creature; `sheet.py` renders a sheet for an eye. So the animals were fixed one report at a time: the stag's carpus, the fox's level chest, the hare's low stance, now the stag's belly.
  - Creatures are skinned with the plain linear blend; the game has no foot IK, so feet float or sink on a slope.
- **The stag's belly, measured** (2026-10-22, the research note): of the skin under the trunk between the fore and hind legs, the stag gives 52% of its weight to leg bones (35% upper arms, 16% thighs, 8% shins) and the boar nearly all of it; through the gallop the 99th percentile of edge stretch there is 118% on the stag and 194% on the boar. Quaternius's hand-rigged deer gives 93% of its belly to the trunk's bones and stretches 10.5% in its gallop. Bone heat gives skin to the nearest bone, and under a deep chest the nearest bone is the top of a leg: the belly swings with the legs. It is not one stag's bug; it is what bone heat does to every deep-bodied quadruped, and would do to every monster.

## What we keep from shipped games

- **Labelled parts, motion carried to any body** (Spore, 2008): each bone tagged by what it can do (spine, foot, mouth, grasper), motion authored once against those tags, legs grouped by length and given a duty factor and a phase each. Unreal's IK Retargeter carries motion between skeletons by labelled chains (spine, leg, tail, tentacle) the same way.
- **Tested over many bodies, not one** (Spore): its animations were run against a changing set of several hundred players' creatures and scored on a grid, about 90% passing. That is the nearest published precedent for a pipeline that is "generally good", and the bar this one aims at.
- **A range-of-motion pass before a rig is accepted** (rigging practice; Unreal's ML Deformer trains on 5,000 to 25,000 random poses within the joints' limits): the body put through its joints, the bad bends fixed. No studio publishes thresholds; they judge by eye. We measure.
- **Weights from a good template, not from scratch** (Robust Skin Weights Transfer, Epic, 2023, MIT code): weights copied from a good body where the closest point is near and its normal agrees, the rest filled smoothly.
- **Procedural gaits from biomechanics** (Spore, Overgrowth, The Flame in the Flood; Alexander's dynamic similarity): footfalls, duty factor and stride from the leg's length and the speed, so a body twice as big walks as a real one would.
- **Foot IK and terrain adaptation at runtime** (Far Cry 4's wildlife: additive layers, ground probes, IK and physics over captured clips): already planned in movement.md's fix-up layer.

## The pipeline

Five stages, each a tool with a measured gate, each writing what it decided into the kind's folder so a rerun is the same (D71).

### 1. Body plans as data (D436, D439)

A plan is a file (`art/creatures/plans/<plan>.json`), not code: its chains and their roles (spine, neck, tail, a leg with its kind and side, a wing, a fin, an arm), each chain's bone count and the share of the chain each bone takes, each joint's limits, the weight zones (which bones may move which region of skin, below), the poses of its pose sheet, the gaits it may use, and the thresholds its check applies. A kind's `creature.json` names its plan and overrides only what differs: a stag is a quadruped whose fore legs are unguligrade (three segments below the shoulder blade, so the carpus is the plan's, not a fix), a hare one whose hind feet lie flat.

| Plan | Chains | Moved by | Today |
|---|---|---|---|
| Biped (people, trolls, ogres) | Our skeleton (D115) | Kimodo clips, carried by aim (D316, D360) | Done, checked |
| Quadruped | Spine, neck, tail; four legs, each plantigrade, digitigrade or unguligrade; shoulder blades | Made gaits (D424) | 10 kinds, unchecked |
| Avian | Spine, neck, tail; two legs; two wings, folded or spread | Made gaits and flight clips (D382) | 6 kinds, the hen, the dragon |
| Serpent | One long spine, head | The spine following the head's path | None |
| Many-legged | Body segments; 6 or 8 legs of three segments | Legs stepping by rule when they pass their reach, in alternating groups | Quaternius's spider only |
| Swimmer | Spine with a travelling wave, fins | The wave, growing toward the tail | None |

**Composites** are a plan with chains grafted at a socket: the Worman is a serpent with two arms, the Seawish a swimmer stood on its tail with two arms, the Drillhog a quadruped with spinning parts. A grafted arm takes humanoid arm clips through a mask (movement.md).

**What does not fit** a skeleton plan, and is made another way: a slime or blob (shape keys or a soft body), a swarm (many small instances, flocking), a mass of tentacles (spline IK chains, more like a serpent per tentacle than one rig), a creature that splits or reassembles, a monster with no fixed shape.

### 2. Skeleton fitting (D440)

The skeleton is fitted to the mesh by the plan, not typed in. As Pinocchio fits a template skeleton into a mesh's interior: the mesh is cut into cross-sections along its length; the limbs are found as the separate blobs of the low sections and the fins and wings as thin outliers of the wide ones; each chain of the plan is matched to one, by the plan's order (front to back, left to right); joints go at the waist of each limb where its width narrows (an elbow, a knee, a hock), else at the plan's share of its length; spine and neck run through each section's centre. Tripo's auto-rig is a second opinion, its joints a starting guess: its current rig claims quadruped, hexapod, octopod, avian, serpentine and aquatic bodies (a vendor's claim, untried here). Learned rigging (UniRig, 2025, MIT code, its weights' licence unstated) is watched, not built on: its authors say results degrade sharply when its skeleton is wrong, and it gives no score of its own.

Scored before anything else: every bone inside the mesh and near its section's centre; every leg's foot at the ground; mirrored joints within 1 cm of mirrored; the proportions within the plan's range. A fit outside its score goes to a sheet with the joints drawn on the mesh and stops; the agent fixes it by an override in `creature.json`, which is today's file, now only the exceptions.

### 3. Automatic weights (D441)

- **First, from the plan's template body**: each plan has one body whose weights an agent made carefully and the check passed (the stag after its belly is fixed, for the quadruped). A new body, posed onto the template's skeleton, takes the template's weights at its closest surface point where the normals agree, and the rest is filled smoothly (Robust Skin Weights Transfer: within 5% of the body's diagonal and 35° of normal; its MIT code ported into the Blender script, named there, D196). This is how the belly stays with the spine: the template's belly does.
- **Second, generated**: bone heat on a closed voxel copy, as now, or geodesic voxel binding (the distance through the body's inside, as Maya binds), then the plan's **zones** applied: the underside between the legs may be moved only by the spine and chest, fading into the leg below the armpit and the flank; the rump by the pelvis (today's `haunch`, now the plan's); the neck's base by neck and chest. Then smoothed within the zones, at most four bones a vertex, normalised.
- Both are made; the check below keeps the one that scores better. Bounded biharmonic weights are the best published method but need a tetrahedral mesh of the body, which generated meshes often cannot give; a later step if the two above fall short.
- **Skinning**: creatures get the game's weight-blended dual quaternions (`terra_walker::skinning`) where a plan says a joint twists (a neck, a tail), linear blend elsewhere.

### 4. Gaits from body size (D443)

Today's gait maker stays (D424): footfalls and duty factor by gait, stride from Alexander's relative stride. What changes is that the paces come from the body, not a table: by dynamic similarity, animals of any size change gait at the same Froude number, v²/(g h) on the hip's height h, walk to trot near 0.5 and trot to gallop near 2 to 3 (Alexander and Jayes 1983; a robotics summary gives 0.3 to 0.6 and 1.1 to 1.7 for sheep, dogs and horses, so the bands are calibrated on our own paced kinds, D424, before any kind relies on them; it holds for running mammals, less for climbers and primates). So a kind with no measured speeds gets its walk, trot and run speeds, and clips, from its hip height and its plan: a 3 m-hipped monster trots from about 3.8 m/s, a hare-sized one from 0.9. A plan caps the Froude number a size may reach (an elephant never gallops), and a kind may set its own (a monster that charges). Many-legged plans step a foot when it passes its reach, in alternating groups (tripod for six, two groups of four for eight); serpents and swimmers move a wave whose speed sets their pace. Clips stay made in Blender and baked, in place; foot IK and stride warping in the game (movement.md) do the rest.

### 5. The creature check (D442)

One command, `tools/creatures/creature_check.py <kind>`, run when a kind is made or changed, writing a score and a sheet, gating its landing as the rig check gates a body. It runs everything the people's checks run that applies to any skeleton (weighted, four bones, normalised, seams, reach, symmetry, ground), and on the plan's **pose sheet** it measures what a rigger looks for in a range-of-motion pass:

| Measure | On | What it catches | Bar, to calibrate |
|---|---|---|---|
| Volume loss | Each bone's region of skin, its enclosed volume against rest | A collapsing elbow, hip or neck | Within 10% |
| Ring squeeze and flatten | Rings across each limb, neck and tail (`deform_check.py`'s) | A joint pinching to a ribbon | Squeeze under 0.25 |
| Candy-wrapper twist | The same rings, the skin's turn against the bone's | A forearm or neck wringing | Under 30° past the bone's own |
| Stretch | Edge length, 99th percentile, per zone (belly, shoulder, groin, neck base, rump) | The stag's belly | Under 25% in gaits, 40% at limits |
| Folds and crossing | Triangles turned inside out; skin passing through other skin | A leg through the belly, a crease | None in gaits |
| Weight creases | The change of weight across an edge | Today's `haunch` failure | Under the plan's limit |
| Foot slide | A planted foot's drift in each gait clip (`gait_check.py`'s) | Skating | Under 1 cm |
| Feet and ground | Lowest point through each clip | Sinking, floating | Within 1 cm |
| Rate | Each clip's pace in its band (D424) | Legs too quick | 0.8 to 1.2 |

The bars are set against **known-good rigs**, measured by the same tool: Quaternius's hand-rigged deer and spider, which we ship, and any quadruped we buy for reference. A creature passes when every measure is inside its bar; the score is how far inside. The sheet shows each failing measure's worst moment, close, coloured by the measure, so the fix is obvious; it is for the agent, not a gate a person must pass. A kind that passes then gets one adversarial look at its gait film beside reference footage (Muybridge's plates, a nature film), as every visual change does.

## Where we depart from shipped games

- **No hand-painted weights per creature.** Studios paint every creature's weights and fix them over a range-of-motion pass by eye. We paint one body per plan and gate the rest by numbers. A hero creature may still be finished by hand (D437).
- **No capture, no motion matching for animals.** AAA quadrupeds are captured clips with layers on top (Far Cry 4's wildlife, Red Dead Redemption 2's horses); The Last of Us Part II's dogs are the one confirmed motion-matched quadruped, captured in suits. Each needs a stage and tens of minutes of a species. We have hundreds of kinds and no stage, and every research capture of dogs or horses is non-commercial (AI4Animation's, CC BY-NC); so gaits are made from biomechanics, as Spore and The Flame in the Flood made theirs. Bought clips (Truebones, royalty-free, not to be redistributed) serve as reference to calibrate against, not as the source.
- **Clips baked per kind, not retargeted live.** Spore fitted its motion to each body at runtime with its own particle IK; we bake each kind's clips offline in Blender, where they can be checked, and keep runtime to the fix-up layer.
- **Deformation measured, not looked at.** No studio publishes a deformation metric or a range-of-motion bar; the measures above come from the papers (local volume, edge length) and our people's checks, and are calibrated on known-good rigs.

## How good it would get, honestly

An estimate, unmeasured until the first round runs: the share of newly generated kinds that pass the check without hand work and read as "generally good" (a harsh review at 7/10 or more for its gait), once the stages above exist. Spore's ~90% over several hundred creatures is the anchor; ours start from generated meshes, not parts Maxis built to fit, so most failures come from the mesh, not the rig: fused legs, a wing grown into the body, a tail merged with a leg.

| Plan | Generally good | What fails |
|---|---|---|
| Biped | 85 to 90% (today's people) | Long robes and big gear, which the gear pipeline handles |
| Quadruped, mammal | 75 to 85% | Legs fused in the mesh, very long necks or horns at the shoulder, manes and shaggy fur |
| Serpent, swimmer | 80 to 90% | Fins and frills fused to the body |
| Avian | 60 to 70% | Folded wings modelled into the body; flight needs per-kind tuning (D382) |
| Many-legged | 55 to 70% | Thin legs fused or missing in generated meshes; the leg count misread |
| Composites (Worman, Seawish, Drillhog) | 40 to 60% | Where the graft goes is a call; a socket picked by hand each |
| Outside the plans | 0% | Made another way |

**What stays hand work:** approving each concept (the design sheet), each plan's template body once, a hero creature's signature moves (an attack, a roar, a special idle), faces and mouths that speak, corrective shapes on a hero, secondary motion of fur, manes and feathers (spring bones, later), and any kind that fails its check twice, which gets a new mesh before it gets more fixes.

## Order of work

1. The creature check on today's 16 kinds, the hen and Quaternius's deer and spider, to calibrate the bars and see the failures in numbers. The stag's and boar's bellies are the first to fail.
2. The quadruped plan as data, and its template body (the stag fixed by zones until it passes).
3. Weight transfer from the template, and the zones fallback, on every quadruped; the check keeps the better.
4. Skeleton fitting for the quadruped, its result against today's hand-placed joints.
5. Paces from Froude, for kinds without measured speeds.
6. Then each further plan, a template body each, in the order monsters need them.

## Open questions for the developers

1. **Which plans, which first** (D436): the six plans and composites above, and serpent or many-legged next for the monsters?
2. **How good is good enough, and who gets hand work** (D437): is "passes the check and 7/10 on a harsh look" the bar for the long tail, with hero creatures (the dragon, the troll, bosses) finished by hand?
3. **Realistic or exaggerated monsters** (D438): do monsters move by real biomechanics as the wildlife does (D122), heavy things slow, or do some move as no animal could, by rules of their own?
4. **Buying reference**: may we buy a quadruped clip library licensed for games (Truebones' zoo, about $100, royalty-free with credit, files not to be redistributed), to calibrate the check and tune gaits against, not to ship as is?
5. **How many kinds a plan should make** (D182): how many distinct quadruped monsters a realm wants, which sets how much effort a template body is worth.
