//! The scene registry: every base the game ships or the checks look at, registered once,
//! by name and family. The mesh checker (`cargo xtask artifacts`), its ratchet's two sets,
//! the drawing audit (`cargo xtask rules`), the texel check and the voxel contract's cases
//! (docs/building-voxels.md) all take their bases from here, so a scene added here is
//! checked with nothing else to do, and a scene the game lays is added here by laying it
//! from a function this registers (the kit sheet's rows are [`super::sheet::placed`]).
//!
//! A scene is named `<kits>: <what>`, its kits by their files' names ("logs", "logs with
//! stone"), or its family where it is of many or stands apart ("sheet", "village", "preview",
//! "burg"); its base is built only when asked for, on ground falling across it and on its
//! footings.

use glam::IVec3;
use std::collections::BTreeMap;
use terra_build::kit::{Kit, Kits, Pattern};
use terra_build::{Base, BuildOp, Kind};
use terra_town::village::Palette;

/// What the scenes are built of: the kits in scope, their files' names, and the settlement's
/// palette where there is one, which the village's buildings are built of.
pub struct Materials {
    pub kits: Kits,
    pub names: BTreeMap<Kind, String>,
    pub village: Option<Palette>,
}

impl Materials {
    /// Every material in an `assets` folder, or with `baseline` only the building baseline's
    /// (D333): the wall materials the settlement is built of (`data/settlement.ron`) and the
    /// roofs over them. The village's palette is read from every material either way.
    pub fn read(assets: &std::path::Path, baseline: bool) -> Result<Materials, String> {
        let all = Kit::read_folder(assets)?;
        let kits = Kits(all.iter().map(|(_, kit)| kit.clone()).collect());
        let named = |name: &str| {
            all.iter()
                .find(|(file, _)| file == name)
                .map(|(_, kit)| kit.kind())
        };
        let read = |path: &str| {
            std::fs::read_to_string(assets.join(path)).map_err(|why| format!("{path}: {why}"))
        };
        let village = Palette::read(&read, &named, &kits)?;
        let names: BTreeMap<Kind, String> = all
            .iter()
            .map(|(file, kit)| (kit.kind(), file.clone()))
            .collect();
        let kits = if baseline {
            let walls: Vec<Kind> = village.houses().collect();
            let roofs: Vec<Kind> = walls.iter().filter_map(|&k| kits.roof_for(k)).collect();
            Kits(
                kits.0
                    .iter()
                    .filter(|kit| walls.contains(&kit.kind()) || roofs.contains(&kit.kind()))
                    .cloned()
                    .collect(),
            )
        } else {
            kits
        };
        Ok(Materials {
            kits,
            names,
            village: Some(village),
        })
    }

    /// A kind's file's name, or its number where it has none.
    pub fn name(&self, kind: Kind) -> String {
        self.names
            .get(&kind)
            .cloned()
            .unwrap_or_else(|| kind.0.to_string())
    }

    fn walls(&self) -> Vec<&Kit> {
        self.kits
            .0
            .iter()
            .filter(|kit| kit.pattern != Pattern::Roof)
            .collect()
    }

    fn roofs(&self) -> Vec<&Kit> {
        self.kits
            .0
            .iter()
            .filter(|kit| kit.pattern == Pattern::Roof)
            .collect()
    }
}

/// What sort of scene a scene is, and so where it comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Family {
    /// Every roof over every plan, in each wall kind (`roof_rows`).
    Plans,
    /// Every roof over every plan, each two wall kinds mixed.
    Mixed,
    /// The QA build film's cabin (`qa_cabin`).
    QaCabin,
    /// The rectangle under a gable in every state a base is drawn in (`STATES`).
    States,
    /// The voxel contract's junctions (`junctions`).
    Junctions,
    /// Where roofs and walls meet (`roof_walls`, D343).
    RoofsAndWalls,
    /// The kit sheet's rows (`sheet::placed`, `sheet::acted_rows`).
    Sheet,
    /// A material's preview (`sheet::preview`, D349).
    Preview,
    /// One of each sort of the village's buildings (`village::sorts`).
    Village,
    /// The example base the build experiment lays (`example`).
    Example,
    /// The castle (`castle`) and the burg built to burn (`burg`).
    Castle,
}

