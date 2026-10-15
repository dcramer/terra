//! A market town and its life (docs/demos.md, a living town; D324, D330): asked for at a
//! place ([`Wanted`]), laid out from its seed on the ground there (`terra_build::town`),
//! every building put up by the shortcuts a player's base is, its things placed as
//! catalogue items are; and its people living their days by `terra_life`, ticked with the
//! world, the camera and every viewpoint their viewers (D145), drawn by `townsfolk`.
//!
//! Places in the town are metres east and south of its square's middle, as the layout gives
//! them, on the ground under them.

use super::Footprint;
use super::catalogue::Builds;
use super::daylight::{Clock, DAY_SECONDS};
use super::flora::ClearAround;
use super::props::{self, Models, Theirs};
use super::tick::Sim;
use crate::build::{self, Build, BuildLook, Built, Site};
use crate::planet::{MainCamera, Planet};
use crate::start::Start;
use bevy::prelude::*;
use terra_build::fitting::Hang;
use terra_build::town::ThingKind;
use terra_build::{Base, CELL_M};
use terra_life::{Animals, Life, Plan};
use terra_world::lod::Viewpoint;
use terra_world::smoke::{Burning, Smoke};

/// A town wanted: its seed, and near where, in metres east and south of the start; its
/// square goes on the most level ground within `within` of there.
#[derive(Resource, Clone, Copy, Debug)]
pub struct Wanted {
    pub seed: u32,
    pub near: Vec2,
    pub within: f32,
    /// Whether the player is taken there once it is laid: to its main street, looking
    /// toward the square.
    pub arrive: bool,
    /// Whether its people and animals are out: without them, what they cost is measured.
    pub lived_in: bool,
}

/// How far round a square's middle a town reaches, for finding level ground for it.
const REACH_M: f32 = 60.0;

/// The most level ground for a town near a place, in metres east and south of the start:
/// of places on a grid within reach, the one whose ground over a town's reach rises and
/// falls least, the nearer winning a tie. The same on every machine: the planet decides it.
fn site_for(planet: &Planet, start: &Start, near: Vec2, within: f32) -> Vec2 {
    let mut best = (f32::MAX, near);
    let steps = (within / 15.0).ceil() as i32;
    for i in -steps..=steps {
        for j in -steps..=steps {
            let at = near + Vec2::new(i as f32, j as f32) * 15.0;
            if at.distance(near) > within {
                continue;
            }
            let middle = planet.ground_at(start.at(at.x, at.y));
            let radius = middle.length();
            let (mut low, mut high) = (f32::MAX, f32::MIN);
            for ring in [0.3, 0.6, 1.0] {
                for k in 0..12 {
                    let angle = k as f32 / 12.0 * std::f32::consts::TAU;
                    let offset = Vec2::new(libm::cosf(angle), libm::sinf(angle)) * REACH_M * ring;
                    let p = at + offset;
                    let ground = planet.ground_at(start.at(p.x, p.y));
                    let height =
                        ground.length() - radius + offset.length_squared() / (2.0 * radius);
                    low = low.min(height);
                    high = high.max(height);
                }
            }
            let score = (high - low) + at.distance(near) * 0.002;
            if score < best.0 {
                best = (score, at);
            }
        }
    }
    best.1
}

/// A town laid out, and its people's lives.
#[derive(Resource)]
pub struct Market {
    /// The square's middle on the ground, and the town's east, south and up there.
    pub middle: Vec3,
    pub east: Vec3,
    pub south: Vec3,
    pub up: Vec3,
    pub life: Life,
    /// Its animals: hens, goats, dogs.
    pub animals: Animals,
}

impl Market {
    /// A place in the town on the ground.
    pub fn ground(&self, planet: &Planet, at: Vec2) -> Vec3 {
        planet.ground_at(self.middle + self.east * at.x + self.south * at.y)
    }

    /// A point in the world as a place in the town.
    pub fn local(&self, point: Vec3) -> Vec2 {
        let offset = point - self.middle;
        Vec2::new(offset.dot(self.east), offset.dot(self.south))
    }

