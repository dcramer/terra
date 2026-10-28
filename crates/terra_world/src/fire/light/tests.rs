use super::pool_tests::at_hour;
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
        // Flickering at about its first tone's rate, just above its puffs: two turns a
        // flicker, give or take, and some noise.
        let flickers = turns / 2.0;
        let first = flicker.puffs_hz * TONES[0].0;
        assert!(
            flickers > first * 0.6 && flickers < first * 2.0,
            "{flicker:?}: {flickers} flickers a second"
        );
    }
}

/// A fire's flicker's spectrum, sampled at 100 Hz over forty seconds less its mean: its share
/// of power below `under` Hz (by a DFT of those bins against the whole variance, Parseval) and
/// its power-weighted mean frequency up to 10 Hz.
fn spectrum(seed: f32, flicker: Flicker, under: f32) -> (f32, f32) {
    let (rate, n) = (100.0, 4000);
    let x: Vec<f32> = (0..n)
        .map(|i| signal(seed, i as f32 / rate, flicker))
        .collect();
    let mean = x.iter().sum::<f32>() / n as f32;
    let x: Vec<f32> = x.iter().map(|v| v - mean).collect();
    let variance = x.iter().map(|v| v * v).sum::<f32>() / n as f32;
    let bin_hz = rate / n as f32;
    let power = |k: usize| {
        let (mut re, mut im) = (0.0f32, 0.0f32);
        for (i, v) in x.iter().enumerate() {
            let a = core::f32::consts::TAU * (k * i % n) as f32 / n as f32;
            re += v * libm::cosf(a);
            im += v * libm::sinf(a);
        }
        2.0 * (re * re + im * im) / (n * n) as f32
    };
    let bins: Vec<(f32, f32)> = (1..=(10.0 / bin_hz) as usize)
        .map(|k| (k as f32 * bin_hz, power(k)))
        .collect();
    let low = bins
        .iter()
        .filter(|(f, _)| *f < under)
        .map(|(_, p)| p)
        .sum::<f32>();
    let total = bins.iter().map(|(_, p)| p).sum::<f32>();
    let centre = bins.iter().map(|(f, p)| f * p).sum::<f32>() / total;
    (low / variance, centre)
}

