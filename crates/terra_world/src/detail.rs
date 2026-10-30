//! How far detail reaches, as one scale over every distance at which something changes its
//! level of detail or comes into view: each object's own distances, as authored, times
//! [`DetailScale`]. The Witcher 3 does the same (`FoliageDistanceScale` over each asset's own
//! distances); it doubles as a quality setting.
//!
//! It also makes popping checkable without a person (D69, D230). A scripted run given
//! `--detail-sweep` holds the camera, the clock and the wind still and sweeps the scale over
//! frames, so every crown, trunk, shadow and ring of grass crosses its switch in front of a
//! still camera, and anything that changes between two frames is a level of detail changing
//! (`tools/sandbox/lodcheck.ts`).

use crate::shell::{Options, ScriptFrame};
use bevy::{
    camera::visibility::VisibilityRange, ecs::entity::EntityHashMap,
    platform::collections::HashMap, prelude::*,
};

/// Every distance at which detail changes is its own, as authored, times this.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct DetailScale(pub f32);

impl Default for DetailScale {
    fn default() -> Self {
        Self(1.0)
    }
}

/// An object's distances as authored, before the scale.
#[derive(Component, Clone)]
struct Authored(VisibilityRange);

pub fn plugin(app: &mut App) {
    app.init_resource::<DetailScale>()
        .init_resource::<InUse>()
        .add_systems(
            PostUpdate,
            (sweep, remember, rescale, guard)
                .chain()
                .before(bevy::camera::visibility::VisibilitySystems::CheckVisibility),
        );
}

/// Gives an object already ranged new distances, as authored: they are scaled as a new
/// object's are, and kept for when the scale changes. Inserting a `VisibilityRange` again
/// alone would be taken as already scaled.
pub fn set_range(entity: &mut EntityCommands, range: VisibilityRange) {
    entity.remove::<Authored>().insert(range);
}

/// Keeps what each newly ranged object was given, and scales it at once.
fn remember(
    mut commands: Commands,
    scale: Res<DetailScale>,
    mut new: Query<(Entity, &mut VisibilityRange), Without<Authored>>,
) {
    for (entity, mut range) in &mut new {
        commands.entity(entity).insert(Authored(range.clone()));
        *range = scaled(&range, scale.0);
    }
}

/// When the scale changes, every object\'s distances follow it.
fn rescale(scale: Res<DetailScale>, mut ranged: Query<(&Authored, &mut VisibilityRange)>) {
    if !scale.is_changed() {
        return;
    }
    for (authored, mut range) in &mut ranged {
        *range = scaled(&authored.0, scale.0);
    }
}

/// Distances are kept to a grid this fine, so however many things are placed, and however
/// their own distances vary, few distinct ranges are in use: Bevy draws at most 65,535,
/// and past that shares a slot among the rest and culls them wrongly (D230).
const GRID_M: f32 = 0.25;
/// Past this many distinct ranges, well short of Bevy's limit, the world says so.
pub const TOO_MANY_RANGES: usize = 20_000;

fn on_grid(m: f32) -> f32 {
    (m / GRID_M).round() * GRID_M
}

fn scaled(range: &VisibilityRange, by: f32) -> VisibilityRange {
    let at = |m: f32| on_grid(m * by);
    VisibilityRange {
        start_margin: at(range.start_margin.start)..at(range.start_margin.end),
        end_margin: at(range.end_margin.start)..at(range.end_margin.end),
        use_aabb: range.use_aabb,
    }
}

/// What tells two ranges apart for Bevy's slots.
type RangeKey = [u32; 4];

fn key(r: &VisibilityRange) -> RangeKey {
    [
        r.start_margin.start,
        r.start_margin.end,
        r.end_margin.start,
        r.end_margin.end,
    ]
    .map(f32::to_bits)
}

/// The distinct ranges in use, kept up as ranges are given, changed and taken away, so a
/// change to one costs the change alone, never a walk over every ranged object (D127).
#[derive(Resource, Default)]
struct InUse {
    of: EntityHashMap<RangeKey>,
    users: HashMap<RangeKey, u32>,
}

