//! The streamed woods floor's tests (`stream.rs`), apart so it stays readable.

use super::*;
use bevy::asset::AssetPlugin;
use bevy::ecs::system::RunSystemOnce;
use terra_core::PlanetSpec;
use terra_world::cover::forest::{Crowns, Wild};
use terra_world::trees::Species;

/// The woods' middle, and the dice the pieces roll by.
const MIDDLE: Vec2 = Vec2::ZERO;
const SEED: Seed = Seed(5);
/// A row of pieces well past the load's reach, under trees.
const ROW: std::ops::Range<i32> = 15..75;

#[derive(Resource)]
struct Map(Floor<Wild>);

#[derive(Resource)]
struct Laid(Kit, Frame);

/// A world to lay pieces in: a smooth planet, the flora's kinds, the floor's looks and a kit
/// with nothing to load, and the woods' floor with oaks every 6 m over the row.
fn app() -> App {
    let planet = Planet::smooth(PlanetSpec {
        seed: Seed(1),
        radius_m: 2000.0,
    });
    let start = planet.start();
    let up = start.normalize();
    let east = up.any_orthonormal_vector();
    let mut canopy = Crowns::default();
    for k in 0..(ROW.end * 3 * 7) {
        let at = Vec2::new(
            ROW.start as f32 * PIECE_M - 10.0 + (k / 7) as f32 * 6.0,
            -10.0 + (k % 7) as f32 * 6.0,
        );
        canopy.add(Stand::of(at, at, 4.0, Species::Oak, false, 0.4));
    }
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Mesh>()
        .init_asset::<StandardMaterial>()
        .init_asset::<floor::Mossy>()
        .init_asset::<Foliage>()
        .init_resource::<MossyScans>()
        .init_resource::<Variants>()
        .insert_resource(planet)
        .insert_resource(Flora::read())
        .insert_resource(Cards::read())
        .insert_resource(FloorLooks {
            bark: default(),
            floor_cards: default(),
            thicket_cards: default(),
            pine_bark: default(),
            spruce_bark: default(),
        })
        .insert_resource(Map(Floor::new(Wild, canopy, Seed(4))))
        .insert_resource(Laid(Kit::bare(), (start, east, east.cross(up))));
    app
}

/// Lays a piece as the stream does, beside the pieces `around` it laid before, leaving out
/// the plants `cut`: what it put, and where each of its things stands.
fn lay(app: &mut App, key: Key, cut: &HashSet<u32>, around: &[&Put]) -> (Put, Vec<Transform>) {
    let cut = cut.clone();
    let around: Vec<Put> = around.iter().map(|put| copy(put)).collect();
    let put = app
        .world_mut()
        .run_system_once(
            move |mut commands: Commands,
                  mut laying: Laying,
                  laid: Res<Laid>,
                  mut map: ResMut<Map>| {
                let mut lay = laying.lay(&mut commands, laid.1, SEED);
                let around: Vec<&Put> = around.iter().collect();
                let at = (key, (MIDDLE, SEED), cut.clone());
                lay_alone(&mut lay, &laid.0, &mut map.0, at, &around)
            },
        )
        .unwrap();
    let world = app.world();
    let places = put
        .things
        .iter()
        .map(|&thing| *world.get::<Transform>(thing).expect("spawned"))
        .collect();
    (put, places)
}

/// What a piece put on the floor, to lay another beside it.
fn copy(put: &Put) -> Put {
    let mut copy = Put::default();
    copy.lying = put.lying.clone();
    copy.feet = put.feet.clone();
    copy.stumps = put.stumps.clone();
    copy
}

/// Streams a piece out as the stream does: its things gone, and what it filed taken off.
fn stream_out(app: &mut App, put: &Put) {
    for &thing in &put.things {
        app.world_mut().despawn(thing);
    }
    take_off(&mut app.world_mut().resource_mut::<Map>().0, put);
}

