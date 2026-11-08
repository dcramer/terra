//! A script's answers to the foes' blows (iso-forest-demo j25): `--on-tell
//! NAME[:X,Z][~LEAD]@FROM-TO` presses one of his abilities when a foe's blow is told on him
//! (its ground tell shown with him in its arc, demo/blows.rs), LEAD seconds before it lands,
//! else as the tell starts; once a blow, while the frame is in FROM..TO. A held one (the
//! shield) is held until the blow has landed. A route's dodge and parry were frame-fixed
//! presses before (j24): moving a walk moved when a mob swung, and the roll then met nothing.
//! Answering the tell is closed-loop, as a player answers a Diablo IV or PoE 2 monster's
//! ground warning; the timing change before it moves the answer with the blow.

use super::Ability;
use bevy::prelude::*;

/// How far an unaimed roll goes, away from the foe that tells the blow (m): past the reach of
/// its arc from where he is likely to stand (play.rs `REACH` 3.0).
const AWAY: f32 = 3.0;
/// How long after the blow lands a held answer stays held (s), so it is up through the frame
/// the blow is judged on (demo/blows.rs judges it the frame its hit time passes).
const HOLD_AFTER: f32 = 0.1;

/// A foe's blow told on him: which foe (its roster index), where it stands, and when the blow
/// lands (s of elapsed time).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Blow {
    pub by: u32,
    pub from: Vec2,
    pub lands: f32,
}

/// The blows told on the hero now: written by demo/blows.rs after his moves each frame, read by
/// his script the next (`answer`). The hero's own seam: demo/ writes it, hero/ asks for it.
#[derive(Component, Default, Clone, Debug)]
pub struct Told(pub Vec<Blow>);

/// One `--on-tell` rule: the ability, its aim point if any, how long before the blow lands it
/// fires (none: as soon as it is told), and the frames it listens over.
#[derive(Clone, Debug, PartialEq)]
pub struct OnTell {
    pub ability: Ability,
    pub at: Option<Vec2>,
    pub lead: Option<f32>,
    pub from: u32,
    pub to: u32,
}

impl OnTell {
    /// `NAME[:X,Z][~LEAD]@FROM-TO`, as a route writes it.
    pub fn parse(v: &str) -> Result<OnTell, String> {
        let bad = |why: &str| {
            format!(
                "--on-tell {v}: {why}; it wants NAME[:X,Z][~LEAD]@FROM-TO (an ability, an aim \
                 point, seconds before the blow lands, the frames it listens over; hero/react.rs)"
            )
        };
        let (what, span) = v.split_once('@').ok_or_else(|| bad("no @FROM-TO"))?;
        let (what, lead) = match what.split_once('~') {
            Some((w, l)) => (
                w,
                Some(l.parse::<f32>().map_err(|_| bad("LEAD not a number"))?),
            ),
            None => (what, None),
        };
        let (name, at) = match what.split_once(':') {
            Some((n, xz)) => {
                let p: Vec<f32> = xz.split(',').filter_map(|s| s.parse().ok()).collect();
                let &[x, z] = &p[..] else {
                    return Err(bad("X,Z not two numbers"));
                };
                (n, Some(Vec2::new(x, z)))
            }
            None => (what, None),
        };
        let ability = Ability::named(name).ok_or_else(|| {
            let all: Vec<_> = Ability::ALL.iter().map(|a| a.name()).collect();
            bad(&format!("no ability {name}; the hero's are {all:?}"))
        })?;
        let (from, to) = span
            .split_once('-')
            .ok_or_else(|| bad("no FROM-TO window"))?;
        let (Ok(from), Ok(to)) = (from.parse(), to.parse()) else {
            return Err(bad("FROM-TO not frames"));
        };
        Ok(OnTell {
            ability,
            at,
            lead,
            from,
            to,
        })
    }
}

/// What a rule has done: the blows it answered (by foe and landing time) and the one it holds
/// a held ability for, until it lands.
#[derive(Clone, Debug, Default)]
pub struct Answers {
    answered: Vec<(u32, f32)>,
    holding: Option<(f32, Option<Vec2>)>,
}

impl Answers {
    /// How many blows it answered.
    pub fn count(&self) -> usize {
        self.answered.len()
    }

    fn answered(&self, b: &Blow) -> bool {
        self.answered
            .iter()
            .any(|&(by, lands)| by == b.by && (lands - b.lands).abs() < 1e-3)
    }
}

/// An answer fired: the rule's index, the blow it answers.
pub type Fired = (usize, Blow);

