//! Laying a base's sections on the workers (docs/building-plan.md, drawing; D211): the
//! sections `store` sets waiting are laid and merged on Bevy's async compute pool, as many
//! at once as it has threads, nearest the camera first; what comes back waits for `draw`
//! to put it on screen, a few a frame. Nothing heavy is done on the main thread.
//!
//! A section laid as it was, its pieces the same, comes back as that alone and is left as
//! it is drawn. A scripted run waits for every section, so its pictures are the same every
//! time; a benchmark does not, as a player does not.

use super::draw::{self, Parts};
use super::store::{self, each};
use super::{BuildLook, Built, Site};
use crate::planet::MainCamera;
use crate::shell::Options;
use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task, block_on, poll_once};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Instant;
use terra_build::kit::Kits;
use terra_build::sections::{self, Section};

/// The steps of drawing builds, in order.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub(super) enum Drawing {
    /// Changes noticed and sections set waiting.
    Notice,
    /// Sections sent to the workers.
    Launch,
    /// What the workers have finished taken back.
    Collect,
    /// Put on screen, and near detail kept to the sections near the camera.
    Upload,
}

/// Sections being laid on the workers, and what has come back.
#[derive(Resource, Default)]
pub(super) struct Jobs {
    running: Vec<Job>,
    pub ready: VecDeque<Done>,
    tickets: u64,
    /// How long sections took on their workers, in milliseconds, laid and merged with
    /// their near detail, and far only, for the bench.
    pub timings: Timings,
    /// Whether everything waiting was drawn when last looked.
    idle: bool,
}

/// How long sections took to lay and merge.
#[derive(Default)]
pub(super) struct Timings {
    pub near: Vec<f32>,
    pub far: Vec<f32>,
}

struct Job {
    root: Entity,
    section: [i32; 3],
    ticket: u64,
    task: Task<Outcome>,
}

/// A section laid, for `draw`.
pub(super) struct Done {
    pub root: Entity,
    pub section: [i32; 3],
    pub ticket: u64,
    pub outcome: Outcome,
}

/// What laying a section came to.
pub(super) enum Outcome {
    /// Its pieces as they were: left as it is drawn.
    Same,
    /// Drawn anew: what it was laid as, whether with its near detail, and what to show.
    Drawn {
        key: u64,
        near: bool,
        parts: Box<Parts>,
        took_ms: f32,
    },
}

/// How near the camera a section's near detail is made, and how far it must go before it
/// is dropped, in metres: past where it is seen (`draw::NEAR_M`), so it is ready in time.
pub(super) const NEAR_LOAD_M: f32 = 95.0;
pub(super) const NEAR_DROP_M: f32 = 140.0;

pub fn plugin(app: &mut App) {
    app.init_resource::<Jobs>()
        .configure_sets(
            Update,
            (
                Drawing::Notice,
                Drawing::Launch,
                Drawing::Collect,
                Drawing::Upload,
            )
                .chain(),
        )
        .add_systems(Update, launch.in_set(Drawing::Launch))
        .add_systems(Update, collect.in_set(Drawing::Collect));
}