impl Family {
    pub const ALL: [Family; 11] = [
        Family::Plans,
        Family::Mixed,
        Family::QaCabin,
        Family::States,
        Family::Junctions,
        Family::RoofsAndWalls,
        Family::Sheet,
        Family::Preview,
        Family::Village,
        Family::Example,
        Family::Castle,
    ];
}

/// Which of the mesh checker's ratchets holds a scene: before every push (`cargo xtask
/// check`, `artifacts --quick`), or only the whole set (`artifacts --ratchet`). Every scene
/// is in the whole set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gate {
    Push,
    Full,
}

/// A registered scene: its name, family and ratchet, and how to build its base.
pub struct Scene<'a> {
    pub name: String,
    pub family: Family,
    pub gate: Gate,
    build: Box<dyn Fn() -> Base + Send + Sync + 'a>,
}

impl Scene<'_> {
    /// Its base, built now.
    pub fn base(&self) -> Base {
        (self.build)()
    }
}

impl std::fmt::Debug for Scene<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({:?}, {:?})", self.name, self.family, self.gate)
    }
}

/// The scenes a ratchet inspects: before a push those gated so, else every one.
pub fn ratchet(materials: &Materials, gate: Gate) -> Vec<Scene<'_>> {
    registry(materials)
        .into_iter()
        .filter(|scene| gate == Gate::Full || scene.gate == Gate::Push)
        .collect()
}

