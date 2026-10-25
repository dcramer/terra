//! Diving, rising and breath (docs/controller.md criterion 17, D74, D478), for two kinds: a
//! swimmer moving while looking down dives at a rate its look's pitch sets, holding jump rises,
//! and with neither floats back up; afloat it keeps its mouth out with no bob growing; its
//! breath is used only with its mouth under (judged here by the water's surface and the kind's
//! mouth, not by the mover), runs out, drowns it a share a second and comes back at the
//! surface; a diver swims along a lake's bed without sinking into it, and swims from under
//! water up a shore and out. Each answer is the fixture's geometry or the kind's numbers.

use super::data::second;
use super::field::{Course, Water, forward_at, local, world};
use super::runs::{Outcome, Run, ground};
use super::water::{SURFACE, lake};
use super::{Intent, Kind, Measure, Mover, TICK};
use glam::{Vec2, Vec3};

/// The deep pool: its walls, the bed under the dry ground, m, and where a diver starts.
const POOL: [(f32, f32); 6] = [
    (-10.0, 0.0),
    (4.0, 0.0),
    (10.0, -12.0),
    (40.0, -12.0),
    (46.0, 0.0),
    (80.0, 0.0),
];
const BED: f32 = -12.0;
const START_U: f32 = 8.0;
/// The pitches dived at, deg: inside and past each kind's `dive_from_deg`.
const PITCHES: [f32; 4] = [-10.0, -30.0, -50.0, -70.0];
/// When a stroke's rate is read, s after it starts: past its acceleration.
const RATE_FROM_S: f32 = 1.0;
const RATE_S: f32 = 1.0;
/// How deep under the surface a riser starts, m, and how long one left alone is given.
const RISE_FROM_M: f32 = 8.0;
const FLOAT_S: f32 = 10.0;
/// How long past the way up at its dive rate a riser holding jump is given to ease into
/// floating depth, s.
const EASE_S: f32 = 2.0;
/// How long a swimmer swims at the surface, s, and at what pitches: level, a camera's rest a
/// little under level, and just short of the kind's dive.
const SURFACE_S: f32 = 8.0;
const REST_PITCH: f32 = -0.08;
/// The pitch a diver goes down to the bed at, deg, and how long it swims along it, s.
const BOTTOM_PITCH: f32 = -60.0;
const ALONG_S: f32 = 4.0;
/// How long a diver dives in the shallow lake before swimming level up its far shore, s, and
/// the slack on the swim out at its kind's speed.
const EXIT_DIVE_S: f32 = 1.5;
const SLACK: f32 = 1.5;

#[derive(Clone, Copy, Debug)]
enum Case {
    /// A kind afloat, moving forward looking down so many degrees.
    Dive(usize, f32),
    /// A kind deep under, holding jump or not.
    Rise(usize, bool),
    /// A kind afloat swimming forward at a pitch, rad.
    Surface(usize, f32),
    /// A kind afloat, then down to the bed and held there till it drowns, then let go.
    Breath(usize),
    /// A kind afloat a long while, never under.
    Afloat(usize),
    /// A kind diving to the bed and swimming along it.
    Bottom(usize),
    /// A kind diving in the shallow lake and swimming up its shore of a grade and out.
    Exit(usize, f32),
}

fn cases(kinds: &[Kind; 2]) -> Vec<Case> {
    let mut cases = Vec::new();
    for (k, kind) in kinds.iter().enumerate() {
        cases.extend(PITCHES.map(|p| Case::Dive(k, p)));
        cases.extend([Case::Rise(k, true), Case::Rise(k, false)]);
        let short = -(kind.dive_from_deg - 2.0).to_radians();
        cases.extend([0.0, REST_PITCH, short].map(|p| Case::Surface(k, p)));
        cases.extend([Case::Breath(k), Case::Afloat(k), Case::Bottom(k)]);
        cases.extend([12.0, 25.0].map(|g| Case::Exit(k, g)));
    }
    cases
}

fn pool() -> Course {
    let mut course = ground(&POOL);
    course.water = Some(Water {
        min: Vec2::new(POOL[1].0, -200.0),
        max: Vec2::new(POOL[4].0, 200.0),
        surface: SURFACE,
    });
    course
}

