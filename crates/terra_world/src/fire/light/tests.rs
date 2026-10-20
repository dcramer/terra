use super::*;

/// A signal sampled over twenty seconds at a millisecond: its least and most, its mean, and
/// how many times a second it turns from rising to falling (twice its puffs).
fn measured(seed: f32, flicker: Flicker) -> (f32, f32, f32, f32) {
    let samples: Vec<f32> = (0..20_000)
        .map(|i| signal(seed, i as f32 * 0.001, flicker))
        .collect();
    let (low, high) = samples
        .iter()
        .fold((f32::MAX, f32::MIN), |(l, h), &w| (l.min(w), h.max(w)));
    let mean = samples.iter().sum::<f32>() / samples.len() as f32;
    let turns = samples
        .windows(3)
        .filter(|w| (w[1] - w[0]) * (w[2] - w[1]) < 0.0)
        .count();
    (low, high, mean, turns as f32 / 20.0)
}

#[test]
fn a_torch_flickers_deep_and_quick_a_campfire_less_a_house_fire_slow_and_shallow() {
    // docs/fire.md: a torch by 0.3-0.4 over 0.3-0.4 s, a campfire about 15%, a house slower
    // and shallower.
    let torch = Flicker::of(0.1);
    let campfire = Flicker::of(0.7);
    let house = Flicker::of(6.0);
    assert!((0.3..=0.4).contains(&torch.depth), "{torch:?}");
    assert!((0.12..=0.18).contains(&campfire.depth), "{campfire:?}");
    assert!(house.depth < campfire.depth && house.puffs_hz < campfire.puffs_hz);
    assert!(campfire.puffs_hz < torch.puffs_hz);
    for (flicker, seed) in [(torch, 0.1), (campfire, 0.5), (house, 0.9)] {
        let (low, high, mean, turns) = measured(seed, flicker);
        // Never past its depth, and reaching most of it.
        assert!(low >= 1.0 - flicker.depth - 1e-4 && high <= 1.0 + flicker.depth + 1e-4);
        assert!(
            high - low > flicker.depth * 1.4,
            "{flicker:?}: {low} {high}"
        );
        // Wavering about its mean, neither dimmed nor brightened on the whole.
        assert!(
            (mean - 1.0).abs() < flicker.depth * 0.15,
            "{flicker:?}: {mean}"
        );
        // Puffing at about its rate: two turns a puff, give or take, and some noise.
        let puffs = turns / 2.0;
        assert!(
            puffs > flicker.puffs_hz * 0.6 && puffs < flicker.puffs_hz * 2.0,
            "{flicker:?}: {puffs} puffs a second"
        );
    }
}

#[test]
fn two_fires_do_not_flicker_together() {
    let flicker = Flicker::of(0.7);
    let a: Vec<f32> = (0..20_000)
        .map(|i| signal(0.13, i as f32 * 0.001, flicker) - 1.0)
        .collect();
    let b: Vec<f32> = (0..20_000)
        .map(|i| signal(0.71, i as f32 * 0.001, flicker) - 1.0)
        .collect();
    let dot = |x: &[f32], y: &[f32]| x.iter().zip(y).map(|(p, q)| p * q).sum::<f32>();
    let correlation = dot(&a, &b) / (dot(&a, &a) * dot(&b, &b)).sqrt();
    assert!(correlation.abs() < 0.3, "{correlation}");
}

#[test]
fn a_fires_light_sways_on_all_three_axes_within_its_reach() {
    let flicker = Flicker::of(0.7);
    let reach = 0.1;
    let mut spread = Vec3::ZERO;
    for i in 0..20_000 {
        let off = sway(0.4, i as f32 * 0.001, flicker, reach);
        assert!(off.abs().max_element() <= reach + 1e-5, "{off}");
        spread += off * off;
    }
    let spread = spread / 20_000.0;
    let most = spread.max_element();
    assert!(spread.min_element() > most * 0.3, "{spread}");
}

