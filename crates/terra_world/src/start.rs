//! The start: the fixed place on the planet that every position given as metres east and
//! south is measured from, by scripts, the map, the log and what is placed around it. It is
//! the world's, not any system's: chosen once from the planet, as the most level ground near
//! the planet's own start that the example base fits on, so a run with no building in it
//! starts in the same place as one with.

use crate::planet::Planet;
use bevy::prelude::*;
use terra_build::{Base, CELL_M, Kind, example};

/// A base's floor clears the highest ground under it by this much.
pub const FLOOR_CLEARANCE_M: f32 = 0.12;

/// Where the start is: its point on the ground, and its east and south as the frame's
/// x and z. A base is first laid with its corner here.
#[derive(Resource, Clone, Copy, Deref, Debug)]
pub struct Start(pub Transform);

impl Start {
    /// The start of a planet. On the yard, the flat test level, it is the planet's own
    /// start, facing east.
    pub fn choose(planet: &Planet, yard: bool) -> Self {
        let mut footprint = Base::default();
        for op in example(Kind::PLAIN) {
            footprint.apply(op);
        }
        Self(if yard {
            let start = planet.start();
            let up = start.normalize();
            let east = up.cross(Vec3::X).normalize();
            Transform::from_translation(start + up * FLOOR_CLEARANCE_M).looking_to(east, up)
        } else {
            lifted(choose_site(planet, &footprint), &footprint, planet)
        })
    }

    /// The start by a lake the land's rules made (`--site`, docs/demos.md): on the shore
    /// nearest its deepest water, as its lakes were found round it (`Planet::look_round`),
    /// facing the water, east and south as that facing's right and back.
    pub fn by_lake(planet: &Planet, site: &terra_land::site::Site) -> Self {
        let water = planet.surface(site.toward);
        let shore = planet
            .waters()
            .and_then(|waters| waters.shore_of(site.toward))
            .unwrap_or(site.shore);
        let shore = planet.surface(shore);
        let up = shore.normalize();
        let facing = (water - shore)
            .reject_from(up)
            .normalize_or(up.any_orthonormal_vector());
        Self(Transform::from_translation(shore + up * FLOOR_CLEARANCE_M).looking_to(facing, up))
    }

    /// The point so many metres east and south of the start, on its level.
    pub fn at(&self, east: f32, south: f32) -> Vec3 {
        self.transform_point(Vec3::new(east, 0.0, south))
    }

    /// A point on the planet as metres east, up and south of the start: the terms `--at`,
    /// `--face` and the log give places in.
    pub fn local(&self, point: Vec3) -> Vec3 {
        self.compute_affine().inverse().transform_point3(point)
    }
}

/// A place the land's rules make, found by its query (`--site`, docs/demos.md).
#[derive(Clone, Copy, Debug)]
pub enum Found {
    /// A mountain lake: the start on its shore, facing the water.
    Lake(terra_land::site::Site),
    /// Open tundra: the start on it, facing the forest's edge.
    Tundra(terra_land::tundra::TundraSite),
    /// A market town's place: the start beside it, out by its fields, facing it; the town is
    /// the layer above's to lay there.
    Town(terra_land::town::TownSite),
    /// A settlement newly founded at a market town's place, only its plots cleared of the
    /// forest (D381): the start beside it, facing it.
    Settlement(terra_land::town::TownSite),
}

/// The place the land's rules made that the world starts at, if `--site` asked for one.
#[derive(Resource, Clone, Copy, Debug, Deref)]
pub struct Sited(pub Found);

impl Found {
    /// Where the land round it is worked out from.
    pub fn middle(&self) -> Vec3 {
        match self {
            Found::Lake(site) => site.toward,
            Found::Tundra(site) => site.toward,
            Found::Town(site) | Found::Settlement(site) => site.toward,
        }
    }

    /// Where the woods round it are grown from, if not the start: a town's own middle, so its
    /// streets look out on grown trees whichever way.
    pub fn woods_middle(&self, planet: &Planet) -> Option<Vec3> {
        match self {
            Found::Town(site) | Found::Settlement(site) => Some(planet.surface(site.toward)),
            Found::Lake(_) | Found::Tundra(_) => None,
        }
    }

    /// Where the flora grows round besides the start, if anywhere: a settlement's meadow, far
    /// from the start, its flowers, herbs and the bushes at its edge (D381).
    pub fn flora_middle(&self, planet: &Planet) -> Option<Vec3> {
        match self {
            Found::Settlement(site) => Some(planet.surface(site.toward)),
            Found::Town(_) | Found::Lake(_) | Found::Tundra(_) => None,
        }
    }

    /// The forest it has cleared, if it is a settlement's: a town's fields.
    pub fn clearings(&self, land: &terra_land::Land) -> Vec<terra_land::town::Clearing> {
        match self {
            Found::Town(site) => terra_land::town::MarketTown::DEMO.clearings(land, site),
            Found::Settlement(site) => {
                terra_land::town::MarketTown::SETTLEMENT.clearings(land, site)
            }
            Found::Lake(_) | Found::Tundra(_) => Vec::new(),
        }
    }

