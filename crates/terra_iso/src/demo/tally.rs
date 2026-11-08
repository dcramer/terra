//! A play run's reading of its foes (iso-forest-demo): how far each has walked, how many
//! stand within the hero's reach, how many he has struck and felled, a line every
//! `EVERY` frames and on the run's last. It reads every posed body but the hero, so the
//! staged cast reads as foes that never move: what a play run's check asks of the forest's
//! roster (demo/mobs.rs) is that each walks to him, he reaches them and they fall.

use super::mobs::{Horde, STALLED};
use crate::Args;
use crate::play::strike::Struck;
use crate::play::{Fallen, Hero, REACH};
use crate::ruin::fight::Pose;
use bevy::diagnostic::FrameCount;
use bevy::prelude::*;
use std::collections::HashMap;

/// A line this often (frames: 5 s of a film's 30 a second).
const EVERY: u32 = 150;
/// A run whose hero went further than this from where he began (m) walked off from the
/// roster (a hero film's script): its mobs are not asked to have reached him, only never
/// to have stood stuck.
const LEASH: f32 = 8.0;

pub fn plugin(app: &mut App) {
    app.add_systems(PostUpdate, tally);
}

/// One foe's run: where it stood last, the ground it has covered (m, along its way, not
/// straight), whether it has stood within his reach.
#[derive(Default, Clone, Copy)]
pub struct Walk {
    last: Option<Vec2>,
    pub walked: f32,
    pub reached: bool,
}

impl Walk {
    pub fn at(&mut self, p: Vec2, hero: Vec2) {
        if let Some(l) = self.last {
            self.walked += l.distance(p);
        }
        self.last = Some(p);
        self.reached |= p.distance(hero) <= REACH;
    }
}

/// The line: how many foes, the least and most any walked, how many reached him, struck and
/// fell.
pub fn line(frame: u32, walks: &[Walk], struck: usize, fallen: usize) -> String {
    let (lo, hi) = walks.iter().fold((f32::INFINITY, 0.0f32), |(lo, hi), w| {
        (lo.min(w.walked), hi.max(w.walked))
    });
    let lo = if walks.is_empty() { 0.0 } else { lo };
    let reached = walks.iter().filter(|w| w.reached).count();
    format!(
        "foes at frame {frame}: {} of them, walked {lo:.1} to {hi:.1} m, {reached} reached him, \
         {struck} struck, {fallen} fallen",
        walks.len()
    )
}

#[allow(clippy::type_complexity)]
fn tally(
    args: Res<Args>,
    frame: Res<FrameCount>,
    hero: Query<&Transform, With<Hero>>,
    foes: Query<(Entity, &Transform, Has<Struck>, Has<Fallen>), (With<Pose>, Without<Hero>)>,
    mut walks: Local<HashMap<Entity, Walk>>,
    mut roam: Local<Option<(Vec2, f32)>>,
    (horde, mut exit): (Option<Res<Horde>>, MessageWriter<AppExit>),
) {
    let Ok(h) = hero.single() else {
        return;
    };
    let here = h.translation.xz();
    let (from, far) = roam.get_or_insert((here, 0.0));
    *far = far.max(from.distance(here));
    let (mut struck, mut fallen) = (0, 0);
    for (e, tr, s, f) in &foes {
        walks
            .entry(e)
            .or_default()
            .at(tr.translation.xz(), h.translation.xz());
        struck += s as usize;
        fallen += f as usize;
    }
    let last = args.last_frame().is_some_and(|l| frame.0 + 1 == l);
    if frame.0.is_multiple_of(EVERY) || last {
        let w: Vec<Walk> = walks.values().copied().collect();
        info!("{}", line(frame.0, &w, struck, fallen));
        // The run's gate, once its mobs are in play (mobs.rs): its exit an error on a fail.
        if last && let Some(h) = horde {
            match verdict(&w, h.stalls.worst, roam.map_or(0.0, |r| r.1)) {
                Ok(()) => info!("✓ the mobs: each walked to him, none stalled"),
                Err(e) => {
                    error!("✗ {e}");
                    exit.write(AppExit::from_code(1));
                }
            }
        }
    }
}

/// The bar a play run's mobs meet (iso-forest-demo, D632): each reached the hero, and none
/// stood wanting to move past `STALLED` (steer.rs `Stalls`, worst and which).
pub fn verdict(walks: &[Walk], (stall, who): (f32, usize), roamed: f32) -> Result<(), String> {
    let short = walks.iter().filter(|w| !w.reached).count();
    if short > 0 && roamed <= LEASH {
        return Err(format!(
            "{short} of {} mobs never reached the hero (within {REACH} m): one is stuck on its \
             way or set down where it cannot walk to him; see the `mob N stalled` lines and \
             demo/steer.rs, not the roster's places alone",
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
        let l = line(30, &[w, far], 1, 0);
        assert!(
            l.contains("2 of them, walked 0.0 to 10.0 m, 1 reached him, 1 struck"),
            "{l}"
        );
    }

    /// The gate passes mobs that each reached him and never stood stuck, and fails the
    /// planted faults: one that stalled on its way, and one that never got to him.
    #[test]
    fn the_gate_catches_a_stall() {
        let hero = Vec2::ZERO;
        let walked = |xs: &[f32]| {
            let mut w = Walk::default();
            for &x in xs {
                w.at(Vec2::new(x, 0.0), hero);
            }
            w
        };
        let good = [walked(&[8.0, 5.0, 2.0]), walked(&[-7.0, -2.0])];
        assert_eq!(verdict(&good, (0.4, 1), 2.0), Ok(()));
        let e = verdict(&good, (2.5, 1), 2.0).unwrap_err();
        assert!(e.contains("mob 1 stood 2.5 s"), "{e}");
        let stuck = [good[0], walked(&[-7.0, -6.0, -6.0])];
        let e = verdict(&stuck, (0.0, 0), 2.0).unwrap_err();
        assert!(e.contains("1 of 2 mobs never reached"), "{e}");
    }

    /// A hero who walked off past the leash (a hero film's script) leaves mobs behind: they
    /// are not asked to have reached him, but a stall still fails.
    #[test]
    fn a_hero_who_walks_off_is_not_waited_for() {
        let hero = Vec2::ZERO;
        let mut far = Walk::default();
        far.at(Vec2::new(-7.0, 0.0), hero);
        far.at(Vec2::new(-5.0, 0.0), hero);
        assert!(verdict(&[far], (0.0, 0), LEASH - 1.0).is_err());
        assert_eq!(verdict(&[far], (0.0, 0), LEASH + 12.0), Ok(()));
        assert!(verdict(&[far], (2.5, 0), LEASH + 12.0).is_err());
    }
}
