//! What `Creature::frightened` says, which a cry of alarm is heard by (D657): a wood's crows
//! cried at every takeoff from perch to perch, 115 times a minute.

use super::*;

#[test]
fn a_bird_put_up_is_frightened_and_one_changing_perch_is_not() {
    let mut world = world();
    let ground = Vec3::Y * 2000.0;
    // Nobody about: it takes wing of its own accord sooner or later, unfrightened.
    let bird = kind(&mut world, "Chaffinch", ground);
    let mut flew = false;
    for _ in 0..400 {
        world.get_mut::<Creature>(bird).unwrap().state = State::Idle(0.0);
        world.run_system_once(think).unwrap();
        if matches!(state(&world, bird), State::Fly { .. }) {
            flew = true;
            break;
        }
    }
    assert!(
        flew,
        "a chaffinch left alone never took wing in 400 choices"
    );
    assert!(!world.get::<Creature>(bird).unwrap().frightened());
    // Someone two metres off puts it up: frightened until it lands.
    world.get_mut::<Creature>(bird).unwrap().state = State::Idle(1.0);
    let player = test_player(&mut world, ground + Vec3::X * 2.0);
    world.run_system_once(think).unwrap();
    let State::Fly { from, to, .. } = state(&world, bird) else {
        panic!("it should fly, not {:?}", state(&world, bird));
    };
    assert!(world.get::<Creature>(bird).unwrap().frightened());
    world.despawn(player);
    let along = 1.0;
    world.get_mut::<Creature>(bird).unwrap().state = State::Fly { from, to, along };
    world.run_system_once(think).unwrap();
    assert!(!matches!(state(&world, bird), State::Fly { .. }));
    assert!(!world.get::<Creature>(bird).unwrap().frightened());
    // Running from someone is fright; crossing their way calmly is not.
    let mut creature = world.get_mut::<Creature>(bird).unwrap();
    creature.dash(Vec3::X, 5.0, Pace::Startled);
    assert!(creature.frightened());
    creature.dash(Vec3::X, 5.0, Pace::Calm);
    assert!(!creature.frightened());
}
