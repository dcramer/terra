//! The save's tests: a world saved and loaded, another version refused, and each world its
//! own record (D471).

use super::*;
use crate::build::DrivenByPlayer;
use crate::play::{Flora, LocalPlayer, registries};
use crate::start::Start;
use bevy::ecs::system::RunSystemOnce;
use terra_build::BuildOp;
use terra_core::{MaterialId, Operation, PlanetSpec, Seed};

/// What saving and loading need, set up as the game sets them up, with no one in it yet.
fn game() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Mesh>()
        .init_asset::<StandardMaterial>()
        .init_asset::<Image>();
    terra_world::instance::in_a_world(&mut app);
    let planet = Planet::new(PlanetSpec {
        seed: Seed(1),
        radius_m: 2000.0,
    });
    app.insert_resource(Options::from_args(std::iter::empty()).unwrap())
        .insert_resource(Start(Transform::from_translation(planet.start())))
        .insert_resource(planet)
        .insert_resource(DrivenByPlayer)
        .insert_resource(Flora::read())
        .init_resource::<Clock>()
        .init_resource::<record::Kept>()
        .init_resource::<Entered>()
        .init_resource::<crate::play::portal::Returns>()
        .init_resource::<Waiting>()
        .add_message::<FellAgain>();
    registries::plugin(&mut app);
    crate::build::building::plugin(&mut app);
    props::plugin(&mut app);
    terra_world::instance::start_by_hand(app.world_mut());
    app
}

/// This machine's player as they arrive: what they start with, and no claim yet.
fn arrive(world: &mut World) -> Entity {
    let at = world.resource::<Planet>().start();
    let kit = inventory::starting_kit(&mut world.resource_mut::<Items>());
    world
        .spawn((
            Player,
            LocalPlayer,
            Transform::from_translation(at),
            View {
                forward: Vec3::X,
                pitch: 0.0,
            },
            Vitals::default(),
            Skills::default(),
            kit,
            Claim::default(),
        ))
        .id()
}

/// A while of play: walked off and turned, hurt and tired, skilled, wearing a
/// container half full and something worn down, a tool used and a spare container
/// carried with a few in it, the bar rearranged, a base begun
/// and built, a station at work, the claim grown, the ground dug and filled, someone
/// at work and someone following, and the evening come.
fn play_a_while(world: &mut World, player: Entity) {
    carry_and_wear(world, player);
    build_and_gather(world, player);
}

/// The player's own: where they stand, what they know, carry and wear.
fn carry_and_wear(world: &mut World, player: Entity) {
    let start = world.resource::<Planet>().start();
    let up = start.normalize();
    let east = up.any_orthonormal_vector();
    let north = up.cross(east);

    world.resource_mut::<Clock>().hour = 21.5;
    world.resource_mut::<Clock>().day = 41;
    let mut planet = world.resource_mut::<Planet>();
    planet.apply(&Operation::Dig {
        centre: start + east * 4.0,
        radius: 0.7,
    });
    planet.apply(&Operation::Fill {
        centre: start + east * 6.0,
        radius: 0.5,
        material: MaterialId(2),
    });
    planet.apply(&Operation::DigBox {
        centre: start + north * 5.0,
        half: Vec3::new(1.0, 1.5, 2.0),
        rotation: Quat::from_rotation_arc(Vec3::Y, up),
    });

    // The body, where it is and what it knows.
    let mut entity = world.entity_mut(player);
    *entity.get_mut::<Transform>().unwrap() =
        Transform::from_translation(start + east * 3.0 + north * 2.0);
    entity.get_mut::<View>().unwrap().forward = (east + north).normalize();
    let mut vitals = entity.get_mut::<Vitals>().unwrap();
    vitals.health = 61.5;
    vitals.fed = 40.0;
    entity.get_mut::<Stamina>().unwrap().left = 12.25;
    let mut skills = entity.get_mut::<Skills>().unwrap();
    for _ in 0..9 {
        skills.did(Skill::Chopping, 1.0);
    }
    skills.did(Skill::Mining, 1.0);

    // A worn container, such as a quiver, put on and half filled, and something worn
    // that wears, worn down.
    world.resource_scope(|world, mut items: Mut<Items>| {
        let (container, (holds, most)) = items
            .kinds()
            .find_map(|kind| Some((kind, items.worn(kind)?.holds.clone()?)))
            .expect("something worn holds things");
        let wearing = items
            .kinds()
            .find(|&kind| items.worn(kind).is_some() && items.def(kind).lasts.is_some())
            .expect("something worn wears");
        let holds = items.kind(&holds);
        let mut entity = world.entity_mut(player);
        let (mut backpack, mut gear, mut bar) = (
            entity.get::<Backpack>().unwrap().0.clone(),
            entity.get::<Gear>().unwrap().0.clone(),
            entity.get::<Bar>().unwrap().0.clone(),
        );
        for kind in [container, wearing] {
            let made = items.make(kind);
            backpack.insert(&items.catalogue, made, None).unwrap();
            inventory::wear(&items, &mut backpack, &mut gear, made.id).unwrap();
        }
        let (slot, _) = gear
            .iter()
            .find(|(_, worn)| worn.is_some_and(|worn| worn.kind == wearing))
            .unwrap();
        gear.wear(&items.catalogue, slot, 3).unwrap();
        assert_eq!(gear.stow(holds, most / 2), most / 2);
        // A spare one, carried in the backpack with a few in it.
        let spare = items.make(container);
        backpack.insert(&items.catalogue, spare, None).unwrap();
        assert_eq!(backpack.stow(holds, 2), 2);
        // A tool worn down, the first slot of the bar emptied, and the last picked.
        let tool = bar.get(1).unwrap();
        backpack.wear(&items.catalogue, tool, 7).unwrap();
        bar.set(0, None);
        bar.pick(bar.len() - 1);
        entity.insert((Backpack(backpack), Gear(gear), Bar(bar)));
    });
}