/// Filmed campfires flicker at 1.8-3.8 Hz (research/2026-10-25-fire-motion.md), and a light
/// that swings slower makes the coals and the flame's foot pump with it in place: a campfire's
/// flicker puts nearly all its power round 2.5-3.5 Hz, above its flame's own puffing (1.8 Hz,
/// fire round g27), little under 1 Hz, and every fire's little under half its puff rate (fire
/// round g14: the product of three sines put a third of it in beats at 0.32 and 0.81 Hz).
#[test]
fn a_fires_flicker_has_its_power_round_its_puffs_not_in_slow_beats() {
    for seed in [0.1, 0.37, 0.62, 0.9] {
        let (slow, centre) = spectrum(seed, Flicker::of(0.7), 1.0);
        assert!(
            slow < 0.05,
            "campfire {seed}: {slow} of its power under 1 Hz"
        );
        assert!(
            (2.5..=3.6).contains(&centre),
            "campfire {seed}: centred at {centre} Hz"
        );
        for wide in [0.1, 6.0] {
            let flicker = Flicker::of(wide);
            let (slow, _) = spectrum(seed, flicker, flicker.puffs_hz * 0.5);
            assert!(
                slow < 0.05,
                "{flicker:?} {seed}: {slow} under half its puffs"
            );
        }
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
        out: Vec3::ZERO,
    };
    let blazes = Blazes {
        kept: vec![campfire],
        blazes: vec![Blaze {
            at: ground + Vec3::X * 40.0,
            tall: 1.5,
            heat: 0.9,
            seed: 0.6,
            wide: 0.0,
            out: Vec3::ZERO,
        }],
        ..default()
    };
    let mut seen = (f32::MAX, f32::MIN);
    let (mut low, mut high) = (f32::MAX, f32::MIN);
    for i in 0..400 {
        let t = i as f32 * 0.05;
        let lit = lit(&blazes, t, crate::fire::BreathTier::Cards);
        assert_eq!(lit.len(), 3);
        let (hearth, top, roof) = (&lit[0], &lit[1], &lit[2]);
        // A key low in its flames and a brighter light at their top (`HEARTH_TOP`): the top
        // one lights the faces of those sat round it, the key the stones at its foot, and for
        // each lumen the top one lights the ground 3 m out more against the stones than the
        // key does, so the pool reaches out past the ring.
        assert!(top.lumens > hearth.lumens && top.at.y > hearth.at.y + 0.5);
        let pool = |l: &Lit| on_ground(l, ground, 3.0, 1.0) / on_ground(l, ground, 0.55, 1.0);
        assert!(
            pool(top) > 2.0 * pool(hearth),
            "{} {}",
            pool(top),
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
        // Its light is its flames' flicker: the signal the shaders are given for the fire, at
        // the time its flames move by (`fire::PACE`).
        assert!((hearth.glow - glow(&campfire, crate::fire::flame_time(t))).abs() < 1e-6);
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
        night_stops: 2.5,
        dusk_gain: 1.0,
        dusk_reach: 1.0,
        casts: true,
        still: None,
    }
}

fn none(n: usize) -> Vec<Was> {
    vec![
        Was {
            at: Vec3::ZERO,
            on: false,
            near: false
        };
        n
    ]
}

#[test]
fn a_burning_village_is_a_score_of_lights_the_nearest_shadowed_none_past_forty_metres() {
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
    let was = |x: f32, near: bool| Was {
        at: Vec3::new(x, 0.0, 0.0),
        on: true,
        near,
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

#[test]
fn a_hearths_pit_key_casts_its_shadows_and_its_top_light_shines_in_the_fog_unshadowed() {
    // Two campfires, 3 and 6 m off, each a key that casts and a top light that does not, and
    // a blaze 4 m off; two shadowed lights.
    let hearth = |x: f32, casts: bool| Lit {
        kept: true,
        casts,
        still: None,
        ..at(x, 1.0e5)
    };
    let lit = vec![
        hearth(3.0, true),
        hearth(3.1, false),
        at(4.0, 1.0e5),
        hearth(6.0, true),
        hearth(6.1, false),
    ];
    let shown = choose(lit, Some(Vec3::ZERO), &none(LIGHTS), 2);
    let on: Vec<&Shown> = shown.iter().flatten().collect();
    let x = |f: fn(&Shown) -> bool| {
        let mut x: Vec<f32> = on.iter().filter(|s| f(s)).map(|s| s.lit.at.x).collect();
        x.sort_by(f32::total_cmp);
        x
    };
    // The nearest two that may cast do: the first fire's key and the blaze; no top light.
    assert_eq!(x(|s| s.shadows), vec![3.0, 4.0]);
    // In the fog: the first fire's key, shadowed, and both top lights; never the blaze.
    assert_eq!(x(|s| s.scatters), vec![3.0, 3.1, 6.1]);
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
            out: Vec3::ZERO,
        }],
        ..default()
    };
    let lights = lit(&blazes, 0.0, crate::fire::BreathTier::Cards);
    assert!(lights.iter().all(|l| l.kept) && lights.len() == 2);
    let lumens: f32 = lights.iter().map(|l| l.lumens).sum();
    let on = |out: f32| -> f32 {
        lights
            .iter()
            .map(|l| on_ground(l, foot, out, l.lumens))
            .sum()
    };
    // A real campfire's light, about 10^4 cd, by night as by day: the stones at its foot take
    // some tens of thousands of lux, so at their albedo they stand some four stops under a
    // flame of 10^5 cd/m² (research/2026-10-19-fire-step-back.md); at 2.5 stops more by night
    // they took hundreds of thousands and out-shone it.
    let candela = lumens / (4.0 * core::f32::consts::PI);
    assert!((3_000.0..30_000.0).contains(&candela), "{candela} cd");
    let stones = on(0.55);
    assert!(stones < 60_000.0, "{stones} lux on the stones");
    // The ground 3 m out still takes about the moon's light, warmer, and past that falls dark,
    // as a night photograph exposed for the flame shows it.
    let moon = crate::daylight::illuminance(-18.0);
    let near = on(3.0);
    assert!(
        near > 0.2 * moon && near < 3.0 * moon,
        "{near} lux at 3 m by a {moon} lux moon"
    );
    // By day a few percent of the sun's, so sunlit stone round it keeps its colour.
    let sun = crate::daylight::illuminance(50.0);
    assert!(near < 0.05 * sun, "{near} lux at 3 m in a {sun} lux sun");
}