    /// A way in the town as a way in the world, level with the ground.
    pub fn way(&self, way: Vec2) -> Vec3 {
        (self.east * way.x + self.south * way.y).normalize_or(self.east)
    }

    /// The clock in hours since the world began, as the town's life counts it.
    pub fn hours(clock: &Clock) -> f64 {
        clock.day as f64 * 24.0 + clock.hour as f64
    }
}

/// A point on the ground in the terms `--at`, `--route` and `--vantage-ground` take it:
/// metres east and south on the start's own level, whose ground straight below (toward the
/// planet's middle) is the point. Not the point's own place in the start's frame, which
/// differs from it by metres a few hundred metres off on a small planet.
fn start_terms(start: &Start, point: Vec3) -> Vec2 {
    let up = start.up().as_vec3();
    let toward = point.normalize();
    let on_level = toward * (start.translation.dot(up) / toward.dot(up));
    let local = start.local(on_level);
    Vec2::new(local.x, local.z)
}

/// A home's hearth, whose smoke rises through its roof: the building it is in.
#[derive(Component)]
pub struct Hearth {
    pub home: usize,
}

/// A home's firelight within, through its door and shutters at dusk and after: the
/// building it is in.
#[derive(Component)]
pub struct Firelight {
    pub home: usize,
}

/// How many homes' firelight is lit at once, the nearest the eye: unshadowed lights each
/// cost the GPU (docs/demos.md's budget: 16 near).
const LIT_HOMES: usize = 16;
/// How bright a hearth's fire is within, in lumens, and how far it reaches.
const FIRELIGHT_LUMENS: f32 = 400_000.0;
const FIRELIGHT_M: f32 = 6.0;

/// A forge's fire, whose smoke rises from it: the thing it is in the town.
#[derive(Component)]
pub struct ForgeFire {
    pub thing: usize,
}

/// Where a building's smoke comes out: the middle of its roof's highest cubes, a little over
/// them, in its own metres.
fn ridge(base: &Base) -> Option<Vec3> {
    let roof: Vec<IVec3> = base
        .cubes()
        .filter(|(_, cube)| cube.kind.is_roof())
        .map(|(at, _)| at)
        .collect();
    let top = roof.iter().map(|at| at.y).max()?;
    let highest: Vec<Vec3> = roof
        .iter()
        .filter(|at| at.y == top)
        .map(|at| (at.as_vec3() + Vec3::new(0.5, 1.0, 0.5)) * CELL_M)
        .collect();
    let middle = highest.iter().copied().sum::<Vec3>() / highest.len() as f32;
    // Toward one end of the ridge, as a smoke hole is over the hearth, not the door.
    let far = highest
        .iter()
        .copied()
        .max_by(|a, b| a.distance(middle).total_cmp(&b.distance(middle)))?;
    Some(middle.lerp(far, 0.45) + Vec3::Y * 0.15)
}

/// How hard a hearth burns at an hour: high for the morning's and the evening's cooking,
/// low between, banked at night; a house's own hours a little earlier or later.
fn hearth_burns(hour: f32, luck: f32) -> f32 {
    let hour = (hour + (luck - 0.5) * 1.5).rem_euclid(24.0);
    let cooking = |at: f32, wide: f32| (1.0 - ((hour - at) / wide).abs()).max(0.0);
    let meals = cooking(6.5, 2.5)
        .max(cooking(12.0, 1.5) * 0.7)
        .max(cooking(19.0, 3.0));
    (0.18 + meals * 0.82).min(1.0)
}

