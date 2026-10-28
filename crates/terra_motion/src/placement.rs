//! Foot placement (docs/movement.md stage 4, the fix-up layer; D455), raising feet onto the
//! ground under them: a clip strides on flat ground at its root, so on a step or a slope the
//! ground under a foot is higher than the root and the foot would sink into it. As Unreal's
//! Foot Placement node and ALS place feet (their documentation; no code taken): a foot is
//! probed once when it is locked, the ground found under its heel (the ankle) and its ball
//! frozen in the world for the lock's life, so a planted foot never shifts with what a probe
//! sees later (the next tread, the body moving over it); a swinging foot is probed under its
//! heel and ball once a frame, so its toe clears a tread ahead; the foot is pitched by
//! the heel's and ball's ground and rolled by the ground's normal (within `TILT_DEG`), turned
//! about its ball so a locked ball stays put, and raised until neither heel nor ball is under
//! its ground (a rigid sole rests on the highest ground under it: a foot locked across a
//! riser stands on the upper tread, its heel over the edge), each eased in the world (the body
//! stepping a tread moves no foot's ground), a raise lagging no more than half the clip's
//! clearance (a swinging foot touches its ground only as the clip's comes down). A foot over lower
//! ground (a tread's lower side, a slope down) is lowered onto it, and the pelvis with it: as
//! ALS's and Unity's foot IK scripts offset the pelvis, it drops by the lower of the two feet's
//! needed offsets (every foot's, swinging or planted: weighting it by the lock bobs it), eased by
//! a critically damped spring as Unreal's Foot Placement node springs its pelvis, and in the
//! world, so a body stepping down onto the ground the pelvis already went to does not dip
//! again. As Unreal's node bounds the pelvis by the legs' extension (its Min/MaxExtensionRatio),
//! the drop is capped where a planted leg would fold past `FOLD_RATIO` of its length, and a foot
//! whose ground is lower than that (a real drop) is left out: it is lowered only as far as its
//! own leg reaches, and stays short. A foot is never put farther from the lowered hip than the
//! clip has it or `REACH_RATIO` of its leg, so lowering never strains a leg. The leg is then
//! bent to the foot by D455's two-bone solve (`feet::two_bone`).
//!
//! Engine-free: places are world positions, so it tests with a field and no skeleton; the game
//! probes the walk's own world and writes the legs (`terra_walker::feet`).

use crate::feet::Pose;
use crate::mover::{Capsule, Query};
use glam::{DVec3, Quat, Vec3};

/// The probe starts this far over the body's ground (where the mover stands it, over the
/// figure's root by a step's ease) and goes this far under the root's level, m: a step up as
/// high as a body steps (a man's 0.55 m, movers.ron) and the ankle over it.
pub const ABOVE_M: f32 = 0.7;
pub const BELOW_M: f32 = 0.6;
/// The probe's radius, m: a foot's narrowest, so a crack a voxel's tenth wide is no hole.
pub const PROBE_M: f32 = 0.02;
/// The line read down from the probe's sphere where it stopped (`straight_under`) is a sphere
/// this thin, m.
pub const LINE_M: f32 = 0.001;
/// A face the probe touches steeper than this, deg, is read at the height it touched, not
/// where its plane crosses the probe's line (`straight_under`): a riser's or a wall's.
pub const STEEPEST_DEG: f32 = 75.0;
/// A foot pitches and rolls to its ground no further than this, deg.
pub const TILT_DEG: f32 = 30.0;
/// A foot's ground is eased to with this time constant, s (ALS eases its foot offsets at 15-30
/// a second).
pub const EASE_S: f32 = 0.04;
/// A planted ball of the foot stands about this high over its ground, m (a man's clips: 4-5
/// cm): a foot higher than this is clear of the ground, and its raise may lag.
pub const BALL_REST_M: f32 = 0.04;
/// A foot's raise lags its ground by no more than this share of how far the clip has its ball
/// clear of its rest: a swinging foot over a higher tread stays clear of it until the clip's
/// foot comes down, so it never touches the ground a frame before its lock.
pub const LAG_SHARE: f32 = 0.5;
/// Raised by less than this and tilted by less than `STILL_DEG`, the foot is left as the clip
/// has it (flat ground's numbers do not move a leg).
pub const STILL_M: f32 = 0.001;
pub const STILL_DEG: f32 = 0.5;
/// The pelvis's spring, rad/s: critically damped, so it settles without overshoot, 95% of the
/// way in 4.7 / this seconds (a tenth of a second: a stride's stance, as Unreal's Foot Placement
/// springs its pelvis stiffly enough to keep up with a run).
pub const PELVIS_HZ: f32 = 45.0;
/// A lowered foot's leg reaches no farther from its hip than this share of its length (or the
/// clip's own reach, if more): nearly straight, never past (Unreal's MaxExtensionRatio).
pub const REACH_RATIO: f32 = 0.98;
/// The pelvis drops no further than folds a planted leg's hip-to-ankle to this share of its
/// length (Unreal's MinExtensionRatio): ground lower than that is a drop, not a step.
pub const FOLD_RATIO: f32 = 0.4;

