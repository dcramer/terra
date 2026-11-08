//! Mobs that are never stuck (iso-forest-demo; the developer, 11-08: "the monsters are
//! kinda getting stuck"). Each foe of the biome's roster is a body that walks to a slot on a
//! ring about the hero along the nav grid's path (nav.rs, its props' footprints blocked
//! too), steered as Craig Reynolds' "Steering Behaviors For Autonomous Characters" (GDC
//! 1999) has it: seek and arrive on the path's next corner, separation from the others and
//! the hero, obstacle avoidance of the footprint circles ahead. The slots are the melee
//! action games' attack slots (a ring of places about the player, each held by one
//! attacker, so they flank him rather than queue behind one another); a second ring
//! holds those waiting their turn. A body that covers no ground for `PATIENCE` while it
//! wants to move plans again and sidesteps; twice, and it gives its slot up for another.
//!
//! This is the plain part, no Bevy world: `step` moves every mob one tick, and the test
//! plays a scripted fight on it. demo/mobs.rs puts it on the bodies.

use crate::nav::Nav;
use bevy::math::Vec2;

/// A walking creature's pace (m/s; the treants' walk clip strides about this).
pub const SPEED: f32 = 1.8;
/// A body's radius on the ground (m), as play's footprint gives a creature one circle.
pub const RADIUS: f32 = 0.45;
/// The attack ring's radius about the hero (m), inside his swing's `REACH` (play.rs), and
/// the waiting ring's.
pub const RING: f32 = 1.8;
pub const WAIT: f32 = 3.8;
const INNER: usize = 6;
const OUTER: usize = 10;
/// Near enough its slot to stand (m).
pub const ARRIVE: f32 = 0.3;
/// A path's corner counts as passed this near (m).
const CORNER: f32 = 0.4;
/// Planned again this often (s), or when its slot moved this far (m).
const REPLAN: f32 = 0.5;
const MOVED: f32 = 0.8;
/// No ground covered for this long while wanting to move is stuck (s), "ground" being a
/// `STRIDE` from where it stood when the clock started (m), so jitter on the spot counts.
pub const PATIENCE: f32 = 0.5;
pub const STRIDE: f32 = 0.25;
/// How long a sidestep lasts (s), how long a given-up slot stays given up.
const SIDESTEP: f32 = 0.45;
const BANNED: f32 = 2.0;
/// How fast a body takes up its wanted velocity (1/s), and the look ahead for obstacles (m).
const ACCEL: f32 = 10.0;
const AHEAD: f32 = 1.6;

/// What a mob's brain does; all of it in play. The test plants a fault by switching one off.
#[derive(Clone, Copy, Debug)]
pub struct Rules {
    /// Follow the nav grid's path (else seek the slot straight).
    pub path: bool,
    /// Keep apart from the others and the hero.
    pub separate: bool,
    /// Plan again and sidestep after `PATIENCE` without ground covered.
    pub recover: bool,
}

impl Rules {
    pub const ALL: Rules = Rules {
        path: true,
        separate: true,
        recover: true,
    };
}

/// One body's steering state.
#[derive(Clone, Debug)]
pub struct Mob {
    pub at: Vec2,
    pub vel: Vec2,
    /// Its slot (`slot_at`), once given one.
    pub slot: Option<usize>,
    /// Stands still until then (s): a blow's stagger.
    pub hold: f32,
    path: Vec<Vec2>,
    planned: Option<(Vec2, f32)>,
    /// Where it stood when the stuck clock started, and when.
    anchor: (Vec2, f32),
    tries: u32,
    sidestep: Option<(Vec2, f32)>,
    banned: Option<(usize, f32)>,
}

impl Mob {
    pub fn new(at: Vec2) -> Mob {
        Mob {
            at,
            vel: Vec2::ZERO,
            slot: None,
            hold: f32::NEG_INFINITY,
            path: Vec::new(),
            planned: None,
            anchor: (at, 0.0),
            tries: 0,
            sidestep: None,
            banned: None,
        }
    }

    /// Where it is going, if it wants to move: its slot, while not held and not there.
    pub fn wants(&self, hero: Vec2, now: f32) -> Option<Vec2> {
        let goal = slot_at(hero, self.slot?);
        (now >= self.hold && self.at.distance(goal) > ARRIVE).then_some(goal)
    }
}