/// The pitch a kind strokes along looking at one, deg (D478): none within its
/// `dive_from_deg` of level, and past it stretched so straight down stays straight down.
fn stroke_deg(kind: &Kind, pitch: f32) -> f32 {
    let from = kind.dive_from_deg;
    let past = (pitch.abs() - from).max(0.0);
    (past * 90.0 / (90.0 - from)).copysign(pitch)
}

/// How deep the feet are under the surface, m.
fn under(feet: Vec3) -> f32 {
    SURFACE - local(feet).y
}

/// Whether the mouth is under the surface, by the kind's mouth.
fn mouth_under(kind: &Kind, feet: Vec3) -> bool {
    under(feet) > kind.mouth * kind.height
}

fn afloat(kind: &Kind) -> f32 {
    kind.float_depth * kind.height
}

fn ticks(s: f32) -> usize {
    (s / TICK).round() as usize
}

fn forward(pitch: f32) -> impl Fn(u32, &super::Body) -> Intent {
    move |_, b| Intent {
        wish: forward_at(b.feet),
        pitch,
        ..Intent::default()
    }
}

/// A run with a kind afloat at rest over the pool's middle, or at a depth.
fn start<'a, M: Mover>(
    new: &impl Fn() -> M,
    course: &'a Course,
    kind: Kind,
    at: f32,
) -> Run<'a, M> {
    let mut run = Run::new(new(), course);
    run.add(0, kind, world(START_U, SURFACE - at, 0.0));
    run
}