/// The world's: a base with a room, a station at work, a chest, the claim and people.
fn build_and_gather(world: &mut World, player: Entity) {
    let start = world.resource::<Planet>().start();
    let up = start.normalize();
    let east = up.any_orthonormal_vector();
    let north = up.cross(east);

    // The base, begun away from where it was first laid, with a room in it.
    let frame = Transform::from_translation(start + north * 12.0)
        .with_rotation(Quat::from_rotation_arc(Vec3::Y, up));
    world
        .run_system_once(
            move |mut commands: Commands,
                  mut site: ResMut<Site>,
                  look: Res<crate::build::BuildLook>,
                  planet: Res<Planet>,
                  mut meshes: ResMut<Assets<Mesh>>| {
                site.move_to(frame, &mut commands, &planet, &mut meshes);
                let room = BuildOp::AddRoom {
                    min: [0, 0, 0],
                    max: [2, 0, 2],
                    kind: look.laying,
                };
                assert!(site.apply(room, &mut commands, &planet, &mut meshes));
            },
        )
        .unwrap();

    // A station outside it, with things queued, one under way and one made.
    world
        .run_system_once(
            |mut commands: Commands,
             builds: Res<Builds>,
             recipes: Res<Recipes>,
             models: Res<Models>,
             site: Res<Site>,
             planet: Res<Planet>| {
                let item = builds
                    .items()
                    .find(|item| item.station.is_some())
                    .expect("something placed is a station");
                let makes: Vec<usize> = recipes.at(item.station.unwrap()).collect();
                assert!(!makes.is_empty(), "{} makes nothing", item.name);
                let at = Vec3::new(-3.0, 0.0, 1.5);
                let station = props::place(
                    &mut commands,
                    &models,
                    &site,
                    &planet,
                    item,
                    at,
                    0.25,
                    30.0,
                    false,
                )
                .expect("the station's model is there");
                let queue = vec![makes[makes.len() - 1], makes[0]];
                commands
                    .entity(station)
                    .insert(Works::resumed(queue, 1.5, vec![makes[0]]));
            },
        )
        .unwrap();

    // A chest beside it, with a few things stored in it.
    world
        .run_system_once(
            |mut commands: Commands,
             builds: Res<Builds>,
             mut items: ResMut<Items>,
             models: Res<Models>,
             site: Res<Site>,
             planet: Res<Planet>| {
                let item = builds
                    .items()
                    .find(|item| item.holds.is_some())
                    .expect("something placed is storage");
                let (w, h) = item.holds.unwrap();
                let chest = props::place(
                    &mut commands,
                    &models,
                    &site,
                    &planet,
                    item,
                    Vec3::new(3.0, 0.0, -2.0),
                    0.0,
                    90.0,
                    false,
                )
                .expect("the chest's model is there");
                let mut grid = terra_items::Grid::new(w, h);
                for (name, cell) in [("Wood", (2, 1)), ("Wood", (0, 0)), ("Stone", (5, 3))] {
                    let made = items.make_named(name).unwrap();
                    let cell = Cell {
                        x: cell.0,
                        y: cell.1,
                    };
                    grid.insert(&items.catalogue, made, Some(cell)).unwrap();
                }
                commands.entity(chest).insert(Storage(grid));
            },
        )
        .unwrap();

    // The claim, its stone set and grown to its largest.
    world.resource_scope(|world, builds: Mut<Builds>| {
        let mut claim = world.get_mut::<Claim>(player).unwrap();
        claim.stone = Some((Entity::PLACEHOLDER, frame.translation));
        claim.set_step(builds.claim.across_m.len() - 1, &builds.claim);
        assert!(claim.step() > 0, "a claim that cannot grow tells nothing");
    });

    // People: one at work, one following, and one still bound, who is not kept.
    world.spawn((
        Person::new("Bren", Doing::Working(start + east * 9.0), None),
        Transform::from_translation(start + east * 8.0),
    ));
    world.spawn((
        Person::new("Tam", Doing::Following(player), None),
        Transform::from_translation(start + north * 4.0),
    ));
    world.spawn((
        Person::new("Ash", Doing::Bound, None),
        Transform::from_translation(start + north * 30.0),
    ));
}

