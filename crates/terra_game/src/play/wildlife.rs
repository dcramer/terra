//! Wildlife about the land (creatures.md): the small animals found wherever their niche is,
//! put out round each player as they come near and taken away once everyone has gone, as a
//! shipped game's ambient population is (Red Dead's ambient animals, Far Cry's wildlife by
//! habitat). Which kinds live where is the kinds' own data (`Lives::About` and `Niche` in
//! `assets/data/creatures.ron`): how warm a growing season each takes and the places it keeps
//! to, read from the world as it is, so a new land has its animals where its climate and
//! cover put them, with no list for any map (D127, D136).
//!
//! The land is divided into cells; a cell near a player is looked at once, and what lives in
//! it is rolled from its own seed, so the same walk meets the same animals. A cell nobody is
//! near any more lets its animals go. Far from every player nothing here runs at all (D145).

use super::creatures::{self, Kinds};
use super::lod::Viewpoint;
use super::species::{Bestiary, Haunt, Lives, Niche, Species};
use crate::planet::Planet;
use crate::start::Start;
use bevy::{ecs::system::SystemParam, platform::collections::HashMap, prelude::*};
use terra_gen::Seed;
use terra_world::cover::CoverRules;
use terra_world::flora::{Obstacles, Trunk, Trunks};

/// The land's cells are cubes this wide, a few tens of metres of ground in each.
const CELL_M: f32 = 32.0;
/// Cells within this of a player are put out; past this, let go.
const POPULATE_M: f32 = 110.0;
const RELEASE_M: f32 = 150.0;
/// At most so many cells new to a look are looked into by it.
const FRESH_A_LOOK: usize = 3;
/// How often the cells round each player are looked at.
const LOOK_EVERY_S: f32 = 0.25;
/// At most so many wild animals about at once, for every player together.
const MOST: usize = 48;
/// A wood is at least so many trunks within so far; open ground has none that near.
const WOODED: (usize, f32) = (2, 10.0);
const OPEN_M: f32 = 14.0;
/// The shore is dry ground within so far of water, or water as shallow as this.
const SHORE_M: f32 = 4.0;
const SHALLOWS_M: f32 = 0.35;
/// Water deep enough to swim on.
const SWIMMING_M: f32 = 0.4;
/// Ground trodden at least this much is a settlement's (`Growing::trodden`).
const TRODDEN: f32 = 0.4;
/// A player on the move meets something about this often, seconds: a group put out ahead
/// of them, beside their way and past where it is drawn, so it comes into view as they near
/// it and breaks from cover as they pass, as shipped open worlds stage their ambient
/// encounters ahead of the player (Red Dead Redemption 2's ambient animals, Far Cry's
/// wildlife spawned out of sight in the direction of travel).
const MEET_EVERY_S: (f32, f32) = (4.0, 9.0);
const MEET_AHEAD_M: (f32, f32) = (34.0, 52.0);
const MEET_ASIDE_M: (f32, f32) = (1.0, 14.0);
/// A kind met this near already is the less likely to be met again.
const MEET_KNOWN_M: f32 = 120.0;
/// Not where others of its kind already are this near.
const MEET_APART_M: f32 = 25.0;
/// A player is on the move going faster than this, metres a second.
const MOVING_MPS: f32 = 1.0;
/// How often what is about is told in the log, seconds.
const TELL_EVERY_S: f32 = 5.0;
/// The growing season cools by this much a metre up, where the land has no seasons of its own
/// worked out (the start's lapse rate, `climate.ron`).
const LAPSE_C_PER_M: f32 = 0.0065;

pub fn plugin(app: &mut App) {
    app.init_resource::<Cells>()
        .init_resource::<Warmth>()
        // Wherever the world says what grows: not on the yard, the flat test level, unless a
        // forest is grown on it.
        .add_systems(Update, populate.run_if(something_grows));
}

