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
    app_of(true, None)
}

/// The same world as a realm has it: with the first realm's oaks over the row or none of its
/// trees (`trees: 0`), and the realm's own undergrowth if it has any.
fn app_of(trees: bool, own: Option<&'static terra_world::realm::Undergrowth>) -> App {
    let mut planet = Planet::smooth(PlanetSpec {
        seed: Seed(1),
        radius_m: 2000.0,
    });
    if !trees {
        planet.set_plants(1.0, Some(0.0));
    }
    planet.set_undergrowth(own);
    let start = planet.start();
    let up = start.normalize();
    let east = up.any_orthonormal_vector();
    let mut canopy = Crowns::default();
    for k in 0..(ROW.end * 3 * 7 * i32::from(trees)) {
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
        .insert_resource(Laid(Kit::bare_of(own), (start, east, east.cross(up))));
    app
}

/// How many plants of each kind the row's first twenty pieces lay, by the kinds' names.
fn plants_laid(app: &mut App) -> HashMap<&'static str, u32> {
    let mut laid: HashMap<&'static str, u32> = HashMap::default();
    for x in ROW.take(20) {
        let (put, _) = lay(app, (x, 0), &HashSet::default(), &[]);
        for &(name, count, _) in &put.undergrowth {
            *laid.entry(name).or_default() += count;
        }
    }
    laid
}

/// The far realm's floor was bare (the developers, 2026-11-09: "its not lush"): the woods'
/// floor is laid only among the trees on its map, the first realm's, and the far realm grows
/// none of them, its forest being its own parts. A realm with none of the first realm's
/// trees lays its floor on open ground; the first realm's still keeps to its trees.
#[test]
fn a_realm_with_none_of_the_first_realms_trees_still_lays_its_floor() {
    let count = |laid: HashMap<&'static str, u32>| laid.values().sum::<u32>();
    let wooded = count(plants_laid(&mut app_of(true, None)));
    let treeless_first = {
        // The first realm's own rule: no trees on the map, no floor (the land's heath).
        let mut app = app_of(true, None);
        app.world_mut().resource_mut::<Map>().0.canopy = Crowns::default();
        count(plants_laid(&mut app))
    };
    let far = count(plants_laid(&mut app_of(false, None)));
    assert!(wooded > 1000, "the first realm's row under oaks: {wooded}");
    assert_eq!(treeless_first, 0, "the first realm's open ground");
    // Fewer than under oaks, as only the first realm's kinds of open ground grow where no
    // crown shades it (877 for 2,071): a realm's floor is full by its own kinds (below).
    assert!(
        far > wooded / 4,
        "a realm of its own trees lays its floor without the first realm's: {far} plants \
         for the first realm's {wooded}"
    );
}

/// A realm's own undergrowth is what its floor lays (D642, D136): every kind of its row, and
/// of the first realm's only those it keeps; the first realm lays none of another's.
#[test]
fn a_realms_floor_lays_its_own_kinds_and_only_the_first_realms_it_keeps() {
    let realms = terra_world::realm::Realms::shared();
    let own = realms
        .of("industrial")
        .and_then(|realm| realm.undergrowth.as_ref())
        .expect("the far realm has undergrowth of its own");
    let laid = plants_laid(&mut app_of(false, Some(own)));
    for kind in &own.kinds {
        let count = laid.get(kind.name.as_str()).copied().unwrap_or(0);
        assert!(count > 20, "{}: {count} over twenty pieces", kind.name);
    }
    for (name, count) in &laid {
        let its_own = own.kinds.iter().any(|kind| kind.name == *name);
        let kept = own.keeps.iter().any(|kept| kept == name);
        assert!(
            its_own || kept || *count == 0,
            "{count} {name} in a realm that does not keep it"
        );
    }
    for name in ["Bracken", "Fern", "Herb", "Hazel", "Holly", "Young spruce"] {
        assert!(!laid.contains_key(name), "{name} is still on its floor");
    }
    let first = plants_laid(&mut app_of(true, None));
    for kind in &own.kinds {
        assert!(!first.contains_key(kind.name.as_str()), "{}", kind.name);
    }
    assert!(first.get("Bracken").copied().unwrap_or(0) > 0);
    // And it is as full as the first realm's floor under its oaks, within a tenth.
    let (far, wooded) = (laid.values().sum::<u32>(), first.values().sum::<u32>());
    println!("the far realm's floor: {laid:?}; the first realm's: {first:?}");
    assert!(
        far * 10 >= wooded * 9,
        "{far} plants for the first realm's {wooded}"
    );
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

/// A plant cut in a piece stays gone when the piece streams in again, and everything else in
/// it lands where it did.
#[test]
fn a_plant_cut_stays_gone_when_its_piece_comes_back_and_the_rest_stay() {
    let mut app = app();
    let none = HashSet::default();
    let (key, (put, places)) = ROW
        .map(|x| (x, 0))
        .find_map(|key| {
            let laid = lay(&mut app, key, &none, &[]);
            if laid.0.cuttable.is_empty() {
                stream_out(&mut app, &laid.0);
                return None;
            }
            Some((key, laid))
        })
        .expect("a piece of the row with a plant to cut");
    let (n, plant) = put.cuttable[0];
    let k = put.things.iter().position(|&thing| thing == plant).unwrap();
    stream_out(&mut app, &put);
    let (again, places_again) = lay(&mut app, key, &HashSet::from_iter([n]), &[]);
    let mut kept = places.clone();
    kept.remove(k);
    assert_eq!(places_again, kept, "all but the cut plant, where they were");
    let numbers = |put: &Put| put.cuttable.iter().map(|&(n, _)| n).collect::<Vec<_>>();
    let uncut: Vec<u32> = numbers(&put).into_iter().filter(|&m| m != n).collect();
    assert_eq!(
        numbers(&again),
        uncut,
        "the other plants keep their numbers"
    );
    assert_eq!(again.lying, put.lying);
}

/// A streamed piece's plant cut for its wood is filed as cut, as is one cleared away for what
/// was built where it stood; one streamed out, which stops being a floor plant first, is not.
#[test]
fn a_floor_plant_cut_or_cleared_is_filed_and_one_streamed_out_is_not() {
    let mut app = App::new();
    let frame = (Vec3::ZERO, Vec3::X, Vec3::Z);
    app.add_message::<ClearedAway>()
        .add_observer(file_the_cut)
        .add_systems(Update, file_the_cleared)
        .insert_resource(FloorStream::new(Kit::bare(), SEED, MIDDLE, frame));
    let piece = (20, 0);
    let mut plant = |n| {
        let growth = Growth::new(None, None, 0.5);
        app.world_mut()
            .spawn((FloorPlant { piece, n }, growth))
            .id()
    };
    let (felled, cleared, streamed) = (plant(3), plant(5), plant(8));
    let world = app.world_mut();
    world.entity_mut(felled).remove::<Growth>();
    world.write_message(ClearedAway(cleared));
    world
        .commands()
        .entity(streamed)
        .try_remove::<FloorPlant>()
        .try_despawn();
    world.flush();
    app.update();
    let cut = &app.world().resource::<FloorStream>().cut[&piece];
    assert_eq!(cut, &HashSet::from_iter([3, 5]));
}