impl InUse {
    fn set(&mut self, entity: Entity, range: &VisibilityRange) {
        let new = key(range);
        if let Some(old) = self.of.insert(entity, new) {
            if old == new {
                return;
            }
            self.drop_one(old);
        }
        *self.users.entry(new).or_default() += 1;
    }

    fn remove(&mut self, entity: Entity) {
        if let Some(old) = self.of.remove(&entity) {
            self.drop_one(old);
        }
    }

    fn drop_one(&mut self, key: RangeKey) {
        if let Some(users) = self.users.get_mut(&key) {
            *users -= 1;
            if *users == 0 {
                self.users.remove(&key);
            }
        }
    }

    fn distinct(&self) -> usize {
        self.users.len()
    }
}

/// Counts the distinct ranges in use as they change, and says so, as an error a check
/// fails on, if they are past [`TOO_MANY_RANGES`].
fn guard(
    changed: Query<(Entity, &VisibilityRange), Changed<VisibilityRange>>,
    mut removed: RemovedComponents<VisibilityRange>,
    mut in_use: ResMut<InUse>,
    mut said: Local<bool>,
) {
    // Removals first: a range taken away and given again this frame is changed as well.
    for entity in removed.read() {
        in_use.remove(entity);
    }
    for (entity, range) in &changed {
        in_use.set(entity, range);
    }
    let distinct = in_use.distinct();
    if distinct > TOO_MANY_RANGES && !*said {
        error!(
            "{distinct} distinct detail ranges are in use, past {TOO_MANY_RANGES}: Bevy draws at most 65,535 and culls the rest wrongly (D230)"
        );
        *said = true;
    }
}

/// Over a scripted run\'s sweep, sets the scale for its frame, the clock held still so
/// nothing but detail changes between frames.
fn sweep(
    options: Res<Options>,
    frame: Res<ScriptFrame>,
    mut scale: ResMut<DetailScale>,
    mut time: ResMut<Time<Virtual>>,
) {
    let Some(sweep) = options.detail_sweep else {
        return;
    };
    if let Some(now) = sweep.at(frame.0) {
        if !time.is_paused() {
            time.pause();
        }
        if scale.0 != now {
            scale.0 = now;
        }
    }
}

/// A sweep of the detail scale over frames of a scripted run: from one scale to another,
/// between two frames, counted as `--press` counts them; logarithmic, so each frame moves
/// the switches by the same share of their distance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sweep {
    pub from: f32,
    pub to: f32,
    pub first: u32,
    pub last: u32,
}

impl Sweep {
    pub fn parse(text: &str) -> Result<Self, String> {
        let problem =
            || format!("--detail-sweep wants scales and frames, like 0.5-2@300-600, not `{text}`");
        let (scales, frames) = text.split_once('@').ok_or_else(problem)?;
        let pair = |part: &str| -> Result<(String, String), String> {
            part.split_once('-')
                .map(|(a, b)| (a.to_string(), b.to_string()))
                .ok_or_else(problem)
        };
        let (from, to) = pair(scales)?;
        let (first, last) = pair(frames)?;
        let sweep = Self {
            from: from.trim().parse().map_err(|_| problem())?,
            to: to.trim().parse().map_err(|_| problem())?,
            first: first.trim().parse().map_err(|_| problem())?,
            last: last.trim().parse().map_err(|_| problem())?,
        };
        if sweep.from <= 0.0 || sweep.to <= 0.0 || sweep.last <= sweep.first {
            return Err(problem());
        }
        Ok(sweep)
    }

