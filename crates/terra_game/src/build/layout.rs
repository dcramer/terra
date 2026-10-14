//! Laying a base's sections on the workers (docs/building-plan.md, drawing; D211): a base
//! changed is laid whole once on Bevy's async compute pool (`sections::Whole`); then the
//! sections `store` sets waiting are cut from it and merged there, as many at once as it
//! has threads, nearest the camera first; what comes back waits for `draw` to put it on
//! screen, a few a frame. Nothing heavy is done on the main thread.
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
use terra_build::sections::{self, Reach, Section};

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
    /// textured detail, and far only, for the bench.
    pub timings: Timings,
    /// Whether everything waiting was drawn when last looked.
    idle: bool,
    /// When work began after everything was last drawn: how long bases take to stream in.
    busy_since: Option<Instant>,
    /// Whether everything has been drawn once, after which work is streaming.
    drawn_once: bool,
    /// How many frames each build's laid sections have been held back for the rest of its
    /// change (`draw::upload`).
    pub held: std::collections::BTreeMap<Entity, u32>,
}

/// How long bases took to lay whole, and sections with anything in them to cut from them
/// and merge.
#[derive(Default)]
pub(super) struct Timings {
    pub whole: Vec<f32>,
    pub near: Vec<f32>,
    pub far: Vec<f32>,
    /// How long changes took from being noticed to all being on screen.
    pub edits: Vec<f32>,
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

/// A base laid again on a worker: how, how long it took, and the sections waiting that
/// lay as they were.
pub(super) struct Laying {
    pub whole: sections::Whole,
    pub took_ms: f32,
    pub same: Vec<[i32; 3]>,
}

/// What laying a section came to.
pub(super) enum Outcome {
    /// Its pieces as they were: left as it is drawn.
    Same,
    /// Drawn anew: what it was laid as, how finely, and what to show.
    Drawn {
        key: u64,
        reach: Reach,
        parts: Box<Parts>,
        drawing: Option<Arc<sections::Drawing>>,
        took_ms: f32,
    },
}

/// How finely a section is drawn for a camera so far from its middle, in metres: finer
/// detail is made a little before it is seen (`draw::NEAR_M`, `draw::MID_M`), and kept until
/// the camera is further off again, so a section near a boundary is not laid over and over;
/// but laid coarser before the detail it fades into is seen (`draw::MID_M` less its fade),
/// as a section drawn near has nothing coarser than its middle detail.
pub(super) fn reach(distance: f32, now: Option<Reach>) -> Reach {
    let kept = |reach: Reach| now.is_some_and(|now| now >= reach);
    if distance < 40.0 || (kept(Reach::Near) && distance < 50.0) {
        Reach::Near
    } else if distance < 95.0 || (kept(Reach::Mid) && distance < 140.0) {
        Reach::Mid
    } else {
        Reach::Far
    }
}

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
        .add_systems(
            Update,
            (super::draw::aside, launch).chain().in_set(Drawing::Launch),
        )
        .add_systems(Update, collect.in_set(Drawing::Collect))
        .add_systems(Update, track.in_set(crate::loading::Track));
}

/// The loading screen waits for the buildings there are to be laid and drawn, so the first
/// thing seen is the finished scene (D135).
fn track(
    jobs: Res<Jobs>,
    progress: Option<ResMut<crate::loading::Progress>>,
    mut site: ResMut<Site>,
    mut builds: Query<&mut Built>,
) {
    let Some(mut progress) = progress else {
        return;
    };
    let (mut waiting, mut drawn) = (0, 0);
    for build in each(&mut site, &mut builds) {
        waiting += build.sections.waiting.len();
        drawn += build.sections.drawn.len();
    }
    progress.report(
        "Laying buildings",
        waiting + jobs.running.len() + jobs.ready.len(),
        drawn,
    );
}