#[test]
fn a_campfires_light_flickers_with_its_flames_lights_round_it_and_a_roof_reaches_farther() {
    let ground = Vec3::new(0.0, 2000.0, 0.0);
    let campfire = Blaze {
        at: ground,
        tall: 1.5,
        heat: 0.9,
        seed: 0.3,
        wide: 0.7,
    };
    let blazes = Blazes {
        kept: vec![campfire],
        blazes: vec![Blaze {
            at: ground + Vec3::X * 40.0,
            tall: 1.5,
            heat: 0.9,
            seed: 0.6,
            wide: 0.0,
        }],
        ..default()
    };
    let mut seen = (f32::MAX, f32::MIN);
    let (mut low, mut high) = (f32::MAX, f32::MIN);
    for i in 0..400 {
        let t = i as f32 * 0.05;
        let lit = lit(&blazes, t);
        assert_eq!(lit.len(), 3);
        let (hearth, fill, roof) = (&lit[0], &lit[1], &lit[2]);
        // One light casts shadows and carries most of it, so the ring of stones shadows the
        // ground outside it (`HEARTH_FILL`); a fill casts none, hung high over the fire so it
        // lights the ground metres out more, and the stones at its foot less, than the
        // shadowed light does for each lumen (`HEARTH_FILL_UP`).
        assert!(!hearth.shadowless && fill.shadowless);
        assert!(fill.lumens < hearth.lumens && fill.at.y > hearth.at.y + 0.5);
        let pool = |l: &Lit| on_ground(l, ground, 3.0, 1.0) / on_ground(l, ground, 0.55, 1.0);
        assert!(
            pool(fill) > 2.0 * pool(hearth),
            "{} {}",
            pool(fill),
            pool(hearth)
        );
        // Low in the body of its flames, swayed a hand's breadth at most, so a ring stone's face
        // toward the fire takes about as much of it as its top (`HEARTH_LIGHT_UP`), not over the
        // heads of those sat by it.
        let off = hearth.at - ground;
        assert!(off.y > 0.4 && off.y < 1.0 && off.length() < 1.3, "{off}");
        let stone = Vec2::new(off.xz().length() + 0.55, off.y - 0.08);
        assert!(stone.x > 0.8 * stone.y, "{stone}");
        assert!(hearth.range <= 15.0 && roof.range > hearth.range);
        // Its light is its flames' flicker: the signal the shaders are given for the fire.
        assert!((hearth.glow - glow(&campfire, t)).abs() < 1e-6);
        seen = (seen.0.min(hearth.at.x), seen.1.max(hearth.at.x));
        // It never sways below where it hangs, into the logs' ends, whose shadow from a light
        // under them balloons over the ground (fire round 30).
        let hangs = 1.5 * HEARTH_LIGHT_UP;
        assert!(off.y >= hangs - 1e-4, "{} under {hangs}", off.y);
        low = low.min(off.y);
        high = high.max(off.y);
        // Its shadows and fall-off soften as a disc as wide as its flames (`crate::falloff`),
        // no wider, or its fall-off levels off past its stones and there is no pool.
        assert!(hearth.radius >= campfire.wide * 0.9 && hearth.radius <= campfire.wide * 1.5);
    }
    // It moves, so the shadows it casts sway, up and down as well as across.
    assert!(seen.1 - seen.0 > 0.04, "{seen:?}");
    assert!(high - low > 0.04, "{low} {high}");
}

/// A light of so many lumens at a place.
fn at(x: f32, lumens: f32) -> Lit {
    Lit {
        at: Vec3::new(x, 0.0, 0.0),
        lumens,
        color: Color::WHITE,
        range: 20.0,
        radius: 0.5,
        glow: 1.0,
        kept: false,
        shadowless: false,
    }
}

fn none(n: usize) -> Vec<Was> {
    vec![
        Was {
            at: Vec3::ZERO,
            on: false,
            shadows: false
        };
        n
    ]
}

#[test]
fn a_burning_village_is_a_dozen_lights_the_nearest_shadowed_none_past_forty_metres() {
    // A hundred fires, one every metre and a half along a street from the eye.
    let lit: Vec<Lit> = (0..100).map(|i| at(1.0 + i as f32 * 1.5, 1.0e6)).collect();
    let shown = choose(lit, Some(Vec3::ZERO), &none(LIGHTS), 2);
    let on: Vec<&Shown> = shown.iter().flatten().collect();
    assert_eq!(on.len(), LIGHTS);
    // The nearest are lit, none past 40 m, and the farthest shown is eased.
    assert!(on.iter().all(|s| s.lit.at.x < LIGHTS as f32 * 1.5 + 1.0));
    // Only the nearest two cast shadows.
    let mut shadowed: Vec<f32> = on
        .iter()
        .filter(|s| s.shadows)
        .map(|s| s.lit.at.x)
        .collect();
    shadowed.sort_by(f32::total_cmp);
    assert_eq!(shadowed, vec![1.0, 2.5]);
    // Far off, nothing is lit; between 30 and 40 m it eases out.
    let far = choose(vec![at(41.0, 1.0e7)], Some(Vec3::ZERO), &none(4), 2);
    assert!(far.iter().all(Option::is_none));
    let fades: Vec<f32> = [30.0, 33.0, 36.0, 39.0]
        .iter()
        .map(|&x| {
            choose(vec![at(x, 1.0e7)], Some(Vec3::ZERO), &none(1), 2)[0]
                .as_ref()
                .unwrap()
                .fade
        })
        .collect();
    assert!(fades.windows(2).all(|w| w[1] < w[0]), "{fades:?}");
    assert!(fades[0] > 0.99 && fades[3] < 0.1);
}