/// The floor over the row, as the laying and the cover read it: what lies there, the plants'
/// feet, the bark and the light, on a 1 m grid.
fn floor_over_row(app: &App) -> Vec<[f32; 4]> {
    let map = &app.world().resource::<Map>().0;
    let (from, to) = (low((ROW.start, 0)), low((ROW.end, 1)));
    let across = (to - from).as_ivec2();
    (0..across.x * across.y)
        .map(|k| from + Vec2::new((k % across.x) as f32, (k / across.x) as f32))
        .map(|at| {
            let here = map.here(at);
            [here.lying, here.feet, here.bark, here.light]
        })
        .collect()
}

/// The pieces of the row next to one, laid already.
fn laid_round(key: Key, laid: &HashMap<Key, (Put, Vec<Transform>)>) -> Vec<&Put> {
    around(key)
        .filter_map(|next| laid.get(&next).map(|(put, _)| put))
        .collect()
}

/// The row laid west to east, streamed out, and laid again east to west: each piece lays the
/// same things in the same places, though the pieces beside it were in the other time; and
/// once streamed out, the floor is as bare as before it was laid.
#[test]
fn a_piece_streamed_out_and_in_again_lays_the_same_whichever_were_in_round_it() {
    let mut app = app();
    let bare = floor_over_row(&app);
    let none = HashSet::default();
    let mut first: HashMap<Key, (Put, Vec<Transform>)> = HashMap::default();
    for x in ROW {
        let key = (x, 0);
        let laid = lay(&mut app, key, &none, &laid_round(key, &first));
        first.insert(key, laid);
    }
    let all = |pick: fn(&Put) -> usize| first.values().map(|(put, _)| pick(put)).sum::<usize>();
    assert!(all(|put| put.things.len()) > 100, "the row is laid");
    assert!(all(|put| put.lying.len()) > 0, "rocks or logs lie there");
    assert!(all(|put| put.feet.len()) > 0, "shrubs stand in their wells");
    assert!(all(|put| put.stumps.len()) > 0, "stumps stand there");
    assert_ne!(floor_over_row(&app), bare, "the floor knows what was laid");
    for (put, _) in first.values() {
        stream_out(&mut app, put);
    }
    assert_eq!(
        floor_over_row(&app),
        bare,
        "streamed out, the floor is bare again"
    );
    let mut again: HashMap<Key, (Put, Vec<Transform>)> = HashMap::default();
    for x in ROW.rev() {
        let key = (x, 0);
        let laid = lay(&mut app, key, &none, &laid_round(key, &again));
        again.insert(key, laid);
    }
    for (key, (put, places)) in &first {
        let (put_again, places_again) = &again[key];
        assert_eq!(places, places_again, "piece {key:?}'s things");
        assert_eq!(put.lying, put_again.lying, "piece {key:?}'s lying");
        assert_eq!(put.feet, put_again.feet, "piece {key:?}'s feet");
        assert_eq!(put.stumps, put_again.stumps, "piece {key:?}'s stumps");
    }
}

/// The pieces to lay round the camera are the nearest first, within reach of it, and none the
/// load laid whole round the woods' middle.
#[test]
fn the_pieces_waiting_are_the_nearest_first_and_none_the_load_laid() {
    let eye = Vec2::new(150.0, 0.0);
    let waiting = waiting(eye, MIDDLE, &HashMap::default());
    assert!(!waiting.is_empty());
    let away: Vec<f32> = waiting
        .iter()
        .map(|&key| middle_of(key).distance(eye))
        .collect();
    assert!(
        away.windows(2).all(|pair| pair[0] <= pair[1]),
        "nearest first"
    );
    assert!(away.iter().all(|&m| m <= ENTER_M), "all within reach");
    for &key in &waiting {
        let corners = [low(key), low(key) + Vec2::splat(PIECE_M)];
        let farthest = corners
            .iter()
            .flat_map(|a| corners.iter().map(move |b| Vec2::new(a.x, b.y)))
            .map(|corner| corner.distance(MIDDLE))
            .fold(0.0, f32::max);
        assert!(
            farthest > REACH_M,
            "piece {key:?} lies wholly in the load's floor"
        );
    }
    let straddling = piece_of(Vec2::new(REACH_M, 0.0));
    assert!(
        waiting.contains(&straddling),
        "a piece across the load's edge is laid"
    );
}
