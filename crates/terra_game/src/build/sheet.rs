//! The kit sheet (docs/building-kits.md, D69, D71): with `--kit-sheet`, one of each
//! way log walls meet, laid in a row of three: built, going up and as a blueprint. Each
//! row's vantage is written to the log, so a picture of it is one command:
//!
//!   cargo run -p terra_sandbox -- --experiment play --level yard --kit-sheet \
//!     --vantage <from the log> --shot out/corner.png
//!
//! On the yard, the flat test level, to compare pieces; without `--level yard`, in the
//! world, among its trees and on its slopes, to judge them in a whole scene.
//!
//! Any model given with `--model` stands in a row after the sheet's, on the ground and lit
//! as the game is, so a piece or an experiment is judged beside what is built.
//!
//! The kit's file is read again when it is saved, so the sheet can be left running while
//! the look is tuned.

use super::{Build, BuildLook, Built, FLOOR_CLEARANCE_M, Shown, Site, frame_at, lifted, read_kits};
use crate::planet::Planet;
use crate::shell::Options;
use bevy::prelude::*;
use std::sync::Arc;
use terra_build::logs::Pattern;
use terra_build::{Base, BuildOp, Cube, Fill, Kind, Panel, RoofSection, RoofStyle, STOREY_CUBES};

/// How far apart the rows are, and the three states along each, in metres.
const ROW_M: f32 = 14.0;
const STATE_M: f32 = 11.0;
/// The first row's distance north of the start.
const FIRST_ROW_M: f32 = 14.0;
/// How far a going-up row has got.
const GOING_UP: f32 = 0.45;

/// A row of wall tiles from `from` to `to` east, on the row `z` south, in cubes.
fn east_west(from: i32, to: i32, z: i32, kind: Kind) -> impl Iterator<Item = BuildOp> {
    (from..=to).map(move |x| wall([x, 0, z], Panel::Solid, kind))
}

/// A row of wall tiles from `from` to `to` south, on the row `x` east, in cubes.
fn north_south(from: i32, to: i32, x: i32, kind: Kind) -> impl Iterator<Item = BuildOp> {
    (from..=to).map(move |z| wall([x, 0, z], Panel::Solid, kind))
}

fn wall(tile: [i32; 3], panel: Panel, kind: Kind) -> BuildOp {
    BuildOp::SetWall {
        tile,
        panel: Some(panel),
        kind,
    }
}

