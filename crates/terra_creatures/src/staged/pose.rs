//! A staged figure's pose bent after its clip: some of its bones turned a set amount about
//! the body's own axes on top of whatever the clip says, as shipped engines adjust a clip made
//! for one body to another of other proportions (Unreal's IK Retargeter's retarget pose, or a
//! "Transform (Modify) Bone" node after the clip in an animation graph). Clips made on a person
//! put a troll's hands inside its belly and a man's round shield through his chest; turning the
//! upper arms out and forward clears them. A bend is told with the figure (`staged.spawn`'s
//! `bend`) or later (`staged.bend`), blended in over a moment, and holds through every clip
//! until it is told again.
//!
//! A bend is degrees `[x, y, z]` about the body's axes as its model file has them: x to its
//! left, y up, z ahead, each turning by the right hand's rule, the z turn first, then y, then
//! x. A left arm hanging down is turned out from the body by a positive z, a right arm by a
//! negative one; either is brought forward by a negative x.

use bevy::{platform::collections::HashMap, prelude::*};
use std::collections::BTreeMap;
use terra_walker::bend::Kept;

/// A figure's bends: what each named bone is to be turned by, what it was turned by when
/// last told, how far through the blend between them, and the bones found in its body.
#[derive(Component, Default)]
pub struct Bends {
    to: HashMap<String, Quat>,
    from: HashMap<String, Quat>,
    blend_s: f32,
    done_s: f32,
    bones: HashMap<String, Bone>,
    missing: Vec<String>,
}

/// A bone being bent: its entity, and the clip's rotation and what was written over it.
struct Bone {
    entity: Entity,
    kept: Kept,
}

/// A bend as the calls give it, degrees about the body's x, y and z.
pub fn turn([x, y, z]: [f32; 3]) -> Quat {
    Quat::from_rotation_x(x.to_radians())
        * Quat::from_rotation_y(y.to_radians())
        * Quat::from_rotation_z(z.to_radians())
}

impl Bends {
    /// Bends told: each bone in `bend` blended to its turn over `blend_s`, any other bent
    /// before blended back to none.
    pub fn tell(&mut self, bend: &BTreeMap<String, [f32; 3]>, blend_s: f32) {
        let names: Vec<String> = self.to.keys().chain(bend.keys()).cloned().collect();
        let mut from = HashMap::new();
        for name in names {
            from.insert(name.clone(), self.now(&name));
        }
        self.from = from;
        self.to = bend
            .iter()
            .map(|(name, degrees)| (name.clone(), turn(*degrees)))
            .collect();
        for name in self.from.keys() {
            self.to.entry(name.clone()).or_insert(Quat::IDENTITY);
        }
        self.blend_s = blend_s.max(0.0);
        self.done_s = 0.0;
    }

    /// A bone's turn now, through the blend.
    fn now(&self, name: &str) -> Quat {
        let to = self.to.get(name).copied().unwrap_or(Quat::IDENTITY);
        let from = self.from.get(name).copied().unwrap_or(Quat::IDENTITY);
        let t = if self.blend_s <= 0.0 {
            1.0
        } else {
            (self.done_s / self.blend_s).clamp(0.0, 1.0)
        };
        from.slerp(to, t * t * (3.0 - 2.0 * t))
    }
}

/// Each figure's bends laid over its clip, after the animation has posed it and before the
/// pose is carried to the skin.
pub fn bend(
    time: Res<Time>,
    mut figures: Query<(Entity, &mut Bends)>,
    children: Query<&Children>,
    names: Query<&Name>,
    parents: Query<&ChildOf>,
    mut transforms: Query<&mut Transform>,
) {
    let dt = time.delta_secs();
    for (figure, mut bends) in &mut figures {
        bends.done_s += dt;
        find(figure, &mut bends, &children, &names);
        let wanted: Vec<(String, Quat)> = bends
            .to
            .keys()
            .map(|name| (name.clone(), bends.now(name)))
            .collect();
        for (name, turn) in wanted {
            let Some(bone) = bends.bones.get_mut(&name) else {
                continue;
            };
            // The bone's parent's rotation in the body: its ancestors' up to the body's own
            // node, a child of the figure, which turns the model to face the figure's way.
            let mut parent = Quat::IDENTITY;
            let mut at = bone.entity;
            while let Ok(ChildOf(up)) = parents.get(at) {
                let up = *up;
                if parents.get(up).is_ok_and(|p| p.parent() == figure) || up == figure {
                    break;
                }
                if let Ok(t) = transforms.get(up) {
                    parent = t.rotation * parent;
                }
                at = up;
            }
            let Ok(mut transform) = transforms.get_mut(bone.entity) else {
                continue;
            };
            let clip = bone.kept.clip(transform.rotation);
            transform.rotation = bone.kept.turn(clip, parent, turn);
        }
    }
}

