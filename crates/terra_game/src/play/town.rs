//! A market town and its life (docs/demos.md, a living town; D324, D330): asked for at a
//! place ([`Wanted`]), laid out from its seed on the ground there (`terra_town::town`),
//! every building put up by the shortcuts a player's base is, its things placed as
//! catalogue items are; and its people living their days by `terra_town`, ticked with the
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
use terra_build::{Base, CELL_M};
use terra_gen::smoothstep;
use terra_town::town::{Size, ThingKind};
use terra_town::{Animals, Life, Plan};
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
    /// The hour its people's day is at, and the clock's when it was last seen: the clock's
    /// own while it runs; while it is held (a trailer's shot keeps the sun where it is), on
    /// at the day's pace, so its people still go on to what they do next.
    pub lived: f64,
    pub clock_seen: f64,
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

/// A torch in a bracket by a home's door, lit at dusk: its flame, with its light.
#[derive(Component)]
pub struct DoorTorch;

/// How many lights, homes' firelight and door torches, are lit at once, the nearest the
/// eye: unshadowed lights each cost the GPU (docs/demos.md's budget: 16 near). A torch's
/// flame shows however far it is.
const LIT_HOMES: usize = 16;
/// The share of homes with a torch by the door, and how bright one is, in lumens, and how
/// far its light reaches, as a torch in hand (`torchlight`).
const DOOR_TORCHES: f32 = 0.45;
const TORCH_LUMENS: f32 = 160_000.0;
const TORCH_M: f32 = 12.0;
/// How far up a door's side its torch's bracket is, how far it leans out from the wall, in
/// radians, and where its head is along it, from the bracket.
const BRACKET_M: f32 = 1.55;
const TORCH_LEAN: f32 = 0.45;
const TORCH_HEAD_M: f32 = 0.54;
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

/// The line of a building's roof's ridge, its two ends a little over its top cubes, in its
/// own metres: the two of its highest roof cubes farthest apart, pushed out half a cube along
/// it, where birds line up on it.
fn ridge_line(base: &Base) -> Option<(Vec3, Vec3)> {
    let roof: Vec<IVec3> = base
        .cubes()
        .filter(|(_, cube)| cube.kind.is_roof())
        .map(|(at, _)| at)
        .collect();
    let top = roof.iter().map(|at| at.y).max()?;
    let highest: Vec<Vec3> = roof
        .iter()
        .filter(|at| at.y == top)
        .map(|at| (at.as_vec3() + Vec3::new(0.5, 1.0, 0.5)) * CELL_M + Vec3::Y * 0.12)
        .collect();
    let (mut a, mut b, mut apart) = (highest[0], highest[0], 0.0);
    for (i, &p) in highest.iter().enumerate() {
        for &q in &highest[i + 1..] {
            if p.distance(q) > apart {
                (a, b, apart) = (p, q, p.distance(q));
            }
        }
    }
    let along = (b - a).normalize_or_zero() * CELL_M * 0.5;
    (apart >= CELL_M).then_some((a - along, b + along))
}

/// How hard a hearth burns at an hour: high for the morning's and the evening's cooking,
/// low between, banked at night; a house's own hours a little earlier or later.
fn hearth_burns(hour: f32, luck: f32) -> f32 {
    let hour = (hour + (luck - 0.5) * 1.5).rem_euclid(24.0);
    let cooking = |at: f32, wide: f32| (1.0 - ((hour - at) / wide).abs()).max(0.0);
    let meals = cooking(6.5, 2.5)
        .max(cooking(12.0, 1.5) * 0.7)
        .max(cooking(19.0, 3.0));
    // Not every house cooks on a big fire: some a pot on embers, so a few columns stand
    // thick over the roofs and most are a wisp.
    let big = (luck * 7.31).fract();
    (0.15 + meals * (0.25 + 0.6 * big)).min(1.0)
}

