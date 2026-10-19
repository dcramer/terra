# How structures collapse, in games and in the world

The question: what rules make a building's fall read as real, for every structure, after the castle film's towers and keep "tipped over as one" (the developer: "destruction of castle went wonky where it topples over it feels unrealistic"; "castle fall worked great in the past ... we just wanna make sure destruction system understand how to do realism there"). Briefly, from talks, documentation and physics; it informs D305, it decides nothing.

## Our own films

The sheets in `~/.cache/terra-review/destruction/sheets/` compare rounds. Before 2026-10-17 (`old`, `old-fall-a`, `r1-fall`) a tower whose support went came down where it stood: its lower courses broke first, it sank into a heap of stone, and its dust rolled out low round its foot. This is the one the developer remembers as better. From 2026-10-17 (`e3cb1ec7`, split out as `fragments::topple` in `ba44d248`) every fall 8 cubes tall and 120 cubes or more was hinged on the edge of its foot, leaning out from the base's middle, and went over as one rigid lattice until 17° at its top and 63° at its foot (`r4`, `r12-tower`, `r12-keep`). A tower read as a rigid slab of cubes, and the keep as a cube of cubes rolling. Its blocks also started 0.035 s a cube apart, which opens gaps of most of a cube at landing speed: a lattice.

## Shipped games

- **Red Faction: Guerrilla** (Geo-Mod 2.0, GDC 2009 and Volition's talks): buildings are pieces joined by links with a strength; each frame the load of what is above runs down the links to the ground, and a link under more stress than it can take breaks. A structure falls where it is weakest, under its own weight, so knocking out one side of a tower's foot brings that side down first and the rest follows, often sagging and breaking in the middle rather than tipping whole.
- **Teardown** (Tuxedo Labs' blog and talks): voxel volumes; when a cut leaves a piece unconnected it becomes its own rigid body, and its integrity is checked loosely so large spans break off. Its falls are rigid bodies colliding, so its debris piles up because the bodies collide with each other, and a big piece breaks again where it hits.
- **Frostbite: Battlefield** (Bad Company 2's Destruction 2.0 to Battlefield 4's Levolution, DICE talks): buildings are pre-fractured into hierarchical pieces; when enough supports are gone the building plays an authored collapse, mostly straight down into its footprint, hidden in a large, low dust cloud that rolls out along the ground, then leaves a rubble mesh. The dust and the heap do much of the work.
- **Unreal's Chaos** (Geometry Collections, documentation): pieces pre-fractured into clusters by Voronoi, each level with a damage threshold; a connection graph between pieces whose strain breaks them; anchor fields hold what touches the ground; clusters break up as they fall and land, so a falling section comes apart into ever smaller clusters rather than staying whole. Dust and debris through Niagara on break events.

Common ground: a fall comes from lost support and is decided by what still holds it; big pieces break up as they fall and land; the result is a heap; and dust, low and broad, covers the moment.

## The physics we lean on

- **Masonry has almost no tensile strength.** A tall body hinged at its foot (the falling chimney: Varieschi and Kamiya, "Toy models for the falling chimney", 2003) bends hardest a third of the way up, and a masonry chimney or tower cracks there within a few degrees of going over; each part then goes on as it was moving and turning, so the parts separate in the air. A tower does not go far over as one.
- **Even loss of support: it falls into its footprint.** A tower whose foot goes evenly (the Campanile of St Mark's, 1902; controlled demolition) comes straight down, its lowest courses landing first and breaking, what is over them coming down onto their heap; its walls peel outward as they go.
- **Tipping needs uneven support and slenderness.** A body tips only when what gave way is to one side of what is over it and it is tall against its depth; a squat block slumps.
- **Rubble bulks and stands at its angle of repose.** Broken masonry takes 1.3 to 1.6 times its standing volume and stands at about 35° to 40°, so a tower's heap is a cone a few metres high, which what falls later lands on.
- **Pieces tumble.** A block landing on a corner or an edge is turned over it by the impulse there (Δω = r × J / I); one landing flat is not.
- **Collapse dust is a gravity current.** The air a falling mass pushes out carries its dust as a current heavier than the air round it, rolling out along the ground at several metres a second and rising only later (the films of falling towers and demolitions).

## What this suggests for us

Our fragments are our own fixed-step physics until D33's engine lands (scene-rules stage 6). Within them, the rules above: support goes from the bottom up, the courses parting a hand's breadth; a fall tips only when its foot went on one side and it is slender, and then cracks off within its material's angle; otherwise it crumbles where it stood with its shell peeling outward; chunks sized by material; tumbling from landings; a heap with repose that later pieces land on; dust that rolls out low. Implemented in `terra_build::fragments` (`collapse.rs`, `rubble.rs`), D305 as amended on 2026-10-18.