/// Sends the waiting sections nearest the camera to the workers, as many as they have
/// threads for.
fn launch(
    mut jobs: ResMut<Jobs>,
    look: Res<BuildLook>,
    options: Res<Options>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
    mut site: ResMut<Site>,
    mut builds: Query<&mut Built>,
) {
    let pool = AsyncComputeTaskPool::get();
    // A scripted run sends everything at once and waits for it.
    let room = if options.scripted() && !options.bench {
        usize::MAX
    } else {
        pool.thread_num().max(2).saturating_sub(jobs.running.len())
    };
    if room == 0 {
        return;
    }
    let eye = camera
        .iter()
        .next()
        .map_or(Vec3::ZERO, GlobalTransform::translation);
    // The nearest waiting sections of every build.
    let mut nearest: Vec<(f32, Entity, [i32; 3])> = Vec::new();
    for build in each(&mut site, &mut builds) {
        for &at in build.sections.waiting.keys() {
            let middle = build
                .frame
                .transform_point(store::centre(IVec3::from_array(at)));
            nearest.push((middle.distance(eye), build.root, at));
        }
    }
    if nearest.is_empty() {
        return;
    }
    nearest.sort_by(|a, b| a.0.total_cmp(&b.0));
    nearest.truncate(room);
    let no_kits: Arc<Kits> = Arc::new(Kits(Vec::new()));
    let maps = look.maps.clone();
    for build in each(&mut site, &mut builds) {
        for &(distance, root, at) in &nearest {
            if root != build.root {
                continue;
            }
            let Some(waiting) = build.sections.waiting.remove(&at) else {
                continue;
            };
            jobs.tickets += 1;
            let ticket = jobs.tickets;
            build.sections.tickets.insert(at, ticket);
            let snapshot = build
                .sections
                .snapshot
                .get_or_insert_with(|| Arc::new(build.base.clone()))
                .clone();
            let kits = build.kits.clone().unwrap_or_else(|| no_kits.clone());
            let progress = build.progress();
            let flights = build.flights.clone();
            let maps = maps.clone();
            let previous = build
                .sections
                .drawn
                .get(&at)
                .map(|drawn| (drawn.key, drawn.near));
            let near = distance < NEAR_LOAD_M;
            let force = waiting.force;
            let task = pool.spawn(async move {
                let started = Instant::now();
                let section = Section {
                    base: &snapshot,
                    kits: &kits,
                    maps: &maps,
                    at: IVec3::from_array(at),
                    progress: &progress,
                    flights: &flights,
                };
                let laid = sections::lay(&section);
                let key = laid.key();
                if !force && previous == Some((key, near)) {
                    return Outcome::Same;
                }
                let drawn = sections::draw(&section, &laid, near);
                let parts = draw::parts(drawn, store::centre(section.at));
                Outcome::Drawn {
                    key,
                    near,
                    parts: Box::new(parts),
                    took_ms: started.elapsed().as_secs_f32() * 1000.0,
                }
            });
            jobs.running.push(Job {
                root,
                section: at,
                ticket,
                task,
            });
        }
    }
}

/// Takes back what the workers have finished, for `draw`; in a scripted run, waits for
/// all of it.
fn collect(mut jobs: ResMut<Jobs>, options: Res<Options>) {
    let wait = options.scripted() && !options.bench;
    let running = std::mem::take(&mut jobs.running);
    for mut job in running {
        let outcome = if wait {
            Some(block_on(&mut job.task))
        } else {
            block_on(poll_once(&mut job.task))
        };
        match outcome {
            Some(outcome) => {
                if let Outcome::Drawn { near, took_ms, .. } = &outcome {
                    let list = if *near {
                        &mut jobs.timings.near
                    } else {
                        &mut jobs.timings.far
                    };
                    list.push(*took_ms);
                }
                jobs.ready.push_back(Done {
                    root: job.root,
                    section: job.section,
                    ticket: job.ticket,
                    outcome,
                });
            }
            None => jobs.running.push(job),
        }
    }
}

impl Jobs {
    /// Whether nothing is being laid or waiting to be put on screen.
    pub fn quiet(&self) -> bool {
        self.running.is_empty() && self.ready.is_empty()
    }

    /// Notes whether everything is drawn, and says so when it first is: what a script
    /// waits for before a picture.
    pub fn settle(&mut self, nothing_waiting: bool) {
        let idle = nothing_waiting && self.quiet();
        if idle && !self.idle {
            let median = |list: &Vec<f32>| {
                let mut sorted = list.clone();
                sorted.sort_by(f32::total_cmp);
                sorted.get(sorted.len() / 2).copied().unwrap_or(0.0)
            };
            info!(
                "Builds drawn: {} sections laid and merged near in a median {:.2} ms, far only in {:.2} ms on a worker.",
                self.timings.near.len() + self.timings.far.len(),
                median(&self.timings.near),
                median(&self.timings.far),
            );
        }
        self.idle = idle;
    }
}
