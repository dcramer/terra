//! The controller's filmed course (docs/controller.md, "The in-game filmed course"; D457): the
//! plain-Rust course's features dug and built in the yard, in a line east of the start, for a
//! scripted player to jog and sprint through (`tools/controller/yard.ts`, `cargo xtask
//! controller --film`). East from the start, from 12 m on (clear of the yard's own arrival
//! and its base's plot), each laid on the ground where it stands:
//!
//! - a trench stepping down 0.1, 0.2, 0.3, 0.4 and 0.5 m to a floor 1.5 m down and up again
//!   0.5 to 0.1 m (the ground's steps, up and down);
//! - a lump of ground 0.35 m high on the way (a rock's step);
//! - a base: three stair cubes up to a deck 1.5 m high, a wall with a doorway 2 m high, a
//!   passage under a roof 2 m up (the low ceiling), and the deck's end, a drop of 1.5 m;
//! - last, a mound 2 m high with sides past the slope limit, square on the way, walked round
//!   (last, so the way round it leaves nothing after it to walk at a slant).
//!
//! The route through them is the experiment's (`WITH`); the log's `Course:` lines give the
//! features' places and the ground's height along the way before it was dug. No water: the
//! yard has none near.
//!
//!   cargo xtask sandbox --experiment controller-yard

use bevy::prelude::*;
use terra_build::{BuildOp, Cube, Facing, Fill, Kind, Shape};
use terra_core::Operation;
use terra_game::build::{Build, BuildLook, Built, frame_at, kit_names};
use terra_voxel::material;
use terra_world::planet::Planet;
use terra_world::start::Start;

/// What the course stands for: the game on the bare yard at noon, the player at the start
/// (the yard's own arrival is a few metres off it) facing east down the course, and the
/// route through it, metres east and south of the start: the trench's floor, over the lump,
/// up the stairs, through the doorway and the passage, off the deck's end, past the mound
/// (square on the way) and on.
pub const WITH: &[&str] = &[
    "--experiment",
    "play",
    "--level",
    "yard",
    "--no-lair",
    "--time",
    "12",
    "--at",
    "0,0",
    "--face",
    "22,0",
    "--route",
    "22.5,0",
    "--route",
    "31,0",
    "--route",
    "35,0",
    "--route",
    "40,0",
    "--route",
    "44,0",
    "--route",
    "56,0",
    "--route",
    "60,0",
];

/// The trench: where it starts and ends east, each tread's run, and each nested dig's depth,
/// the next a tread in from each end and deeper: steps 0.1, 0.2, 0.3, 0.4 and 0.5 m down to
/// a floor 1.5 m under the ground, and as many up. Nested, not side by side: two digs that
/// meet face to face leave a wall of no thickness between them in the field the mover reads
/// (each dig's distance is taken alone, `max(ground, -box, -box)` is 0 on the shared face),
/// which stopped the walk at the first such face.
const TRENCH_M: (f32, f32) = (16.0, 28.8);
const TREAD_M: f32 = 1.2;
const DEPTHS: [f32; 5] = [0.1, 0.3, 0.6, 1.0, 1.5];
/// Half the trench's width, m; a dig reaches so high over the ground, to clear it all.
const TRENCH_HALF_M: f32 = 1.5;
const CLEAR_M: f32 = 3.0;
/// The lump: a ball of ground so far east, its centre so far under the ground, so big.
const LUMP: (f32, f32, f32) = (30.5, 0.3, 0.65);
/// The mound: likewise, 2 m high with sides past 50 deg its first metre up.
const MOUND: (f32, f32, f32) = (50.0, 1.0, 3.0);
/// The base's frame so far east; its cubes are half a metre.
const BASE_M: f32 = 33.5;
const CUBE_M: f32 = 0.5;
/// The kit the base is laid in.
const KIT: &str = "stone";

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, dig).add_systems(
        Update,
        put_up.run_if(resource_exists::<BuildLook>.and_then(resource_exists::<Pending>)),
    );
}

/// The base's frame, waiting for building's look.
#[derive(Resource)]
struct Pending(Transform);

/// A point so far east and south of the start and so far up from the ground there, and the
/// frame of the ground there (its up the planet's).
fn on_ground(planet: &Planet, start: &Start, east: f32, south: f32) -> Transform {
    let dir = start.at(east, south).normalize();
    frame_at(planet.surface(dir), &start.0)
}

