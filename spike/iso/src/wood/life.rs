//! The forest's life beyond the glade's beasts (wildlife.rs, which graze, root, peck and
//! dabble as they do in the glade): a moose walking the pond's far bank and back (its one
//! clip a walk, so it never stands), a few finches crossing the clearing high over the
//! fight and round again, and pollen and midges drifting in the sun about the fighters.

use crate::forest::Rng;
use crate::party::{Actor, creature};
use crate::ruin::effects::kit::{Cam, glow};
use crate::terrain::shape;
use crate::wildlife::{BOAR, CHAFFINCH, FOX, MOOSE, SPARROW};
use bevy::prelude::*;

pub fn plugin(app: &mut App) {
    app.add_systems(PostStartup, spawn)
        .add_systems(Update, (roam, drift));
}

/// The moose's beat along the pond's north bank, in the frame's upper right: it walks to
/// each in turn, then back (j11's beat from x 12 to 20.5 was off the frame's edge). Down
/// the bank toward the water, clear of the oak's leaves: at z -1.5 two thirds of it hid
/// behind them (8,400 px to 18,000 here at 1080, iso-art j47); past x 11 it meets the
/// frame's edge.
const BEAT: [Vec2; 3] = [
    Vec2::new(8.0, 0.2),
    Vec2::new(9.3, 0.5),
    Vec2::new(10.4, 0.9),
];
/// The moose's hide: a dark brown (Tripo's coat is a pale grey-brown) at a fighter's sheen
/// (rim::Hide); in the bank's shade it stood at the ground's value and hue.
const HIDE: Vec3 = Vec3::new(0.55, 0.38, 0.25);
/// The repository's own beasts on beats of their own (not the Tripo pipeline's): a fox
/// trotting the grass below the fight, a boar along the wood's edge west of the camp.
const FOX_BEAT: [Vec2; 3] = [
    Vec2::new(-1.5, 9.0),
    Vec2::new(1.0, 9.8),
    Vec2::new(3.0, 10.4),
];
const BOAR_BEAT: [Vec2; 2] = [Vec2::new(-3.0, 4.0), Vec2::new(-2.5, 7.5)];
/// The finches' crossing, from the wood west of the camp over the fight to the wood past
/// the pond's north bank (about 3 s across the frame), and how high over the land.
const CROSS: (Vec2, Vec2) = (Vec2::new(-3.0, 9.0), Vec2::new(11.0, -2.0));
const HIGH: f32 = 4.5;
/// Specks of pollen and midges about the fight, and how far from its middle they keep.
const SPECKS: usize = 90;
const SPREAD: f32 = 7.0;

/// An actor that goes round its beat for ever: walking it back and forth, or flying it and
/// starting again from the first point.
#[derive(Component)]
struct Roam {
    beat: Vec<Vec2>,
    next: usize,
    back: bool,
    fly: bool,
}

/// A speck: its home, the phases and sizes of its wander.
#[derive(Component)]
struct Speck {
    home: Vec3,
    phase: Vec3,
    reach: f32,
    size: f32,
}