/// This frame's asks of the rules: each rule whose window holds `frame` answers the soonest
/// blow told on him (`told`, standing at `here`, `now` s) it has not answered, once it lands
/// within the rule's lead; a held one is asked again each frame until its blow has landed.
/// Returns the asks in rule order, and the answers fired this frame (to log).
pub fn answer(
    rules: &[OnTell],
    (frame, now): (u32, f32),
    told: &[Blow],
    here: Vec2,
    state: &mut Vec<Answers>,
) -> (Vec<(Ability, Option<Vec2>)>, Vec<Fired>) {
    state.resize_with(rules.len(), Answers::default);
    let (mut asks, mut fired) = (Vec::new(), Vec::new());
    for (i, (rule, st)) in rules.iter().zip(state.iter_mut()).enumerate() {
        if let Some((until, at)) = st.holding {
            if now < until {
                asks.push((rule.ability, at));
                continue;
            }
            st.holding = None;
        }
        if !(rule.from..rule.to).contains(&frame) {
            continue;
        }
        let next = told
            .iter()
            .filter(|b| !st.answered(b) && b.lands > now)
            .min_by(|a, b| a.lands.total_cmp(&b.lands));
        let Some(&blow) = next.filter(|b| rule.lead.is_none_or(|l| b.lands - now <= l)) else {
            continue;
        };
        st.answered.push((blow.by, blow.lands));
        let at = rule.at.or_else(|| {
            Some(match rule.ability {
                Ability::Roll => here + (here - blow.from).normalize_or(Vec2::Y) * AWAY,
                _ => blow.from,
            })
        });
        if rule.ability.held() {
            st.holding = Some((blow.lands + HOLD_AFTER, at));
        }
        asks.push((rule.ability, at));
        fired.push((i, blow));
    }
    (asks, fired)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 30.0;

    fn rule(v: &str) -> OnTell {
        OnTell::parse(v).unwrap()
    }

    /// What a run of `frames` asks of `rules`, by frame, with each blow told over the half
    /// second before it lands (as demo/blows.rs's TELL), him at the origin.
    fn asked_on(
        rules: &[OnTell],
        blows: &[Blow],
        frames: u32,
    ) -> Vec<(u32, Ability, Option<Vec2>)> {
        let mut state = Vec::new();
        let mut out = Vec::new();
        for f in 0..frames {
            let now = f as f32 * DT;
            let told: Vec<Blow> = blows
                .iter()
                .filter(|b| now < b.lands && b.lands - now <= 0.5)
                .copied()
                .collect();
            for (a, at) in answer(rules, (f, now), &told, Vec2::ZERO, &mut state).0 {
                out.push((f, a, at));
            }
        }
        out
    }

    #[test]
    fn a_rule_reads_as_a_route_writes_it() {
        assert_eq!(
            rule("roll:11.5,19.8~0.3@1000-1100"),
            OnTell {
                ability: Ability::Roll,
                at: Some(Vec2::new(11.5, 19.8)),
                lead: Some(0.3),
                from: 1000,
                to: 1100
            }
        );
        assert_eq!(rule("block@5-9").lead, None);
        for bad in [
            "fireball@1-2",
            "roll@10",
            "roll:1@1-2",
            "block~x@1-2",
            "block",
        ] {
            let e = OnTell::parse(bad).unwrap_err();
            assert!(e.contains("NAME[:X,Z][~LEAD]@FROM-TO"), "{bad}: {e}");
        }
    }

    /// Whether a press at frame `f` comes `lead` s before a blow landing at `lands`, to a frame.
    fn meets(f: u32, lands: f32, lead: f32) -> bool {
        let before = lands - f as f32 * DT;
        before <= lead + 1e-4 && before > lead - DT
    }

    /// The answer follows the blow, wherever in the window it comes: a press fires once a blow,
    /// LEAD before it lands, whenever that is; nothing outside its window. The fault planted,
    /// a press at the fixed frame the first blow was timed for (the open-loop route, j24's
    /// roll that met nothing once mob 6 swung later), fails the same check on the others.
    #[test]
    fn a_roll_answers_the_blow_whenever_it_comes() {
        let roll = [rule("roll~0.3@30-200")];
        let fixed = ((2.0 - 0.3) / DT).ceil() as u32;
        for lands in [2.0, 3.1, 5.55] {
            let blow = Blow {
                by: 6,
                from: Vec2::new(0.0, 2.0),
                lands,
            };
            let asks = asked_on(&roll, &[blow], 240);
            assert_eq!(asks.len(), 1, "once a blow ({lands} s): {asks:?}");
            let (f, a, at) = asks[0];
            assert_eq!(a, Ability::Roll);
            assert!(
                meets(f, lands, 0.3),
                "fired at frame {f}, not 0.3 s before {lands} s"
            );
            // Unaimed, away from the foe: it stands at +Z, so he rolls toward -Z.
            assert!(at.unwrap().y < -2.0, "{at:?}");
            assert_eq!(
                meets(fixed, lands, 0.3),
                lands == 2.0,
                "the fault: a fixed frame meets only the blow it was timed for"
            );
        }
        let early = Blow {
            by: 1,
            from: Vec2::Y,
            lands: 0.9,
        };
        assert!(
            asked_on(&roll, &[early], 240).is_empty(),
            "told before its window"
        );
        // No lead: as soon as it is told (half a second before, as asked_on tells it).
        let asks = asked_on(&[rule("light@0-200")], &[early], 240);
        assert_eq!(asks.len(), 1);
        assert!(meets(asks[0].0, 0.9, 0.5), "{asks:?}");
        assert_eq!(asks[0].2, Some(Vec2::Y), "a blow unaimed goes at the foe");
    }

    /// Each blow is answered once, two foes' each in turn; the shield is held from LEAD before
    /// its blow, every frame, until just past it lands (so it is up when judged), toward its foe.
    #[test]
    fn a_block_is_held_through_each_blow_it_answers() {
        let block = [rule("block~0.1@0-300")];
        let blows = [
            Blow {
                by: 3,
                from: Vec2::new(2.0, 0.0),
                lands: 2.0,
            },
            Blow {
                by: 4,
                from: Vec2::new(-2.0, 0.0),
                lands: 4.0,
            },
        ];
        let asks = asked_on(&block, &blows, 300);
        for b in blows {
            let frames: Vec<u32> = asks
                .iter()
                .filter(|a| a.2 == Some(b.from))
                .map(|a| a.0)
                .collect();
            let (first, last) = (frames[0], *frames.last().unwrap());
            assert!(
                meets(first, b.lands, 0.1),
                "raised at frame {first} for {} s",
                b.lands
            );
            let held_to = last as f32 * DT;
            assert!(
                held_to >= b.lands && held_to < b.lands + HOLD_AFTER,
                "held to {held_to} s"
            );
            assert_eq!(frames.len() as u32, last - first + 1, "no gap: {frames:?}");
        }
        assert!(asks.iter().all(|a| a.1 == Ability::Block));
    }
}
