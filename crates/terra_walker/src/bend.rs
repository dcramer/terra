//! A bone turned after its clip about the body's own axes, as shipped engines adjust a pose in
//! the animation graph after the clips (Unreal's "Transform (Modify) Bone" node, Godot's
//! `SkeletonModifier3D`): what the fix-ups that run after the animation share (the staged
//! figures' bends, the arms kept off the body, the head kept on the aim).
//!
//! The body's space is the model's: its root's children down, the root's own transform (which
//! turns the model to face its figure's way) left out, so +z is the way the body faces, +y up
//! and +x its left, as the model file has them.

use bevy::{math::Affine3A, prelude::*};

/// A bone's own rotation, `local` under a parent turned `parent` from the body, turned by
/// `turn` about the body's axes at its joint: what makes the bone, in the body, `turn` times
/// where the clip put it.
pub fn bent(parent: Quat, local: Quat, turn: Quat) -> Quat {
    (parent.inverse() * turn * parent * local).normalize()
}

/// `entity`'s place in the body whose model root is `root`: its own and its ancestors'
/// transforms as they are now (before this frame's propagation), up to but not including the
/// root's.
pub fn in_body(
    entity: Entity,
    root: Entity,
    parents: &Query<&ChildOf>,
    transforms: &Query<&mut Transform>,
) -> Affine3A {
    let mut at = Affine3A::IDENTITY;
    let mut each = entity;
    while each != root {
        let Ok(local) = transforms.get(each) else {
            break;
        };
        at = local.compute_affine() * at;
        let Ok(parent) = parents.get(each) else {
            break;
        };
        each = parent.parent();
    }
    at
}

/// The rotation a clip gave a bone and what a fix-up wrote over it the last frame, so a bone no
/// clip poses again this frame is turned from the clip's pose rather than again on top of its
/// own turn.
#[derive(Clone, Copy, Debug, Default)]
pub struct Kept {
    last: Option<(Quat, Quat)>,
}

impl Kept {
    /// The clip's rotation for a bone whose rotation is `now`: `now`, unless it is still what
    /// was written last frame.
    pub fn clip(&self, now: Quat) -> Quat {
        match self.last {
            Some((clip, written)) if now == written => clip,
            _ => now,
        }
    }

    /// The bone's rotation from its clip's `clip`, turned by `turn` about the body's axes under
    /// a parent turned `parent` in the body; remembered, and returned to be written.
    pub fn turn(&mut self, clip: Quat, parent: Quat, turn: Quat) -> Quat {
        let written = bent(parent, clip, turn);
        self.last = Some((clip, written));
        written
    }
}

/// The named bones under a model root, by name, in the order asked; none if any is missing.
pub fn find<const N: usize>(
    root: Entity,
    wanted: [&str; N],
    children: &Query<&Children>,
    names: &Query<&Name>,
) -> Option<[Entity; N]> {
    let mut found = [None; N];
    for each in children.iter_descendants(root) {
        if let Ok(name) = names.get(each)
            && let Some(k) = wanted.iter().position(|w| *w == name.as_str())
            && found[k].is_none()
        {
            found[k] = Some(each);
        }
    }
    let mut out = [Entity::PLACEHOLDER; N];
    for (k, each) in found.into_iter().enumerate() {
        out[k] = each?;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bent_bone_points_where_the_turn_turns_it_in_the_body() {
        let parent = Quat::from_euler(EulerRot::XYZ, 0.4, -1.1, 0.3);
        let local = Quat::from_euler(EulerRot::XYZ, -0.7, 0.2, 1.3);
        let turn = Quat::from_euler(EulerRot::XYZ, 0.2, -0.4, 0.7);
        let along = Vec3::new(0.2, -0.9, 0.1).normalize();
        let before = parent * local * along;
        let after = parent * bent(parent, local, turn) * along;
        assert!((after - turn * before).length() < 1e-5);
    }

    #[test]
    fn a_bone_no_clip_posed_again_is_turned_from_the_clips_pose_not_twice() {
        let mut kept = Kept::default();
        let clip = Quat::from_rotation_x(0.3);
        let turn = Quat::from_rotation_z(0.5);
        let once = kept.turn(kept.clip(clip), Quat::IDENTITY, turn);
        // The next frame no clip wrote it: it still reads what was written.
        let again = kept.turn(kept.clip(once), Quat::IDENTITY, turn);
        assert!(again.angle_between(once) < 1e-5);
        // A clip wrote it anew: turned from that.
        let posed = Quat::from_rotation_x(-0.2);
        assert_eq!(kept.clip(posed), posed);
    }
}