/// Whether the world grows anything: anywhere but the yard, the flat test level, unless a
/// forest is grown on it, which says what covers its ground.
fn something_grows(options: Res<crate::shell::Options>, cover: Option<Res<CoverRules>>) -> bool {
    !options.yard() || cover.is_some()
}

/// The cells looked at, and the animals each put out.
#[derive(Resource, Default)]
struct Cells {
    looked: HashMap<IVec3, Vec<Entity>>,
    since_s: f32,
    /// Each player's place when last looked and how long until they next meet something.
    ways: HashMap<Entity, (Vec3, f32)>,
    meetings: u64,
    /// The longest a look took since it was last told, milliseconds.
    slowest_ms: f32,
    told_s: f32,
}

/// What the world is like at a place, as an animal's niche asks it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Place {
    /// The growing season's mean temperature, °C.
    pub warmth_c: f32,
    /// How many trunks stand within `WOODED`'s reach, and whether any within `OPEN_M`.
    pub trunks: usize,
    pub open: bool,
    /// How deep water stands over the ground, in metres; below nothing, dry.
    pub depth_m: f32,
    /// Whether water is within `SHORE_M`.
    pub by_water: bool,
    /// How trodden the ground is by people.
    pub trodden: f32,
}

impl Place {
    /// Whether it is a kind of place an animal keeps to.
    pub fn is(&self, haunt: Haunt) -> bool {
        let dry = self.depth_m <= 0.0;
        match haunt {
            Haunt::Woods => dry && self.trunks >= WOODED.0,
            Haunt::Open => dry && !self.by_water && self.open && self.trodden < TRODDEN,
            Haunt::Shore => {
                (dry && self.by_water) || (self.depth_m > 0.0 && self.depth_m <= SHALLOWS_M)
            }
            Haunt::Water => self.depth_m >= SWIMMING_M,
            Haunt::Trodden => dry && self.trodden >= TRODDEN,
        }
    }

    pub fn fits(&self, niche: &Niche) -> bool {
        (niche.warmth_c.0..=niche.warmth_c.1).contains(&self.warmth_c)
            && niche.haunts.iter().any(|&haunt| self.is(haunt))
    }
}

/// The world round an animal, asked of its parts: the ground and its water, the trees, what
/// covers the ground, and the land's seasons.
#[derive(SystemParam)]
pub struct Surroundings<'w> {
    planet: Res<'w, Planet>,
    trunks: Res<'w, Trunks>,
    obstacles: Res<'w, Obstacles>,
    cover: Option<Res<'w, CoverRules>>,
    start: Res<'w, Start>,
    warmth: Res<'w, Warmth>,
}

/// The growing season's warmth as last worked out for each patch of ground: the land's
/// seasons are slow to ask (a fifth of a millisecond) and change little over a few tens of
/// metres, so a patch is asked once.
#[derive(Resource, Default)]
pub struct Warmth(std::sync::Mutex<HashMap<IVec3, f32>>);

/// A patch of ground whose growing season is taken as one, metres.
const WARMTH_PATCH_M: f32 = 48.0;