/// The ground under a point: how high it is over the root's level, m, and its normal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Under {
    pub height: f32,
    pub normal: Vec3,
    /// The normal of the ground straight under the point, where a line was read down to it
    /// (`straight_under`): the sphere's `normal` is where it touched, which short of a face's
    /// foot is the face's over level ground. None where the height is the sphere's own (level
    /// ground, a face steeper than `STEEPEST_DEG`, a line that met nothing between).
    pub straight: Option<Vec3>,
}

/// The ground under `at` (a world point), probed by `ground`, the mover's own query: a sphere
/// of `PROBE_M` swept down along `up` from `ABOVE_M` over the body's ground (`body`, how far it
/// stands over `root`'s level: a figure shown under its body while it eases a step up) to
/// `BELOW_M` under `root`'s level, at `at`'s place across. None where nothing is met (a drop),
/// or where the probe starts inside the ground (a face higher than a step). Started over the
/// root rather than the body, a figure eased half a metre under its body up a stair started a
/// swinging foot's probe inside the next tread and sank the foot into it.
pub fn probe<Q: Query + ?Sized>(
    ground: &Q,
    at: DVec3,
    root: DVec3,
    up: Vec3,
    body: f32,
) -> Option<Under> {
    let level = at - up.as_dvec3() * (at - root).dot(up.as_dvec3());
    let above = ABOVE_M + body.max(0.0);
    let top = up * (above + PROBE_M);
    let sphere = Capsule {
        a: top,
        b: top,
        radius: PROBE_M,
        origin: level,
    };
    let hit = ground.sweep(&sphere, -up * (above + BELOW_M))?;
    // Met where it started: it started in the ground.
    if hit.fraction <= 0.0 || hit.contact.at.dot(up) > above {
        return None;
    }
    let normal = hit.contact.normal.normalize_or(up);
    let centre = top - up * ((above + BELOW_M) * hit.fraction);
    let (height, straight) = straight_under(ground, &sphere, centre, hit.contact.at, normal, up);
    Some(Under {
        height,
        normal,
        straight,
    })
}

/// How high the ground is straight under the probe's line (through the sphere's origin along
/// `up`), the sphere stopped with its centre at `centre` touching the ground at `at` (both over
/// the root's level) with `normal` there. On a slope the sphere touches uphill of its centre,
/// higher than the ground under the foot's point by the radius's run up the slope (2.7 cm on
/// the yard's lump's 58 deg foot), so, as Unreal's character movement reads the floor's
/// distance under its sweep's centre by a line trace after the sweep (`ComputeFloorDist`, its
/// documentation; no code taken), a line goes down from the centre: its ground is taken
/// between where the touched plane crosses the line (the lowest a plane under the sphere
/// leaves it; a crack narrower than the sphere is no hole) and where it touched (the highest
/// the ground under it can be, as the sphere came down clear of it). Within `STILL_M` of each
/// other (level ground), or on a face steeper than `STEEPEST_DEG`, it is read where touched.
/// With it, the normal the line met, where it met the ground between (`Under::straight`).
fn straight_under<Q: Query + ?Sized>(
    ground: &Q,
    sphere: &Capsule,
    centre: Vec3,
    at: Vec3,
    normal: Vec3,
    up: Vec3,
) -> (f32, Option<Vec3>) {
    let (rise, highest) = (normal.dot(up), at.dot(up));
    if rise < libm::cosf(STEEPEST_DEG.to_radians()) {
        return (highest, None);
    }
    let plane = normal.dot(at) / rise;
    if plane >= highest - STILL_M {
        return (highest, None);
    }
    let line = Capsule {
        a: centre,
        b: centre,
        radius: LINE_M,
        ..*sphere
    };
    let down = centre.dot(up) - plane + PROBE_M;
    match ground.sweep(&line, -up * down) {
        Some(hit) if hit.fraction > 0.0 => {
            let met = hit.contact.at.dot(up);
            let normal = hit.contact.normal.normalize_or(up);
            let between = met > plane - STILL_M && met < highest + STILL_M;
            (met.clamp(plane, highest), between.then_some(normal))
        }
        _ => (plane, None),
    }
}