/// The hearths and the forge burning as the day has them: a home's while anyone of its
/// household is in or by it, the forge's while its smith works it.
fn burn(
    clock: Res<Clock>,
    market: Res<Market>,
    mut hearths: Query<(&Hearth, &mut Smoke), Without<ForgeFire>>,
    mut forges: Query<(&ForgeFire, &mut Smoke), Without<Hearth>>,
) {
    let life = &market.life;
    for (hearth, mut smoke) in &mut hearths {
        let door = life
            .town
            .things
            .iter()
            .position(|t| t.kind == ThingKind::Door && t.building == Some(hearth.home));
        let home = door.is_some_and(|door| {
            life.people.iter().any(|p| {
                p.home == door
                    && (p.state == terra_life::State::Inside
                        || p.at.distance(life.town.things[door].at) < 12.0)
            })
        });
        let luck = (hearth.home as f32 * 0.618_034) % 1.0;
        let wanted = if home {
            hearth_burns(clock.hour, luck)
        } else {
            0.1
        };
        if (smoke.thickness - wanted).abs() > 0.01 {
            smoke.thickness = wanted;
        }
    }
    for (forge, mut smoke) in &mut forges {
        let worked = (0..life.people.len()).any(|p| {
            life.activity(p) == Some("Smith")
                && matches!(life.people[p].state, terra_life::State::Doing { .. })
                && life.people[p]
                    .spot
                    .is_some_and(|s| life.spots[s].thing == forge.thing)
        });
        let wanted = if worked { 1.0 } else { 0.25 };
        if (smoke.thickness - wanted).abs() > 0.01 {
            smoke.thickness = wanted;
        }
    }
}

/// Each home's firelight lit when the light outside fails and its hearth burns with someone
/// home, the nearest the eye only, so the square empties into lit doorways at dusk.
fn light(
    clock: Res<Clock>,
    sky: Option<Res<super::daylight::SkyPath>>,
    market: Res<Market>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
    mut lights: Query<(&Firelight, &GlobalTransform, &mut PointLight)>,
) {
    // From when the sun is a few degrees up in the evening until it is up again.
    let dark = sky.map_or(0.0, |sky| {
        ((6.0 - sky.height(clock.hour)) / 8.0).clamp(0.0, 1.0)
    });
    let eye = camera
        .single()
        .map(|c| c.translation())
        .unwrap_or(market.middle);
    let life = &market.life;
    let mut near: Vec<(f32, usize)> = lights
        .iter()
        .enumerate()
        .map(|(n, (_, at, _))| (at.translation().distance(eye), n))
        .collect();
    near.sort_by(|a, b| a.0.total_cmp(&b.0));
    let nearest: Vec<usize> = near.iter().take(LIT_HOMES).map(|(_, n)| *n).collect();
    for (n, (fire, _, mut light)) in lights.iter_mut().enumerate() {
        let door = life
            .town
            .things
            .iter()
            .position(|t| t.kind == ThingKind::Door && t.building == Some(fire.home));
        let home = door.is_some_and(|door| {
            life.people
                .iter()
                .any(|p| p.home == door && p.state == terra_life::State::Inside)
        });
        let luck = (fire.home as f32 * 0.618_034) % 1.0;
        let wanted = if nearest.contains(&n) && home {
            FIRELIGHT_LUMENS * dark * (0.7 + 0.6 * luck)
        } else {
            0.0
        };
        if (light.intensity - wanted).abs() > 1.0 {
            light.intensity = wanted;
        }
    }
}

/// How bustling the town looks from the eye, every so often, as the life check counts it
/// (D326): its people out within 30 m and 100 m and in front of the eye, those walking, its
/// animals near, and the smoke rising.
fn report(
    time: Res<Time>,
    clock: Res<Clock>,
    market: Res<Market>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
    smoke: Query<(&Smoke, &GlobalTransform)>,
    mut wait: Local<f32>,
) {
    *wait -= time.delta_secs();
    if *wait > 0.0 {
        return;
    }
    *wait = 10.0;
    let Ok(eye) = camera.single() else {
        return;
    };
    let here = market.local(eye.translation());
    let ahead = market.local(eye.translation() + eye.forward() * 10.0) - here;
    let in_front = |at: Vec2| (at - here).dot(ahead) > 0.0;
    let life = &market.life;
    let out: Vec<usize> = (0..life.people.len())
        .filter(|&p| !matches!(life.shows(p), terra_life::Shows::Hidden))
        .collect();
    let within = |metres: f32| {
        out.iter()
            .filter(|&&p| {
                let at = life.people[p].at;
                at.distance(here) < metres && in_front(at)
            })
            .count()
    };
    let walking = out
        .iter()
        .filter(|&&p| {
            matches!(
                life.shows(p),
                terra_life::Shows::Walk(_) | terra_life::Shows::Run(_)
            )
        })
        .count();
    let animals = market
        .animals
        .animals
        .iter()
        .filter(|a| a.at.distance(here) < 30.0)
        .count();
    let columns = smoke
        .iter()
        .filter(|(s, at)| s.thickness > 0.3 && at.translation().distance(eye.translation()) < 200.0)
        .count();
    info!(
        "TOWN at {:02}:{:02}: {} of {} people out, {walking} walking; in front of the eye {} within 30 m and {} within 100 m; {animals} animals within 30 m; {columns} smoke columns within 200 m",
        clock.hour as u32,
        ((clock.hour.fract()) * 60.0) as u32,
        out.len(),
        life.people.len(),
        within(30.0),
        within(100.0),
    );
}

