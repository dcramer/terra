//! A landed blow's feel in play, as the ARPGs sell a hit beside its spark (Diablo IV's, Path
//! of Exile 2's and Hades's hit-pause and screen shake; Jan Willem Nijman's "The art of screen
//! shake", Vlambeer 2013): both bodies held a few frames where the blade bit (the hit-stop),
//! and the view shaken a moment, more for a heavier blow. The numbers are the effects'
//! (`Feel` in a preset, biomes/effects/), read for each blow the land systems report
//! (`Cues::landed`: play/strike.rs's, demo/blows.rs's); a staged scene reports none, so its
//! stills neither stop nor shake.
//!
//! The hero is held through his own move's halt (`Doing::hold`, which his clip follows); a
//! foe by pausing what its player plays, resumed when the stop ends. The shake is laid on the
//! camera after everything that moves it and taken off in `First`, as strike.rs's flinch is.

use super::Hero;
use crate::camera::MainCamera;
use crate::hero::Doing;
use crate::kit::effects::{Cue, Effects, Feel, Landed};
use bevy::animation::graph::AnimationNodeIndex;
use bevy::prelude::*;
use bevy::transform::TransformSystems;

pub fn plugin(app: &mut App) {
    app.init_resource::<Felt>()
        .add_systems(First, settle)
        .add_systems(Update, (feel, release).chain())
        .add_systems(PostUpdate, show.before(TransformSystems::Propagate));
}

/// A value of a light blow's and a heavy's at `weight` (0 to 1) between them.
pub fn between((light, heavy): (f32, f32), weight: f32) -> f32 {
    light + (heavy - light) * weight.clamp(0.0, 1.0)
}

/// A hit-stop: held until its end, a later blow's lengthening it, never stacking.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stop {
    pub until: f32,
}

impl Stop {
    /// Held `s` seconds from `now`, or to the end already set if that is later.
    pub fn hold(&mut self, now: f32, s: f32) {
        self.until = self.until.max(now + s);
    }
    pub fn on(&self, now: f32) -> bool {
        now < self.until
    }
}

/// The hit-stop and shake of a blow: the largest of every preset the blow sets off (its
/// spark's, and a parry's ring's when parried).
pub fn of<'a>(feels: impl Iterator<Item = (Cue, &'a Feel)>, landed: &Landed) -> Option<Kick> {
    feels
        .filter(|(cue, _)| *cue == Cue::Blow || (landed.parried && *cue == Cue::Parry))
        .map(|(_, f)| Kick {
            stop: between(f.stop, landed.weight),
            shake: between(f.shake, landed.weight),
            fall: f.fall,
            hz: f.hz,
        })
        .reduce(|a, b| Kick {
            stop: a.stop.max(b.stop),
            shake: a.shake.max(b.shake),
            fall: a.fall.max(b.fall),
            hz: if b.shake > a.shake { b.hz } else { a.hz },
        })
}

/// One blow's feel: its stop (s), its shake (m), the shake's e-fold (s) and rate (Hz).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Kick {
    pub stop: f32,
    pub shake: f32,
    pub fall: f32,
    pub hz: f32,
}

/// The shake `age` seconds after its blow, along the screen's right and up (m): two waves
/// out of step, so it shakes about rather than along a line, dying by its e-fold; nothing
/// past five of them.
pub fn shake_at(k: &Kick, age: f32) -> Vec2 {
    if age < 0.0 || age > 5.0 * k.fall.max(1e-3) {
        return Vec2::ZERO;
    }
    let tau = core::f32::consts::TAU;
    let fade = k.shake * libm::expf(-age / k.fall.max(1e-3));
    Vec2::new(
        libm::sinf(tau * k.hz * age + 0.4),
        0.7 * libm::sinf(tau * k.hz * 0.77 * age + 1.9),
    ) * fade
}

/// The last blow felt, the shake under way and the one laid on the camera this frame.
#[derive(Resource, Default)]
struct Felt {
    seen: f32,
    shake: Option<(f32, Kick)>,
    laid: Vec3,
}

