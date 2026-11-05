//! What a floor is laid with, made once (`Kit`), and the laying of an area with it, which only
//! places (`lay_with`): the load's area round the woods' middle, and each piece streamed round
//! the camera after it (`floor_stream`), so a piece costs its placing alone.

use super::*;
use bevy::platform::collections::HashSet;

/// A rock's or a boulder's scan, and the meshes of a log (its bark, its cut end, how long and
/// how thick) or of a stump (its bark, its cut end, how thick).
type Scan = (Handle<Mesh>, Handle<Mossy>);
type LogMeshes = (Handle<Mesh>, Option<Handle<Mesh>>, f32, f32);
type StumpMeshes = (Handle<Mesh>, Option<Handle<Mesh>>, f32);

/// What a floor is laid with: the rocks' and boulders' scans (each scan a material of its
/// own), the logs' and stumps' meshes, the cut ends' grain, and each kind of undergrowth's
/// variants and drifts, in `PLACEMENTS`' order. Making it rolls none of the laying's dice.
pub(in crate::experiments::lush) struct Kit {
    rocks: Vec<Scan>,
    boulders: Vec<Scan>,
    end_grain: Handle<StandardMaterial>,
    logs: Vec<LogMeshes>,
    stumps: Vec<StumpMeshes>,
    undergrowth: Vec<(Vec<Drawn>, Noise)>,
}

impl Kit {
    pub(in crate::experiments::lush) fn make(lay: &mut Lay) -> Kit {
        let rocks = (1..=7)
            .map(|n| lay.mossy_scan(&format!("mossy_rock_b_{n}"), 0.8))
            .collect();
        let mut boulders: Vec<_> = (1..=6)
            .map(|n| lay.mossy_scan(&format!("mossy_rock_a_{n}"), 1.0))
            .collect();
        boulders.push(lay.mossy_scan("boulder_1", 0.9));
        let end_grain = lay.end_grain();
        // Kept on the CPU too, for its footing to be fitted to (D267).
        let on_cpu = |bark: Build| {
            let mut mesh = bark.mesh();
            mesh.asset_usage = bevy::asset::RenderAssetUsages::all();
            mesh.generate_tangents()
                .expect("a log or stump has normals and texture places");
            mesh
        };
        let logs = (0..LOG_KINDS)
            .map(|kind| {
                let log = fallen_log(&mut lay.seed.derive("log").index(kind).rng());
                let bark = lay.meshes.add(on_cpu(log.bark));
                let end = log.end.map(|end| lay.meshes.add(end.mesh()));
                (bark, end, log.length, log.radius)
            })
            .collect();
        let stumps = (0..STUMP_KINDS)
            .map(|kind| {
                let mut dice = lay.seed.derive("stump").index(kind).rng();
                let stump = stump(&mut dice, kind.is_multiple_of(2));
                let bark = lay.meshes.add(on_cpu(stump.bark));
                let end = stump.end.map(|end| lay.meshes.add(end.mesh()));
                (bark, end, stump.radius)
            })
            .collect();
        lay.grow_all(&PLACEMENTS);
        let undergrowth = PLACEMENTS
            .iter()
            .map(|placement| {
                let variants = lay.grow(placement);
                let seed = lay.seed.derive(placement.name).index(placement.kind as u64);
                (variants, Noise::new(seed))
            })
            .collect();
        Kit {
            rocks,
            boulders,
            end_grain,
            logs,
            stumps,
            undergrowth,
        }
    }

    /// A kit of empty handles, one of each thing and one variant of each kind of undergrowth,
    /// for laying in a test with no assets to load.
    #[cfg(test)]
    pub(in crate::experiments::lush) fn bare() -> Kit {
        let scan = || vec![(Handle::default(), Handle::default())];
        Kit {
            rocks: scan(),
            boulders: scan(),
            end_grain: Handle::default(),
            logs: vec![(Handle::default(), Some(Handle::default()), 4.0, 0.25); LOG_KINDS as usize],
            stumps: vec![(Handle::default(), Some(Handle::default()), 0.3); STUMP_KINDS as usize],
            undergrowth: PLACEMENTS
                .iter()
                .map(|placement| {
                    let drawn = Drawn::one(Handle::default(), Look::Foliage(Handle::default()));
                    (vec![drawn], Noise::new(Seed(7).derive(placement.name)))
                })
                .collect(),
        }
    }
}