/// What a hearth's lights give a face so far out from the fire's foot and so high, turned to
/// the fire: each a disc (`crate::falloff`) on a vertical face, lux per lumen of the light,
/// eased out to nothing at its range as drawn (bevy_pbr 0.20's `getRangeFalloff`).
pub(super) fn on_face(lit: &Lit, foot: Vec3, out: f32, high: f32) -> f32 {
    let (across, up) = (out, foot.y + high - lit.at.y);
    let d2 = across * across + up * up;
    let facing = across / libm::sqrtf(d2);
    let f = d2 / (lit.range * lit.range);
    let window = (1.0 - f * f).clamp(0.0, 1.0).powi(2);
    lit.lumens / (4.0 * core::f32::consts::PI) * facing / (d2 + lit.radius * lit.radius) * window
}

/// How many times a light is keyed up at an hour, as `light` keys it, the sun a degree or more
/// under (`Sky::twilight` 1).
pub(super) fn keyed(lit: &Lit, night: f32, sky: f32) -> f32 {
    hearth_by_hour(lit.night_stops, lit.dusk_gain, night, 1.0, sky)
}

/// What a light gives the ground so far out from the fire's foot, as drawn: a disc
/// (`crate::falloff`) eased out to nothing at its range (bevy_pbr 0.20's `getRangeFalloff`).
pub(super) fn on_ground_drawn(lit: &Lit, foot: Vec3, out: f32) -> f32 {
    let h = lit.at.y - foot.y;
    let d2 = h * h + out * out;
    let f = d2 / (lit.range * lit.range);
    let window = (1.0 - f * f).clamp(0.0, 1.0).powi(2);
    lit.lumens / (4.0 * core::f32::consts::PI) * h
        / libm::sqrtf(d2)
        / (d2 + lit.radius * lit.radius)
        * window
}

/// A torch staked on open ground, its head 1.2 m up (the review's staged torch), lit by a rig:
/// the light its lights give the ground so far out from its foot at an hour (`keyed`).
fn staked_pool(colour: Color, rig: Rig, night: f32, sky: f32) -> impl Fn(f32) -> f32 {
    let ground = Vec3::new(0.0, 2000.0, 0.0);
    let blazes = Blazes {
        kept: vec![Blaze {
            at: ground + Vec3::Y * 1.2,
            tall: 0.55,
            heat: 0.9,
            seed: 0.3,
            wide: 0.22,
            out: Vec3::ZERO,
        }],
        lights: vec![colour],
        rigs: vec![rig],
        ..default()
    };
    let lights = lit(&blazes, 0.0, crate::fire::BreathTier::Cards);
    move |out: f32| -> f32 {
        lights
            .iter()
            .map(|l| on_ground_drawn(&at_hour(l, night, sky), ground, out))
            .sum()
    }
}

#[test]
fn a_staked_torchs_night_pool_falls_off_from_its_foot_more_than_a_hearths_rig_would() {
    // By full night the ground half a metre from the staked torch's foot takes many times the
    // light of the ground 2.5 m out, a pool round the foot. Lit by a hearth's rig, its top light
    // high and wide, the yard took a far evener tint (torch round t3).
    let pool = |colour: Color, rig: Rig| {
        let on = staked_pool(colour, rig, 1.0, 36.0);
        (on(0.5) / on(2.5), on(0.5))
    };
    let ((torch, at_foot), (hearth, hearth_foot)) = (
        pool(TORCH_LIGHT, Rig::Torch),
        pool(HEARTH_LIGHT, Rig::Hearth),
    );
    println!("torch pool falls {torch:.1}x from 0.5 to 2.5 m, a hearth's rig {hearth:.1}x");
    assert!(torch > 6.0, "a torch's pool falls only {torch:.1}x");
    assert!(
        torch > hearth * 1.3,
        "{torch:.1}x against a hearth rig's {hearth:.1}x"
    );
    assert!(at_foot > hearth_foot, "its foot as lit as a hearth rig's");
}

#[test]
fn a_staked_torchs_dusk_pool_is_as_bright_at_its_foot_as_a_hearths_rig_would_light_it() {
    // At the yard's dusk (the sun five degrees under, `night` 0.13, the sky's light on the
    // shade 830, `HEARTH_DUSK_SKY`) the ground by the staked torch's foot takes at least the
    // light the hearth's rig gave it, its top light keyed up 9 times (`HEARTH_TOP_DUSK`): keyed
    // up no more than a pit light, its pool at dusk took half (torch round t4).
    let (night, sky) = (0.13, 830.0);
    let torch = staked_pool(TORCH_LIGHT, Rig::Torch, night, sky);
    let hearth = staked_pool(HEARTH_LIGHT, Rig::Hearth, night, sky);
    let (at_foot, hearth_foot) = (torch(0.5), hearth(0.5));
    println!(
        "torch dusk foot {at_foot:.1}, a hearth's rig {hearth_foot:.1}; falls {:.1}x to 2.5 m",
        torch(0.5) / torch(2.5)
    );
    assert!(
        at_foot >= hearth_foot,
        "{at_foot:.1} against {hearth_foot:.1}"
    );
    assert!(torch(0.5) / torch(2.5) > 4.0, "no pool at dusk");
}