    /// The start there, once the land round it is known.
    pub fn start(&self, planet: &Planet) -> Start {
        match self {
            Found::Lake(site) => Start::by_lake(planet, site),
            Found::Tundra(site) => {
                let at = planet.surface(site.toward);
                let up = at.normalize();
                let facing = (planet.surface(site.forest) - at)
                    .reject_from(up)
                    .normalize_or(up.any_orthonormal_vector());
                Start(
                    Transform::from_translation(at + up * FLOOR_CLEARANCE_M).looking_to(facing, up),
                )
            }
            Found::Town(site) | Found::Settlement(site) => {
                let at = planet.surface(site.start);
                let up = at.normalize();
                let facing = (planet.surface(site.toward) - at)
                    .reject_from(up)
                    .normalize_or(up.any_orthonormal_vector());
                Start(
                    Transform::from_translation(at + up * FLOOR_CLEARANCE_M).looking_to(facing, up),
                )
            }
        }
    }
}

/// The place `--site` names, found by its query. Panics, saying why, if the land has no such
/// place, as a scripted run should. `<name>@<east>,<north>` pins it (D393): the place its
/// query finds nearest so many kilometres east and north of the planet's start, within 200 m,
/// where a demo's place was first found, so a change to the land elsewhere never moves the
/// demo, and one that spoils its place stops the run saying so.
pub fn find_site(planet: &Planet, name: &str) -> Found {
    let climate: terra_climate::Climate = crate::data::read(crate::daylight::WorldClimate::FILE);
    let land = terra_land::Land::new(planet.voxels().field().clone(), climate);
    let (name, pinned_km) = terra_land::site::pinned(name).unwrap_or_else(|why| panic!("{why}"));
    match name {
        "lake" => {
            let want = terra_land::site::MountainLake {
                pinned_km,
                ..terra_land::site::MountainLake::DEMO
            };
            let (site, searched) = land.mountain_lake_searched(terra_land::PLANET_START, &want);
            let site = site.unwrap_or_else(|| {
                panic!(
                    "--site lake: no mountain lake on this world ({searched:?}); mountains need --radius 30000"
                )
            });
            info!(
                "Site lake: a lake {:.0} m over the sea, {:.2} km², the tree line {:.0} m, peaks to {:.0} m, {:.2} km east and {:.2} km north of the planet's start",
                site.level_m,
                site.area_m2 / 1e6,
                site.tree_line_m,
                site.highest_m,
                site.at_km.x,
                site.at_km.y
            );
            Found::Lake(site)
        }
        "tundra" => {
            let want = terra_land::tundra::OpenTundra {
                pinned_km,
                ..terra_land::tundra::OpenTundra::DEMO
            };
            let (site, searched) = land.tundra_searched(terra_land::PLANET_START, &want);
            let site = site.unwrap_or_else(|| {
                panic!(
                    "--site tundra: no open tundra on this world ({searched:?}); it needs --radius 30000"
                )
            });
            info!(
                "Site tundra: {:.0} m over the sea, its growing season {:.1} °C and its year {:.1} °C, the closed forest {:.0} m off, {:.2} km east and {:.2} km north of the planet's start",
                site.height_m,
                site.growing_mean_c,
                site.year_mean_c,
                site.forest_m,
                site.at_km.x,
                site.at_km.y
            );
            Found::Tundra(site)
        }
        "town" | "settlement" => {
            let want = terra_land::town::MarketTown {
                pinned_km,
                ..if name == "town" {
                    terra_land::town::MarketTown::DEMO
                } else {
                    terra_land::town::MarketTown::SETTLEMENT
                }
            };
            let (site, searched) = land.market_town_searched(terra_land::PLANET_START, &want);
            let site = site.unwrap_or_else(|| {
                panic!(
                    "--site {name}: no place for a market town on this world ({searched:?}); it needs --radius 30000"
                )
            });
            info!(
                "Site town: {:.0} m over the sea, its growing season {:.1} °C, {:.1} m of rise and fall over the town, closed forest past its fields on {:.0}% of the ways out, the highest ground {:.0} m over the sea {:.1} km off, the skyline to {:.1}°, {:.2} km east and {:.2} km north of the planet's start",
                site.height_m,
                site.growing_mean_c,
                site.relief_m,
                site.forest * 100.0,
                site.peak_m,
                site.peak_off_m / 1000.0,
                site.skyline_deg,
                site.at_km.x,
                site.at_km.y
            );
            match site.water {
                Some(water) => info!(
                    "Site town: by a lake, its shore {:.0} m off, {:.0} m east and {:.0} m north",
                    water.length(),
                    water.x,
                    water.y
                ),
                None if want.water_m.is_some() => {
                    info!("Site town: no lake beside any of the best places")
                }
                None => {}
            }
            if name == "town" {
                Found::Town(site)
            } else {
                Found::Settlement(site)
            }
        }
        other => panic!("--site {other}: the sites are lake, tundra, town and settlement"),
    }
}

