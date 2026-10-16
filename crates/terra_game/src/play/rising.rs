//! A settlement rising, as a time-lapse (D381): `--timelapse <file>[@frame]` with the town
//! experiment lays the town's plan of streets and plots on the ground and puts nothing up;
//! from the frame it rolls on, its buildings go up one after another from the square out,
//! each from its plan to its last trim by the building system's own steps in the order the
//! hammer takes them (`terra_build::construct`), its builders hammering at its sites while it
//! rises (`terra_life`'s `Build`), and the clock runs through the day. As each is finished its
//! hearth smokes, its household comes out of its door, and its yard, fences and torch are
//! there; the square's well, benches and stalls come as the settlement reaches them.
//!
//! What happens when is data, `assets/data/timelapse/*.ron` ([`Timelapse`]), so one tool
//! makes any settlement's time-lapse, a bench of building as it happens or a film; the camera
//! is a shot's (`--cine`), as any film's is.

use super::catalogue::Builds;
use super::daylight::{Clock, Ticking};
use super::flora::ClearAround;
use super::props::{self, Models, Theirs};
use super::town::{DoorTorch, Firelight, ForgeFire, Hearth, Market, Wear};
use super::town_fences::TownFence;
use crate::build::{Built, PlanGhost, Site};
use crate::planet::Planet;
use crate::shell::{Options, ScriptFrame};
use bevy::prelude::*;
use serde::Deserialize;
use std::path::Path;
use terra_build::Base;
use terra_build::construct;
use terra_build::fitting::Hang;
use terra_build::ops::{Context, Op};
use terra_build::town::ThingKind;

/// What a time-lapse does when, as its file says.
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct Timelapse {
    #[serde(default)]
    pub about: String,
    /// How long it runs once it rolls, in seconds.
    pub seconds: f32,
    /// The clock's hour as it rolls and as it ends, moved on evenly between.
    pub hours: (f32, f32),
    /// When the claim stone is set at the square's middle, in seconds after it rolls.
    #[serde(default)]
    pub claim: Option<f32>,
    /// When the first building's plan is laid, and the last's: the rest between, the
    /// nearest the square first.
    pub first: f32,
    pub last: f32,
    /// How long a building takes to go up, the smallest to the largest, in seconds.
    pub takes: (f32, f32),
    /// How many of the town's buildings rise, the nearest the square first; all if none.
    #[serde(default)]
    pub buildings: Option<usize>,
    /// How long after a building is finished its yard, fences, torch and smoke are there.
    #[serde(default)]
    pub settled_after: f32,
    /// When the things of no one building (the well, the benches, the stalls) come, from
    /// the nearest the square's middle to the furthest.
    pub common: (f32, f32),
    /// At most so many building steps a frame, across the whole settlement.
    #[serde(default = "Timelapse::most_steps")]
    pub steps_a_frame: u32,
}

impl Timelapse {
    fn most_steps() -> u32 {
        400
    }

    /// Reads a time-lapse file, `name@frame` rolling on that frame.
    pub fn read(arg: &str) -> Result<(Timelapse, u32), String> {
        let (file, rolls) = match arg.rsplit_once('@') {
            Some((file, frame)) if frame.chars().all(|c| c.is_ascii_digit()) => (
                file,
                frame.parse().map_err(|_| format!("--timelapse: {frame}"))?,
            ),
            _ => (arg, 0),
        };
        let path = if Path::new(file).exists() {
            file.to_string()
        } else {
            terra_world::data::path(&format!("timelapse/{file}"))
                .to_string_lossy()
                .to_string()
        };
        let text = std::fs::read_to_string(&path).map_err(|why| format!("{path}: {why}"))?;
        Self::parse(&text).map(|t| (t, rolls))
    }

    pub fn parse(text: &str) -> Result<Timelapse, String> {
        let lapse: Timelapse = ron::from_str(text).map_err(|why| why.to_string())?;
        if lapse.seconds <= 0.0
            || lapse.last < lapse.first
            || lapse.takes.0 <= 0.0
            || lapse.takes.1 < lapse.takes.0
            || lapse.common.1 < lapse.common.0
        {
            return Err("a time-lapse's times are out of order".into());
        }
        Ok(lapse)
    }

    /// The hour at so many seconds in.
    pub fn hour(&self, t: f32) -> f32 {
        let share = (t / self.seconds).clamp(0.0, 1.0);
        self.hours.0 + (self.hours.1 - self.hours.0) * share
    }
}

