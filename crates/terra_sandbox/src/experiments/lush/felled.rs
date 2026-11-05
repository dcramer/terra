//! The lush forest's trees are felled as the flora's are (`terra_survival::harvest`), the
//! forest walk's and the round world's woods' alike (D619): each is cut as the flora's "Pine"
//! or "Leafy tree" kind says at its height, stops a walker at its trunk as drawn
//! (`footing::Trunk`), filed at its foot, which felling takes away, and leaves where it stood
//! a stump of the floor's own wood, sawn, its end grain showing, cut in turn for wood as the
//! "Stump" kind says. What the stump is, a `woods::Stump`, is what a saved game keeps of it;
//! a game loaded spawns it alone, and it is drawn again here.

use super::floor::{self, laid_wood};
use super::footing::{self, Trunk};
use super::species::Species;
use super::{FloorLooks, Foliage, TreeModel};
use bevy::prelude::*;
use terra_gen::Seed;
use terra_survival::harvest::Felled;
use terra_world::flora::Growth;
use terra_world::flora_kinds::{Cuttable, Flora};
use terra_world::planet::Planet;
use terra_world::woods::Stump;

/// A stump is never thinner or thicker than its tree's trunk by more than this.
const STUMP_SCALE: (f32, f32) = (0.4, 2.5);

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (stump_the_felled, draw_stumps)
            .chain()
            .run_if(terra_world::instance::in_world),
    );
}

/// The flora's kind a tree of a species is cut as.
pub(super) fn cut_kind(species: Species) -> &'static str {
    if species == Species::Oak {
        "Leafy tree"
    } else {
        "Pine"
    }
}

/// What makes a grown tree placed as `place` fellable: its trunk's footing, still to be
/// fitted (D248, D267), filed at its place, where it is felled from; cut as `cut` says, aimed
/// at 1.4 m up; and the stump it leaves.
pub(super) fn standing(
    model: &TreeModel,
    cut: Option<Cuttable>,
    place: &Transform,
) -> (Trunk, Growth, LeavesStump) {
    (
        Trunk {
            wood: model.wood[0].clone(),
            crown: model.crown_near.clone(),
            reach_m: footing::trunk_reach(model.trunk_m),
            skirt_m: model.skirt_m,
        },
        Growth::new(cut, Some(place.translation), 1.4),
        LeavesStump(Stump {
            at: place.translation,
            turn: place.rotation,
            trunk_m: model.trunk_m * place.scale.x,
            conifer: model.species != Species::Oak,
        }),
    )
}

/// A tree that leaves a stump when it is felled: where it stood and how it was turned, how
/// thick its trunk is at the foot, and whether it is a conifer, whose stump is of pine bark.
#[derive(Component, Clone, Copy, Debug)]
pub(super) struct LeavesStump(Stump);

/// What a felled tree's stump is drawn in: the floor's oak and pine bark and its end grain.
#[derive(Resource, Clone)]
pub(super) struct StumpLooks {
    pub bark: Handle<Foliage>,
    pub pine_bark: Handle<Foliage>,
    pub end_grain: Handle<StandardMaterial>,
}

impl StumpLooks {
    pub(super) fn of(floor: &FloorLooks, end_grain: Handle<StandardMaterial>) -> Self {
        Self {
            bark: floor.bark.clone(),
            pine_bark: floor.pine_bark.clone(),
            end_grain,
        }
    }
}

/// Leaves a stump, still to be drawn, where each tree felled since last time stood.
fn stump_the_felled(mut commands: Commands, felled: Query<&LeavesStump, Added<Felled>>) {
    for stood in &felled {
        commands.spawn(stood.0);
    }
}