/// What a laying put: its things, the circles it filed as lying and as feet, and its stumps'
/// feet on the canopy, so a streamed piece can be taken off the floor again; its cuttable
/// plants by their place in its order of plants; and the counts for the log.
#[derive(Default)]
pub(in crate::experiments::lush) struct Put {
    /// The plants not to spawn, by their place in the order of plants: those cut before.
    pub cut: HashSet<u32>,
    pub things: Vec<Entity>,
    pub lying: Vec<(Vec2, f32)>,
    pub feet: Vec<(Vec2, f32)>,
    /// Each stump's foot and its trunk's width as the canopy files it (`Stand::stump`).
    pub stumps: Vec<(Vec2, f32)>,
    pub cuttable: Vec<(u32, Entity)>,
    /// Rocks, boulders, logs and stumps laid, and the nearest of each to the start.
    pub(super) wood: [u32; 4],
    pub(super) nearest: [Vec2; 4],
    /// Each kind of undergrowth's count, and the nearest to the start.
    pub(super) undergrowth: Vec<(Under, u32, Vec2)>,
    plants: u32,
}

impl Put {
    fn note(&mut self, what: usize, at: Vec2) {
        self.wood[what] += 1;
        if self.nearest[what] == Vec2::ZERO || at.length() < self.nearest[what].length() {
            self.nearest[what] = at;
        }
    }

    fn lies<W: Ways>(&mut self, map: &mut Floor<W>, at: Vec2, radius: f32) {
        map.lying.add(at, radius);
        self.lying.push((at, radius));
    }
}

/// Lays an area with a kit, by `dice`: rocks and boulders, fallen logs, stumps, then the
/// undergrowth, in that order, as the forest walk always has.
pub(in crate::experiments::lush) fn lay_with<W: Ways>(
    lay: &mut Lay,
    kit: &Kit,
    map: &mut Floor<W>,
    area: &Area,
    dice: &mut Rng,
    put: &mut Put,
) {
    rocks(lay, kit, map, area, dice, put);
    logs(lay, kit, map, area, dice, put);
    stumps(lay, kit, map, area, dice, put);
    undergrowth(lay, kit, map, area, dice, put);
}

/// One rock or boulder at a place, if the place holds one.
fn rock<W: Ways>(
    lay: &mut Lay,
    map: &mut Floor<W>,
    (kit, area): (&Kit, &Area),
    dice: &mut Rng,
    put: &mut Put,
    at: Vec2,
    boulder: bool,
) {
    let here = map.here(at);
    if here.path < 1.2 || here.bark < 0.8 || here.lying < 0.2 || !area.holds(&here) {
        return;
    }
    let (scale, kinds, size) = if boulder {
        (dice.range(0.6, 1.3), &kit.boulders, 1.2)
    } else {
        // Many small stones and a few big ones.
        let u = dice.unit();
        (0.4 + 1.8 * u * u, &kit.rocks, 0.35)
    };
    // Most sit a fifth to two fifths in the ground; a third are half buried or more, only
    // their mossy backs showing, as hummocks of the floor.
    let sink = if dice.unit() < 0.35 {
        dice.range(0.45, 0.75)
    } else {
        dice.range(0.2, 0.4)
    };
    let place = lay.on(at, dice, 0.0, scale);
    let (mesh, material) = kinds[dice.below(kinds.len() as u32) as usize].clone();
    if lay.off(area, &place) {
        return;
    }
    let thing = lay.commands.spawn((
        Name::new(if boulder { "Boulder" } else { "Rock" }),
        Mesh3d(mesh),
        MeshMaterial3d(material),
        place,
        within(if boulder { 160.0 } else { 80.0 }),
        Footing,
        Sink(sink),
    ));
    put.things.push(thing.id());
    put.lies(map, at, size * scale * 0.6);
    put.note(boulder as usize, at);
}