/// The hearths and the forge burning as the day has them: a home's while anyone of its
/// household is in or by it, the forge's while its smith works it.
fn burn(
    clock: Res<Clock>,
    market: Res<Market>,
    mut hearths: Query<(&Hearth, &mut Smoke), Without<ForgeFire>>,
    mut forges: Query<(&ForgeFire, &mut Smoke), Without<Hearth>>,
    rising: Option<Res<super::rising::Rising>>,
) {
    let life = &market.life;
    for (hearth, mut smoke) in &mut hearths {
        // Rising, a hearth is lit once its house is up and lived in (D381).
        if rising.as_ref().is_some_and(|r| !r.settled(hearth.home)) {
            smoke.thickness = 0.0;
            continue;
        }
        let door = life
            .town
            .things
            .iter()
            .position(|t| t.kind == ThingKind::Door && t.building == Some(hearth.home));
        let home = door.is_some_and(|door| {
            life.people.iter().any(|p| {
                p.home == door
                    && (p.state == terra_town::State::Inside
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
        if rising
            .as_ref()
            .is_some_and(|r| !r.reached(&life.town, life.town.things[forge.thing].at))
        {
            smoke.thickness = 0.0;
            continue;
        }
        let worked = (0..life.people.len()).any(|p| {
            life.activity(p) == Some("Smith")
                && matches!(life.people[p].state, terra_town::State::Doing { .. })
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
/// home, and the torches by doors lit, the nearest the eye only, so the square empties into
/// lit doorways at dusk. A torch's flame shows at any distance.
#[allow(clippy::type_complexity)]
fn light(
    clock: Res<Clock>,
    sky: Option<Res<super::daylight::SkyPath>>,
    market: Res<Market>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
    mut fires: Query<(&Firelight, &GlobalTransform, &mut PointLight), Without<DoorTorch>>,
    mut torches: Query<
        (&GlobalTransform, &mut PointLight, &mut Visibility),
        (With<DoorTorch>, Without<Firelight>),
    >,
    mut lit: ResMut<Lit>,
) {
    // From when the sun is ten degrees up in the evening until it is up again, as people
    // light their fires and torches before the light has gone.
    let dark = sky.map_or(0.0, |sky| {
        ((10.0 - sky.height(clock.hour)) / 10.0).clamp(0.0, 1.0)
    });
    let eye = camera
        .single()
        .map(|c| c.translation())
        .unwrap_or(market.middle);
    let life = &market.life;
    // Every light that would burn, by how far it is from the eye.
    let mut wanted: Vec<(f32, bool, usize, f32)> = Vec::new();
    for (n, (fire, at, _)) in fires.iter().enumerate() {
        let door = life
            .town
            .things
            .iter()
            .position(|t| t.kind == ThingKind::Door && t.building == Some(fire.home));
        let home = door.is_some_and(|door| {
            life.people.iter().any(|p| {
                p.home == door
                    && (p.state == terra_town::State::Inside
                        || p.at.distance(life.town.things[door].at) < 12.0)
            })
        });
        let luck = (fire.home as f32 * 0.618_034) % 1.0;
        if home && dark > 0.0 {
            let brightness = FIRELIGHT_LUMENS * dark * (0.7 + 0.6 * luck);
            wanted.push((at.translation().distance(eye), false, n, brightness));
        }
    }
    for (n, (at, _, _)) in torches.iter().enumerate() {
        if dark > 0.05 {
            wanted.push((at.translation().distance(eye), true, n, TORCH_LUMENS * dark));
        }
    }
    wanted.sort_by(|a, b| a.0.total_cmp(&b.0));
    wanted.truncate(LIT_HOMES);
    let brightness = |torch: bool, n: usize| {
        wanted
            .iter()
            .find(|w| w.1 == torch && w.2 == n)
            .map_or(0.0, |w| w.3)
    };
    lit.0 = wanted.len();
    for (n, (_, _, mut light)) in fires.iter_mut().enumerate() {
        let want = brightness(false, n);
        if (light.intensity - want).abs() > 1.0 {
            light.intensity = want;
        }
    }
    let shown = if dark > 0.05 {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for (n, (_, mut light, mut visible)) in torches.iter_mut().enumerate() {
        let want = brightness(true, n);
        if (light.intensity - want).abs() > 1.0 {
            light.intensity = want;
        }
        if *visible != shown {
            *visible = shown;
        }
    }
}

/// How many of the town's lights are lit now, for the report.
#[derive(Resource, Default)]
pub struct Lit(usize);

/// How bustling the town looks from the eye, every so often, as the life check counts it
/// (D326): its people out within 30 m and 100 m and in front of the eye, those walking, its
/// animals near, and the smoke rising.
fn report(
    time: Res<Time>,
    clock: Res<Clock>,
    market: Res<Market>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
    smoke: Query<(&Smoke, &GlobalTransform)>,
    lit: Res<Lit>,
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
        .filter(|&p| !matches!(life.shows(p), terra_town::Shows::Hidden))
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
                terra_town::Shows::Walk(_) | terra_town::Shows::Run(_)
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
        "TOWN at {:02}:{:02} (its day at {:02}:{:02}): {} of {} people out, {walking} walking; in front of the eye {} within 30 m and {} within 100 m; {animals} animals within 30 m; {columns} smoke columns within 200 m; {} lights lit",
        clock.hour as u32,
        ((clock.hour.fract()) * 60.0) as u32,
        (market.lived.rem_euclid(24.0)) as u32,
        (market.lived.fract() * 60.0) as u32,
        out.len(),
        life.people.len(),
        within(30.0),
        within(100.0),
        lit.0,
    );
}

/// What grows in and round a town, as the world's ground cover asks it (`terra_world::cover`):
/// its streets and square trodden to bare earth, as are the ways round its stalls and the
/// thresholds of its doors; its yards worn thin by hens and feet; grass elsewhere as the
/// planet grows it.
#[derive(Clone)]
struct TownCover {
    middle: Vec3,
    east: Vec3,
    south: Vec3,
    /// The town's middle in metres east and south of the start, for what it reaches.
    at: Vec2,
    square: Vec<Vec2>,
    streets: Vec<(Vec<Vec2>, f32)>,
    yards: Vec<[Vec2; 4]>,
    /// Its gardens within its wall, those laid out so far.
    gardens: super::town_gardens::GardenGround,
    /// The ground of each building going up in a settlement rising (D381).
    plots: Vec<[Vec2; 4]>,
    doors: Vec<Vec2>,
    /// How far each plot, and the square, is worn yet, 0 to 1: a settlement rising's ground is
    /// trodden over a few seconds of its time-lapse as its builders come and go (D381), not all
    /// at once; a plot not given one is worn through.
    plots_worn: Vec<f32>,
    square_worn: f32,
    /// Within so far of its middle a meadow, grass all over but where it is worn: the
    /// clearing a settlement rising has cut (D381); none for a town of old.
    meadow_m: f32,
    /// How far round its middle its cover is its own (`COVER_REACH_M`, further for a
    /// settlement grown bigger).
    reach_m: f32,
    /// Whether its ways are paved with setts, as an old town's are; a settlement rising's are
    /// packed earth.
    paved: bool,
}

/// How thick and how green a settlement's cleared meadow grows at least
/// (`TownCover::meadow_m`).
const MEADOW_GRASS: f32 = 0.8;
const MEADOW_LUSH: f32 = 0.65;
/// How trodden an unpaved way is: packed earth, under where the ground shader lays setts.
const UNPAVED: f32 = 0.5;
/// How far round a plot going up its ground is trodden.
const PLOT_WORN_M: f32 = 1.8;
/// How far round its middle a settlement rising has cleared a meadow at most: to the forest's
/// edge and down to its lake (`MarketTown::SETTLEMENT`).
const MEADOW_M: f32 = 260.0;
/// Its meadow is where the forest's clearings keep less than this share of its stand: the
/// ground it has cleared, toward its edge bracken and shrubs coming back.
const MEADOW_KEPT: f32 = 0.6;

/// How far inside a street's or the square's edge its paving starts.
const PAVING_EDGE_M: f32 = 0.35;

/// How far round a house's door its ground is worn, as far as the town clears it.
const HOUSE_WORN_M: f32 = 9.0;
/// Within how far of a door it is worn as bare as a yard.
const HOUSE_BARE_M: f32 = 4.5;

/// How far ground being worn, `worn` 0 to 1 of the way, is trodden and how far its grass has
/// given out: the treading comes first and the grass goes after it, as feet wear a meadow to
/// earth, so the ground between is never bare soil.
fn treading(worn: f32) -> (f32, f32) {
    let w = worn.clamp(0.0, 1.0);
    (libm::sqrtf(w), w * w)
}

/// Smooth value noise from 0 to 1 over a place, a unit a cell: the drifts of a meadow's mix.
fn patch(p: Vec2) -> f32 {
    let hash = |x: i32, y: i32| {
        let h = terra_gen::hash::prospector32_round(
            (x as u32).wrapping_mul(0x9e37_79b9) ^ (y as u32).wrapping_mul(0x85eb_ca6b),
        );
        (h & 0xffff) as f32 / 65535.0
    };
    let (x, y) = (libm::floorf(p.x), libm::floorf(p.y));
    let (fx, fy) = (p.x - x, p.y - y);
    let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let (x, y) = (x as i32, y as i32);
    let top = hash(x, y) + (hash(x + 1, y) - hash(x, y)) * sx;
    let bottom = hash(x, y + 1) + (hash(x + 1, y + 1) - hash(x, y + 1)) * sx;
    top + (bottom - top) * sy
}

/// How far round its middle the town's cover is its own.
const COVER_REACH_M: f32 = 170.0;

impl TownCover {
    /// How much of the forest's stand the clearings round a settlement rising keep at a
    /// point: 0 in its meadow, 1 in the forest; 1 for a town of old, which has none of its own.
    fn forest_kept(&self, planet: &Planet, ground: Vec3, p: Vec2) -> f32 {
        if p.length() >= self.meadow_m {
            return 1.0;
        }
        planet.round().map_or(0.0, |round| {
            round.clearings.iter().map(|c| c.keeps(ground)).product()
        })
    }
}

impl terra_world::cover::CoverSource for TownCover {
    fn growing(&self, planet: &Planet, ground: Vec3, _at: Vec2) -> terra_world::cover::Growing {
        let offset = ground - self.middle;
        let p = Vec2::new(offset.dot(self.east), offset.dot(self.south));
        let wild = planet.cover(ground, ground.normalize());
        let kept = self.forest_kept(planet, ground, p);
        let meadow = kept < MEADOW_KEPT;
        let mut growing = terra_world::cover::Growing {
            grass: if meadow {
                wild.grass.max(MEADOW_GRASS)
            } else {
                wild.grass
            },
            // A cleared meadow grazed and trodden green, not left to last year's straw.
            lush: if meadow {
                wild.lush.max(MEADOW_LUSH)
            } else {
                wild.lush
            },
            ..default()
        };
        // A meadow is a mix (D381): taller grass in drifts, greener and drier patches, and
        // toward the forest's edge bracken and low shrubs coming back into the clearing.
        if meadow {
            growing.tall = (patch(p / 9.0) - 0.35).max(0.0) * 1.5;
            growing.lush = (growing.lush + (patch(p / 14.0 + 31.0) - 0.5) * 0.4).clamp(0.0, 1.0);
            let edge = (kept / MEADOW_KEPT).clamp(0.0, 1.0);
            growing.shrub = edge * (patch(p / 5.0 + 77.0) - 0.45).max(0.0) * 1.2;
            // A lowland lake's bank in a meadow: rank grass and sedge thick and tall along the
            // waterline in clumps, as reeds and sedge fringe a lake by a pasture, hiding where
            // the water meets the bank (D381). The lakes' grid is asked first, which is cheap.
            if let Some(level) = planet
                .round()
                .and_then(|round| round.water_near(ground.normalize(), 1))
            {
                let over = ground.length() - level;
                let band = smoothstep(-0.15, 0.05, over) * (1.0 - smoothstep(0.6, 1.4, over));
                let fringe = band * (0.55 + 0.9 * patch(p / 4.0 + 13.0)).min(1.0);
                growing.grass = growing.grass.max(fringe);
                growing.tall = growing.tall.max(fringe * 1.3);
                growing.lush = growing.lush.max(fringe);
            }
        }
        // How far into a way a point is, by the nearest street or the square, a soft edge
        // where the grass comes back.
        let street = self
            .streets
            .iter()
            .map(|(points, wide)| {
                points
                    .windows(2)
                    .map(|pair| terra_town::town::point_segment(p, pair[0], pair[1]) - wide * 0.5)
                    .fold(f32::MAX, f32::min)
            })
            .fold(f32::MAX, f32::min);
        // No square laid yet, none: a polygon of no corners holds every point.
        let square = if self.square.len() < 3 {
            f32::MAX
        } else if terra_town::town::inside(p, &self.square) {
            -1.0
        } else {
            terra_town::town::segment_polygon(p, p, &self.square)
        };
        let door = self
            .doors
            .iter()
            .map(|d| d.distance(p) - 1.2)
            .fold(f32::MAX, f32::min);
        let worn = street.min(square).min(door);
        // The square laid only so far yet: the meadow as it was, worn toward its laid ground.
        let unworn = (square <= street.min(door) && self.square_worn < 1.0).then_some(growing);
        if worn < 0.0 && !self.paved && door <= street.min(square) {
            // Before a new house's door, trodden by its builders a few days: the grass thinned
            // and earth through it, not cut to a bare patch the meadow's own green (D381, the
            // review of cut 3: bright green squares under every site's logs).
            growing.grass *= 0.3;
            growing.trodden = 0.7;
        } else if worn < 0.0 {
            growing.grass = 0.0;
            growing.path = true;
            // Paved with setts (the ground shader's from 0.85, D350), packed earth at the very
            // edge of a way where the paving gives out and straw and dung collect.
            growing.trodden = if !self.paved {
                // A settlement new-founded: its ways packed earth, not yet paved.
                UNPAVED
            } else if -worn > PAVING_EDGE_M {
                1.0
            } else {
                0.75
            };
        } else if worn < 1.5 {
            growing.grass *= worn / 1.5 * 0.7;
            growing.trodden = 0.6 * (1.0 - worn / 1.5);
            // Duff gathers where the grass comes back along an old way's edge, not yet along a
            // new one's.
            if self.paved {
                growing.roots = 0.35 * (1.0 - worn / 1.5);
            }
        }
        if let Some(before) = unworn {
            // Trodden first, the grass giving out after, so earth comes through the grass and
            // never the bare soil between.
            let (tread, thin) = treading(self.square_worn);
            growing.grass = before.grass + (growing.grass - before.grass) * thin;
            growing.trodden = before.trodden + (growing.trodden - before.trodden) * tread;
            growing.roots = before.roots + (growing.roots - before.roots) * thin;
            growing.path = growing.path && thin > 0.5;
        }
        if self
            .yards
            .iter()
            .any(|yard| terra_town::town::inside(p, yard))
        {
            // Worn by hens and feet, straw strewn about; a new settlement's still mostly
            // grass, grazed short.
            let (grass, trodden) = if self.paved { (0.35, 0.45) } else { (0.6, 0.3) };
            growing.grass *= grass;
            growing.trodden = growing.trodden.max(trodden);
        }
        self.gardens.grow(p, &mut growing);
        // A plot laid for a building going up, trodden to earth round its footings as
        // builders work about it (D381): not the blades cut away from a lawn.
        let plot_worn = self
            .plots
            .iter()
            .enumerate()
            .filter(|(_, plot)| {
                terra_town::town::inside(p, &plot[..])
                    || (0..4).any(|i| {
                        terra_town::town::point_segment(p, plot[i], plot[(i + 1) % 4]) < PLOT_WORN_M
                    })
            })
            .map(|(i, _)| self.plots_worn.get(i).copied().unwrap_or(1.0))
            .fold(0.0f32, f32::max);
        if plot_worn > 0.0 {
            let (tread, thin) = treading(plot_worn);
            growing.grass *= 1.0 - 0.6 * thin;
            growing.trodden = growing.trodden.max(0.45 * tread);
        }
        // Round every house, where the town clears what grows (`ClearAround` at its door),
        // the ground is worn by feet going about it, not left the planet's bare soil.
        if door < HOUSE_WORN_M {
            // Worn bare round the house, thinning toward where the clearing ends.
            let thins = ((door - HOUSE_BARE_M) / (HOUSE_WORN_M - HOUSE_BARE_M)).clamp(0.0, 1.0);
            growing.trodden = growing.trodden.max(0.6 * (1.0 - thins));
        }
        growing
    }

    fn reaches(&self, middle: Vec2, half: f32) -> bool {
        middle.distance(self.at) < self.reach_m + half
    }

    /// A settlement's cleared meadow is grass all over but where it is worn, the land's bare
    /// patches too; an old town's ground is the land's.
    fn sows(&self) -> bool {
        self.meadow_m > 0.0
    }
}

/// A town's cover within its reach, and past it what the world grows, on a world that grows
/// its own (the land's heath and meadow round a town the land's rules found, `--site town`).
struct TownOverWorld {
    town: TownCover,
    world: std::sync::Arc<dyn terra_world::cover::CoverSource>,
}

impl terra_world::cover::CoverSource for TownOverWorld {
    fn growing(&self, planet: &Planet, ground: Vec3, at: Vec2) -> terra_world::cover::Growing {
        // A settlement rising's own cover is its meadow and its ways; the forest round it is
        // the world's, its needles, litter and moss (D381).
        let theirs = if self.town.meadow_m > 0.0 {
            let offset = ground - self.town.middle;
            let p = Vec2::new(offset.dot(self.town.east), offset.dot(self.town.south));
            self.town.forest_kept(planet, ground, p) < MEADOW_KEPT
        } else {
            at.distance(self.town.at) < COVER_REACH_M
        };
        if theirs {
            self.town.growing(planet, ground, at)
        } else {
            self.world.growing(planet, ground, at)
        }
    }

    fn reaches(&self, middle: Vec2, half: f32) -> bool {
        self.town.reaches(middle, half) || self.world.reaches(middle, half)
    }

    fn sows(&self) -> bool {
        self.town.sows()
    }
}

/// A settlement rising's ground, worn as it is built (D381): its town's cover as it will
/// be, the world's under it, and which building each door and yard is of.
#[derive(Resource)]
pub struct Wear {
    world: Option<terra_world::cover::CoverRules>,
    whole: TownCover,
    doors: Vec<Option<usize>>,
    yards: Vec<usize>,
    /// Its gardens, each laid out as its home is up.
    gardens: terra_town::town::Gardens,
    /// Each building's ground, by the building.
    plots: Vec<(usize, [Vec2; 4])>,
}

impl Wear {
    /// What grows with the plots of the buildings `started` trodden (each as far as
    /// `started` says, 0 to 1), the doors and yards of those `up` worn round, the streets
    /// trodden out from the square as far as `reach_m`, where the settlement has got to, and
    /// the square as far as `square` says.
    pub fn rules(
        &self,
        planet: &Planet,
        started: impl Fn(usize) -> Option<f32>,
        up: impl Fn(usize) -> bool,
        reach_m: f32,
        square: f32,
    ) -> terra_world::cover::CoverRules {
        let worn = TownCover {
            streets: self
                .whole
                .streets
                .iter()
                .map(|(points, wide)| {
                    let near: Vec<Vec2> = points
                        .iter()
                        .copied()
                        .take_while(|p| p.length() <= reach_m)
                        .collect();
                    (near, *wide)
                })
                .filter(|(points, _)| points.len() >= 2)
                .collect(),
            doors: self
                .whole
                .doors
                .iter()
                .zip(&self.doors)
                .filter(|(_, b)| b.is_some_and(&up))
                .map(|(door, _)| *door)
                .collect(),
            yards: self
                .whole
                .yards
                .iter()
                .zip(&self.yards)
                .filter(|(_, b)| up(**b))
                .map(|(yard, _)| *yard)
                .collect(),
            gardens: super::town_gardens::GardenGround::new(&self.gardens, &up),
            plots: self
                .plots
                .iter()
                .filter(|(b, _)| started(*b).is_some())
                .map(|(_, plot)| *plot)
                .collect(),
            plots_worn: self.plots.iter().filter_map(|(b, _)| started(*b)).collect(),
            square: if square > 0.0 {
                self.whole.square.clone()
            } else {
                Vec::new()
            },
            square_worn: square.clamp(0.0, 1.0),
            ..self.whole.clone()
        };
        over_the_worlds(self.world.as_ref(), planet, worn)
    }
}

/// The rules for what grows once a town is laid: its own, over the world's if it has any.
fn over_the_worlds(
    world: Option<&terra_world::cover::CoverRules>,
    planet: &Planet,
    town: TownCover,
) -> terra_world::cover::CoverRules {
    match world {
        Some(world) => terra_world::cover::CoverRules {
            source: std::sync::Arc::new(TownOverWorld {
                town,
                world: world.source.clone(),
            }),
            seed: world.seed,
        },
        None => terra_world::cover::CoverRules {
            source: std::sync::Arc::new(town),
            seed: planet.seed().derive("town cover"),
        },
    }
}

/// The catalogue item each thing in a town is drawn as, if it is drawn.
fn item_of(kind: ThingKind) -> Option<&'static str> {
    match kind {
        ThingKind::Well => Some("Well"),
        ThingKind::Stall => Some("MarketStall"),
        ThingKind::Forge => Some("Forge"),
        ThingKind::Bench => Some("Bench"),
        ThingKind::Block => Some("Log"),
        ThingKind::Trough => Some("Trough"),
        ThingKind::Barrel => Some("Barrel"),
        ThingKind::Cart => Some("HandCart"),
        _ => None,
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        lay.run_if(resource_exists::<Wanted>)
            .run_if(not(resource_exists::<Market>))
            .in_set(Laid),
    )
    .add_systems(
        FixedUpdate,
        live.run_if(resource_exists::<Market>)
            .in_set(Sim::World)
            .in_set(Live),
    )
    .init_resource::<Lit>()
    .add_systems(
        Update,
        (burn, light, report).run_if(resource_exists::<Market>),
    );
}

/// How wide a player is to the town's walkers; and the camera, wider, so they keep out of
/// the lens rather than brush it, where they would be faded out half seen.
const VISITOR_M: f32 = 0.4;
const LENS_M: f32 = 1.0;

/// The town laid out, which what changes it after follows.
#[derive(SystemSet, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Laid;

/// The town's people's tick, which what draws them follows.
#[derive(SystemSet, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Live;

/// Lays the town out on the ground where it is wanted, and puts it up: its buildings by the
/// shortcuts, their doors and shutters, the things of its day as catalogue items, the
/// streets and square kept clear of what grows; then its people, placed where the clock
/// says they are.
#[allow(clippy::too_many_arguments)]
/// The corners of a box round a town's ways, ten metres out past them.
fn ways_bounds(town: &terra_town::town::Town) -> (Vec2, Vec2) {
    let nodes = || town.paths.nodes.iter().map(|n| n.0);
    let low = nodes().reduce(Vec2::min).unwrap_or_default();
    (
        low - 10.0,
        nodes().reduce(Vec2::max).unwrap_or_default() + 10.0,
    )
}

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
    assets: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    (mut obstacles, rising, mut perches): (
        Option<ResMut<super::flora::Obstacles>>,
        Option<Res<super::rising::Rising>>,
        Option<ResMut<super::wildlife::Perches>>,
    ),
) {
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
    let mut plan = match plan {
        Ok(plan) => plan,
        Err(why) => {
            error!("Town: {why}");
            commands.remove_resource::<Wanted>();
            return;
        }
    };
    // A settlement rising has as many builders as its time-lapse asks (D381).
    if let Some(builders) = rising.as_ref().and_then(|r| r.lapse.builders) {
        for role in plan.roles.iter_mut().filter(|r| r.name == "builder") {
            role.count = terra_town::plan::Count::Fixed(builders);
        }
    }
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
    // A settlement rising grows as big as its time-lapse asks (D381).
    let size = rising
        .as_ref()
        .and_then(|r| r.lapse.town)
        .map_or(Size::MARKET, |g| g.size());
    let grown = size.streets;
    let mut town = terra_town::town::town_sized(wanted.seed, &palette, &heights, &size);
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
    let (low, high) = ways_bounds(&town);
    let on_way = |p: Vec2| {
        terra_town::town::inside(p, &town.square.outline)
            || town.streets.iter().any(|street| {
                street.points.windows(2).any(|pair| {
                    terra_town::town::point_segment(p, pair[0], pair[1]) < street.wide * 0.5
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
    // Rising, its ways are trodden out as it grows (`Wear`), not cleared from the first.
    if rising.is_none() {
        commands.spawn((
            Name::new("Town streets and square"),
            Footprint {
                frame,
                cell_m: CELL_M,
                ground,
                ..default()
            },
        ));
    }

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
        if rising.is_none() {
            clear.write(ClearAround(door, 9.0));
            // Its ridge, for the town's birds to sit on, once it stands.
            if let (Some(perches), Some((a, b))) = (perches.as_mut(), ridge_line(&shape)) {
                perches
                    .0
                    .push((frame.transform_point(a), frame.transform_point(b)));
            }
        }
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
        // A settlement rising (D381): the building as it will be, left to go up a step at
        // a time from its plan, its doors and shutters fitted once it is up.
        if rising.is_some() {
            let whole = built.ready_to_rise(&building.ops, &mut commands, &planet, &mut meshes);
            commands.spawn((
                Name::new(format!("{label}, rising")),
                Built(built),
                super::rising::Rises {
                    index,
                    whole,
                    planned: false,
                    near: Vec3::ZERO,
                    called: false,
                    dressed: Vec::new(),
                },
            ));
            continue;
        }
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
                terra_gen::hash::lowbias32_round(
                    seed ^ building.at[0].to_bits()
                        ^ building.at[1].to_bits().rotate_left(7)
                        ^ extra.wrapping_mul(0x9e37_79b9),
                )
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
    // Each stall's look, its awning's dye drawn by the seed (`market.ron`'s `stalls`).
    let looks: std::collections::HashMap<usize, String> =
        std::fs::read_to_string(terra_world::data::path("market.ron"))
            .ok()
            .and_then(|text| terra_town::town::Market::parse(&text).ok())
            .map(|market| market.looks(&town, wanted.seed).into_iter().collect())
            .unwrap_or_default();
    for (index, thing) in town.things.iter().enumerate() {
        let Some(file) = looks
            .get(&index)
            .map(String::as_str)
            .or_else(|| item_of(thing.kind))
        else {
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

    // The goods on and by the market's stalls (D352), placed as its things are.
    let on_site = |p: Vec2, facing: f32| {
        let point = to_site.transform_point3(middle + east * p.x + south * p.y);
        let way = terra_town::town::looking(facing);
        let front = to_site.transform_vector3(east * way.x + south * way.y);
        (
            point.with_y(0.0),
            libm::atan2f(front.x, front.z).to_degrees(),
        )
    };
    placed += super::stall_goods::lay(
        &mut commands,
        &models,
        &builds,
        &site,
        &planet,
        &town,
        wanted.seed,
        &at,
        &on_site,
    );

    // Torches in brackets by some homes' doors, beside the door and leaning out from the
    // wall, lit at dusk (`light`).
    let flame = (
        meshes.add(Sphere::new(0.06)),
        materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.6, 0.2),
            emissive: LinearRgba::rgb(14.0, 5.0, 1.2),
            unlit: true,
            ..default()
        }),
    );
    let torch = assets.load(
        bevy::gltf::GltfAssetLabel::Scene(0).from_asset(crate::character::held_look("torch")),
    );
    for (index, thing) in town.things.iter().enumerate() {
        if thing.kind != ThingKind::Door || !thing.building.is_some_and(|b| town.homes.contains(&b))
        {
            continue;
        }
        let luck = ((index as f32 + 1.0) * 0.618_034) % 1.0;
        if luck >= DOOR_TORCHES {
            continue;
        }
        let side = if luck < DOOR_TORCHES * 0.5 { 0.8 } else { -0.8 };
        let foot = at(thing.place(Vec2::new(side, 0.08)));
        let up = foot.normalize();
        let out = (east * thing.front().x + south * thing.front().y)
            .reject_from(up)
            .normalize_or(east);
        let rotation = Quat::from_axis_angle(up.cross(out).normalize_or(east), TORCH_LEAN)
            * Quat::from_rotation_arc(Vec3::Y, up);
        commands
            .spawn((
                Name::new("Door torch"),
                bevy::world_serialization::WorldAssetRoot(torch.clone()),
                Transform::from_translation(foot + up * BRACKET_M).with_rotation(rotation),
                Visibility::default(),
            ))
            .with_child((
                DoorTorch,
                Mesh3d(flame.0.clone()),
                MeshMaterial3d(flame.1.clone()),
                bevy::light::NotShadowCaster,
                PointLight {
                    color: Color::srgb(1.0, 0.62, 0.3),
                    intensity: 0.0,
                    range: TORCH_M,
                    radius: 0.05,
                    shadow_maps_enabled: false,
                    ..default()
                },
                Transform::from_xyz(0.0, TORCH_HEAD_M, 0.0),
                Visibility::Hidden,
            ));
    }
    // Its yards' fences, a module at a time, each solid.
    let fences = super::town_fences::lay(
        &mut commands,
        &models,
        &builds,
        &planet,
        obstacles.as_deref_mut(),
        at,
        &town.fences,
    );
    info!("Town: {} fence modules round its yards.", fences);

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
        // Where the square and its market are, in the terms a route takes, to plan one by.
        let spot = |p: Vec2| {
            let l = start_terms(&start, at(p));
            format!("{:.1},{:.1}", l.x, l.y)
        };
        let marked = |kind: ThingKind| {
            town.things
                .iter()
                .filter(|t| t.kind == kind)
                .map(|t| spot(t.at))
                .collect::<Vec<_>>()
                .join(" ")
        };
        info!(
            "Town square: corners {}; well {}; stalls {}; benches {}",
            town.square
                .outline
                .iter()
                .map(|&p| spot(p))
                .collect::<Vec<_>>()
                .join(" "),
            marked(ThingKind::Well),
            marked(ThingKind::Stall),
            marked(ThingKind::Bench),
        );
    }

    // Its ground cover, over what the world grows round it if it grows its own.
    {
        let at2 = start.local(middle);
        let whole = TownCover {
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
            gardens: super::town_gardens::GardenGround::new(&town.gardens, |_| true),
            plots: Vec::new(),
            doors: town
                .things
                .iter()
                .filter(|t| t.kind == ThingKind::Door)
                .map(|t| t.place(Vec2::new(0.0, 0.8)))
                .collect(),
            plots_worn: Vec::new(),
            square_worn: 1.0,
            meadow_m: if rising.is_some() {
                MEADOW_M * grown
            } else {
                0.0
            },
            reach_m: COVER_REACH_M * grown,
            paved: rising.is_none(),
        };
        if rising.is_some() {
            // Rising, a house's ground is worn once its plot is laid and its yard once it
            // is up (`Wear`); the streets and the square are laid out from the first.
            let wear = Wear {
                world: cover.as_deref().cloned(),
                doors: town
                    .things
                    .iter()
                    .filter(|t| t.kind == ThingKind::Door)
                    .map(|t| t.building)
                    .collect(),
                yards: town.yards.iter().map(|(b, _)| *b).collect(),
                gardens: town.gardens.clone(),
                plots: town
                    .footprints
                    .iter()
                    .enumerate()
                    .flat_map(|(b, quads)| quads.iter().map(move |q| (b, *q)))
                    .collect(),
                whole,
            };
            commands.insert_resource(wear.rules(&planet, |_| None, |_| false, 0.0, 0.0));
            commands.insert_resource(wear);
        } else {
            commands.insert_resource(over_the_worlds(cover.as_deref(), &planet, whole));
        }
    }
    // A settlement rising has its builders' places round every building (D381).
    if rising.is_some() {
        town.add_sites();
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
    // Its day as if it had been going on since the night, not everyone stood where the hour
    // first puts them (`Life::start_settled`).
    let outline = &life.town.square.outline;
    let square = outline.iter().copied().sum::<Vec2>() / outline.len().max(1) as f32;
    life.start_settled(Market::hours(&clock), square);
    let mut animals = Animals::new(&life, life.plan.animals.clone(), seed);
    animals.settle(&life);
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
        lived: f64::NAN,
        clock_seen: f64::NAN,
    });
}

/// The hour the town's day is at: the clock's `now` while it runs, and while it stands where
/// it was last seen (`seen`), the day's own pace on from `lived` by `step` seconds.
fn lived_hours(now: f64, seen: f64, lived: f64, step: f32) -> f64 {
    let pace = step as f64 * 24.0 / DAY_SECONDS as f64;
    // Held, or as good as: it moved less than half the day's pace (a held clock is set
    // again each frame and need not read the same to the last bit).
    if lived.is_finite() && (now - seen).abs() < pace * 0.5 {
        lived + pace
    } else {
        now
    }
}

/// The town's people a tick on, the camera and every viewpoint their viewers.
fn live(
    time: Res<Time>,
    clock: Res<Clock>,
    rising: Option<Res<super::rising::Rising>>,
    mut market: ResMut<Market>,
    viewers: Query<&GlobalTransform, Or<(With<Viewpoint>, With<MainCamera>)>>,
    visitors: Query<
        (&GlobalTransform, Has<MainCamera>),
        Or<(With<super::Player>, With<MainCamera>)>,
    >,
) {
    let viewers: Vec<Vec2> = viewers
        .iter()
        .map(|seen| market.local(seen.translation()))
        .collect();
    // Players and the camera are walked round, as the town's people walk round each other,
    // so nobody walks through a player or into the lens.
    let visitors = visitors
        .iter()
        .map(|(seen, lens)| terra_town::orca::Body {
            at: market.local(seen.translation()),
            velocity: Vec2::ZERO,
            radius: if lens { LENS_M } else { VISITOR_M },
        })
        .collect();
    market.life.visitors = visitors;
    // A time-lapse runs the sun through the day, and its people their own day a few times
    // faster than the film, as a time-lapse shows people hurrying about (D381).
    let pace = rising.as_ref().map_or(1.0, |r| r.lapse.pace);
    let step = time.delta_secs() * pace;
    let now = Market::hours(&clock);

    let hours = if rising.is_some() && market.lived.is_finite() {
        market.lived + step as f64 * 24.0 / DAY_SECONDS as f64
    } else {
        lived_hours(now, market.clock_seen, market.lived, step)
    };
    market.clock_seen = now;
    market.lived = hours;
    market.life.step(hours, step, &viewers);
    let market = &mut *market;
    market.animals.step(&market.life, step, &viewers);
}

#[cfg(test)]
mod tests;