#[test]
fn a_campfires_pool_reads_warm_a_few_metres_out_at_dusk() {
    // The lake's campfire at the lake's civil dusk (the sun 1.6 degrees under at 21:45 on
    // 07-20: night 0.13, the sky's light on the shade 1,240). Lake round 20's review measured
    // its pool "about 2x up close in a neutral grey ambient": at one key for both lights the
    // ground 2 m out took a third of the sky's light and 4 m out a sixteenth. The games' dusk
    // campfires light the camp orange a few metres round, so the ground 3 m out takes at least
    // half the sky's light from it and reads orange, 4 m out still warm. The sky's light is
    // the probe's on a face (`a_figure_sat_by_a_campfire_at_dusk_is_lit_by_it_twice_as_bright_and_warm`).
    let foot = Vec3::new(0.0, 2000.0, 0.0);
    let blazes = Blazes {
        kept: vec![Blaze {
            at: foot,
            tall: 1.95,
            heat: 0.9,
            seed: 0.3,
            wide: 0.9,
            out: Vec3::ZERO,
        }],
        ..default()
    };
    let (night, sky) = (0.13, 1_240.0);
    let sky_on_ground = 4.56 * core::f32::consts::PI * sky;
    let luma = |c: Vec3| c.dot(Vec3::new(0.2126, 0.7152, 0.0722));
    let fire_colour = Vec3::from_array(HEARTH_LIGHT.to_linear().to_f32_array_no_alpha());
    let dusk = Vec3::new(0.32, 0.27, 0.42);
    let share = |out: f32| {
        (0..100)
            .map(|i| {
                let lights = lit(&blazes, i as f32 * 0.1, crate::fire::BreathTier::Cards);
                let lux: f32 = lights
                    .iter()
                    .map(|l| on_ground_drawn(&at_hour(l, night, sky), foot, out))
                    .sum();
                lux / sky_on_ground / 100.0
            })
            .sum::<f32>()
    };
    let warmth = |share: f32| {
        let c = fire_colour / luma(fire_colour) * share + dusk / luma(dusk);
        c.x / c.z
    };
    let (two, three, four) = (share(2.0), share(3.0), share(4.0));
    assert!(three >= 0.5, "3 m out: {three:.2} of the sky's light");
    assert!(
        warmth(three) > 1.5,
        "3 m out: red {:.2} times blue",
        warmth(three)
    );
    assert!(
        warmth(four) > 1.25,
        "4 m out: red {:.2} times blue",
        warmth(four)
    );
    // A pool, not a tint to the horizon: it falls off past the camp.
    assert!(two > 1.5 * four, "2 m {two:.2}, 4 m {four:.2}");
    assert!(share(8.0) < 0.15, "8 m out: {:.2}", share(8.0));
}