/// A player paused by a hit-stop: until when, and what it paused.
#[derive(Component)]
struct Stopped {
    stop: Stop,
    paused: Vec<AnimationNodeIndex>,
}

#[allow(clippy::type_complexity)]
fn feel(
    time: Res<Time>,
    cues: Option<Res<crate::kit::effects::Cues>>,
    effects: Option<Res<Effects>>,
    mut felt: ResMut<Felt>,
    mut doing: Query<&mut Doing>,
    children: Query<&Children>,
    mut players: Query<(Entity, &mut AnimationPlayer, Option<&mut Stopped>)>,
    mut commands: Commands,
) {
    let (Some(cues), Some(effects)) = (cues, effects) else {
        return;
    };
    let Some(landed) = cues.landed.as_ref().filter(|l| l.at > felt.seen) else {
        return;
    };
    felt.seen = landed.at;
    let Some(kick) = of(
        effects
            .0
            .iter()
            .filter_map(|(_, p, _)| p.feel.as_ref().map(|f| (p.cue, f))),
        landed,
    ) else {
        return;
    };
    let now = time.elapsed_secs();
    debug!(
        "a blow's feel: stop {:.3} s, shake {:.3} m (weight {:.1}{})",
        kick.stop,
        kick.shake,
        landed.weight,
        if landed.parried { ", parried" } else { "" }
    );
    // A bigger shake under way stands; a smaller one gives way to this.
    let left = felt
        .shake
        .map_or(0.0, |(at, k)| shake_at(&k, now - at).length());
    if kick.shake >= left {
        felt.shake = Some((now, kick));
    }
    for &body in &landed.bodies {
        if let Ok(mut d) = doing.get_mut(body) {
            d.hold(kick.stop);
        }
        for e in std::iter::once(body).chain(children.iter_descendants(body)) {
            let Ok((e, mut p, stopped)) = players.get_mut(e) else {
                continue;
            };
            let mut paused = Vec::new();
            for (i, a) in p.playing_animations_mut() {
                if !a.is_paused() {
                    a.pause();
                    paused.push(*i);
                }
            }
            match stopped {
                Some(mut s) => {
                    s.stop.hold(now, kick.stop);
                    s.paused.extend(paused);
                }
                None => {
                    let mut stop = Stop::default();
                    stop.hold(now, kick.stop);
                    commands.entity(e).insert(Stopped { stop, paused });
                }
            }
        }
    }
}

/// Each player whose stop is over plays on what it paused.
fn release(
    time: Res<Time>,
    mut players: Query<(Entity, &mut AnimationPlayer, &Stopped)>,
    mut commands: Commands,
) {
    let now = time.elapsed_secs();
    for (e, mut p, s) in &mut players {
        if s.stop.on(now) {
            continue;
        }
        for i in &s.paused {
            if let Some(a) = p.animation_mut(*i) {
                a.resume();
            }
        }
        commands.entity(e).remove::<Stopped>();
    }
}

/// The shake laid on the camera along its own right and up, after it has followed the hero.
fn show(
    time: Res<Time>,
    mut felt: ResMut<Felt>,
    mut camera: Query<&mut Transform, (With<MainCamera>, Without<Hero>)>,
) {
    let Some((at, k)) = felt.shake else {
        return;
    };
    let Ok(mut c) = camera.single_mut() else {
        return;
    };
    let s = shake_at(&k, time.elapsed_secs() - at);
    if s == Vec2::ZERO {
        felt.shake = None;
        return;
    }
    let laid = c.rotation * Vec3::new(s.x, s.y, 0.0);
    c.translation += laid;
    felt.laid = laid;
}

