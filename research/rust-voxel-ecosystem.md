# What exists in Rust for a voxel planet game

2026-10-05 and 2026-10-06. Question: is there a Bevy-based or other Rust voxel engine we could build on, and are the supporting libraries current?

## Answer

There are good parts, but no equivalent of Voxel Tools: nothing that gives a diggable smooth-voxel planet with level of detail from orbit to ground. The system that ties the parts together is ours to write.

## Voxel-specific

| Piece | Project | State |
|---|---|---|
| Smooth meshing | [fast-surface-nets](https://github.com/bonsairobo/fast-surface-nets-rs) | MIT. Small and stable; last updated January 2025. |
| Smooth meshing with seams between detail levels | [transvoxel](https://gnurfos.github.io/transvoxel_rs/) 2.0 | Apache-2.0. Last updated November 2025. Marked experimental by its author. |
| Voxel world framework | [bevy_voxel_world](https://github.com/splashdust/bevy_voxel_world) | Active, supports Bevy 0.19, but built for blocky cubes on flat infinite worlds. |
| Smooth sculpting | [bevy-sculpter](https://crates.io/crates/bevy-sculpter) | One person's project with a few hundred downloads. |
| Smooth voxel planets | [Voxanet](https://github.com/verlorengest/voxanet), [Minos](https://github.com/owenyuwono/minos) | Hobby projects with days or weeks of commits and a handful of stars. Voxanet has no licence. |
| Heightmap planets | [bevy_terrain](https://github.com/kurtkuehnert/bevy_terrain) | Not voxels, so not diggable. Last pushed April 2025. |
| Older frameworks | [feldspar](https://github.com/bonsairobo/feldspar), building-blocks | Abandoned in 2022. |
| A shipped Rust voxel game | [Veloren](https://github.com/veloren/veloren) | Large and active, but blocky, GPL-3.0, and a game rather than a library. |

## Supporting libraries, versions on crates.io on 2026-10-06

Bevy's stable release is 0.19.1 (August 2026). 0.20 is at release candidate 2.

| Need | Crate | Version | Supports Bevy 0.19? |
|---|---|---|---|
| Physics | avian3d | 0.7.0 | Yes |
| Physics, alternative | bevy_rapier3d | 0.36.0 | Yes |
| Solar-system coordinates | big_space | 0.12.0 | No, 0.18 only |
| Networking transport | bevy_renet | 5.0.0 | Yes |
| Networking, full replication | lightyear | 0.30.1 | Yes |
| Networking, replication | bevy_replicon | 0.44.3 | Yes |
| Steam | bevy_steamworks | 0.17.0 | Yes |
| Developer UI | bevy_egui | 0.43.0-rc.1 | The release candidate targets 0.20; an earlier release covers 0.19 |
| Inspector | bevy-inspector-egui | 0.37.0 | Yes |
| Audio | bevy_seedling | 0.8.0 | Yes |
| Particles | bevy_hanabi | 0.19.0 | Yes |
| Character controller | bevy_tnua | 0.32.0 | Yes |
| Input | bevy_enhanced_input | 0.26.0 | Yes |
| Portable maths | libm | 0.2.16 | Not Bevy-specific |

Bevy itself now includes atmospheric scattering (0.18 added a configurable scattering medium, described as suitable for other planets), a new scene system and more UI widgets (0.19).

## Limits

- Versions and activity were read from crates.io and GitHub, not tested. Only Bevy and `transvoxel` have been built here.
- The hobby planet projects were judged from their repository pages, not their code.
- Whether Bevy's atmosphere works on a 2 km planet seen from orbit is unknown.

## Sources

- [Bevy 0.19 release notes](https://bevy.org/news/bevy-0-19/)
- [Bevy 0.18 release notes](https://bevy.org/news/bevy-0-18/)
- [bevy_voxel_world](https://github.com/splashdust/bevy_voxel_world)
- [Voxanet](https://github.com/verlorengest/voxanet)