/// The ground a foot is placed on, in the world: the points under its heel and its ball, and
/// the normal it rolls to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ground {
    pub heel: Vec3,
    pub ball: Vec3,
    pub normal: Vec3,
    /// The ground found under heel and ball themselves: where a sole straddling a riser is
    /// put on the higher tread (`heel`, `ball`), its point over the edge has lower ground under
    /// it, and does not rest there (a lock's start reads it, `Grounded::over`).
    pub found: [Vec3; 2],
    /// How far the foot is moved across the up to keep it off a face steeper than it stands on
    /// (`off_face`): its toe at the face, not in it.
    pub back: Vec3,
}

/// One foot's placing: the ground frozen for its lock, and the ground eased to.
#[derive(Clone, Copy, Debug, Default)]
struct Placed {
    /// The lock (its ankle where it was locked) the frozen ground is of.
    lock: Option<Vec3>,
    frozen: Option<Ground>,
    /// The heel's and ball's ground over the root's level and its normal as eased, and the
    /// root they were over.
    eased: Option<((f32, f32, Vec3), Vec3)>,
    /// The foot's move off a face (`Ground::back`), as eased.
    back: Vec3,
}

impl Placed {
    /// A foot locked with its ankle `at`: its ground probed once at its lock (`probe`) and
    /// frozen for the lock, and whether it came off a riser it straddled this frame. Locked
    /// straddling a riser, it is probed again while it straddles (as ALS traces a planted foot
    /// each frame): once the point it had on the higher tread has left it (a toe-off rolling the
    /// heel over a step down's edge), the sole is set on the ground now under it, not left over
    /// the lower tread at the higher's height.
    fn locked(
        &mut self,
        at: Vec3,
        up: Vec3,
        mut probe: impl FnMut() -> Option<Ground>,
    ) -> (Option<Ground>, bool) {
        let straddles = |ground: &Ground| riser(ground.found[0], ground.found[1], up);
        if self.lock != Some(at) {
            self.lock = Some(at);
            self.frozen = probe();
            return (self.frozen, false);
        }
        if self.frozen.as_ref().is_some_and(straddles) {
            let now = probe();
            if now.as_ref().is_some_and(|ground| !straddles(ground)) {
                self.frozen = now;
                return (now, true);
            }
        }
        (self.frozen, false)
    }
}

/// The pelvis's drop as eased: how far along the up, how fast, and the root it was over.
#[derive(Clone, Copy, Debug, Default)]
struct Pelvis {
    drop: f32,
    speed: f32,
    root: Option<Vec3>,
}

/// A body's two feet and its pelvis placed on the ground under them, frame by frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct Placing {
    feet: [Placed; 2],
    pelvis: Pelvis,
}

/// A foot's ground this frame, before the pelvis is lowered (`Placing::ground`): it asks the
/// pelvis for its `need`, and is placed once the pelvis's drop is known (`Placing::pelvis`,
/// `Grounded::placed`).
#[derive(Clone, Copy, Debug)]
pub struct Grounded {
    shown: Pose,
    ball_at: Vec3,
    /// The heel's and ball's ground over the root's level and the normal stood to, as eased.
    heights: (f32, f32, Vec3),
    /// The ground under heel and ball themselves over the root's level as probed this frame
    /// (frozen for a lock), before easing and before a riser puts the sole on its higher tread:
    /// what a lock's start reads (`over`).
    probed: (f32, f32),
    up: Vec3,
    hip: Vec3,
    reach: f32,
    extent: Option<f32>,
    locked: bool,
    off: bool,
    root: Vec3,
    refused: bool,
}

/// What `Placing::ground` reads of a foot this frame.
#[derive(Clone, Copy, Debug)]
pub struct Foot {
    /// The foot as the lock shows it (`feet::Foot::update`), at the clip's height.
    pub shown: Pose,
    /// The ball of the foot in the foot's own frame.
    pub ball: Vec3,
    /// Where it was locked, while it is (`feet::Foot::locked`).
    pub lock: Option<Pose>,
    /// The clip's flat ground: the figure's root as shown.
    pub root: Vec3,
    pub up: Vec3,
    /// Feet not kept this frame (a body down, aloft): it goes back to the clip.
    pub off: bool,
    /// The leg's hip, in the world, before the pelvis is lowered: the foot is never put farther
    /// from it than the clip has it, or than `REACH_RATIO` of the leg.
    pub hip: Vec3,
    /// The leg's length, hip to knee to ankle, m.
    pub reach: f32,
    /// A warped stride's (`terra_walker::pace::gait`): how far the clip has its ankle from the
    /// hip, m, unwarped. A locked foot asks the pelvis down until its leg reaches it no farther
    /// (`Grounded::need`), as Unreal's Stride Warping pulls the pelvis down toward its warped
    /// feet so a leg is not stretched past its clip's.
    pub extent: Option<f32>,
    /// For a lock new this frame (`feet::Foot::fresh`), where the foot rests: its heel and ball
    /// over the clip's flat ground while locked. The lock is refused (`Grounded::refused`) when
    /// its own probe finds the sole's lower point farther over its ground than `feet::PLANT_M`
    /// past that: the lock's gate read last frame's ground, and a sole that straddled an edge
    /// then (its heel on the higher tread) may be wholly over the lower one now.
    pub fresh: Option<[f32; 2]>,
    /// The steepest ground the foot stands on, deg: the body's kind's slope limit
    /// (`mover::Kind::slope_limit_deg`), as Unreal's foot placement honours its walkable floor
    /// angle. A face steeper than this the foot is never raised onto (`off_face`).
    pub walkable_deg: f32,
}

