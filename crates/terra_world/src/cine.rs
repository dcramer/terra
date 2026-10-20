//! Cinematic shots for scripted runs (`--cine <shot.json>[@frame]`, D357): a camera on a
//! keyframed path, its lens, focus and exposure, as Unreal's Sequencer drives a Cine Camera
//! and Unity's Timeline a Cinemachine camera. A shot file holds keys, each a time and where
//! the camera stands, what it looks at, its lens's focal length, its focus and its aperture.
//! Between keys every channel moves on a cubic through the keys (Catmull-Rom tangents over
//! time, Sequencer's "auto" keys), eased to a stop at the ends unless the shot says it is
//! cut into moving. From those few keys come the usual moves: a dolly or a crane (keys
//! along a line or up one), a push-in (toward what it looks at, or a longer lens), a slow
//! pan (one place, the look moving), an orbit (keys given round a point, interpolated round
//! it, so it is a true arc), and a tracking shot (keys given from the player's feet, as
//! Cinemachine's Follow in world space). The player stands, walks a route (which waits for
//! the shot to roll) or is hidden.
//!
//! A shot with no keys leaves the camera to the game, its own third-person follow behind the
//! player, and takes only the look below: the trailer's runs through each place are filmed
//! so (docs/trailer.md).
//!
//! Its look: depth of field from a physical lens (Bevy's `DepthOfField`, a Super 35
//! filmback as the Cine Camera's default), motion blur with a shutter given in degrees at
//! the film's 30 frames a second, the exposure held from the shot's first frame so it does
//! not pump, and the clock held at the shot's hour. Letterbox, titles and the cut are the
//! trailer tool's (`tools/sandbox/trailer.ts`).
//!
//! The shot rolls on its frame, counted as `--press` counts them; before it the camera
//! waits on the first key, so streaming, the eye and the anti-aliasing settle on the view
//! it opens on (Movie Render Queue's warm-up frames). A run a person watches plays the
//! shot over and over.

use crate::daylight::Ticking;
use crate::loading::Screen;
use crate::planet::{MainCamera, Planet};
use crate::remote::CameraHold;
use crate::shell::{Options, ScriptFrame, counting};
use crate::start::Start;
use bevy::{
    anti_alias::taa::TemporalAntiAliasing,
    camera::{Exposure, visibility::VisibilitySystems},
    post_process::{
        auto_exposure::AutoExposure,
        dof::{DepthOfField, DepthOfFieldMode},
        motion_blur::MotionBlur,
    },
    prelude::*,
    transform::TransformSystems,
};
use serde::Deserialize;
use std::ops::{Add, Mul, Sub};
use std::path::Path;

/// The filmback's width in metres: Super 35, the Cine Camera's and most cinema's. A lens of
/// 35 mm on it sees 39 degrees across.
pub const FILMBACK_M: f32 = 0.024_89;
/// Scripted runs step the game a sixtieth of a second a frame and the film keeps every
/// second one, 30 a second (`film.ts`).
pub const STEPS_A_SECOND: f32 = 60.0;
pub const FILM_FPS: f32 = 30.0;
/// The lens when no key gives one.
const LENS_MM: f32 = 35.0;
/// Where on the player a camera looks: about the chest and shoulders.
const PLAYER_LOOK_M: f32 = 1.45;
/// How far the camera's eye moves in one frame for it to be a cut, whoever moved it (a
/// scene's remote calls cut by moving the camera, not by the shot's keys): faster than any
/// camera flies, 300 m/s at 60 steps a second.
const CUT_M: f32 = 5.0;

/// How far the view turns in one frame for it to be a cut: a reverse angle a few metres from
/// the last set-up moves the eye too little to count, but no pan turns 30 degrees in a
/// sixtieth of a second (1800 degrees a second; a fast whip-pan is a few hundred).
const CUT_TURN_DEG: f32 = 30.0;