/// Where to stand a base: the most level ground near the start that is clear of the sea,
/// turned so its door looks out over the water if there is any.
fn choose_site(planet: &Planet, base: &Base) -> Transform {
    let start = planet.start();
    let up = start.normalize();
    let east = up.cross(Vec3::X).normalize();
    let north = east.cross(up);
    let sea = planet.sea_radius_m();

    let ground_floor = || {
        base.room_cells()
            .filter(|(at, _)| at.y == 0)
            .map(|(at, room)| (at.with_y(0), room))
    };
    let low = ground_floor().fold(IVec3::MAX, |low, (tile, _)| low.min(tile));
    let high = ground_floor().fold(IVec3::MIN, |high, (tile, _)| high.max(tile));
    let (low, high) = (
        low.as_vec3() * CELL_M - 3.0,
        (high + 1).as_vec3() * CELL_M + 3.0,
    );
    let middle = (low + high) * 0.5;

    let mut best: Option<(f32, Transform)> = None;
    for ring in 3..22 {
        for spoke in 0..24 {
            let angle = spoke as f32 / 24.0 * std::f32::consts::TAU;
            let offset =
                (east * libm::cosf(angle) + north * libm::sinf(angle)) * (ring as f32 * 6.0);
            let anchor = planet.ground_at(start + offset);
            let anchor_up = anchor.normalize();
            for turn in 0..8 {
                let facing =
                    Quat::from_axis_angle(anchor_up, turn as f32 * std::f32::consts::TAU / 8.0)
                        * east.reject_from(anchor_up).normalize();
                let mut frame = Transform::from_translation(anchor).looking_to(facing, anchor_up);
                // Put the middle of the base, not its corner, on the chosen spot.
                frame.translation -= frame.rotation * Vec3::new(middle.x, 0.0, middle.z);

                // How far the ground under the base rises above and falls below the frame.
                let (mut lowest, mut highest) = (f32::MAX, f32::MIN);
                let mut x = low.x;
                while x <= high.x {
                    let mut z = low.z;
                    while z <= high.z {
                        let height = -planet.distance(frame.transform_point(Vec3::new(x, 0.0, z)));
                        lowest = lowest.min(height);
                        highest = highest.max(height);
                        z += 3.0;
                    }
                    x += 3.0;
                }
                let above_sea = anchor.length() + lowest - sea;
                if above_sea < 1.6 {
                    continue;
                }
                if highest - lowest > 2.5 {
                    continue;
                }
                // Doors are on the base's +Z side; water a short walk out that way is
                // worth a little slope.
                let water_ahead = [40.0, 60.0, 85.0]
                    .iter()
                    .filter(|&&ahead| {
                        let point = frame.transform_point(Vec3::new(middle.x, 0.0, ahead));
                        planet.under_sea(point)
                    })
                    .count();
                // Grass to stand on, at the base and out in front of it.
                let grassy: f32 = [0.0, 12.0, 24.0]
                    .iter()
                    .map(|&ahead| {
                        let point = frame.transform_point(Vec3::new(middle.x, 0.0, ahead));
                        let point = planet.ground_at(point);
                        planet.cover(point, planet.normal(point)).grass
                    })
                    .sum();
                let score = (highest - lowest) - 0.8 * water_ahead as f32 - 0.5 * grassy
                    + 0.04 * (above_sea - 4.0).abs();
                if best.as_ref().is_none_or(|(least, _)| score < *least) {
                    best = Some((score, frame));
                }
            }
        }
    }
    best.map(|(_, frame)| frame).unwrap_or_else(|| {
        let anchor = planet.ground_at(start + east * 60.0);
        Transform::from_translation(anchor).looking_to(north, anchor.normalize())
    })
}

/// Raises a build's frame until its floor clears the highest ground under it: under its
/// rooms, or with none, as a floor laid alone, under its floor. The stand-in planet cannot
/// be dug, so a build has to stay out of the ground's way; a foundation and steps make up
/// the difference.
pub fn lifted(mut frame: Transform, base: &Base, planet: &Planet) -> Transform {
    let mut columns: Vec<IVec3> = base
        .room_cells()
        .filter(|(at, _)| at.y == 0)
        .map(|(at, _)| at)
        .collect();
    if columns.is_empty() {
        let floor = terra_build::floor_layer(0);
        columns = base
            .cubes()
            .filter(|(at, _)| at.y == floor)
            .map(|(at, _)| at.with_y(0))
            .collect();
    }
    if columns.is_empty() {
        return frame;
    }
    let highest_ground = columns
        .into_iter()
        .flat_map(|tile| {
            [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0), (0.5, 0.5)].map(|(dx, dz)| {
                let on_floor = Vec3::new(
                    (tile.x as f32 + dx) * CELL_M,
                    0.0,
                    (tile.z as f32 + dz) * CELL_M,
                );
                -planet.distance(frame.transform_point(on_floor))
            })
        })
        .fold(f32::MIN, f32::max);
    let up = *frame.up();
    frame.translation += up * (highest_ground + FLOOR_CLEARANCE_M);
    frame
}
