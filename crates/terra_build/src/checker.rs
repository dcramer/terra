//! The kit checker (docs/building-plan.md, the art pipeline): what `cargo xtask kits` and
//! `cargo xtask check` say of the kit files in `assets/data/kits/`. Every material a kit's
//! pattern needs is there, with every map its textures need; its sizes are life size and its
//! numbers in range; and what it draws keeps to the style guide's budgets (D14,
//! docs/art-style.md), measured on a cabin under a roof laid in it.
//!
//! It reads no files itself: it is given whether a file under `assets/` is there, and the
//! items there are, so it runs as a test and as a tool alike.

use crate::kit::{Kit, Pattern};
use crate::layout::{self, Piece};
use crate::mesh::Material;
use crate::{Base, BuildOp, CELL_M, Kind, RoofSection, RoofStyle, Surfaces};
use std::collections::BTreeMap;

/// The style guide's budgets for what kits draw (docs/art-style.md, Budgets): triangles in
/// a metre of a long member, and in a square metre of roof, its covering, battens, boards
/// and caps together.
pub const MEMBER_PER_METRE: usize = 700;
pub const ROOF_PER_SQUARE_METRE: usize = 500;

/// What a kit drew on the checker's cabin: triangles a metre of its long members, and a
/// square metre of its roof.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Measured {
    pub per_metre: Option<usize>,
    pub per_square_metre: usize,
    pub triangles: usize,
}

/// What the checker found of a kit: its problems, none if it passes, and what it measured.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Report {
    pub problems: Vec<String>,
    pub measured: Measured,
}

/// The materials a kit laying in a pattern cannot do without: what its walls, their trims,
/// its roofs' frame and covering and their flashing are drawn in.
pub fn needs(kit: &Kit) -> Vec<Material> {
    let mut needs = match kit.pattern {
        Pattern::Logs => vec![Material::Log, Material::LogEnd, Material::Board],
        Pattern::Masonry => vec![Material::Masonry, Material::Log, Material::LogEnd],
    };
    if kit.pattern == Pattern::Logs && kit.chinking {
        needs.push(Material::Daub);
    }
    needs.extend([Material::Board, Material::Covering, Material::Flashing]);
    needs.dedup();
    needs
}

