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

mod groups;
mod meet;
use meet::{Band, Way, eye_at, meet, shows_from};

use super::creatures::{self, Kinds};
use super::lod::Viewpoint;
use super::species::{Bestiary, Haunt, Lives, Niche, Species};
use crate::planet::Planet;
use crate::start::Start;
use bevy::{ecs::system::SystemParam, platform::collections::HashMap, prelude::*};
use groups::{BIG_CROSSINGS, CROSSINGS, cross, put_group};
#[cfg(test)]
use groups::{CROSS_LEAD_M, crossing_point};
use terra_gen::Seed;
use terra_world::cover::CoverRules;
use terra_world::daylight::{Clock, SkyPath};
use terra_world::flora::{Obstacles, Trunk, Trunks};
use terra_world::parting::ObstructionFade;
use terra_world::sky_occlusion::SkyOccluders;
use terra_world::spatial::SpatialHash;

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
/// A meeting is staged where it will be seen, as those games stage theirs: a few spots are
/// tried beside the way, never on it, where the player's own body hides it from the camera
/// behind them, but out to one side, where it passes through the open third of the picture,
/// 15° to 30° off the way the camera looks; and the one chosen is in view all the while it is
/// there, nothing standing between, in the open and the light, on bare or short ground
/// rather than in a thicket: a hare on the path's edge, finches on the path, ducks off the
/// shore. What then happens is the animals' own: each breaks from a player on the move
/// across their way and on ahead (creatures' `bolt`), so it crosses the picture near rather
/// than vanishing into cover.
const MEET_EVERY_S: (f32, f32) = (3.0, 6.0);
/// A player setting off meets something within so long, so a walk does not begin empty.
const SET_OFF_S: f32 = 0.5;
/// Every so many meetings on the move, the first among them, is with a big animal (a body
/// this wide or more) if one lives there: a deer or a boar is what a walk in a wood is
/// remembered by, as a shipped game's ambient encounters put the big game first (Red Dead
/// Redemption 2's deer on the trail).
const BIG_EVERY: u32 = 3;
const BIG_M: f32 = 0.3;
/// How far ahead and aside a meeting's spot is, metres: a big animal's further, a small one's
/// nearer, so each is met at a size it can be made out at (`Band`).
const MEET_BIG: Band = Band {
    ahead_m: (30.0, 40.0),
    aside_m: (6.0, 9.0),
};
const MEET_SMALL: Band = Band {
    ahead_m: (16.0, 24.0),
    aside_m: (4.0, 6.0),
};
/// The angles off the camera's way through which a spot beside it is seen in view, as the
/// player comes on: a little off their line, then well off it (`eyes_on`).
const SEEN_OFF: [f32; 2] = [0.31, 0.49];
/// How far either side of its middle an animal met is looked for, metres.
const BODY_SPAN_M: f32 = 0.5;
/// Looks a meeting's spots are tried over, each side of the way, and spots tried a look: in
/// a thick wood most are hidden.
const MEET_TRIES: usize = 3;
const SPOTS_A_LOOK: usize = 3;
/// The camera follows so far behind and above the player, metres (play's camera): where a
/// meeting's spot must be seen from.
const EYE_BEHIND_M: f32 = 4.6;
const EYE_UP_M: f32 = 2.2;
/// So many met animals at most besides the land's own, for every player together, so the
/// cells round a player, full, do not leave nothing to meet ahead; one so far from every
/// player is let go.
const MET_MOST: usize = 24;
const MET_RELEASE_M: f32 = 80.0;
/// Standing, a player meets no more than so many.
const MET_STANDING: usize = 8;
/// Grass thinner than this, as thick by how much taller it grows, leaves an animal on it in
/// plain sight.
const BARE_GRASS: f32 = 0.35;
/// A line through a crown within this share of its size, as thick as its leaves close, is
/// hidden by them.
const LEAVES_HIDE: f32 = 0.85;
/// Grass this tall hides a small animal in it.
const HIDING_GRASS_M: f32 = 0.45;
/// A spot in the sun shows what is on it so many times better than one in shade.
const SUNLIT: f32 = 2.5;
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
        .init_resource::<Perches>()
        .init_resource::<Thickets>()
        .add_systems(Update, file_thickets)
        .init_resource::<Warmth>()
        // Wherever the world says what grows: not on the yard, the flat test level, unless a
        // forest is grown on it.
        .add_systems(Update, populate.run_if(something_grows).run_if(wild_shown));
}