/// Whether the camera going from `was` to `now` in one frame is a cut, not a move: its eye
/// jumped, or its view turned at once.
fn jumped(was: Option<Transform>, now: &Transform) -> bool {
    was.is_some_and(|was| {
        was.translation.distance(now.translation) > CUT_M
            || was.forward().angle_between(*now.forward()) > CUT_TURN_DEG.to_radians()
    })
}

/// A shot, as its file has it. The file may hold more (the place, the hour, the player's
/// route, the letterbox), which the trailer tool reads and this leaves alone.
#[derive(Deserialize, Clone, Debug)]
pub struct Shot {
    #[serde(default)]
    pub about: String,
    /// How long it runs once it rolls.
    pub seconds: f32,
    #[serde(default)]
    pub rig: Rig,
    /// The camera's path; none leaves the camera to the game.
    #[serde(default)]
    pub keys: Vec<Key>,
    #[serde(default)]
    pub ease: Ease,
    /// A handheld drift, in degrees: Cinemachine's noise, a few slow waves, never repeating
    /// within a shot.
    #[serde(default)]
    pub noise: f32,
    #[serde(default)]
    pub look: Look,
    #[serde(default)]
    pub player: Filmed,
    /// The rest of the file, for the game to read what it asks of a shot (the wildlife
    /// meetings a run asks for, D384) without the camera knowing of it.
    #[serde(flatten)]
    pub more: serde_json::Map<String, serde_json::Value>,
    /// The frame it rolls on, counted as `--press` counts them (`--cine file@frame`).
    #[serde(skip)]
    pub rolls_at: u32,
}

/// How the keys' places are measured.
#[derive(Deserialize, Clone, Copy, Debug, Default)]
pub struct Rig {
    /// Heights are over the ground under each point, not over the start (`--vantage-ground`).
    #[serde(default)]
    pub ground: bool,
    /// Places are metres east, up and south of the player's feet, so the camera keeps with
    /// them as they walk.
    #[serde(default)]
    pub follow: bool,
    /// What `orbit` keys go round.
    #[serde(default)]
    pub around: Option<Target>,
}

/// A point in metres east, up and south of the start (or of the ground, `Rig::ground`), or
/// the player.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(untagged)]
pub enum Target {
    At([f32; 3]),
    Named(Named),
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Named {
    Player,
}

/// What the lens is focused on: so many metres, or what the camera looks at, followed
/// (the Cine Camera's tracking focus).
#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(untagged)]
pub enum Focus {
    Metres(f32),
    Named(FocusNamed),
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FocusNamed {
    Look,
}

/// One key: its time, in seconds from when the shot rolls, and any of its channels. A
/// channel a key does not give is left to the keys that do, as Sequencer's channels are.
#[derive(Deserialize, Clone, Copy, Debug, Default)]
pub struct Key {
    pub t: f32,
    /// Where the camera stands: east, up, south.
    #[serde(default)]
    pub at: Option<[f32; 3]>,
    /// Or round `Rig::around`: a bearing in degrees (0 north, 90 east), metres out and
    /// metres up from it.
    #[serde(default)]
    pub orbit: Option<[f32; 3]>,
    #[serde(default)]
    pub look: Option<Target>,
    /// The lens's focal length in millimetres, on a Super 35 filmback.
    #[serde(default)]
    pub lens: Option<f32>,
    #[serde(default)]
    pub focus: Option<Focus>,
    /// The aperture in f-stops: any key giving one turns depth of field on.
    #[serde(default)]
    pub aperture: Option<f32>,
    /// The camera cuts to this key rather than moving to it: a new set-up, as an editor's cut
    /// between two takes in one sequence. The keys from it to the next cut are a path of their
    /// own, eased as the shot says.
    #[serde(default)]
    pub cut: bool,
}

/// Whether the camera starts and ends at rest.
#[derive(Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Ease {
    /// From rest and to rest.
    #[default]
    InOut,
    /// From rest, cut away from still moving.
    In,
    /// Cut into moving, coming to rest.
    Out,
    /// Moving through both ends, cut in and out of.
    None,
}

impl Ease {
    fn rests_first(self) -> bool {
        matches!(self, Ease::InOut | Ease::In)
    }
    fn rests_last(self) -> bool {
        matches!(self, Ease::InOut | Ease::Out)
    }
}

/// The shot's look.
#[derive(Deserialize, Clone, Copy, Debug)]
pub struct Look {
    #[serde(default)]
    pub exposure: Exposing,
    /// Stops brighter than held, as a photographer's exposure compensation.
    #[serde(default)]
    pub exposure_bias: f32,
    /// The shutter, in degrees of a frame at the film's 30 a second: 180 is film's; 0 none.
    #[serde(default = "Look::film_shutter")]
    pub shutter: f32,
    /// Hold the clock at the shot's hour, so the sun and its shadows stand still.
    #[serde(default = "Look::held")]
    pub hold_clock: bool,
    /// Depth of field drawn with bokeh's bright discs rather than a blur.
    #[serde(default = "Look::held")]
    pub bokeh: bool,
    /// With the game's camera, depth of field at this aperture in f-stops, focused on the
    /// player as the camera follows them.
    #[serde(default)]
    pub aperture: Option<f32>,
    /// How much further every level of detail reaches than the preset's, as a cinematic's
    /// level-of-detail bias does (Unreal's view distance scale, raised by Movie Render
    /// Queue's cinematic scalability): a film's buildings and plants drawn finely out to
    /// where its shots look.
    #[serde(default = "Look::unbiased")]
    pub detail: f32,
    /// A breath marched as a volume whatever the preset, the cinematic tier (D430,
    /// `fire::BreathTier`), as a cinematic asks for its own scalability.
    #[serde(default)]
    pub marched_breath: bool,
}

impl Look {
    fn unbiased() -> f32 {
        1.0
    }

