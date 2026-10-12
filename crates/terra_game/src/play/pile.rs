//! What does not fit drops to the ground (asked for by the developers on 2026-10-12):
//! anything given to the player that their backpack cannot hold, whether cut, dug,
//! butchered or given back by taking something apart, lands in a pile at their feet
//! instead of being lost. More of it while they stand there goes on the same pile. F at a
//! pile picks up what fits, as at a dropped bag (`death`), and a pile left alone is gone
//! after a while, as a bag is.

use super::Player;
use super::death::DroppedBag;
use super::interact::{CanInteract, Interactable};
use super::inventory::{Items, Spilled};
use super::tick::Sim;
use super::told::Told;
use bevy::prelude::*;
use terra_items::Grid;

/// How near a pile must be to the player's feet for more to go on it.
const SAME_PILE_M: f32 = 2.5;
/// How many cells a pile holds.
const PILE_CELLS: u8 = 10;

/// What a pile looks like, made once.
#[derive(Resource)]
struct PileLook {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
}

pub fn plugin(app: &mut App) {
    app.add_message::<Spilled>()
        .add_systems(Startup, setup)
        .add_systems(FixedUpdate, drop.in_set(Sim::World));
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(PileLook {
        mesh: meshes.add(Cuboid::new(0.5, 0.25, 0.5)),
        material: materials.add(StandardMaterial {
            base_color: Color::srgb(0.48, 0.4, 0.3),
            perceptual_roughness: 0.95,
            ..default()
        }),
    });
}

/// Puts what did not fit on the nearest pile at that player's feet, or a new one.
fn drop(
    mut commands: Commands,
    look: Res<PileLook>,
    mut items: ResMut<Items>,
    players: Query<&Transform, With<Player>>,
    mut piles: Query<(Entity, &mut DroppedBag, &Transform), Without<Player>>,
    mut spilled: MessageReader<Spilled>,
    mut told: MessageWriter<Told>,
) {
    for &Spilled {
        player,
        kind,
        count,
    } in spilled.read()
    {
        let Ok(feet) = players.get(player).map(|at| at.translation) else {
            continue;
        };
        let near = piles
            .iter()
            .filter(|(_, _, at)| at.translation.distance(feet) < SAME_PILE_M)
            .min_by(|a, b| {
                a.2.translation
                    .distance(feet)
                    .total_cmp(&b.2.translation.distance(feet))
            })
            .map(|(entity, ..)| entity);
        let mut grid = match near.and_then(|entity| piles.get_mut(entity).ok()) {
            Some((_, mut pile, _)) => std::mem::replace(&mut pile.grid, Grid::new(0, 0)),
            None => Grid::new(PILE_CELLS, PILE_CELLS),
        };
        let mut put = 0;
        for _ in 0..count {
            let item = items.make(kind);
            if grid.insert(&items.catalogue, item, None).is_err() {
                break;
            }
            put += 1;
        }
        match near.and_then(|entity| piles.get_mut(entity).ok()) {
            Some((_, mut pile, _)) => pile.grid = grid,
            None => {
                let up = feet.normalize();
                commands.spawn((
                    Name::new("Pile"),
                    DroppedBag::new(grid),
                    CanInteract(Interactable::Bag),
                    Mesh3d(look.mesh.clone()),
                    MeshMaterial3d(look.material.clone()),
                    Transform::from_translation(feet + up * 0.12)
                        .with_rotation(Quat::from_rotation_arc(Vec3::Y, up)),
                ));
            }
        }
        let name = items.name(kind).to_string();
        info!("Dropped {put} {name}: the backpack is full.");
        told.write(Told::new(
            player,
            format!("Your backpack is full: {put} {name} dropped at your feet"),
        ));
    }
}
