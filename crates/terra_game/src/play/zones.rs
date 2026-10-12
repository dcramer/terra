//! Where on a creature a blow lands (D119): its head, body or legs, each a few spheres
//! round the joints of its model that its kind names (`zones` in creatures.ron). They
//! move with it as it is animated, so a deer's head is where it is drawn, grazing or
//! running. An arrow strikes the first it passes through; a swing, the one under the aim.

use super::creatures::{Creature, Species};
use super::species::{Bestiary, Part};
use bevy::{ecs::system::SystemParam, prelude::*};

/// A creature's zones: each joint they are round, which part it is, and how far round.
#[derive(Component, Debug)]
pub struct HitZones(Vec<(Entity, Part, f32)>);

pub fn plugin(app: &mut App) {
    app.add_systems(Update, find);
}

/// Finds the joints of each creature's zones once its model is in the world.
fn find(
    mut commands: Commands,
    creatures: Query<(Entity, &Creature), Without<HitZones>>,
    children: Query<&Children>,
    names: Query<&Name>,
    bestiary: Res<Bestiary>,
    mut told: Local<Vec<Species>>,
) {
    for (entity, creature) in &creatures {
        let defs = &bestiary.get(creature.species).zones;
        if defs.is_empty() {
            continue;
        }
        let zones: Vec<(Entity, Part, f32)> = children
            .iter_descendants(entity)
            .filter_map(|joint| {
                let name = names.get(joint).ok()?;
                defs.iter()
                    .find(|def| name.as_str().contains(def.joint.as_str()))
                    .map(|def| (joint, def.part, def.radius_m))
            })
            .collect();
        // Its model may not be in the world yet; look again next frame.
        if !zones.is_empty() {
            let count = |part| zones.iter().filter(|(_, each, _)| *each == part).count();
            // Said once for each kind, so a test can see its zones were found.
            if !told.contains(&creature.species) {
                told.push(creature.species);
                info!(
                    "A {} has {} head, {} body and {} leg zones.",
                    bestiary.name(creature.species),
                    count(Part::Head),
                    count(Part::Body),
                    count(Part::Legs)
                );
            }
            commands.entity(entity).try_insert(HitZones(zones));
        }
    }
}

/// The creatures' zones where they are now, to test a blow's path against.
#[derive(SystemParam)]
pub struct Zones<'w, 's> {
    zones: Query<'w, 's, &'static HitZones>,
    joints: Query<'w, 's, &'static GlobalTransform>,
}

impl Zones<'_, '_> {
    /// The first of a creature's zones a path from one point to another passes through:
    /// how far along it, from 0 to 1, and which part. Nothing if it misses them all, or
    /// the creature has none yet.
    pub fn along(&self, creature: Entity, from: Vec3, to: Vec3) -> Option<(f32, Part)> {
        let zones = self.zones.get(creature).ok()?;
        first_hit(
            from,
            to,
            zones.0.iter().filter_map(|&(joint, part, radius)| {
                let at = self.joints.get(joint).ok()?.translation();
                Some((at, radius, part))
            }),
        )
    }
}

/// The first sphere a path from one point to another enters, and how far along it.
pub fn first_hit(
    from: Vec3,
    to: Vec3,
    spheres: impl Iterator<Item = (Vec3, f32, Part)>,
) -> Option<(f32, Part)> {
    let step = to - from;
    let length_squared = step.length_squared();
    spheres
        .filter_map(|(centre, radius, part)| {
            let along = if length_squared > 1e-10 {
                ((centre - from).dot(step) / length_squared).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let nearest = from + step * along;
            let gap_squared = nearest.distance_squared(centre);
            if gap_squared > radius * radius {
                return None;
            }
            // Back from the nearest point to where the path enters the sphere.
            let back = (radius * radius - gap_squared).sqrt() / length_squared.sqrt().max(1e-5);
            Some(((along - back).max(0.0), part))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_strikes_the_first_zone_it_enters() {
        let head = (Vec3::new(0.0, 1.0, 5.0), 0.15, Part::Head);
        let body = (Vec3::new(0.0, 1.0, 6.0), 0.3, Part::Body);
        let legs = (Vec3::new(0.0, 0.3, 6.0), 0.08, Part::Legs);
        let zones = || [head, body, legs].into_iter();
        // Straight along at head height: the head comes first.
        let along_head = first_hit(Vec3::Y, Vec3::new(0.0, 1.0, 10.0), zones());
        assert_eq!(along_head.map(|(_, part)| part), Some(Part::Head));
        // Low, under the head and body: the legs.
        let low = first_hit(Vec3::new(0.0, 0.3, 0.0), Vec3::new(0.0, 0.3, 10.0), zones());
        assert_eq!(low.map(|(_, part)| part), Some(Part::Legs));
        // Wide of everything: a miss.
        assert!(first_hit(Vec3::new(2.0, 1.0, 0.0), Vec3::new(2.0, 1.0, 10.0), zones()).is_none());
        // Where along it enters: just before the head's middle.
        let (along, _) = along_head.unwrap();
        assert!((along * 10.0 - (5.0 - 0.15)).abs() < 0.01, "{along}");
    }
}