    fn film_shutter() -> f32 {
        180.0
    }
    fn held() -> bool {
        true
    }

    /// Bevy's shutter angle, a share of the step between drawn frames, for the shutter in
    /// degrees at the film's rate.
    pub fn shutter_angle(&self) -> f32 {
        self.shutter / 360.0 * STEPS_A_SECOND / FILM_FPS
    }
}

impl Default for Look {
    fn default() -> Self {
        Self {
            exposure: Exposing::default(),
            exposure_bias: 0.0,
            shutter: Self::film_shutter(),
            hold_clock: true,
            bokeh: true,
            aperture: None,
            detail: Self::unbiased(),
            marched_breath: false,
        }
    }
}

/// How a shot is exposed.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(untagged)]
pub enum Exposing {
    /// `"hold"`, the default: the eye adapts while the shot waits to roll, then holds; or
    /// `"auto"`: the game's own eye throughout.
    Named(ExposingNamed),
    /// An exposure fixed for the whole shot, as EV at ISO 100.
    Ev100(f32),
}

impl Default for Exposing {
    fn default() -> Self {
        Exposing::Named(ExposingNamed::Hold)
    }
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ExposingNamed {
    Hold,
    Auto,
}

/// The player in the shot.
#[derive(Deserialize, Clone, Copy, Debug, Default)]
pub struct Filmed {
    #[serde(default)]
    pub hidden: bool,
}

impl Shot {
    /// A shot from its file, `path` or `path@frame`.
    pub fn read(text: &str) -> Result<Self, String> {
        let (path, frame) = match text.rsplit_once('@') {
            Some((path, frame)) if frame.chars().all(|c| c.is_ascii_digit()) => (
                path,
                frame
                    .parse::<u32>()
                    .map_err(|_| format!("--cine: `{frame}` is not a frame"))?,
            ),
            _ => (text, 0),
        };
        let json = std::fs::read_to_string(Path::new(path))
            .map_err(|e| format!("--cine: cannot read {path}: {e}"))?;
        let mut shot = Self::parse(&json).map_err(|e| format!("--cine {path}: {e}"))?;
        shot.rolls_at = frame;
        Ok(shot)
    }