/// When each building starts and how long it takes, from the nearest the square to the
/// furthest: the first `count`, by their doors' distances and how big each is in cubes.
pub fn schedule(lapse: &Timelapse, distances: &[f32], sizes: &[usize]) -> Vec<Option<(f32, f32)>> {
    let mut order: Vec<usize> = (0..distances.len()).collect();
    order.sort_by(|&a, &b| distances[a].total_cmp(&distances[b]));
    let count = lapse.buildings.unwrap_or(order.len()).min(order.len());
    let (small, large) = sizes
        .iter()
        .fold((usize::MAX, 0), |(lo, hi), &s| (lo.min(s), hi.max(s)));
    let mut out = vec![None; distances.len()];
    for (rank, &index) in order.iter().take(count).enumerate() {
        let share = if count > 1 {
            rank as f32 / (count - 1) as f32
        } else {
            0.0
        };
        let start = lapse.first + (lapse.last - lapse.first) * share;
        let size = if large > small {
            (sizes[index] - small) as f32 / (large - small) as f32
        } else {
            0.0
        };
        let takes = lapse.takes.0 + (lapse.takes.1 - lapse.takes.0) * size.sqrt();
        out[index] = Some((start, takes));
    }
    out
}

/// How far along a building should be at a share of its time: slow to start as its
/// footings go in, steady, and slow at the end with its trims, as a time-lapse of work is.
fn eased(share: f32) -> f32 {
    let s = share.clamp(0.0, 1.0);
    s * s * (3.0 - 2.0 * s)
}

/// A time-lapse under way.
#[derive(Resource)]
pub struct Rising {
    pub lapse: Timelapse,
    /// The frame it rolls on, counted as `--press` counts them.
    pub rolls_at: u32,
    /// Seconds since it rolled.
    pub t: f32,
    /// Each building's start and time, once the town is laid; and whether it is up.
    pub times: Vec<Option<(f32, f32)>>,
    pub up: Vec<bool>,
    /// When each building was finished, in seconds.
    pub finished: Vec<Option<f32>>,
    /// Whether the town's things have been taken in hand.
    prepared: bool,
    claimed: bool,
    /// How far from the square's middle the furthest thing of the town is.
    reach: f32,
    /// When the ground was last worn again, and how many plots were laid then.
    worn: (f32, usize),
}

impl Rising {
    pub fn new(lapse: Timelapse, rolls_at: u32) -> Self {
        Self {
            lapse,
            rolls_at,
            t: 0.0,
            times: Vec::new(),
            up: Vec::new(),
            finished: Vec::new(),
            prepared: false,
            claimed: false,
            reach: 1.0,
            worn: (0.0, 0),
        }
    }

    /// Whether a building stands finished, so its household lives in it.
    pub fn is_up(&self, building: Option<usize>) -> bool {
        building.is_none_or(|b| self.up.get(b).copied().unwrap_or(true))
    }

    /// Whether one of the town's people is about: a builder always, anyone else once their
    /// home is up.
    pub fn about(&self, market: &Market, person: usize) -> bool {
        let life = &market.life;
        life.role(person) == "builder"
            || self.is_up(life.town.things[life.people[person].home].building)
    }

    /// Whether a building is up and lived in long enough for its yard, smoke and torch.
    pub fn settled(&self, building: usize) -> bool {
        self.finished
            .get(building)
            .copied()
            .flatten()
            .is_some_and(|done| self.t >= done + self.lapse.settled_after)
    }

    /// Whether the settlement has reached a place in the town: its building settled, or for
    /// a place of no building, its time come, from the square out.
    pub fn reached(&self, town: &terra_build::town::Town, at: Vec2) -> bool {
        match owner(town, at) {
            Some(building) => self.settled(building),
            None => self.t >= self.common_at(at),
        }
    }

    fn common_at(&self, at: Vec2) -> f32 {
        let (from, to) = self.lapse.common;
        from + (to - from) * (at.length() / self.reach.max(1.0)).min(1.0)
    }
}