/// What the bodies walk round: the nav grid (trees, the pond; `with_circles` adds the
/// footprints), the footprints' circles, and the layout's ground (play.rs `Ground::open`).
pub struct Field<'a> {
    pub nav: Option<&'a Nav>,
    pub circles: &'a [(Vec2, f32)],
    pub open: &'a dyn Fn(Vec2) -> bool,
}

/// The nav grid with every footprint circle blocked as well, padded to a mob's body
/// (Nav pads by a person's 0.3 m), so a path goes round a fallen log, not into it.
pub fn with_circles(nav: &Nav, circles: &[(Vec2, f32)]) -> Nav {
    let mut out = nav.clone();
    for &(c, r) in circles {
        out.block_circle(c, r + RADIUS - 0.3);
    }
    out
}

impl Field<'_> {
    /// Whether a body may stand at `p`.
    pub fn stands(&self, p: Vec2) -> bool {
        (self.open)(p)
            && self.nav.is_none_or(|n| n.open_at(p))
            && self
                .circles
                .iter()
                .all(|&(c, r)| p.distance(c) >= r + RADIUS)
    }

    /// Whether a step from `here` to `to` is refused: off the ground, or into (deeper into,
    /// for a body already in one) a blocked cell or a circle.
    fn refuses(&self, here: Vec2, to: Vec2) -> bool {
        if !(self.open)(to) {
            return true;
        }
        if let Some(n) = self.nav
            && !n.open_at(to)
            && n.open_at(here)
        {
            return true;
        }
        self.circles.iter().any(|&(c, r)| {
            let r = r + RADIUS;
            to.distance(c) < r && to.distance(c) < here.distance(c)
        })
    }

    fn path(&self, from: Vec2, to: Vec2, rules: Rules) -> Option<Vec<Vec2>> {
        match self.nav {
            Some(n) if rules.path => n.path(from, to),
            _ => Some(vec![to]),
        }
    }
}

/// Slot `k`'s place about the hero: the first `INNER` on the attack ring, the rest on the
/// waiting ring between them.
pub fn slot_at(hero: Vec2, k: usize) -> Vec2 {
    let (r, n, k, turn) = if k < INNER {
        (RING, INNER, k, 0.0)
    } else {
        (WAIT, OUTER, k - INNER, 0.5)
    };
    let a = (k as f32 + turn) / n as f32 * std::f32::consts::TAU;
    hero + Vec2::new(libm::cosf(a), libm::sinf(a)) * r
}

/// Gives each mob a slot: nearest the hero first, each the free one nearest it that a body
/// can stand on, the attack ring before the waiting ring; a mob keeps its own unless another
/// is much nearer it (so they do not trade places every tick).
fn assign(mobs: &mut [Mob], hero: Vec2, field: &Field, active: &[bool], now: f32) {
    let mut order: Vec<usize> = (0..mobs.len()).filter(|&i| active[i]).collect();
    order.sort_by(|&a, &b| {
        let d = |i: usize| mobs[i].at.distance(hero);
        d(a).total_cmp(&d(b))
    });
    let mut taken = [false; INNER + OUTER];
    for &i in &order {
        let m = &mobs[i];
        let banned = m.banned.filter(|b| now < b.1).map(|b| b.0);
        let cost = |k: usize| {
            let p = slot_at(hero, k);
            let ring = if k < INNER { 0.0 } else { 6.0 };
            let keep = if m.slot == Some(k) { -0.8 } else { 0.0 };
            m.at.distance(p) + ring + keep
        };
        let best = (0..INNER + OUTER)
            .filter(|&k| !taken[k] && Some(k) != banned && field.stands(slot_at(hero, k)))
            .min_by(|&a, &b| cost(a).total_cmp(&cost(b)));
        if let Some(k) = best {
            taken[k] = true;
        }
        let m = &mut mobs[i];
        if m.slot != best {
            m.path.clear();
            m.planned = None;
        }
        m.slot = best;
    }
}