/// What grows in and round a town, as the world's ground cover asks it (`terra_world::cover`):
/// its streets and square trodden to bare earth, as are the ways round its stalls and the
/// thresholds of its doors; its yards worn thin by hens and feet; grass elsewhere as the
/// planet grows it.
struct TownCover {
    middle: Vec3,
    east: Vec3,
    south: Vec3,
    /// The town's middle in metres east and south of the start, for what it reaches.
    at: Vec2,
    square: Vec<Vec2>,
    streets: Vec<(Vec<Vec2>, f32)>,
    yards: Vec<[Vec2; 4]>,
    doors: Vec<Vec2>,
}

/// How far round its middle the town's cover is its own.
const COVER_REACH_M: f32 = 170.0;

impl terra_world::cover::CoverSource for TownCover {
    fn growing(&self, planet: &Planet, ground: Vec3, _at: Vec2) -> terra_world::cover::Growing {
        let offset = ground - self.middle;
        let p = Vec2::new(offset.dot(self.east), offset.dot(self.south));
        let wild = planet.cover(ground, ground.normalize());
        let mut growing = terra_world::cover::Growing {
            grass: wild.grass,
            lush: wild.lush,
            ..default()
        };
        // How far into a way a point is, by the nearest street or the square, a soft edge
        // where the grass comes back.
        let street = self
            .streets
            .iter()
            .map(|(points, wide)| {
                points
                    .windows(2)
                    .map(|pair| terra_build::town::point_segment(p, pair[0], pair[1]) - wide * 0.5)
                    .fold(f32::MAX, f32::min)
            })
            .fold(f32::MAX, f32::min);
        let square = if terra_build::town::inside(p, &self.square) {
            -1.0
        } else {
            terra_build::town::segment_polygon(p, p, &self.square)
        };
        let door = self
            .doors
            .iter()
            .map(|d| d.distance(p) - 1.2)
            .fold(f32::MAX, f32::min);
        let worn = street.min(square).min(door);
        if worn < 0.0 {
            growing.grass = 0.0;
            growing.path = true;
            // Trodden earth darkened and muddied by feet, wheels and what falls from carts,
            // in broken patches, not the planet's bare soil.
            growing.litter = 0.16;
            growing.roots = 0.42;
        } else if worn < 1.5 {
            growing.grass *= worn / 1.5 * 0.7;
        }
        if self
            .yards
            .iter()
            .any(|yard| terra_build::town::inside(p, yard))
        {
            growing.grass *= 0.35;
        }
        growing
    }

    fn reaches(&self, middle: Vec2, half: f32) -> bool {
        middle.distance(self.at) < COVER_REACH_M + half
    }
}

/// The catalogue item each thing in a town is drawn as, if it is drawn.
fn item_of(kind: ThingKind) -> Option<&'static str> {
    match kind {
        ThingKind::Well => Some("Well"),
        ThingKind::Stall => Some("MarketStall"),
        ThingKind::Forge => Some("Forge"),
        ThingKind::Bench | ThingKind::Block => Some("Log"),
        _ => None,
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        lay.run_if(resource_exists::<Wanted>)
            .run_if(not(resource_exists::<Market>)),
    )
    .add_systems(
        FixedUpdate,
        live.run_if(resource_exists::<Market>)
            .in_set(Sim::World)
            .in_set(Live),
    )
    .add_systems(
        Update,
        (burn, light, report).run_if(resource_exists::<Market>),
    );
}