/// Whose a place in the town is: the building whose ground or yard it is on or within a
/// couple of metres of, if any.
pub fn owner(town: &terra_build::town::Town, at: Vec2) -> Option<usize> {
    let near = |quad: &[Vec2; 4]| {
        if terra_build::town::inside(at, quad) {
            return 0.0;
        }
        (0..4)
            .map(|i| terra_build::town::point_segment(at, quad[i], quad[(i + 1) % 4]))
            .fold(f32::MAX, f32::min)
    };
    let mut best = (2.5, None);
    for (index, quads) in town.footprints.iter().enumerate() {
        for quad in quads {
            let d = near(quad);
            if d < best.0 {
                best = (d, Some(index));
            }
        }
    }
    for (index, quad) in &town.yards {
        let d = near(quad);
        if d < best.0 {
            best = (d, Some(*index));
        }
    }
    best.1
}

/// A building that goes up: which of the town's, what it will be, and where its builders
/// are at work, in its own metres.
#[derive(Component)]
pub struct Rises {
    pub index: usize,
    pub whole: Base,
    pub planned: bool,
    pub near: Vec3,
}

/// Something of the town kept out of sight until its building is up, or for one of no
/// building, until the settlement reaches it.
#[derive(Component)]
struct Waits {
    building: Option<usize>,
    at: f32,
}

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, read).add_systems(
        Update,
        (
            hold_the_hour.before(Ticking),
            (prepare, build, reveal, hide_ghosts, wear)
                .chain()
                .after(super::town::Laid)
                .run_if(resource_exists::<Market>),
        )
            .run_if(resource_exists::<Rising>),
    );
}

fn read(mut commands: Commands, options: Res<Options>) {
    let Some(arg) = options.timelapse.as_deref() else {
        return;
    };
    match Timelapse::read(arg) {
        Ok((lapse, rolls_at)) => {
            info!(
                "Time-lapse: {}: {} s, {:.1}h to {:.1}h, rolling on frame {rolls_at}.",
                lapse.about, lapse.seconds, lapse.hours.0, lapse.hours.1
            );
            commands.insert_resource(Rising::new(lapse, rolls_at));
        }
        Err(why) => error!("Time-lapse: {why}"),
    }
}

/// The clock at the time-lapse's hour, moved on as it runs.
fn hold_the_hour(
    mut rising: ResMut<Rising>,
    frame: Res<ScriptFrame>,
    time: Res<Time>,
    market: Option<Res<Market>>,
    mut clock: ResMut<Clock>,
) {
    let rolled = rising.rolls_at == 0 || frame.0 >= rising.rolls_at;
    if market.is_some() && rolled {
        rising.t += time.delta_secs();
    }
    clock.hour = rising.lapse.hour(rising.t);
}

/// Once the town is laid: each building's time, its things out of sight, its sites shut.
#[allow(clippy::too_many_arguments)]
fn prepare(
    mut commands: Commands,
    mut rising: ResMut<Rising>,
    mut market: ResMut<Market>,
    clock: Res<Clock>,
    rises: Query<&Rises>,
    waiting: Query<
        (Entity, &GlobalTransform),
        Or<(
            With<Theirs>,
            With<TownFence>,
            With<Hearth>,
            With<Firelight>,
            With<ForgeFire>,
        )>,
    >,
    torches: Query<&ChildOf, With<DoorTorch>>,
    roots: Query<&GlobalTransform>,
    mut seen: Local<u32>,
) {
    if rising.prepared || rises.is_empty() {
        return;
    }
    // A couple of frames on from the town being laid, so where its things stand has been
    // worked out.
    *seen += 1;
    if *seen < 3 {
        return;
    }
    rising.prepared = true;
    let town = &market.life.town;
    let count = town.buildings.len();
    let mut distances = vec![f32::MAX; count];
    let mut sizes = vec![0; count];
    for rise in &rises {
        distances[rise.index] = Vec2::from(town.buildings[rise.index].at).length();
        sizes[rise.index] = rise.whole.cubes().count();
    }
    let times = schedule(&rising.lapse, &distances, &sizes);
    rising.up = times.iter().map(Option::is_none).collect();
    rising.finished = vec![None; count];
    rising.times = times;
    rising.reach = town
        .things
        .iter()
        .map(|t| t.at.length())
        .fold(1.0f32, f32::max);
    let wait = |at: Vec2| Waits {
        building: owner(town, at),
        at: rising.common_at(at),
    };
    let mut hidden = 0;
    for (entity, seen) in &waiting {
        let at = market.local(seen.translation());
        commands
            .entity(entity)
            .insert((wait(at), Visibility::Hidden));
        hidden += 1;
    }
    for parent in &torches {
        if let Ok(seen) = roots.get(parent.parent()) {
            let at = market.local(seen.translation());
            commands
                .entity(parent.parent())
                .insert((wait(at), Visibility::Hidden));
        }
    }
    // Nobody builds where nothing is going up yet.
    let hours = Market::hours(&clock);
    let sites: Vec<usize> = (0..market.life.town.things.len())
        .filter(|&i| market.life.town.things[i].kind == ThingKind::Site)
        .collect();
    for &site in &sites {
        market.life.set_open(site, false, hours);
    }
    let up = rising.times.iter().filter(|t| t.is_some()).count();
    info!(
        "Time-lapse: {up} of {count} buildings to go up, {} sites, {hidden} things kept back.",
        sites.len()
    );
}

