//! A play run's reading of its foes (iso-forest-demo): how far each has walked, how many
//! stand within the hero's reach, how many he has struck and felled, a line every
//! `EVERY` frames and on the run's last. It reads every posed body but the hero, so the
//! staged cast reads as foes that never move: what a play run's check asks of the forest's
//! roster (demo/mobs.rs) is that each walks to him, he reaches them and they fall.

use crate::Args;
use crate::play::strike::Struck;
use crate::play::{Fallen, Hero, REACH};
use crate::ruin::fight::Pose;
use bevy::diagnostic::FrameCount;
use bevy::prelude::*;
use std::collections::HashMap;

/// A line this often (frames: 5 s of a film's 30 a second).
const EVERY: u32 = 150;

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
) {
    let Ok(h) = hero.single() else {
        return;
    };
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
    }
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
}