/// Checks a kit: `exists` says whether a file under `assets/` is there, `items` names the
/// items there are.
pub fn check(kit: &Kit, exists: &dyn Fn(&str) -> bool, items: &[String]) -> Report {
    let mut problems = Vec::new();
    let mut fail = |problem: String| problems.push(problem);
    if kit.kind() == Kind::PLAIN || kit.kind() > Kind::LAST {
        fail(format!("its kind {} is not a kit's", kit.id));
    }
    // Every material its pattern needs, and every map of each.
    for material in needs(kit) {
        if !kit.textures.contains_key(&material) {
            fail(format!(
                "no {material:?} material, which its walls or roofs need"
            ));
        }
    }
    for (material, texture) in &kit.textures {
        let mut maps = vec!["color.jpg", "normal.jpg", "arm.jpg"];
        let relief = match (kit.pattern, material) {
            (Pattern::Logs, Material::Log) => kit.timber.relief > 0.0,
            (Pattern::Masonry, Material::Masonry) => kit.relief > 0.0,
            _ => false,
        };
        if texture.depth > 0.0 || relief {
            maps.push("height.jpg");
        }
        for map in maps {
            if !exists(&format!("{}/{map}", texture.path)) {
                fail(format!("{material:?}: no {}/{map}", texture.path));
            }
        }
        if !texture.metres.iter().all(|m| m.is_finite() && *m > 0.0) {
            fail(format!("{material:?}: a texture no metres across"));
        }
        if !(0.0..=1.0).contains(&texture.reflectance) {
            fail(format!("{material:?}: reflectance out of 0 to 1"));
        }
    }
    for model in [&kit.models.door, &kit.models.window].into_iter().flatten() {
        if !exists(model) {
            fail(format!("no {model}"));
        }
    }
    // Life size, and numbers in range (D14: lightly stylised, true proportions).
    let t = &kit.timber;
    let r = &kit.roof;
    let ranges: [(&str, f32, f32, f32); 14] = [
        ("diameter", kit.diameter, 0.12, CELL_M),
        ("eaves", kit.eaves, 0.0, 1.0),
        ("frame", kit.frame, 0.0, 0.2),
        ("bevel", kit.bevel, 0.0, CELL_M * 0.5),
        ("relief", kit.relief, 0.0, 0.12),
        ("timber taper", t.taper, 0.0, 0.5),
        ("timber uneven", t.uneven, 0.0, 0.3),
        ("timber relief", t.relief, 0.0, 0.08),
        ("roof butt", r.butt, 0.0, 0.1),
        ("roof thickness", r.thick, 0.005, 0.4),
        ("rafters", r.rafter.1, 0.06, 0.3),
        ("boards' depth", r.board.0, 0.05, 0.4),
        ("cap", r.cap, 0.04, 0.5),
        ("courses a texture", r.rows, 1.0, 40.0),
    ];
    for (what, value, low, high) in ranges {
        if !value.is_finite() || value < low || value > high {
            fail(format!("{what} {value} is not from {low} to {high} m"));
        }
    }
    if kit.pattern == Pattern::Logs {
        let (count, whole) = kit.courses();
        if !whole || count == 0 {
            fail(format!(
                "a course of {} m does not divide the storey",
                kit.course
            ));
        }
        if kit.course > kit.diameter {
            fail("its courses are further apart than its logs are thick".into());
        }
        if kit.longest < CELL_M {
            fail(format!(
                "its longest log, {} m, is shorter than a cube",
                kit.longest
            ));
        }
        if t.sides < 6 || t.step <= 0.01 {
            fail("its logs too coarse to be round".into());
        }
        if !(t.daub.0 > 0.0 && t.daub.0 < 1.0 && t.daub.1 > 0.0) {
            fail("its daub out of its groove".into());
        }
        let v = &t.variation;
        if v.thickness >= 0.2 || v.shade >= 0.5 || v.warmth >= 0.2 {
            fail("its logs vary more than a tree's do".into());
        }
    }
    let c = &kit.colours;
    let colours = [
        c.light, c.dark, c.roof, c.chinking, t.tint, t.end_tint, r.tint,
    ];
    if !colours.iter().flatten().all(|v| (0.0..=1.0).contains(v)) {
        fail("a colour out of 0 to 1".into());
    }
    // What it costs, in items there are and amounts a base may keep.
    let steps = kit.cost.steps;
    if steps == 0 || !crate::cubes::FULL.is_multiple_of(steps) {
        fail(format!("{steps} steps do not fill a cube"));
    }
    for (thing, num, den) in kit.cost.step.iter().chain(&kit.cost.roof) {
        if !items.iter().any(|item| item == thing) {
            fail(format!("costs {thing}, which is no item"));
        }
        if *den == 0 || !crate::cost::Amount::new(*num, *den).in_bounds() {
            fail(format!(
                "costs {num}/{den} of {thing}, which a base cannot keep"
            ));
        }
    }
    // Its budgets, on a cabin laid in it.
    let measured = measure(kit);
    if measured.per_metre.is_some_and(|n| n > MEMBER_PER_METRE) {
        fail(format!(
            "{} triangles a metre of log, over the {MEMBER_PER_METRE} the style guide allows",
            measured.per_metre.unwrap_or(0)
        ));
    }
    if measured.per_square_metre > ROOF_PER_SQUARE_METRE {
        fail(format!(
            "{} triangles a square metre of roof, over the {ROOF_PER_SQUARE_METRE} the style guide allows",
            measured.per_square_metre
        ));
    }
    Report { problems, measured }
}

/// Checks every kit there is, by its file's name: each as [`check`], and no two laying the
/// same kind.
pub fn check_all(
    kits: &[(String, Kit)],
    exists: &dyn Fn(&str) -> bool,
    items: &[String],
) -> BTreeMap<String, Report> {
    let mut reports = BTreeMap::new();
    let mut kinds: BTreeMap<u16, &str> = BTreeMap::new();
    for (name, kit) in kits {
        let mut report = check(kit, exists, items);
        if let Some(other) = kinds.insert(kit.id, name) {
            report
                .problems
                .push(format!("lays kind {}, as {other} does", kit.id));
        }
        reports.insert(name.clone(), report);
    }
    reports
}

