//! Leaving and coming back (D592, D637): where a guest stood, what is kept of them, and the
//! host's own leaving.

use super::*;

/// One who leaves and comes back is put where it stood (D592, D637), and when the host leaves
/// every guest is told so in words.
#[test]
fn a_guest_returns_to_where_it_left_and_is_told_when_the_host_leaves() {
    let (mut world, _, network) = hosted();
    let mut bot = Bot::join(&network, Some(Action::Forward));
    run(&mut world, &network, &mut bot, 16 + 96);
    let mut guests = world.query::<(&Guest, &Transform)>();
    let stood = guests.single(&world).expect("one guest").1.translation;
    let start = world
        .resource::<Planet>()
        .ground_at(Vec3::new(0.0, 2100.0, 3.0));
    assert!(stood.distance(start) > 4.0, "it walked before it left");
    // What a layer above keeps of it (D592) is its again in the greeting when it returns;
    // its first greeting, the host knowing nothing of it, was the realm alone.
    let greeting = |heard: &[Heard]| {
        heard.iter().find_map(|heard| match heard {
            Heard::Welcome {
                you: None, realm, ..
            } => Some(realm.clone()),
            _ => None,
        })
    };
    let first = greeting(&bot.heard).expect("it was greeted");
    assert!(greeted(&first).1.is_empty() && !greeted(&first).0.is_empty());
    let peer = world.resource::<Host>().peers()[0];
    (world.resource_mut::<Host>()).keep(peer, b"what it carried".to_vec());
    {
        use terra_net::transport::ClientTransport;
        bot.wire.close();
    }
    run(&mut world, &network, &mut bot, 4);
    assert_eq!(guests.iter(&world).count(), 0);
    // The same player again (the bot's one identity), holding nothing: where it stood.
    let mut back = Bot::join(&network, None);
    run(&mut world, &network, &mut back, 16 + 32);
    let returned = guests.single(&world).expect("it came back").1.translation;
    assert!(
        returned.distance(stood) < 0.2,
        "it came back {} m from where it left",
        returned.distance(stood)
    );
    assert_eq!(world.resource::<Host>().players(), ["bot"]);
    let again = greeting(&back.heard).expect("it was greeted again");
    assert_eq!(
        greeted(&again),
        (greeted(&first).0, &b"what it carried"[..])
    );
    // Bytes that are no greeting are all realm, whatever their last four say.
    assert_eq!(
        greeted(&[1, 2, 9, 9, 9, 9]),
        (&[1, 2, 9, 9, 9, 9][..], &[][..])
    );
    // The host leaves.
    world.resource_mut::<Host>().close();
    run(&mut world, &network, &mut back, 2);
    assert!(
        back.heard
            .contains(&Heard::Closed(terra_net::proto::Reason::HostLeft)),
        "{:?}",
        back.heard.last()
    );
}