/// Rocks and boulders, the rocks scattered round the boulders and alone; sunk a fifth to two
/// fifths of their height once their meshes are loaded (`Sink`).
fn rocks<W: Ways>(
    lay: &mut Lay,
    kit: &Kit,
    map: &mut Floor<W>,
    area: &Area,
    dice: &mut Rng,
    put: &mut Put,
) {
    for _ in 0..area.tries(70, false, dice) {
        let at = area.place(dice);
        let skip = area.skips(at);
        if !skip {
            rock(lay, map, (kit, area), dice, put, at, true);
        }
        // Stones broken from it round its foot.
        for _ in 0..dice.below(5) {
            let near = round(dice, at, 2.2);
            if !skip {
                rock(lay, map, (kit, area), dice, put, near, false);
            }
        }
    }
    for _ in 0..area.tries(1400, false, dice) {
        let at = area.place(dice);
        // More in the shade, where the soil is thin over them.
        if area.skips(at) || dice.unit() > 0.4 + 0.6 * (1.0 - map.here(at).light) {
            continue;
        }
        rock(lay, map, (kit, area), dice, put, at, false);
    }
}

/// Fallen logs, under the trees most, each from the forest's own trees: tapering, broken at
/// the crown's end, its stubs of branches, settled into the ground.
fn logs<W: Ways>(
    lay: &mut Lay,
    kit: &Kit,
    map: &mut Floor<W>,
    area: &Area,
    dice: &mut Rng,
    put: &mut Put,
) {
    for _ in 0..area.tries(180, false, dice) {
        let at = area.place(dice);
        if area.skips(at) {
            continue;
        }
        let here = map.here(at);
        if dice.unit() > 0.25 + 0.75 * (1.0 - here.light) || !area.holds(&here) {
            continue;
        }
        let (bark_mesh, end_mesh, length, radius) =
            kit.logs[dice.below(LOG_KINDS as u32) as usize].clone();
        let scale = dice.range(0.85, 1.15);
        let place = lay.on(at, dice, 0.12 * scale, scale);
        let along = lay.on_map(place.rotation * Vec3::X);
        let half = length * scale * 0.5;
        // Clear of the paths, the trunks and what else lies there, all along it.
        let steps = (half / 0.8).ceil() as i32;
        let clear = (-steps..=steps).all(|k| {
            let p = at + along * (k as f32 / steps as f32) * half;
            let h = map.here(p);
            h.path > 1.0 && h.bark > 0.6 && h.lying > 0.3
        });
        if !clear || lay.off(area, &place) {
            continue;
        }
        for k in -steps..=steps {
            put.lies(
                map,
                at + along * (k as f32 / steps as f32) * half,
                radius * scale,
            );
        }
        // Of the trees round it.
        let bark = if here.needles > 0.5 {
            lay.pine_bark.clone()
        } else {
            lay.bark.clone()
        };
        let mut log = lay.commands.spawn((
            Name::new("Log"),
            Mesh3d(bark_mesh),
            MeshMaterial3d(bark),
            place,
            within(110.0),
            Footing,
        ));
        if let Some(end) = end_mesh {
            log.with_child((
                Mesh3d(end),
                MeshMaterial3d(kit.end_grain.clone()),
                within(110.0),
            ));
        }
        put.things.push(log.id());
        put.note(2, at);
    }
}

