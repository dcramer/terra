use super::*;

/// A tree drawn as an oak 20 m tall, its crown 4 m round its stem.
fn drawn() -> Drawn {
    Drawn {
        to_m: 200.0,
        kind: TreeKind::Oak,
        crown_off: Vec3::Y * 12.0,
        crown_m: 4.0,
        trunk_m: 0.3,
        young: false,
        solid: false,
    }
}

use bevy::mesh::VertexAttributeValues;

/// The lush experiment's far forest draws by `far_mesh` too: a square's bounds hold its
/// cards whole, turned any way, though each is cut to its outline.
#[test]
fn a_squares_bounds_hold_its_cards_turned_any_way() {
    let up = Vec3::Y * 2000.0;
    let card = |foot, width, height, kind, luck| FarCard {
        foot: up + foot,
        width,
        height,
        kind,
        luck,
        comes_in: 0.0,
    };
    let cards = [
        card(Vec3::new(3.0, 0.0, -4.0), 10.0, 25.0, 0, 0.3),
        card(Vec3::new(-50.0, 0.4, 20.0), 6.0, 30.0, 1, 0.9),
    ];
    let (mesh, middle, aabb) = far_mesh(&cards);
    assert!(mesh.count_vertices() >= 8);
    let (low, high) = (Vec3::from(aabb.min()), Vec3::from(aabb.max()));
    for card in &cards {
        for turn in 0..16 {
            let angle = turn as f32 / 16.0 * std::f32::consts::TAU;
            let across = Vec3::new(libm::cosf(angle), 0.0, libm::sinf(angle));
            for corner in [-0.5f32, 0.5] {
                for rise in [0.0, 1.0] {
                    let p = card.foot - middle
                        + across * corner * card.width
                        + Vec3::Y * rise * card.height;
                    assert!(
                        p.cmpge(low - 1e-3).all() && p.cmple(high + 1e-3).all(),
                        "{p} outside {low} to {high}"
                    );
                }
            }
        }
    }
}

fn tree(at: Vec3, kind: TreeKind, stature: f32) -> Tree {
    Tree {
        at,
        kind,
        stature,
        luck: 0.5,
        footing: terra_land::woods::Footing::Soil,
        age: 1.0,
    }
}

#[test]
fn a_card_standing_for_a_grown_tree_comes_in_where_it_goes_and_looks_as_it_was_drawn() {
    let ground = Vec3::new(0.0, 30_000.0, 0.0);
    let cards = [
        // A tall pine grown near, drawn as a pine out to 588 m.
        Card {
            tree: tree(ground, TreeKind::Pine, 1.0),
            widen: 1.0,
            grown: Some(Drawn {
                to_m: 588.0,
                kind: TreeKind::Pine,
                ..drawn()
            }),
        },
        // A stunted pine grown near, drawn as a young spruce out to 189 m.
        Card {
            tree: tree(ground + Vec3::X * 20.0, TreeKind::Pine, 0.25),
            widen: 1.0,
            grown: Some(Drawn {
                to_m: 189.0,
                kind: TreeKind::Spruce,
                ..drawn()
            }),
        },
        // A pine past the grown trees.
        Card {
            tree: tree(ground + Vec3::X * 900.0, TreeKind::Pine, 1.0),
            widen: 1.0,
            grown: None,
        },
    ];
    let (mesh, _, _) = merged(&cards);
    let Some(VertexAttributeValues::Float32x4(colours)) = mesh.attribute(Mesh::ATTRIBUTE_COLOR)
    else {
        panic!("no colours");
    };
    let Some(VertexAttributeValues::Float32x2(sizes)) = mesh.attribute(Mesh::ATTRIBUTE_UV_1) else {
        panic!("no sizes");
    };
    let Some(VertexAttributeValues::Float32x3(feet)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION)
    else {
        panic!("no feet");
    };
    // A card's corners (four a band of its outline) all at its foot, each telling the
    // shader the same but for which way it is widened.
    let firsts: Vec<usize> = (0..feet.len())
        .filter(|&i| i == 0 || feet[i] != feet[i - 1])
        .collect();
    assert_eq!(firsts.len(), cards.len());
    let card = |k: usize| (colours[firsts[k]], sizes[firsts[k]]);
    let (tall, tall_size) = card(0);
    assert_eq!(tall[0], 0.5, "a pine's card");
    assert!((tall[2] * HANDOVER_SCALE_M - 588.0).abs() < 1e-3);
    let (stunted, stunted_size) = card(1);
    assert_eq!(stunted[0], 1.0, "drawn as a spruce, its card a spruce's");
    assert!((stunted[2] * HANDOVER_SCALE_M - 189.0).abs() < 1e-3);
    // Krummholz's card is squat as its tree: wider for its height than a tall one's.
    assert!(stunted_size[0] / stunted_size[1] > tall_size[0] / tall_size[1]);
    let (beyond, _) = card(2);
    assert_eq!(beyond[2], 0.0, "no grown tree to come in after");
}

