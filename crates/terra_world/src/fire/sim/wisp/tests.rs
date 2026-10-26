use super::*;
use std::collections::BTreeSet;

/// A loop whose every frame lists `tips` tips at 0.8 m, 10 cm apart down its wind, the gas
/// rising at 4 m/s and blowing 1 m/s downwind there.
fn sim(tips: usize) -> Sim {
    Sim {
        frames: 240,
        fps: 30.0,
        cells: [40, 36, 52],
        columns: 12,
        rows: 20,
        size: [480, 720, 52],
        box_min: [-0.4, -0.4, -0.02],
        box_size: [0.9, 0.8, 1.2],
        tips: (0..240)
            .map(|_| {
                (0..tips)
                    .map(|i| [0.1 * i as f32, 0.0, 0.8, 1.0, 0.0, 4.0])
                    .collect()
            })
            .collect(),
    }
}

fn placed(turn: f32, scale: f32) -> Placement {
    Placement {
        offset: 1.3,
        turn,
        mirror: -1.0,
        scale,
    }
}

/// Every wisp a lane sheds whose life starts in [from, to).
fn births(s: &Sim, p: Placement, seed: f32, from: f32, to: f32) -> BTreeSet<(u32, u32)> {
    let mut out = BTreeSet::new();
    for lane in 0..WISPS as u32 {
        for k in (from / period()) as i64 - 1..=(to / period()) as i64 {
            if let Some(w) =
                born(s, p, seed, Vec2::ZERO, lane, k).filter(|w| w.born >= from && w.born < to)
            {
                out.insert((lane, w.born.to_bits()));
            }
        }
    }
    out
}

#[test]
fn a_fire_sheds_its_rate_from_each_tip_its_frame_lists() {
    for tips in 0..=3 {
        let n = births(&sim(tips), placed(0.0, 1.0), 0.37, 0.0, 600.0).len();
        let rate = n as f32 / 600.0;
        let want = TIP_RATE * tips as f32;
        assert!(
            (rate - want).abs() <= 0.08 * want.max(1.0),
            "{tips} tips shed {rate:.2} a second, not {want:.2}"
        );
    }
}

#[test]
fn every_wisp_is_found_alive_through_its_life_and_a_lane_has_one_at_a_time() {
    let (s, p) = (sim(3), placed(0.4, 1.2));
    let want = births(&s, p, 0.81, 0.0, 30.0);
    let mut seen = BTreeSet::new();
    for step in 0..15_100 {
        let t = step as f32 * 0.002;
        for (lane, w) in alive(&s, p, 0.81, Vec2::ZERO, t).iter().enumerate() {
            if let Some(w) = w {
                assert!(t >= w.born && t < w.born + w.life);
                if w.born < 30.0 {
                    seen.insert((lane as u32, w.born.to_bits()));
                }
            }
        }
    }
    // A wisp lives 0.04 s at least, so 2 ms steps meet each one.
    assert_eq!(seen, want);
    assert!(
        LIFE_S[1] * libm::sqrtf(2.0) < period(),
        "a lane's wisps would overlap"
    );
}

#[test]
fn neighbouring_fires_shed_apart() {
    let s = sim(3);
    let a: Vec<u32> = births(&s, placed(0.0, 1.0), 0.11, 0.0, 60.0)
        .into_iter()
        .map(|(_, b)| b)
        .collect();
    let b: Vec<u32> = births(&s, placed(0.0, 1.0), 0.12, 0.0, 60.0)
        .into_iter()
        .map(|(_, b)| b)
        .collect();
    let same = a.iter().filter(|t| b.contains(t)).count();
    assert!(a.len() > 100 && b.len() > 100 && same == 0);
    // Nor at a beat: the gaps between one fire's births spread over much of a period.
    let mut times: Vec<f32> = a.iter().map(|&t| f32::from_bits(t)).collect();
    times.sort_by(f32::total_cmp);
    let gaps: Vec<f32> = times.windows(2).map(|w| w[1] - w[0]).collect();
    let (least, most) = gaps
        .iter()
        .fold((f32::MAX, 0.0f32), |(l, m), &g| (l.min(g), m.max(g)));
    assert!(
        least < 0.1 * period() && most > 0.5 * period(),
        "gaps {least:.3}-{most:.3} s"
    );
}