/// The ground along the way is logged so far east, a metre apart, before it is dug.
const ALONG_M: usize = 75;

fn dig(mut commands: Commands, mut planet: ResMut<Planet>, start: Res<Start>) {
    // The ground as it was along the way, metres up in the start's terms: what the filmed
    // course's checks measure heights from (the yard rolls a little, and falls away with the
    // planet's curve).
    let along: Vec<String> = (0..=ALONG_M)
        .map(|east| {
            let at = on_ground(&planet, &start, east as f32, 0.0).translation;
            format!("{:.3}", start.local(at).y)
        })
        .collect();
    info!(
        "Course: ground every metre east from the start: {}",
        along.join(" ")
    );
    for (k, deep) in DEPTHS.into_iter().enumerate() {
        let (from, to) = (
            TRENCH_M.0 + TREAD_M * k as f32,
            TRENCH_M.1 - TREAD_M * k as f32,
        );
        let frame = on_ground(&planet, &start, (from + to) / 2.0, 0.0);
        planet.apply(&Operation::DigBox {
            centre: frame.transform_point(Vec3::Y * (CLEAR_M - deep) / 2.0),
            half: Vec3::new((to - from) / 2.0, (CLEAR_M + deep) / 2.0, TRENCH_HALF_M),
            rotation: frame.rotation,
        });
    }
    for (what, (east, under, radius)) in [("lump", LUMP), ("mound", MOUND)] {
        let frame = on_ground(&planet, &start, east, 0.0);
        planet.apply(&Operation::Fill {
            centre: frame.transform_point(Vec3::NEG_Y * under),
            radius,
            material: material::SOIL,
        });
        info!("Course: {what} at {east},0, {:.2} m high", radius - under);
    }
    info!(
        "Course: trench {}-{} east, steps 0.1-0.5 m down to 1.5 m and up",
        TRENCH_M.0, TRENCH_M.1
    );
    let frame = on_ground(&planet, &start, BASE_M, 0.0);
    info!(
        "Course: base at {BASE_M},0: stairs to a deck 1.5 m up, doorway and passage 2 m high, a 1.5 m drop at {:.1}",
        BASE_M + 15.0 * CUBE_M
    );
    commands.insert_resource(Pending(frame));
}

/// The base's cubes, east (+x) in its frame, across (z) -3..=3: three stair cubes rising
/// east, a deck 1.5 m up from x 3 to 14, a wall at 7 with a doorway three cubes wide and
/// four high, a passage 10..=12 with walls either side and a roof over at 2 m.
fn base(kind: Kind) -> Vec<BuildOp> {
    let solid = Some(Cube::new(kind, Fill::Solid));
    let stair = Some(Cube {
        shape: Shape::Stair(Facing::East),
        ..Cube::new(kind, Fill::Solid)
    });
    let fill = |min: [i32; 3], max: [i32; 3], cube| BuildOp::Fill { min, max, cube };
    let mut ops = Vec::new();
    for x in 0..3 {
        if x > 0 {
            ops.push(fill([x, -1, -3], [x, x - 1, 3], solid));
        }
        ops.push(fill([x, x, -3], [x, x, 3], stair));
    }
    ops.push(fill([3, -1, -3], [14, 2, 3], solid));
    // The wall and its doorway.
    ops.push(fill([7, 3, -3], [7, 8, 3], solid));
    ops.push(fill([7, 3, -1], [7, 6, 1], None));
    // The passage: walls, then the roof at 2 m over the deck.
    ops.push(fill([10, 3, -3], [12, 6, -2], solid));
    ops.push(fill([10, 3, 2], [12, 6, 3], solid));
    ops.push(fill([10, 7, -3], [12, 7, 3], solid));
    ops
}

fn put_up(
    mut commands: Commands,
    pending: Res<Pending>,
    look: Res<BuildLook>,
    planet: Res<Planet>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let kits = kit_names();
    let kind = kits
        .iter()
        .find(|(_, kit)| kit.eq_ignore_ascii_case(KIT))
        .map(|(&kind, _)| kind)
        .unwrap_or_default();
    let mut build = Build::new(&mut commands, &look, pending.0, "Course base");
    build.replace_with(&base(kind), &mut commands, &planet, &mut meshes);
    build.redraw_all();
    commands.spawn((Name::new("Course base, as built"), Built(build)));
    commands.remove_resource::<Pending>();
}