fn run<M: Mover>(new: &impl Fn() -> M, kinds: &[Kind; 2], case: Case) -> Outcome {
    let course = pool();
    match case {
        Case::Dive(k, deg) => {
            let kind = kinds[k];
            let mut run = start(new, &course, kind, afloat(&kind));
            let stroke = forward(deg.to_radians());
            for _ in 0..ticks(RATE_FROM_S) {
                run.tick(&stroke);
            }
            let from = local(run.body(0).feet);
            for _ in 0..ticks(RATE_S) {
                run.tick(&stroke);
            }
            let to = local(run.body(0).feet);
            let along = stroke_deg(&kind, deg).to_radians();
            let down = (from.y - to.y) / RATE_S;
            let level = (to.x - from.x) / RATE_S;
            let want_down = -kind.dive * libm::sinf(along);
            let want_level = kind.swim * libm::cosf(along);
            let off = ((down - want_down) / kind.dive)
                .abs()
                .max(((level - want_level) / kind.swim).abs());
            run.outcome(vec![off])
        }
        Case::Rise(k, held) => {
            let kind = kinds[k];
            let mut run = start(new, &course, kind, RISE_FROM_M);
            let intent = |_: u32, _: &super::Body| Intent {
                rise: held,
                ..Intent::default()
            };
            let mut rate_off = 0.0;
            let given = if held {
                for _ in 0..ticks(RATE_FROM_S) {
                    run.tick(intent);
                }
                let from = under(run.body(0).feet);
                for _ in 0..ticks(RATE_S) {
                    run.tick(intent);
                }
                let rate = (from - under(run.body(0).feet)) / RATE_S;
                rate_off = ((rate - kind.dive) / kind.dive).abs();
                // The rest of the way at its rate, and its ease into floating depth.
                (RISE_FROM_M - afloat(&kind)) / kind.dive + EASE_S
            } else {
                FLOAT_S
            };
            let mut mouth_under = 0.0_f32;
            let mut bobs = (Bob::default(), Bob::default());
            let total = ticks(given);
            let done = ticks(if held { RATE_FROM_S + RATE_S } else { 0.0 });
            for t in done..total {
                run.tick(intent);
                let feet = run.body(0).feet;
                // Let alone, its last four seconds afloat: the mouth out, the bob not growing.
                if !held && t + ticks(4.0) >= total {
                    mouth_under = mouth_under.max(under(feet) - kind.mouth * kind.height);
                    let late = t + ticks(2.0) >= total;
                    if late { &mut bobs.1 } else { &mut bobs.0 }.keep(under(feet));
                }
            }
            let off = (under(run.body(0).feet) - afloat(&kind)).abs();
            let surface = mouth_under.max(0.0) + (bobs.1.size() - bobs.0.size()).max(0.0);
            run.outcome(vec![rate_off, off, surface])
        }
        Case::Surface(k, pitch) => {
            let kind = kinds[k];
            let mut run = start(new, &course, kind, afloat(&kind));
            let mut worst = 0.0_f32;
            let mut bobs = (Bob::default(), Bob::default());
            let total = ticks(SURFACE_S);
            for t in 0..total {
                run.tick(forward(pitch));
                let feet = run.body(0).feet;
                worst = worst.max(under(feet) - kind.mouth * kind.height);
                let late = t + ticks(SURFACE_S * 0.5) >= total;
                if late { &mut bobs.1 } else { &mut bobs.0 }.keep(under(feet));
            }
            let grew = (bobs.1.size() - bobs.0.size()).max(0.0);
            run.outcome(vec![worst.max(0.0) + grew])
        }
        Case::Breath(k) => breath(new, &course, kinds[k]),
        Case::Afloat(k) => {
            let kind = kinds[k];
            let mut run = start(new, &course, kind, afloat(&kind));
            let mut used = 0.0_f32;
            for _ in 0..ticks(kind.breath_s * 1.5) {
                run.tick(|_, _| Intent::default());
                used = used.max(kind.breath_s - run.mover().breath(0));
            }
            run.outcome(vec![used / kind.breath_s])
        }
        Case::Bottom(k) => {
            let kind = kinds[k];
            let mut run = start(new, &course, kind, afloat(&kind));
            let along = stroke_deg(&kind, BOTTOM_PITCH).to_radians();
            let down = -kind.dive * libm::sinf(along);
            let to_bed = (SURFACE - BED - afloat(&kind)) / down + 1.0;
            let stroke = forward(BOTTOM_PITCH.to_radians());
            for _ in 0..ticks(to_bed) {
                run.tick(&stroke);
            }
            let from = local(run.body(0).feet);
            let mut deepest = 0.0_f32;
            for _ in 0..ticks(ALONG_S) {
                run.tick(&stroke);
                deepest = deepest.max(BED - local(run.body(0).feet).y);
            }
            let to = local(run.body(0).feet);
            let want = kind.swim * libm::cosf(along) * ALONG_S;
            // On the bed it is, and it went along it at its stroke's level speed.
            let off_bed = (to.y - BED - kind.padding_m).max(0.0).max(deepest);
            let short = (want - (to.x - from.x)).max(0.0);
            run.outcome(vec![off_bed.max(short)])
        }
        Case::Exit(k, grade) => {
            let kind = kinds[k];
            let (lake, dry_u) = lake(grade);
            let middle = dry_u - 15.0;
            let mut run = Run::new(new(), &lake);
            run.add(0, kind, world(middle, SURFACE - afloat(&kind), 0.0));
            let dive = forward(BOTTOM_PITCH.to_radians());
            for _ in 0..ticks(EXIT_DIVE_S) {
                run.tick(&dive);
            }
            let left = dry_u + 0.5 - local(run.body(0).feet).x;
            for _ in 0..ticks(left / kind.swim * SLACK) {
                run.tick(forward(0.0));
            }
            let short = (dry_u + 0.5 - local(run.body(0).feet).x).max(0.0);
            run.outcome(vec![short])
        }
    }
}

/// The span of a bob: the deepest and shallowest it went.
#[derive(Default)]
struct Bob(Option<(f32, f32)>);

impl Bob {
    fn keep(&mut self, depth: f32) {
        let (lo, hi) = self.0.unwrap_or((depth, depth));
        self.0 = Some((lo.min(depth), hi.max(depth)));
    }

    fn size(&self) -> f32 {
        self.0.map_or(0.0, |(lo, hi)| hi - lo)
    }
}

