//! A fighter's feet are kept where they land as every body's are (`terra_walker::feet`,
//! D455); what the fight adds is its stance (the move set's plant heights, its feet read in
//! the body's own frame whatever it plays) and when they are let go: a body down, or a move
//! driving it faster than feet step (a roll), goes as the clip has it.

use bevy::prelude::*;
use terra_motion::feet;
use terra_walker::feet::Kept;

use super::Fighter;

pub fn plugin(app: &mut App) {
    app.add_systems(PostUpdate, let_go.before(bevy::app::AnimationSystems));
}

fn let_go(set: Res<super::Set>, mut fighters: Query<(&Fighter, &mut Kept)>) {
    for (fighter, mut kept) in &mut fighters {
        let driven = fighter
            .runner
            .now(&set.0)
            .is_some_and(|now| now.drive.iter().any(|&(_, mps)| mps > feet::DRIVEN_MPS));
        kept.let_go = fighter.down_s.is_some() || driven;
        kept.stance = Some(set.0.plant);
    }
}
