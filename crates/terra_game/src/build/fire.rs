//! Fire in the player's base (D377): its fire (`terra_build::fire`) stepped on its own tick,
//! its harm sent as any harm is, through the base's acts, so what burns through brings down
//! what it held, breaking into fragments as any loss does; what a fire reaches blackened;
//! and what burns given to the world's drawing of fire (`terra_world::fire`) as blazes, two
//! metres' cubes a side to a blaze, taller where more burns round it.
//!
//! What sets a base alight is anything that breathes fire ([`BreathesFire`]): a cone of
//! flame from its mouth, cast into the base as rays that heat what they meet, as a breath in
//! shipped games is tested along its cone rather than by its particles; and, over the remote,
//! `fire.light` and `fire.heat`, with `fire.state` saying how it burns.

use super::Site;
use super::act::Building;
use crate::play::hazards::{BreathesFire, Frightful};
use crate::remote::{method, refused};
use bevy::{
    ecs::system::SystemState,
    prelude::*,
    remote::{BrpResult, builtin_methods::parse_some},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::time::Instant;
use terra_build::fire::{Burns, Fire, TICK_S};
use terra_build::ops::Op;
use terra_build::{CELL_M, Kind};
use terra_world::climate::ClimateFields;
use terra_world::daylight::Clock;
use terra_world::fire::{Blaze, Blazes, Jet};

/// The base's fire, what each kind burns as, and its blazes as last worked out.
#[derive(Resource, Default)]
pub struct SiteFire {
    pub fire: Fire,
    burns: BTreeMap<Kind, Burns>,
    health: BTreeMap<Kind, u16>,
    /// Game time not yet stepped, in seconds.
    owed_s: f32,
    blazes: Vec<Blaze>,
    /// Where each breath's stream meets the base this frame, in the world: a flare drawn
    /// there, as a flamethrower's stream splashes where it strikes.
    splashes: Vec<Vec3>,
    /// How long the last ticks took, in milliseconds: the fire's own work, and its acts'.
    pub step_ms: f32,
    pub act_ms: f32,
    /// Ticks since the fire was last said in the log.
    since_said: u32,
}

/// How many rays a breath casts a frame, and how far round the cube a ray meets its heat
/// reaches, in cubes.
const RAYS: usize = 14;
const SPLASH: i32 = 2;
/// How far short of what a breath meets its splash is drawn, and how far past it the
/// stream runs on into it, in metres; and the splash's flame, how tall and how wide: a sheet
/// broader than a flame so tall, as the stream spreads over what it strikes.
const SPLASH_BACK_M: f32 = 0.8;
const SPLASH_TALL_M: f32 = 5.5;
const SPLASH_WIDE_M: f32 = 4.0;

pub fn plugin(app: &mut App) {
    app.init_resource::<SiteFire>()
        .add_systems(Update, (kinds, breathe, burn, blazes).chain());
    method(app, "fire.light", light);
    method(app, "fire.heat", heat);
    method(app, "fire.state", state);
}

/// What each kind burns as and how much health it has, from the kits, once they are read.
fn kinds(
    look: Option<Res<super::BuildLook>>,
    rules: Res<super::act::BuildRules>,
    mut fire: ResMut<SiteFire>,
) {
    if !fire.burns.is_empty() {
        return;
    }
    let Some(kits) = look.and_then(|look| look.kits.clone()) else {
        return;
    };
    fire.burns = kits.burning();
    fire.health = kits
        .0
        .iter()
        .map(|kit| (kit.kind(), rules.rules.toughness(kit.kind()).health))
        .collect();
}

/// Each thing breathing fire: its jet drawn, and its cone cast into the base, heating what
/// it meets and what is round that.
fn breathe(
    time: Res<Time>,
    breathers: Query<(&BreathesFire, &GlobalTransform)>,
    site: Res<Site>,
    mut fire: ResMut<SiteFire>,
    mut blazes: ResMut<Blazes>,
    mut turn: Local<u32>,
) {
    blazes.jets.clear();
    let step = time.delta_secs();
    let to_base = site.frame.compute_affine().inverse();
    let SiteFire {
        fire,
        burns,
        splashes,
        ..
    } = &mut *fire;
    splashes.clear();
    let burns_as = |kind: Kind| burns.get(&kind).copied();
    for (breath, at) in &breathers {
        let from = at.translation();
        let way = breath.aim.map_or(at.forward().as_vec3(), |aim| {
            (aim - from).normalize_or(at.forward().as_vec3())
        });
        let origin = to_base.transform_point3(from);
        let ahead = to_base.transform_vector3(way).normalize_or_zero();
        // The stream ends where its middle meets the base, and splashes there.
        let struck = breath
            .on
            .then(|| site.base.raycast(origin, ahead, breath.reach_m))
            .flatten();
        if let Some(hit) = &struck {
            splashes.push(from + way * (hit.distance - SPLASH_BACK_M).max(0.0));
        }
        blazes.jets.push(Jet {
            from,
            way,
            reach: struck
                .as_ref()
                .map_or(breath.reach_m, |hit| hit.distance + SPLASH_BACK_M),
            spread: breath.spread,
            on: breath.on,
            face: struck.as_ref().map_or(0.0, |hit| hit.distance),
            normal: struck.as_ref().map_or(Vec3::ZERO, |hit| {
                (site.frame.rotation * hit.normal.as_vec3()).normalize_or_zero()
            }),
        });
        if !breath.on || step <= 0.0 {
            continue;
        }
        *turn = turn.wrapping_add(1);
        let side = ahead.any_orthonormal_vector();
        let lift = ahead.cross(side);
        for ray in 0..RAYS {
            // Spread over the cone's disc by the golden angle, turned a little each frame.
            let r = ((ray as f32 + 0.5) / RAYS as f32).sqrt() * breath.spread;
            let a = ray as f32 * 2.399_963 + *turn as f32 * 0.7;
            let dir = (ahead + (side * libm::cosf(a) + lift * libm::sinf(a)) * r).normalize();
            let Some(hit) = site.base.raycast(origin, dir, breath.reach_m) else {
                continue;
            };
            let heat = breath.heat * step * 6.0 / RAYS as f32;
            for x in -SPLASH..=SPLASH {
                for y in -SPLASH..=SPLASH {
                    for z in -SPLASH..=SPLASH {
                        let near = hit.cube + IVec3::new(x, y, z);
                        let off = (x * x + y * y + z * z) as f32;
                        let share = 1.0 / (1.0 + off);
                        fire.heat(&site.base, &burns_as, near, heat * share);
                    }
                }
            }
        }
    }
}

/// The fire stepped by its tick for the game time passed: its harm sent as acts, the
/// burnt-through first, what fell from them landing as brands; and how blackened each cube
/// is told to the drawing.
fn burn(
    time: Res<Time>,
    clock: Res<Clock>,
    climate: Option<Res<ClimateFields>>,
    mut fire: ResMut<SiteFire>,
    mut building: Building,
) {
    if fire.fire.is_out() {
        fire.owed_s = 0.0;
        return;
    }
    fire.owed_s += time.delta_secs();
    if fire.owed_s < TICK_S {
        return;
    }
    // The wind where the base stands, in its own space.
    let frame = building.site.frame;
    let here = frame.translation;
    let wind = climate.map_or(Vec3::ZERO, |climate| {
        let wind = climate.fields.at(climate.place(here), clock.time()).wind;
        let up = here.normalize_or(Vec3::Y);
        let north = climate.frame.north(here);
        let east = north.cross(up);
        let to = -(north * libm::cosf(wind.from) + east * libm::sinf(wind.from)) * wind.speed_m_s;
        frame.rotation.inverse() * to
    });
    let (mut step_ms, mut act_ms) = (0.0, 0.0);
    while fire.owed_s >= TICK_S {
        fire.owed_s -= TICK_S;
        let started = Instant::now();
        let SiteFire {
            fire: burning,
            burns,
            health,
            ..
        } = &mut *fire;
        let burns_as = |kind: Kind| burns.get(&kind).copied();
        let health_of = |kind: Kind| health.get(&kind).copied().unwrap_or(100);
        let stepped = burning.step(&building.site.base, &burns_as, &health_of, wind);
        step_ms += started.elapsed().as_secs_f32() * 1000.0;
        let acting = Instant::now();
        // Burnt through first, then the rest of what burns, each as an act of its own so
        // what one brings down is not harmed again by the next.
        let mut harm = stepped.harm;
        harm.sort_by_key(|(amount, _)| std::cmp::Reverse(*amount));
        for (amount, cubes) in harm {
            let cubes: Vec<[i32; 3]> = cubes
                .into_iter()
                .filter(|&at| {
                    building
                        .site
                        .base
                        .cube(IVec3::from_array(at))
                        .is_some_and(|c| c.is_in())
                })
                .collect();
            if cubes.is_empty() {
                continue;
            }
            if let Ok(done) = building.act(None, "fire", &[Op::Damage { cubes, amount }]) {
                burning.fell(&building.site.base, &burns_as, &done.fell);
            }
        }
        burning.show(&mut building.site.base);
        if stepped.scorched {
            building.site.revision += 1;
        }
        act_ms += acting.elapsed().as_secs_f32() * 1000.0;
    }
    fire.step_ms = step_ms;
    fire.act_ms = act_ms;
    let blazes = gather(&fire.fire, &building.site.base, &frame);
    fire.blazes = blazes;
    // Said every few seconds while it burns, for a script to follow and its cost to be read.
    fire.since_said += 1;
    if fire.since_said >= SAY_EVERY {
        fire.since_said = 0;
        info!(
            "Fire: {} cubes burning as {} blazes, {} cubes standing; its tick {step_ms:.1} ms and its harm {act_ms:.1} ms on the main thread.",
            fire.fire.burning_count(),
            fire.blazes.len(),
            building.site.base.cubes().count()
        );
    }
}

/// How many cubes a side of the base burn as one blaze, and from how many of those beside
/// each other on a level they burn as one broad one.
const BLAZE_CUBES: i32 = 4;
const BROAD_CELLS: usize = 3;

/// How often the fire is said in the log while it burns, in frames that stepped it.
const SAY_EVERY: u32 = 8;

/// Each two metres' cubes a side that burn as a blaze ([`BLAZE_CUBES`]), as games draw a big
/// fire as a few large flames rather than many small ones, so no more than a few cards lie
/// over any pixel: at the middle of their open faces, as hot as
/// they burn on average, as tall as there is fire in it and round it.
fn gather(fire: &Fire, base: &terra_build::Base, frame: &Transform) -> Vec<Blaze> {
    #[derive(Default)]
    struct Gathered {
        at: Vec3,
        count: f32,
        fierce: f32,
    }
    let mut cells: BTreeMap<[i32; 3], Gathered> = BTreeMap::new();
    for flame in fire.flames(base) {
        if flame.open == 0 {
            continue;
        }
        let middle = (flame.at.as_vec3() + 0.5) * CELL_M;
        // Out of its open faces, the top most of all.
        let mut out = Vec3::ZERO;
        for (i, way) in terra_build::support::FACES.iter().enumerate() {
            if flame.open & (1 << i) != 0 {
                out += way.as_vec3() * if way.y > 0 { 1.0 } else { 0.6 };
            }
        }
        let at = middle + out.normalize_or_zero() * CELL_M * 0.5;
        let cell = cells
            .entry(flame.at.div_euclid(IVec3::splat(BLAZE_CUBES)).to_array())
            .or_default();
        cell.at += at;
        cell.count += 1.0;
        cell.fierce += flame.fierce;
    }
    let near: BTreeMap<[i32; 3], f32> = cells.iter().map(|(&k, c)| (k, c.count)).collect();
    // Where fire is broad, a roof, a floor or a yard burning, the cells beside each other on
    // a level burn as one big blaze a sheet of fire as wide as they are, as a big fire is a
    // few large flames: one flame to each two-metre cell read as rows of the same sticker.
    // The pairs set off a cell on each level from the one under, so they do not line up.
    let mut broad: BTreeMap<[i32; 3], Vec<[i32; 3]>> = BTreeMap::new();
    for &key in cells.keys() {
        let set_off = (key[1].rem_euclid(2), (key[1].div_euclid(2)).rem_euclid(2));
        broad
            .entry([
                (key[0] + set_off.0).div_euclid(2),
                key[1],
                (key[2] + set_off.1).div_euclid(2),
            ])
            .or_default()
            .push(key);
    }
    let mut merged: BTreeMap<[i32; 3], (Gathered, f32)> = BTreeMap::new();
    for keys in broad.values() {
        if keys.len() < BROAD_CELLS {
            for key in keys {
                let cell = &cells[key];
                merged.insert(
                    *key,
                    (
                        Gathered {
                            at: cell.at,
                            count: cell.count,
                            fierce: cell.fierce,
                        },
                        0.0,
                    ),
                );
            }
            continue;
        }
        let mut all = Gathered::default();
        for key in keys {
            let cell = &cells[key];
            all.at += cell.at;
            all.count += cell.count;
            all.fierce += cell.fierce;
        }
        let wide = BLAZE_CUBES as f32 * CELL_M * (keys.len() as f32).sqrt() * 1.1;
        merged.insert(keys[0], (all, wide));
    }
    let mut out: Vec<Blaze> = merged
        .iter()
        .map(|(&key, (cell, wide))| {
            let round: f32 = (-1..=1)
                .flat_map(|x| {
                    (-1..=1).flat_map(move |y| {
                        (-1..=1).map(move |z| [key[0] + x, key[1] + y, key[2] + z])
                    })
                })
                .filter_map(|k| near.get(&k))
                .sum::<f32>()
                - cell.count;
            let heat = (cell.fierce / cell.count).clamp(0.0, 1.0);
            let tall = (0.8 + 0.3 * cell.count.sqrt() + 0.25 * round.sqrt()) * (0.5 + 0.5 * heat);
            let local = cell.at / cell.count;
            let seed = (key[0].wrapping_mul(73_856_093)
                ^ key[1].wrapping_mul(19_349_663)
                ^ key[2].wrapping_mul(83_492_791)) as u32;
            Blaze {
                at: frame.transform_point(local),
                tall: tall.min(7.0),
                heat,
                seed: (seed % 10_000) as f32 / 10_000.0,
                wide: *wide,
            }
        })
        .collect();
    out.sort_by(|a, b| (b.tall * b.heat).total_cmp(&(a.tall * a.heat)));
    out
}

/// The blazes given to the drawing.
fn blazes(
    mut commands: Commands,
    fire: Res<SiteFire>,
    mut drawn: ResMut<Blazes>,
    mut fright: Local<Option<Entity>>,
    mut frights: Query<(&mut Transform, &mut Frightful)>,
) {
    let splashes = fire.splashes.len();
    if fire.is_changed() || drawn.blazes.len() != fire.blazes.len() + splashes {
        // The splashes first, so the drawing's cap never leaves them out.
        drawn.blazes.clear();
        drawn
            .blazes
            .extend(fire.splashes.iter().enumerate().map(|(i, &at)| Blaze {
                at,
                tall: SPLASH_TALL_M,
                heat: 1.0,
                seed: 0.37 + 0.21 * i as f32,
                wide: SPLASH_WIDE_M,
            }));
        drawn.blazes.extend_from_slice(&fire.blazes);
    }
    // A fire frightens what lives round it, the more of it burns the further off: from the
    // middle of its blazes, past their reach by as much again.
    let count = fire.blazes.len();
    let entity = *fright.get_or_insert_with(|| {
        commands
            .spawn((
                Name::new("Fire: fright"),
                Transform::IDENTITY,
                Frightful { within_m: 0.0 },
            ))
            .id()
    });
    if let Ok((mut at, mut frightful)) = frights.get_mut(entity) {
        if count == 0 {
            frightful.within_m = 0.0;
            return;
        }
        let middle = fire.blazes.iter().map(|b| b.at).sum::<Vec3>() / count as f32;
        let reach = fire
            .blazes
            .iter()
            .map(|b| b.at.distance(middle))
            .fold(0.0, f32::max);
        at.translation = middle;
        frightful.within_m = reach * 2.0 + 15.0;
    }
}

#[derive(Deserialize)]
struct Lit {
    at: [i32; 3],
    to: Option<[i32; 3]>,
    heat: Option<f32>,
}

impl Lit {
    fn cubes(&self) -> Vec<IVec3> {
        let (a, b) = (
            IVec3::from_array(self.at),
            IVec3::from_array(self.to.unwrap_or(self.at)),
        );
        let (low, high) = (a.min(b), a.max(b));
        (low.x..=high.x)
            .flat_map(|x| {
                (low.y..=high.y)
                    .flat_map(move |y| (low.z..=high.z).map(move |z| IVec3::new(x, y, z)))
            })
            .collect()
    }
}

/// Sets alight what burns in a box of the base's cubes, from `at` to `to` (`at` alone if not
/// given), as a fire is lit.
fn light(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let lit: Lit = parse_some(params)?;
    let mut state = SystemState::<(Res<Site>, ResMut<SiteFire>)>::new(world);
    let (site, mut fire) = state
        .get_mut(world)
        .map_err(|why| refused(format!("{why}")))?;
    let SiteFire {
        fire: burning,
        burns,
        ..
    } = &mut *fire;
    let burns_as = |kind: Kind| burns.get(&kind).copied();
    for at in lit.cubes() {
        burning.light(&site.base, &burns_as, at);
    }
    let count = burning.burning_count();
    info!("Fire: lit, {count} cubes burning.");
    Ok(json!({ "done": true, "burning": count }))
}

/// Heats a box of the base's cubes by so much (a second of a burning neighbour's full flame
/// gives its heat), as a brand or a breath does.
fn heat(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let lit: Lit = parse_some(params)?;
    let mut state = SystemState::<(Res<Site>, ResMut<SiteFire>)>::new(world);
    let (site, mut fire) = state
        .get_mut(world)
        .map_err(|why| refused(format!("{why}")))?;
    let SiteFire {
        fire: burning,
        burns,
        ..
    } = &mut *fire;
    let burns_as = |kind: Kind| burns.get(&kind).copied();
    for at in lit.cubes() {
        burning.heat(&site.base, &burns_as, at, lit.heat.unwrap_or(4.0));
    }
    Ok(json!({ "done": true }))
}

/// How the base burns: how many cubes are burning, how many blazes it is drawn as, and how
/// long the fire's last ticks took on the main thread.
fn state(In(_): In<Option<Value>>, world: &mut World) -> BrpResult {
    let fire = world.resource::<SiteFire>();
    let cubes = world.resource::<Site>().base.cubes().count();
    Ok(json!({
        "burning": fire.fire.burning_count(),
        "blazes": fire.blazes.len(),
        "step_ms": fire.step_ms,
        "act_ms": fire.act_ms,
        "cubes": cubes,
    }))
}