#[test]
fn a_figure_sat_by_a_campfire_at_dusk_is_lit_by_it_twice_as_bright_and_warm() {
    // The lake's campfire, 1.95 m of flame 0.9 m wide at 0.9 heat, at the lake's dusk (the
    // sun 3.7 degrees under: night 0.311, the sky's light on the shade 1,038), as lake round
    // 18's probe logged it.
    let foot = Vec3::new(0.0, 2000.0, 0.0);
    let fire = Blaze {
        at: foot,
        tall: 1.95,
        heat: 0.9,
        seed: 0.3,
        wide: 0.9,
        out: Vec3::ZERO,
    };
    let blazes = Blazes {
        kept: vec![fire],
        ..default()
    };
    let (night, sky) = (0.311, 1_038.0);
    // The sky's light on a face in the shade, as the probe measured it: the troll's face lit
    // by the sky alone stood at 76 (linear luma, under the tonemapper's shoulder), and the old
    // rule's lights, which this model puts at 1.64 times pi x the sky's light on his face,
    // added 0.36 of that, so the sky gave his face 4.56 times pi x its light. The same model
    // puts round 18's two lights at 1.38 of it, his face measured at 2.4 times: it reads the
    // probe right.
    let sky_on_face = 4.56 * core::f32::consts::PI * sky;
    // The light's colour on the face, fire and the dusk sky's lavender (`daylight`'s -6 key's
    // horizon) each brought to a luma of one.
    let luma = |c: Vec3| c.dot(Vec3::new(0.2126, 0.7152, 0.0722));
    let fire_colour = Vec3::from_array(HEARTH_LIGHT.to_linear().to_f32_array_no_alpha());
    let dusk = Vec3::new(0.32, 0.27, 0.42);
    // The troll sat where the fireside rule keeps him (terra_creatures' `staged::fireside`):
    // the fire's middle 1.08 m and more from his toes and his face some 1.5 m behind them, so
    // 2.6 to 3.0 m out across the ground and 2.2 m up; a man sat a metre from it, his 1.1 m up.
    for (out, high) in [(2.6, 2.2), (3.0, 2.2), (1.0, 1.1)] {
        let mut share = 0.0;
        for i in 0..100 {
            let lights = lit(&blazes, i as f32 * 0.1, crate::fire::BreathTier::Cards);
            let lux: f32 = lights
                .iter()
                .map(|l| on_face(&at_hour(l, night, sky), foot, out, high))
                .sum();
            share += lux / sky_on_face / 100.0;
        }
        // At least as much from the fire as from the sky: twice as lit as with no fire.
        assert!(
            1.0 + share >= 2.0,
            "a face {out} m out and {high} m up: {:.2} times the sky's",
            1.0 + share
        );
        // But not driven past the white: no more than some five times as lit, a metre out by
        // the stones too (with the pit key keyed up 30 times at dusk, g80, some twelve times:
        // the stones by it read flat clay-orange, g82).
        assert!(
            1.0 + share <= 5.0,
            "a face {out} m out and {high} m up: {:.2} times the sky's",
            1.0 + share
        );
        // And warm: his face reads firelit orange, not the dusk's mauve.
        let c = fire_colour / luma(fire_colour) * share + dusk / luma(dusk);
        assert!(c.x > 2.0 * c.z, "{c} on a face {out} m out");
    }
}

#[test]
fn a_campfires_light_reaches_the_ground_a_few_metres_out_as_a_real_fires_does() {
    // A real fire's light falls by the square of the distance and nothing else; the game's
    // lights also ease out to nothing at their range (bevy_pbr 0.20's `getRangeFalloff`, after
    // Unreal's). The pool a few metres across that places a fire in a scene must keep most of
    // a real fire's light there: the ground 4 m from the lake's campfire took half of it, its
    // pool weak past 2 m (lake round 19's review).
    let window = |d2: f32, range: f32| {
        let f = d2 / (range * range);
        (1.0 - f * f).clamp(0.0, 1.0).powi(2)
    };
    let foot = Vec3::new(0.0, 2000.0, 0.0);
    let blazes = Blazes {
        kept: vec![Blaze {
            at: foot,
            tall: 1.95,
            heat: 0.9,
            seed: 0.3,
            wide: 0.9,
            out: Vec3::ZERO,
        }],
        ..default()
    };
    let lights = lit(&blazes, 0.0, crate::fire::BreathTier::Cards);
    for out in [3.0, 4.0] {
        let real: f32 = lights
            .iter()
            .map(|l| on_ground(l, foot, out, l.lumens))
            .sum();
        let drawn: f32 = lights
            .iter()
            .map(|l| {
                let h = l.at.y - foot.y;
                on_ground(l, foot, out, l.lumens) * window(h * h + out * out, l.range)
            })
            .sum();
        assert!(
            drawn > 0.8 * real,
            "{out} m out: {:.2} of a real fire's light",
            drawn / real
        );
    }
}

#[test]
fn a_hearth_lights_the_grass_round_it_deep_orange() {
    // RDR2's night campfires light the ground round them at hue 20-21 (sRGB HSV), saturation
    // 0.7-0.9; at 2,700 K's green (#ffa957 less blue) our olive grass (noon sRGB 80,87,38) took
    // it as yellow-olive, hue 36 (campfire step round). The light's colour on that grass, by
    // the albedo alone, must come out orange-red: hue under 30. Its blue stays under a fifth
    // of its green, so the lit stones don't go pink (fire round 13's salmon was the
    // tonemapper's shoulder, since kept from it; `groundlight` checks the stones' hue).
    let c = HEARTH_LIGHT.to_linear();
    let grass = Color::srgb(80.0 / 255.0, 87.0 / 255.0, 38.0 / 255.0).to_linear();
    let lit = Color::linear_rgb(
        c.red * grass.red,
        c.green * grass.green,
        c.blue * grass.blue,
    );
    let hsv = bevy::color::Hsva::from(lit.to_srgba());
    assert!(hsv.hue < 30.0 && hsv.hue > 10.0, "{lit:?} hue {}", hsv.hue);
    let s = HEARTH_LIGHT.to_srgba();
    assert!(s.red > 0.99 && s.blue < 0.2 * s.green, "{s:?}");
}