#[test]
fn a_wisp_leaves_its_tip_at_the_gas_speed_and_thins_through_the_edge_as_it_dies() {
    let s = sim(1);
    let turn = core::f32::consts::FRAC_PI_2;
    let (scale, p) = (1.44, placed(turn, 1.44));
    let w = (0..200)
        .find_map(|k| born(&s, p, 0.5, Vec2::ZERO, 0, k))
        .expect("a lane sheds");
    // The tip is 0.8 m up at the bake's middle; the bake's 4 m/s up and 1 m/s downwind are the
    // fire's at its scale times its loop's rate (1.2 at 1.44): 4.8 up, 1.2 down the wind, which
    // blows north at this turn.
    assert!(w.from.distance(Vec3::new(0.0, 0.8 * scale, 0.0)) < 1e-5);
    assert!(
        w.velocity.distance(Vec3::new(0.0, 4.8, 1.2)) < 1e-4,
        "{}",
        w.velocity
    );
    // It lives a filmed tongue's life, at the loop's pace.
    assert!(w.life >= LIFE_S[0] * 1.2 - 1e-6 && w.life <= LIFE_S[1] * 1.2 + 1e-6);
    let at = |t: f32| w.drawn(w.born + t * w.life, scale, &s);
    let [mid, axis] = at(0.0).expect("drawn at its birth");
    assert!(mid.truncate().distance(w.from) < 1e-5 && (mid.w - FLAME).abs() < 1e-6);
    // Along its way, reaching further than it is wide.
    assert!(axis.truncate().normalize().dot(w.velocity.normalize()) > 0.999);
    assert!(axis.truncate().length() > axis.w);
    let [half, half_axis] = at(0.5).expect("drawn halfway");
    assert!(half.truncate().distance(w.from + w.velocity * 0.5 * w.life) < 1e-4);
    // A burning pocket: its flame mostly held halfway through its life, as it shrinks.
    assert!(half.w > FLAME * 0.7 && half.w < FLAME && half_axis.w < axis.w);
    // Its flame falls through the drawn edge (0.08) before it is gone, and then it is.
    let late = at(0.97).expect("drawn near its end");
    assert!(late[0].w < 0.08);
    assert!(at(1.01).is_none());
}

#[test]
fn only_the_wind_beyond_the_bakes_carries_a_wisp_on() {
    let s = sim(1);
    let p = placed(0.0, 1.0);
    let first = |wind: Vec2| {
        (0..200)
            .find_map(|k| born(&s, p, 0.5, wind, 0, k))
            .expect("a lane sheds")
    };
    let calm = first(Vec2::ZERO);
    assert_eq!(first(Vec2::new(1.5, 0.0)).velocity, calm.velocity);
    let blown = first(Vec2::new(3.0, 0.0)).velocity - calm.velocity;
    assert!(blown.distance(Vec3::new(1.2, 0.0, 0.0)) < 1e-5, "{blown}");
}

#[test]
fn a_falling_tip_sheds_nothing_and_no_wisp_is_drawn_past_the_box() {
    let mut s = sim(1);
    for f in &mut s.tips {
        f[0][5] = 0.2;
    }
    assert!(births(&s, placed(0.0, 1.0), 0.5, 0.0, 60.0).is_empty());
    let w = Wisp {
        born: 0.0,
        life: 0.1,
        from: Vec3::new(0.0, 1.7, 0.0),
        velocity: Vec3::new(0.0, 4.0, 0.0),
    };
    // The box tops out at 1.18 m, the wisps 0.6 over it.
    assert!(w.drawn(0.0, 1.0, &s).is_none());
    assert!(w.drawn(0.0, 1.5, &s).is_some());
}

#[test]
fn the_bake_lists_rising_tongue_tips_in_its_box() {
    let s = Sim::read();
    assert_eq!(s.tips.len(), s.frames as usize);
    let lo = Vec3::from(s.box_min);
    let hi = lo + Vec3::from(s.box_size);
    let mut up = Vec::new();
    for frame in &s.tips {
        assert!(!frame.is_empty() && frame.len() <= TIPS);
        for pair in frame.windows(2) {
            assert!(pair[0][2] >= pair[1][2], "highest first");
        }
        for t in frame {
            let at = Vec3::new(t[0], t[1], t[2]);
            assert!(
                at.cmpge(lo).all() && at.cmple(hi).all(),
                "{at} outside the box"
            );
            up.push(t[5]);
        }
    }
    up.sort_by(f32::total_cmp);
    // The flame's gas rises at 3.2-3.4 m/s (g23), faster still at its tips.
    let median = up[up.len() / 2];
    assert!((3.0..9.0).contains(&median), "tips rise at {median} m/s");
}