    /// A shot from its JSON, checked.
    pub fn parse(json: &str) -> Result<Self, String> {
        let shot: Shot = serde_json::from_str(json).map_err(|e| e.to_string())?;
        if shot.seconds <= 0.0 {
            return Err("a shot needs its seconds".into());
        }
        if !shot.holds_camera() {
            return Ok(shot);
        }
        // A cut may come at the time of the take before's last key, that take's end.
        if shot
            .keys
            .windows(2)
            .any(|pair| pair[1].t < pair[0].t || (pair[1].t == pair[0].t && !pair[1].cut))
        {
            return Err("keys must come in order of time, each later than the last".into());
        }
        let takes = shot.keys.iter().filter(|k| k.cut).map(|k| shot.take(k.t));
        if takes
            .into_iter()
            .any(|take| !take.iter().any(|k| k.look.is_some()))
        {
            return Err("a take after a cut says what the camera looks at (`look`)".into());
        }
        let at = shot.keys.iter().filter(|k| k.at.is_some()).count();
        let orbit = shot.keys.iter().filter(|k| k.orbit.is_some()).count();
        if at > 0 && orbit > 0 {
            return Err("a shot's keys place the camera by `at` or by `orbit`, not both".into());
        }
        if at + orbit == 0 {
            return Err("no key says where the camera stands (`at` or `orbit`)".into());
        }
        if orbit > 0 && shot.rig.around.is_none() {
            return Err("`orbit` keys need `rig.around`, a point or \"player\"".into());
        }
        if !shot.keys.iter().any(|k| k.look.is_some()) {
            return Err("no key says what the camera looks at (`look`)".into());
        }
        Ok(shot)
    }

    /// Whether any key, or the look of a shot the game's camera films, turns depth of field on.
    pub fn focused(&self) -> bool {
        self.look.aperture.is_some() || self.keys.iter().any(|k| k.aperture.is_some())
    }

    /// Whether the shot moves the camera itself, rather than leaving it to the game.
    pub fn holds_camera(&self) -> bool {
        !self.keys.is_empty()
    }
}

/// A value a channel can carry.
pub trait Channel:
    Copy + Add<Output = Self> + Sub<Output = Self> + Mul<f32, Output = Self>
{
}
impl<T: Copy + Add<Output = T> + Sub<Output = T> + Mul<f32, Output = T>> Channel for T {}

/// A channel's value at time `t` through its keys `(time, value)`, in order of time: a
/// cubic Hermite through each key, its tangent the Catmull-Rom one over time (the slope from
/// the key before to the key after), at the ends at rest or carried on by `ease`. Before the
/// first key it is the first, after the last the last.
pub fn sample<T: Channel>(keys: &[(f32, T)], t: f32, ease: Ease) -> Option<T> {
    let (first, last) = (keys.first()?, keys.last()?);
    if keys.len() == 1 || t <= first.0 {
        return Some(first.1);
    }
    if t >= last.0 {
        return Some(last.1);
    }
    let i = keys.iter().rposition(|k| k.0 <= t).unwrap_or(0);
    let tangent = |k: usize| -> T {
        let n = keys.len() - 1;
        let zero = keys[k].1 * 0.0;
        if k == 0 {
            if ease.rests_first() {
                zero
            } else {
                (keys[1].1 - keys[0].1) * (1.0 / (keys[1].0 - keys[0].0))
            }
        } else if k == n {
            if ease.rests_last() {
                zero
            } else {
                (keys[n].1 - keys[n - 1].1) * (1.0 / (keys[n].0 - keys[n - 1].0))
            }
        } else {
            (keys[k + 1].1 - keys[k - 1].1) * (1.0 / (keys[k + 1].0 - keys[k - 1].0))
        }
    };
    let (t0, p0) = keys[i];
    let (t1, p1) = keys[i + 1];
    let h = t1 - t0;
    let s = (t - t0) / h;
    let (s2, s3) = (s * s, s * s * s);
    let h00 = 2.0 * s3 - 3.0 * s2 + 1.0;
    let h10 = s3 - 2.0 * s2 + s;
    let h01 = -2.0 * s3 + 3.0 * s2;
    let h11 = s3 - s2;
    Some(p0 * h00 + tangent(i) * (h10 * h) + p1 * h01 + tangent(i + 1) * (h11 * h))
}

/// The vertical field of view, in radians, of a lens of `mm` on the Super 35 filmback, in a
/// frame `aspect` times as wide as high: the filmback's width fills the frame's, and its
/// height is cropped to the frame, as a letterbox crops it again.
pub fn vertical_fov(mm: f32, aspect: f32) -> f32 {
    let half_across = FILMBACK_M * 0.5 / (mm * 0.001);
    2.0 * libm::atanf(half_across / aspect)
}

/// Where an orbit key puts the camera from what it goes round, in the start's east, up and
/// south: a bearing in degrees from north toward east, metres out and up.
pub fn orbit_offset([bearing, out, up]: [f32; 3]) -> Vec3 {
    let b = bearing.to_radians();
    Vec3::new(libm::sinf(b) * out, up, -libm::cosf(b) * out)
}

/// A handheld drift at `t` seconds, as small turns left and up in radians: three slow
/// waves each way at frequencies that never line up, so it neither repeats nor jerks.
pub fn drift(t: f32, degrees: f32) -> (f32, f32) {
    let wave = |a: f32, b: f32, c: f32| {
        0.5 * libm::sinf(a * t) + 0.3 * libm::sinf(b * t + 1.3) + 0.2 * libm::sinf(c * t + 2.1)
    };
    let amount = degrees.to_radians();
    (
        wave(0.71, 1.37, 2.63) * amount,
        wave(0.53, 1.13, 2.29) * amount * 0.6,
    )
}

/// What the camera does on a frame, in the world.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Framing {
    pub eye: Vec3,
    pub target: Vec3,
    pub lens_mm: f32,
    pub focus_m: f32,
    pub aperture: Option<f32>,
}

/// Turns a shot's places into the world's: the start's frame, the ground under a point,
/// and where the player stands.
pub struct Places<'a> {
    pub start: &'a Transform,
    pub ground: &'a dyn Fn(Vec3) -> Vec3,
    pub player: Option<Vec3>,
}

