use super::*;
use crate::model_files;

#[test]
fn a_body_is_as_tall_as_it_says_and_holds_things_at_their_own_size() {
    use bevy::ecs::system::RunSystemOnce;
    let mut world = World::new();
    let body = world
        .spawn((
            Body,
            Appearance {
                height: 1.2,
                ..Appearance::default()
            },
            Transform::default(),
        ))
        .id();
    let hand = world.spawn((Transform::default(), ChildOf(body))).id();
    let held = world
        .spawn((Held::default(), Transform::default(), ChildOf(hand)))
        .id();
    world.run_system_once(size_bodies).unwrap();
    world.run_system_once(keep_held_size).unwrap();
    assert_eq!(
        world.get::<Transform>(body).unwrap().scale,
        Vec3::splat(1.2)
    );
    assert!((world.get::<Transform>(held).unwrap().scale - Vec3::splat(1.0 / 1.2)).length() < 1e-6);
}

#[test]
fn templates_parse_and_name_their_morphs() {
    let templates =
        parse_templates(r#"{"male": {}, "female": {"morphs": {"female": 1.0}, "skin": "female"}}"#)
            .unwrap();
    assert_eq!(templates["male"], Appearance::default());
    assert_eq!(templates["male"].height, 1.0);
    assert_eq!(templates["female"].weight("female"), 1.0);
    assert_eq!(templates["female"].skin.as_deref(), Some("female"));
}

#[test]
fn the_shipped_templates_name_morphs_and_skins_there_are() {
    // Read as the game reads it, without the fallback that would leave no templates.
    let templates = parse_templates(include_str!(
        "../../../../assets/models/character/templates.json"
    ))
    .unwrap();
    let morphs = model_files::morphs(&model_files::json(BASE));
    assert!(!morphs.is_empty());
    for (name, template) in &templates {
        // A morph the body lacks would be left out of its shape without a word.
        for (morph, weight) in &template.morphs {
            assert!(
                morphs.contains(morph),
                "{name}: the base body has no {morph}"
            );
            assert!(weight.is_finite(), "{name}: {morph}");
        }
        if let Some(skin) = &template.skin {
            let file = skin_file(skin);
            assert!(model_files::asset(&file).exists(), "{name}: no {file}");
        }
        assert!(template.height > 0.0, "{name}");
    }
    // That the templates the game asks for by name are here is checked where they are
    // named (`terra_folk::people`).
}

#[test]
fn the_base_body_says_how_each_of_its_morphs_moves_its_joints() {
    let base = model_files::json(BASE);
    let offsets = model_files::extras(&base)
        .iter()
        .find_map(|(_, extras)| offsets_in(&extras.to_string()))
        .expect("the base body's file carries no morph offsets that read");
    let morphs = model_files::morphs(&base);
    let nodes = model_files::nodes(&base);
    for morph in &morphs {
        assert!(offsets.contains_key(morph), "nothing moves for {morph}");
    }
    for (morph, moved) in &offsets {
        assert!(morphs.contains(morph), "offsets for {morph}, not a morph");
        for node in moved.keys() {
            assert!(nodes.contains(node), "{morph} moves {node}, not a node");
        }
    }
}

#[test]
fn a_mix_is_each_morphs_value_by_its_weight() {
    let value = |morph: &str| match morph {
        "heavy" => Some(Vec3::new(0.0, 0.0, -0.04)),
        "strong" => Some(Vec3::new(0.0, 0.02, -0.02)),
        _ => None,
    };
    let mix = |weights: &[(&str, f32)]| {
        let weights: Vec<(String, f32)> =
            weights.iter().map(|(m, w)| (m.to_string(), *w)).collect();
        mix(&weights, value)
    };
    // Half heavy and fully strong: -0.02 and -0.02 back, 0.02 up; thin gives nothing.
    let both = mix(&[("heavy", 0.5), ("strong", 1.0), ("thin", 1.0)]);
    assert!(
        (both - Vec3::new(0.0, 0.02, -0.04)).length() < 1e-6,
        "{both}"
    );
    // A morph named twice counts twice, as `Appearance::weight` adds them.
    let twice = mix(&[("heavy", 0.25), ("heavy", 0.25)]);
    assert!(
        (twice - Vec3::new(0.0, 0.0, -0.02)).length() < 1e-6,
        "{twice}"
    );
    assert_eq!(mix(&[]), Vec3::ZERO);
}

#[test]
fn morphs_mix_by_weight() {
    let offsets = parse_offsets(
        r#"{"heavy": {"socket.back": [0.0, 0.0, -0.04]}, "female": {"socket.back": [0.0, -0.1, 0.0], "Head": [0.0, -0.1, 0.0]}}"#,
    )
    .unwrap();
    let appearance = Appearance {
        morphs: vec![("heavy".into(), 0.5), ("female".into(), 1.0)],
        ..Appearance::default()
    };
    let moved = mixed(&offsets, &appearance);
    assert!((moved["socket.back"] - Vec3::new(0.0, -0.1, -0.02)).length() < 1e-6);
    assert!((moved["Head"] - Vec3::new(0.0, -0.1, 0.0)).length() < 1e-6);
}

/// Where skinning draws `vertex` (in the mesh's space) of a mesh whose space is turned
/// `model_from_mesh` in the model's, bound to a joint standing at `joint` in the model,
/// before and after the joint moves `by` and the mesh is bound again for it.
fn drawn_before_and_after(
    model_from_mesh: Quat,
    joint: Mat4,
    by: Vec3,
    rebound: impl Fn(Mat4) -> Mat4,
    vertex: Vec3,
) -> (Vec3, Vec3) {
    // A bind pose takes the mesh's space into the joint's at rest.
    let pose = (Mat4::from_quat(model_from_mesh).inverse() * joint).inverse();
    let moved = Mat4::from_translation(by) * joint;
    (
        (joint * pose).transform_point3(vertex),
        (moved * rebound(pose)).transform_point3(vertex),
    )
}

#[test]
fn a_moved_joint_bound_again_leaves_its_skin_where_it_was() {
    // Skinning draws a vertex at joint times bind pose times vertex. Moved and bound again,
    // a joint must draw the skin where it was: the morph, not the joint, moves the skin.
    // So in a mesh as the model has it, and in meshes turned in the model.
    let joint = Mat4::from_translation(Vec3::new(0.2, 1.0, -0.1)) * Mat4::from_rotation_z(0.3);
    let by = Vec3::new(0.05, -0.1, 0.02);
    for model_from_mesh in [
        Quat::IDENTITY,
        Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
        Quat::from_rotation_x(0.7),
    ] {
        let mesh_from_model = Mat3::from_quat(model_from_mesh.inverse());
        for vertex in [
            Vec3::ZERO,
            Vec3::new(0.2, 1.3, 0.0),
            Vec3::new(-0.4, 0.1, 0.6),
        ] {
            let (before, after) = drawn_before_and_after(
                model_from_mesh,
                joint,
                by,
                |pose| moved_bindposes(&[pose], &[by], mesh_from_model)[0],
                vertex,
            );
            assert!(
                before.distance(after) < 1e-5,
                "{model_from_mesh}: {before} became {after}"
            );
        }
    }
}

#[test]
fn a_joint_held_by_its_appearance_moves_from_where_the_animation_put_it_or_its_rest() {
    use bevy::ecs::system::RunSystemOnce;
    let mut world = World::new();
    let rest = Vec3::new(0.0, 1.0, 0.0);
    let by = Vec3::new(0.0, -0.1, 0.0);
    let joint = world.spawn(Transform::from_translation(rest)).id();
    world.spawn(Moved(vec![MovedNode {
        node: joint,
        rest,
        by,
        held: Some(rest),
    }]));
    let frame = |world: &mut World, animated: Option<Vec3>| {
        if let Some(at) = animated {
            world.get_mut::<Transform>(joint).unwrap().translation = at;
        }
        world.run_system_once(hold_joints).unwrap();
        world.get::<Transform>(joint).unwrap().translation
    };
    // Frames no clip sets it: moved from its rest, and never further each frame.
    assert!(frame(&mut world, None).distance(rest + by) < 1e-6);
    assert!(frame(&mut world, None).distance(rest + by) < 1e-6);
    // A clip sets it: moved from there.
    let animated = Vec3::new(0.0, 1.2, 0.05);
    assert!(frame(&mut world, Some(animated)).distance(animated + by) < 1e-6);
    assert!(frame(&mut world, None).distance(rest + by) < 1e-6);
    // A frame that leaves it where it is writes nothing, so its skeleton is not walked
    // again by the transform propagation.
    let changed = |world: &World| {
        world
            .entity(joint)
            .get_ref::<Transform>()
            .unwrap()
            .last_changed()
    };
    let before = changed(&world);
    frame(&mut world, None);
    assert_eq!(changed(&world), before, "an unmoved joint was written");
}

#[test]
fn a_rest_is_chained_up_to_the_root_and_not_past_it() {
    use bevy::ecs::system::RunSystemOnce;
    let mut world = World::new();
    let root = world.spawn_empty().id();
    let arm = world.spawn(ChildOf(root)).id();
    let hand = world.spawn(ChildOf(arm)).id();
    let rest = Rest(HashMap::from([
        // The root's own rest places the model in the world, and is left out.
        (root, Transform::from_xyz(100.0, 0.0, 0.0)),
        (
            arm,
            Transform::from_xyz(0.0, 1.0, 0.0)
                .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
        ),
        (hand, Transform::from_xyz(1.0, 0.0, 0.0)),
    ]));
    let at = world
        .run_system_once(move |parents: Query<&ChildOf>| {
            rest_from_root(hand, root, &rest, &parents)
        })
        .unwrap();
    // The hand is 1 along the arm's X, which the quarter turn points up: 1 above the
    // arm, 2 above the root, and turned as the arm is.
    assert!(
        Vec3::from(at.translation).distance(Vec3::Y * 2.0) < 1e-6,
        "{at:?}"
    );
    assert!(Vec3::from(at.matrix3 * Vec3A::X).distance(Vec3::Y) < 1e-6);
}

#[test]
fn a_worn_look_takes_its_bodys_shape() {
    use bevy::ecs::system::RunSystemOnce;
    let mut world = World::new();
    world.init_resource::<Shared>();
    world.insert_resource(Assets::<Mesh>::default());
    world.insert_resource(Assets::<SkinnedMeshInverseBindposes>::default());
    let weights = vec![("heavy".to_string(), 0.5), ("thin".to_string(), 0.25)];
    let by = Vec3::new(0.0, -0.1, 0.05);
    let body = world
        .spawn(Shape {
            moved: HashMap::from([("Head".to_string(), by)]),
            weights_by_name: weights,
        })
        .id();
    let head = world.spawn((Name::new("Head"), ChildOf(body))).id();
    let head_at = Mat4::from_translation(Vec3::new(0.0, 1.6, 0.05));

    // A rigid look hung on a socket, shifted beyond it by each morph.
    let hung = world
        .spawn((
            Worn { body },
            Covers(vec![]),
            Rest(HashMap::new()),
            Shifts(HashMap::from([
                ("heavy".to_string(), Vec3::new(0.0, 0.0, -0.04)),
                ("thin".to_string(), Vec3::new(0.02, 0.0, 0.0)),
            ])),
            Transform::default(),
        ))
        .id();

    // A skinned look whose mesh is turned a quarter in the look, with morphs of its own
    // in an order of its own.
    let turned = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
    let pose = (Mat4::from_quat(turned).inverse() * head_at).inverse();
    let poses = world
        .resource_mut::<Assets<SkinnedMeshInverseBindposes>>()
        .add(SkinnedMeshInverseBindposes::from(vec![pose]));
    let shape = world.resource_mut::<Assets<Mesh>>().add(
        Mesh::new(
            bevy::mesh::PrimitiveTopology::TriangleList,
            bevy::asset::RenderAssetUsages::default(),
        )
        .with_morph_target_names(vec!["thin".into(), "female".into(), "heavy".into()]),
    );
    let skinned = world.spawn(Worn { body }).id();
    let mesh = world
        .spawn((
            SkinnedMesh {
                inverse_bindposes: poses.clone(),
                joints: vec![head],
            },
            MorphWeights::new(vec![0.0; 3], Some(shape)).unwrap(),
            ChildOf(skinned),
        ))
        .id();
    world.entity_mut(skinned).insert((
        Covers(vec![]),
        Rest(HashMap::from([(mesh, Transform::from_rotation(turned))])),
    ));

    world.run_system_once(fit_worn).unwrap();

    // Half the heavy shift and a quarter of the thin.
    let shifted = world.get::<Transform>(hung).unwrap().translation;
    assert!(
        shifted.distance(Vec3::new(0.005, 0.0, -0.02)) < 1e-6,
        "{shifted}"
    );
    // Its morphs by name, whatever their order.
    let morphs = world.get::<MorphWeights>(mesh).unwrap().weights().to_vec();
    assert_eq!(morphs, vec![0.25, 0.0, 0.5]);
    // Bound again for the moved head, in the mesh's own turned space: the skin is drawn
    // where it was, before the morph moves it.
    let bound = world
        .get::<SkinnedMesh>(mesh)
        .unwrap()
        .inverse_bindposes
        .clone();
    assert_ne!(bound.id(), poses.id());
    let rebound = world
        .resource::<Assets<SkinnedMeshInverseBindposes>>()
        .get(&bound)
        .unwrap()[0];
    for vertex in [Vec3::ZERO, Vec3::new(0.1, 1.5, -0.2)] {
        let (before, after) = drawn_before_and_after(turned, head_at, by, |_| rebound, vertex);
        assert!(before.distance(after) < 1e-5, "{before} became {after}");
    }
}
