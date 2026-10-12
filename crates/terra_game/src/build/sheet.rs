//! The kit sheet (docs/kits.md, D69, D71): on the yard, with `--kit-sheet`, one of each
//! way log walls meet, laid in a row of three: built, going up and as a blueprint. Each
//! row's vantage is written to the log, so a picture of it is one command:
//!
//!   cargo run -p terra_sandbox -- --experiment play --level yard --kit-sheet \
//!     --vantage <from the log> --shot out/corner.png
//!
//! The kit's file is read again when it is saved, so the sheet can be left running while
//! the look is tuned.

use super::{Build, BuildLook, Built, FLOOR_CLEARANCE_M, Shown, Site, frame_at, lifted, read_kit};
use crate::planet::Planet;
use crate::shell::Options;
use bevy::prelude::*;
use std::sync::Arc;
use terra_build::{Base, BuildOp, Face, Panel, RoofSection, RoofStyle, Side};

/// How far apart the rows are, and the three states along each, in metres.
const ROW_M: f32 = 14.0;
const STATE_M: f32 = 11.0;
/// The first row's distance north of the start.
const FIRST_ROW_M: f32 = 14.0;
/// How far a going-up row has got.
const GOING_UP: f32 = 0.45;

/// A wall on the grid line `z` cells south, from grid point `from` to `to` east.
fn east_west(from: i32, to: i32, z: i32) -> impl Iterator<Item = BuildOp> {
    (from..to).map(move |x| {
        BuildOp::SetWall(
            Face::new(IVec3::new(x, 0, z - 1), Side::South),
            Some(Panel::Solid),
        )
    })
}

/// A wall on the grid line `x` cells east, from grid point `from` to `to` south.
fn north_south(from: i32, to: i32, x: i32) -> impl Iterator<Item = BuildOp> {
    (from..to).map(move |z| {
        BuildOp::SetWall(
            Face::new(IVec3::new(x - 1, 0, z), Side::East),
            Some(Panel::Solid),
        )
    })
}

/// Each row: its name, and the operations that make it, in cells of half a metre.
fn rows() -> Vec<(&'static str, Vec<BuildOp>)> {
    let cabin = {
        let mut ops = vec![BuildOp::AddRoom {
            min: [0, 0, 0],
            max: [8, 0, 6],
        }];
        // A door a metre wide in the south wall, and a window in it and the east wall.
        ops.extend((2..4).map(|x| {
            BuildOp::SetWall(
                Face::new(IVec3::new(x, 0, 6), Side::South),
                Some(Panel::Door),
            )
        }));
        ops.extend((5..7).map(|x| {
            BuildOp::SetWall(
                Face::new(IVec3::new(x, 0, 6), Side::South),
                Some(Panel::WINDOW),
            )
        }));
        ops.extend((2..5).map(|z| {
            BuildOp::SetWall(
                Face::new(IVec3::new(8, 0, z), Side::East),
                Some(Panel::WINDOW),
            )
        }));
        ops.push(BuildOp::AddRoof(RoofSection {
            min: [0, 0, 0],
            max: [8, 0, 6],
            style: RoofStyle::Gable,
            along_x: true,
            pitch: 0.7,
        }));
        ops
    };
    vec![
        ("free end", east_west(0, 6, 0).collect()),
        (
            "corner",
            east_west(0, 6, 0).chain(north_south(0, 6, 0)).collect(),
        ),
        (
            "T",
            east_west(0, 10, 0).chain(north_south(0, 6, 5)).collect(),
        ),
        (
            "cross",
            east_west(0, 10, 5).chain(north_south(0, 10, 5)).collect(),
        ),
        ("long wall", east_west(0, 20, 0).collect()),
        ("cabin", cabin),
    ]
}

/// Lays the sheet once the site is there.
#[allow(clippy::too_many_arguments)]
pub(super) fn lay(
    mut done: Local<bool>,
    options: Res<Options>,
    site: Option<Res<Site>>,
    look: Option<Res<BuildLook>>,
    planet: Res<Planet>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    if *done || !options.kit_sheet || !options.yard() {
        return;
    }
    let (Some(site), Some(look)) = (site, look) else {
        return;
    };
    *done = true;
    let Some(kit) = read_kit().map(Arc::new) else {
        return;
    };
    for (row, (name, ops)) in rows().into_iter().enumerate() {
        let north = -FIRST_ROW_M - ROW_M * row as f32;
        for (column, shown) in [Shown::Built, Shown::Plan(GOING_UP), Shown::Plan(0.0)]
            .into_iter()
            .enumerate()
        {
            let east = STATE_M * column as f32;
            // On the ground there, the yard being round.
            let point = site.frame.transform_point(Vec3::new(east, 0.0, north));
            let ground = point - point.normalize() * planet.distance(point);
            let mut base = Base::default();
            for &op in &ops {
                base.apply(op);
            }
            let frame = frame_at(ground + ground.normalize() * FLOOR_CLEARANCE_M, &site.frame);
            // Walls with no floor stand on footings in the ground instead.
            let frame = if base.floors().next().is_some() {
                lifted(frame, &base, &planet)
            } else {
                frame
            };
            let label = format!("Kit sheet: {name}, {shown:?}");
            let mut build = Build::new(&mut commands, &look, frame, &label);
            build.logs = Some(kit.clone());
            build.shown = shown;
            build.replace_with(&ops, &mut commands, &planet, &mut meshes);
            commands.spawn((Name::new(label), Built(build)));
        }
        info!(
            "Kit sheet: {name}: --vantage {},{},{},{},{},{}",
            STATE_M + 2.0,
            9.0,
            north + 22.0,
            STATE_M + 2.0,
            0.5,
            north + 1.5
        );
    }
}