/// Each building on from where it was to where its time says, a step at a time in the
/// hammer's order; its plan laid and its plot cleared as it starts, its sites open while it
/// goes up.
#[allow(clippy::too_many_arguments)]
fn build(
    mut commands: Commands,
    mut rising: ResMut<Rising>,
    mut market: ResMut<Market>,
    clock: Res<Clock>,
    planet: Res<Planet>,
    mut builds: Query<(&mut Built, &mut Rises)>,
    mut clear: MessageWriter<ClearAround>,
    site: Option<Res<Site>>,
    models: Res<Models>,
    catalogue: Res<Builds>,
) {
    if !rising.prepared {
        return;
    }
    let t = rising.t;
    let hours = Market::hours(&clock);
    // The claim stone, at the heart of it.
    if !rising.claimed
        && let Some(when) = rising.lapse.claim
        && t >= when
        && let Some(site) = site.as_deref()
    {
        rising.claimed = true;
        let middle = market.middle;
        let to_site = site.frame.compute_affine().inverse();
        if let Some(item) = catalogue.item_from("ClaimStone") {
            let point = to_site.transform_point3(middle + market.east * 1.5);
            props::place(
                &mut commands,
                &models,
                site,
                &planet,
                &item,
                point.with_y(0.0),
                0.0,
                30.0,
                false,
            );
        }
    }
    let mut budget = rising.lapse.steps_a_frame;
    for (mut built, mut rise) in &mut builds {
        let index = rise.index;
        let Some((start, takes)) = rising.times.get(index).copied().flatten() else {
            continue;
        };
        if t < start || rising.up[index] {
            continue;
        }
        let rules = built
            .kits
            .as_deref()
            .map(terra_build::kit::Kits::rules)
            .unwrap_or_default();
        let cx = Context::free(&rules);
        if !rise.planned {
            rise.planned = true;
            let plan: Vec<Op> = rise.whole.plan_operations();
            if let Err(why) = built.act(&plan, &cx) {
                warn!(
                    "Time-lapse: building {index}'s plan was refused: {}",
                    why.message()
                );
                rising.up[index] = true;
                continue;
            }
            clear.write(ClearAround(built.frame.translation, 9.0));
            open_sites(&mut market, index, true, hours);
        }
        let kits = built.kits.clone();
        let one_at_a_time = |kind| kits.as_deref().is_some_and(|k| k.one_at_a_time(kind));
        let wanted = eased((t - start) / takes);
        while budget > 0 && built.base.built_share() < wanted {
            let Some((step, op)) = construct::next_step(&built.base, rise.near, &cx, one_at_a_time)
            else {
                break;
            };
            if built.act(&[op], &cx).is_err() {
                break;
            }
            rise.near = construct::worked_at(&step).unwrap_or(rise.near);
            budget -= 1;
        }
        if construct::finished(&built.base) {
            // Its doors hung, mostly shut, and its shutters, mostly open.
            let fittings = catalogue.fittings();
            crate::build::props::fit_all(&mut built.0, |hang, size, at| {
                let kinds: Vec<_> = fittings
                    .iter()
                    .filter(|fitting| fitting.hang == hang && fitting.fits(size.0, size.1))
                    .collect();
                let fitting = kinds.get((index + at) % kinds.len().max(1))?;
                let open = match hang {
                    Hang::Door => (index + at) % 3 == 0,
                    Hang::Shutters => (index + at) % 4 != 0,
                };
                Some((fitting.kind, open))
            });
            rising.up[index] = true;
            rising.finished[index] = Some(t);
            open_sites(&mut market, index, false, hours);
            info!("Time-lapse: building {index} up at {t:.1} s.");
        }
    }
}

