//! What fighting costs in stamina (D61, D447): each move's cost is a field of its set, paid
//! when it starts; a blow taken on the guard costs the set's `guard_stamina` a point of its
//! damage. The pool itself is the walker's (`terra_walker::Stamina`); these rules say only
//! what may start and what is charged.
//!
//! Out of stamina is the developers' call, still Open (D449). Until they decide, a
//! placeholder: no roll or heavy blow starts without its full cost; a light blow with too
//! little comes out tired (slower and weaker, the set's `tired`); a guard with less left than
//! a blow costs is broken by it, as a heavy blow breaks one.

use crate::moves::{Move, MoveSet, Runner, Taken};

/// What a body with so much stamina left may do of a move.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Afford {
    /// All of it.
    Full,
    /// A light blow, slower and weaker (placeholder, D449).
    Tired,
    /// Nothing: it does not start (placeholder, D449).
    No,
}

/// What a move asked with `left` stamina comes to.
pub fn afford(each: &Move, left: f32) -> Afford {
    if each.stamina <= 0.0 || left >= each.stamina {
        Afford::Full
    } else if each.damage > 0.0 && !each.heavy {
        Afford::Tired
    } else {
        Afford::No
    }
}

/// Asks a runner for a move as the stamina left allows: `None` when it cannot be afforded at
/// all, otherwise whether it started now (as `Runner::ask`). Its cost is the caller's to
/// charge when it starts (`Move::stamina`).
pub fn ask(runner: &mut Runner, set: &MoveSet, name: &str, left: f32) -> Option<bool> {
    match afford(set.get(name)?, left) {
        Afford::No => None,
        Afford::Full => Some(runner.ask_as(set, name, false)),
        Afford::Tired => Some(runner.ask_as(set, name, true)),
    }
}

/// Starts a move at once as the stamina left allows, as `Runner::start`; says whether it
/// started.
pub fn start(runner: &mut Runner, set: &MoveSet, name: &str, left: f32) -> bool {
    let Some(each) = set.get(name) else {
        return false;
    };
    match afford(each, left) {
        Afford::No => false,
        afford => {
            runner.start_as(name, afford == Afford::Tired);
            true
        }
    }
}

/// How a blow reaching a guard with `left` stamina is taken, and what it charges the guard:
/// a blocked blow costs `guard_stamina` a point of its damage, and one costing more than is
/// left breaks the guard and takes what there was (placeholder, D449). Any other way of
/// taking it costs nothing.
pub fn guarded(set: &MoveSet, taken: Taken, damage: f32, left: f32) -> (Taken, f32) {
    if taken != Taken::Blocked {
        return (taken, 0.0);
    }
    let cost = set.guard_stamina * damage;
    if cost > left {
        (Taken::GuardBroken, left)
    } else {
        (Taken::Blocked, cost)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set() -> MoveSet {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/data/moves/sword_and_shield.ron"
        );
        MoveSet::read(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    #[test]
    fn every_blow_and_the_roll_cost_stamina_and_a_heavy_costs_more_than_a_light() {
        let set = set();
        let cost = |name: &str| set.get(name).unwrap().stamina;
        for name in ["slash", "backhand", "finisher", "heavy", "roll"] {
            assert!(cost(name) > 0.0, "{name}");
        }
        assert!(cost("heavy") > cost("slash"));
        assert!(set.guard_stamina > 0.0);
    }

    #[test]
    fn rested_everything_starts_and_tired_only_a_light_blow_does() {
        let set = set();
        let each = |name: &str| set.get(name).unwrap();
        for name in ["slash", "heavy", "roll"] {
            assert_eq!(afford(each(name), 100.0), Afford::Full, "{name}");
        }
        // One short of the cost: the heavy and the roll do not start, a light comes tired.
        let short = |name: &str| each(name).stamina - 1.0;
        assert_eq!(afford(each("heavy"), short("heavy")), Afford::No);
        assert_eq!(afford(each("roll"), short("roll")), Afford::No);
        assert_eq!(afford(each("slash"), short("slash")), Afford::Tired);
        assert_eq!(afford(each("slash"), 0.0), Afford::Tired);
        // A move that costs nothing, as raising the guard, is never too dear.
        assert_eq!(afford(each("block"), 0.0), Afford::Full);
    }

    #[test]
    fn a_tired_light_blow_runs_slower_and_hits_softer() {
        let set = set();
        let slash = set.get("slash").unwrap();
        let mut rested = Runner::default();
        let mut tired = Runner::default();
        assert_eq!(ask(&mut rested, &set, "slash", 100.0), Some(true));
        assert_eq!(ask(&mut tired, &set, "slash", 1.0), Some(true));
        assert_eq!(rested.might(&set), 1.0);
        assert!(tired.might(&set) < 1.0);
        let lasts = |runner: &mut Runner| {
            let mut frames = 0;
            while runner.name().is_some() {
                runner.tick(&set, 1.0 / 60.0);
                frames += 1;
            }
            frames as f32 / 60.0
        };
        assert!((lasts(&mut rested) - slash.lasts()).abs() < 2.0 / 60.0);
        assert!((lasts(&mut tired) - slash.lasts() / set.tired.pace).abs() < 2.0 / 60.0);
        // A heavy blow too dear is not asked at all, nor started.
        let mut runner = Runner::default();
        assert_eq!(ask(&mut runner, &set, "heavy", 1.0), None);
        assert!(!start(&mut runner, &set, "heavy", 1.0));
        assert!(runner.name().is_none());
    }

    #[test]
    fn a_block_costs_by_the_blows_damage_and_one_too_dear_breaks_the_guard() {
        let set = set();
        let cost = set.guard_stamina * 15.0;
        assert_eq!(
            guarded(&set, Taken::Blocked, 15.0, 100.0),
            (Taken::Blocked, cost)
        );
        assert_eq!(
            guarded(&set, Taken::Blocked, 15.0, cost - 1.0),
            (Taken::GuardBroken, cost - 1.0)
        );
        assert_eq!(
            guarded(&set, Taken::Blocked, 15.0, 0.0),
            (Taken::GuardBroken, 0.0)
        );
        // A parry is free; a hit or a dodge has no guard to pay.
        for taken in [
            Taken::Parried,
            Taken::Hit,
            Taken::Dodged,
            Taken::GuardBroken,
        ] {
            assert_eq!(guarded(&set, taken, 15.0, 0.0), (taken, 0.0));
        }
    }
}
