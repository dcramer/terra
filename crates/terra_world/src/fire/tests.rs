use super::*;

#[test]
fn a_campfire_clears_the_ground_a_stride_past_its_stones_and_no_farther() {
    let fire = Kindled {
        tall: 1.5,
        wide: 0.7,
        heat: 0.9,
    };
    let at = Transform::from_translation(Vec3::new(3.0, 2000.0, -4.0));
    let ground = scorched(&fire, &at);
    let out = |m: f32| at.translation + Vec3::new(m * 0.6, 0.0, m * 0.8);
    assert!(ground.covers(at.translation, 0.0));
    assert!(ground.covers(out(1.8), 0.0));
    assert!(!ground.covers(out(2.4), 0.0));
}

/// How far a light wavers over ten seconds, and how many times a second it turns.
fn wavers(lumens: f32) -> (f32, f32) {
    let samples: Vec<f32> = (0..10_000)
        .map(|i| flicker(i as f32 * 0.001, 0.0, lumens).0)
        .collect();
    let (low, high) = samples
        .iter()
        .fold((f32::MAX, f32::MIN), |(l, h), &w| (l.min(w), h.max(w)));
    let turns = samples
        .windows(3)
        .filter(|w| (w[1] - w[0]) * (w[2] - w[1]) < 0.0)
        .count();
    (high - low, turns as f32 / 10.0)
}

#[test]
fn a_kindled_fire_burns_where_it_stands_while_it_is_there() {
    use bevy::ecs::system::RunSystemOnce;
    let mut world = World::new();
    world.init_resource::<Blazes>();
    let at = Vec3::new(3.0, 2000.0, -4.0);
    let fire = Kindled {
        tall: 0.6,
        wide: 0.4,
        heat: 0.9,
    };
    let lit = world
        .spawn((fire, GlobalTransform::from_translation(at)))
        .id();
    world.run_system_once(keep).unwrap();
    let drawn: Vec<Blaze> = world.resource::<Blazes>().all().copied().collect();
    assert_eq!(drawn.len(), 1);
    assert_eq!((drawn[0].at, drawn[0].tall, drawn[0].wide), (at, 0.6, 0.4));
    world.despawn(lit);
    world.run_system_once(keep).unwrap();
    assert_eq!(world.resource::<Blazes>().all().count(), 0);
}

#[test]
fn a_campfire_lights_round_it_from_its_flames_and_a_burning_roof_across_the_way() {
    let ground = Vec3::new(0.0, 2000.0, 0.0);
    let blazes = Blazes {
        kept: vec![Blaze {
            at: ground,
            tall: 1.5,
            heat: 0.9,
            seed: 0.3,
            wide: 0.7,
        }],
        blazes: vec![Blaze {
            at: ground + Vec3::X * 40.0,
            tall: 1.5,
            heat: 0.9,
            seed: 0.6,
            wide: 0.0,
        }],
        ..default()
    };
    let lit = lit(&blazes);
    assert_eq!(lit.len(), 2);
    let (hearth, roof) = (&lit[0], &lit[1]);
    assert_eq!(hearth.at, ground);
    // In its flames, over its logs, not over the heads of those sat by it.
    assert!(hearth.lift < 1.5, "{}", hearth.lift);
    // Lighting a ring round it, not the far shore; a burning roof's light reaches farther.
    assert!(hearth.range <= 15.0 && roof.range > hearth.range);
    assert!(hearth.lumens > 0.0);
}

#[test]
fn a_small_fires_light_flickers_quicker_and_further_than_a_big_ones() {
    let (small_reach, small_turns) = wavers(LUMENS_A_METRE * 0.5);
    let (big_reach, big_turns) = wavers(LUMENS_A_METRE * 40.0);
    assert!(small_reach > big_reach * 1.5, "{small_reach} {big_reach}");
    assert!(small_turns > big_turns * 1.5, "{small_turns} {big_turns}");
    // Never out, never doubled.
    for lumens in [LUMENS_A_METRE * 0.1, LUMENS_A_METRE * 100.0] {
        for i in 0..1000 {
            let (waver, _) = flicker(i as f32 * 0.01, 1.0, lumens);
            assert!((0.6..=1.4).contains(&waver), "{waver}");
        }
    }
}
