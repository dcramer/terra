//! A drawn sword against creatures (D405's set, D97's rules): the blade that meets the
//! sparring partner's body meets a creature's too. Through a blow's active frames the
//! player's blade, and the way its tip has come since the last tick, are tried against each
//! living creature's zones (`zones::Zones`), and the first met takes the blow by the rule
//! every blow on a creature goes by (`combat::strike`): where it landed, whether it was seen
//! coming, what it teaches. Once a blow, as against a fighter. What the blow does is its
//! move's own damage, less for a tired arm (`Runner::might`), more for a practised one.
//!
//! The other way, a creature's blow on a player with a sword drawn is answered by the set's
//! own guard, parry and roll ([`answer`]), which `combat::take_blows` asks.

use super::{Drawn, Fighter};
use crate::combat::{self, Blow, Struck};
use crate::creatures::Creature;
use crate::species::Bestiary;
use crate::zones::Zones;
use bevy::prelude::*;
use terra_motion::blade::{Edge, HILT_M, TIP_M};
use terra_motion::moves::Phase;
use terra_survival::skills::{Did, Skill, Skills};
use terra_survival::told::Told;
use terra_walker::Player;

/// A creature farther than this from the player is not tried, metres: past any blade's
/// reach and the largest creature's girth.
const TRIED_WITHIN_M: f32 = 9.0;

/// How a player with a sword drawn meets a blow that is not a fighter's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answer {
    /// Open to it: it lands whole.
    Open,
    /// On their guard: most of it is taken on stamina.
    Guarded,
    /// Rolled through it, or turned by a guard just raised: nothing lands.
    Clear,
}

/// How a fighter meets a creature's blow now: clear of it in a roll's untouchable frames or
/// a parry's (a guard raised within the set's parry time), guarded on its guard, else open.
pub fn answer(fighter: &Fighter) -> Answer {
    let rolling = (fighter.runner.now(&fighter.set))
        .is_some_and(|each| each.invulnerable(fighter.runner.t()));
    match fighter.guard_s {
        _ if rolling => Answer::Clear,
        Some(raised) if raised <= fighter.set.parry_s => Answer::Clear,
        Some(_) => Answer::Guarded,
        None => Answer::Open,
    }
}

/// Lands each drawn player's blow on the first creature its blade meets, before the
/// fighters' own contacts (which take the blade's place this tick as where it was).
#[allow(clippy::too_many_arguments)]
pub fn strike(
    mut commands: Commands,
    mut fighters: Query<(Entity, &Transform, &mut Fighter, &Skills), (With<Player>, With<Drawn>)>,
    hands: Query<&GlobalTransform>,
    mut creatures: Query<(Entity, &mut Creature, &Transform), Without<Player>>,
    zones: Zones,
    bestiary: Res<Bestiary>,
    (mut did, mut struck, mut told): (
        MessageWriter<Did>,
        MessageWriter<Struck>,
        MessageWriter<Told>,
    ),
) {
    for (player, at, mut fighter, skills) in &mut fighters {
        let Some(each) = fighter.runner.now(&fighter.set) else {
            continue;
        };
        let open = each.phase(fighter.runner.t()) == Phase::Active && each.damage > 0.0;
        if !open || fighter.runner.landed {
            continue;
        }
        let damage = each.damage * fighter.runner.might(&fighter.set);
        let edge = (fighter.hand)
            .and_then(|hand| hands.get(hand).ok())
            .map(|hand| Edge {
                hilt: hand.transform_point(Vec3::Y * HILT_M),
                tip: hand.transform_point(Vec3::Y * TIP_M),
            });
        let (Some(was), Some(now)) = (fighter.was, edge) else {
            continue;
        };
        // The blade as it is, and the ways its tip and middle came: a fast blow crosses a
        // body between two ticks.
        let middle = |edge: Edge| edge.hilt.lerp(edge.tip, 0.5);
        let tried = [
            (now.hilt, now.tip),
            (was.tip, now.tip),
            (middle(was), middle(now)),
        ];
        let met = creatures
            .iter_mut()
            .filter(|(_, creature, place)| {
                creature.alive() && place.translation.distance(at.translation) < TRIED_WITHIN_M
            })
            .find_map(|(entity, creature, _)| {
                let (at, part) = tried.iter().find_map(|(a, b)| {
                    let (along, part) = zones.along(entity, *a, *b)?;
                    Some((a.lerp(*b, along), part))
                })?;
                Some((entity, creature, at, part))
            });
        let Some((entity, mut creature, at, part)) = met else {
            continue;
        };
        combat::strike(
            &mut commands,
            &bestiary,
            player,
            entity,
            &mut creature,
            Blow {
                damage: damage * skills.speed(Skill::Fighting),
                at,
                part,
                skill: Skill::Fighting,
            },
            &mut did,
            &mut struck,
            &mut told,
        );
        fighter.runner.landed = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::melee::drawn_set;

    /// A creature's blow on a player with a sword drawn: open, it lands; on their guard,
    /// guarded, and clear of it within the parry's time; through a roll's untouchable
    /// frames, clear, and open again before and after them.
    #[test]
    fn a_drawn_players_guard_parry_and_roll_answer_a_creatures_blow() {
        let mut fighter = Fighter::new(None, drawn_set());
        assert_eq!(answer(&fighter), Answer::Open);
        let parry_s = fighter.set.parry_s;
        fighter.guard_s = Some(parry_s * 0.5);
        assert_eq!(answer(&fighter), Answer::Clear);
        fighter.guard_s = Some(parry_s + 0.5);
        assert_eq!(answer(&fighter), Answer::Guarded);
        fighter.guard_s = None;
        let set = fighter.set.clone();
        let roll = (set.moves.iter())
            .find_map(|(name, each)| each.invulnerable.map(|_| name.clone()))
            .expect("the set has a roll with untouchable frames");
        fighter.runner.start(&roll);
        let mut answers = Vec::new();
        while fighter.runner.now(&set).is_some() && answers.len() < 500 {
            answers.push(answer(&fighter));
            fighter.runner.tick(&set, 0.02);
        }
        // Clear through a stretch in its middle, open as it begins and once it is over.
        let clear = answers
            .iter()
            .filter(|each| **each == Answer::Clear)
            .count();
        assert!(
            clear > 5 && clear < answers.len(),
            "{clear} of {} ticks clear",
            answers.len()
        );
        assert_eq!(answers.last(), Some(&Answer::Open));
        assert_eq!(answer(&fighter), Answer::Open);
    }
}