impl Places<'_> {
    /// A point east, up and south of the start, or of the ground under it.
    fn point(&self, [east, up, south]: [f32; 3], over_ground: bool) -> Vec3 {
        if over_ground {
            let ground = (self.ground)(self.start.transform_point(Vec3::new(east, 0.0, south)));
            ground + ground.normalize_or(Vec3::Y) * up
        } else {
            self.start.transform_point(Vec3::new(east, up, south))
        }
    }

    /// The player's feet, or the start's ground if there is no player.
    fn feet(&self) -> Vec3 {
        self.player.unwrap_or(self.start.translation)
    }

    fn target(&self, target: Target, over_ground: bool) -> Vec3 {
        match target {
            Target::At(at) => self.point(at, over_ground),
            Target::Named(Named::Player) => {
                let feet = self.feet();
                feet + feet.normalize_or(Vec3::Y) * PLAYER_LOOK_M
            }
        }
    }

    /// The start's east, up and south as directions in the world.
    fn turned(&self, offset: Vec3) -> Vec3 {
        self.start.rotation * offset
    }
}

impl Shot {
    /// The keys of the set-up filming at `t`: from the last cut at or before it to the next.
    pub fn take(&self, t: f32) -> &[Key] {
        let first = self
            .keys
            .iter()
            .rposition(|k| k.cut && k.t <= t)
            .unwrap_or(0);
        let end = self.keys[first + 1..]
            .iter()
            .position(|k| k.cut)
            .map_or(self.keys.len(), |n| first + 1 + n);
        &self.keys[first..end]
    }

    /// Whether the shot cuts between `from` and `to` seconds, so what the camera saw the
    /// frame before is not what it sees now.
    pub fn cuts_between(&self, from: f32, to: f32) -> bool {
        self.keys.iter().any(|k| k.cut && k.t > from && k.t <= to)
    }