/// What a kit draws on a cabin five metres by four under a gable roof, with a door and a
/// window: triangles a metre of its long members and a square metre of its roof.
pub fn measure(kit: &Kit) -> Measured {
    let mut base = Base::default();
    base.apply(BuildOp::AddRoom {
        min: [0, 0, 0],
        max: [9, 0, 7],
        kind: kit.kind(),
    });
    for x in 3..=4 {
        base.apply(BuildOp::SetWall {
            tile: [x, 0, 7],
            panel: Some(crate::Panel::Door),
            kind: kit.kind(),
        });
    }
    base.apply(BuildOp::SetWall {
        tile: [0, 0, 3],
        panel: Some(crate::Panel::WINDOW),
        kind: kit.kind(),
    });
    base.apply(BuildOp::AddRoof(RoofSection {
        min: [0, 0, 0],
        max: [9, 0, 7],
        style: RoofStyle::Gable,
        along_x: true,
        pitch: 0.7,
    }));
    let layout = layout::lay(&base, kit);
    let (mut logs, mut length, mut roof, mut squares, mut all) = (0, 0.0, 0, 0, 0);
    for piece in &layout.pieces {
        let mut surfaces = Surfaces::default();
        crate::draw::draw(piece, kit, None, &mut surfaces);
        let count: usize = surfaces.0.values().map(|s| s.triangle_count()).sum();
        all += count;
        match *piece {
            Piece::Log { from, to, .. } => {
                logs += count;
                length += from.metres().distance(to.metres());
            }
            Piece::Covering { .. } => {
                roof += count;
                squares += 1;
            }
            Piece::Roof { part, .. } if part != layout::roofs::RoofPart::Rafter => {
                roof += count;
            }
            _ => {}
        }
    }
    let area = squares as f32 * layout::roofs::STEP_M * layout::roofs::STEP_M;
    Measured {
        per_metre: (length > 0.0).then(|| (logs as f32 / length).round() as usize),
        per_square_metre: (roof as f32 / area.max(1e-3)).round() as usize,
        triangles: all,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shipped() -> (Vec<(String, Kit)>, Vec<String>) {
        let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        #[derive(serde::Deserialize)]
        struct Items {
            kinds: Vec<Item>,
        }
        #[derive(serde::Deserialize)]
        struct Item {
            name: String,
        }
        let items = std::fs::read_to_string(assets.join("data/items.ron")).unwrap();
        let items: Items = ron::from_str(&items).unwrap();
        let mut kits = Vec::new();
        for entry in std::fs::read_dir(assets.join("data/kits")).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|e| e == "ron") {
                let text = std::fs::read_to_string(&path).unwrap();
                let kit: Kit =
                    ron::from_str(&text).unwrap_or_else(|why| panic!("{}: {why}", path.display()));
                let name = path.file_name().unwrap().to_string_lossy().to_string();
                kits.push((name, kit));
            }
        }
        kits.sort_by(|a, b| a.0.cmp(&b.0));
        (
            kits,
            items.kinds.into_iter().map(|item| item.name).collect(),
        )
    }

    #[test]
    fn every_shipped_kit_passes_the_checker() {
        let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        let (kits, items) = shipped();
        assert!(kits.len() >= 3, "{kits:?}");
        let exists = |path: &str| assets.join(path).is_file();
        for (name, report) in check_all(&kits, &exists, &items) {
            assert!(report.problems.is_empty(), "{name}: {:?}", report.problems);
            assert!(report.measured.per_square_metre > 0, "{name}: no roof");
        }
    }

    #[test]
    fn a_kit_missing_what_it_needs_or_out_of_size_is_refused() {
        let (kits, items) = shipped();
        let mut kit = kits
            .iter()
            .find(|(name, _)| name == "logs.ron")
            .map(|(_, kit)| kit.clone())
            .unwrap();
        kit.textures.remove(&Material::Covering);
        kit.diameter = 2.0;
        kit.cost.step = vec![("Gold".into(), 1, 3)];
        let report = check(&kit, &|_| true, &items);
        let said = report.problems.join("; ");
        for words in [
            "no Covering material",
            "diameter 2",
            "Gold, which is no item",
        ] {
            assert!(said.contains(words), "{said}");
        }
        // Two kits of one kind.
        let twice = [("a".to_string(), kit.clone()), ("b".to_string(), kit)];
        let reports = check_all(&twice, &|_| true, &items);
        assert!(
            reports["b"]
                .problems
                .iter()
                .any(|p| p.contains("as a does"))
        );
    }
}