/// Every scene, in the materials given.
pub fn registry(m: &Materials) -> Vec<Scene<'_>> {
    let kits = &m.kits;
    let walls = m.walls();
    let roofs = m.roofs();
    let roof = |walls: Kind| kits.roof_over(walls);
    let mut out: Vec<Scene> = Vec::new();
    let mut add = |name: String, family: Family, gate: Gate, ops: Vec<BuildOp>| {
        out.push(Scene {
            name,
            family,
            gate,
            build: Box::new(move || on_ground(kits, &ops)),
        });
    };
    // Every roof over every plan, in each wall kind and each two mixed; a gable first, every push.
    let first = |row: &str| {
        if row == "rectangle, gable roof" {
            Gate::Push
        } else {
            Gate::Full
        }
    };
    for kit in &walls {
        for (row, plan) in terra_build::roof_rows(&[kit.kind()], false, &roof) {
            let gate = first(&row);
            add(
                format!("{}: {row}", m.name(kit.kind())),
                Family::Plans,
                gate,
                plan,
            );
        }
    }
    let pairs: Vec<(&Kit, &Kit)> = walls
        .iter()
        .enumerate()
        .flat_map(|(i, a)| walls[i + 1..].iter().map(move |b| (*a, *b)))
        .collect();
    for &(a, b) in &pairs {
        for (row, plan) in terra_build::roof_rows(&[a.kind(), b.kind()], true, &roof) {
            let name = format!("{} with {}: {row}", m.name(a.kind()), m.name(b.kind()));
            add(name, Family::Mixed, first(&row), plan);
        }
    }
    // The voxel contract's junctions, in each wall kind under its roof, two roof kinds
    // meeting in the next roof kind there is.
    for kit in &walls {
        let own = roof(kit.kind());
        let at = roofs.iter().position(|r| r.kind() == own).unwrap_or(0);
        let other = roofs
            .get((at + 1) % roofs.len().max(1))
            .map_or(own, |r| r.kind());
        for (row, base) in terra_build::junctions::junctions(kit.kind(), own, other) {
            let name = format!("{}: junction, {row}", m.name(kit.kind()));
            add(name, Family::Junctions, Gate::Push, base);
        }
    }
    // Where roofs and walls meet (D343): every case, in each wall kind under each roof.
    for wall in &walls {
        for covering in &roofs {
            let other = super::sheet::other_pattern(kits, wall);
            for (row, base) in crate::roof_walls::rows(wall.kind(), covering.kind(), other) {
                let name = format!(
                    "{} with {}: roofs and walls: {row}",
                    m.name(wall.kind()),
                    m.name(covering.kind())
                );
                add(name, Family::RoofsAndWalls, Gate::Push, base);
            }
        }
    }
    // The kit sheet's rows as the game lays them, built; its walls' rows before every push.
    let (placed, walls_rows) = super::sheet::placed(kits, &m.names);
    for (index, (row, base, _, _)) in placed.into_iter().enumerate() {
        let gate = if index < walls_rows {
            Gate::Push
        } else {
            Gate::Full
        };
        add(format!("sheet: {row}"), Family::Sheet, gate, base);
    }
    // The example base, in the first wall kind.
    if let Some(kit) = walls.first() {
        let name = format!("example: {}", m.name(kit.kind()));
        add(
            name,
            Family::Example,
            Gate::Full,
            terra_build::example(kit.kind()),
        );
    }
    // The castle in the footings' stone with each kind of long members for its timber, and
    // the burg built to burn in the settlement's materials.
    if let Some(stone) = kits.footing() {
        for timber in walls.iter().filter(|kit| kit.pattern == Pattern::Logs) {
            let name = format!(
                "{} with {}: castle",
                m.name(stone.kind()),
                m.name(timber.kind())
            );
            let castle =
                terra_build::castle::castle(stone.kind(), timber.kind(), roof(stone.kind()));
            add(name, Family::Castle, Gate::Full, castle);
        }
    }
    if let Some(palette) = &m.village {
        let first = palette.first();
        let burg = terra_build::burg::burg(terra_build::burg::Materials {
            stone: first.masonry,
            beams: first.beams,
            logs: first.timber,
            shakes: first.timber_roof,
        });
        add(
            "burg: built to burn".into(),
            Family::Castle,
            Gate::Full,
            burg,
        );
    }
    let rules = std::sync::Arc::new(kits.rules());
    // The sheet's rows of cubes in states of their own, a scene a state, each acted on as the
    // sheet acts on it.
    if let (Some(laying), Some(stone)) = (walls.first(), kits.footing()) {
        for row in super::sheet::acted_rows(laying.kind(), stone.kind(), &roof) {
            for (column, acts) in row.columns.iter().enumerate() {
                let (plan, acts, rules) = (row.ops.clone(), acts.clone(), rules.clone());
                out.push(Scene {
                    name: format!("sheet: {}, {column}", row.name),
                    family: Family::Sheet,
                    gate: Gate::Full,
                    build: Box::new(move || {
                        let mut base = on_ground(kits, &plan);
                        let _ = base.act_all(&acts, &terra_build::ops::Context::free(&rules));
                        base
                    }),
                });
            }
        }
    }
    // Each material's preview, its parts side by side as the sheet lays them.
    if let Some(stone) = kits.footing() {
        for kit in &kits.0 {
            let parts = super::sheet::preview(kit, stone.kind(), roof(kit.kind()), &rules);
            for (part, (plan, acts, _)) in parts.into_iter().enumerate() {
                let rules = rules.clone();
                out.push(Scene {
                    name: format!("preview: {}, {part}", m.name(kit.kind())),
                    family: Family::Preview,
                    gate: Gate::Full,
                    build: Box::new(move || {
                        let mut base = on_ground(kits, &plan);
                        let _ = base.act_all(&acts, &terra_build::ops::Context::free(&rules));
                        base
                    }),
                });
            }
        }
    }
    // The QA build film's cabin as a player leaves it (`qa_cabin`): a storey over a roof,
    // a dormer, a deck, a wall changed and a window carved, in each wall kit, the wall changed
    // to the next of its pattern, and in all of them mixed; and its storey laid round the
    // first storey's roof left on (D343). On ground falling half a metre across it, as the
    // film's does where its stairs come down.
    let other = |kit: &Kit| {
        let like: Vec<&Kit> = walls
            .iter()
            .copied()
            .filter(|other| other.pattern == kit.pattern)
            .collect();
        let pool = if like.len() > 1 { like } else { walls.clone() };
        let at = pool.iter().position(|o| o.id == kit.id).unwrap_or(0);
        pool[(at + 1) % pool.len()].kind()
    };
    let mut cabin = |name: String, kinds: Vec<Kind>, other: Kind, roof_left_on: bool| {
        out.push(Scene {
            name,
            family: Family::QaCabin,
            gate: Gate::Push,
            build: Box::new(move || {
                let height = |[x, z]: [i32; 2]| (-(x as f32) * 2.0 - z as f32) as i16;
                super::qa_cabin::cabin(kits, &kinds, other, &height, roof_left_on)
            }),
        });
    };
    for kit in &walls {
        let name = m.name(kit.kind());
        cabin(
            format!("{name}: QA cabin"),
            vec![kit.kind()],
            other(kit),
            false,
        );
        cabin(
            format!("{name}: QA cabin, its storey over its roof left on"),
            vec![kit.kind()],
            other(kit),
            true,
        );
    }
    if let Some(first) = walls.first() {
        let names: Vec<String> = walls.iter().map(|kit| m.name(kit.kind())).collect();
        let kinds = walls.iter().map(|kit| kit.kind()).collect();
        cabin(
            format!("{}: QA cabin", names.join(" with ")),
            kinds,
            other(first),
            false,
        );
    }
    // The rectangle under a gable, in each wall kind and each two mixed, in every state a
    // base is drawn in (docs/building-voxels.md, gap 8).
    let mut rectangles: Vec<(String, Vec<BuildOp>)> = Vec::new();
    for kit in &walls {
        if let Some((_, plan)) = terra_build::roof_rows(&[kit.kind()], false, &roof)
            .into_iter()
            .next()
        {
            rectangles.push((m.name(kit.kind()), plan));
        }
    }
    for &(a, b) in &pairs {
        if let Some((_, plan)) = terra_build::roof_rows(&[a.kind(), b.kind()], true, &roof)
            .into_iter()
            .next()
        {
            rectangles.push((
                format!("{} with {}", m.name(a.kind()), m.name(b.kind())),
                plan,
            ));
        }
    }
    for (who, plan) in rectangles {
        let plan = std::sync::Arc::new(plan);
        for (index, state) in STATES.iter().enumerate() {
            let plan = plan.clone();
            out.push(Scene {
                name: format!("{who}: rectangle under a gable, {state}"),
                family: Family::States,
                gate: Gate::Full,
                build: Box::new(move || {
                    let finished = on_ground(kits, &plan);
                    states(kits, &finished).swap_remove(index)
                }),
            });
        }
    }
    // One of each sort of the village's buildings, in the settlement's own materials: the
    // shipped scene the game builds most of.
    if let Some(palette) = &m.village {
        for building in terra_town::village::sorts(palette) {
            out.push(Scene {
                name: format!("village: {}", building.name),
                family: Family::Village,
                gate: Gate::Push,
                build: Box::new(move || terra_town::village::standing(&building, palette)),
            });
        }
    }
    out
}

