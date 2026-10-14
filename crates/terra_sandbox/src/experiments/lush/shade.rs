//! The middle layer's shadows only near the camera (D268). Each shrub's, sapling's and snag's
//! shadow is cast from a copy only the sun sees, which shrinks away within a few tens of metres
//! (D234). Bevy checks every shadow caster against every cascade of the sun each frame, seen or
//! not, so ten thousand of them cost more than a millisecond doing nothing. Instead the plants
//! are filed by square, and only those in squares near the camera have their shadow copies,
//! spawned as the camera comes near and despawned as it leaves, past where they have shrunk to
//! nothing: as open worlds stream what is near the player, changing only what changed.

use super::{FADE, Foliage, SUN_ONLY};
use bevy::{
    camera::visibility::{RenderLayers, VisibilityRange},
    platform::collections::HashMap,
    prelude::*,
};
use terra_world::detail::DetailScale;
use terra_world::planet::MainCamera;

/// Plants are filed in squares this wide.
const SQUARE_M: f32 = 16.0;

/// A kind's shadow: the copies it is cast from, and where it has shrunk away by.
pub(super) struct ShadowSet {
    pub parts: Vec<(Handle<Mesh>, Handle<Foliage>)>,
    pub end_m: f32,
}

/// The plants whose shadows follow the camera, filed by square of the forest's map, and the
/// shadow copies alive now in each square near it.
#[derive(Resource)]
pub(super) struct NearShadows {
    sets: Vec<ShadowSet>,
    filed: HashMap<(i32, i32), Vec<(Entity, usize)>>,
    alive: HashMap<(i32, i32), Vec<Entity>>,
    start: Vec3,
    east: Vec3,
    south: Vec3,
    /// The square the camera was in and the reach in squares, when they were last updated.
    last: Option<((i32, i32), i32)>,
}

impl NearShadows {
    /// None yet, for the forest's map from `start`, east and south of it.
    pub(super) fn new(start: Vec3, east: Vec3, south: Vec3) -> Self {
        Self {
            sets: Vec::new(),
            filed: HashMap::default(),
            alive: HashMap::default(),
            start,
            east,
            south,
            last: None,
        }
    }

    /// A kind's shadow, by which its plants are filed.
    pub(super) fn add_set(&mut self, set: ShadowSet) -> usize {
        self.sets.push(set);
        self.sets.len() - 1
    }

    /// Files a plant standing at `at` on the map, with its kind's shadow.
    pub(super) fn add(&mut self, plant: Entity, at: Vec2, set: usize) {
        self.filed.entry(square(at)).or_default().push((plant, set));
    }

    fn reach(&self, scale: f32) -> i32 {
        let end = self.sets.iter().map(|set| set.end_m).fold(0.0, f32::max) * scale;
        (end / SQUARE_M).ceil() as i32 + 1
    }
}

fn square(at: Vec2) -> (i32, i32) {
    (
        (at.x / SQUARE_M).floor() as i32,
        (at.y / SQUARE_M).floor() as i32,
    )
}

/// Gives the plants in squares near the camera their shadows, and takes them from those it
/// has left, when the camera changes square or detail reaches farther or less far.
pub(super) fn follow_camera(
    mut commands: Commands,
    near: Option<ResMut<NearShadows>>,
    camera: Option<Single<&GlobalTransform, With<MainCamera>>>,
    detail: Res<DetailScale>,
    standing: Query<()>,
) {
    let (Some(mut near), Some(camera)) = (near, camera) else {
        return;
    };
    let off = camera.translation() - near.start;
    let here = square(Vec2::new(off.dot(near.east), off.dot(near.south)));
    let reach = near.reach(detail.0);
    if near.last == Some((here, reach)) {
        return;
    }
    near.last = Some((here, reach));
    let within =
        |key: &(i32, i32)| (key.0 - here.0).abs() <= reach && (key.1 - here.1).abs() <= reach;
    let gone: Vec<(i32, i32)> = near
        .alive
        .keys()
        .filter(|key| !within(key))
        .copied()
        .collect();
    for key in gone {
        for shadow in near.alive.remove(&key).into_iter().flatten() {
            commands.entity(shadow).try_despawn();
        }
    }
    for i in here.0 - reach..=here.0 + reach {
        for j in here.1 - reach..=here.1 + reach {
            if near.alive.contains_key(&(i, j)) {
                continue;
            }
            let Some(plants) = near.filed.get(&(i, j)) else {
                continue;
            };
            let mut spawned = Vec::new();
            for &(plant, set) in plants {
                // A plant cut down is gone, its shadow with it.
                if standing.get(plant).is_err() {
                    continue;
                }
                let set = &near.sets[set];
                for (mesh, material) in &set.parts {
                    let shadow = commands
                        .spawn((
                            Mesh3d(mesh.clone()),
                            MeshMaterial3d(material.clone()),
                            VisibilityRange {
                                start_margin: 0.0..0.0,
                                end_margin: set.end_m * (1.0 - FADE)..set.end_m,
                                use_aabb: false,
                            },
                            RenderLayers::layer(SUN_ONLY),
                            ChildOf(plant),
                        ))
                        .id();
                    spawned.push(shadow);
                }
            }
            near.alive.insert((i, j), spawned);
        }
    }
}