    /// The scale on a frame: none before the sweep, the end\'s after it.
    pub fn at(&self, frame: u32) -> Option<f32> {
        if frame < self.first {
            return None;
        }
        let t = ((frame - self.first) as f32 / (self.last - self.first) as f32).min(1.0);
        Some(libm::expf(
            libm::logf(self.from) * (1.0 - t) + libm::logf(self.to) * t,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sweep_runs_from_one_scale_to_the_other_evenly_by_ratio() {
        let sweep = Sweep::parse("0.5-2@100-200").unwrap();
        assert_eq!(sweep.at(99), None);
        assert!((sweep.at(100).unwrap() - 0.5).abs() < 1e-5);
        assert!(
            (sweep.at(150).unwrap() - 1.0).abs() < 1e-4,
            "halfway by ratio is 1"
        );
        assert!((sweep.at(200).unwrap() - 2.0).abs() < 1e-4);
        assert!((sweep.at(900).unwrap() - 2.0).abs() < 1e-4);
        assert!(Sweep::parse("2@1-2").is_err() && Sweep::parse("1-2@5-5").is_err());
    }

    #[test]
    fn a_forest_of_trees_each_its_own_size_shares_few_ranges() {
        // Ten thousand trees, each scaled by its own height, as the lush forest grows them:
        // on the grid, their ranges are a few thousand at most, far from Bevy's limit.
        let ranges: Vec<VisibilityRange> = (0..10_000)
            .flat_map(|i| {
                let size = 0.45 + 0.95 * ((i as f32 * 0.618_034).fract());
                [30.0, 75.0, 180.0, 420.0].map(|m: f32| {
                    let range = VisibilityRange {
                        start_margin: 0.0..0.0,
                        end_margin: m * size * 0.8..m * size,
                        use_aabb: false,
                    };
                    scaled(&range, 1.0)
                })
            })
            .collect();
        let count = recount(ranges.iter());
        assert!(count < 6_000, "{count} distinct ranges");
        assert!(count < TOO_MANY_RANGES);
    }

    /// The distinct ranges counted afresh, the other way to know the answer.
    fn recount<'a>(ranges: impl Iterator<Item = &'a VisibilityRange>) -> usize {
        ranges
            .map(key)
            .collect::<std::collections::HashSet<_>>()
            .len()
    }

    fn range(end: f32) -> VisibilityRange {
        VisibilityRange {
            start_margin: 0.0..0.0,
            end_margin: end - 5.0..end,
            use_aabb: false,
        }
    }

    #[test]
    fn the_ranges_in_use_are_kept_as_they_are_given_changed_and_taken_away() {
        let mut app = App::new();
        app.init_resource::<InUse>().add_systems(Update, guard);
        let world = app.world_mut();
        let things: Vec<Entity> = (0..40)
            .map(|i| world.spawn(range(10.0 * (i % 7) as f32)).id())
            .collect();
        let tallied = |app: &mut App| {
            let world = app.world_mut();
            let kept = world.resource::<InUse>().distinct();
            let afresh = recount(world.query::<&VisibilityRange>().iter(world));
            assert_eq!(kept, afresh);
            kept
        };
        app.update();
        assert_eq!(tallied(&mut app), 7);
        // A change to a range in use by others, then to one used by no one else.
        *app.world_mut()
            .get_mut::<VisibilityRange>(things[0])
            .unwrap() = range(500.0);
        app.update();
        assert_eq!(tallied(&mut app), 8);
        // The only user of a range despawned, a range taken away, and one taken away and
        // given again in the same frame.
        app.world_mut().despawn(things[0]);
        app.world_mut()
            .entity_mut(things[1])
            .remove::<VisibilityRange>();
        app.world_mut()
            .entity_mut(things[2])
            .remove::<VisibilityRange>()
            .insert(range(600.0));
        app.update();
        tallied(&mut app);
        for &thing in &things[3..] {
            app.world_mut().despawn(thing);
        }
        app.update();
        assert_eq!(tallied(&mut app), 1);
    }

    #[test]
    fn distances_follow_the_scale_from_what_was_authored() {
        let mut app = App::new();
        app.insert_resource(DetailScale(2.0))
            .add_systems(Update, (remember, rescale).chain());
        let thing = app
            .world_mut()
            .spawn(VisibilityRange {
                start_margin: 10.0..12.0,
                end_margin: 40.0..50.0,
                use_aabb: false,
            })
            .id();
        app.update();
        let range = app.world().get::<VisibilityRange>(thing).unwrap().clone();
        assert_eq!(range.end_margin, 80.0..100.0);
        app.world_mut().resource_mut::<DetailScale>().0 = 0.5;
        app.update();
        let range = app.world().get::<VisibilityRange>(thing).unwrap().clone();
        assert_eq!(
            range.start_margin,
            5.0..6.0,
            "from what was authored, not compounded"
        );
    }
}
