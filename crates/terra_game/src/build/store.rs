//! A base's sections as entities (docs/building-plan.md, drawing; D211): the base is an
//! entity placed where it stands, and each section of its cubes, 16 a side, a child entity
//! with a [`Section`] and what is drawn of it under that. When the base changes, the
//! sections its change marks (`terra_build::Base::take_dirty`, a cube's section and its
//! neighbours on an edge) and the rest of the building they are in wait to be laid again
//! (`terra_build::sections::affected`); `layout` lays them on the workers, nearest the
//! camera first, and `draw` puts what they give on screen, a few a frame.

use super::{Build, Built, Shown, Site};
use crate::planet::Planet;
use bevy::prelude::*;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use terra_build::Base;
use terra_build::sections::{Reach, affected, drawable};

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
    /// What it was laid as (`terra_build::sections::Laid::key`).
    pub key: u64,
    /// How finely it is drawn.
    pub reach: Reach,
    /// How it was drawn, piece by piece and face by face, for the player's own base, so an
    /// edit draws again only what it reached (`sections::draw_again`).
    pub drawing: Option<Arc<terra_build::sections::Drawing>>,
    /// How many bytes its meshes take on the GPU.
    pub gpu_bytes: usize,
    /// Its near and middle detail a part at a time (`sections::CLUSTER`), each with what it
    /// was drawn from (`sections::cluster_keys`), its entities and their bytes on the GPU:
    /// an edit puts on screen again only the parts whose key changed.
    pub parts: Vec<OnScreen>,
    /// The entities of the rest of what is drawn of it, put on screen again each time.
    pub rest: Vec<Entity>,
}

/// A part of a section on screen: what it was drawn from, if that may be compared (not
/// once its look has changed), its entities, and their bytes on the GPU.
#[derive(Debug, Default)]
pub(super) struct OnScreen {
    pub key: Option<u64>,
    pub entities: Vec<Entity>,
    pub gpu_bytes: usize,
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
    /// The snapshot laid whole, which its sections are cut from, once a worker has laid it;
    /// and the worker laying it.
    pub whole: Option<Arc<terra_build::sections::Whole>>,
    pub laying: Option<bevy::tasks::Task<super::layout::Laying>>,
    /// The base as last laid and how, and the sections marked since: a change lays again
    /// only the buildings it marked (`Whole::lay_again`), not every one in the claim.
    pub last_laid: Option<(Arc<Base>, Arc<terra_build::sections::Whole>)>,
    pub marked_since: BTreeSet<[i32; 3]>,
    /// When the oldest change not yet all on screen was noticed, to time how long a
    /// change takes to be seen.
    pub changed_at: Option<std::time::Instant>,
    /// What a walker meets of a plan shown so far through, being worked out.
    pub standing: Option<bevy::tasks::Task<Base>>,
    /// Sections further off than they are seen, let go and not laid (`draw::aside`).
    pub far_off: BTreeSet<[i32; 3]>,
    /// The base's far meshes, a cell of it each (`hlod`).
    pub cells: super::hlod::Cells,
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
    for build in each(&mut site, &mut builds) {
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
    // Timed from here, a change to something on screen, not a build first laid.
    if !sections.drawn.is_empty() {
        sections
            .changed_at
            .get_or_insert_with(std::time::Instant::now);
    }
    // What was laid last, kept to lay only what changed from: the base it was laid from
    // and how, if it was laid; if not, what was kept before still stands.
    if let (Some(base), Some(whole)) = (sections.snapshot.take(), sections.whole.take()) {
        sections.last_laid = Some((base, whole));
        sections.marked_since.clear();
    }
    sections.laying = None;
    let marked = build.base.take_dirty();
    sections.marked_since.extend(marked.iter().copied());
    let mut wait = |at: [i32; 3], force: bool| {
        let waiting = sections.waiting.entry(at).or_default();
        waiting.force |= force;
    };
    if std::mem::take(&mut sections.everything) {
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
            sections.standing = Some(
                bevy::tasks::AsyncComputeTaskPool::get()
                    .spawn(async move { terra_build::sections::standing(&base, &kits, fraction) }),
            );
        }
        return;
    }
    // The sections whose cubes changed are merged again whatever; the rest of their
    // building, and where what goes in next is lit, only if their pieces did.
    for &at in &marked {
        wait(at, true);
    }
    // As far as the change reaches, from the base as last laid (`sections::reach`); with
    // nothing laid to go by, the whole of every building it marked.
    let reached: BTreeSet<[i32; 3]> = match (&sections.last_laid, &build.kits) {
        (Some((before, _)), Some(kits)) => {
            terra_build::sections::reach(before, &build.base, kits, &marked)
                .into_iter()
                .flatten()
                .collect()
        }
        _ => affected(&build.base, &marked),
    };
    for at in reached {
        wait(at, false);
    }
    let relit: BTreeSet<[i32; 3]> = std::mem::take(&mut sections.relight)
        .into_iter()
        .map(|cube| terra_build::cubes::section_of(IVec3::from_array(cube)).to_array())
        .collect();
    for at in affected(&build.base, &relit) {
        wait(at, false);
    }
    sections.change = Some(Change {
        since: std::time::Instant::now(),
        waiting: sections.waiting.len(),
        merged: 0,
        same: 0,
        laid_ms: 0.0,
        slowest_ms: 0.0,
        back_ms: 0.0,
    });
}

/// The middle of a section, in its base's own space, in metres: where what is drawn of it
/// is placed, and what its distance from the camera is measured to.
pub(super) fn centre(at: IVec3) -> Vec3 {
    let side = terra_build::cubes::SECTION as f32 * terra_build::CELL_M;
    (at.as_vec3() + 0.5) * side
}