/// One tick of `dt` at time `now` for every mob that is `active` (not fallen) about the hero.
pub fn step(
    mobs: &mut [Mob],
    active: &[bool],
    hero: Vec2,
    field: &Field,
    rules: Rules,
    now: f32,
    dt: f32,
) {
    assign(mobs, hero, field, active, now);
    let places: Vec<(Vec2, bool)> = mobs.iter().zip(active).map(|(m, a)| (m.at, *a)).collect();
    for (i, m) in mobs.iter_mut().enumerate() {
        if !active[i] {
            m.vel = Vec2::ZERO;
            // Its stuck clock starts when it moves again (a pack woken late is not stuck).
            m.anchor = (m.at, now);
            continue;
        }
        let goal = m.wants(hero, now);
        let mut want = Vec2::ZERO;
        if let Some(goal) = goal {
            let stale = m
                .planned
                .is_none_or(|(g, t)| g.distance(goal) > MOVED || now - t > REPLAN);
            if m.path.is_empty() || stale {
                match field.path(m.at, goal, rules) {
                    Some(p) => m.path = p,
                    None => {
                        // Nowhere to go from here: that slot is given up.
                        m.banned = m.slot.map(|k| (k, now + BANNED));
                        m.path.clear();
                    }
                }
                m.planned = Some((goal, now));
            }
            while m.path.len() > 1 && m.at.distance(m.path[0]) < CORNER {
                m.path.remove(0);
            }
            if let Some(&next) = m.path.first() {
                let to = next - m.at;
                let last = m.path.len() == 1;
                // Arrive: slowing over its last metre.
                let pace = if last {
                    (to.length() / 1.0).min(1.0)
                } else {
                    1.0
                };
                want = to.normalize_or_zero() * SPEED * pace.max(0.35);
            }
            if let Some((side, until)) = m.sidestep {
                if now < until {
                    want = (side * 0.8 + want.normalize_or_zero() * 0.4) * SPEED;
                } else {
                    m.sidestep = None;
                }
            }
            want += avoid(m.at, want, field.circles);
        }
        if rules.separate {
            want += separate(i, m.at, &places, hero) * if goal.is_some() { 1.0 } else { 0.6 };
        }
        let k = (ACCEL * dt).min(1.0);
        m.vel = (m.vel + (want - m.vel) * k).clamp_length_max(SPEED * 1.2);
        m.at = slide(field, m.at, m.vel * dt);
        // Stuck: no stride covered for `PATIENCE` while wanting to move.
        if goal.is_none() || m.at.distance(m.anchor.0) >= STRIDE {
            if goal.is_none() {
                m.tries = 0;
            }
            m.anchor = (m.at, now);
        } else if rules.recover && now - m.anchor.1 > PATIENCE {
            m.tries += 1;
            m.path.clear();
            m.planned = None;
            // Off to the side of the way it wanted, turning the other way each try.
            let way = goal.map_or(Vec2::X, |g| (g - m.at).normalize_or(Vec2::X));
            let sign = if (m.tries + i as u32).is_multiple_of(2) {
                1.0
            } else {
                -1.0
            };
            m.sidestep = Some((way.perp() * sign, now + SIDESTEP));
            if m.tries >= 2 {
                m.banned = m.slot.map(|k| (k, now + BANNED));
                m.tries = 0;
            }
            m.anchor = (m.at, now);
        }
    }
}

/// Reynolds' obstacle avoidance: each circle ahead within `AHEAD` along the way it wants,
/// whose edge (padded by its body) the way passes inside, turns it aside, harder the nearer.
fn avoid(at: Vec2, want: Vec2, circles: &[(Vec2, f32)]) -> Vec2 {
    let Some(ahead) = want.try_normalize() else {
        return Vec2::ZERO;
    };
    let mut push = Vec2::ZERO;
    for &(c, r) in circles {
        let d = c - at;
        let along = d.dot(ahead);
        let r = r + RADIUS + 0.1;
        if along <= 0.0 || along > AHEAD + r {
            continue;
        }
        let side = d - ahead * along;
        if side.length() >= r {
            continue;
        }
        let away = (-side).normalize_or(ahead.perp());
        push += away * SPEED * (1.0 - along / (AHEAD + r));
    }
    push
}

/// Separation: away from every other body nearer than two bodies and a gap, and from the
/// hero nearer than the attack ring's inside.
fn separate(i: usize, at: Vec2, places: &[(Vec2, bool)], hero: Vec2) -> Vec2 {
    let mut push = Vec2::ZERO;
    let mut from = |p: Vec2, range: f32, w: f32| {
        let d = at - p;
        let n = d.length();
        if n < range {
            let away = d.try_normalize().unwrap_or(Vec2::from_angle(i as f32));
            push += away * (1.0 - n / range) * SPEED * w;
        }
    };
    for (j, &(p, alive)) in places.iter().enumerate() {
        if j != i && alive {
            from(p, RADIUS * 2.0 + 0.35, 1.5);
        }
    }
    from(hero, RING - 0.3, 2.0);
    push
}