impl Placing {
    /// Foot `side` placed on the ground under it with the pelvis where the clip has it (one
    /// foot alone: `ground`, then `Grounded::placed`).
    pub fn foot(
        &mut self,
        side: usize,
        foot: Foot,
        dt: f32,
        under: impl FnMut(Vec3) -> Option<Under>,
    ) -> Pose {
        self.ground(side, foot, dt, under).placed(0.0)
    }

    /// The pelvis's drop this frame, m along the up: the lower foot's need (`Grounded::need`,
    /// ALS's and Unity's pelvis offset), a foot whose need is past what a planted leg folds to
    /// (`Grounded::fold`) left out; nothing while a foot is let go. Eased by a critically damped
    /// spring (`PELVIS_HZ`), and in the world: the root come down by some of the drop (a body
    /// stepping down onto ground the pelvis already went to) takes that much of it, so the
    /// pelvis does not dip again. A root going up adds nothing: the figure eases its own steps
    /// up (`Stepping`).
    pub fn pelvis(&mut self, feet: [&Grounded; 2], root: Vec3, dt: f32) -> f32 {
        let up = feet[0].up;
        let cap = feet
            .iter()
            .map(|foot| foot.fold())
            .fold(f32::INFINITY, f32::min);
        let to = if feet.iter().any(|foot| foot.off) {
            0.0
        } else {
            feet.iter()
                .map(|foot| foot.need())
                .filter(|&need| need <= cap)
                .fold(0.0, f32::max)
                // A warped stride's planted leg reached (`Grounded::stretch`), no lower than a
                // planted leg folds.
                .max(
                    feet.iter()
                        .map(|foot| foot.stretch())
                        .fold(0.0, f32::max)
                        .min(cap),
                )
        };
        let pelvis = &mut self.pelvis;
        if let Some(was) = pelvis.root {
            pelvis.drop = (pelvis.drop + (root - was).dot(up)).clamp(0.0, pelvis.drop);
        }
        pelvis.root = Some(root);
        // A critically damped spring stepped exactly (x'' = -2wx' - w^2 x).
        let (w, dt) = (PELVIS_HZ, dt.max(0.0));
        let (x, v) = (pelvis.drop - to, pelvis.speed);
        let (e, j) = (libm::expf(-w * dt), v + w * x);
        pelvis.drop = to + (x + j * dt) * e;
        pelvis.speed = (v - w * j * dt) * e;
        if pelvis.drop <= 0.0 {
            (pelvis.drop, pelvis.speed) = (0.0, 0.0);
        }
        pelvis.drop
    }