/// The town's people's tick, which what draws them follows.
#[derive(SystemSet, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Live;

/// Lays the town out on the ground where it is wanted, and puts it up: its buildings by the
/// shortcuts, their doors and shutters, the things of its day as catalogue items, the
/// streets and square kept clear of what grows; then its people, placed where the clock
/// says they are.
#[allow(clippy::too_many_arguments)]
fn lay(
    mut commands: Commands,
    wanted: Res<Wanted>,
    site: Option<Res<Site>>,
    look: Option<Res<BuildLook>>,
    start: Res<Start>,
    planet: Res<Planet>,
    models: Res<Models>,
    builds: Res<Builds>,
    clock: Res<Clock>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut clear: MessageWriter<ClearAround>,
    mut warp: ResMut<super::WarpTo>,
    cover: Option<Res<terra_world::cover::CoverRules>>,
) {
    let has_cover = cover.is_some();
    let (Some(site), Some(look)) = (site, look) else {
        return;
    };
    let Some(palette) = look.kits.as_deref().and_then(build::village_kinds) else {
        warn!("Town: the town needs the settlement's kits.");
        return;
    };
    let plan = std::fs::read_to_string(terra_world::data::path("life.ron"))
        .map_err(|why| why.to_string())
        .and_then(|text| Plan::parse(&text));
    let plan = match plan {
        Ok(plan) => plan,
        Err(why) => {
            error!("Town: {why}");
            commands.remove_resource::<Wanted>();
            return;
        }
    };
    let place = site_for(&planet, &start, wanted.near, wanted.within);
    let middle = planet.ground_at(start.at(place.x, place.y));
    let up = middle.normalize();
    let east = (start.rotation * Vec3::X)
        .reject_from(up)
        .normalize_or(up.any_orthonormal_vector());
    let south = east.cross(up);
    let radius = middle.length();
    // The ground's height in the town's own terms: above the square's middle, the planet's
    // curve taken out, so a level street reads level.
    let heights = |at: Vec2| {
        let ground = planet.ground_at(middle + east * at.x + south * at.y);
        ground.length() - radius + at.length_squared() / (2.0 * radius)
    };
    let town = terra_build::town::town(wanted.seed, &palette, &heights);
    let at = |p: Vec2| planet.ground_at(middle + east * p.x + south * p.y);

    // The streets and the square, their plants cleared and kept off, trodden bare.
    for street in &town.streets {
        for pair in street.points.windows(2) {
            let steps = (pair[0].distance(pair[1]) / 3.0).ceil().max(1.0) as usize;
            for step in 0..=steps {
                let point = pair[0].lerp(pair[1], step as f32 / steps as f32);
                clear.write(ClearAround(at(point), street.wide * 0.5 + 1.5));
            }
        }
    }
    let frame = Transform::from_matrix(Mat4::from_cols(
        east.extend(0.0),
        up.extend(0.0),
        south.extend(0.0),
        middle.extend(1.0),
    ));
    let cells = |metres: f32| libm::floorf(metres / CELL_M) as i32;
    let mut ground = bevy::platform::collections::HashSet::new();
    let low = town
        .paths
        .nodes
        .iter()
        .map(|n| n.0)
        .reduce(Vec2::min)
        .unwrap_or_default()
        - 10.0;
    let high = town
        .paths
        .nodes
        .iter()
        .map(|n| n.0)
        .reduce(Vec2::max)
        .unwrap_or_default()
        + 10.0;
    let on_way = |p: Vec2| {
        terra_build::town::inside(p, &town.square.outline)
            || town.streets.iter().any(|street| {
                street.points.windows(2).any(|pair| {
                    terra_build::town::point_segment(p, pair[0], pair[1]) < street.wide * 0.5
                })
            })
    };
    for x in cells(low.x)..cells(high.x) {
        for z in cells(low.y)..cells(high.y) {
            let p = Vec2::new((x as f32 + 0.5) * CELL_M, (z as f32 + 0.5) * CELL_M);
            if on_way(p) {
                ground.insert(IVec2::new(x, z));
            }
        }
    }
    commands.spawn((
        Name::new("Town streets and square"),
        Footprint {
            frame,
            cell_m: CELL_M,
            ground,
            ..default()
        },
    ));

    // The buildings, each stood on the ground where its door is, by the shortcuts.
    let seed = wanted.seed;
    for (index, building) in town.buildings.iter().enumerate() {
        let door = at(Vec2::from(building.at));
        let up = door.normalize();
        let east = east
            .reject_from(up)
            .normalize_or(up.any_orthonormal_vector());
        let south = east.cross(up);
        let rotation = Quat::from_mat3(&Mat3::from_cols(east, up, south))
            * Quat::from_rotation_y(building.facing);
        let mut shape = Base::default();
        for &op in &building.ops {
            shape.apply(op);
        }
        let frame = build::lifted(
            Transform::from_translation(door).with_rotation(rotation),
            &shape,
            &planet,
        );
        clear.write(ClearAround(door, 9.0));
        // Its hearth's light within, a little inside its door, at a fire's height.
        if town.homes.contains(&index) {
            let within = frame.transform_point(Vec3::new(0.0, 1.0, -1.1));
            commands.spawn((
                Name::new(format!("Firelight of {}", building.name)),
                Firelight { home: index },
                PointLight {
                    color: Color::srgb(1.0, 0.58, 0.28),
                    intensity: 0.0,
                    range: FIRELIGHT_M,
                    radius: 0.3,
                    shadow_maps_enabled: false,
                    ..default()
                },
                Transform::from_translation(within),
            ));
        }
        // A home's hearth smokes through its roof's ridge, where a smoke hole would be.
        if town.homes.contains(&index)
            && let Some(ridge) = ridge(&shape)
        {
            commands.spawn((
                Name::new(format!("Hearth of {}", building.name)),
                Hearth { home: index },
                Smoke {
                    burning: Burning::Hearth,
                    thickness: 0.0,
                },
                Transform::from_translation(frame.transform_point(ridge)),
                Visibility::default(),
            ));
        }
        let label = format!("Town: {}", building.name);
        let mut built = Build::new(&mut commands, &look, frame, &label);
        if let Err(why) = built.put_up(&building.ops, &mut commands, &planet, &mut meshes) {
            warn!(
                "Town: {}: not put up by the operations: {why}",
                building.name
            );
        }
        // Its doors mostly shut, its shutters mostly open, one kind each a house.
        let pick = |hang, size: (i32, i32), salt: u32| {
            let kinds: Vec<_> = builds
                .fittings()
                .iter()
                .filter(|fitting| fitting.hang == hang && fitting.fits(size.0, size.1))
                .collect();
            let roll = |extra: u32| {
                let mut x = seed
                    ^ building.at[0].to_bits()
                    ^ building.at[1].to_bits().rotate_left(7)
                    ^ extra.wrapping_mul(0x9e37_79b9);
                x ^= x >> 16;
                x = x.wrapping_mul(0x7feb_352d);
                x ^= x >> 15;
                x
            };
            let fitting =
                kinds.get(roll(u32::from(hang == Hang::Door)) as usize % kinds.len().max(1))?;
            let open = match hang {
                Hang::Door => roll(salt + 7) % 3 == 0,
                Hang::Shutters => roll(salt + 11) % 4 != 0,
            };
            Some((fitting.kind, open))
        };
        build::props::fit_all(&mut built, |hang, size, index| {
            pick(hang, size, index as u32)
        });
        commands.spawn((Name::new(format!("{label}, as built")), Built(built)));
    }

    // The things of its day, placed as a player places an item, but the town's.
    let to_site = site.frame.compute_affine().inverse();
    let mut placed = 0;
    for thing in &town.things {
        let Some(file) = item_of(thing.kind) else {
            continue;
        };
        let Some(item) = builds.item_from(file) else {
            error!("Town: the town wants {file}, which is not in the catalogue.");
            continue;
        };
        let point = to_site.transform_point3(middle + east * thing.at.x + south * thing.at.y);
        // Its front the way the layout faces it: a turn about up, from the site's own.
        let front = to_site.transform_vector3(east * thing.front().x + south * thing.front().y);
        let turn = libm::atan2f(front.x, front.z).to_degrees();
        if let Some(entity) = props::place(
            &mut commands,
            &models,
            &site,
            &planet,
            &item,
            point.with_y(0.0),
            0.0,
            turn,
            false,
        ) {
            commands.entity(entity).insert(Theirs);
            placed += 1;
        }
        if thing.kind == ThingKind::Forge {
            let index = town
                .things
                .iter()
                .position(|t| std::ptr::eq(t, thing))
                .unwrap_or(0);
            let ground = at(thing.at);
            commands.spawn((
                Name::new("Forge fire"),
                ForgeFire { thing: index },
                Smoke {
                    burning: Burning::Forge,
                    thickness: 0.0,
                },
                Transform::from_translation(ground + ground.normalize() * 2.4),
                Visibility::default(),
            ));
        }
    }

    // Where to stand to see it, as the settlement gives its own (D69).
    let vantage = |name: &str, eye: Vec2, eye_up: f32, look: Vec2, look_up: f32| {
        let point = |p: Vec2, h: f32| {
            let ground = at(p);
            start.local(ground + ground.normalize() * h)
        };
        let (eye, look) = (point(eye, eye_up), point(look, look_up));
        info!(
            "Town vantage: {name}: --vantage {:.2},{:.2},{:.2},{:.2},{:.2},{:.2}",
            eye.x, eye.y, eye.z, look.x, look.y, look.z
        );
    };
    let along = town.square.along;
    let across = town.square.across;
    let (low_a, high_a) = town
        .square
        .outline
        .iter()
        .map(|p| p.dot(along))
        .fold((f32::MAX, f32::MIN), |(l, h), x| (l.min(x), h.max(x)));
    vantage(
        "square",
        along * (low_a + 3.0) + across * 2.0,
        1.7,
        along * high_a * 0.5,
        1.6,
    );
    vantage(
        "market",
        along * high_a * 0.6 - across * 4.0,
        1.7,
        -along * 4.0,
        1.4,
    );
    // A hen yard, and the smithy, from a few metres off.
    for (name, kind, off) in [
        ("yard", ThingKind::Yard, -7.0),
        ("forge", ThingKind::Forge, 6.0),
    ] {
        if let Some(thing) = town.things.iter().find(|t| t.kind == kind) {
            let eye = thing.place(Vec2::new(off * 0.5, off));
            vantage(name, eye, 1.7, thing.at, 0.6);
        }
    }
    vantage(
        "above",
        along * low_a * 1.6 + across * 30.0,
        28.0,
        Vec2::ZERO,
        0.0,
    );
    if let Some(street) = town.streets.first() {
        let n = street.points.len();
        // Down the street toward the square, from a little off its middle.
        let (eye, toward) = (street.points[n / 2], street.points[n / 5]);
        let aside = (toward - eye).normalize_or_zero().perp() * 1.2;
        vantage("street", eye + aside, 1.7, toward, 1.8);
        // Where to stand to walk in, as --at and --face take it.
        let (from, toward) = (
            start_terms(&start, at(street.points[n / 2])),
            start_terms(&start, middle),
        );
        info!(
            "Town arrival: --at {:.1},{:.1} --face {:.1},{:.1}",
            from.x, from.y, toward.x, toward.y
        );
        if wanted.arrive {
            warp.0 = Some(((from.x, from.y), Some((toward.x, toward.y))));
        }
        // A walk for a film: in along the main street from halfway out, across the square
        // past the market, and on up the other main street a way, as `--route` takes it.
        let mut walk: Vec<Vec2> = street.points[..=n / 2].iter().rev().copied().collect();
        walk.push(along * low_a * 0.3 - across * 2.0);
        walk.push(along * high_a * 0.3 + across * 2.0);
        if let Some(other) = town.streets.get(1) {
            walk.extend(other.points.iter().take(other.points.len() / 3));
        }
        let route: Vec<String> = walk
            .iter()
            .step_by(2)
            .map(|&p| {
                let l = start_terms(&start, at(p));
                format!("--route {:.1},{:.1}", l.x, l.y)
            })
            .collect();
        info!("Town route: {}", route.join(" "));
    }

    // Its ground cover, unless the world grows its own.
    if !has_cover {
        let at2 = start.local(middle);
        commands.insert_resource(terra_world::cover::CoverRules {
            source: std::sync::Arc::new(TownCover {
                middle,
                east,
                south,
                at: Vec2::new(at2.x, at2.z),
                square: town.square.outline.clone(),
                streets: town
                    .streets
                    .iter()
                    .map(|s| (s.points.clone(), s.wide))
                    .collect(),
                yards: town.yards.iter().map(|(_, quad)| *quad).collect(),
                doors: town
                    .things
                    .iter()
                    .filter(|t| t.kind == ThingKind::Door)
                    .map(|t| t.place(Vec2::new(0.0, 0.8)))
                    .collect(),
            }),
            seed: planet.seed().derive("town cover"),
        });
    }
    let life = Life::new(town, plan, seed, DAY_SECONDS / 24.0);
    let local = start.local(middle);
    info!(
        "Town: a {:?} square, {} streets, {} buildings ({} homes), {placed} things placed, {} people; its middle {:.0} m east and {:.0} m south of the start.",
        life.town.square.shape,
        life.town.streets.len(),
        life.town.buildings.len(),
        life.town.homes.len(),
        life.people.len(),
        local.x,
        local.z
    );
    let mut life = life;
    if !wanted.lived_in {
        life.people.clear();
    }
    life.start(Market::hours(&clock));
    let mut animals = Animals::new(&life, life.plan.animals.clone(), seed);
    if !wanted.lived_in {
        animals.animals.clear();
    }
    info!("Town: {} animals kept.", animals.animals.len());
    commands.insert_resource(Market {
        middle,
        east,
        south,
        up,
        life,
        animals,
    });
}

