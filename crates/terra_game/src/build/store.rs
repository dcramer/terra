//! A base's sections as entities (docs/building-plan.md, drawing; D211): the base is an
//! entity placed where it stands, and each section of its cubes, 16 a side, a child entity
//! with a [`Section`] and what is drawn of it under that. When the base changes, the
//! sections its change marks (`terra_build::Base::take_dirty`, a cube's section and its
//! neighbours on an edge) and the rest of the building they are in wait to be laid again
//! (`terra_build_mesh::sections::affected`); `layout` lays them on the workers, nearest the
//! camera first, and `draw` puts what they give on screen, a few a frame.

use super::{Build, Built, Shown, Site};
use crate::planet::Planet;
use bevy::prelude::*;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use terra_build::Base;
use terra_build_mesh::sections::{Reach, drawable};

/// A base's entity: where it stands, its sections its children.
#[derive(Component)]
pub struct BaseRoot;

/// A section of a base's cubes, counted in sections: what is drawn of it is its children.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Section {
    pub at: IVec3,
}

/// What is drawn of a section.
#[derive(Debug)]
pub(super) struct Drawn {
    /// Its entity, if anything of it is drawn.
    pub entity: Option<Entity>,
    /// The job it came from: while the section's latest is another, it is being laid.
    pub ticket: u64,
    /// What it was laid as (`terra_build_mesh::sections::Laid::key`).
    pub key: u64,
    /// How finely it is drawn.
    pub reach: Reach,
    /// How it was drawn, piece by piece and face by face, for the player's own base, so an
    /// edit draws again only what it reached (`sections::draw_again`).
    pub drawing: Option<Arc<terra_build_mesh::sections::Drawing>>,
    /// How many bytes its meshes take on the GPU.
    pub gpu_bytes: usize,
    /// A group's sections with anything in them, as a box in its base's own space: what
    /// its distance is measured to once it is drawn ([`distance_from`]).
    pub bounds: Option<(Vec3, Vec3)>,
    /// Its near and middle detail a part at a time (`sections::CLUSTER`), each with what it
    /// was drawn from (`sections::cluster_keys`), its entities and their bytes on the GPU:
    /// an edit puts on screen again only the parts whose key changed.
    pub parts: Vec<OnScreen>,
    /// The entities of the rest of what is drawn of it, put on screen again each time.
    pub rest: Vec<Entity>,
    /// Drawn a detail at a time, each held in its slot of `parts` (`draw::detail`): a
    /// group of a base nobody is building.
    pub by_detail: bool,
}

/// A part of a section on screen: what it was drawn from, if that may be compared (not
/// once its look has changed), its entities, and their bytes on the GPU.
#[derive(Debug, Default)]
pub(super) struct OnScreen {
    pub key: Option<u64>,
    pub entities: Vec<Entity>,
    pub gpu_bytes: usize,
    /// The texture arrays' layers its meshes wear, kept resident while it is drawn (D290).
    pub layers: Vec<u16>,
    /// Its meshes in the texture arrays, kept on the CPU too for a base built by its cubes,
    /// so a repaint looks their layers up in place (D298).
    pub arrays: Vec<Handle<Mesh>>,
    /// Of its entities, the copies only the sun sees, whose range is their own.
    pub casters: Vec<Entity>,
}

/// A change of material and of nothing else (D298): each cube, from one kind to another,
/// and when it was done, for the drawing to look up its layers at once while the section is
/// laid again behind it.
#[derive(Clone, Debug)]
pub(super) struct Repaint {
    pub cubes: Vec<([i32; 3], terra_build::Kind, terra_build::Kind)>,
    pub since: std::time::Instant,
}

/// A section waiting to be laid: whether it must be merged again even if it lays the same,
/// as one whose cubes changed or whose look has must; and how finely, if that is decided.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Waiting {
    pub force: bool,
    pub reach: Option<Reach>,
}