/// The step taken: the one wanted if the ground takes it, else the nearest turn of it (up to
/// a right angle either way) that does, sliding along what is in the way; else none.
fn slide(field: &Field, at: Vec2, step: Vec2) -> Vec2 {
    if step.length_squared() < 1e-10 {
        return at;
    }
    for deg in [0.0f32, 30.0, -30.0, 60.0, -60.0, 90.0, -90.0] {
        let s = Vec2::from_angle(deg.to_radians()).rotate(step);
        if !field.refuses(at, at + s) {
            return at + s;
        }
    }
    at
}

/// The longest time any active mob spent wanting to move without covering a `STRIDE` (s),
/// and which, over a run: what the test asks of a fight. Called each tick after `step`.
#[derive(Default)]
pub struct Stalls {
    since: Vec<Option<(Vec2, f32)>>,
    pub worst: (f32, usize),
}

impl Stalls {
    pub fn watch(&mut self, mobs: &[Mob], active: &[bool], hero: Vec2, now: f32) {
        self.since.resize(mobs.len(), None);
        for (i, m) in mobs.iter().enumerate() {
            let moving = active[i] && m.wants(hero, now).is_some();
            let s = &mut self.since[i];
            match *s {
                Some((p, t)) if moving && m.at.distance(p) < STRIDE => {
                    if now - t > self.worst.0 {
                        self.worst = (now - t, i);
                    }
                }
                _ => *s = moving.then_some((m.at, now)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 30.0;

    /// A forest's patch: oaks in the nav grid, a fallen log across the way, a rock, and a
    /// pocket of three logs open away from the hero (a steering trap: walking straight at
    /// him from inside it meets its closed end).
    fn wood() -> (Nav, Vec<(Vec2, f32)>) {
        let mut nav = Nav::default();
        for t in [
            Vec2::new(3.0, 4.0),
            Vec2::new(-4.0, 2.5),
            Vec2::new(6.0, -3.0),
        ] {
            nav.block_circle(t, 0.5);
        }
        let mut circles = Vec::new();
        // The log: 4 m along x at z = 5.
        for k in 0..8 {
            circles.push((Vec2::new(-2.0 + k as f32 * 0.5, 5.0), 0.3));
        }
        circles.push((Vec2::new(-5.0, -4.0), 0.7));
        // The pocket about (8, 8): closed toward the hero (its -z side and its two flanks).
        for k in 0..7 {
            circles.push((Vec2::new(6.5 + k as f32 * 0.5, 6.5), 0.3));
        }
        for k in 1..6 {
            circles.push((Vec2::new(6.5, 6.5 + k as f32 * 0.5), 0.3));
            circles.push((Vec2::new(9.5, 6.5 + k as f32 * 0.5), 0.3));
        }
        (with_circles(&nav, &circles), circles)
    }

    /// Plays a scripted fight: the hero stands, then walks east 8 m, then back north-west
    /// and stands; the mobs start about the wood (one behind the log, one in the pocket,
    /// two side by side, one behind a tree); one is felled at 9 s and one staggered at 6 s.
    /// Gives the longest stall and the mobs at the end.
    fn fight(rules: Rules) -> ((f32, usize), Vec<Mob>, Vec<bool>, Vec2) {
        let (nav, circles) = wood();
        let inside = |p: Vec2| p.abs().max_element() < 20.0;
        let field = Field {
            nav: Some(&nav),
            circles: &circles,
            open: &inside,
        };
        let starts = [
            Vec2::new(0.0, 7.5),
            Vec2::new(8.0, 8.0),
            Vec2::new(-9.0, -1.0),
            Vec2::new(-9.0, 0.2),
            Vec2::new(3.0, 6.0),
            Vec2::new(-6.0, -6.5),
            Vec2::new(10.0, -8.0),
        ];
        let mut mobs: Vec<Mob> = starts.iter().map(|&p| Mob::new(p)).collect();
        let mut active = vec![true; mobs.len()];
        let mut stalls = Stalls::default();
        let hero_at = |t: f32| {
            let walk = |a: Vec2, b: Vec2, t0: f32, t1: f32| {
                a.lerp(b, ((t - t0) / (t1 - t0)).clamp(0.0, 1.0))
            };
            if t < 8.0 {
                walk(Vec2::ZERO, Vec2::new(8.0, 0.0), 4.0, 8.0)
            } else {
                walk(Vec2::new(8.0, 0.0), Vec2::new(4.0, -1.0), 12.0, 15.0)
            }
        };
        let mut hero = Vec2::ZERO;
        for n in 0..(24.0 / DT) as usize {
            let now = n as f32 * DT;
            hero = hero_at(now);
            if (now - 6.0).abs() < DT / 2.0 {
                mobs[2].hold = now + 0.6;
            }
            if (now - 9.0).abs() < DT / 2.0 {
                active[3] = false;
            }
            step(&mut mobs, &active, hero, &field, rules, now, DT);
            stalls.watch(&mobs, &active, hero, now);
        }
        (stalls.worst, mobs, active, hero)
    }

    /// In a scripted fight no mob stalls for more than a second while it wants to move, and
    /// by its end every standing one holds its own slot, the attack ring's first, none in a
    /// log, a tree or another. Its planted fault (D632): seeking straight with no path and
    /// no recovery leaves the pocket's mob against its closed end.
    #[test]
    fn no_mob_stalls_in_a_fight() {
        let ((worst, who), mobs, active, hero) = fight(Rules::ALL);
        assert!(
            worst <= 1.0,
            "mob {who} stalled {worst:.2} s wanting to move"
        );
        let (nav, circles) = wood();
        let mut slots = Vec::new();
        for (i, m) in mobs.iter().enumerate().filter(|(i, _)| active[*i]) {
            let k = m.slot.expect("a slot");
            assert!(!slots.contains(&k), "mob {i} shares slot {k}");
            slots.push(k);
            assert!(
                m.at.distance(slot_at(hero, k)) < 0.6,
                "mob {i} {:.1} m from its slot",
                m.at.distance(slot_at(hero, k))
            );
            assert!(
                nav.open_at(m.at),
                "mob {i} stands in a blocked cell at {}",
                m.at
            );
            for &(c, r) in &circles {
                assert!(
                    m.at.distance(c) > r + RADIUS * 0.8,
                    "mob {i} in a log at {}",
                    m.at
                );
            }
            for (j, o) in mobs.iter().enumerate() {
                if j != i && active[j] {
                    assert!(
                        m.at.distance(o.at) > RADIUS * 1.5,
                        "mobs {i} and {j} overlap"
                    );
                }
            }
        }
        // Six standing: the attack ring full, none left waiting.
        assert!(slots.iter().all(|&k| k < INNER), "{slots:?}");
        let ((worst, _), ..) = fight(Rules {
            path: false,
            separate: true,
            recover: false,
        });
        assert!(
            worst > 1.0,
            "the straight seeker never stalled ({worst:.2} s)"
        );
    }

    /// Slots spread round the hero: mobs coming from one side take the ring's places on
    /// both his flanks, not a queue behind the first.
    #[test]
    fn mobs_from_one_side_flank_him() {
        let inside = |_: Vec2| true;
        let field = Field {
            nav: None,
            circles: &[],
            open: &inside,
        };
        let mut mobs: Vec<Mob> = (0..4)
            .map(|k| Mob::new(Vec2::new(-1.5 + k as f32, 9.0)))
            .collect();
        let active = vec![true; 4];
        for n in 0..(12.0 / DT) as usize {
            step(
                &mut mobs,
                &active,
                Vec2::ZERO,
                &field,
                Rules::ALL,
                n as f32 * DT,
                DT,
            );
        }
        let (mut left, mut right) = (false, false);
        for m in &mobs {
            assert!(m.at.length() < RING + 0.5, "one waits at {}", m.at);
            left |= m.at.x < -0.8;
            right |= m.at.x > 0.8;
        }
        assert!(
            left && right,
            "{:?}",
            mobs.iter().map(|m| m.at).collect::<Vec<_>>()
        );
    }

    /// A mob held out of play (a pack not yet woken) walks straight to him when it wakes:
    /// the time it stood is not counted as stuck, so it neither sidesteps nor replans.
    #[test]
    fn a_mob_woken_late_is_not_stuck() {
        let inside = |_: Vec2| true;
        let field = Field {
            nav: None,
            circles: &[],
            open: &inside,
        };
        let mut mobs = vec![Mob::new(Vec2::new(0.0, 9.0))];
        let mut now = 0.0;
        for active in [false, true] {
            for _ in 0..(5.0 / DT) as usize {
                step(
                    &mut mobs,
                    &[active],
                    Vec2::ZERO,
                    &field,
                    Rules::ALL,
                    now,
                    DT,
                );
                now += DT;
                if active {
                    assert!(mobs[0].sidestep.is_none(), "it sidestepped at {now:.2} s");
                }
            }
        }
        assert!(
            mobs[0].at.length() < RING + 0.5,
            "it stands at {}",
            mobs[0].at
        );
    }
}