impl Surroundings<'_> {
    /// The ground at or under a point.
    pub fn ground(&self, point: Vec3) -> Vec3 {
        self.planet.ground_at(point)
    }

    /// How deep the water over the ground at a point is; below nothing where it is dry.
    pub fn depth_m(&self, point: Vec3) -> f32 {
        self.planet.water_radius_m(point) - self.planet.ground_at(point).length()
    }

    /// The standing trees within so far of a point.
    pub fn trunks(&self, at: Vec3, reach: f32) -> impl Iterator<Item = Trunk> + '_ {
        self.trunks.near(at, reach, &self.obstacles)
    }

    pub fn fits(&self, niche: &Niche, point: Vec3) -> bool {
        self.place(point).fits(niche)
    }

    /// What a point on the ground is like.
    pub fn place(&self, point: Vec3) -> Place {
        let ground = self.planet.ground_at(point);
        let up = ground.normalize();
        let depth_m = self.depth_m(ground);
        let side = up.any_orthonormal_vector();
        // Water near is the lakes' own map of where their water reaches, asked round the
        // point at its own height: a sample of the ground costs a tenth of a millisecond.
        let height = ground.length();
        let by_water = (0..6).any(|i| {
            let turn = Quat::from_axis_angle(up, i as f32 * std::f32::consts::TAU / 6.0);
            self.planet.water_near_m(ground + turn * side * SHORE_M) > height + 0.1
        });
        let trunks = self.trunks(ground, WOODED.1).count();
        let open = trunks == 0 && self.trunks(ground, OPEN_M).next().is_none();
        let local = self.start.local(ground);
        let trodden = self.cover.as_ref().map_or(0.0, |cover| {
            cover
                .source
                .growing(&self.planet, ground, Vec2::new(local.x, local.z))
                .trodden
        });
        Place {
            warmth_c: self.warmth_c(ground),
            trunks,
            open,
            depth_m,
            by_water,
            trodden,
        }
    }

    /// The growing season's mean at a point on the ground: the land's own seasons where they
    /// were worked out, else the start's climate at its latitude, cooling with height.
    fn warmth_c(&self, ground: Vec3) -> f32 {
        let patch = (ground / WARMTH_PATCH_M).floor().as_ivec3();
        if let Ok(known) = self.warmth.0.lock()
            && let Some(&warmth) = known.get(&patch)
        {
            return warmth;
        }
        let warmth = self.warmth_now(ground);
        if let Ok(mut known) = self.warmth.0.lock() {
            // Kept for the patches near players only, as cells are.
            if known.len() > 4096 {
                known.clear();
            }
            known.insert(patch, warmth);
        }
        warmth
    }

    fn warmth_now(&self, ground: Vec3) -> f32 {
        if let Some(round) = self.planet.round() {
            let place = round.land.place(ground, self.planet.normal(ground));
            return round
                .seasons
                .at(place.latitude, place.height_m, place.sunward)
                .growing_mean_c;
        }
        let over_sea = ground.length() - self.planet.sea_radius_m();
        start_warmth_c() - over_sea.max(0.0) * LAPSE_C_PER_M
    }
}

/// The growing season's mean at sea level at the start's latitude, by the world's climate.
fn start_warmth_c() -> f32 {
    use std::sync::OnceLock;
    static WARMTH: OnceLock<f32> = OnceLock::new();
    *WARMTH.get_or_init(|| {
        let climate: terra_climate::Climate =
            terra_world::data::read(terra_world::daylight::WorldClimate::FILE);
        let fields = terra_climate::Fields::new(climate, Seed(0));
        fields
            .season(terra_climate::Place::level(
                Vec3::ZERO,
                0.0,
                terra_world::daylight::LATITUDE,
            ))
            .growing_mean_c
    })
}