#[test]
fn a_marched_fireball_lights_the_face_it_strikes_from_in_front_once_it_has_struck() {
    use crate::fire::{BreathTier, Jet};
    let from = Vec3::new(0.0, 2000.0, 0.0);
    let jet = Jet {
        from,
        way: Vec3::Z,
        reach: 10.0,
        spread: 0.2,
        on: true,
        face: 10.0,
        normal: -Vec3::Z,
    };
    let blazes = Blazes {
        jets: vec![jet],
        ..default()
    };
    // On at 0 s, its front strikes 10 m out at 10 / 24 s.
    let state = [(true, 0.0, 0.0)];
    assert!(strikes(&blazes, &state, 0.3, BreathTier::Marched).is_empty());
    assert!(strikes(&blazes, &state, 2.0, BreathTier::Cards).is_empty());
    let lit = strikes(&blazes, &state, 2.0, BreathTier::Marched);
    assert_eq!(lit.len(), 1);
    // In front of the face, on the breather's side, above the stream: never behind the stone.
    assert!(lit[0].at.z < 10.0 - 1.0, "{:?}", lit[0].at);
    assert!(lit[0].at.y > from.y, "{:?}", lit[0].at);
    // Off, its fireball's light goes with it.
    assert!(strikes(&blazes, &[(false, 1.0, 0.0)], 2.0, BreathTier::Marched).is_empty());
    // By night it and its stream's lights gain less than a blaze's, so the night ground under
    // the stream keeps its orange rather than clipping to cream; drawn as cards, as a blaze.
    let blaze = by_night(NIGHT_STOPS, 1.0);
    assert!(by_night(lit[0].night_stops, 1.0) < blaze * 0.5);
    assert!(by_night(lit[0].night_stops, 1.0) > 1.0);
    for jet in lit_jets(&blazes, BreathTier::Marched) {
        assert!(by_night(jet.night_stops, 1.0) < blaze * 0.5, "{jet:?}");
    }
    for jet in lit_jets(&blazes, BreathTier::Cards) {
        assert_eq!(by_night(jet.night_stops, 1.0), blaze, "{jet:?}");
    }
}

#[test]
fn a_kept_fire_is_keyed_up_as_the_sun_goes_down_not_by_day() {
    let stops = HEARTH_NIGHT_STOPS;
    let full = libm::exp2f(stops);
    // Noon: the sun high, the sky bright, no boost.
    assert_eq!(hearth_by_hour(stops, 1.0, 0.0, 0.0, 5_000.0), 1.0);
    // The lake's civil dusk, the sun a degree under and the sky's light on the shade most of
    // the day's: more than half the night's boost, where by the night alone it was a fifth of
    // a stop.
    let lake = hearth_by_hour(stops, 1.0, 0.13, 1.0, 1_240.0);
    let dusk = libm::exp2f(HEARTH_DUSK_STOPS);
    assert!(lake >= dusk * 0.55, "{lake}");
    assert!(lake > by_night(stops, 0.13) * 2.5, "{lake}");
    // The yard's darker dusk, the sun five degrees under: its night's share, which is more
    // than its sky asks, and so less than the lake's brighter sky asks.
    let yard = hearth_by_hour(stops, 1.0, 0.5, 1.0, 830.0);
    assert_eq!(yard, by_night(stops, 0.5));
    assert!(yard < lake, "{yard}");
    // Full night: as by the night's stops, the dark sky asking for less.
    assert_eq!(hearth_by_hour(stops, 1.0, 1.0, 1.0, 36.0), full);
    // Never past the night's whole boost, however bright the sky with the sun going.
    assert!(hearth_by_hour(stops, 1.0, 0.0, 0.6, 20_000.0) <= dusk);
    // A top light keyed up more at dusk: so many times the pit light's at the lake's dusk,
    // none by day, and by full night the night's stops alone, as the pit light's.
    let top = hearth_by_hour(stops, HEARTH_TOP_DUSK, 0.13, 1.0, 1_240.0);
    assert!(top > lake * HEARTH_TOP_DUSK * 0.8, "{top} against {lake}");
    assert_eq!(
        hearth_by_hour(stops, HEARTH_TOP_DUSK, 0.0, 0.0, 5_000.0),
        1.0
    );
    assert_eq!(hearth_by_hour(stops, HEARTH_TOP_DUSK, 1.0, 1.0, 36.0), full);
    // The top light keyed up at dusk as much whatever its fewer night stops, and by full night
    // only by those.
    let fewer = HEARTH_TOP_NIGHT_STOPS;
    assert_eq!(
        hearth_by_hour(fewer, HEARTH_TOP_DUSK, 0.13, 1.0, 1_240.0),
        top
    );
    assert_eq!(
        hearth_by_hour(fewer, HEARTH_TOP_DUSK, 1.0, 1.0, 36.0),
        libm::exp2f(fewer)
    );
}