/// Everyone the save keeps, as they might be before it is loaded: elsewhere, and
/// still bound.
fn waiting_to_be_found(world: &mut World) {
    let start = world.resource::<Planet>().start();
    for name in ["Bren", "Tam"] {
        world.spawn((
            Person::new(name, Doing::Bound, None),
            Transform::from_translation(start + Vec3::X * 50.0),
        ));
    }
}

/// Everything a save should keep, read from a world's own components and
/// resources, never through the save.
#[derive(Debug, PartialEq)]
struct Kept {
    at: Vec3,
    facing: Vec3,
    body: [f32; 3],
    skills: Skills,
    /// Each slot's kind, its wear, and how many it holds.
    worn: Vec<Option<(String, u16, Option<u32>)>>,
    /// Each item carried: its kind, where, its wear, and how many it holds.
    carried: Vec<(String, (u8, u8), u16, Option<u32>)>,
    /// What each slot of the bar points at, by where it is carried.
    bar: Vec<Option<(u8, u8)>>,
    picked: usize,
    base: Snapshot,
    frame: Transform,
    /// Each station or thing placed, with its queue, progress and what it has made.
    props: Vec<(
        String,
        Vec3,
        f32,
        f32,
        bool,
        Option<(Vec<usize>, f32, Vec<usize>)>,
    )>,
    /// What each piece of storage holds: each item's kind and where.
    stored: Vec<Vec<(String, (u8, u8))>>,
    claim: usize,
    hour: f32,
    day: i64,
    ground: Vec<Operation>,
    people: Vec<(&'static str, String, Vec3)>,
}

fn kept(world: &mut World) -> Kept {
    let mut players = world.query_filtered::<Entity, IsLocalPlayer>();
    let player = players.single(world).unwrap();
    let entity = world.entity(player);
    let items = world.resource::<Items>();
    let backpack = &entity.get::<Backpack>().unwrap().0;
    let gear = &entity.get::<Gear>().unwrap().0;
    let bar = &entity.get::<Bar>().unwrap().0;
    let (vitals, stamina) = (entity.get::<Vitals>().unwrap(), entity.get::<Stamina>());
    let site = world.resource::<Site>();
    let mut kept = Kept {
        at: entity.get::<Transform>().unwrap().translation,
        facing: entity.get::<View>().unwrap().forward,
        body: [vitals.health, stamina.unwrap().left, vitals.fed],
        skills: entity.get::<Skills>().unwrap().clone(),
        worn: gear
            .iter()
            .map(|(slot, worn)| {
                let held = gear.stack(slot).map(|stack| stack.held);
                worn.map(|worn| (items.name(worn.kind).to_string(), worn.wear, held))
            })
            .collect(),
        carried: backpack
            .iter()
            .map(|placed| {
                (
                    items.name(placed.item.kind).to_string(),
                    (placed.at.x, placed.at.y),
                    placed.item.wear,
                    placed.stack.map(|stack| stack.held),
                )
            })
            .collect(),
        bar: (0..bar.len())
            .map(|slot| {
                let placed = backpack.get(bar.get(slot)?)?;
                Some((placed.at.x, placed.at.y))
            })
            .collect(),
        picked: bar.picked(),
        base: site.base.snapshot(),
        frame: site.frame,
        props: Vec::new(),
        stored: Vec::new(),
        claim: entity.get::<Claim>().unwrap().step(),
        hour: world.resource::<Clock>().hour,
        day: world.resource::<Clock>().day,
        ground: world.resource::<Planet>().operations(),
        people: Vec::new(),
    };
    let mut props = world.query::<(&Prop, Option<&Works>)>();
    kept.props = props
        .iter(world)
        .map(|(prop, works)| {
            (
                prop.item.name.clone(),
                prop.at,
                prop.lift,
                prop.turn,
                prop.indoors,
                works.map(|works| {
                    let queue = works.queue.iter().copied().collect();
                    (queue, works.progress, works.done.clone())
                }),
            )
        })
        .collect();
    kept.props.sort_by(|a, b| a.0.cmp(&b.0));
    let mut stores = world.query::<&Storage>();
    let items = world.resource::<Items>();
    kept.stored = stores
        .iter(world)
        .map(|storage| {
            let mut held: Vec<(String, (u8, u8))> = storage
                .0
                .iter()
                .map(|placed| {
                    let kind = items.name(placed.item.kind).to_string();
                    (kind, (placed.at.x, placed.at.y))
                })
                .collect();
            held.sort();
            held
        })
        .collect();
    kept.stored.sort();
    let mut people = world.query::<(&Person, &Transform)>();
    kept.people = people
        .iter(world)
        .map(|(person, at)| {
            let doing = match person.doing() {
                Doing::Following(whom) if whom == player => "following the player".into(),
                Doing::Working(spot) => format!("working at {spot}"),
                ref other => format!("{other:?}"),
            };
            (person.name, doing, at.translation)
        })
        .collect();
    kept.people.sort_by(|a, b| a.0.cmp(b.0));
    kept
}

/// Saves a world the way the game does, to a file of its own.
fn save(world: &mut World, name: &str) -> PathBuf {
    world.run_system_once(keep).unwrap();
    let kept = world.resource::<record::Kept>();
    let save = kept.file.as_ref().expect("the game was kept");
    let path = std::env::temp_dir().join(format!(
        "terra-{name}-{}-{:?}.ron",
        std::process::id(),
        std::thread::current().id()
    ));
    assert!(write(&path, save));
    path
}

/// Loads a save the way the game does as it starts, and lets the claim and people
/// take up where they were once their stone and they are there.
fn load_from(world: &mut World, path: &std::path::Path) {
    world.insert_resource(SavePath(path.to_path_buf()));
    world.run_system_once(load).unwrap();
    world.run_system_once(restore_claim).unwrap();
    world.run_system_once(restore_people).unwrap();
}

#[test]
fn a_world_saved_and_loaded_into_another_is_the_same_world() {
    let mut played = game();
    let world = played.world_mut();
    let player = arrive(world);
    play_a_while(world, player);
    let before = kept(world);
    // What the world was given to keep is there to keep.
    assert!(before.worn.iter().flatten().any(|(_, wear, _)| *wear > 0));
    assert!(
        before
            .worn
            .iter()
            .flatten()
            .any(|(_, _, held)| *held > Some(0))
    );
    assert!(before.carried.iter().any(|(_, _, wear, _)| *wear > 0));
    assert!(before.carried.iter().any(|(_, _, _, held)| *held > Some(0)));
    assert_eq!(before.bar[0], None);
    assert!(before.props.iter().any(|(.., works)| works.is_some()));
    assert!(before.stored.iter().any(|held| held.len() == 3));
    assert_eq!(before.ground.len(), 3);
    assert_eq!(before.people.len(), 3);
    let path = save(world, "round-trip");

    let mut fresh = game();
    let world = fresh.world_mut();
    let player = arrive(world);
    waiting_to_be_found(world);
    load_from(world, &path);
    // The claim waits for its stone before it grows back.
    assert_eq!(world.get::<Claim>(player).unwrap().step(), 0);
    let stone = before.frame.translation;
    world.get_mut::<Claim>(player).unwrap().stone = Some((Entity::PLACEHOLDER, stone));
    world.run_system_once(restore_claim).unwrap();
    let _ = std::fs::remove_file(&path);

    // Whoever was still bound is not kept: they are found again where they are.
    let mut before = before;
    before.people.retain(|(name, ..)| *name != "Ash");
    assert_eq!(kept(world), before);
}

/// Each world has its own record (D471): kept in one world and then in another, the save
/// holds both, the first as it was left; loaded, the player is where they were left, in
/// the second world, and the game crosses there first; and each world entered is given
/// its own record, not the other's. With one record for the save (as before the split)
/// the first world would be written over with the second's.
#[test]
fn each_world_keeps_its_own_record_and_a_save_is_loaded_into_the_world_it_was_left_in() {
    use terra_world::instance::InstanceKey;
    let spec = |key: u64| WorldSpec {
        key: InstanceKey(key),
        kind: "world".into(),
        seed: key + 1,
    };
    let mut played = game();
    let world = played.world_mut();
    let player = arrive(world);
    play_a_while(world, player);
    world.insert_resource(spec(1));
    world.resource_mut::<Clock>().hour = 21.5;
    world.run_system_once(keep).unwrap();
    // On through a gate: another world, another hour, the way back kept.
    world.insert_resource(spec(2));
    world.resource_mut::<Clock>().hour = 3.0;
    world
        .resource_mut::<crate::play::portal::Returns>()
        .0
        .push(crate::play::portal::Return {
            world: spec(1),
            at: Vec3::new(1.0, 2.0, 3.0),
            toward: Vec3::X,
        });
    let path = save(world, "two-worlds");
    let text = std::fs::read_to_string(&path).unwrap();
    let file: SaveFile = ron::from_str(&text).unwrap();
    assert_eq!(file.here.key, 2);
    assert_eq!(file.instances.keys().copied().collect::<Vec<_>>(), [1, 2]);
    assert_eq!(
        (file.instances[&1].hour, file.instances[&2].hour),
        (Some(21.5), Some(3.0))
    );

    // Loaded where the save was begun, world 1: not put there, but crossed to world 2.
    let mut fresh = game();
    let world = fresh.world_mut();
    arrive(world);
    waiting_to_be_found(world);
    // One system kept across the calls, as the game's is: a save chosen once, then a world
    // entered each time (`Entered` made afresh).
    let enter = |world: &mut World, key: u64| {
        world.insert_resource(spec(key));
        // Gone with the world left, made afresh for the one entered.
        world.remove_resource::<Entered>();
        world.insert_resource(Entered);
        world.run_system_cached(load).unwrap();
    };
    world.resource_mut::<Clock>().hour = 12.0;
    let before = kept(world);
    world.insert_resource(SavePath(path.clone()));
    enter(world, 1);
    assert_eq!(kept(world), before, "restored into the wrong world");
    assert_eq!(world.resource::<Clock>().hour, 12.0);
    // In world 2: its record, the player, and the way back.
    enter(world, 2);
    assert_eq!(world.resource::<Clock>().hour, 3.0);
    let returns = &world.resource::<crate::play::portal::Returns>().0;
    assert_eq!(returns.len(), 1);
    assert_eq!(returns[0].world.key, InstanceKey(1));
    // Back in world 1: its own record, as it was left.
    enter(world, 1);
    assert_eq!(world.resource::<Clock>().hour, 21.5);
    let _ = std::fs::remove_file(&path);
}

/// A new game, with no save yet, that crosses on through a gate stays where it crossed
/// to: the world it left is kept, and the player is not sent back to be put in it. (The
/// gate's check found this: the kept save still held the player as not yet placed.)
#[test]
fn a_new_game_that_crosses_on_is_not_sent_back_to_the_world_it_left() {
    use terra_world::instance::InstanceKey;
    let spec = |key: u64| WorldSpec {
        key: InstanceKey(key),
        kind: "world".into(),
        seed: key + 1,
    };
    let mut played = game();
    let world = played.world_mut();
    arrive(world);
    let enter = |world: &mut World, key: u64| {
        world.insert_resource(spec(key));
        world.remove_resource::<Entered>();
        world.insert_resource(Entered);
        world.run_system_cached(load).unwrap();
    };
    let nowhere = std::env::temp_dir().join(format!("terra-no-save-{}.ron", std::process::id()));
    world.insert_resource(SavePath(nowhere));
    enter(world, 1);
    // Left through a gate: kept as it is left.
    world.run_system_once(keep).unwrap();
    enter(world, 2);
    let mut names = world.query::<&Name>();
    assert!(
        names.iter(world).all(|name| name.as_str() != "Crossing"),
        "sent back to the world it left"
    );
}

#[test]
fn a_save_from_another_version_is_left_alone() {
    let mut played = game();
    let world = played.world_mut();
    let player = arrive(world);
    play_a_while(world, player);
    let path = save(world, "other-version");
    let text = std::fs::read_to_string(&path).unwrap();
    let mut save: SaveFile = ron::from_str(&text).unwrap();
    save.version = VERSION + 1;
    assert!(write(&path, &save));

    let mut fresh = game();
    let world = fresh.world_mut();
    arrive(world);
    let before = kept(world);
    load_from(world, &path);
    let _ = std::fs::remove_file(&path);
    assert_eq!(kept(world), before);
}