/// Puts out the animals of the cells near each player, and lets go of those nobody is near.
#[allow(clippy::too_many_arguments)]
fn populate(
    mut commands: Commands,
    time: Res<Time>,
    mut cells: ResMut<Cells>,
    kinds: Option<Res<Kinds>>,
    bestiary: Res<Bestiary>,
    around: Surroundings,
    viewers: Query<(Entity, &Transform), With<Viewpoint>>,
    placed: Query<(&Transform, &creatures::Creature)>,
) {
    let Some(kinds) = kinds.filter(|kinds| kinds.ready()) else {
        return;
    };
    cells.since_s += time.delta_secs();
    if cells.since_s < LOOK_EVERY_S {
        return;
    }
    cells.since_s = 0.0;
    let began = std::time::Instant::now();
    let moving: Vec<(Entity, Vec3)> = viewers
        .iter()
        .map(|(entity, at)| (entity, at.translation))
        .collect();
    let viewers: Vec<Vec3> = moving.iter().map(|(_, at)| *at).collect();
    let near = |middle: Vec3, within: f32| viewers.iter().any(|v| v.distance(middle) < within);
    // Let go of the cells nobody is near, and of their animals, alive or dead.
    let gone: Vec<IVec3> = cells
        .looked
        .keys()
        .filter(|&&cell| !near(middle_of(cell), RELEASE_M))
        .copied()
        .collect();
    for cell in gone {
        for animal in cells.looked.remove(&cell).unwrap_or_default() {
            commands.entity(animal).try_despawn();
        }
    }
    let wild: Vec<_> = bestiary
        .all()
        .filter_map(
            |species| match (bestiary.get(species).lives, &bestiary.get(species).niche) {
                (Lives::About { per_ha, group }, Some(niche)) => {
                    Some((species, per_ha, group, niche))
                }
                _ => None,
            },
        )
        .collect();
    if wild.is_empty() {
        return;
    }
    let mut about: usize = cells.looked.values().map(Vec::len).sum();
    let reach = (POPULATE_M / CELL_M).ceil() as i32;
    let seed = around.planet.seed().derive("wildlife");
    let mut fresh = 0;
    for viewer in &viewers {
        let home = cell_of(*viewer);
        for &offset in nearest_first(reach) {
            let cell = home + offset;
            if cells.looked.contains_key(&cell) || !near(middle_of(cell), POPULATE_M) {
                continue;
            }
            // So many new cells a look at most, so a player arriving does not wait
            // on all of them at once.
            if fresh >= FRESH_A_LOOK {
                continue;
            }
            fresh += 1;
            // Each patch of ground belongs to the cell its ground is in; a cell of
            // air or rock is remembered as empty, so it is not asked again.
            let ground = around.ground(middle_of(cell));
            if cell_of(ground) != cell {
                cells.looked.insert(cell, Vec::new());
                continue;
            }
            let mut put = Vec::new();
            let mut dice = seed.index(cell_key(cell)).rng();
            let up = ground.normalize();
            let (east, north) = (
                up.any_orthonormal_vector(),
                up.any_orthonormal_vector().cross(up),
            );
            for &(species, per_ha, (fewest, most), niche) in &wild {
                // A group lives here as often as its kind's numbers make groups on
                // ground of its niche, wherever in the cell it fits.
                let groups =
                    per_ha * (CELL_M * CELL_M / 10_000.0) * 2.0 / (fewest + most).max(1) as f32;
                if dice.unit() >= groups || about >= MOST {
                    continue;
                }
                let at = ground
                    + east * dice.range(-0.5, 0.5) * CELL_M
                    + north * dice.range(-0.5, 0.5) * CELL_M;
                let at = around.ground(at);
                if !around.fits(niche, at) {
                    continue;
                }
                let count = fewest + (dice.unit() * (most - fewest + 1) as f32) as u32;
                for _ in 0..count.min(most) {
                    let turn = Quat::from_axis_angle(up, dice.range(0.0, std::f32::consts::TAU));
                    let spot = around.ground(at + turn * east * dice.range(0.0, 2.5));
                    if let Some(animal) = creatures::spawn(
                        &mut commands,
                        (&kinds, &bestiary),
                        &around.planet,
                        species,
                        spot,
                    ) {
                        put.push(animal);
                        about += 1;
                    }
                }
                debug!(
                    "Wildlife: {} {} at {:.0} m east and {:.0} m south of the start.",
                    count,
                    bestiary.name(species),
                    around.start.local(at).x,
                    around.start.local(at).z
                );
            }
            cells.looked.insert(cell, put);
        }
    }
    meet(
        &mut commands,
        &mut cells,
        (&kinds, &bestiary),
        &around,
        &wild,
        &moving,
        &placed,
        &mut about,
    );
    cells.slowest_ms = cells.slowest_ms.max(began.elapsed().as_secs_f32() * 1000.0);
    cells.told_s += LOOK_EVERY_S;
    if cells.told_s >= TELL_EVERY_S {
        cells.told_s = 0.0;
        let animals: Vec<Vec3> = cells
            .looked
            .values()
            .flatten()
            .filter_map(|&animal| placed.get(animal).ok())
            .map(|(at, _)| at.translation)
            .collect();
        let near = animals
            .iter()
            .filter(|at| viewers.iter().any(|v| v.distance(**at) < 30.0))
            .count();
        info!(
            "Wildlife: {} about, {} within 30 m of a player, {} met ahead so far; a look took up to {:.2} ms.",
            animals.len(),
            near,
            cells.meetings,
            cells.slowest_ms
        );
        cells.slowest_ms = 0.0;
    }
}

