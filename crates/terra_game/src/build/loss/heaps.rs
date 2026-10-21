//! The heaps falls leave, drawn (`fragments::Rubble::surfaces`, D305).

use super::*;

/// How often a heap that changed is drawn again, at most, in seconds, and by how much of its
/// volume it must have changed.
const HEAP_EVERY_S: f32 = 0.2;
const HEAP_CHANGE: f32 = 0.02;

/// Each build's heap drawn again where it changed (`Rubble::surfaces`): the volume of what
/// fell lies on the ground as a mound of its stone, whatever of the pieces on it the budget
/// lets go.
pub(super) fn draw_heaps(
    commands: &mut Commands,
    shards: &mut Shards,
    by_root: &BTreeMap<Entity, &Build>,
    look: &BuildLook,
    arrays: Option<&Arrays>,
    meshes: &mut Assets<Mesh>,
) {
    let now = shards.clock_s;
    let Some(kits) = look.kits.as_deref() else {
        return;
    };
    let roots: Vec<Entity> = shards.rubble.keys().copied().collect();
    for root in roots {
        let volume = shards.rubble[&root].volume(now);
        let (drawn, when, was) = shards.heaps.remove(&root).unwrap_or_default();
        let changed = (volume - was).abs() > HEAP_CHANGE * was.max(0.05);
        if !changed || now - when < HEAP_EVERY_S && !drawn.is_empty() {
            shards.heaps.insert(root, (drawn, when, was));
            continue;
        }
        for entity in drawn {
            if let Ok(mut entity) = commands.get_entity(entity) {
                entity.despawn();
            }
        }
        let Some(build) = by_root.get(&root) else {
            continue;
        };
        let mut made = Vec::new();
        let kit_of = |kind: Kind| kits.of(kind);
        for (kind, surfaces) in shards.rubble[&root].surfaces(now, &kit_of) {
            let surfaces: BTreeMap<_, _> = surfaces
                .0
                .into_iter()
                .map(|(material, surface)| ((kind, material), surface))
                .collect();
            let mut used = BTreeSet::new();
            let (in_arrays, rest) = draw::in_arrays(
                &surfaces,
                &look.layers,
                Vec3::ZERO,
                &BTreeMap::new(),
                &mut used,
            );
            shards.layers.extend(used);
            let mut put = |mesh: Mesh, drawn: Drawn| {
                let mut entity = commands.spawn((
                    Name::new("Rubble heap"),
                    Mesh3d(meshes.add(mesh)),
                    build.frame,
                ));
                match drawn {
                    Drawn::Arrays(material) => entity.insert(MeshMaterial3d(material)),
                    Drawn::Own(material) => entity.insert(MeshMaterial3d(material)),
                };
                made.push(entity.id());
            };
            if let (Some(mesh), Some(arrays)) = (in_arrays, arrays) {
                put(mesh, Drawn::Arrays(arrays.material.clone()));
                continue;
            }
            for (kind, material, mesh) in rest {
                if let Some(own) = look
                    .materials
                    .get(kind, material)
                    .or_else(|| look.materials.get(Kind::PLAIN, material))
                {
                    put(mesh, Drawn::Own(own.clone()));
                }
            }
        }
        shards.heaps.insert(root, (made, now, volume));
    }
}