/// A base's sections: what is drawn of each, and what waits to be laid again.
#[derive(Default)]
pub(super) struct Sections {
    pub drawn: BTreeMap<[i32; 3], Drawn>,
    /// Changes of material alone, waiting to be looked up in what is drawn (D298).
    pub repaints: Vec<Repaint>,
    pub waiting: BTreeMap<[i32; 3], Waiting>,
    /// The job each section was last sent out in: an answer to an older one is dropped.
    pub tickets: BTreeMap<[i32; 3], u64>,
    /// Lay every section again: it was replaced or moved, or its kits or maps changed.
    pub everything: bool,
    /// Cubes lit or no longer lit as what goes in next, since last noticed.
    pub relight: BTreeSet<[i32; 3]>,
    /// The revision last noticed, and what its sections are laid from.
    pub noticed: Option<u64>,
    pub snapshot: Option<Arc<Base>>,
    /// How many changes have been noticed: each section sent out is laid for one, and what
    /// comes back for a change is shown once nothing sent out for it or before it is out.
    pub generation: u64,
    /// The snapshot laid whole, which its sections are cut from, once a worker has laid it;
    /// and the worker laying it.
    pub whole: Option<Arc<terra_build_mesh::sections::Whole>>,
    pub laying: Option<bevy::tasks::Task<super::layout::Laying>>,
    /// The base as last laid and how, and the sections marked since: a change lays again
    /// only the buildings it marked (`Whole::lay_again`), not every one in the claim.
    pub last_laid: Option<(Arc<Base>, Arc<terra_build_mesh::sections::Whole>)>,
    pub marked_since: BTreeSet<[i32; 3]>,
    /// The sections of cubes lit or no longer lit as what goes in next since the base was
    /// last laid: the worker laying it again works out what they reach.
    pub relit_since: BTreeSet<[i32; 3]>,
    /// The change that lays every section again, as a base put up or replaced whole: shown
    /// a section at a time as each comes back, as a world streams in, not held for the
    /// rest, there being nothing of it on screen to tear.
    pub streaming: Option<u64>,
    /// When the oldest change not yet all on screen was noticed, to time how long a
    /// change takes to be seen.
    pub changed_at: Option<std::time::Instant>,
    /// What a walker meets of a plan shown so far through, being worked out.
    pub standing: Option<bevy::tasks::Task<Base>>,
    /// Sections further off than they are seen, let go and not laid (`draw::aside`).
    pub far_off: BTreeSet<[i32; 3]>,
    /// The base's far meshes, a cell of it each (`hlod`).
    pub cells: super::hlod::Cells,
    /// How many sections a side it is laid and drawn by: one for the base being built, so
    /// an edit lays again only what it reached; [`GROUP`] for every other, each group of
    /// sections one drawing, one entity a detail, as an HLOD cluster is ([`unit_of`]).
    pub unit: i32,
    /// The last change since it was noticed, until all it set waiting is on screen: when,
    /// how many sections it set waiting, and how many came back merged anew or the same,
    /// for `Build: change on screen` (research/building-audit.md).
    pub change: Option<Change>,
}

/// A change being drawn, timed from when it was noticed to when it is all on screen.
#[derive(Debug)]
pub(super) struct Change {
    pub since: std::time::Instant,
    pub waiting: usize,
    pub merged: usize,
    pub same: usize,
    /// How long after it the base was laid again, the slowest section took on its worker,
    /// and the last of them came back, in ms: where the time to the screen went.
    pub laid_ms: f32,
    pub slowest_ms: f32,
    pub back_ms: f32,
    /// How many corners builds had put on the GPU when it was made (`draw::PUT`).
    pub put_from: usize,
}

pub fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (notice, stand).in_set(super::layout::Drawing::Notice),
    );
}

/// Gives a plan shown so far through what stands of it, once worked out.
fn stand(mut site: ResMut<Site>, mut builds: Query<&mut Built>) {
    for build in each(&mut site, &mut builds) {
        let Some(task) = build.sections.standing.as_mut() else {
            continue;
        };
        if let Some(standing) = bevy::tasks::block_on(bevy::tasks::poll_once(task)) {
            build.standing = Some(standing);
            build.sections.standing = None;
        }
    }
}

/// Every build: the player's and everyone else's.
pub(super) fn each<'a>(
    site: &'a mut Site,
    builds: &'a mut Query<&mut Built>,
) -> impl Iterator<Item = &'a mut Build> + 'a {
    std::iter::once(&mut site.build).chain(builds.iter_mut().map(|b| &mut b.into_inner().0))
}

/// Notices which builds have changed since last looked at, works out again where they
/// stand on the ground, and sets the sections their changes reach waiting to be laid.
fn notice(planet: Res<Planet>, mut site: ResMut<Site>, mut builds: Query<&mut Built>) {
    for (index, build) in each(&mut site, &mut builds).enumerate() {
        // The player's own base, the first, is the one edited: drawn a section at a time.
        let unit = if index == 0 { 1 } else { GROUP };
        if build.sections.unit != unit {
            build.sections.unit = unit;
            build.sections.everything = true;
            build.sections.noticed = None;
        }
        if build.sections.noticed == Some(build.revision) {
            continue;
        }
        if build.refreshed != build.revision {
            build.refresh(&planet);
        }
        noticed(build);
    }
}