#[test]
fn a_lights_shadows_stay_on_it_until_it_is_well_past_their_reach() {
    // Two fires; the one at 21 m had shadows and keeps them, a new one at 21 m gets none.
    let was = |x: f32, shadows: bool| Was {
        at: Vec3::new(x, 0.0, 0.0),
        on: true,
        shadows,
    };
    let shown = choose(
        vec![at(21.0, 1.0e7)],
        Some(Vec3::ZERO),
        &[was(21.0, true)],
        2,
    );
    assert!(shown[0].as_ref().unwrap().shadows);
    let shown = choose(
        vec![at(21.0, 1.0e7)],
        Some(Vec3::ZERO),
        &[was(21.0, false)],
        2,
    );
    assert!(!shown[0].as_ref().unwrap().shadows);
    let shown = choose(
        vec![at(24.0, 1.0e7)],
        Some(Vec3::ZERO),
        &[was(24.0, true)],
        2,
    );
    assert!(!shown[0].as_ref().unwrap().shadows);
    // A light stays on the entity that showed it, wherever it comes in the order.
    let shown = choose(
        vec![at(5.0, 1.0e7), at(10.0, 1.0e6)],
        Some(Vec3::ZERO),
        &[was(10.2, false), was(5.1, true)],
        2,
    );
    assert_eq!(shown[0].as_ref().unwrap().lit.at.x, 10.0);
    assert_eq!(shown[1].as_ref().unwrap().lit.at.x, 5.0);
}

/// The illuminance a light gives the ground so far out from under it, lux.
fn on_ground(lit: &Lit, foot: Vec3, out: f32, lumens: f32) -> f32 {
    let h = lit.at.y - foot.y;
    lumens / (4.0 * core::f32::consts::PI) * h / libm::powf(h * h + out * out, 1.5)
}

#[test]
fn a_campfire_lights_its_stones_under_its_flame_and_the_ground_about_as_the_moon() {
    // The fire sheet's campfire: 1.5 m of flame at 0.9 heat, 0.7 m wide.
    let foot = Vec3::new(0.0, 2000.0, 0.0);
    let blazes = Blazes {
        kept: vec![Blaze {
            at: foot,
            tall: 1.5,
            heat: 0.9,
            seed: 0.3,
            wide: 0.7,
        }],
        ..default()
    };
    let lit = lit(&blazes, 0.0).remove(0);
    assert!(lit.kept);
    // A real campfire's light, about 10^4 cd, by night as by day: the stones at its foot take
    // some tens of thousands of lux, so at their albedo they stand some four stops under a
    // flame of 10^5 cd/m² (research/2026-10-19-fire-step-back.md); at 2.5 stops more by night
    // they took hundreds of thousands and out-shone it.
    let candela = lit.lumens / (4.0 * core::f32::consts::PI);
    assert!((3_000.0..30_000.0).contains(&candela), "{candela} cd");
    let stones = on_ground(&lit, foot, 0.55, lit.lumens);
    assert!(stones < 60_000.0, "{stones} lux on the stones");
    // The ground 3 m out still takes about the moon's light, warmer, and past that falls dark,
    // as a night photograph exposed for the flame shows it.
    let moon = crate::daylight::illuminance(-18.0);
    let near = on_ground(&lit, foot, 3.0, lit.lumens);
    assert!(
        near > 0.2 * moon && near < 3.0 * moon,
        "{near} lux at 3 m by a {moon} lux moon"
    );
    // By day a few percent of the sun's, so sunlit stone round it keeps its colour.
    let sun = crate::daylight::illuminance(50.0);
    assert!(near < 0.05 * sun, "{near} lux at 3 m in a {sun} lux sun");
}

#[test]
fn a_hearth_lights_wood_flame_orange_with_its_green_kept() {
    // Mitchell Charity's blackbody table (sRGB, CIE 1964 10 degree): 2,200 K #ff932c and
    // 2,500 K #ffa148, as the games white-balance firelight. At 1,900 K (#ff8300) its green
    // was so low that the lit stones went salmon pink by night (fire round 13); its blue stays
    // under the blackbody's, so they don't go pink the other way. At 2,200-2,500 K's green
    // they still read pink-salmon, so it may reach 2,700 K's (#ffa957, fire round 21).
    let c = HEARTH_LIGHT.to_srgba();
    let (k2200, k2700) = (147.0 / 255.0, 169.0 / 255.0);
    assert!(c.red > 0.99, "{c:?}");
    assert!((k2200 - 0.01..=k2700 + 0.01).contains(&c.green), "{c:?}");
    assert!(c.blue < 44.0 / 255.0, "{c:?}");
}
