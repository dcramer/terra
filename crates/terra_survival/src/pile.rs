//! What does not fit drops to the ground (asked for by the developers on 2026-10-12):
//! anything given to the player that their backpack cannot hold, whether cut, dug,
//! butchered or given back by taking something apart, lands in a pile at their feet
//! instead of being lost. More of it while they stand there goes on the same pile. F at a
//! pile picks up what fits, as at a dropped bag (`death`), and a pile left alone is gone
//! after a while, as a bag is.

use crate::death::{BagLook, DroppedBag};
use crate::interact::{CanInteract, Interactable};
use crate::inventory::{Items, Spilled};
use crate::told::Told;
use bevy::prelude::*;
use terra_items::Grid;
use terra_walker::Player;
use terra_world::tick::Sim;

/// How near a pile must be to the player's feet for more to go on it.
const SAME_PILE_M: f32 = 2.5;
/// How many cells a pile holds.
const PILE_CELLS: u8 = 10;

pub fn plugin(app: &mut App) {
    app.add_message::<Spilled>()
        .add_systems(FixedUpdate, drop.in_set(Sim::World));
}

/// Puts what did not fit on the nearest pile at that player's feet, or a new one.
fn drop(
    mut commands: Commands,
    look: Res<BagLook>,
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
                    Transform::from_translation(feet)
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