/// Sets what a change to a build reaches waiting to be laid.
fn noticed(build: &mut Build) {
    let sections = &mut build.sections;
    sections.noticed = Some(build.revision);
    sections.generation += 1;
    // Timed from here, a change to something on screen, not a build first laid.
    if !sections.drawn.is_empty() {
        sections
            .changed_at
            .get_or_insert_with(std::time::Instant::now);
    }
    // What was laid last, kept to lay only what changed from: the base it was laid from
    // and how, if it was laid; if not, what was kept before still stands.
    if let (Some(base), Some(whole)) = (sections.snapshot.take(), sections.whole.take()) {
        let_go(sections.last_laid.replace((base, whole)));
        sections.marked_since.clear();
    }
    sections.laying = None;
    let marked = build.base.take_dirty();
    sections.marked_since.extend(marked.iter().copied());
    let unit = sections.unit;
    let mut wait = |at: [i32; 3], force: bool| {
        let waiting = sections.waiting.entry(unit_of(at, unit)).or_default();
        waiting.force |= force;
    };
    if std::mem::take(&mut sections.everything) {
        sections.streaming = Some(sections.generation);
        // Drawn again whole: its look may have changed whatever its parts were drawn from.
        for drawn in sections.drawn.values_mut() {
            for part in &mut drawn.parts {
                part.key = None;
            }
        }
        sections.last_laid = None;
        let all: BTreeSet<[i32; 3]> = drawable(&build.base)
            .into_iter()
            .chain(sections.drawn.keys().copied())
            .collect();
        for at in all {
            wait(at, true);
        }
        if let (Shown::Plan(fraction), Some(kits)) = (build.shown, build.kits.clone()) {
            let base = build.base.clone();
            sections.standing = Some(bevy::tasks::AsyncComputeTaskPool::get().spawn(async move {
                terra_build_mesh::sections::standing(&base, &kits, fraction)
            }));
        }
        return;
    }
    // The sections whose cubes changed are merged again whatever; the rest of their
    // building, and where what goes in next is lit, only if their pieces did.
    for &at in &marked {
        wait(at, true);
    }
    // How far the change reaches, and where what goes in next is lit, is worked out on the
    // worker that lays the base again (`layout`), which sets those sections waiting too:
    // the main thread only marks what changed.
    let relit: BTreeSet<[i32; 3]> = std::mem::take(&mut sections.relight)
        .into_iter()
        .map(|cube| terra_build::cubes::section_of(IVec3::from_array(cube)).to_array())
        .collect();
    for &at in &relit {
        wait(at, false);
    }
    sections.relit_since.extend(relit);
    sections.change = Some(Change {
        since: std::time::Instant::now(),
        waiting: sections.waiting.len(),
        merged: 0,
        same: 0,
        laid_ms: 0.0,
        slowest_ms: 0.0,
        back_ms: 0.0,
        put_from: super::draw::PUT[1].load(std::sync::atomic::Ordering::Relaxed),
    });
}

/// Lets something big go on a worker rather than the main thread: a castle's base and how
/// it was laid, or how a section was drawn, are hundreds of thousands of small allocations,
/// slow to free (D308).
pub(super) fn let_go<T: Send + 'static>(big: T) {
    // Where there are no workers, as in a test, here.
    if let Some(pool) = bevy::tasks::AsyncComputeTaskPool::try_get() {
        pool.spawn(async move { drop(big) }).detach();
    }
}

/// How much further a group is taken to be than its nearest point: half a section's
/// diagonal, so its details reach as far from what is drawn as a section's measured from
/// its middle do.
pub(super) const GROUP_SHIFT_M: f32 = 7.0;

/// How many sections a side a base not being built is drawn by: a house is one or two such
/// groups, so the town's entities are a few a house where they were a few a section.
pub(super) const GROUP: i32 = 4;

/// The group a section is drawn in, as the first section of it, for a base drawn `unit`
/// sections a side; itself, drawn alone.
pub(super) fn unit_of(at: [i32; 3], unit: i32) -> [i32; 3] {
    if unit <= 1 {
        return at;
    }
    // Groups straddle a base's middle rather than meet there: a base is laid about its
    // origin, and a house there would be eight groups.
    let shift = unit / 2;
    at.map(|a| (a + shift).div_euclid(unit) * unit - shift)
}

/// The sections of a base in a group that have anything to draw.
pub(super) fn members(base: &Base, key: [i32; 3], unit: i32) -> Vec<[i32; 3]> {
    if unit <= 1 {
        return vec![key];
    }
    drawable(base)
        .into_iter()
        .filter(|&at| unit_of(at, unit) == key)
        .collect()
}

/// A group's box in its base's own space, in metres, low corner and high.
pub(super) fn unit_box(key: [i32; 3], unit: i32) -> (Vec3, Vec3) {
    let side = terra_build::cubes::SECTION as f32 * terra_build::CELL_M;
    let low = IVec3::from_array(key).as_vec3() * side;
    (low, low + Vec3::splat(side * unit.max(1) as f32))
}

