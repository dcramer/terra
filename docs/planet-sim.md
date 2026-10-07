# Planet simulation

Status: the local-plus-global model is Decided (D6). Field details are Leaning until the simulation toy tests them.

## Two tiers

A planet can't be simulated at voxel resolution. So:

- **Coarse tier**: the whole planet as a few thousand cells on a sphere, each holding a handful of numbers. Always running, cheap, and the only thing that needs to stay in sync for the whole planet in co-op.
- **Fine tier**: voxel terrain, fluids, plants and creatures, realised only near players, driven by the coarse values of the cells they stand in.

The coarse tier is the truth. The fine tier is a local, detailed view of it.

## Fields

Rule of thumb: things that flow mix globally, things that are solid stay local.

| Field | Scope | Behaviour |
|---|---|---|
| Atmosphere (pressure, composition) | Global | Any machine anywhere feeds one pool. |
| Temperature | Global baseline plus local offset | Greenhouse effect sets the baseline. Latitude, altitude and heaters shift it locally. Diffuses to neighbouring cells. |
| Water | Global sea level, local ice | Ice sits on cells and melts when its cell warms. Meltwater raises one global sea level, which floods the lowest ground first. No flowing water (D24). |
| Biomass and soil | Local | Spreads slowly to neighbours when conditions allow. |
| Radiation | Global shield plus local sources | Deferred; add after the first four work. |

Start with heat, pressure, water and biomass. Oxygen is derived from biomass.

## From fields to a world

- **Biome** is chosen per cell from local temperature and moisture.
- **Life** is chosen from the biome and the phase.
- **Phases** are gated by global values.
- **Terrain appearance** follows the fields: tint from moisture and biomass, sky colour from pressure, sea level from total liquid water.
- **The solid ground never changes shape** from terraforming (D20). Only what sits on it changes: ice, sea, surface colour and plants.

The result: a base becomes a green patch that spreads, visible from orbit.

## How players act on it

- Machines change their own cell (heat, moisture, seeding) or the global pool (gas).
- Local changes diffuse to neighbours.
- Global values are the sum of local ones, so improving many regions is how you move the planet.

## First draft

A first draft of these rules is implemented in [prototypes/sim_toy](../prototypes/sim_toy), with three machines: a heater (local), a gas generator (global) and a seeder (local). The rules and numbers there are for tuning, not a specification.

## Constraints on the implementation

- Pure and headless: no rendering or engine dependencies, so it can run on a host or server and be tested alone.
- Deterministic for a given seed and input sequence.
- Cheap enough to fast-forward, for offline progress and for testing balance.

## Open questions

- Cell count and shape. The starting guess was a few thousand cells on a subdivided icosahedron; with the simulation in Rust, 40 thousand cells (about 35 m each on a 2 km planet) costs under half a millisecond a tick, so resolution can be chosen for gameplay.
- Tick rate, and whether day/night and seasons exist.
- Whether values can regress.
