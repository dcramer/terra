//! The grown kinds a world has, kept once it is planted, so what stands by a realm's rules
//! (`Grows::Stood`: the far realm's cap trees, `terra_game::play::realm_parts`) is stood as
//! the player walks, of the same variants and in the same materials as what was planted.

use super::model::{Drawn, Model};
use crate::flora_kinds::Flora;
use crate::grown;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;

/// Every grown kind's variants by its name, and the materials they are drawn in.
#[derive(Resource)]
pub struct Stock {
    kinds: HashMap<String, Vec<grown::Variant>>,
    looks: grown::Looks,
}

impl Stock {
    pub(super) fn of(flora: &Flora, models: &[Vec<Model>], looks: grown::Looks) -> Stock {
        let kinds = (flora.kinds().iter().zip(models))
            .filter_map(|(kind, models)| {
                let variants: Vec<grown::Variant> = (models.iter())
                    .filter_map(|model| match &model.look {
                        Drawn::Grown(variant) => Some((**variant).clone()),
                        Drawn::Mesh(_) => None,
                    })
                    .collect();
                (!variants.is_empty()).then(|| (kind.name.clone(), variants))
            })
            .collect();
        Stock { kinds, looks }
    }

    /// How many variants a kind has: none for one this world does not grow.
    pub fn variants(&self, kind: &str) -> usize {
        self.kinds.get(kind).map_or(0, Vec::len)
    }

    /// Stands one of a kind's variants on the ground at a point, so tall and so turned (a
    /// turn about the up, radians): its entity and half its width at the ground, or none
    /// for a kind this world does not grow.
    pub fn stand(
        &self,
        commands: &mut Commands,
        kind: &str,
        (variant, at, tall, turn): (usize, Vec3, f32, f32),
    ) -> Option<(Entity, f32)> {
        let variants = self.kinds.get(kind)?;
        let variant = &variants[variant % variants.len()];
        let up = at.normalize();
        let scale = tall / variant.height.max(0.01);
        let transform = Transform {
            translation: at,
            rotation: Quat::from_axis_angle(up, turn) * Quat::from_rotation_arc(Vec3::Y, up),
            scale: Vec3::splat(scale),
        };
        // Named for its kind, so a scripted run finds it (D69).
        let mut entity = commands.spawn((
            transform,
            Visibility::default(),
            Name::new(kind.to_string()),
        ));
        grown::draw(&mut entity, variant, tall, &self.looks);
        Some((entity.id(), variant.spread * scale))
    }
}