/// The stream's own lights of the jets that are on.
fn lit_jets(blazes: &Blazes, tier: crate::fire::BreathTier) -> Vec<Lit> {
    let jets = blazes.jets.iter().filter(|jet| jet.on).count() * 3;
    let all = lit(blazes, 2.0, tier);
    assert!(all.len() >= jets);
    all.into_iter().take(jets).collect()
}

/// A campfire swells and sinks as a whole over a second or two, a little: under 3.5% either way
/// (filmed campfires' area varies 9-19% in all, mostly in puffs that rise them, fire rounds
/// g13 and g15), and slowly: its steepest, two depths over a cell at 1.5 times, about 0.06 a
/// second.
#[test]
fn a_campfire_swells_slowly_and_shallowly() {
    let flicker = Flicker::of(0.7);
    let samples: Vec<f32> = (0..30_000)
        .map(|i| swell(0.37, i as f32 * 0.001, flicker))
        .collect();
    let (low, high) = samples
        .iter()
        .fold((f32::MAX, f32::MIN), |(l, h), &w| (l.min(w), h.max(w)));
    assert!(low >= 0.965 && high <= 1.035, "{low} {high}");
    assert!(high - low > 0.02, "it swells: {low} {high}");
    let fastest = samples
        .windows(2)
        .map(|w| (w[1] - w[0]).abs() / 0.001)
        .fold(0.0f32, f32::max);
    assert!(fastest < 0.08, "slow: {fastest} a second");
}

#[test]
fn a_torch_lights_yellower_than_a_campfire_and_each_fire_by_its_own_colour() {
    // A pitch torch's light is less orange than wood's: more green to its red, and still warm
    // (red over green over blue), so torchlit stone reads warm off-white, not rust.
    let (t, h) = (TORCH_LIGHT.to_srgba(), HEARTH_LIGHT.to_srgba());
    assert!(
        t.green / t.red > h.green / h.red + 0.1,
        "{t:?} against {h:?}"
    );
    assert!(t.red > t.green && t.green > t.blue && t.blue > 0.2, "{t:?}");
    // Two kept fires, a campfire and a torch: each one's lights take its own colour.
    let ground = Vec3::new(0.0, 2000.0, 0.0);
    let fire = |x: f32| Blaze {
        at: ground + Vec3::X * x,
        tall: 0.6,
        heat: 0.8,
        seed: x * 0.1,
        wide: 0.2,
        out: Vec3::ZERO,
    };
    let blazes = Blazes {
        kept: vec![fire(0.0), fire(8.0)],
        lights: vec![HEARTH_LIGHT, TORCH_LIGHT],
        rigs: vec![Rig::Hearth, Rig::Torch],
        ..default()
    };
    let lit = lit(&blazes, 0.0, crate::fire::BreathTier::Cards);
    let colour_by = |x: f32| {
        lit.iter()
            .filter(|l| l.kept && (l.at.x - x).abs() < 1.0)
            .map(|l| l.color)
            .collect::<Vec<_>>()
    };
    let (camp, torch) = (colour_by(0.0), colour_by(8.0));
    assert!(
        !camp.is_empty() && camp.iter().all(|&c| c == HEARTH_LIGHT),
        "{camp:?}"
    );
    assert!(
        !torch.is_empty() && torch.iter().all(|&c| c == TORCH_LIGHT),
        "{torch:?}"
    );
}

