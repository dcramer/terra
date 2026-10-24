//! How the camera frames the player: over the right shoulder for the walk, building and
//! everything else (the camera's own, D274, D285); with a weapon drawn, as Enshrouded frames a
//! fight (D180): the player centred and low, about a quarter of the frame's height, no shoulder
//! offset; locked on, from behind the player on the line to the target, both in the frame, as
//! action games' lock-on cameras do. It eases from one to the other on drawing and sheathing.
//! The numbers are data, `assets/data/camera.ron`; this says where the boom hangs, which way it
//! looks and how long it is, and the camera's collision takes it from there as for the walk.

use super::HEAD_M;
use bevy::prelude::*;
use serde::Deserialize;

/// The fight's framing, from `camera.ron` (its comments say what each is).
#[derive(Deserialize, Clone, Debug)]
pub struct Fight {
    pub body_m: f32,
    pub share: f32,
    pub rise_m: f32,
    pub locked_pitch_deg: f32,
    pub locked_focus: f32,
    pub ease_in_s: f32,
    pub ease_out_s: f32,
    pub lock_s: f32,
}

/// The camera's framings, read from `assets/data/camera.ron`.
#[derive(Resource, Deserialize, Clone, Debug)]
pub struct Framing {
    pub fight: Fight,
}

impl Framing {
    pub const FILE: &str = "camera.ron";
}

impl Default for Framing {
    fn default() -> Self {
        terra_world::data::read(Self::FILE)
    }
}

/// On a player while a weapon is drawn, put there by what draws it: the camera frames the
/// fight, and keeps `target`, what they are locked on to, in the frame with them.
#[derive(Component, Default, Debug)]
pub struct FightView {
    pub target: Option<Entity>,
}

/// How far the camera has come to the fight's framing (0 the walk's, 1 the fight's) and to
/// the lock, and where the target's head was last seen, to ease away from it once it is gone.
#[derive(Default, Debug)]
pub struct Eased {
    fight: f32,
    locked: f32,
    target: Vec3,
}

/// Where the boom hangs from, as an offset from the head; which way the camera looks; and how
/// long the boom would be, before anything in the way shortens it.
#[derive(Debug, Clone, Copy)]
pub struct Frame {
    pub offset: Vec3,
    pub looking: Vec3,
    pub boom_m: f32,
}

/// Where a point falls up the picture of a lens at `eye` looking along `looking`, `up` the
/// world's up, -1 at the bottom and 1 at the top, for a vertical field of view of `vfov`.
fn rise_in_frame(eye: Vec3, looking: Vec3, up: Vec3, vfov: f32, at: Vec3) -> f32 {
    let right = looking.cross(up).normalize();
    let lens_up = right.cross(looking);
    let v = at - eye;
    v.dot(lens_up) / (v.dot(looking) * libm::tanf(vfov * 0.5))
}

/// The share of the frame's height a body standing at `feet`, `body_m` tall, fills from a lens
/// at `eye` looking along `looking`.
pub fn share(eye: Vec3, looking: Vec3, up: Vec3, vfov: f32, feet: Vec3, body_m: f32) -> f32 {
    let rise = |h: f32| rise_in_frame(eye, looking, up, vfov, feet + up * h);
    (rise(body_m) - rise(0.0)) * 0.5
}

/// How long the fight's boom is: so the player fills `fight.share` of the frame's height, the
/// boom hanging `fight.rise_m` over the head along `looking`, at the field of view `vfov`.
/// Found by bisection, the share falling as the boom grows.
pub fn boom(fight: &Fight, vfov: f32, looking: Vec3, up: Vec3) -> f32 {
    let head = up * HEAD_M;
    let pivot = head + up * fight.rise_m;
    let filled = |length: f32| {
        share(
            pivot - looking * length,
            looking,
            up,
            vfov,
            Vec3::ZERO,
            fight.body_m,
        )
    };
    let (mut near, mut far) = (0.5_f32, 60.0_f32);
    for _ in 0..40 {
        let mid = (near + far) * 0.5;
        if filled(mid) > fight.share {
            near = mid;
        } else {
            far = mid;
        }
    }
    (near + far) * 0.5
}