/// Each row: its name, and the operations that make it, in cubes of half a metre. Laid
/// in `kind`, with rows mixing in `stone` where there is a stone kit (D179).
fn rows(kind: Kind, stone: Option<Kind>) -> Vec<(&'static str, Vec<BuildOp>)> {
    let cabin = {
        let mut ops = vec![BuildOp::AddRoom {
            min: [0, 0, 0],
            max: [9, 0, 7],
            kind,
        }];
        // A door a metre wide in the south wall, a window beside it, and one in the east
        // wall.
        ops.extend((3..=4).map(|x| wall([x, 0, 7], Panel::Door, kind)));
        ops.extend((6..=7).map(|x| wall([x, 0, 7], Panel::WINDOW, kind)));
        ops.extend((2..=4).map(|z| wall([9, 0, z], Panel::WINDOW, kind)));
        ops.push(BuildOp::AddRoof(RoofSection {
            min: [0, 0, 0],
            max: [9, 0, 7],
            style: RoofStyle::Gable,
            along_x: true,
            pitch: 0.7,
        }));
        ops
    };
    // A room five metres by four under a roof of a style, with a door.
    let roofed = |style: RoofStyle, along_x: bool| {
        let mut ops = vec![BuildOp::AddRoom {
            min: [0, 0, 0],
            max: [9, 0, 7],
            kind,
        }];
        ops.extend((4..=5).map(|x| wall([x, 0, 7], Panel::Door, kind)));
        ops.push(BuildOp::AddRoof(RoofSection {
            min: [0, 0, 0],
            max: [9, 0, 7],
            style,
            along_x,
            pitch: if style == RoofStyle::LeanTo { 0.4 } else { 0.7 },
        }));
        ops
    };
    // A wall with a hole carved in it, a cube at a time: a cross, and a slot.
    let carved = {
        let mut ops: Vec<BuildOp> = east_west(0, 9, 0, kind).collect();
        let open = |x: i32, layer: i32| BuildOp::Fill {
            min: [x, layer, 0],
            max: [x, layer, 0],
            cube: Some(Cube {
                kind,
                fill: Fill::Open,
            }),
        };
        ops.extend([open(2, 3), open(3, 2), open(3, 3), open(3, 4), open(4, 3)]);
        ops.extend((6..=8).map(|x| open(x, 1)));
        ops
    };
    let mut mixed = Vec::new();
    if let Some(stone) = stone {
        // A house all of stone under a hip roof, with a door and windows.
        let mut house = vec![BuildOp::AddRoom {
            min: [0, 0, 0],
            max: [9, 0, 7],
            kind: stone,
        }];
        house.extend((4..=5).map(|x| wall([x, 0, 7], Panel::Door, stone)));
        house.extend(
            (1..=2)
                .chain(7..=8)
                .map(|x| wall([x, 0, 7], Panel::WINDOW, stone)),
        );
        house.extend((3..=4).map(|z| wall([0, 0, z], Panel::WINDOW, stone)));
        house.push(BuildOp::AddRoof(RoofSection {
            min: [0, 0, 0],
            max: [9, 0, 7],
            style: RoofStyle::Hip,
            along_x: true,
            pitch: 0.7,
        }));
        // The cabin on a stone plinth a metre high: one wall over another.
        let mut plinth = vec![BuildOp::AddRoom {
            min: [0, 0, 0],
            max: [9, 0, 7],
            kind,
        }];
        let stone_cube = Some(Cube {
            kind: stone,
            fill: Fill::Solid,
        });
        plinth.extend((6..=7).map(|x| wall([x, 0, 7], Panel::WINDOW, kind)));
        plinth.extend((2..=4).map(|z| wall([9, 0, z], Panel::WINDOW, kind)));
        // After the windows, which set their whole column.
        for (min, max) in [
            ([0, 0, 0], [9, 1, 0]),
            ([0, 0, 7], [9, 1, 7]),
            ([0, 0, 0], [0, 1, 7]),
            ([9, 0, 0], [9, 1, 7]),
        ] {
            plinth.push(BuildOp::Fill {
                min,
                max,
                cube: stone_cube,
            });
        }
        plinth.push(BuildOp::AddRoof(RoofSection {
            min: [0, 0, 0],
            max: [9, 0, 7],
            style: RoofStyle::Gable,
            along_x: true,
            pitch: 0.7,
        }));
        // Stone and logs meeting every way walls meet.
        let column = |x: i32, z: i32| BuildOp::Fill {
            min: [x, 0, z],
            max: [x, STOREY_CUBES - 1, z],
            cube: stone_cube,
        };
        let ell: Vec<BuildOp> = east_west(0, 5, 0, kind)
            .chain(north_south(1, 5, 0, stone))
            .collect();
        let tee_stone: Vec<BuildOp> = east_west(0, 9, 0, stone)
            .chain(north_south(1, 5, 4, kind))
            .collect();
        let tee_logs: Vec<BuildOp> = east_west(0, 9, 0, kind)
            .chain(north_south(1, 5, 4, stone))
            .collect();
        let in_line: Vec<BuildOp> = east_west(0, 4, 0, kind)
            .chain(east_west(5, 9, 0, stone))
            .collect();
        let mut pillars: Vec<BuildOp> = east_west(0, 11, 0, kind).collect();
        pillars.extend([0, 4, 8, 11].map(|x| column(x, 0)));
        // A log cabin with its east end, under the gable, of stone, as a chimney end.
        let mut end = vec![BuildOp::AddRoom {
            min: [0, 0, 0],
            max: [9, 0, 7],
            kind,
        }];
        end.extend((3..=4).map(|x| wall([x, 0, 7], Panel::Door, kind)));
        end.extend((0..=7).map(|z| column(9, z)));
        end.push(BuildOp::AddRoof(RoofSection {
            min: [0, 0, 0],
            max: [9, 0, 7],
            style: RoofStyle::Gable,
            along_x: true,
            pitch: 0.7,
        }));
        mixed = vec![
            ("stone house", house),
            ("stone plinth", plinth),
            ("stone and logs: L", ell),
            ("stone and logs: T, stone through", tee_stone),
            ("stone and logs: T, logs through", tee_logs),
            ("stone and logs: in line", in_line),
            ("stone pillars in logs", pillars),
            ("stone end", end),
        ];
    }
    let mut rows = vec![
        ("free end", east_west(0, 5, 0, kind).collect()),
        (
            "corner",
            east_west(0, 5, 0, kind)
                .chain(north_south(1, 5, 0, kind))
                .collect(),
        ),
        (
            "T",
            east_west(0, 9, 0, kind)
                .chain(north_south(1, 5, 4, kind))
                .collect(),
        ),
        (
            "cross",
            east_west(0, 9, 4, kind)
                .chain(north_south(0, 9, 4, kind))
                .collect(),
        ),
        ("long wall", east_west(0, 19, 0, kind).collect()),
        (
            "posts",
            [0, 3, 6]
                .map(|x| wall([x, 0, 0], Panel::Solid, kind))
                .to_vec(),
        ),
        ("carved", carved),
        ("cabin", cabin),
        ("hip roof", roofed(RoofStyle::Hip, true)),
        ("lean-to", roofed(RoofStyle::LeanTo, true)),
    ];
    rows.extend(mixed);
    rows
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
    assets: Res<AssetServer>,
) {
    if *done || !options.kit_sheet {
        return;
    }
    let (Some(site), Some(look)) = (site, look) else {
        return;
    };
    *done = true;
    let Some(kits) = read_kits().map(Arc::new) else {
        return;
    };
    let laying = kits.0.first().map_or(Kind::PLAIN, |kit| kit.kind());
    let stone = kits
        .0
        .iter()
        .find(|kit| kit.pattern == Pattern::Masonry)
        .map(|kit| kit.kind());
    // How high the ground is at a place, in the start's own space, the yard being round:
    // what a vantage near it needs to stand on it.
    let ground_at = |east: f32, north: f32| {
        let point = site.frame.transform_point(Vec3::new(east, 0.0, north));
        let ground = point - point.normalize() * planet.distance(point);
        (
            ground,
            site.frame
                .compute_affine()
                .inverse()
                .transform_point3(ground)
                .y,
        )
    };
    let rows = rows(laying, stone);
    let count = rows.len();
    for (row, (name, ops)) in rows.into_iter().enumerate() {
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
            build.kits = Some(kits.clone());
            build.shown = shown;
            build.replace_with(&ops, &mut commands, &planet, &mut meshes);
            commands.spawn((Name::new(label), Built(build)));
        }
        let (_, height) = ground_at(STATE_M, north);
        info!(
            "Kit sheet: {name}: --vantage {},{},{},{},{},{}",
            STATE_M + 2.0,
            height + 9.0,
            north + 22.0,
            STATE_M + 2.0,
            height + 0.5,
            north + 1.5
        );
    }
    // Models to judge beside the sheet, each in a row of its own after it.
    for (index, path) in options.models.iter().enumerate() {
        let (east, north) = (0.0, -FIRST_ROW_M - ROW_M * (count + index) as f32);
        let (ground, height) = ground_at(east, north);
        commands.spawn((
            Name::new(format!("Kit sheet: model {path}")),
            frame_at(ground, &site.frame),
            Visibility::default(),
            bevy::world_serialization::WorldAssetRoot(
                assets.load(GltfAssetLabel::Scene(0).from_asset(path.clone())),
            ),
        ));
        info!(
            "Kit sheet: model {path}: --vantage {},{},{},{},{},{}",
            east + 2.0,
            height + 2.2,
            north + 7.0,
            east + 2.0,
            height + 1.0,
            north
        );
    }
}