/// Opens or shuts a building's sites.
fn open_sites(market: &mut Market, building: usize, open: bool, hours: f64) {
    let sites: Vec<usize> = (0..market.life.town.things.len())
        .filter(|&i| {
            let thing = &market.life.town.things[i];
            thing.kind == ThingKind::Site && thing.building == Some(building)
        })
        .collect();
    for site in sites {
        market.life.set_open(site, open, hours);
    }
}

/// A building rising shows what stands of it, not a blueprint: its plan's ghosts are kept
/// out of sight, so a plot reads as ground cleared and footings going in, as a
/// city-builder's time-lapse shows one.
fn hide_ghosts(
    mut ghosts: Query<(&ChildOf, &mut Visibility), Added<PlanGhost>>,
    sections: Query<&ChildOf>,
    rises: Query<&Built, With<Rises>>,
) {
    if ghosts.is_empty() {
        return;
    }
    let roots: Vec<Entity> = rises.iter().map(|built| built.root).collect();
    for (section, mut shown) in &mut ghosts {
        let root = sections
            .get(section.parent())
            .map_or(section.parent(), |up| up.parent());
        if roots.contains(&root) || roots.contains(&section.parent()) {
            *shown = Visibility::Hidden;
        }
    }
}

/// The ground worn round each plot laid and in each yard of a house up, a few seconds at a
/// time, since every square of grass in reach grows again when what grows changes.
fn wear(
    mut commands: Commands,
    mut rising: ResMut<Rising>,
    wear: Option<Res<Wear>>,
    planet: Res<Planet>,
    rises: Query<&Rises>,
) {
    const EVERY_S: f32 = 4.0;
    let Some(wear) = wear else { return };
    let planned: Vec<usize> = rises
        .iter()
        .filter(|r| r.planned)
        .map(|r| r.index)
        .collect();
    if planned.len() == rising.worn.1 || rising.t < rising.worn.0 + EVERY_S {
        return;
    }
    rising.worn = (rising.t, planned.len());
    let up = rising.up.clone();
    let times = rising.times.clone();
    commands.insert_resource(wear.rules(
        &planet,
        |b| planned.contains(&b),
        |b| times.get(b).copied().flatten().is_some() && up.get(b).copied().unwrap_or(false),
    ));
}

/// Shows what waited, once its building is up and settled, or the settlement has reached
/// it.
fn reveal(
    mut commands: Commands,
    rising: Res<Rising>,
    mut waiting: Query<(Entity, &Waits, &mut Visibility)>,
) {
    for (entity, waits, mut shown) in &mut waiting {
        let ready = match waits.building {
            Some(b) => rising.settled(b),
            None => rising.t >= waits.at,
        };
        if ready {
            *shown = Visibility::Inherited;
            commands.entity(entity).remove::<Waits>();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lapse() -> Timelapse {
        Timelapse::parse(
            "(seconds: 40.0, hours: (7.0, 17.0), first: 2.0, last: 26.0, takes: (6.0, 12.0), buildings: Some(3), common: (10.0, 30.0))",
        )
        .unwrap()
    }

    #[test]
    fn the_nearest_buildings_go_up_first_and_the_biggest_take_longest() {
        let times = schedule(&lapse(), &[30.0, 5.0, 50.0, 12.0], &[400, 1600, 900, 100]);
        // Three of four, the furthest left for later.
        assert!(times[2].is_none());
        let (a, b, d) = (times[0].unwrap(), times[1].unwrap(), times[3].unwrap());
        assert!(b.0 < d.0 && d.0 < a.0, "{times:?}");
        assert_eq!(b.0, 2.0);
        assert_eq!(a.0, 26.0);
        assert!(b.1 > a.1 && a.1 > d.1, "{times:?}");
        assert_eq!(b.1, 12.0);
    }

    #[test]
    fn the_clock_runs_from_its_first_hour_to_its_last() {
        let lapse = lapse();
        assert_eq!(lapse.hour(-1.0), 7.0);
        assert_eq!(lapse.hour(20.0), 12.0);
        assert_eq!(lapse.hour(99.0), 17.0);
    }

    #[test]
    fn every_time_lapse_file_reads() {
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/data/timelapse");
        let mut read = 0;
        for entry in std::fs::read_dir(dir).expect("the time-lapses") {
            let path = entry.unwrap().path();
            let text = std::fs::read_to_string(&path).unwrap();
            Timelapse::parse(&text).unwrap_or_else(|why| panic!("{}: {why}", path.display()));
            read += 1;
        }
        assert!(read > 0);
    }
}
