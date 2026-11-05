use super::*;
use crate::flora::{Growth, Obstacles};

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
    let mut woods = FarWoods::new();
    woods.app.update();
    assert_eq!(woods.vertices(), woods.both, "standing, its card is drawn");

    let standing = woods.standing;
    woods
        .app
        .world_mut()
        .entity_mut(standing)
        .remove::<Growth>();
    woods.app.update();
    woods.only_the_other_card_left("felled");
}

/// A grown tree despawned outright, as `world.fell` takes the trees round a set piece, takes
/// its far card with it as one felled for its logs does: it is filed as it goes, while it is
/// still there to read.
#[test]
fn a_grown_tree_despawned_outright_leaves_its_far_square() {
    let mut woods = FarWoods::new();
    woods.app.update();
    let standing = woods.standing;
    woods.app.world_mut().despawn(standing);
    woods.app.update();
    woods.only_the_other_card_left("despawned");
}

/// A grown tree streamed out, past the near woods' reach, is not felled: its far card stays
/// drawn and its block, streamed in again, lays it again.
#[test]
fn a_grown_tree_streamed_out_keeps_its_far_card() {
    let mut woods = FarWoods::new();
    woods.app.update();
    let standing = woods.standing;
    felled::take_out(&mut woods.app.world_mut().commands(), standing);
    woods.app.update();
    assert!(
        woods.app.world().get_entity(standing).is_err(),
        "it is gone"
    );
    assert_eq!(woods.vertices(), woods.both, "its card is still drawn");
    let streamed = woods.app.world().resource::<near::NearWoods>();
    assert!(!streamed.felled.contains(&foot_key(woods.felled_at)));
}

/// A grown tree cleared away by what is set down where it stood (as building over it or
/// digging under it does, `flora`'s `make_way` and `follow_digs`) is hidden and takes its far
/// card with it, as one felled does; one standing off the clearing keeps its card.
#[test]
fn a_cleared_grown_tree_leaves_its_far_square_as_a_felled_one_does() {
    let mut woods = FarWoods::new();
    woods.app.update();
    let standing = woods.standing;
    let felled_at = woods.felled_at;
    woods
        .app
        .world_mut()
        .write_message(crate::flora::ClearAround(felled_at, 3.0));
    woods.app.update();
    let hidden = woods.app.world().get::<Visibility>(standing);
    assert_eq!(
        hidden,
        Some(&Visibility::Hidden),
        "cleared away, it is hidden"
    );
    woods.only_the_other_card_left("cleared");
}

/// A square of two far cards, one of a grown woods tree standing there and one of a tree
/// 40 m off, with the systems that clear and fell what grows.
struct FarWoods {
    app: App,
    square: Entity,
    standing: Entity,
    felled_at: Vec3,
    kept_at: Vec3,
    both: usize,
    one: usize,
}

impl FarWoods {
    fn new() -> Self {
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
            .init_resource::<Obstacles>()
            .add_observer(felled::file_the_felled)
            .add_message::<crate::flora::ClearAround>()
            .add_message::<crate::flora::ClearedAway>()
            .add_systems(
                Update,
                (crate::flora::clear_around, felled::unfar_the_felled).chain(),
            );
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
                Transform::from_translation(felled_at),
                Visibility::Inherited,
            ))
            .id();
        Self {
            app,
            square,
            standing,
            felled_at,
            kept_at,
            both,
            one,
        }
    }

    /// The vertices the square draws.
    fn vertices(&self) -> usize {
        let handle = &self.app.world().get::<Mesh3d>(self.square).unwrap().0;
        let meshes = self.app.world().resource::<Assets<Mesh>>();
        meshes.get(handle).unwrap().count_vertices()
    }

    /// The square keeps only the card of the tree off the clearing, and draws only it; and
    /// the tree stays gone: its block, streamed in again, leaves it out (D622).
    fn only_the_other_card_left(&self, how: &str) {
        let far = self
            .app
            .world()
            .get::<felled::FarSquare>(self.square)
            .unwrap();
        assert_eq!(far.cards.len(), 1, "{how}, its card is gone");
        assert_eq!(far.cards[0].tree.at, self.kept_at);
        assert_eq!(
            self.vertices(),
            self.one,
            "{how}, only the other card is drawn"
        );
        let streamed = self.app.world().resource::<near::NearWoods>();
        assert!(
            streamed.felled.contains(&foot_key(self.felled_at)),
            "{how}, kept out"
        );
        assert!(!streamed.felled.contains(&foot_key(self.kept_at)));
    }
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
