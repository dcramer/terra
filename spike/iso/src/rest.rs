//! The party at rest about its fire (`--demo` tells them to make camp): each member's place,
//! sitting on a trunk facing the flames or standing watch with their back to it, and how
//! they settle there when they arrive. An order elsewhere stands them up (command.rs).

use bevy::prelude::*;

use crate::party::{Actor, Model, PortraitOf};
use crate::scene::FIRE;

/// A member's place in the camp, and whether they sit there or stand watch.
#[derive(Component)]
pub struct Rest {
    pub at: Vec2,
    pub sits: bool,
}

/// How far below the land a sitter's feet go: the sitting clip's hips are 0.57 m up, a
/// chair's height, and a trunk's top is about 0.37 m (camp.rs), less the flesh under the hips.
const SINK: f32 = 0.1;

pub fn plugin(app: &mut App) {
    app.add_systems(Update, (settle, portraits));
}

/// A member who has come to their place sits (their busy clip) or stands, facing the fire or
/// away from it; one who has somewhere to go is up.
fn settle(
    mut members: Query<(&Rest, &mut Actor, &mut Transform, &Children)>,
    mut models: Query<&mut Transform, (With<Model>, Without<Actor>)>,
) {
    for (rest, mut actor, mut tr, kids) in &mut members {
        let home = actor.path.is_empty() && tr.translation.xz().distance(rest.at) < 0.3;
        let sits = home && rest.sits;
        if actor.busy != sits {
            actor.busy = sits;
            actor.lift = if sits { -SINK } else { 0.0 };
        }
        if !home {
            continue;
        }
        tr.translation.x = rest.at.x;
        tr.translation.z = rest.at.y;
        let to = if rest.sits {
            FIRE - rest.at
        } else {
            rest.at - FIRE
        };
        // The models face +Z.
        let want = Quat::from_rotation_y(to.x.atan2(to.y));
        for kid in kids {
            if let Ok(mut m) = models.get_mut(*kid) {
                m.rotation = want;
            }
        }
    }
}

/// How much lower a sitter's face is than a stander's: the sitting clip's hips are 0.375 m
/// lower than standing, and the sitter sinks SINK more.
const SEATED_DROP: f32 = 0.375 + SINK;

/// A portrait's camera, drawn from in front of the face (party.rs), follows it down when its
/// member sits, or it shows the wood behind them.
fn portraits(
    actors: Query<&Actor, Changed<Actor>>,
    mut cameras: Query<(&PortraitOf, &mut Transform)>,
    mut seated: Local<bevy::platform::collections::HashMap<Entity, bool>>,
) {
    for (of, mut tr) in &mut cameras {
        let Ok(actor) = actors.get(of.0) else {
            continue;
        };
        let was = seated.insert(of.0, actor.busy).unwrap_or(false);
        if was != actor.busy {
            tr.translation.y += if actor.busy {
                -SEATED_DROP
            } else {
                SEATED_DROP
            };
        }
    }
}