/// How far a camera, in its base's own space, is from what is drawn of a group: from a
/// section's middle, drawn alone, as its details' ranges are; from the nearest point of a
/// group's box, as a group's are (`VisibilityRange::use_aabb`), so a house is drawn near
/// wherever it is stood beside.
pub(super) fn distance_to(eye: Vec3, key: [i32; 3], unit: i32) -> f32 {
    if unit <= 1 {
        return centre(IVec3::from_array(key)).distance(eye);
    }
    let (low, high) = unit_box(key, unit);
    eye.clamp(low, high).distance(eye) + GROUP_SHIFT_M
}

/// Where what is drawn of a group is placed, in its base's own space: a section's middle,
/// or the middle of a group's box.
pub(super) fn unit_centre(key: [i32; 3], unit: i32) -> Vec3 {
    if unit <= 1 {
        return centre(IVec3::from_array(key));
    }
    let (low, high) = unit_box(key, unit);
    (low + high) * 0.5
}

/// How far the camera is from what is drawn of a group, in metres, as [`distance_to`].
/// A group drawn is measured to the box of its sections with anything in them, a house's
/// own, rather than to the group's.
pub(super) fn distance_from(
    frame: &Transform,
    eye: Vec3,
    key: [i32; 3],
    unit: i32,
    bounds: Option<(Vec3, Vec3)>,
) -> f32 {
    let local = frame.compute_affine().inverse().transform_point3(eye);
    match bounds {
        Some(bounds) if unit > 1 => {
            let (middle, reach) = sphere(bounds);
            (local.distance(middle) - reach).max(0.0) + GROUP_SHIFT_M
        }
        _ => distance_to(local, key, unit),
    }
}

/// A box's middle and how far its corners are from it: a drawn group is placed at its
/// sections' middle and measured from there less this, as Bevy measures a detail's range
/// from where it is placed and dithers its crossfade by that; measured to its box instead
/// (`use_aabb`), a whole house popped as it switched.
pub(super) fn sphere((low, high): (Vec3, Vec3)) -> (Vec3, f32) {
    ((low + high) * 0.5, (high - low).length() * 0.5)
}

/// The box of a group's members, in its base's own space.
pub(super) fn members_box(members: &[[i32; 3]]) -> Option<(Vec3, Vec3)> {
    let side = terra_build::cubes::SECTION as f32 * terra_build::CELL_M;
    members.iter().fold(None, |sum, &at| {
        let low = IVec3::from_array(at).as_vec3() * side;
        let high = low + Vec3::splat(side);
        Some(sum.map_or((low, high), |(a, b): (Vec3, Vec3)| {
            (a.min(low), b.max(high))
        }))
    })
}

/// What a group of sections lays as, from what each of its members lays as: a section
/// alone's own key.
pub(super) fn group_key(keys: impl IntoIterator<Item = u64>) -> u64 {
    keys.into_iter()
        .fold(0u64, |sum, key| sum.rotate_left(17) ^ key)
}

/// The middle of a section, in its base's own space, in metres: where what is drawn of it
/// is placed, and what its distance from the camera is measured to.
pub(super) fn centre(at: IVec3) -> Vec3 {
    let side = terra_build::cubes::SECTION as f32 * terra_build::CELL_M;
    (at.as_vec3() + 0.5) * side
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_house_about_its_base_middle_is_one_group_measured_from_its_middle_less_its_reach() {
        // A house's sections round its base's middle, two storeys: one group, not eight.
        let house = [[-1, 0, -1], [0, 0, 0], [-1, 1, 0], [0, 1, -1]];
        let groups: BTreeSet<[i32; 3]> = house.iter().map(|&at| unit_of(at, GROUP)).collect();
        assert_eq!(groups.len(), 1);
        // The base being built is drawn a section at a time.
        assert_eq!(unit_of([5, 0, -3], 1), [5, 0, -3]);
        // Its sections reach 8 m each way and 16 m up from its middle, (0, 8, 0): from 30 m
        // off along x it is 30 m less that reach, and half a section further, as a section
        // measured from its middle would be.
        let bounds = members_box(&house);
        let (middle, reach) = sphere(bounds.unwrap());
        assert_eq!(middle, Vec3::new(0.0, 8.0, 0.0));
        let key = unit_of(house[0], GROUP);
        let off = Vec3::new(30.0, 8.0, 0.0);
        let distance = distance_from(&Transform::IDENTITY, off, key, GROUP, bounds);
        let expected = 30.0 - reach + GROUP_SHIFT_M;
        assert!((distance - expected).abs() < 1e-4, "{distance}");
        // Beside its wall, it is as near as can be.
        let beside = Vec3::new(9.0, 1.0, 0.0);
        let near = distance_from(&Transform::IDENTITY, beside, key, GROUP, bounds);
        assert!((near - GROUP_SHIFT_M).abs() < 1e-4, "{near}");
    }
}