fn smooth(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// This frame's framing: `walk` the walk's (over the shoulder, as zoomed), eased toward the
/// fight's while `drawn`, and toward the lock while `target`, the target's head, is known.
/// `head` is the player's head, `up` the world's up there, `vfov` the camera's field of view.
pub fn frame(
    fight: &Fight,
    eased: &mut Eased,
    dt: f32,
    drawn: bool,
    target: Option<Vec3>,
    head: Vec3,
    walk: Frame,
    vfov: f32,
) -> Frame {
    let up = head.normalize();
    let step = |now: f32, on: bool, into: f32, out: f32| {
        let rate = if on {
            dt / into.max(1e-3)
        } else {
            -dt / out.max(1e-3)
        };
        (now + rate).clamp(0.0, 1.0)
    };
    eased.fight = step(eased.fight, drawn, fight.ease_in_s, fight.ease_out_s);
    eased.locked = step(
        eased.locked,
        drawn && target.is_some(),
        fight.lock_s,
        fight.lock_s,
    );
    if let Some(target) = target {
        eased.target = target;
    }
    if eased.fight <= 0.0 {
        return walk;
    }
    // Drawn: from over the head, along the view.
    let mut offset = up * fight.rise_m;
    let mut looking = walk.looking;
    let mut extra = 0.0;
    // Locked: from over a point between the fighters' heads, looking at it down the line from
    // the player to the target.
    let toward = (eased.target - head).reject_from(up);
    if eased.locked > 0.0 && toward.length() > 0.1 {
        let k = smooth(eased.locked);
        let ahead = toward.normalize();
        let pitch = fight.locked_pitch_deg.to_radians();
        let locked = (ahead * libm::cosf(pitch) + up * libm::sinf(pitch)).normalize();
        looking = looking.lerp(locked, k).normalize_or(locked);
        let focus = toward * fight.locked_focus;
        offset += focus * k;
        // So the player stays as far from the lens as unlocked.
        extra = focus.dot(ahead) * k;
    }
    let drawn = Frame {
        offset,
        looking,
        boom_m: boom(fight, vfov, looking, up) + extra,
    };
    let k = smooth(eased.fight);
    Frame {
        offset: walk.offset.lerp(drawn.offset, k),
        looking: walk
            .looking
            .lerp(drawn.looking, k)
            .normalize_or(drawn.looking),
        boom_m: walk.boom_m + (drawn.boom_m - walk.boom_m) * k,
    }
}

/// This frame's framing: the boom over the right shoulder as zoomed, or eased to a fight's
/// while a weapon is drawn (`fight`), keeping what it is locked on to (`target`) in the frame.
pub(super) fn framed(
    zoom: &mut super::Zoom,
    framing: &Framing,
    dt: f32,
    view: &crate::View,
    head: Vec3,
    looking: Vec3,
    fight: Option<&FightView>,
    target: Option<&Transform>,
    lens: Option<&Projection>,
) -> Frame {
    let up = head.normalize();
    let vfov = match lens {
        Some(Projection::Perspective(lens)) => lens.fov,
        _ => PerspectiveProjection::default().fov,
    };
    let target = target.map(|at| at.translation + at.translation.normalize() * HEAD_M);
    let [aside, rise] = super::SHOULDER_M;
    let walk = Frame {
        offset: view.forward.cross(up) * aside + up * rise,
        looking,
        boom_m: zoom.now,
    };
    let (drawn, eased) = (fight.is_some(), &mut zoom.framing);
    frame(&framing.fight, eased, dt, drawn, target, head, walk, vfov)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fight() -> Fight {
        Framing::default().fight
    }

    /// The bars D180 sets (tools/combat/refs/enshrouded.json's `targets`).
    const SHARE: [f32; 2] = [0.20, 0.27];
    const BODIES_BACK: [f32; 2] = [3.2, 4.8];

    fn pitched(pitch_deg: f32) -> Vec3 {
        let p = pitch_deg.to_radians();
        Vec3::new(libm::cosf(p), libm::sinf(p), 0.0)
    }

    #[test]
    fn level_the_boom_is_as_long_as_the_pinhole_says() {
        // Level, every point of the body is as deep as the boom is long, so the body fills
        // body / (2 d tan(vfov / 2)) of the frame's height.
        let fight = fight();
        for vfov_deg in [45.0_f32, 55.0, 70.0] {
            let vfov = vfov_deg.to_radians();
            let d = fight.body_m / (2.0 * fight.share * libm::tanf(vfov * 0.5));
            let got = boom(&fight, vfov, Vec3::X, Vec3::Y);
            assert!((got - d).abs() < 0.01, "{vfov_deg} deg: {got} against {d}");
        }
    }

    #[test]
    fn drawn_the_player_fills_a_quarter_of_the_frame_from_four_bodies_back() {
        let fight = fight();
        let up = Vec3::Y;
        for pitch in [-25.0, -14.0, -5.0, 0.0, 10.0] {
            let looking = pitched(pitch);
            let length = boom(&fight, 55_f32.to_radians(), looking, up);
            let eye = up * (HEAD_M + fight.rise_m) - looking * length;
            let filled = share(
                eye,
                looking,
                up,
                55_f32.to_radians(),
                Vec3::ZERO,
                fight.body_m,
            );
            assert!(
                (SHARE[0]..=SHARE[1]).contains(&filled),
                "{pitch} deg: {filled}"
            );
            let back = eye.distance(up * fight.body_m * 0.5) / fight.body_m;
            assert!(
                (BODIES_BACK[0]..=BODIES_BACK[1]).contains(&back),
                "{pitch} deg: {back} bodies back"
            );
        }
    }

    /// The frame after `seconds` of easing, at 60 frames a second.
    fn eased(
        seconds: f32,
        eased: &mut Eased,
        drawn: bool,
        target: Option<Vec3>,
        walk: Frame,
    ) -> Frame {
        let head = Vec3::Y * HEAD_M;
        let mut out = walk;
        for _ in 0..(seconds * 60.0) as usize {
            out = frame(
                &fight(),
                eased,
                1.0 / 60.0,
                drawn,
                target,
                head,
                walk,
                55_f32.to_radians(),
            );
        }
        out
    }

    fn walk() -> Frame {
        Frame {
            offset: Vec3::new(0.0, 0.25, 0.6),
            looking: Vec3::X,
            boom_m: 4.6,
        }
    }

    #[test]
    fn it_eases_in_on_drawing_and_back_to_the_walk_on_sheathing() {
        let mut state = Eased::default();
        let start = eased(0.1, &mut state, false, None, walk());
        assert_eq!((start.offset, start.boom_m), (walk().offset, walk().boom_m));
        let halfway = eased(fight().ease_in_s * 0.5, &mut state, true, None, walk());
        let drawn = eased(fight().ease_in_s, &mut state, true, None, walk());
        assert!(
            walk().boom_m < halfway.boom_m && halfway.boom_m < drawn.boom_m,
            "{halfway:?}"
        );
        assert!(
            drawn.offset.z.abs() < 1e-4,
            "no shoulder offset drawn: {:?}",
            drawn.offset
        );
        let sheathed = eased(fight().ease_out_s + 0.05, &mut state, false, None, walk());
        assert_eq!(
            (sheathed.offset, sheathed.boom_m),
            (walk().offset, walk().boom_m)
        );
    }

    #[test]
    fn locked_on_both_fighters_are_in_the_frame() {
        let fight = fight();
        let vfov = 55_f32.to_radians();
        let up = Vec3::Y;
        let head = up * HEAD_M;
        // Targets near and at the lock's reach, ahead, aside and behind the view.
        for (far, bearing) in [
            (1.5_f32, 0.0_f32),
            (2.0, 60.0),
            (4.0, 120.0),
            (8.0, -150.0),
            (15.0, 30.0),
        ] {
            let b = bearing.to_radians();
            let feet = Vec3::new(libm::cosf(b), 0.0, libm::sinf(b)) * far;
            let mut state = Eased::default();
            let f = eased(1.0, &mut state, true, Some(feet + up * HEAD_M), walk());
            let eye = head + f.offset - f.looking * f.boom_m;
            let right = f.looking.cross(up).normalize();
            let lens_up = right.cross(f.looking);
            let inside = |at: Vec3| {
                let v = at - eye;
                let z = v.dot(f.looking);
                let tan = libm::tanf(vfov * 0.5);
                let (x, y) = (
                    v.dot(right) / (z * tan * 16.0 / 9.0),
                    v.dot(lens_up) / (z * tan),
                );
                z > 0.0 && x.abs() < 0.95 && y.abs() < 0.95
            };
            for body in [Vec3::ZERO, feet] {
                assert!(
                    inside(body) && inside(body + up * fight.body_m),
                    "{far} m at {bearing} deg: {body}"
                );
            }
            let filled = share(eye, f.looking, up, vfov, Vec3::ZERO, fight.body_m);
            assert!(
                (SHARE[0]..=SHARE[1] + 0.02).contains(&filled),
                "{far} m at {bearing} deg: {filled}"
            );
        }
    }
}