/// The last frame's shake taken off the camera, before anything moves it.
fn settle(mut felt: ResMut<Felt>, mut camera: Query<&mut Transform, With<MainCamera>>) {
    if felt.laid == Vec3::ZERO {
        return;
    }
    if let Ok(mut c) = camera.single_mut() {
        c.translation -= felt.laid;
    }
    felt.laid = Vec3::ZERO;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kit::effects::Preset;

    fn landed(weight: f32, parried: bool) -> Landed {
        Landed {
            at: 1.0,
            weight,
            parried,
            bodies: Vec::new(),
        }
    }

    fn kick(names: &[&str], l: &Landed) -> Kick {
        let presets: Vec<Preset> = names.iter().map(|n| Preset::load(n).unwrap()).collect();
        of(
            presets
                .iter()
                .filter_map(|p| p.feel.as_ref().map(|f| (p.cue, f))),
            l,
        )
        .expect("a feel")
    }

    /// The frames a stop set at `at` holds, played at `fps` from a frame on the blow.
    fn frames_held(stop: f32, fps: f32) -> usize {
        let mut s = Stop::default();
        s.hold(1.0, stop);
        (0..60)
            .map(|n| 1.0 + n as f32 / fps)
            .take_while(|t| s.on(*t))
            .count()
    }

    /// A hit-stop holds its bodies for as long as the preset says, to the frame: a hit's
    /// 50 to 80 ms is 3 to 5 frames at 60 frames a second, never none and never a stall.
    #[test]
    fn a_hit_stop_holds_for_its_time() {
        let light = kick(&["hit-spark"], &landed(0.0, false));
        let heavy = kick(&["hit-spark"], &landed(1.0, false));
        for (k, what) in [(light, "light"), (heavy, "heavy")] {
            assert!(
                (0.05..=0.08).contains(&k.stop),
                "{what} blow's stop {} s, outside an ARPG's 50 to 80 ms",
                k.stop
            );
            for fps in [30.0, 60.0, 144.0] {
                let held = frames_held(k.stop, fps) as f32 / fps;
                assert!(
                    held >= k.stop - 1e-4 && held < k.stop + 1.0 / fps,
                    "{what} at {fps} fps held {held} s for a stop of {} s",
                    k.stop
                );
            }
        }
        assert!(heavy.stop > light.stop, "a heavy blow stops longer");
        assert!(heavy.shake > light.shake, "and shakes harder");
    }

    /// A second blow inside a stop lengthens it to its own end, not adds its time on.
    #[test]
    fn blows_inside_a_stop_do_not_stack() {
        let mut s = Stop::default();
        s.hold(1.0, 0.06);
        s.hold(1.03, 0.06);
        assert!(s.on(1.08) && !s.on(1.09 + 1e-4), "until {}", s.until);
        s.hold(1.04, 0.01);
        assert!((s.until - 1.09).abs() < 1e-6, "a shorter one stands aside");
    }

    /// A parry rings harder than a block on the guard, its own preset's feel taken over the
    /// spark's; unparried, the ring sets nothing off.
    #[test]
    fn a_parry_takes_its_ring_s_feel() {
        let names = ["hit-spark", "parry-ring"];
        let block = kick(&names, &landed(0.0, false));
        let parry = kick(&names, &landed(0.0, true));
        assert_eq!(block, kick(&["hit-spark"], &landed(0.0, false)));
        assert!(parry.stop > block.stop && parry.shake > block.shake);
    }

    /// The shake starts at once, dies by its e-fold and is gone: the camera comes back.
    #[test]
    fn the_shake_dies_away() {
        let k = kick(&["hit-spark"], &landed(1.0, false));
        let peak = (0..20)
            .map(|n| shake_at(&k, n as f32 / 240.0).length())
            .fold(0.0, f32::max);
        assert!(peak > 0.5 * k.shake, "peak {peak} of {}", k.shake);
        assert!(shake_at(&k, 3.0 * k.fall).length() < 0.06 * k.shake);
        assert_eq!(shake_at(&k, 6.0 * k.fall), Vec2::ZERO);
        assert_eq!(shake_at(&k, -0.01), Vec2::ZERO);
    }
}