/// Stumps of trees cut or broken long ago, each its foot in duff and roots as a tree's.
fn stumps<W: Ways>(
    lay: &mut Lay,
    kit: &Kit,
    map: &mut Floor<W>,
    area: &Area,
    dice: &mut Rng,
    put: &mut Put,
) {
    for _ in 0..area.tries(110, false, dice) {
        let at = area.place(dice);
        if area.skips(at) {
            continue;
        }
        let here = map.here(at);
        if here.path < 1.0
            || here.bark < 1.2
            || here.lying < 0.5
            || !area.holds(&here)
            || dice.unit() > 0.3 + 0.7 * (1.0 - here.light)
        {
            continue;
        }
        let (bark_mesh, end_mesh, radius) =
            kit.stumps[dice.below(STUMP_KINDS as u32) as usize].clone();
        let scale = dice.range(0.8, 1.25);
        let place = lay.on(at, dice, 0.0, scale);
        if lay.off(area, &place) {
            continue;
        }
        let bark = if here.needles > 0.5 {
            lay.pine_bark.clone()
        } else {
            lay.bark.clone()
        };
        let mut stump = lay.commands.spawn((
            Name::new("Stump"),
            Mesh3d(bark_mesh),
            MeshMaterial3d(bark),
            place,
            within(90.0),
            Footing,
        ));
        if let Some(end) = end_mesh {
            stump.with_child((
                Mesh3d(end),
                MeshMaterial3d(kit.end_grain.clone()),
                within(90.0),
            ));
        }
        put.things.push(stump.id());
        map.canopy.add(Stand::stump(at, radius * scale * 1.3));
        put.stumps.push((at, radius * scale * 1.3));
        put.note(3, at);
    }
}

/// The undergrowth, kind by kind, in drifts. A plant cut before (`Put::cut`) has its dice
/// rolled as ever, so everything else stays where it was, and is not spawned.
fn undergrowth<W: Ways>(
    lay: &mut Lay,
    kit: &Kit,
    map: &mut Floor<W>,
    area: &Area,
    dice: &mut Rng,
    put: &mut Put,
) {
    for (placement, (variants, drift)) in PLACEMENTS.iter().zip(&kit.undergrowth) {
        let clear = placement.kind.clear_of();
        let mut count = 0;
        let mut closest = Vec2::splat(f32::MAX);
        for _ in 0..area.tries(placement.tries, true, dice) {
            let Some(at) = placement.kind.tried_at(&map.ways, area, dice) else {
                continue;
            };
            if area.skips(at) {
                continue;
            }
            let drifts = smoothstep(
                0.38,
                0.62,
                0.5 + 0.5 * drift.fbm(Vec3::new(at.x, 0.0, at.y) / placement.drift_m, 2, 2.0, 0.5),
            );
            if !lay.starts(placement.kind, at, map.here(at), dice.unit(), drifts) {
                continue;
            }
            let many = placement.clump.0 + dice.below(placement.clump.1 - placement.clump.0 + 1);
            for k in 0..many {
                let p = if k == 0 {
                    at
                } else {
                    round(dice, at, placement.spread_m)
                };
                let member = lay.member(placement.kind, p, map.here(p), clear);
                let Some(here) = member.filter(|here| area.holds(here)) else {
                    continue;
                };
                // Smaller toward the clump's edge.
                let edge = 1.0 - 0.3 * (p.distance(at) / (placement.spread_m * 2.0)).min(1.0);
                let scale = dice.range(placement.scale.0, placement.scale.1) * edge;
                let place = lay.on(p, dice, 0.03 * scale, scale);
                let pick = dice.below(variants.len() as u32) as usize;
                // Rolled as before, so every other plant stays where it was, then left out if
                // it would stand in the path's line.
                if !placement.kind.keeps_path_line(here.path) || lay.off(area, &place) {
                    continue;
                }
                let n = put.plants;
                put.plants += 1;
                if put.cut.contains(&n) {
                    continue;
                }
                if let Some((plant, cuttable)) = lay.plant(placement, &variants[pick], p, place) {
                    put.things.push(plant);
                    if cuttable {
                        put.cuttable.push((n, plant));
                    }
                }
                if placement.kind.stands_in_a_well() {
                    map.feet.add(p, FOOT_M * scale);
                    put.feet.push((p, FOOT_M * scale));
                }
                count += 1;
                if p.length() < closest.length() {
                    closest = p;
                }
            }
        }
        put.undergrowth.push((placement.kind, count, closest));
    }
}