fn spawn(
    mut commands: Commands,
    asset: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let mut moose = Actor::new(MOOSE, 1.45, 1.45, 2.2, 0.8);
    moose.path.push_back(BEAT[1]);
    let e = creature(&mut commands, &asset, moose, BEAT[0], 0.0, 1.0);
    commands.entity(e).insert((
        Roam {
            beat: BEAT.to_vec(),
            next: 1,
            back: false,
            fly: false,
        },
        crate::rim::Tint(HIDE),
        crate::rim::Hide,
    ));
    for (model, beat, walk, height, radius) in [
        (FOX, &FOX_BEAT[..], 0.9, 0.6, 0.25),
        (BOAR, &BOAR_BEAT[..], 0.8, 0.9, 0.45),
    ] {
        let mut beast = Actor::new(model, walk, walk, height, radius);
        beast.path.push_back(beat[1]);
        let e = creature(&mut commands, &asset, beast, beat[0], 0.0, 1.0);
        commands.entity(e).insert(Roam {
            beat: beat.to_vec(),
            next: 1,
            back: false,
            fly: false,
        });
    }
    let dir = (CROSS.1 - CROSS.0).normalize();
    let side = dir.perp();
    for i in 0..5 {
        let lag = dir * (-1.6 * i as f32) + side * [0.0, 0.9, -0.8, 1.7, -1.5][i];
        let model = if i % 2 == 0 { CHAFFINCH } else { SPARROW };
        let mut bird = Actor::new(model, 5.0, 5.0, 0.15, 0.08);
        bird.running = true;
        bird.lift = HIGH + 0.4 * i as f32;
        let from = CROSS.0 + lag;
        bird.path.push_back(CROSS.1 + lag);
        let e = creature(&mut commands, &asset, bird, from, 0.0, 1.7);
        commands.entity(e).insert(Roam {
            beat: vec![from, CROSS.1 + lag],
            next: 1,
            back: false,
            fly: true,
        });
    }
    let mut rng = Rng::new(0x9011e);
    let soft = images.add(crate::motes::soft_image());
    let card = meshes.add(Rectangle::new(1.0, 1.0));
    let mid = (super::KNIGHT + super::FOE + super::TARGET) / 3.0;
    let pollen = materials.add(glow(LinearRgba::rgb(2.4, 2.2, 1.5), Some(soft.clone())));
    let midge = materials.add(glow(LinearRgba::rgb(0.9, 0.85, 0.7), Some(soft)));
    for i in 0..SPECKS {
        let p = mid + Vec2::new(rng.range(-SPREAD, SPREAD), rng.range(-SPREAD, SPREAD));
        let home = p.extend(shape(p) + rng.range(0.3, 2.6)).xzy();
        let is_midge = i % 3 == 0;
        commands.spawn((
            Mesh3d(card.clone()),
            MeshMaterial3d(if is_midge {
                midge.clone()
            } else {
                pollen.clone()
            }),
            Transform::from_translation(home),
            bevy::light::NotShadowCaster,
            Speck {
                home,
                phase: Vec3::new(
                    rng.range(0.0, 6.3),
                    rng.range(0.0, 6.3),
                    rng.range(0.0, 6.3),
                ),
                reach: if is_midge { 0.25 } else { 0.9 },
                size: if is_midge {
                    0.03
                } else {
                    rng.range(0.035, 0.06)
                },
            },
        ));
    }
}

/// Gives each roamer its next point when it gets to the last.
fn roam(mut roamers: Query<(&mut Roam, &mut Actor, &mut Transform)>) {
    for (mut r, mut actor, mut tr) in &mut roamers {
        if !actor.path.is_empty() {
            continue;
        }
        if r.fly {
            // Round again: back to the start, out of sight in the wood, and across.
            let start = r.beat[0];
            tr.translation.x = start.x;
            tr.translation.z = start.y;
            actor.path.push_back(r.beat[1]);
            actor.running = true;
            continue;
        }
        let last = r.beat.len() - 1;
        if r.next == last {
            r.back = true;
        } else if r.next == 0 {
            r.back = false;
        }
        r.next = if r.back { r.next - 1 } else { r.next + 1 };
        let to = r.beat[r.next];
        actor.path.push_back(to);
    }
}

/// The specks wander about their homes (midges in a tight dance, pollen in a slow drift
/// on the breeze) facing the camera.
fn drift(time: Res<Time>, cam: Res<Cam>, mut specks: Query<(&Speck, &mut Transform)>) {
    let t = time.elapsed_secs();
    for (s, mut tr) in &mut specks {
        let k = if s.reach < 0.5 { 3.1 } else { 0.35 };
        let w = Vec3::new(
            (t * k * 1.3 + s.phase.x).sin() + 0.5 * (t * k * 2.9 + s.phase.y).sin(),
            0.6 * (t * k * 0.9 + s.phase.z).sin(),
            (t * k * 1.1 + s.phase.y).sin() + 0.5 * (t * k * 2.3 + s.phase.x).sin(),
        );
        tr.translation = s.home + w * s.reach;
        tr.rotation = cam.rot;
        tr.scale = Vec3::splat(s.size);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The moose's beat and the finches' crossing keep out of the pond, on dry land.
    #[test]
    fn the_moose_walks_dry_land() {
        for p in BEAT.into_iter().chain(FOX_BEAT).chain(BOAR_BEAT) {
            assert!(crate::terrain::pond(p) < -0.05, "{p} is in the pond");
            let fire = p.distance(crate::scene::FIRE);
            assert!(fire > 2.5, "{p} is {fire} m from the camp fire");
        }
    }

    /// No fern or stone stands in the moose's way, nor within half a metre of it.
    #[test]
    fn the_bank_is_clear_along_the_beat() {
        for w in BEAT.windows(2) {
            for k in 0..=20 {
                let p = w[0].lerp(w[1], k as f32 / 20.0);
                for side in [-0.5, 0.0, 0.5] {
                    let q = p + Vec2::new(0.0, side);
                    assert!(!super::super::dress::free(q), "a plant may stand at {q}");
                }
            }
        }
    }
}