/// A dungeon's torches: so many kept fires a metre and a half up the walls of a corridor along
/// x, one every three metres on alternate sides.
fn torches(n: usize) -> Blazes {
    let fire = |i: usize| Blaze {
        at: Vec3::new(
            i as f32 * 3.0,
            2001.5,
            if i.is_multiple_of(2) { -1.4 } else { 1.4 },
        ),
        tall: 0.45,
        heat: 0.6,
        seed: (i as f32 * 0.37).fract(),
        wide: 0.12,
        out: Vec3::ZERO,
    };
    Blazes {
        kept: (0..n).map(fire).collect(),
        lights: vec![TORCH_LIGHT; n],
        rigs: vec![Rig::Torch; n],
        ..default()
    }
}

#[test]
fn a_torchs_light_flickers_half_as_deep_as_its_flame_a_campfires_as_deep() {
    // Sampled over ten seconds: how far each kept fire's lights stray from their mean, against
    // how far its flame's signal does (`glow`, what its flames are drawn by).
    let stray = |blazes: &Blazes| {
        let (mut light, mut flame) = (Vec::new(), Vec::new());
        for i in 0..2000 {
            let t = i as f32 * 0.005;
            let lit = lit(blazes, t, crate::fire::BreathTier::Cards);
            light.push(lit[0].lumens + lit[1].lumens);
            flame.push(lit[0].glow);
        }
        let spread = |s: &[f32]| {
            let mean = s.iter().sum::<f32>() / s.len() as f32;
            s.iter().fold(0.0f32, |m, v| m.max((v / mean - 1.0).abs()))
        };
        (spread(&light), spread(&flame))
    };
    let (light, flame) = stray(&torches(1));
    assert!(flame > 0.25, "a torch's flame strays {flame}");
    let share = light / flame;
    assert!(
        (0.4..0.6).contains(&share),
        "{light} against its flame's {flame}"
    );
    let campfire = Blazes {
        kept: vec![Blaze {
            at: Vec3::new(0.0, 2000.0, 0.0),
            tall: 1.5,
            heat: 0.9,
            seed: 0.3,
            wide: 0.7,
            out: Vec3::ZERO,
        }],
        ..default()
    };
    let (light, flame) = stray(&campfire);
    assert!(
        (light / flame - 1.0).abs() < 0.05,
        "{light} against its flame's {flame}"
    );
}

#[test]
fn every_torch_of_a_dungeon_lights_as_a_torch_however_many_burn() {
    // Thirty torches, more than the sixteen drawn as hearths: each one lights in its own
    // colour from just over its flame, none gathered into a blaze's light.
    let blazes = torches(30);
    let lit = lit(&blazes, 0.0, crate::fire::BreathTier::Cards);
    assert!(lit.iter().all(|l| l.kept && l.color == TORCH_LIGHT));
    for fire in &blazes.kept {
        let own = lit.iter().filter(|l| l.at.distance(fire.at) < 1.0).count();
        assert_eq!(own, 2, "{fire:?}");
    }
}

#[test]
fn walking_a_torchlit_corridor_no_light_blinks_in_or_out() {
    // Down the corridor a centimetre a step: each torch's light, all its lights summed, moves
    // from step to step by a small share of the brightest torch's, never by a light cut whole.
    let blazes = torches(40);
    let lit = lit(&blazes, 0.0, crate::fire::BreathTier::Cards);
    let mut was = none(LIGHTS);
    let mut last: Option<Vec<f32>> = None;
    let (mut worst, mut brightest) = (0.0f32, 0.0f32);
    for step in 0..3000 {
        let eye = Vec3::new(20.0 + step as f32 * 0.01, 2001.6, 0.0);
        let shown = choose(lit.clone(), Some(eye), &was, 2);
        let mut by_torch = vec![0.0f32; blazes.kept.len()];
        for s in shown.iter().flatten() {
            let i = (s.lit.at.x / 3.0).round() as usize;
            by_torch[i] += s.lit.lumens * s.fade;
        }
        brightest = brightest.max(by_torch.iter().copied().fold(0.0, f32::max));
        if let Some(last) = &last {
            for (a, b) in by_torch.iter().zip(last) {
                worst = worst.max((a - b).abs());
            }
        }
        was = shown
            .iter()
            .map(|s| Was {
                at: s.as_ref().map_or(Vec3::ZERO, |s| s.lit.at),
                on: s.is_some(),
                near: s.as_ref().is_some_and(|s| s.shadows),
            })
            .collect();
        last = Some(by_torch);
    }
    assert!(worst < 0.02 * brightest, "{worst} of {brightest}");
}