/// A kind living about the land: its species, numbers to a hectare, group and niche.
type Wild<'a> = (Species, f32, (u32, u32), &'a Niche);

/// Puts a group out ahead of each player on the move, now and then: somewhere of its
/// niche beside their way, past where it is drawn, where none of its kind already is.
#[allow(clippy::too_many_arguments)]
fn meet(
    commands: &mut Commands,
    cells: &mut Cells,
    (kinds, bestiary): (&Kinds, &Bestiary),
    around: &Surroundings,
    wild: &[Wild],
    moving: &[(Entity, Vec3)],
    placed: &Query<(&Transform, &creatures::Creature)>,
    about: &mut usize,
) {
    let seed = around.planet.seed().derive("wildlife meetings");
    cells
        .ways
        .retain(|player, _| moving.iter().any(|(each, _)| each == player));
    for &(player, here) in moving {
        let (last, mut left) = cells.ways.get(&player).copied().unwrap_or((here, 1.0));
        let up = here.normalize_or(Vec3::Y);
        let went = (here - last).reject_from(up);
        if went.length() / LOOK_EVERY_S > MOVING_MPS {
            left -= LOOK_EVERY_S;
        }
        if left <= 0.0 && *about < MOST {
            cells.meetings += 1;
            let mut dice = seed.index(cells.meetings).rng();
            left = dice.range(MEET_EVERY_S.0, MEET_EVERY_S.1);
            let ahead = went.normalize_or_zero();
            let side = ahead.cross(up);
            // A spot each side of the way, each with the kinds whose niche it is and that
            // are not about there already; a kind already met near is less likely again, so
            // a walk by a wood and a lake meets both.
            let mut fits: Vec<(Vec3, Vec3, &Wild, f32)> = Vec::new();
            for sign in [1.0, -1.0] {
                let aside = side * sign;
                let spot = around.ground(
                    here + ahead * dice.range(MEET_AHEAD_M.0, MEET_AHEAD_M.1)
                        + aside * dice.range(MEET_ASIDE_M.0, MEET_ASIDE_M.1),
                );
                let place = around.place(spot);
                for kind in wild {
                    let (species, per_ha, _, niche) = kind;
                    if !place.fits(niche) {
                        continue;
                    }
                    let (mut close, mut seen) = (false, 0);
                    for (at, creature) in placed {
                        if creature.species == *species {
                            let apart = at.translation.distance(spot);
                            close |= apart < MEET_APART_M;
                            seen += (apart < MEET_KNOWN_M) as u32;
                        }
                    }
                    if !close {
                        fits.push((spot, aside, kind, per_ha / (1.0 + seen as f32)));
                    }
                }
            }
            let total: f32 = fits.iter().map(|(.., weight)| weight).sum();
            let mut roll = dice.unit() * total;
            let chosen = fits.iter().find(|(.., weight)| {
                roll -= weight;
                roll <= 0.0
            });
            if let Some(&(spot, aside, &(species, _, (fewest, most), _), _)) = chosen {
                let count = fewest + (dice.unit() * (most - fewest + 1) as f32) as u32;
                let cell = cell_of(spot);
                for _ in 0..count.min(most) {
                    let turn = Quat::from_axis_angle(up, dice.range(0.0, std::f32::consts::TAU));
                    let at = around.ground(spot + turn * aside * dice.range(0.0, 2.0));
                    if let Some(animal) =
                        creatures::spawn(commands, (kinds, bestiary), &around.planet, species, at)
                    {
                        cells.looked.entry(cell).or_default().push(animal);
                        *about += 1;
                    }
                }
                debug!("Wildlife: {} {} met ahead.", count, bestiary.name(species));
            }
        }
        cells.ways.insert(player, (here, left));
    }
}