    /// The camera at `t` seconds into the shot.
    pub fn framing(&self, t: f32, places: &Places) -> Framing {
        let keys = if self.keys.is_empty() {
            &[]
        } else {
            self.take(t)
        };
        let ground = self.rig.ground;
        let around = self
            .rig
            .around
            .map(|around| places.target(around, ground))
            .unwrap_or(places.start.translation);
        // The camera's place channel, in the terms its keys use: a point in the world, a
        // bearing, distance and height round `around`, or an offset from the player.
        let channel: Vec<(f32, Vec3)> = keys
            .iter()
            .filter_map(|k| {
                let value = match (k.at, k.orbit) {
                    (_, Some(orbit)) => Vec3::from(orbit),
                    (Some(at), _) if self.rig.follow => Vec3::from(at),
                    (Some(at), _) => places.point(at, ground),
                    _ => return None,
                };
                Some((k.t, value))
            })
            .collect();
        let placed = sample(&channel, t, self.ease).unwrap_or(Vec3::ZERO);
        let eye = if keys.iter().any(|k| k.orbit.is_some()) {
            around + places.turned(orbit_offset(placed.to_array()))
        } else if self.rig.follow {
            places.feet() + places.turned(placed)
        } else {
            placed
        };
        let looks: Vec<(f32, Vec3)> = keys
            .iter()
            .filter_map(|k| Some((k.t, places.target(k.look?, ground))))
            .collect();
        let mut target = sample(&looks, t, self.ease).unwrap_or(around);
        if self.noise > 0.0 {
            let (yaw, pitch) = drift(t, self.noise);
            let up = eye.normalize_or(Vec3::Y);
            let ahead = target - eye;
            let right = ahead.cross(up).normalize_or(Vec3::X);
            let turned = Quat::from_axis_angle(up, yaw) * Quat::from_axis_angle(right, pitch);
            target = eye + turned * ahead;
        }
        let scalar = |pick: fn(&Key) -> Option<f32>| -> Vec<(f32, f32)> {
            keys.iter().filter_map(|k| Some((k.t, pick(k)?))).collect()
        };
        let lens_mm = sample(&scalar(|k| k.lens), t, self.ease).unwrap_or(LENS_MM);
        let aperture = sample(&scalar(|k| k.aperture), t, self.ease);
        // Focus keyed in metres where keys give it, else on what the camera looks at.
        let focus_keys = scalar(|k| match k.focus {
            Some(Focus::Metres(m)) => Some(m),
            _ => None,
        });
        let tracking = keys
            .iter()
            .all(|k| !matches!(k.focus, Some(Focus::Metres(_))));
        let focus_m = if tracking {
            eye.distance(target)
        } else {
            sample(&focus_keys, t, self.ease).unwrap_or(10.0)
        };
        Framing {
            eye,
            target,
            lens_mm,
            focus_m: focus_m.max(0.1),
            aperture,
        }
    }
}

/// The shot a run is filming, and how far into it it is.
#[derive(Resource, Clone, Debug)]
pub struct Cine {
    pub shot: Shot,
    /// Seconds into the shot once it rolls; before, `None`.
    pub t: Option<f32>,
    /// A run a person watches keeps its own time, from when loading is done.
    watched_s: f32,
    /// The exposure held from when the shot rolled.
    held_ev100: Option<f32>,
    /// The shot cut on this frame: the frame before is no history for this one.
    cut: bool,
}

impl Cine {
    /// Whether the shot is rolling: a route waits for it.
    pub fn rolling(&self) -> bool {
        self.t.is_some()
    }
}

/// Marks what the shot films as the player: their feet are where `follow`, `around` and
/// `look: "player"` mean, and it is hidden in a shot that hides the player. The game puts it
/// on its local player.
#[derive(Component, Default)]
pub struct Subject;

/// A run condition: the clock moves on unless a shot holds it.
pub fn clock_runs(cine: Option<Res<Cine>>) -> bool {
    cine.is_none_or(|cine| !cine.shot.look.hold_clock)
}

