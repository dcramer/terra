//! A torch in hand burns: a small flame at its head and a warm, wavering light around
//! whoever holds it, which is what lets them see at night (D134). The torch's look is shown in
//! hand as any held thing's is (`held`); this only sets it alight.

use crate::inventory::{self, Backpack, Bar, Items};
use bevy::{light::NotShadowCaster, platform::collections::HashSet, prelude::*};
use terra_walker::Player;
use terra_walker::character::Held;
use terra_walker::figure::Grip;

/// How bright a torch is, in lumens, and how far its light reaches.
const TORCH_LUMENS: f32 = 160_000.0;
const TORCH_RANGE_M: f32 = 14.0;
/// Where its head is along its look, from where it is held, in metres.
const HEAD_M: f32 = 0.47;

/// A torch's flame, with its light.
#[derive(Component)]
struct Flame;

/// The flame's look, made once.
#[derive(Resource)]
struct FlameLook {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
}

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, setup)
        .add_systems(Update, (light, flicker).chain());
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(FlameLook {
        mesh: meshes.add(Cone::new(0.04, 0.14)),
        material: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.6, 0.2),
            emissive: LinearRgba::rgb(14.0, 5.0, 1.2),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        }),
    });
}

/// Sets a torch in a player's hand alight, and puts it out when it is put away. The flame
/// goes on the look in hand, so it moves with it.
fn light(
    mut commands: Commands,
    look: Res<FlameLook>,
    items: Res<Items>,
    players: Query<(&Backpack, &Bar), With<Player>>,
    grips: Query<(Entity, &Grip)>,
    children: Query<&Children>,
    held: Query<(), With<Held>>,
    flames: Query<(Entity, &ChildOf), With<Flame>>,
) {
    // What is alight now: the look in each hand holding a torch.
    let mut alight = HashSet::new();
    for (hand, grip) in &grips {
        let burning = players.get(grip.player).is_ok_and(|(backpack, bar)| {
            inventory::holding(&items, &backpack.0, &bar.0)
                .and_then(|kind| items.tool(kind))
                .is_some_and(|tool| tool.lights)
        });
        let in_hand = children
            .get(hand)
            .ok()
            .and_then(|looks| looks.iter().find(|&each| held.contains(each)));
        if let (true, Some(torch)) = (burning, in_hand) {
            alight.insert(torch);
        }
    }
    // Anything alight that is not a torch in hand goes out.
    let mut lit = HashSet::new();
    for (flame, parent) in &flames {
        if alight.contains(&parent.parent()) {
            lit.insert(parent.parent());
        } else {
            commands.entity(flame).despawn();
        }
    }
    for torch in alight.into_iter().filter(|torch| !lit.contains(torch)) {
        light_torch(&mut commands, &look, torch);
    }
}

/// Puts a flame, with its light, on a torch's head.
fn light_torch(commands: &mut Commands, look: &FlameLook, torch: Entity) {
    commands.entity(torch).with_child((
        Name::new("Flame"),
        Flame,
        Mesh3d(look.mesh.clone()),
        MeshMaterial3d(look.material.clone()),
        NotShadowCaster,
        PointLight {
            color: Color::srgb(1.0, 0.62, 0.3),
            intensity: TORCH_LUMENS,
            range: TORCH_RANGE_M,
            radius: 0.05,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(0.0, HEAD_M + 0.07, 0.0),
    ));
}

/// A flame wavers: its light, and the flame itself.
fn flicker(time: Res<Time>, mut flames: Query<(&mut PointLight, &mut Transform), With<Flame>>) {
    let t = time.elapsed_secs();
    let waver = 0.85 + 0.09 * libm::sinf(t * 13.0) + 0.06 * libm::sinf(t * 29.0 + 0.7);
    for (mut light, mut transform) in &mut flames {
        light.intensity = TORCH_LUMENS * waver;
        transform.scale = Vec3::new(1.0, 0.85 + 0.3 * (waver - 0.7), 1.0);
    }
}