/// The states a finished base is inspected in besides finished, in order (`states`).
pub const STATES: [&str; 5] = [
    "as a plan",
    "walls going up",
    "roof going on",
    "damaged",
    "after a collapse",
];

/// A finished base in each of [`STATES`]: all of it a plan; its walls built to their
/// second course, the next half in, the rest planned; its walls built and its roof half
/// on; harmed, a wall's middle half worn through and a cube of it knocked out; and with a
/// wall's lowest course knocked out, what it alone held come down.
pub fn states(kits: &Kits, finished: &Base) -> Vec<Base> {
    let rules = kits.rules();
    let cx = terra_build::ops::Context::free(&rules);
    let refilled = |level: &dyn Fn(IVec3, &terra_build::Cube) -> u8| {
        let mut base = finished.clone();
        for (at, cube) in finished.cubes() {
            base.apply(BuildOp::Fill {
                min: at.to_array(),
                max: at.to_array(),
                cube: Some(cube.at_level(level(at, &cube))),
            });
        }
        base
    };
    let low = finished.cubes().map(|(at, _)| at.y).min().unwrap_or(0);
    let full = terra_build::cubes::FULL;
    let plan = refilled(&|_, _| 0);
    let walls = refilled(&|at, cube| {
        if cube.kind.is_roof() || at.y > low + 3 {
            0
        } else if at.y == low + 3 {
            full / 2
        } else {
            full
        }
    });
    let roof = refilled(&|_, cube| if cube.kind.is_roof() { full / 2 } else { full });
    // The wall along the north, x from 1 to 8 at z 0 (`roof_rows`' rectangle).
    let north: Vec<[i32; 3]> = finished
        .cubes()
        .filter(|(at, cube)| at.z == 0 && (1..=8).contains(&at.x) && !cube.kind.is_roof())
        .map(|(at, _)| at.to_array())
        .collect();
    let worn = |cubes: &[[i32; 3]], quarters: u16| -> Base {
        let mut base = finished.clone();
        for &at in cubes {
            let Some(cube) = base.cube(IVec3::from_array(at)) else {
                continue;
            };
            let health = rules.toughness(cube.kind).health;
            let amount = (health * quarters / 4).max(1);
            let _ = base.act(
                &terra_build::ops::Op::Damage {
                    cubes: vec![at],
                    amount,
                },
                &cx,
            );
        }
        base
    };
    let middle: Vec<[i32; 3]> = north
        .iter()
        .copied()
        .filter(|at| (3..=6).contains(&at[0]) && at[1] >= 1 && at[1] <= 3)
        .collect();
    let mut damaged = worn(&middle, 2);
    let hole = [4, 2, 0];
    if damaged.cube(IVec3::from_array(hole)).is_some() {
        let health = damaged
            .cube(IVec3::from_array(hole))
            .map_or(1, |cube| rules.toughness(cube.kind).health);
        let _ = damaged.act(
            &terra_build::ops::Op::Damage {
                cubes: vec![hole],
                amount: health,
            },
            &cx,
        );
    }
    let lowest = north
        .iter()
        .map(|at| at[1])
        .filter(|&y| y >= 0)
        .min()
        .unwrap_or(0);
    let course: Vec<[i32; 3]> = north
        .iter()
        .copied()
        .filter(|at| at[1] == lowest && (2..=7).contains(&at[0]))
        .collect();
    let collapsed = worn(&course, 4);
    vec![plan, walls, roof, damaged, collapsed]
}