#[test]
fn a_tree_toward_its_limit_is_shorter_and_squatter() {
    let ground = Vec3::new(0.0, 30_000.0, 0.0);
    let (tall, low) = (
        planted(&tree(ground, TreeKind::Spruce, 1.0)),
        planted(&tree(ground, TreeKind::Spruce, 0.3)),
    );
    assert!(low.height_m < tall.height_m * 0.4);
    assert_eq!(tall.squat, 1.0);
    assert!(low.squat > 1.5);
    // Standing up off the ground, whatever its turn.
    assert!((tall.rotation * Vec3::Y).dot(ground.normalize()) > 0.999);
}

/// A grown tree felled takes its far card with it: the square holding it is merged again
/// without it, so it does not come back as a billboard past its last level, and the square's
/// other cards stay as they were.
#[test]
fn a_felled_grown_tree_leaves_its_far_square_and_the_rest_stay() {
    use crate::flora::Growth;
    let ground = Vec3::new(0.0, 30_000.0, 0.0);
    // Both in the square east and north of the middle.
    let felled_at = ground + Vec3::new(10.0, 0.0, -5.0);
    let kept_at = ground + Vec3::new(30.0, 0.0, -40.0);
    let card = |at, grown| Card {
        tree: tree(at, TreeKind::Pine, 0.8),
        widen: 1.0,
        grown,
    };
    let cards = vec![card(felled_at, Some(drawn())), card(kept_at, None)];
    let both = merged(&cards).0.count_vertices();
    let one = merged(&cards[1..]).0.count_vertices();
    assert!(one < both);

    let mut app = App::new();
    app.init_resource::<Assets<Mesh>>()
        .add_systems(Update, felled::unfar_the_felled);
    let mut index = felled::FarSquares::new(ground, Vec3::X, Vec3::NEG_Z);
    assert_eq!(index.square_of(felled_at), index.square_of(kept_at));
    let (mesh, middle, aabb) = merged(&cards);
    let handle = app.world_mut().resource_mut::<Assets<Mesh>>().add(mesh);
    let square = app
        .world_mut()
        .spawn((
            Mesh3d(handle),
            Transform::from_translation(middle),
            aabb,
            felled::FarSquare { cards },
        ))
        .id();
    index.by_square.insert(index.square_of(felled_at), square);
    app.insert_resource(index);
    let map = near::Map::new(ground, Vec3::X, Vec3::NEG_Z, ground.length());
    let streamed = near::NearWoods::new(map, terra_gen::Seed(1), Arc::new(Bare));
    app.insert_resource(streamed);
    let standing = app
        .world_mut()
        .spawn((
            felled::WoodsTree { foot: felled_at },
            Growth::new(None, Some(felled_at), 1.4),
        ))
        .id();
    let vertices = |app: &App| {
        let handle = &app.world().get::<Mesh3d>(square).unwrap().0;
        let meshes = app.world().resource::<Assets<Mesh>>();
        meshes.get(handle).unwrap().count_vertices()
    };
    app.update();
    assert_eq!(vertices(&app), both, "standing, its card is drawn");

    app.world_mut().entity_mut(standing).remove::<Growth>();
    app.update();
    let far = app.world().get::<felled::FarSquare>(square).unwrap();
    assert_eq!(far.cards.len(), 1);
    assert_eq!(far.cards[0].tree.at, kept_at);
    assert_eq!(vertices(&app), one, "felled, only the other card is drawn");
    // And it stays felled: its block, streamed in again, leaves it out (D622).
    let streamed = app.world().resource::<near::NearWoods>();
    assert!(streamed.felled.contains(&foot_key(felled_at)));
    assert!(!streamed.felled.contains(&foot_key(kept_at)));
}

/// Conifers drawn as nothing, for the woods' own systems alone.
struct Bare;

impl ConiferModels for Bare {
    fn draw(&self, _: &mut EntityCommands, _: &Planted) -> Drawn {
        drawn()
    }
    fn drawn(&self, _: &Planted) -> Drawn {
        drawn()
    }
    fn far_crowns(&self) -> [Vec3; 3] {
        [Vec3::ONE; 3]
    }
}
