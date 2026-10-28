//! The dungeon demos: a generated dungeon (`terra_dungeon`) dug into the lush forest, laid in
//! building's cubes and dressed with its props, fires, chests, doors and creatures, the player
//! on the grass by the stair down. Each experiment reads its theme and seed from
//! `assets/data/dungeon_demo.ron` (a theme is data, D136, D249):
//!
//!   cargo xtask sandbox --experiment dungeon        (the crypt)
//!   cargo xtask sandbox --experiment dungeon-mine   (the flooded mine)
//!   cargo xtask sandbox --experiment dungeon-keep   (the ruined keep)
//!
//! The log gives the tour's points as `--vantage` and `--route` take them.

mod figure;
mod interior;
mod spawn;

use bevy::prelude::*;
use serde::Deserialize;
use terra_core::Operation;
use terra_dungeon::stamp::{self, Stamp};
use terra_dungeon::{ModuleSet, Theme};
use terra_game::build::{BuildLook, frame_at, kit_names};
use terra_gen::Seed;
use terra_walker::walker::ArriveAt;
use terra_world::planet::Planet;
use terra_world::start::Start;

/// What each dungeon experiment stands for: the game in the lush forest on its yard, at noon.
pub const WITH: &[&str] = &[
    "--experiment",
    "play",
    "--experiment",
    "lush",
    "--level",
    "yard",
    "--no-lair",
    "--time",
    "12",
];

/// The demos' data file, in `assets/data`.
const FILE: &str = "dungeon_demo.ron";
/// Candidate sites are so far apart, in metres.
const SITE_STEP_M: f32 = 8.0;
/// The ground over a footprint is sampled so far apart, in metres.
const SAMPLE_M: f32 = 4.0;
/// The dungeon's top sits so far under the lowest ground over it, so no thin skin of
/// ground is left over its roof.
const UNDER_GROUND_M: f32 = 0.1;
/// The player starts so far beyond the stair's top, on the grass.
const START_BEYOND_M: f32 = 2.0;
/// The tour's views are from so high over the floor.
const EYE_M: f32 = 1.6;
/// Placement can give a seed up; so many seeds after it are tried.
const SEEDS_TRIED: u64 = 8;

#[derive(Deserialize)]
struct Demos {
    demos: Vec<Demo>,
    near: (f32, f32),
    within: f32,
}

#[derive(Deserialize)]
struct Demo {
    experiment: String,
    theme: String,
    seed: u64,
}

