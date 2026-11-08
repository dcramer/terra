//! A fighter and a player off its level (found by a scripted fight: the troll in its den
//! struck a player standing on the ground 18 m over its roof).

use super::*;

/// What a troll at home does, and what it lands, in so many seconds with a player standing
/// so far above it and three metres off across the ground.
fn with_a_player_above(rise_m: f32, seconds: f32) -> (State, Vec<f32>) {
    let mut world = world();
    let ground = Vec3::Y * 2000.0;
    test_player(&mut world, ground + Vec3::X * 3.0 + Vec3::Y * rise_m);
    let troll = kind(&mut world, "Troll", ground);
    let mut landed = Vec::new();
    for _ in 0..(seconds / 0.02) as usize {
        world
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(20));
        world.run_system_once(think).unwrap();
        let mut bites = world.resource_mut::<Messages<Bitten>>();
        landed.extend(bites.drain().map(|bite| bite.damage));
    }
    (world.get::<Creature>(troll).unwrap().state, landed)
}

#[test]
fn a_fighter_neither_notices_nor_strikes_a_player_over_its_roof() {
    // On its level, and a step up a bank: it goes for them, and its blows land.
    for rise_m in [0.0, 2.0] {
        let (_, landed) = with_a_player_above(rise_m, 12.0);
        assert!(!landed.is_empty(), "{rise_m} m above, nothing landed");
    }
    // Over its roof, or down a cliff from it: three metres off across the ground, and it
    // does not so much as rouse.
    for rise_m in [18.0, -18.0] {
        let (state, landed) = with_a_player_above(rise_m, 12.0);
        assert!(landed.is_empty(), "{rise_m} m above, it landed {landed:?}");
        assert!(
            !matches!(state, State::Attack(_) | State::Chase),
            "{rise_m} m above, it is at {state:?}"
        );
    }
}