pub fn plugin(app: &mut App) {
    let Some(shot) = app
        .world()
        .get_resource::<Options>()
        .and_then(|options| options.cine.clone())
    else {
        return;
    };
    app.insert_resource(Cine {
        shot,
        t: None,
        watched_s: 0.0,
        held_ev100: None,
        cut: false,
    })
    .configure_sets(PostUpdate, CameraHold.before(TransformSystems::Propagate))
    .add_systems(PreUpdate, roll.after(crate::shell::scripted_presses))
    .add_systems(Update, (hold_exposure.after(Ticking), bias_detail))
    .add_systems(
        PostUpdate,
        (
            film.in_set(CameraHold).after(crate::remote::hold_camera),
            hide_screen.before(VisibilitySystems::VisibilityPropagate),
        ),
    );
}

/// A shot films the world without the screen's bars, panels and help, as a game's cinematic
/// hides its HUD: every UI tree's root is hidden, its children with it, however late it is
/// spawned (offscreen pictures draw the UI since D405's spar).
fn hide_screen(mut roots: Query<&mut Visibility, (With<Node>, Without<ChildOf>)>) {
    for mut visibility in &mut roots {
        visibility.set_if_neq(Visibility::Hidden);
    }
}

/// How far into the shot it is: in a scripted run by the frame, so the same shot is the
/// same every time; in a watched one by the clock, round and round.
fn roll(
    mut cine: ResMut<Cine>,
    options: Res<Options>,
    frame: Res<ScriptFrame>,
    screen: Res<State<Screen>>,
    time: Res<Time>,
) {
    let lead = cine.shot.rolls_at as f32 / STEPS_A_SECOND;
    let seconds = if options.scripted() {
        frame.0 as f32 / STEPS_A_SECOND
    } else {
        if counting(&options, &screen) {
            cine.watched_s += time.delta_secs();
        }
        let length = lead + cine.shot.seconds + 1.0;
        cine.watched_s % length
    };
    let t = (seconds >= lead).then_some(seconds - lead);
    let cut = matches!((cine.t, t), (Some(from), Some(to)) if cine.shot.cuts_between(from, to));
    if cine.cut != cut {
        cine.cut = cut;
    }
    if cine.t != t {
        cine.t = t;
    }
}

/// Puts the camera where the shot has it, after anything else placed it.
fn film(
    mut commands: Commands,
    cine: Res<Cine>,
    mut eye: Local<Option<Transform>>,
    start: Option<Res<Start>>,
    planet: Res<Planet>,
    mut subject: Query<(&Transform, &mut Visibility), (With<Subject>, Without<MainCamera>)>,
    camera: Single<
        (
            Entity,
            &mut Transform,
            &mut Projection,
            Option<&mut DepthOfField>,
            Option<&mut MotionBlur>,
            Option<&mut TemporalAntiAliasing>,
        ),
        With<MainCamera>,
    >,
) {
    let Some(start) = start else { return };
    let shot = &cine.shot;
    let player = subject.iter().next().map(|(t, _)| t.translation);
    if shot.player.hidden {
        for (_, mut visibility) in &mut subject {
            visibility.set_if_neq(Visibility::Hidden);
        }
    }
    let ground = |at: Vec3| planet.ground_at(at);
    let places = Places {
        start: &start.0,
        ground: &ground,
        player,
    };
    let (entity, mut transform, mut projection, dof, blur, taa) = camera.into_inner();
    // The shot's own path, or the game's camera focused on the player.
    let focus = if shot.holds_camera() {
        let framing = shot.framing(cine.t.unwrap_or(0.0), &places);
        *transform = Transform::from_translation(framing.eye)
            .looking_at(framing.target, framing.eye.normalize_or(Vec3::Y));
        if let Projection::Perspective(perspective) = &mut *projection {
            perspective.fov = vertical_fov(framing.lens_mm, perspective.aspect_ratio.max(0.1));
        }
        framing.aperture.map(|aperture| (framing.focus_m, aperture))
    } else {
        shot.look.aperture.zip(player).map(|(aperture, feet)| {
            let chest = feet + feet.normalize_or(Vec3::Y) * PLAYER_LOOK_M;
            (transform.translation.distance(chest).max(0.1), aperture)
        })
    };
    if let Some((focus_m, aperture)) = focus {
        let aspect = match &*projection {
            Projection::Perspective(p) => p.aspect_ratio.max(0.1),
            _ => 16.0 / 9.0,
        };
        let focused = DepthOfField {
            mode: if shot.look.bokeh {
                DepthOfFieldMode::Bokeh
            } else {
                DepthOfFieldMode::Gaussian
            },
            focal_distance: focus_m,
            aperture_f_stops: aperture,
            // Bevy takes the focal length from the field of view and the sensor's height, so
            // the height is the filmback's cropped to the frame, as the field of view is.
            sensor_height: FILMBACK_M / aspect,
            ..default()
        };
        match dof {
            Some(mut dof) => *dof = focused,
            None => {
                commands.entity(entity).insert(focused);
            }
        }
    }
    // On a cut the anti-aliasing's history and the blur's motion are the last set-up's: the
    // history is dropped and the frame drawn sharp, as a renderer's camera cut does; the
    // shot's own cuts, or the camera put somewhere else at once by whatever holds it.
    let cut = cine.cut || jumped(eye.replace(*transform), &transform);
    if let Some(mut taa) = taa
        && taa.reset != cut
    {
        taa.reset = cut;
    }
    let shutter = if cut { 0.0 } else { shot.look.shutter_angle() };
    match blur {
        Some(mut blur) if blur.shutter_angle != shutter => blur.shutter_angle = shutter,
        Some(_) => {}
        None if shot.look.shutter > 0.0 => {
            commands.entity(entity).insert(MotionBlur {
                shutter_angle: shutter,
                samples: 4,
            });
        }
        None => {}
    }
}

