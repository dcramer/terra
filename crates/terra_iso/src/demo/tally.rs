//! A play run's reading of its foes (iso-forest-demo): how far each has walked, how many
//! stand within the hero's reach, how many he has struck and felled, a line every
//! `EVERY` frames and on the run's last. It reads every posed body but the hero, so the
//! staged cast reads as foes that never move: what a play run's check asks of the forest's
//! roster (demo/mobs.rs) is that each walks to him, he reaches them and they fall.

use super::mobs::{Foe, Horde, STALLED};
use crate::Args;
use crate::play::strike::Struck;
use crate::play::{Fallen, Hero, REACH};
use crate::ruin::fight::Pose;
use bevy::diagnostic::FrameCount;
use bevy::prelude::*;
use std::collections::HashMap;

/// A line this often (frames: 5 s of a film's 30 a second).
const EVERY: u32 = 150;
/// A hero who went further than this (m) from where he stood when a mob woke walked off
/// from it (a hero film's script): it is not asked to have reached him, only never to have
/// stood stuck. A mob whose pack never woke (a route he did not walk) is not asked either.
const LEASH: f32 = 8.0;

pub fn plugin(app: &mut App) {
    app.add_systems(PostUpdate, tally);
}

/// One foe's run: where it stood last, the ground it has covered (m, along its way, not
/// straight), whether it has stood within his reach, where he stood when it woke and how
/// far he has gone from there since.
#[derive(Default, Clone, Copy)]
pub struct Walk {
    last: Option<Vec2>,
    pub walked: f32,
    pub reached: bool,
    woke: Option<Vec2>,
    roamed: f32,
}

impl Walk {
    pub fn at(&mut self, p: Vec2, hero: Vec2) {
        if let Some(l) = self.last {
            self.walked += l.distance(p);
        }
        self.last = Some(p);
        self.reached |= p.distance(hero) <= REACH;
        if let Some(w) = self.woke {
            self.roamed = self.roamed.max(w.distance(hero));
        }
    }

    /// Its pack woke (mobs.rs), the hero at `hero`; again changes nothing.
    pub fn wake(&mut self, hero: Vec2) {
        self.woke.get_or_insert(hero);
    }
}

/// The line: how many foes and how many awake, the least and most any walked, how many
/// reached him, struck and fell, and where he stands (a route's script is read off it).
pub fn line(frame: u32, walks: &[Walk], struck: usize, fallen: usize, hero: Vec2) -> String {
    let (lo, hi) = walks.iter().fold((f32::INFINITY, 0.0f32), |(lo, hi), w| {
        (lo.min(w.walked), hi.max(w.walked))
    });
    let lo = if walks.is_empty() { 0.0 } else { lo };
    let reached = walks.iter().filter(|w| w.reached).count();
    let awake = walks.iter().filter(|w| w.woke.is_some()).count();
    format!(
        "foes at frame {frame}: {} of them, {awake} awake, walked {lo:.1} to {hi:.1} m, \
         {reached} reached him, {struck} struck, {fallen} fallen; he is at ({:.1}, {:.1})",
        walks.len(),
        hero.x,
        hero.y
    )
}

#[allow(clippy::type_complexity)]
fn tally(
    args: Res<Args>,
    frame: Res<FrameCount>,
    hero: Query<&Transform, With<Hero>>,
    foes: Query<
        (Entity, &Transform, Has<Struck>, Has<Fallen>, Option<&Foe>),
        (With<Pose>, Without<Hero>),
    >,
    mut walks: Local<HashMap<Entity, Walk>>,
    (horde, mut exit): (Option<Res<Horde>>, MessageWriter<AppExit>),
) {
    let Ok(h) = hero.single() else {
        return;
    };
    let here = h.translation.xz();
    let (mut struck, mut fallen) = (0, 0);
    for (e, tr, s, f, foe) in &foes {
        let w = walks.entry(e).or_default();
        // A body that is no mob (or has no horde) is awake from the first.
        let awake = match (foe, horde.as_deref()) {
            (Some(foe), Some(h)) => h.awake.get(foe.i).copied().unwrap_or(true),
            _ => true,
        };
        if awake {
            w.wake(here);
        }
        w.at(tr.translation.xz(), here);
        struck += s as usize;
        fallen += f as usize;
    }
    let last = args.last_frame().is_some_and(|l| frame.0 + 1 == l);
    if frame.0.is_multiple_of(EVERY) || last {
        let w: Vec<Walk> = walks.values().copied().collect();
        info!("{}", line(frame.0, &w, struck, fallen, here));
        // The run's gate, once its mobs are in play (mobs.rs): its exit an error on a fail.
        if last && let Some(h) = horde {
            match verdict(&w, h.stalls.worst) {
                Ok(()) => info!("✓ the mobs: each walked to him, none stalled"),
                Err(e) => {
                    error!("✗ {e}");
                    exit.write(AppExit::from_code(1));
                }
            }
        }
    }
}

