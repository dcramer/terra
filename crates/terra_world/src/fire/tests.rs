use super::*;

#[test]
fn the_shaders_and_the_lights_fly_a_breath_at_one_speed_and_draw_its_sprites_alike() {
    let shader = std::fs::read_to_string(crate::data::assets().join("shaders/fire.wesl")).unwrap();
    assert!(shader.contains(&format!("const JET_SPEED: f32 = {JET_SPEED_MPS:.1};")));
    assert!(shader.contains(&format!("const BREATH_CARDS: f32 = {BREATH_CARDS}.0;")));
    assert!(shader.contains(&format!(
        "const BREATH_EMBER_CARDS: f32 = {BREATH_EMBER_CARDS}.0;"
    )));
}

#[test]
fn a_hearth_draws_as_many_cards_as_the_shader_lays_out() {
    let shader = std::fs::read_to_string(crate::data::assets().join("shaders/fire.wesl")).unwrap();
    assert!(shader.contains(&format!(
        "const HEARTH_CARDS: f32 = {HEARTH_FLAME_CARDS}.0;"
    )));
    // Its ground, its lit card, its columns, tongues and glows inside the stack, and the glow
    // round its flames: every card one of them, none left over or missing.
    let count = |name: &str| -> usize {
        let line = shader
            .lines()
            .find(|l| l.starts_with(&format!("const {name}: i32 = ")))
            .unwrap();
        line.trim_end_matches(';')
            .rsplit(' ')
            .next()
            .unwrap()
            .parse()
            .unwrap()
    };
    let laid = 2 + count("HEARTH_COLUMNS") + count("HEARTH_TONGUES") + count("HEARTH_CORES") + 1;
    assert_eq!(laid, HEARTH_FLAME_CARDS);
}

#[test]
fn a_blast_flashes_for_a_moment_then_is_gone() {
    assert!(light::flash(0.0) > 0.99);
    assert!(light::flash(0.1) > 0.0 && light::flash(0.1) < light::flash(0.02));
    assert_eq!(light::flash(0.3), 0.0);
    assert_eq!(light::flash(-0.01), 0.0);
}

#[test]
fn a_campfire_clears_the_ground_a_stride_past_its_stones_and_no_farther() {
    let fire = Kindled {
        tall: 1.5,
        wide: 0.7,
        heat: 0.9,
        logs: super::Logs::CAMPFIRE,
        body: super::Body::Marched,
        light: super::HEARTH_LIGHT,
        rig: Rig::Hearth,
    };
    let at = Transform::from_translation(Vec3::new(3.0, 2000.0, -4.0));
    let ground = scorched(&fire, &at);
    let out = |m: f32| at.translation + Vec3::new(m * 0.6, 0.0, m * 0.8);
    assert!(ground.covers(at.translation, 0.0));
    assert!(ground.covers(out(1.8), 0.0));
    assert!(!ground.covers(out(2.4), 0.0));
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
        logs: super::Logs::CAMPFIRE,
        body: super::Body::Marched,
        light: super::HEARTH_LIGHT,
        rig: Rig::Hearth,
    };
    let lit = world
        .spawn((fire, GlobalTransform::from_translation(at)))
        .id();
    world.run_system_once(hearth::keep).unwrap();
    let drawn: Vec<Blaze> = world.resource::<Blazes>().all().copied().collect();
    assert_eq!(drawn.len(), 1);
    assert_eq!((drawn[0].at, drawn[0].tall, drawn[0].wide), (at, 0.6, 0.4));
    // Its kind's body reaches the shader in its slot (`fire.wesl`'s `hearth_marched`).
    assert_eq!(world.resource::<Blazes>().bodies, vec![1.0]);
    world.despawn(lit);
    world.run_system_once(hearth::keep).unwrap();
    assert_eq!(world.resource::<Blazes>().all().count(), 0);
}

#[test]
fn each_mask_is_drawn_after_the_flames_it_masks() {
    // A flame blended over its mask takes the share back as it covers, so the hearths' mask
    // comes after every flame and ember card, and the jets' after their hulls.
    for colour in [0, 1, 3, 4] {
        assert_eq!(drawn_kind(colour), colour as f32);
        assert!(
            drawn_bias(MASK) > drawn_bias(colour),
            "hearths' mask before material {colour}"
        );
    }
    assert!(drawn_bias(JET_MASK) > drawn_bias(2));
    // And before the breaths, which stand over the flames.
    assert!(drawn_bias(MASK) < drawn_bias(5));
}

#[test]
fn the_shader_moves_the_flames_at_the_pace_the_lights_and_loop_do() {
    let shader = std::fs::read_to_string(crate::data::assets().join("shaders/fire.wesl")).unwrap();
    assert!(shader.contains(&format!("const FLAME_PACE: f32 = {PACE:?};")));
}

#[test]
fn a_fires_light_flickers_with_the_glow_its_flames_are_drawn_by() {
    // `show` draws a kept fire's flames and embers by its glow at the flames' time; its light
    // flickers with them, not at the clock's own pace beside them.
    let campfire = Blaze {
        at: Vec3::new(0.0, 100.0, 0.0),
        tall: 1.5,
        heat: 0.9,
        seed: 0.3,
        wide: 0.7,
    };
    let blazes = Blazes {
        kept: vec![campfire],
        ..default()
    };
    let mut apart = 0.0f32;
    for i in 0..200 {
        let t = 10.0 + i as f32 * 0.05;
        let drawn = light::glow(&campfire, flame_time(t));
        let key = &light::lit(&blazes, t, BreathTier::Cards)[0];
        assert!(
            (key.glow - drawn).abs() < 1e-6,
            "at {t} s its light's glow is {}, its flames' {drawn}",
            key.glow
        );
        apart = apart.max((light::glow(&campfire, t) - drawn).abs());
    }
    // The flames' time is not the clock's, or the check above held nothing.
    assert!(apart > 0.01, "{apart}");
}
