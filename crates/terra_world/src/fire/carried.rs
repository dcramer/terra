//! A kept fire carried by a body (a torch in hand): where on that body its lights hang, and
//! the remote's `fire.carried`, which reads and sets it so one game run can shoot the same
//! place with one rig and another (tools/sandbox/torchlight.ts).

use crate::remote::method;
use bevy::prelude::*;
use bevy::remote::{BrpResult, builtin_methods::parse_some};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// On a kept fire carried by a body: whose body, on which side of it the fire is held (1 its
/// right, -1 its left), and where its lights hang on that body, each a place in metres from
/// the body's feet: to the fire's side, up, ahead. The flames and the unshadowed light at
/// their tips ride the hand always. Set by whoever puts the fire in the hand
/// (`terra_survival::torchlight`); the fire knows no players.
///
/// `key` is its shadowed light's place: on the body, the shadows it casts move as the holder
/// does, not with each swing of the arm (on the hand bone the key's step changed 16 mm a
/// frame on the mean while running, 228 at most, torch-light j3). None, and the key hangs
/// over the flames (`light::torch`).
///
/// `fill` is a dim unshadowed light on the holder (`light::torch::fill`); none, and there is
/// none.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Carried {
    pub by: Entity,
    pub side: f32,
    pub key: Option<[f32; 3]>,
    pub fill: Option<Fill>,
}

/// A carried fire's fill: where it hangs (to the fire's side, up, ahead, metres from the
/// holder's feet) and the share of the fire's lumens it gives.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Fill {
    pub at: [f32; 3],
    pub share: f32,
}

/// A carried fire's lights' places in the world this frame (`Carried` through its holder's
/// frame): its key's, and its fill's with its share.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct CarriedAt {
    pub key: Option<Vec3>,
    pub fill: Option<(Vec3, f32)>,
}

impl Carried {
    /// Its places with its holder's frame `holder` (a body faces -z, its right +x).
    pub(super) fn at(&self, holder: &GlobalTransform) -> CarriedAt {
        let hung = |m: [f32; 3]| holder.transform_point(Vec3::new(self.side * m[0], m[1], -m[2]));
        CarriedAt {
            key: self.key.map(hung),
            fill: self.fill.map(|fill| (hung(fill.at), fill.share)),
        }
    }
}

pub fn plugin(app: &mut App) {
    method(app, "fire.carried", carried);
}

/// `fire.carried`'s parameters: with `set`, every carried fire's key and fill become these
/// (a null or missing one is none).
#[derive(Deserialize)]
struct Set {
    set: Option<Rig>,
}

#[derive(Deserialize)]
struct Rig {
    key: Option<[f32; 3]>,
    fill: Option<Fill>,
}

/// Every carried fire's key and fill as they were when called; with `{set: {key, fill}}` each
/// then takes those. Until the fire is put down: a torch taken in hand again has its
/// maker's. What a script shoots one place with two rigs by.
fn carried(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let set = match params {
        Some(params) => parse_some::<Set>(Some(params))?.set,
        None => None,
    };
    let mut fires = world.query::<&mut Carried>();
    let mut were = Vec::new();
    for mut fire in fires.iter_mut(world) {
        were.push(json!({ "side": fire.side, "key": fire.key, "fill": fire.fill }));
        if let Some(rig) = &set {
            fire.key = rig.key;
            fire.fill = rig.fill;
        }
    }
    Ok(Value::Array(were))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A place is to the fire's side, up and ahead on its holder's body, wherever the body
    /// stands and faces; a fire that says none has none.
    #[test]
    fn a_carried_fires_lights_hang_on_its_holders_body_on_the_fires_side() {
        let feet = Vec3::new(5.0, 2000.0, -3.0);
        let fire = |side| Carried {
            by: Entity::PLACEHOLDER,
            side,
            key: Some([0.3, 2.0, 0.5]),
            fill: Some(Fill {
                at: [0.0, 1.4, -1.0],
                share: 0.03,
            }),
        };
        let stood = GlobalTransform::from(Transform::from_translation(feet));
        let (right, left) = (fire(1.0).at(&stood), fire(-1.0).at(&stood));
        assert!(
            right
                .key
                .unwrap()
                .distance(feet + Vec3::new(0.3, 2.0, -0.5))
                < 1e-3
        );
        assert!(
            left.key
                .unwrap()
                .distance(feet + Vec3::new(-0.3, 2.0, -0.5))
                < 1e-3
        );
        assert_eq!(right.fill.unwrap().1, 0.03);
        assert!(
            right
                .fill
                .unwrap()
                .0
                .distance(feet + Vec3::new(0.0, 1.4, 1.0))
                < 1e-3
        );
        // Facing +x, ahead is +x and the right +z.
        let turned = Transform::from_translation(feet).looking_to(Vec3::X, Vec3::Y);
        let key = fire(1.0).at(&GlobalTransform::from(turned)).key.unwrap();
        assert!(
            key.distance(feet + Vec3::new(0.5, 2.0, 0.3)) < 1e-3,
            "{}",
            key - feet
        );
        let bare = Carried {
            key: None,
            fill: None,
            ..fire(1.0)
        };
        assert_eq!(bare.at(&stood), CarriedAt::default());
    }

    /// `fire.carried` tells each carried fire's rig and, with `set`, gives each another.
    #[test]
    fn the_remote_reads_and_sets_a_carried_fires_rig() {
        let mut world = World::new();
        let fire = world
            .spawn(Carried {
                by: Entity::PLACEHOLDER,
                side: -1.0,
                key: None,
                fill: None,
            })
            .id();
        let id = world.register_system(carried);
        let rig =
            json!({ "key": [0.25, 2.25, 0.5], "fill": { "at": [0.0, 1.5, -1.0], "share": 0.04 } });
        let was = world
            .run_system_with(id, Some(json!({ "set": rig })))
            .unwrap()
            .unwrap();
        assert_eq!(was, json!([{ "side": -1.0, "key": null, "fill": null }]));
        let now = *world.get::<Carried>(fire).unwrap();
        assert_eq!(now.key, Some([0.25, 2.25, 0.5]));
        assert_eq!(now.fill.unwrap().share, 0.04);
        // Read alone, nothing changes; set with nulls, both go.
        let read = world.run_system_with(id, None).unwrap().unwrap();
        assert_eq!(read[0]["key"], json!([0.25, 2.25, 0.5]));
        assert_eq!(*world.get::<Carried>(fire).unwrap(), now);
        world
            .run_system_with(id, Some(json!({ "set": {} })))
            .unwrap()
            .unwrap();
        assert_eq!(world.get::<Carried>(fire).unwrap().key, None);
        assert_eq!(world.get::<Carried>(fire).unwrap().fill, None);
    }
}