/// Every level of detail's distances as the preset sets them, with the shot's bias.
fn bias_detail(
    cine: Res<Cine>,
    quality: Res<crate::quality::Quality>,
    mut scale: ResMut<crate::detail::DetailScale>,
) {
    let wanted = biased_reach(quality.detail_reach(), cine.shot.look.detail);
    if (scale.0 - wanted).abs() > 1.0e-4 {
        scale.0 = wanted;
    }
}

/// The exposure as the shot has it: held once it rolls at what the eye had adapted to, or
/// fixed, or the game's own.
fn hold_exposure(
    mut commands: Commands,
    mut cine: ResMut<Cine>,
    mut camera: Query<(Entity, &mut Exposure, Option<&mut AutoExposure>), With<MainCamera>>,
) {
    let Ok((entity, mut exposure, adapting)) = camera.single_mut() else {
        return;
    };
    let look = cine.shot.look;
    match look.exposure {
        Exposing::Named(ExposingNamed::Auto) => {}
        Exposing::Ev100(ev) => {
            exposure.ev100 = ev - look.exposure_bias;
            if adapting.is_some() {
                commands.entity(entity).remove::<AutoExposure>();
            }
        }
        Exposing::Named(ExposingNamed::Hold) => {
            if !cine.rolling() {
                return;
            }
            let held = *cine
                .held_ev100
                .get_or_insert(exposure.ev100 - look.exposure_bias);
            exposure.ev100 = held;
            // The eye stays where it had come to while the shot waited.
            if let Some(mut adapting) = adapting
                && (adapting.speed_brighten != 0.0 || adapting.speed_darken != 0.0)
            {
                adapting.speed_brighten = 0.0;
                adapting.speed_darken = 0.0;
            }
        }
    }
}

#[cfg(test)]
mod tests;

/// A preset's level-of-detail reach with a shot's bias (D428). Two raises do not stack: a
/// shot asking for 1.35 under ultra's 2.0 gets 2.0, the larger, as Unreal's
/// `r.ViewDistanceScale` is set by whichever of the scalability group and the cinematic asks
/// further; multiplied, 2.7 overran the 10 GB card's memory at the settlement's pull-back. A
/// cut still scales: a lower preset's reach or a shot's bias under 1 multiplies.
fn biased_reach(reach: f32, bias: f32) -> f32 {
    let bias = bias.max(0.1);
    (reach * bias).min(reach.max(bias))
}
