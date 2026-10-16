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
//! A material's preview (D349), every role and shape of its pattern in one place, is laid
//! only when asked for by name, `--sheet-rows "<name>: preview"`, alone and west of the
//! start, where `tools/kits/preview.ts` pictures it.
//!
//! The kit's file is read again when it is saved, so the sheet can be left running while
//! the look is tuned.

use super::{Build, BuildLook, Built, Shown, Site, frame_at, lifted, read_kits};
use crate::planet::Planet;
use crate::shell::Options;
use crate::start::FLOOR_CLEARANCE_M;
use bevy::prelude::*;
use std::sync::Arc;
use terra_build::kit::Kits;
use terra_build::ops::Op;
use terra_build::scenes::sheet::{acted_rows, placed, preview};
use terra_build::{Base, BuildOp, Kind};
use terra_core::Operation;

/// How far apart the rows are, and the three states along each, in metres.
const ROW_M: f32 = 14.0;
const STATE_M: f32 = 11.0;
/// The first row's distance north of the start.
const FIRST_ROW_M: f32 = 14.0;
/// How far a going-up row has got.
const GOING_UP: f32 = 0.45;
/// How far a roof row going up has got: its walls up, its frame and battens on and its
/// covering going on, most of a roof's pieces being its covering.
const ROOF_GOING_UP: f32 = 0.72;

/// The ground under the yard cut away into a slope falling north from a row's south edge,
/// a metre for every four, as a cabin on a hillside stands.
fn slope(frame: &Transform) -> Operation {
    let fall = libm::atanf(0.25);
    let (half, south) = (Vec3::new(60.0, 6.0, 8.0), 4.6);
    let turn = Quat::from_rotation_x(-fall);
    // What is over the slope is dug: the box's underside is the slope, its south edge on
    // the ground at the row's south edge.
    let edge = Vec3::new(0.0, 0.0, south);
    let centre = edge - turn * Vec3::new(0.0, -half.y, half.z);
    Operation::DigBox {
        centre: frame.transform_point(centre),
        half,
        rotation: frame.rotation * turn,
    }
}

/// How far east of the walls' rows each kind's roof rows are, a kind after another.
const ROOF_GROUP_M: f32 = 40.0;

/// Where a material's preview is laid, in metres east and south of the start: west of the
/// sheet's rows, the same for every material, so the views `tools/kits/preview.ts` takes of
/// it are fixed (its `ORIGIN`).
const PREVIEW: [f32; 2] = [-40.0, 0.0];

/// Where a base laid at a place on the sheet stands: on the ground there, the yard being
/// round, lifted onto its floor if it has one, else on footings in the ground.
fn base_frame(at: Vec3, site: &Site, planet: &Planet, ops: &[BuildOp]) -> Transform {
    let point = site.frame.transform_point(at);
    let ground = point - point.normalize() * planet.distance(point);
    let mut base = Base::default();
    for &op in ops {
        base.apply(op);
    }
    let frame = frame_at(ground + ground.normalize() * FLOOR_CLEARANCE_M, &site.frame);
    if base.floors().next().is_some() {
        lifted(frame, &base, planet)
    } else {
        frame
    }
}

/// Does `acts` to a laid base, as a player's operations would, and lights what is marked to
/// be carved out and not yet begun, or changing and not yet in, as the next work.
fn act(build: &mut Build, acts: &[Op], rules: &terra_build::Rules, name: &str) {
    let cx = terra_build::ops::Context::free(rules);
    if let Err(why) = build.base.act_all(acts, &cx) {
        warn!("Kit sheet: {name}: {why:?}");
    }
    build.next = build
        .base
        .cubes()
        .filter(|(_, cube)| {
            (cube.pending == terra_build::Pending::Carve && cube.is_built())
                || (cube.pending.changing() && !cube.is_built())
        })
        .map(|(at, _)| at.to_array())
        .collect();
    build.revision += 1;
}

/// Where a gable end row's west gable is seen from straight on, and what is looked at, in
/// the house's own metres: eight metres out, level with the middle of the gable.
const GABLE_END_VIEW: [Vec3; 2] = [Vec3::new(-8.0, 3.6, 2.0), Vec3::new(0.0, 3.6, 2.0)];

/// What the sheet lays its rows with.
struct Laying<'a, 'w, 's, 'm> {
    commands: &'a mut Commands<'w, 's>,
    meshes: &'a mut ResMut<'m, Assets<Mesh>>,
    site: &'a Site,
    look: &'a BuildLook,
    kits: &'a Arc<Kits>,
}