    /// Foot `side`'s ground this frame, `dt` seconds on: `under` probes the ground under a
    /// world point (`probe`). A locked foot is probed under its heel and ball once, at its lock,
    /// and keeps that ground while it is locked; a free, stepping or released foot once a frame
    /// under its heel and ball.
    pub fn ground(
        &mut self,
        side: usize,
        foot: Foot,
        dt: f32,
        mut under: impl FnMut(Vec3) -> Option<Under>,
    ) -> Grounded {
        let Foot {
            shown,
            ball,
            lock,
            root,
            up,
            off,
            hip,
            reach,
            extent,
            fresh,
            walkable_deg,
        } = foot;
        let drop = self.pelvis.drop;
        let placed = &mut self.feet[side];
        let ball_at = shown.at + shown.turn * ball;
        let walkable = libm::cosf(walkable_deg.to_radians());
        let mut sole_ground = || sole_ground([shown.at, ball_at], root, up, walkable, &mut under);
        let (target, stepped_off) = match lock {
            _ if off => (None, false),
            Some(lock) => placed.locked(lock.at, up, sole_ground),
            None => {
                placed.lock = None;
                (sole_ground(), false)
            }
        };
        // Heights over the root's level now; no ground found, or the foot let go: the clip's own
        // flat ground.
        let over = |at: Vec3| (at - root).dot(up);
        let probed = target.map_or((0.0, 0.0), |ground| {
            (over(ground.found[0]), over(ground.found[1]))
        });
        let back_to = target.map_or(Vec3::ZERO, |ground| ground.back);
        let target = target.map_or((0.0, 0.0, up), |ground| {
            (over(ground.heel), over(ground.ball), ground.normal)
        });
        // Eased as ALS eases its foot offsets, in the world: the root moving (the body stepping
        // up or down a tread, its figure easing the step) shifts neither a locked foot's frozen
        // ground nor a swinging foot's. A foot let go (aloft) eases back to the clip over the
        // root, so a jump's rise does not leave it behind.
        let k = 1.0 - libm::expf(-dt.max(0.0) / EASE_S);
        let was = placed.eased;
        let ease_for = |locked: bool| match was {
            // Come off the edge it straddled, it is on the ground now under it at once: it
            // was over it, the edge holding it up, and does not float down.
            _ if stepped_off => target,
            None => target,
            Some(((heel, ball, normal), was_root)) => {
                let moved = if off { 0.0 } else { (root - was_root).dot(up) };
                let (heel, ball) = (heel - moved, ball - moved);
                // Raised behind its ground no further than `LAG_SHARE` of the clip's foot's
                // clearance over its flat ground (its ball over its rest): a foot is lifted
                // out of the ground at once (a lock across a riser), and a swinging foot over a
                // higher tread is kept clear of it, coming down onto it only as the clip's
                // foot comes down, when the lock takes it. A locked foot's raise does not lag:
                // the lag is for a swinging foot, and the clip has a locked one on the ground,
                // so it is out of its ground at once (as any foot under its ground is), never
                // under the clip's height over it (a heel strike onto a higher tread).
                let clear = if locked {
                    0.0
                } else {
                    ((ball_at - root).dot(up) - BALL_REST_M).max(0.0) * LAG_SHARE
                };
                let ease = |was: f32, to: f32| (was + (to - was) * k).max(to - clear);
                (
                    ease(heel, target.0),
                    ease(ball, target.1),
                    normal.lerp(target.2, k).normalize_or(up),
                )
            }
        };
        // Kept off a face (`off_face`), moved across the up, eased as its ground is.
        let back = match was {
            Some(_) if !stepped_off => placed.back + (back_to - placed.back) * k,
            _ => back_to,
        };
        placed.back = back;
        let mut grounded = Grounded {
            shown: Pose {
                at: shown.at + back,
                ..shown
            },
            ball_at: ball_at + back,
            heights: ease_for(lock.is_some()),
            probed,
            up,
            hip,
            reach,
            extent,
            locked: lock.is_some(),
            off,
            root,
            refused: false,
        };
        // Refused, a new lock is placed as a free foot on the same probe (`Grounded::aloft`).
        if let Some(rest) = fresh.filter(|_| lock.is_some() && !off)
            && grounded.aloft(rest, drop)
        {
            placed.lock = None;
            grounded.locked = false;
            grounded.heights = ease_for(false);
            grounded.refused = true;
        }
        placed.eased = Some((grounded.heights, root));
        grounded
    }
}

impl Grounded {
    /// Whether a new lock's foot, placed with the pelvis lowered by `drop` (last frame's), is
    /// not down on this frame's own probe, as Unreal's Foot Placement plants a foot only within
    /// DistanceToGround of the trace it makes that frame: its sole's lower point farther over
    /// its ground than `feet::PLANT_M` past where it rests (`rest`), or its ball hanging
    /// (`feet::hangs`). The lock's gate read the ground found last frame, and a sole that then
    /// straddled a step down's edge, its heel on the higher tread, may now be wholly over the
    /// lower one at the higher's height; locked there it would be eased down while locked.
    fn aloft(&self, rest: [f32; 2], drop: f32) -> bool {
        let over = self.over(self.placed(drop));
        let height = |at: Vec3| (at - self.root).dot(self.up);
        let sole = [height(self.shown.at), height(self.ball_at)];
        let aloft = (0..2)
            .map(|i| sole[i] + over[i] - rest[i])
            .fold(f32::INFINITY, f32::min);
        aloft > crate::feet::PLANT_M || crate::feet::hangs(over)
    }

    /// A lock new this frame refused (`Foot::fresh`): the foot is placed as a free foot, and
    /// the lock is to be let go (`feet::Foot::refuse`).
    pub fn refused(&self) -> bool {
        self.refused
    }