/// A kind dived to the bed and held there till it drowns, then let go: its breath empties
/// `breath_s` after its mouth went under, it is dead `drown_s` after, and, its mouth out again,
/// it is back at its kind's rate (full from none in `breath_back_s` of time out, what a bob
/// under the surface spends taken back too); breath falls only with its mouth under and never
/// rises there. The worst share any span is off by, and a tick's worth for each tick its
/// breath went the wrong way.
fn breath<M: Mover>(new: &impl Fn() -> M, course: &Course, kind: Kind) -> Outcome {
    let mut run = start(new, course, kind, afloat(&kind));
    let (mut went_under, mut empty, mut dead) = (None, None, None);
    // Since it drowned: how long its mouth was out and under while its breath came back, and
    // whether it came back full.
    let (mut out_s, mut under_s, mut full) = (0.0, 0.0, false);
    let mut harm = 0.0;
    let mut wrong = 0;
    let mut was = run.mover().breath(0);
    let (down, up) = (
        forward(-std::f32::consts::FRAC_PI_2),
        |_: u32, _: &super::Body| Intent::default(),
    );
    let total = ticks(
        (SURFACE - BED) / kind.dive + kind.breath_s + kind.drown_s + kind.breath_back_s + 12.0,
    );
    for t in 0..total {
        let s = (t + 1) as f32 * TICK;
        if dead.is_none() {
            run.tick(&down);
        } else {
            run.tick(up);
        }
        let feet = run.body(0).feet;
        let now = run.mover().breath(0);
        let under = mouth_under(&kind, feet);
        if (now < was && !under) || (now > was && under) {
            wrong += 1;
        }
        was = now;
        if under && went_under.is_none() {
            went_under = Some(s - TICK);
        }
        if now <= 0.0 && empty.is_none() {
            empty = Some(s);
        }
        harm += run.mover().drown(0);
        if harm >= 1.0 - 1.0e-4 && dead.is_none() {
            dead = Some(s);
        } else if dead.is_some() && !full && (out_s > 0.0 || !under) {
            if under {
                under_s += TICK;
            } else {
                out_s += TICK;
            }
            full = now >= kind.breath_s;
        }
    }
    let span = |a: Option<f32>, b: Option<f32>, want: f32| match (a, b) {
        (Some(a), Some(b)) => ((b - a - want) / want).abs(),
        _ => 1.0,
    };
    let back = kind.breath_back_s * (1.0 + under_s / kind.breath_s);
    let refilled = if full {
        ((out_s - back) / back).abs()
    } else {
        1.0
    };
    let off = span(went_under, empty, kind.breath_s)
        .max(span(empty, dead, kind.drown_s))
        .max(refilled);
    run.outcome(vec![off + wrong as f32 * TICK])
}

/// The diving measures, and the deepest any body went into the ground.
pub(super) fn score<M: Mover>(new: &(impl Fn() -> M + Sync)) -> (Vec<Measure>, f32) {
    let man = Kind::man();
    let kinds = [man, second(&man)];
    let cases = cases(&kinds);
    let outcomes = terra_gen::par::map(&cases, |case| run(new, &kinds, *case));
    let all = || cases.iter().zip(&outcomes);
    let worst = |want: fn(&Case) -> bool, i: usize| {
        all()
            .filter(|(case, _)| want(case))
            .map(|(_, o)| o.values[i])
            .fold(0.0, f32::max)
    };
    let held = |c: &Case| matches!(c, Case::Rise(_, true));
    let rate = worst(|c| matches!(c, Case::Dive(..)), 0).max(worst(held, 0));
    let surface = worst(|c| matches!(c, Case::Surface(..)), 0)
        .max(worst(|c| matches!(c, Case::Rise(_, false)), 2));
    let breath =
        worst(|c| matches!(c, Case::Breath(_)), 0).max(worst(|c| matches!(c, Case::Afloat(_)), 0));
    let deepest = outcomes
        .iter()
        .flat_map(|o| o.depths.iter().copied())
        .fold(0.0, f32::max);
    let m = |id, value: f32| Measure { id, value };
    let measures = vec![
        m("swim.dive", rate),
        m("swim.rise", worst(|c| matches!(c, Case::Rise(..)), 1)),
        m("swim.surface", surface),
        m("swim.breath", breath),
        m("swim.bottom", worst(|c| matches!(c, Case::Bottom(_)), 0)),
        m("swim.exit", worst(|c| matches!(c, Case::Exit(..)), 0)),
    ];
    (measures, deepest)
}