/// Sends the waiting sections nearest the camera to the workers, as many as they have
/// threads for.
fn launch(
    scale: Res<terra_world::detail::DetailScale>,
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
    let eye = camera
        .iter()
        .next()
        .map_or(Vec3::ZERO, GlobalTransform::translation);
    // Each build with sections waiting laid whole first, once a change, on a worker; a
    // scripted run waits for it.
    let wait = options.scripted() && !options.bench;
    let no_kits: Arc<Kits> = Arc::new(Kits(Vec::new()));
    for build in each(&mut site, &mut builds) {
        let progress = build.progress();
        let frame = build.frame;
        let root = build.root;
        let in_order = matches!(progress, terra_build::sections::Progress::Fraction(_));
        let sections = &mut build.sections;
        if sections.waiting.is_empty() || sections.whole.is_some() {
            continue;
        }
        if sections.laying.is_none() {
            let snapshot = sections
                .snapshot
                .get_or_insert_with(|| Arc::new(build.base.clone()))
                .clone();
            let kits = build.kits.clone().unwrap_or_else(|| no_kits.clone());
            // Only the buildings changed laid again, unless where pieces come in the order
            // the whole base goes in is wanted.
            let last = sections.last_laid.clone().filter(|_| !in_order);
            let marked = sections.marked_since.clone();
            // The sections waiting only because their building changed, drawn already as
            // finely as they would be: the worker cuts each from the base laid again, a
            // few hundredths of a millisecond, and says which lay as they were, to be left
            // as drawn rather than merged again, so an edit waits only for the sections it
            // changed and the main thread cuts none of them.
            let candidates: Vec<([i32; 3], u64)> = sections
                .waiting
                .iter()
                .filter(|(_, waiting)| !waiting.force && waiting.reach.is_none())
                .filter_map(|(&at, _)| {
                    let drawn = sections.drawn.get(&at)?;
                    let middle = frame.transform_point(store::centre(IVec3::from_array(at)));
                    let wanted = reach(middle.distance(eye) / scale.0, Some(drawn.reach));
                    (wanted == drawn.reach).then_some((at, drawn.key))
                })
                .collect();
            let progress = progress.clone();
            let maps = look.maps.clone();
            sections.laying = Some(pool.spawn(async move {
                let started = Instant::now();
                let whole = match last {
                    Some((before, whole)) => whole.lay_again(&before, &snapshot, &kits, &marked),
                    None => sections::Whole::lay(&snapshot, &kits),
                };
                let took_ms = started.elapsed().as_secs_f32() * 1000.0;
                let same = candidates
                    .into_iter()
                    .filter(|&(at, key)| {
                        let section = Section {
                            base: &snapshot,
                            kits: &kits,
                            maps: &maps,
                            at: IVec3::from_array(at),
                            progress: &progress,
                        };
                        sections::lay(&section, &whole).key() == key
                    })
                    .map(|(at, _)| at)
                    .collect();
                Laying {
                    whole,
                    took_ms,
                    same,
                }
            }));
        }
        let Some(task) = sections.laying.as_mut() else {
            continue;
        };
        let laid = if wait {
            Some(block_on(task))
        } else {
            block_on(poll_once(task))
        };
        if let Some(Laying {
            whole,
            took_ms,
            same,
        }) = laid
        {
            if let Some(change) = sections.change.as_mut() {
                change.laid_ms = change.since.elapsed().as_secs_f32() * 1000.0;
            }
            sections.whole = Some(Arc::new(whole));
            sections.laying = None;
            jobs.timings.whole.push(took_ms);
            for at in same {
                if sections
                    .waiting
                    .get(&at)
                    .is_none_or(|waiting| waiting.force || waiting.reach.is_some())
                {
                    continue;
                }
                sections.waiting.remove(&at);
                jobs.tickets += 1;
                let ticket = jobs.tickets;
                sections.tickets.insert(at, ticket);
                jobs.ready.push_back(Done {
                    root,
                    section: at,
                    ticket,
                    outcome: Outcome::Same,
                });
            }
        }
    }
    if room == 0 {
        return;
    }
    // The nearest waiting sections of every build laid whole.
    let mut nearest: Vec<(f32, Entity, [i32; 3])> = Vec::new();
    for build in each(&mut site, &mut builds) {
        if build.sections.whole.is_none() {
            continue;
        }
        for &at in build.sections.waiting.keys() {
            let middle = build
                .frame
                .transform_point(store::centre(IVec3::from_array(at)));
            // Measured against distances as the detail scale has them (D230).
            let distance = middle.distance(eye) / scale.0;
            nearest.push((distance, build.root, at));
        }
    }
    if nearest.is_empty() {
        return;
    }
    nearest.sort_by(|a, b| a.0.total_cmp(&b.0));
    nearest.truncate(room);
    // The player's own base, the first, which is the one edited, keeps how each section
    // was drawn, so an edit draws again only what it reached; the rest are drawn whole.
    for (index, build) in each(&mut site, &mut builds).enumerate() {
        let keeps = index == 0;
        let Some(whole) = build.sections.whole.clone() else {
            continue;
        };
        for &(distance, root, at) in &nearest {
            if root != build.root {
                continue;
            }
            let Some(waiting) = build.sections.waiting.remove(&at) else {
                continue;
            };
            jobs.tickets += 1;
            jobs.busy_since.get_or_insert_with(Instant::now);
            let ticket = jobs.tickets;
            build.sections.tickets.insert(at, ticket);
            let snapshot = build
                .sections
                .snapshot
                .get_or_insert_with(|| Arc::new(build.base.clone()))
                .clone();
            let kits = build.kits.clone().unwrap_or_else(|| no_kits.clone());
            let progress = build.progress();
            let maps = look.maps.clone();
            let drawn_before = build.sections.drawn.get(&at);
            let previous = drawn_before.map(|drawn| (drawn.key, drawn.reach));
            // What each part on screen was drawn from, while it is on screen.
            let part_keys: Vec<Option<u64>> = drawn_before
                .filter(|drawn| drawn.entity.is_some())
                .map(|drawn| drawn.parts.iter().map(|part| part.key).collect())
                .unwrap_or_default();
            let last = drawn_before
                .and_then(|drawn| drawn.drawing.clone())
                .filter(|_| keeps);
            let wanted = waiting
                .reach
                .unwrap_or_else(|| reach(distance, previous.map(|(_, reach)| reach)));
            let force = waiting.force;
            let whole = whole.clone();
            let task = pool.spawn(async move {
                let started = Instant::now();
                let section = Section {
                    base: &snapshot,
                    kits: &kits,
                    maps: &maps,
                    at: IVec3::from_array(at),
                    progress: &progress,
                };
                let laid = sections::lay(&section, &whole);
                let key = laid.key();
                if !force && previous == Some((key, wanted)) {
                    return Outcome::Same;
                }
                let mut drawing = keeps.then(sections::Drawing::default);
                // The base being built a part at a time, and only the parts whose key
                // changed, if it is drawn as finely as it was; any other in one part, fewer
                // meshes.
                let keys = if keeps {
                    sections::cluster_keys(&section, &laid, wanted)
                } else {
                    Vec::new()
                };
                let same_reach = previous.is_some_and(|(_, reach)| reach == wanted);
                let only: Vec<bool> = keys
                    .iter()
                    .enumerate()
                    .map(|(c, key)| {
                        !same_reach || part_keys.get(c).copied().flatten() != Some(*key)
                    })
                    .collect();
                let drawn = sections::draw_parts(
                    &section,
                    &laid,
                    wanted,
                    last.as_deref(),
                    drawing.as_mut(),
                    keeps
                        .then_some(only.as_slice())
                        .filter(|only| !only.is_empty()),
                );
                let parts = draw::parts(drawn, store::centre(section.at), &only, keys);
                Outcome::Drawn {
                    key,
                    reach: wanted,
                    parts: Box::new(parts),
                    drawing: drawing.map(Arc::new),
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
                // Only sections with something in them are timed.
                if let Outcome::Drawn {
                    reach,
                    took_ms,
                    parts,
                    ..
                } = &outcome
                    && !parts.is_empty()
                {
                    let list = if *reach > Reach::Far {
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
    /// Whether any section of a build is still on a worker.
    pub fn laying(&self, root: Entity) -> bool {
        self.running.iter().any(|job| job.root == root)
    }

    /// Whether nothing is being laid or waiting to be put on screen.
    pub fn quiet(&self) -> bool {
        self.running.is_empty() && self.ready.is_empty()
    }

    /// Notes whether everything is drawn, and says so when it first is: what a script
    /// waits for before a picture.
    pub fn settle(&mut self, nothing_waiting: bool) {
        let idle = nothing_waiting && self.quiet();
        let any = !self.timings.near.is_empty() || !self.timings.far.is_empty();
        if idle && !self.idle && any {
            let took_ms = self
                .busy_since
                .take()
                .map_or(0.0, |since| since.elapsed().as_secs_f32() * 1000.0);
            let median = |list: &Vec<f32>| {
                let mut sorted = list.clone();
                sorted.sort_by(f32::total_cmp);
                sorted.get(sorted.len() / 2).copied().unwrap_or(0.0)
            };
            if !self.timings.edits.is_empty() {
                info!(
                    "Builds changed: {} changes on screen in a median {:.0} ms from being made, the slowest {:.0} ms.",
                    self.timings.edits.len(),
                    median(&self.timings.edits),
                    self.timings.edits.iter().copied().fold(0.0, f32::max),
                );
            }
            if !self.drawn_once {
                info!(
                    "Builds drawn: {} bases laid whole in a median {:.2} ms, then {} sections cut from them and merged, textured in a median {:.2} ms, far only in {:.2} ms, on a worker; {:.0} ms from the first to the last on screen.",
                    self.timings.whole.len(),
                    median(&self.timings.whole),
                    self.timings.near.len() + self.timings.far.len(),
                    median(&self.timings.near),
                    median(&self.timings.far),
                    took_ms,
                );
            } else {
                // Sections laid again as the camera came nearer or went further.
                info!("Builds streamed in: {took_ms:.0} ms");
            }
            self.drawn_once = true;
        }
        self.idle = idle;
    }
}