impl Laying<'_, '_, '_, '_> {
    /// A base standing at `frame`, named `label`, drawn in the sheet's kits.
    fn put(&mut self, frame: Transform, label: &str) -> Build {
        let mut build = Build::new(self.commands, self.look, frame, label);
        build.kits = Some(self.kits.clone());
        build
    }
}

/// How high the ground is at a place, in the start's own space, the yard being round: what
/// a vantage near it needs to stand on it.
fn ground_at(site: &Site, planet: &Planet, east: f32, north: f32) -> (Vec3, f32) {
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
}

/// Lays the sheet once the site is there.
#[allow(clippy::too_many_arguments)]
pub(super) fn lay(
    mut done: Local<bool>,
    options: Res<Options>,
    site: Option<Res<Site>>,
    look: Option<Res<BuildLook>>,
    mut planet: ResMut<Planet>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    assets: Res<AssetServer>,
    rules: Option<Res<super::act::BuildRules>>,
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
    let stone = kits.footing().map(|kit| kit.kind());
    let names = super::kit_names();
    let (placed, count) = placed(&kits, &names);
    let mut sheet = Laying {
        commands: &mut commands,
        meshes: &mut meshes,
        site: &site,
        look: &look,
        kits: &kits,
    };
    for row in placed {
        lay_row(&mut sheet, &planet, &options.sheet_rows, row);
    }
    // Rows of cubes in states of their own, after them.
    let roof = |walls: Kind| kits.roof_over(walls);
    let acted = stone
        .map(|stone| acted_rows(laying, stone, &roof))
        .unwrap_or_default();
    let rules = rules.map(|rules| rules.rules.clone()).unwrap_or_default();
    let wanted = &options.sheet_rows;
    for (index, row) in acted.iter().enumerate() {
        if !wanted.is_empty() && !wanted.iter().any(|text| row.name.contains(text.as_str())) {
            continue;
        }
        let north = -FIRST_ROW_M - ROW_M * (count + index) as f32;
        lay_acted(&mut sheet, &mut planet, (&options, &rules), row, north);
    }
    lay_preview(&mut sheet, &planet, (wanted, &rules), &names, stone);
    let count = count + acted.len();
    // Models to judge beside the sheet, each in a row of its own after it.
    for (index, path) in options.models.iter().enumerate() {
        let (east, north) = (0.0, -FIRST_ROW_M - ROW_M * (count + index) as f32);
        let (ground, height) = ground_at(&site, &planet, east, north);
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

/// A row laid, if it is one of those asked for, in its three states along it: built, going
/// up and a blueprint, each where it always is; its vantage logged.
fn lay_row(
    sheet: &mut Laying,
    planet: &Planet,
    wanted: &[String],
    (name, ops, group, row): terra_build::scenes::sheet::Placed,
) {
    // Only the rows asked for, each where it always is.
    if !wanted.is_empty() && !wanted.iter().any(|text| name.contains(text.as_str())) {
        return;
    }
    let site = sheet.site;
    let group_east = ROOF_GROUP_M * group as f32;
    let north = -FIRST_ROW_M - ROW_M * row as f32;
    for (column, shown) in [
        Shown::Built,
        Shown::Plan(if group_east > 0.0 {
            ROOF_GOING_UP
        } else {
            GOING_UP
        }),
        Shown::Plan(0.0),
    ]
    .into_iter()
    .enumerate()
    {
        let east = group_east + STATE_M * column as f32;
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
            lifted(frame, &base, planet)
        } else {
            frame
        };
        // A gable end's own vantage, straight on to the built house's gable, through its
        // frame, as the yard is round.
        if column == 0 && name.starts_with("gable end:") {
            let start = site.frame.compute_affine().inverse();
            let [eye, at] =
                GABLE_END_VIEW.map(|p| start.transform_point3(frame.transform_point(p)));
            info!(
                "Kit sheet: {name}: straight on: --vantage {},{},{},{},{},{}",
                eye.x, eye.y, eye.z, at.x, at.y, at.z
            );
        }
        let label = format!("Kit sheet: {name}, {shown:?}");
        let mut build = sheet.put(frame, &label);
        build.shown = shown;
        build.replace_with(&ops, sheet.commands, planet, sheet.meshes);
        sheet.commands.spawn((Name::new(label), Built(build)));
    }
    let (_, height) = ground_at(site, planet, group_east + STATE_M, north);
    info!(
        "Kit sheet: {name}: --vantage {},{},{},{},{},{}",
        group_east + STATE_M + 2.0,
        height + 9.0,
        north + 22.0,
        group_east + STATE_M + 2.0,
        height + 0.5,
        north + 1.5
    );
}

/// A row whose columns are each a state its cubes are in, `north` of the start, on ground
/// sloping away where it asks for it on the yard; its vantage logged.
fn lay_acted(
    sheet: &mut Laying,
    planet: &mut ResMut<Planet>,
    (options, rules): (&Options, &terra_build::Rules),
    row: &terra_build::scenes::sheet::Acted,
    north: f32,
) {
    let site = sheet.site;
    let row_frame = frame_at(
        {
            let point = site.frame.transform_point(Vec3::new(0.0, 0.0, north));
            point - point.normalize() * planet.distance(point)
        },
        &site.frame,
    );
    if row.sloped && options.yard() {
        planet.apply(&slope(&row_frame));
    }
    for (column, acts) in row.columns.iter().enumerate() {
        let label = format!("Kit sheet: {}, {column}", row.name);
        let at = Vec3::new(STATE_M * column as f32, 0.0, north);
        let frame = base_frame(at, site, planet, &row.ops);
        let mut build = sheet.put(frame, &label);
        build.by_cubes = true;
        build.replace_with(&row.ops, sheet.commands, planet, sheet.meshes);
        act(&mut build, acts, rules, row.name);
        sheet.commands.spawn((Name::new(label), Built(build)));
    }
    let (_, height) = ground_at(site, planet, STATE_M, north);
    let corner = Vec3::new(STATE_M, height, north);
    let (eye, at) = (corner + row.view[0], corner + row.view[1]);
    info!(
        "Kit sheet: {}: --vantage {},{},{},{},{},{}",
        row.name, eye.x, eye.y, eye.z, at.x, at.y, at.z
    );
}

/// A material's preview (D349), only when asked for by name (`<name>: preview`), laid alone
/// where `tools/kits/preview.ts` looks; the first asked for, as all are laid in one place.
fn lay_preview(
    sheet: &mut Laying,
    planet: &Planet,
    (wanted, rules): (&[String], &terra_build::Rules),
    names: &std::collections::BTreeMap<Kind, String>,
    stone: Option<Kind>,
) {
    let kits = sheet.kits.clone();
    let asked = |name: &str| {
        let row = format!("{name}: preview");
        wanted
            .iter()
            .any(|text| text.contains("preview") && row.contains(text.as_str()))
    };
    let previewed = kits.0.iter().find_map(|kit| {
        let name = names.get(&kit.kind())?;
        asked(name).then_some((name.clone(), kit))
    });
    let (Some((name, kit)), Some(stone)) = (previewed, stone) else {
        return;
    };
    for (part, (ops, acts, aside)) in preview(kit, stone, kits.roof_over(kit.kind()), rules)
        .into_iter()
        .enumerate()
    {
        let label = format!("Kit sheet: {name}: preview, {part}");
        let at = Vec3::new(PREVIEW[0] + aside, 0.0, PREVIEW[1]);
        let frame = base_frame(at, sheet.site, planet, &ops);
        let mut build = sheet.put(frame, &label);
        build.by_cubes = true;
        build.replace_with(&ops, sheet.commands, planet, sheet.meshes);
        act(&mut build, &acts, rules, &label);
        sheet.commands.spawn((Name::new(label), Built(build)));
    }
    info!(
        "Kit sheet: {name}: preview at {},{} (tools/kits/preview.ts)",
        PREVIEW[0], PREVIEW[1]
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_build::kit::Kit;

    /// Each base of a material's preview, laid and acted on: what a refused act leaves, as
    /// a refusal undoes every act and the sheet would show the material only whole.
    fn previewed(kit: &Kit, stone: Kind, roof: Kind, rules: &terra_build::Rules) -> Vec<Base> {
        preview(kit, stone, roof, rules)
            .iter()
            .map(|(ops, acts, _)| {
                let mut base = Base::default();
                for &op in ops {
                    base.apply(op);
                }
                if let Err(why) = base.act_all(acts, &terra_build::ops::Context::free(rules)) {
                    panic!("material {}: {why:?}", kit.id);
                }
                base
            })
            .collect()
    }

    #[test]
    fn every_materials_preview_is_carved_damaged_and_part_built() {
        let kits = read_kits().expect("the shipped materials");
        let stone = kits
            .footing()
            .map(|kit| kit.kind())
            .expect("a footing kind");
        let rules = kits.rules();
        for kit in &kits.0 {
            let roof = kits.roof_over(kit.kind());
            let bases = previewed(kit, stone, roof, &rules);
            assert!(
                bases.iter().any(|base| base.damaged().next().is_some()),
                "material {}: nothing damaged",
                kit.id
            );
            assert!(
                bases.iter().any(|base| base
                    .cubes()
                    .any(|(_, cube)| cube.kind == kit.kind() && !cube.is_built())),
                "material {}: nothing part-built",
                kit.id
            );
        }
    }
}
