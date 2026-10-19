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