/// A base of shortcuts on ground falling a metre across it, with its footings laid, as the
/// game lays them (`Build::lay_footings`).
pub fn on_ground(kits: &Kits, ops: &[BuildOp]) -> Base {
    let mut base = Base::default();
    for &op in ops {
        base.apply(op);
    }
    let height = |[x, z]: [i32; 2]| (-(x as f32) * 4.0 - z as f32 * 2.0) as i16;
    let ring = base.ground_ring();
    base.sample_ground(ring.into_iter().map(|column| terra_build::Sample {
        column,
        height_cm: height(column),
    }));
    if let Some(stone) = kits.footing() {
        base.lay_footings(stone.kind(), height);
    }
    base
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The materials the game ships, the building baseline's (D333).
    fn baseline() -> Materials {
        let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        Materials::read(&assets, true).expect("the baseline's materials")
    }

    #[test] // Slow: minutes.
    fn every_registered_scene_is_in_the_ratchet_and_every_family_before_a_push() {
        let materials = baseline();
        let all: Vec<String> = registry(&materials).into_iter().map(|s| s.name).collect();
        let full: std::collections::BTreeSet<String> = ratchet(&materials, Gate::Full)
            .into_iter()
            .map(|s| s.name)
            .collect();
        let missing: Vec<&String> = all.iter().filter(|name| !full.contains(*name)).collect();
        assert!(missing.is_empty(), "registered, not ratcheted: {missing:?}");
        // Each family is registered and some of it is held before every push, but the
        // castles, of 140,000 cubes, and the states and previews, each a finished base acted
        // on, which the whole set holds.
        let push = ratchet(&materials, Gate::Push);
        for family in Family::ALL {
            assert!(
                registry(&materials).iter().any(|s| s.family == family),
                "no scene of {family:?} is registered"
            );
            let whole_only = matches!(
                family,
                Family::Castle | Family::States | Family::Preview | Family::Example
            );
            assert_eq!(
                push.iter().any(|s| s.family == family),
                !whole_only,
                "{family:?}'s scenes before a push"
            );
        }
        // No two scenes share a name, so a name finds one scene.
        let mut seen = std::collections::BTreeSet::new();
        let twice: Vec<&String> = all.iter().filter(|name| !seen.insert(*name)).collect();
        assert!(twice.is_empty(), "registered twice: {twice:?}");
    }

    #[test]
    fn the_kit_sheet_the_game_lays_is_registered_row_for_row() {
        let materials = baseline();
        let names: std::collections::BTreeSet<String> = registry(&materials)
            .into_iter()
            .filter(|s| s.family == Family::Sheet)
            .map(|s| s.name)
            .collect();
        let (placed, _) = crate::sheet::placed(&materials.kits, &materials.names);
        assert!(!placed.is_empty());
        for (row, ..) in placed {
            assert!(names.contains(&format!("sheet: {row}")), "{row}");
        }
    }
}