/// Draws each stump still to be drawn, a tree's felled or a saved game's, on the ground there,
/// as thick as its trunk, turned as it was.
fn draw_stumps(
    mut commands: Commands,
    stumps: Query<(Entity, &Stump), Without<Mesh3d>>,
    looks: Option<Res<StumpLooks>>,
    flora: Res<Flora>,
    planet: Res<Planet>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let Some(looks) = looks else { return };
    for (entity, &stood) in &stumps {
        commands.entity(entity).despawn();
        let ground = planet.ground_at(stood.at);
        let at = ground.to_array().map(f32::to_bits);
        let mut dice = Seed(u64::from(at[0]) << 32 | u64::from(at[2]))
            .derive("felled stump")
            .rng();
        let stump = floor::stump(&mut dice, true);
        let scale = (stood.trunk_m / stump.radius).clamp(STUMP_SCALE.0, STUMP_SCALE.1);
        let bark = if stood.conifer {
            looks.pine_bark.clone()
        } else {
            looks.bark.clone()
        };
        let drawn = laid_wood(
            &mut commands,
            &mut meshes,
            "Stump",
            (stump.bark, stump.end),
            Transform {
                translation: ground,
                rotation: stood.turn,
                scale: Vec3::splat(scale),
            },
            (bark, looks.end_grain.clone()),
            flora.cut_rule("Stump").map(|rule| rule.at(1.0)),
        );
        commands.entity(drawn).insert(stood);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::schedule::Schedules;
    use bevy::gizmos::config::{DefaultGizmoConfigGroup, GizmoConfig, GizmoConfigStore};
    use std::time::Duration;
    use terra_core::PlanetSpec;
    use terra_survival::harvest::{Cutting, Swing};
    use terra_survival::inventory::{Backpack, Bar, Cut, Gained, Items};
    use terra_survival::registries::FloraGives;
    use terra_survival::skills::Did;
    use terra_walker::controls::Action;
    use terra_walker::intent::Intent;
    use terra_world::flora::Obstacles;

    /// A pine of the woods 15 m tall, as `conifers` draws one, felled with the axe a player
    /// starts with: they are given its wood, it stops nobody where it stood, and a stump of
    /// its wood stands there, itself cut for wood.
    #[test]
    fn a_woods_tree_felled_gives_wood_and_leaves_a_stump_where_it_stood() {
        let planet = Planet::smooth(PlanetSpec {
            seed: Seed(3),
            radius_m: 2000.0,
        });
        let feet = planet.start();
        let up = feet.normalize();
        let east = up.any_orthonormal_vector();
        let foot = planet.ground_at(feet + east * 2.0) - up * 0.3;
        let flora = Flora::read();
        // Gathering's systems and these, run on a world with a player's belongings.
        let mut app = App::new();
        terra_survival::harvest::plugin(&mut app);
        plugin(&mut app);
        let mut world = terra_survival::test_world();
        let world = &mut world;
        world.insert_resource(app.world_mut().remove_resource::<Schedules>().unwrap());
        world.insert_resource(FloraGives::resolve(&flora, world.resource::<Items>()).unwrap());
        world.insert_resource(flora);
        world.insert_resource(planet);
        world.init_resource::<Obstacles>();
        world.init_resource::<Messages<Did>>();
        world.init_resource::<Assets<Mesh>>();
        world.init_resource::<GizmoConfigStore>();
        world
            .resource_mut::<GizmoConfigStore>()
            .insert(GizmoConfig::default(), DefaultGizmoConfigGroup);
        let mut time = Time::<()>::default();
        time.advance_by(Duration::from_secs(10));
        world.insert_resource(time);
        world.insert_resource(StumpLooks {
            bark: Handle::default(),
            pine_bark: Handle::default(),
            end_grain: Handle::default(),
        });
        let player = terra_survival::test_player(world, feet);
        world
            .entity_mut(player)
            .insert((Cutting::default(), Swing::default()));
        world.resource_scope(|world, items: Mut<Items>| {
            let mut player = world.entity_mut(player);
            let backpack = player.get::<Backpack>().unwrap().0.clone();
            let mut bar = player.get_mut::<Bar>().unwrap();
            let axe = (0..bar.0.len())
                .find(|&slot| {
                    bar.0.get(slot).is_some_and(|id| {
                        let kind = backpack.get(id).unwrap().item.kind;
                        items.tool(kind).is_some_and(|tool| tool.works(Cut::Tree))
                    })
                })
                .expect("an axe on the bar at the start");
            bar.0.pick(axe);
        });

        // The tree, as `conifers` gives it: placed, drawn, cut as a pine of its height, and
        // stopping a walker at its foot (as `footing` fits it).
        let place =
            Transform::from_translation(foot).with_rotation(Quat::from_rotation_arc(Vec3::Y, up));
        let cut = world
            .resource::<Flora>()
            .cut_rule(cut_kind(Species::Pine))
            .map(|rule| rule.at(15.0));
        let tree = world
            .spawn((
                place,
                Visibility::default(),
                Growth::new(cut, Some(foot), 1.4),
                LeavesStump(Stump {
                    at: foot,
                    turn: place.rotation,
                    trunk_m: 0.3,
                    conifer: true,
                }),
            ))
            .id();
        world.resource_mut::<Obstacles>().add(foot, 0.3);

        // Held on it till it goes.
        let eye = feet + up * 1.6;
        let aim = Ray3d::new(eye, Dir3::new(foot + up * 1.4 - eye).unwrap());
        for _ in 0..4 {
            let mut intent = world.get_mut::<Intent>(player).unwrap();
            intent.aim = Some(aim);
            intent.hold(Action::Use);
            world.run_schedule(FixedUpdate);
            world.run_schedule(Update);
        }
        let wood = world.resource::<Items>().kind("Wood");
        let given: u32 = world
            .resource_mut::<Messages<Gained>>()
            .drain()
            .filter(|gained| gained.player == player && gained.kind == wood)
            .map(|gained| gained.count)
            .sum();
        assert_eq!(given, cut.unwrap().gives.1, "the pine's wood is given");
        assert!(given > 0);
        assert!(world.get::<Growth>(tree).is_none(), "the tree is felled");
        assert_eq!(
            world.resource::<Obstacles>().solids_near(foot, 1.0).count(),
            0,
            "nothing stops a walker where it stood"
        );
        let stumps: Vec<(Vec3, Option<Cuttable>)> = world
            .query::<(&Name, &Transform, &Growth)>()
            .iter(world)
            .filter(|(name, ..)| name.as_str() == "Stump")
            .map(|(_, at, growth)| (at.translation, growth.cut))
            .collect();
        let [(at, Some(stump_cut))] = stumps[..] else {
            panic!("one stump, cut for something: {stumps:?}");
        };
        assert!(
            at.distance(foot) < 0.5,
            "the stump stands where the tree did"
        );
        assert_eq!(world.resource::<FloraGives>().item(stump_cut.gives.0), wood);
        // What a saved game keeps of it is the tree's stump, and only the one.
        let kept: Vec<Stump> = world.query::<&Stump>().iter(world).copied().collect();
        assert_eq!(kept.len(), 1, "one stump kept: {kept:?}");
        assert!(kept[0].at.distance(foot) < 1e-3 && kept[0].conifer);
    }

    /// A stump as a saved game gives it back, alone, is drawn where its tree stood, on the
    /// ground, cut in turn for wood, and is the one stump kept, so a game saved again keeps it
    /// once.
    #[test]
    fn a_stump_a_saved_game_gives_back_is_drawn_where_its_tree_stood() {
        let planet = Planet::smooth(PlanetSpec {
            seed: Seed(3),
            radius_m: 2000.0,
        });
        let foot = planet.ground_at(planet.start()) - planet.start().normalize() * 0.2;
        let mut app = App::new();
        plugin(&mut app);
        let world = app.world_mut();
        world.insert_resource(Flora::read());
        world.insert_resource(planet);
        world.init_resource::<Assets<Mesh>>();
        world.insert_resource(StumpLooks {
            bark: Handle::default(),
            pine_bark: Handle::default(),
            end_grain: Handle::default(),
        });
        let saved = Stump {
            at: foot,
            turn: Quat::IDENTITY,
            trunk_m: 0.35,
            conifer: false,
        };
        world.spawn(saved);
        for _ in 0..2 {
            world.run_schedule(Update);
        }
        let drawn: Vec<(&Name, &Transform, &Growth, &Stump)> = world
            .query::<(&Name, &Transform, &Growth, &Stump)>()
            .iter(world)
            .collect();
        let [(name, at, growth, kept)] = drawn[..] else {
            panic!("one stump drawn, not {}", drawn.len());
        };
        assert_eq!(name.as_str(), "Stump");
        let ground = world.resource::<Planet>().ground_at(foot);
        assert!(
            at.translation.distance(ground) < 1e-3,
            "on the ground where the tree stood"
        );
        assert!(growth.cut.is_some(), "cut for its wood");
        assert_eq!(*kept, saved);
        assert_eq!(world.query::<&Stump>().iter(world).count(), 1, "kept once");
    }
}
