//! Places kept clear of what grows (`ClearAround`): asked once, held for the world, so a
//! plant that grows there later is cleared as it comes.

use super::{ClearedAway, Growth, Obstacles, clear_away};
use bevy::prelude::*;

/// The places asked to be cleared ([`ClearAround`]), each its middle and how far round it:
/// kept for the world, so what grows there after is cleared as it comes.
#[derive(Resource, Default, Debug, Clone)]
pub struct KeptClear(Vec<(Vec3, f32)>);

impl KeptClear {
    /// Keeps a place clear from now on: what is laid or grows there after stands down. Asked
    /// twice, kept once.
    pub fn keep(&mut self, at: Vec3, radius: f32) {
        if !self.0.contains(&(at, radius)) {
            self.0.push((at, radius));
        }
    }

    /// Keeps a place clear no longer: the thing that stood there is gone (a realm's part let
    /// go behind the player), so what is laid there after may stand.
    pub fn release(&mut self, at: Vec3, radius: f32) {
        self.0.retain(|kept| *kept != (at, radius));
    }

    /// Whether a place is within one kept clear, measured over the ground.
    pub fn holds(&self, place: Vec3) -> bool {
        self.0
            .iter()
            .any(|&(at, radius)| (place - at).reject_from(at.normalize()).length() < radius)
    }
}

/// Clears what has grown since in a place kept clear: a plant laid, or a tree streamed in,
/// after the thing it would stand in was set down.
pub(crate) fn clear_grown(
    kept: Option<Res<KeptClear>>,
    mut obstacles: ResMut<Obstacles>,
    mut grown: Query<(Entity, &Growth, &Transform, &mut Visibility), Added<Growth>>,
    mut gone: MessageWriter<ClearedAway>,
) {
    let Some(kept) = kept.filter(|kept| !kept.0.is_empty()) else {
        return;
    };
    for (entity, growth, transform, visibility) in &mut grown {
        if kept.holds(transform.translation) {
            clear_away(entity, growth, visibility, &mut obstacles, &mut gone);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_place_kept_clear_is_held_until_released() {
        let mut kept = KeptClear::default();
        let at = Vec3::new(0.0, 1000.0, 0.0);
        kept.keep(at, 3.0);
        kept.keep(at, 3.0);
        assert!(kept.holds(at + Vec3::X * 2.0) && !kept.holds(at + Vec3::X * 4.0));
        // Asked twice it was kept once, so one release frees it.
        kept.release(at, 3.0);
        assert!(!kept.holds(at + Vec3::X * 2.0));
    }
}