/// The town's people a tick on, the camera and every viewpoint their viewers.
fn live(
    time: Res<Time>,
    clock: Res<Clock>,
    mut market: ResMut<Market>,
    viewers: Query<&GlobalTransform, Or<(With<Viewpoint>, With<MainCamera>)>>,
) {
    let viewers: Vec<Vec2> = viewers
        .iter()
        .map(|seen| market.local(seen.translation()))
        .collect();
    let hours = Market::hours(&clock);
    let step = time.delta_secs();
    market.life.step(hours, step, &viewers);
    let market = &mut *market;
    market.animals.step(&market.life, step, &viewers);
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_build::village::{Kinds, Plan, Scheme};

    #[test]
    fn a_hearth_burns_for_the_meals_and_is_banked_at_night() {
        for luck in [0.0, 0.5, 1.0] {
            let at = |hour| hearth_burns(hour, luck);
            assert!(
                at(6.5) > at(15.0) && at(19.0) > at(15.0),
                "cooking is the most"
            );
            assert!(at(3.0) < 0.3 && at(3.0) > 0.0, "banked, not out, at night");
        }
    }

    #[test]
    fn a_house_smokes_from_over_its_ridge() {
        let kinds = Kinds {
            timber: terra_build::Kind(1),
            masonry: terra_build::Kind(2),
            beams: terra_build::Kind(3),
            cob: terra_build::Kind(4),
            framed: terra_build::Kind(5),
            timber_roof: terra_build::Kind(0x800),
            masonry_roof: terra_build::Kind(0x801),
            beams_roof: terra_build::Kind(0x802),
        };
        let town = terra_build::town::town(3, kinds, &|_| 0.0);
        let house = town
            .buildings
            .iter()
            .find(|b| b.plan == Plan::Rectangle && b.scheme != Scheme::Masonry)
            .expect("a rectangular house");
        let base = Base::from_shortcuts(&house.ops);
        let top = base
            .cubes()
            .filter(|(_, c)| c.kind.is_roof())
            .map(|(at, _)| (at.y + 1) as f32 * CELL_M)
            .fold(0.0, f32::max);
        let ridge = ridge(&base).expect("a roof");
        assert!(ridge.y > top, "{} under the roof's top {top}", ridge.y);
        // Within the house's own ground, not out over its door.
        assert!(ridge.z < 0.0);
    }
}