/// The cells round a cell within so many of it, nearest first, so the nearest are put out
/// first when a look leaves some for the next.
fn nearest_first(reach: i32) -> &'static [IVec3] {
    use std::sync::OnceLock;
    static OFFSETS: OnceLock<(i32, Vec<IVec3>)> = OnceLock::new();
    let (made_for, offsets) = OFFSETS.get_or_init(|| {
        let mut offsets: Vec<IVec3> = (-reach..=reach)
            .flat_map(|x| {
                (-reach..=reach)
                    .flat_map(move |y| (-reach..=reach).map(move |z| IVec3::new(x, y, z)))
            })
            .collect();
        offsets.sort_by_key(|offset| offset.length_squared());
        (reach, offsets)
    });
    debug_assert_eq!(*made_for, reach);
    offsets
}

fn cell_of(point: Vec3) -> IVec3 {
    (point / CELL_M).floor().as_ivec3()
}

fn middle_of(cell: IVec3) -> Vec3 {
    (cell.as_vec3() + 0.5) * CELL_M
}

fn cell_key(cell: IVec3) -> u64 {
    let part = |v: i32| (v as u32 as u64) & 0x1F_FFFF;
    part(cell.x) | part(cell.y) << 21 | part(cell.z) << 42
}

#[cfg(test)]
mod tests {
    use super::*;

    fn place() -> Place {
        Place {
            warmth_c: 12.0,
            trunks: 0,
            open: true,
            depth_m: -1.0,
            by_water: false,
            trodden: 0.0,
        }
    }

    #[test]
    fn each_kind_of_place_is_told_apart_by_what_is_there() {
        let open = place();
        assert!(open.is(Haunt::Open) && !open.is(Haunt::Woods) && !open.is(Haunt::Water));
        let woods = Place {
            trunks: 5,
            open: false,
            ..place()
        };
        assert!(woods.is(Haunt::Woods) && !woods.is(Haunt::Open));
        let shore = Place {
            by_water: true,
            ..place()
        };
        assert!(shore.is(Haunt::Shore) && !shore.is(Haunt::Open));
        let shallows = Place {
            depth_m: 0.2,
            by_water: true,
            ..place()
        };
        assert!(shallows.is(Haunt::Shore) && !shallows.is(Haunt::Water));
        let lake = Place {
            depth_m: 2.0,
            by_water: true,
            ..place()
        };
        assert!(lake.is(Haunt::Water) && !lake.is(Haunt::Shore) && !lake.is(Haunt::Woods));
        let street = Place {
            trodden: 0.9,
            ..place()
        };
        assert!(street.is(Haunt::Trodden) && !street.is(Haunt::Open));
    }

    #[test]
    fn a_niche_takes_its_warmth_and_one_of_its_places() {
        let niche = Niche {
            warmth_c: (-5.0, 7.0),
            haunts: vec![Haunt::Open],
        };
        assert!(
            Place {
                warmth_c: 5.0,
                ..place()
            }
            .fits(&niche)
        );
        assert!(!place().fits(&niche), "too warm");
        assert!(
            !Place {
                warmth_c: 5.0,
                trunks: 4,
                open: false,
                ..place()
            }
            .fits(&niche)
        );
    }

    #[test]
    fn every_patch_of_ground_is_in_one_cell() {
        let point = Vec3::new(1000.3, -20.0, 5.5);
        let cell = cell_of(point);
        let middle = middle_of(cell);
        assert!((middle - point).abs().max_element() <= CELL_M * 0.5);
        assert_eq!(cell_of(middle), cell);
        assert_ne!(cell_key(cell), cell_key(cell + IVec3::X));
    }
}