    /// How far under the clip's flat ground this foot's ground is (the higher of its heel's and
    /// ball's: a sole rests on the highest under it), m: the drop it asks of the pelvis.
    pub fn need(&self) -> f32 {
        if self.off {
            return 0.0;
        }
        (-self.heights.0.max(self.heights.1)).max(0.0)
    }

    /// How far the hip must come down for a locked foot of a warped stride, on its ground, to
    /// be reached no farther than the clip's leg reaches its own (`Foot::extent`), m, as
    /// Unreal's Stride Warping pulls the pelvis down toward its warped feet: nothing unwarped
    /// on flat ground, a lower ground's depth (`need`) where the foot is where its clip has it.
    pub fn stretch(&self) -> f32 {
        let Some(extent) = self.extent.filter(|_| self.locked && !self.off) else {
            return 0.0;
        };
        let lift = self.heights.0.max(self.heights.1);
        let from = self.hip - (self.shown.at + self.up * lift);
        let (high, across) = (
            from.dot(self.up),
            from.reject_from(self.up).length_squared(),
        );
        (high - (extent * extent - across).max(0.0).sqrt()).max(0.0)
    }

    /// How far the pelvis may drop before this leg, its foot on its ground, folds its hip to
    /// ankle to `FOLD_RATIO` of its length: none for a foot not locked (a swinging foot is not
    /// stood on).
    fn fold(&self) -> f32 {
        if !self.locked || self.off {
            return f32::INFINITY;
        }
        let lift = self.heights.0.max(self.heights.1);
        let from = self.hip - (self.shown.at + self.up * lift);
        let (high, across) = (
            from.dot(self.up),
            from.reject_from(self.up).length_squared(),
        );
        let least = FOLD_RATIO * self.reach;
        (high - (least * least - across).max(0.0).sqrt()).max(0.0)
    }

    /// The foot placed on its ground with the pelvis lowered by `drop` (`raised`).
    pub fn placed(&self, drop: f32) -> Pose {
        raised(self, drop)
    }

    /// How far the foot placed at `placed` stands over its ground past where the clip has it
    /// over its flat ground, heel (the ankle) and ball, m: what placement could not lower (a
    /// foot over lower ground the pelvis has not yet come down to, or still easing down to
    /// ground it has just come over); never under, as a raise that lags is put on its ground
    /// once locked; 0 for a foot let go. A lock's start reads it (`feet::PLANT_M`), so it is
    /// over the ground as probed, not as eased: as Unreal's Foot Placement plants by the
    /// trace's distance to the ground, a foot swung off a higher face whose eased ground still
    /// holds it up is not down, nor is one whose raise onto a face lags under it, nor a heel
    /// strike over the edge of a riser whose ball is still up over the higher tread.
    pub fn over(&self, placed: Pose) -> [f32; 2] {
        if self.off {
            return [0.0; 2];
        }
        let ball = self.shown.turn.inverse() * (self.ball_at - self.shown.at);
        let height = |at: Vec3| (at - self.root).dot(self.up);
        [
            (height(placed.at) - self.probed.0 - height(self.shown.at)).max(0.0),
            (height(placed.at + placed.turn * ball) - self.probed.1 - height(self.ball_at))
                .max(0.0),
        ]
    }
}

/// The ground under a foot from one point of it, `at` with its `normal`: its other point
/// `reach` away across, on the plane the normal gives.
fn plane(at: Vec3, normal: Vec3, reach: Vec3, up: Vec3) -> Ground {
    let across = reach.reject_from(up);
    let rise = if normal.dot(up) > 1e-3 {
        -normal.dot(across) / normal.dot(up)
    } else {
        0.0
    };
    let ball = at + across + up * rise;
    Ground {
        heel: at,
        ball,
        normal,
        found: [at, ball],
        back: Vec3::ZERO,
    }
}

