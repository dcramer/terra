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
    /// Plants filed into a square already alive (a streamed piece's, `merge`), whose shadows
    /// the camera's next look gives them.
    pending: Vec<((i32, i32), Entity, usize)>,
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
            pending: Vec::new(),
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

    /// Files the plants another laid (a streamed piece's, by this one's shadows), each given
    /// its shadow at once where its square is alive; and gives back where each was filed, for
    /// `remove`.
    pub(super) fn merge(&mut self, from: NearShadows) -> Vec<((i32, i32), Entity)> {
        let mut filed = Vec::new();
        for (key, plants) in from.filed {
            for (plant, set) in plants {
                if self.alive.contains_key(&key) {
                    self.pending.push((key, plant, set));
                }
                self.filed.entry(key).or_default().push((plant, set));
                filed.push((key, plant));
            }
        }
        filed
    }

    /// Takes plants filed by `merge` out again, as their piece streams out; their shadows,
    /// their children, go with them.
    pub(super) fn remove(&mut self, filed: &[((i32, i32), Entity)]) {
        for &(key, plant) in filed {
            if let Some(plants) = self.filed.get_mut(&key) {
                plants.retain(|&(filed, _)| filed != plant);
                if plants.is_empty() {
                    self.filed.remove(&key);
                }
            }
        }
        self.pending
            .retain(|(_, plant, _)| !filed.iter().any(|(_, gone)| gone == plant));
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
    // Plants filed since into squares alive now.
    for (key, plant, set) in std::mem::take(&mut near.pending) {
        if !near.alive.contains_key(&key) || standing.get(plant).is_err() {
            continue;
        }
        let shadows = shadows_of(&mut commands, &near.sets[set], plant);
        near.alive.entry(key).or_default().extend(shadows);
    }
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
                spawned.extend(shadows_of(&mut commands, &near.sets[set], plant));
            }
            near.alive.insert((i, j), spawned);
        }
    }
}

/// Spawns a plant's shadow copies, its children only the sun sees.
fn shadows_of(commands: &mut Commands, set: &ShadowSet, plant: Entity) -> Vec<Entity> {
    set.parts
        .iter()
        .map(|(mesh, material)| {
            commands
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
                .id()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    fn shadows(world: &mut World, plant: Entity) -> usize {
        world
            .get::<Children>(plant)
            .map_or(0, |children| children.len())
    }

    /// A plant filed into a square the camera already gives shadows is given its own at the
    /// camera's next look, without the camera moving; one taken out is not, and its square
    /// stays filed with the rest.
    #[test]
    fn plants_filed_near_the_camera_get_their_shadows_and_those_taken_out_do_not() {
        let mut world = World::new();
        world.insert_resource(DetailScale(1.0));
        world.spawn((MainCamera, GlobalTransform::IDENTITY));
        let mut near = NearShadows::new(Vec3::ZERO, Vec3::X, Vec3::Z);
        let set = near.add_set(ShadowSet {
            parts: vec![(Handle::default(), Handle::default())],
            end_m: 20.0,
        });
        let first = world.spawn_empty().id();
        near.add(first, Vec2::new(3.0, 3.0), set);
        world.insert_resource(near);
        world.run_system_once(follow_camera).unwrap();
        assert_eq!(
            shadows(&mut world, first),
            1,
            "the camera's square is alive"
        );

        let (late, gone) = (world.spawn_empty().id(), world.spawn_empty().id());
        let mut piece = NearShadows::new(Vec3::ZERO, Vec3::X, Vec3::Z);
        piece.add(late, Vec2::new(5.0, 4.0), set);
        piece.add(gone, Vec2::new(6.0, 4.0), set);
        let far = world.spawn_empty().id();
        piece.add(far, Vec2::new(900.0, 0.0), set);
        let filed = world.resource_mut::<NearShadows>().merge(piece);
        assert_eq!(filed.len(), 3);
        let gone_filed: Vec<_> = filed.iter().filter(|(_, p)| *p == gone).copied().collect();
        world.resource_mut::<NearShadows>().remove(&gone_filed);
        world.run_system_once(follow_camera).unwrap();
        assert_eq!(shadows(&mut world, late), 1, "filed into an alive square");
        assert_eq!(
            shadows(&mut world, gone),
            0,
            "taken out before it was given one"
        );
        assert_eq!(shadows(&mut world, far), 0, "far from the camera");
        assert_eq!(shadows(&mut world, first), 1, "given one once");
        let near = world.resource::<NearShadows>();
        let square = &near.filed[&square(Vec2::new(5.0, 4.0))];
        assert_eq!(square.len(), 2, "the first and the late one stay filed");
    }
}