/// The figure's bones its bends name, found once its body is in.
fn find(figure: Entity, bends: &mut Bends, children: &Query<&Children>, names: &Query<&Name>) {
    let unfound: Vec<String> = bends
        .to
        .keys()
        .filter(|name| !bends.bones.contains_key(*name) && !bends.missing.contains(name))
        .cloned()
        .collect();
    if unfound.is_empty() || children.get(figure).is_err() {
        return;
    }
    let mut any = false;
    for each in children.iter_descendants(figure) {
        any = true;
        if let Ok(name) = names.get(each)
            && unfound.iter().any(|n| n == name.as_str())
        {
            bends.bones.insert(
                name.to_string(),
                Bone {
                    entity: each,
                    kept: Kept::default(),
                },
            );
        }
    }
    // A body in with bones (a skin's nodes are many) but none of that name: said once.
    if any && children.iter_descendants(figure).count() > 20 {
        for name in unfound {
            if !bends.bones.contains_key(&name) {
                warn!("Staged: no bone {name} to bend.");
                bends.missing.push(name);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use std::time::Duration;

    /// A figure, its body's node turned to face its way, a pelvis turned in it and an upper
    /// arm under that pointing down in the body, and some bones besides, as a model's are.
    fn figure(world: &mut World) -> (Entity, Entity, Quat) {
        let figure = world.spawn(Transform::default()).id();
        let body = world
            .spawn((
                Transform::from_rotation(Quat::from_rotation_y(std::f32::consts::PI)),
                ChildOf(figure),
            ))
            .id();
        let pelvis_turn = Quat::from_rotation_x(0.6);
        let pelvis = world
            .spawn((
                Name::new("pelvis"),
                Transform::from_rotation(pelvis_turn),
                ChildOf(body),
            ))
            .id();
        // Pointing down in the body: its own -y turned by the pelvis's is the body's -y.
        let arm = world
            .spawn((
                Name::new("upperarm_l"),
                Transform::from_rotation(pelvis_turn.inverse()),
                ChildOf(pelvis),
            ))
            .id();
        for k in 0..24 {
            world.spawn((Name::new(format!("bone{k}")), ChildOf(pelvis)));
        }
        (figure, arm, pelvis_turn)
    }

    #[test]
    fn an_arm_bent_out_by_thirty_degrees_hangs_thirty_degrees_out_and_no_more_each_frame() {
        let mut world = World::new();
        let mut time = Time::<()>::default();
        time.advance_by(Duration::from_secs_f32(1.0 / 60.0));
        world.insert_resource(time);
        let (figure, arm, pelvis) = figure(&mut world);
        let mut bends = Bends::default();
        bends.tell(
            &BTreeMap::from([("upperarm_l".to_string(), [0.0, 0.0, 30.0])]),
            0.0,
        );
        world.entity_mut(figure).insert(bends);
        // No clip poses it again: three frames bend it once.
        for _ in 0..3 {
            world.run_system_once(bend).unwrap();
        }
        let rotation = world.get::<Transform>(arm).unwrap().rotation;
        let down = pelvis * rotation * Vec3::NEG_Y;
        // Out to its left (+x) by sin 30 degrees, still as low as cos 30.
        assert!((down.x - 0.5).abs() < 1e-4, "{down}");
        assert!((down.y + 0.866).abs() < 1e-3, "{down}");
        // Told none, it blends back to the clip's pose.
        world
            .get_mut::<Bends>(figure)
            .unwrap()
            .tell(&BTreeMap::new(), 0.0);
        world.run_system_once(bend).unwrap();
        let rotation = world.get::<Transform>(arm).unwrap().rotation;
        assert!((pelvis * rotation * Vec3::NEG_Y - Vec3::NEG_Y).length() < 1e-4);
    }
}