/// The ground under a sole's heel (the ankle) and ball (`at`, world points), probed by `under`,
/// as a rigid sole stands on it: off a face steeper than walkable (`off_face`), on the higher
/// tread of a riser it straddles, else pitched between them.
fn sole_ground(
    at: [Vec3; 2],
    root: Vec3,
    up: Vec3,
    walkable: f32,
    under: &mut impl FnMut(Vec3) -> Option<Under>,
) -> Option<Ground> {
    let [heel_at, ball_at] = at;
    let mut found = |at: Vec3| {
        let level = at - up * (at - root).dot(up);
        under(at).map(|under| (level + up * under.height, under.normal, under.straight))
    };
    let (heel, toe) = (found(heel_at), found(ball_at));
    if let (Some(heel), Some(toe)) = (heel, toe)
        && let Some(ground) = off_face(heel, toe, at, root, up, walkable)
    {
        return Some(ground);
    }
    let (heel, toe) = (
        heel.map(|(at, n, _)| (at, n)),
        toe.map(|(at, n, _)| (at, n)),
    );
    match (heel, toe) {
        // A riser between them (heel and ball grounds steeper apart than the sole pitches
        // to): the sole rests on the higher tread, flat on it as the tread is, its other point
        // over the edge, as ALS aligns a foot to the normal its trace meets rather than
        // pitching it down toward the lower tread.
        (Some(heel), Some(toe)) if riser(heel.0, toe.0, up) => {
            let found = [heel.0, toe.0];
            if (toe.0 - heel.0).dot(up) > 0.0 {
                let ground = plane(toe.0, toe.1, heel_at - ball_at, up);
                Some(Ground {
                    heel: ground.ball,
                    ball: ground.heel,
                    normal: toe.1,
                    found,
                    back: Vec3::ZERO,
                })
            } else {
                Some(Ground {
                    found,
                    ..plane(heel.0, heel.1, ball_at - heel_at, up)
                })
            }
        }
        (Some(heel), Some(toe)) => Some(Ground {
            heel: heel.0,
            ball: toe.0,
            normal: sole(heel.0, toe.0, (heel.1 + toe.1).normalize_or(up), up),
            found: [heel.0, toe.0],
            back: Vec3::ZERO,
        }),
        (Some((at, normal)), None) => Some(plane(at, normal, ball_at - heel_at, up)),
        (None, Some((at, normal))) => {
            let heel = plane(at, normal, heel_at - ball_at, up);
            Some(Ground {
                heel: heel.ball,
                ball: heel.heel,
                normal,
                found: [heel.ball, heel.heel],
                back: Vec3::ZERO,
            })
        }
        (None, None) => None,
    }
}

/// The toes reach this far past the ball of the foot, m: a foot kept off a face stops with its
/// ball so far short of the face's foot (`off_face`).
pub const TOE_M: f32 = 0.05;

/// A foot is moved off a face (`off_face`) no farther than this, m.
pub const BACK_MOST_M: f32 = 0.35;

/// The ground a sole stands on when one of its points, heel or ball (`heel`, `ball`: the ground
/// found under each, its normal where touched and straight under it), is over a face steeper
/// than walkable (`walkable`, the cosine of the slope limit) and higher than the other: as
/// Unreal's foot placement takes no floor its character could not stand on, the sole stands on
/// the lower ground, pitched by that ground's own normal, moved back across the up
/// (`Ground::back`, from `at`, the heel's and ball's places) until the point over the face is
/// `TOE_M` short of the face's foot, the crease where its plane meets the ground's: its toe at
/// the face, never raised up it or into it. The lower point's ground is its own where it is
/// walkable; on the face's foot too (steep, or touched steep and read where touched), it is the
/// body's own floor, the clip's flat ground at `root`. None where no point is over such a face
/// higher than the other.
fn off_face(
    heel: (Vec3, Vec3, Option<Vec3>),
    ball: (Vec3, Vec3, Option<Vec3>),
    at: [Vec3; 2],
    root: Vec3,
    up: Vec3,
    walkable: f32,
) -> Option<Ground> {
    let steep = |point: &(Vec3, Vec3, Option<Vec3>)| point.2.is_some_and(|n| n.dot(up) < walkable);
    let higher = (ball.0 - heel.0).dot(up) > 0.0;
    let (stand, face, ball_on_face) = match (steep(&heel), steep(&ball)) {
        (_, true) if higher => (heel, ball, true),
        (true, _) if !higher => (ball, heel, false),
        _ => return None,
    };
    let (from, to) = if ball_on_face {
        (at[0], at[1])
    } else {
        (at[1], at[0])
    };
    let face_normal = face.2?;
    let (stand, normal) = match stand.2.unwrap_or(stand.1) {
        normal if normal.dot(up) >= walkable => (stand.0, normal),
        _ => (from - up * (from - root).dot(up), up),
    };
    let across = (to - from).reject_from(up);
    let length = across.length();
    let rise = (face.0 - stand).dot(up);
    if length < 1e-3 || rise <= STILL_M {
        return None;
    }
    let ahead = across / length;
    // Rise per metre along the sole of the ground stood on and of the face.
    let grade = |n: Vec3| -n.dot(ahead) / n.dot(up).max(1e-3);
    let (ground, steeper) = (grade(normal), grade(face_normal));
    if steeper - ground < 1e-3 {
        return None;
    }
    // The crease, along the sole from the stood point, and the move putting the face's point
    // `TOE_M` short of it.
    let crease = (steeper * length - rise) / (steeper - ground);
    let back = (length - (crease - TOE_M)).clamp(0.0, BACK_MOST_M);
    let stood = stand - ahead * back - up * (ground * back);
    let placed = plane(stood, normal, to - from, up);
    let back = -ahead * back;
    Some(if ball_on_face {
        Ground { back, ..placed }
    } else {
        Ground {
            heel: placed.ball,
            ball: placed.heel,
            normal,
            found: [placed.ball, placed.heel],
            back,
        }
    })
}