/// Which demo this run is.
#[derive(Resource)]
struct Wanted(&'static str);

/// A dungeon laid out and dug, waiting for building's look to be laid in cubes.
#[derive(Resource)]
struct Pending {
    stamp: Stamp,
    frame: Transform,
    /// What of its pieces a walker cannot pass (`stamp::blocks`), in the dungeon's frame.
    blocks: Vec<stamp::Blocking>,
    /// The person at the goal, if it is one (`theme::Figure`).
    figure: Option<terra_dungeon::theme::Figure>,
}

pub fn plugin(app: &mut App, experiment: &'static str) {
    interior::plugin(app);
    app.insert_resource(Wanted(experiment))
        .add_systems(Startup, lay_out)
        .add_systems(
            Update,
            spawn::put_up.run_if(resource_exists::<BuildLook>.and_then(resource_exists::<Pending>)),
        )
        .add_systems(Update, (spawn::form_mobs, spawn::push_doors));
}

/// The side's direction in the dungeon's frame: 0..4 = north, east, south, west.
fn dir(side: usize) -> Vec3 {
    match side % 4 {
        0 => Vec3::NEG_Z,
        1 => Vec3::X,
        2 => Vec3::Z,
        _ => Vec3::NEG_X,
    }
}

/// The theme's dungeon from the demo's seed, or the first of the seeds after it placement
/// does not give up.
fn generate(theme: &Theme, seed: u64) -> Option<terra_dungeon::Dungeon> {
    let set = ModuleSet::new(theme)
        .inspect_err(|why| warn!("Dungeon: theme {}: {why}", theme.name))
        .ok()?;
    (seed..seed + SEEDS_TRIED).find_map(|seed| {
        terra_dungeon::generate(theme, &set, Seed(seed))
            .inspect_err(|why| warn!("Dungeon: seed {seed} given up: {why}"))
            .ok()
    })
}

/// Lays the demo's dungeon out, finds the most level ground for it, digs it and says where
/// the player arrives.
fn lay_out(
    mut commands: Commands,
    wanted: Res<Wanted>,
    mut planet: ResMut<Planet>,
    start: Res<Start>,
) {
    let demos: Demos = terra_world::data::read(FILE);
    let Some(demo) = demos.demos.iter().find(|d| d.experiment == wanted.0) else {
        warn!("Dungeon: {FILE} has no demo for {}", wanted.0);
        return;
    };
    let themes = match Theme::all(&terra_world::data::assets()) {
        Ok(themes) => themes,
        Err(why) => return warn!("Dungeon: the themes: {why}"),
    };
    let Some(theme) = themes.iter().find(|t| t.name == demo.theme) else {
        return warn!("Dungeon: no theme {}", demo.theme);
    };
    let Some(dungeon) = generate(theme, demo.seed) else {
        return;
    };
    let kits = kit_names();
    let kind = |name: &str| {
        kits.iter()
            .find(|(_, kit)| kit.eq_ignore_ascii_case(name))
            .map(|(&kind, _)| kind)
            .unwrap_or_else(|| {
                warn!("Dungeon: no kit {name}; laid in the first");
                kits.keys().next().copied().unwrap_or_default()
            })
    };
    let stamp = stamp::stamp(&dungeon, theme, &kind);
    let frame = site(&planet, &start, &stamp, demos.near, demos.within);
    dig_in(&mut planet, &stamp, &frame);
    let up_side = (stamp.start_facing + 2) % 4;
    let at = stamp.start + dir(up_side) * START_BEYOND_M;
    commands.insert_resource(ArriveAt {
        at: frame.transform_point(at),
        toward: frame.transform_point(stamp.start + dir(stamp.start_facing) * 3.0),
    });
    let local = |p: Vec3| {
        let p = start.local(frame.transform_point(p));
        format!("{:.1},{:.1}", p.x, p.z)
    };
    info!(
        "Dungeon: {} ({} rooms) from seed {}, {} cubes' operations, {} props, {} lights, {} mobs; start {}",
        theme.name,
        dungeon.layout.rooms.len(),
        demo.seed,
        stamp.ops.len(),
        stamp.props.len(),
        stamp.lights.len(),
        stamp.mobs.len(),
        local(at),
    );
    let tour: Vec<String> = stamp.route.iter().map(|&p| local(p)).collect();
    info!("Dungeon tour (--route): {}", tour.join(" "));
    // Each leg of the tour as a view from eye height at its start to its end.
    let point = |p: Vec3| {
        let p = start.local(frame.transform_point(p));
        format!("{:.2},{:.2},{:.2}", p.x, p.y, p.z)
    };
    for (i, leg) in stamp.route.windows(2).enumerate() {
        info!(
            "Dungeon vantage {i}: {},{}",
            point(leg[0] + Vec3::Y * EYE_M),
            point(leg[1] + Vec3::Y * (EYE_M - 0.2))
        );
    }
    // The camera's eye along the tour, kept off walls, leaves and slabs (a film's keys).
    for (i, &eye) in stamp.view.iter().enumerate() {
        info!("Dungeon view {i}: {}", point(eye));
    }
    // Where it looks from each: at the way ahead, or at the room's goal or key once near.
    for (i, &at) in stamp.look.iter().enumerate() {
        info!("Dungeon look {i}: {}", point(at));
    }
    // Where each creature forms, on the floor, so a `--vantage` can frame one.
    for mob in &stamp.mobs {
        info!("Dungeon creature {} at {}", mob.species, point(mob.at));
    }
    // Where each prop stands and which side it faces, so a `--vantage` can frame a room's dressing.
    for prop in &stamp.props {
        let name = prop.model.rsplit('/').next().unwrap_or(&prop.model);
        info!(
            "Dungeon prop {name} at {} facing {}",
            point(prop.at),
            prop.facing
        );
    }
    // Where each fire and lamp stands, so a `--vantage` can frame a camp's fire or a goal's bowls.
    for light in &stamp.lights {
        info!("Dungeon light {:?} at {}", light.kind, point(light.at));
    }
    // Where each door stands, so a film's dolly can stop short of it.
    for door in stamp
        .objects
        .iter()
        .filter(|o| o.role == stamp::ObjectRole::Door)
    {
        info!("Dungeon door {} at {}", door.name, point(door.at));
    }
    // Each room's purpose, floor middle, cells and modules, so a `--vantage` can frame one.
    let set = ModuleSet::new(theme).ok();
    for (r, rect) in dungeon.layout.rooms.iter().enumerate() {
        let mut modules: Vec<&str> = rect
            .cells()
            .filter_map(|at| dungeon.layout.cell(at))
            .filter_map(|c| {
                let v = set
                    .as_ref()?
                    .variants
                    .get(*dungeon.dressing.picks.get(c)?)?;
                Some(theme.modules[v.module].name.as_str())
            })
            .collect();
        modules.sort_unstable();
        info!(
            "Dungeon room {r} ({:?}): middle {}, {}x{} cells: {}",
            dungeon.graph.rooms[r].role,
            point(stamp::room_centre(&dungeon.layout, r) + Vec3::Y * EYE_M),
            rect.size.x,
            rect.size.y,
            modules.join(" "),
        );
    }
    // How long a dolly takes from each to the next, slowed where the look turns fast.
    for (i, &seconds) in stamp.pace.iter().enumerate() {
        info!("Dungeon pace {i}: {seconds:.3}");
    }
    commands.insert_resource(interior::Interior::new(&stamp, frame));
    let blocks = stamp::blocks(&stamp, theme);
    commands.insert_resource(Pending {
        stamp,
        frame,
        blocks,
        figure: theme.features.figure.clone(),
    });
}

/// Digs the dungeon's boxes out of the ground at its frame. What grows is planted after
/// (Startup before the forest's Update), and nothing is planted on dug ground
/// (`Planet::dug`), so no tree, bush or grass stands inside.
fn dig_in(planet: &mut Planet, stamp: &Stamp, frame: &Transform) {
    for dig in &stamp.digs {
        planet.apply(&Operation::DigBox {
            centre: frame.transform_point(dig.centre),
            half: dig.half,
            rotation: frame.rotation,
        });
    }
}

/// The dungeon's frame: its stair's top at the candidate within `within` of `near` whose
/// footprint has the most level ground, its top a little under that ground's lowest.
fn site(planet: &Planet, start: &Start, stamp: &Stamp, near: (f32, f32), within: f32) -> Transform {
    let (mut low, mut high) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
    for dig in &stamp.digs {
        let (a, b) = (dig.centre - dig.half, dig.centre + dig.half);
        low = low.min(Vec2::new(a.x, a.z));
        high = high.max(Vec2::new(b.x, b.z));
    }
    let from_start = Vec2::new(stamp.start.x, stamp.start.z);
    let height = |east: f32, south: f32| planet.surface(start.at(east, south).normalize()).length();
    let steps = (within / SITE_STEP_M) as i32;
    let mut best: Option<(f32, Vec2, f32)> = None;
    for i in -steps..=steps {
        for j in -steps..=steps {
            let offset = Vec2::new(i as f32, j as f32) * SITE_STEP_M;
            if offset.length() > within {
                continue;
            }
            let origin = Vec2::new(near.0, near.1) + offset - from_start;
            let (mut lowest, mut highest) = (f32::MAX, f32::MIN);
            let mut x = low.x;
            while x <= high.x + SAMPLE_M {
                let mut z = low.y;
                while z <= high.y + SAMPLE_M {
                    let h = height(origin.x + x.min(high.x), origin.y + z.min(high.y));
                    lowest = lowest.min(h);
                    highest = highest.max(h);
                    z += SAMPLE_M;
                }
                x += SAMPLE_M;
            }
            let score = highest - lowest;
            if best.is_none_or(|(s, _, _)| score < s) {
                best = Some((score, origin, lowest));
            }
        }
    }
    let (score, origin, lowest) = best.unwrap_or((0.0, Vec2::new(near.0, near.1), 0.0));
    let point = start.at(origin.x, origin.y).normalize() * (lowest - UNDER_GROUND_M);
    info!(
        "Dungeon site: {:.0} m east, {:.0} m south of the start, the ground over it within {score:.1} m",
        origin.x, origin.y
    );
    frame_at(point, &start.0)
}

#[cfg(test)]
mod tests;