/// The bar a play run's mobs meet (iso-forest-demo, D632): each that woke reached the hero
/// (unless he walked off past the leash from where it woke), and none stood wanting to move
/// past `STALLED` (steer.rs `Stalls`, worst and which).
pub fn verdict(walks: &[Walk], (stall, who): (f32, usize)) -> Result<(), String> {
    let short = walks
        .iter()
        .filter(|w| w.woke.is_some() && !w.reached && w.roamed <= LEASH)
        .count();
    if short > 0 {
        return Err(format!(
            "{short} of {} mobs woke but never reached the hero (within {REACH} m): one is \
             stuck on its way or set down where it cannot walk to him; see the `mob N stalled` \
             lines and demo/steer.rs, not the roster's places alone",
            walks.len()
        ));
    }
    if stall > STALLED {
        return Err(format!(
            "mob {who} stood {stall:.1} s wanting to move (over {STALLED} s): steering left it \
             stuck (demo/steer.rs recovery); its `mob {who} stalled` lines say when"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A foe's walk is the ground it covered along its way (out and back counts twice), and
    /// it has reached him once it stood within his reach, though it left again.
    #[test]
    fn a_walk_counts_its_way_and_its_reach() {
        let hero = Vec2::ZERO;
        let mut w = Walk::default();
        w.wake(hero);
        for x in [8.0, 6.0, 2.0, 6.0] {
            w.at(Vec2::new(x, 0.0), hero);
        }
        assert!((w.walked - 10.0).abs() < 1e-5, "{}", w.walked);
        assert!(w.reached);
        let mut far = Walk::default();
        far.at(Vec2::new(9.0, 0.0), hero);
        far.at(Vec2::new(9.0, 0.0), hero);
        assert_eq!(far.walked, 0.0);
        assert!(!far.reached);
        let l = line(30, &[w, far], 1, 0, Vec2::new(-2.0, 3.5));
        assert!(
            l.contains(
                "2 of them, 1 awake, walked 0.0 to 10.0 m, 1 reached him, 1 struck, 0 fallen; \
                 he is at (-2.0, 3.5)"
            ),
            "{l}"
        );
    }

    /// The gate passes mobs that each reached him and never stood stuck, and fails the
    /// planted faults: one that stalled on its way, and one that woke and never got to him.
    #[test]
    fn the_gate_catches_a_stall() {
        let hero = Vec2::ZERO;
        let walked = |xs: &[f32]| {
            let mut w = Walk::default();
            w.wake(hero);
            for &x in xs {
                w.at(Vec2::new(x, 0.0), hero);
            }
            w
        };
        let good = [walked(&[8.0, 5.0, 2.0]), walked(&[-7.0, -2.0])];
        assert_eq!(verdict(&good, (0.4, 1)), Ok(()));
        let e = verdict(&good, (2.5, 1)).unwrap_err();
        assert!(e.contains("mob 1 stood 2.5 s"), "{e}");
        let stuck = [good[0], walked(&[-7.0, -6.0, -6.0])];
        let e = verdict(&stuck, (0.0, 0)).unwrap_err();
        assert!(e.contains("1 of 2 mobs woke but never reached"), "{e}");
    }

    /// A hero who walked off past the leash from where a mob woke (a hero film's script)
    /// leaves it behind: it is not asked to have reached him, but a stall still fails.
    #[test]
    fn a_hero_who_walks_off_is_not_waited_for() {
        let run = |to: f32| {
            let mut w = Walk::default();
            w.wake(Vec2::ZERO);
            w.at(Vec2::new(-7.0, 0.0), Vec2::ZERO);
            w.at(Vec2::new(-5.0, 0.0), Vec2::new(to, 0.0));
            w
        };
        assert!(verdict(&[run(LEASH - 1.0)], (0.0, 0)).is_err());
        assert_eq!(verdict(&[run(LEASH + 12.0)], (0.0, 0)), Ok(()));
        assert!(verdict(&[run(LEASH + 12.0)], (2.5, 0)).is_err());
    }

    /// A route's pack the hero never came near is not asked to reach him; once it woke, it
    /// is, though he walked far before it woke (he is measured from where it woke).
    #[test]
    fn a_pack_is_asked_only_once_it_woke() {
        let mut asleep = Walk::default();
        for x in [0.0, 30.0, 40.0] {
            asleep.at(Vec2::new(50.0, 0.0), Vec2::new(x, 0.0));
        }
        assert_eq!(verdict(&[asleep], (0.0, 0)), Ok(()));
        let mut woke = asleep;
        woke.wake(Vec2::new(40.0, 0.0));
        woke.at(Vec2::new(46.0, 0.0), Vec2::new(42.0, 0.0));
        let e = verdict(&[woke], (0.0, 0)).unwrap_err();
        assert!(e.contains("1 of 1 mobs woke but never reached"), "{e}");
        woke.at(Vec2::new(44.0, 0.0), Vec2::new(42.0, 0.0));
        assert_eq!(verdict(&[woke], (0.0, 0)), Ok(()));
    }
}