/// Whether the ground found under a sole's heel and ball (`heel`, `ball`, world points) is a
/// riser between two treads: steeper apart than the sole pitches to (`TILT_DEG`).
fn riser(heel: Vec3, ball: Vec3, up: Vec3) -> bool {
    let span = (ball - heel).reject_from(up).length();
    (ball - heel).dot(up).abs() > span * libm::tanf(TILT_DEG.to_radians())
}

/// The normal a sole stands to on ground found under its heel and ball (`heel`, `ball`, world
/// points): pitched by the slope from one to the other, rolled by the ground's normal there
/// (`normal`) about that line.
fn sole(heel: Vec3, ball: Vec3, normal: Vec3, up: Vec3) -> Vec3 {
    let span = (ball - heel).reject_from(up);
    let length = span.length();
    if length < 1e-3 || normal.dot(up) <= 1e-3 {
        return normal;
    }
    let ahead = span / length;
    let side = up.cross(ahead);
    let pitch = (ball - heel).dot(up) / length;
    let roll = -normal.dot(side) / normal.dot(up);
    (up - ahead * pitch - side * roll).normalize_or(up)
}

/// The foot `grounded.shown` (its ball at `ball_at`) put on its ground (its heel's and ball's
/// heights over the root's level and the normal it stands to), the pelvis lowered by `drop`:
/// turned to the normal within `TILT_DEG` of `up`, about its ball (so a locked ball stays
/// where the lock holds it), and raised or lowered until neither heel nor ball is lower over
/// its ground than the clip has it over its flat ground and one of them is not higher, as a
/// rigid sole rests on the highest ground under it. Lowered no farther from the lowered hip
/// than the clip has it or `REACH_RATIO` of the leg (a foot over a drop stays short); a turn
/// that would put the ankle farther from it than unturned (a foot behind the body at its
/// stance's end, turned about its ball) is made only as far as that: a leg is never asked for
/// more than the clip did.
fn raised(grounded: &Grounded, drop: f32) -> Pose {
    let Grounded {
        shown,
        ball_at,
        heights: (heel, ball, normal),
        up,
        hip,
        reach: leg,
        ..
    } = *grounded;
    let hip_at = hip - up * drop;
    let longest = shown.at.distance(hip).max(REACH_RATIO * leg);
    let full = lean(up, normal, TILT_DEG.to_radians());
    let reach = ball_at - shown.at;
    // The foot turned `s` of the way to the ground's normal.
    let turned = |s: f32| {
        let tilt = Quat::IDENTITY.slerp(full, s);
        let moved = tilt * reach - reach;
        let across = shown.at - moved.reject_from(up);
        let mut lift = heel.max(ball - moved.dot(up));
        if lift < 0.0 {
            // Lowered no farther than the leg reaches from the lowered hip.
            let from = hip_at - across;
            let (high, side) = (from.dot(up), from.reject_from(up).length_squared());
            let deepest = high - (longest * longest - side).max(0.0).sqrt();
            lift = lift.max(deepest).min(0.0);
        }
        (
            Pose {
                at: across + up * lift,
                turn: (tilt * shown.turn).normalize(),
            },
            lift,
        )
    };
    let (placed, lift) = turned(1.0);
    if lift.abs() < STILL_M && full.angle_between(Quat::IDENTITY).to_degrees() < STILL_DEG {
        return shown;
    }
    // Unturned it reaches no farther than the clip's foot or the lowered hip allows.
    let most = shown
        .at
        .distance(hip)
        .max(turned(0.0).0.at.distance(hip_at));
    if placed.at.distance(hip_at) <= most + 1e-5 {
        return placed;
    }
    let (mut lo, mut hi) = (0.0, 1.0);
    for _ in 0..10 {
        let mid = (lo + hi) * 0.5;
        if turned(mid).0.at.distance(hip_at) > most {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    turned(lo).0
}

/// The turn from `up` toward `normal`, no further than `most` radians.
fn lean(up: Vec3, normal: Vec3, most: f32) -> Quat {
    let full = Quat::from_rotation_arc(up, normal.normalize_or(up));
    let (axis, angle) = full.to_axis_angle();
    if angle <= most || !axis.is_finite() {
        full
    } else {
        Quat::from_axis_angle(axis, most)
    }
}

#[cfg(test)]
#[path = "placement_tests.rs"]
mod tests;