/// Whether wild animals are put out: not in a time-lapse without people (D381).
fn wild_shown(rising: Option<Res<super::rising::Rising>>) -> bool {
    rising.is_none_or(|r| r.wildlife())
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
    ways: HashMap<Entity, Way>,
    meetings: u64,
    /// The animals met ahead and still about, apart from the cells' own.
    met: Vec<Entity>,
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
    /// How much canopy is over it, from open sky (0) to none seen (1), and whether its ground
    /// is bare or short: a path, a street, grass grown thin.
    pub shade: f32,
    pub bare: bool,
    /// How tall the grass on it stands, most of its blades (`grass_height`).
    pub grass_m: f32,
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
pub struct Surroundings<'w, 's> {
    planet: Res<'w, Planet>,
    trunks: Res<'w, Trunks>,
    obstacles: Res<'w, Obstacles>,
    cover: Option<Res<'w, CoverRules>>,
    start: Res<'w, Start>,
    warmth: Res<'w, Warmth>,
    /// The crowns of the trees, where a forest says them: leaves hide what is behind them.
    crowns: Option<Res<'w, SkyOccluders>>,
    perches: Res<'w, Perches>,
    /// The bushes and young trees, filed by place: their leaves hide what is behind them.
    thickets: Res<'w, Thickets>,
    plants: Query<'w, 's, &'static ObstructionFade>,
    /// Where the sun is, for what stands in its light.
    sky: Option<Res<'w, SkyPath>>,
    clock: Res<'w, Clock>,
}

/// The bushes and young trees whose leaves hide what is behind them (those the camera fades,
/// `ObstructionFade`), filed by place once their meshes are in.
#[derive(Resource)]
pub struct Thickets(SpatialHash<Entity>);

impl Default for Thickets {
    fn default() -> Self {
        Self(SpatialHash::new(16.0))
    }
}

/// A thicket's leaves reach at most so far from where it is filed.
const THICKET_REACH_M: f32 = 6.0;
/// How far toward the sun a point is looked from for what shades it.
const SUN_LOOKS_M: f32 = 60.0;

/// Filed already among the thickets.
#[derive(Component)]
struct Filed;

/// Files each new bush and young tree once its box is known.
fn file_thickets(
    mut commands: Commands,
    mut thickets: ResMut<Thickets>,
    plants: Query<(Entity, &ObstructionFade), Without<Filed>>,
) {
    for (plant, fade) in &plants {
        if let Some((centre, reach)) = fade.round() {
            if reach <= THICKET_REACH_M * 1.5 {
                thickets.0.insert(centre, plant);
            }
            commands.entity(plant).insert(Filed);
        }
    }
}

/// Where birds sit up off the ground besides the trees: the ridges of a settlement's roofs,
/// each as its two ends, which the layer that puts the buildings up fills in (the town's).
/// Pigeons and sparrows line a ridge, as they do on every town's roofs.
#[derive(Resource, Default)]
pub struct Perches(pub Vec<(Vec3, Vec3)>);

impl Surroundings<'_, '_> {
    /// A spot on a ridge roughly the way given, between so near and so far across the ground:
    /// the one most that way, somewhere along it.
    pub fn perch(
        &self,
        here: Vec3,
        way: Vec3,
        (near, far): (f32, f32),
        dice: &mut terra_gen::Rng,
    ) -> Option<Vec3> {
        let up = here.normalize_or(Vec3::Y);
        let mut best: Option<(Vec3, f32)> = None;
        for &(a, b) in &self.perches.0 {
            let point = a.lerp(b, dice.range(0.1, 0.9));
            let off = (point - here).reject_from(up);
            let apart = off.length();
            let lean = off.normalize_or_zero().dot(way);
            if (near..=far).contains(&apart) && lean > -0.2 && best.is_none_or(|(_, l)| lean > l) {
                best = Some((point, lean));
            }
        }
        best.map(|(point, _)| point)
    }

    /// Whether ridges were put up since this was last asked: the cells looked at before had
    /// none to put their birds on.
    pub fn new_ridges(&self) -> bool {
        self.perches.is_changed() && !self.perches.0.is_empty()
    }

    /// The ridge nearest a point within so far across the ground, if any.
    pub fn ridge_near(&self, at: Vec3, within: f32) -> Option<(Vec3, Vec3)> {
        let up = at.normalize_or(Vec3::Y);
        self.perches
            .0
            .iter()
            .map(|&(a, b)| {
                let s = ((at - a).dot(b - a) / (b - a).length_squared().max(1e-4)).clamp(0.0, 1.0);
                ((a, b), (a.lerp(b, s) - at).reject_from(up).length())
            })
            .filter(|(_, apart)| *apart <= within)
            .min_by(|x, y| x.1.total_cmp(&y.1))
            .map(|(ridge, _)| ridge)
    }
}

/// The growing season's warmth as last worked out for each patch of ground: the land's
/// seasons are slow to ask (a fifth of a millisecond) and change little over a few tens of
/// metres, so a patch is asked once.
#[derive(Resource, Default)]
pub struct Warmth(std::sync::Mutex<HashMap<IVec3, f32>>);

/// A patch of ground whose growing season is taken as one, metres.
const WARMTH_PATCH_M: f32 = 48.0;

impl Surroundings<'_, '_> {
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
        let growing = self.cover.as_ref().map(|cover| {
            cover
                .source
                .growing(&self.planet, ground, Vec2::new(local.x, local.z))
        });
        let trodden = growing.as_ref().map_or(0.0, |g| g.trodden);
        let shade = growing.as_ref().map_or(0.0, |g| g.shade);
        let bare = growing.as_ref().is_none_or(|g| {
            g.path || g.trodden >= TRODDEN || g.grass * (1.0 + g.tall) < BARE_GRASS
        });
        Place {
            warmth_c: self.warmth_c(ground),
            trunks,
            open,
            depth_m,
            by_water,
            trodden,
            shade,
            bare,
            grass_m: growing.as_ref().map_or(0.0, grass_height),
        }
    }

    /// Whether nothing is between an eye and a point: no trunk, rock or log, no tree's crown
    /// (a young tree's beside a path is a thicket of leaves to the ground), and not the
    /// ground.
    pub fn in_view(&self, eye: Vec3, at: Vec3) -> bool {
        let line = at - eye;
        let length = line.length();
        if length < 1.0 {
            return true;
        }
        let way = line / length;
        if self.obstacles.sweep(eye, way, length - 0.5, 0.15).is_some() {
            return false;
        }
        let middle = eye + line * 0.5;
        let in_a_thicket = self
            .thickets
            .0
            .near(middle, length * 0.5 + THICKET_REACH_M)
            .filter_map(|(_, plant)| self.plants.get(*plant).ok())
            .any(|plant| plant.hides(eye, at));
        if in_a_thicket || self.through_crowns(eye, at) {
            return false;
        }
        // A sample of the ground costs a tenth of a millisecond: a few, over its middle.
        const GROUND_SAMPLES: usize = 8;
        !(1..GROUND_SAMPLES).any(|i| {
            let point = eye + line * (i as f32 / GROUND_SAMPLES as f32);
            self.planet.ground_at(point).length() > point.length() + 0.05
        })
    }

    /// Whether a point stands in the sun: nothing standing and no crown between it and the
    /// sun, which is up. Where nothing says where the sun is, it does.
    pub fn sunlit(&self, at: Vec3) -> bool {
        let Some(sky) = &self.sky else {
            return true;
        };
        let sun = sky.sun(self.clock.hour);
        if sun.dot(at.normalize_or(Vec3::Y)) < 0.02 {
            return false;
        }
        let far = at + sun * SUN_LOOKS_M;
        self.obstacles.sweep(at, sun, SUN_LOOKS_M, 0.05).is_none() && !self.through_crowns(at, far)
    }

    /// Whether the line between two points passes through the thick of a tree's crown.
    fn through_crowns(&self, eye: Vec3, at: Vec3) -> bool {
        let line = at - eye;
        let length = line.length();
        let way = line / length.max(1e-3);
        if let Some(wood) = &self.crowns {
            let south = wood.east.cross(wood.up);
            return wood.crowns.iter().any(|crown| {
                // Into the crown's own frame, where it is a ball of one.
                let into = |p: Vec3| {
                    let off = p - crown.middle;
                    Vec3::new(
                        off.dot(wood.east) / crown.half.x.max(0.1),
                        off.dot(wood.up) / crown.half.y.max(0.1),
                        off.dot(south) / crown.half.z.max(0.1),
                    )
                };
                let reach = crown.half.max_element();
                let nearest = (crown.middle - eye).dot(way).clamp(0.0, length);
                if (eye + way * nearest).distance(crown.middle) > reach {
                    return false;
                }
                let (from, to) = (into(eye), into(at));
                let along = to - from;
                let s = (-from.dot(along) / along.length_squared().max(1e-6)).clamp(0.0, 1.0);
                // Through its thick, not grazing the gaps at its edge.
                (from + along * s).length() < LEAVES_HIDE * crown.closes.sqrt()
            });
        }
        false
    }

    /// How tall the grass stands at a point on the ground, as most of its blades do (the
    /// cover's own measure, `cover::blades`); none where nothing covers the ground.
    pub fn grass_m(&self, ground: Vec3) -> f32 {
        let local = self.start.local(ground);
        self.cover.as_ref().map_or(0.0, |cover| {
            grass_height(
                &cover
                    .source
                    .growing(&self.planet, ground, Vec2::new(local.x, local.z)),
            )
        })
    }

    /// The growing season's mean at a point on the ground: the land's own seasons where they
    /// were worked out, else the start's climate at its latitude, cooling with height.
    pub fn warmth_c(&self, ground: Vec3) -> f32 {
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

/// How tall grass grown so stands, as most of its blades do (`cover::blades`' measure).
fn grass_height(g: &terra_world::cover::Growing) -> f32 {
    if g.path || g.grass <= 0.05 {
        return 0.0;
    }
    (0.12 + 0.42 * libm::powf(g.grass, 0.7) * (0.6 + 0.4 * g.lush)) * (1.0 + 0.5 * g.tall) * 0.85
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
    // A town put up after its cells were looked at: looked at again, so its birds sit on its
    // roofs too. Asked every frame, so the change is not missed.
    if around.new_ridges() {
        for animal in cells.looked.drain().flat_map(|(_, put)| put) {
            commands.entity(animal).try_despawn();
        }
        info!("Wildlife: {} ridges to sit on.", around.perches.0.len());
    }
    let Some(kinds) = kinds.filter(|kinds| kinds.ready()) else {
        return;
    };
    cells.since_s += time.delta_secs();
    if cells.since_s < LOOK_EVERY_S {
        return;
    }
    cells.since_s = 0.0;
    let began = std::time::Instant::now();
    let moving: Vec<(Entity, Vec3, Vec3)> = viewers
        .iter()
        .map(|(entity, at)| (entity, at.translation, at.forward().as_vec3()))
        .collect();
    let viewers: Vec<Vec3> = moving.iter().map(|(_, at, _)| *at).collect();
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
                let group = put_group(
                    &mut commands,
                    (&kinds, &bestiary),
                    &around,
                    species,
                    (at, east * 2.5),
                    count.min(most),
                    &mut dice,
                );
                about += group.len();
                put.extend(group);
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
    // The met let go once nobody is near them, or once gone.
    let mut met = std::mem::take(&mut cells.met);
    met.retain(|&animal| {
        let kept = placed
            .get(animal)
            .is_ok_and(|(at, _)| near(at.translation, MET_RELEASE_M));
        if !kept {
            commands.entity(animal).try_despawn();
        }
        kept
    });
    cells.met = met;
    meet(
        &mut commands,
        &mut cells,
        (&kinds, &bestiary),
        &around,
        &wild,
        &moving,
        &placed,
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
            shade: 0.0,
            bare: true,
            grass_m: 0.0,
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
    fn a_crossing_meets_the_way_a_little_ahead_of_the_player_as_they_come_on() {
        let here = Vec3::ZERO;
        let ahead = Vec3::NEG_Z;
        let start = Vec3::new(20.0, 0.0, -50.0);
        for (mps, speed) in [(5.5, 4.3), (4.2, 4.3), (12.0, 4.3), (5.0, 0.0)] {
            let (point, takes) = crossing_point(here, ahead, start, mps, speed).unwrap();
            // It gets there in the time it says, going as fast as it goes.
            assert!((point.distance(start) / mps - takes).abs() < 0.05, "{mps}");
            // And the player, coming on meanwhile, is that far short of it.
            let player = here + ahead * speed * takes;
            let short = (point - player).dot(ahead);
            assert!((short - CROSS_LEAD_M).abs() < 0.5, "{mps} {speed}: {short}");
        }
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
