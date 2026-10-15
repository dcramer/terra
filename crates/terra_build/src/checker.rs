//! The kit checker (docs/building-plan.md, the art pipeline): what `cargo xtask kits` and
//! `cargo xtask check` say of the kit files in `assets/data/kits/`. Every material a kit's
//! pattern needs is there, with every map its textures need; its sizes are life size and its
//! numbers in range; and what it draws keeps to the style guide's budgets (D14,
//! docs/art-style.md), measured on a cabin laid in it, or for a roof kind, a gable of it
//! over one.
//!
//! It reads no files itself: it is given whether a file under `assets/` is there, and the
//! items there are, so it runs as a test and as a tool alike.

use crate::kit::{Kit, Pattern};
use crate::layout::roofs::RoofPart;
use crate::layout::{self, Piece};
use crate::mesh::Material;
use crate::{Base, BuildOp, CELL_M, Cube, Fill, Kind, Surfaces};
use std::collections::BTreeMap;

/// The style guide's budgets for what kits draw (docs/art-style.md, Budgets): triangles in
/// a metre of a long member, and in a square metre of roof, its covering, battens, boards
/// and caps together.
pub const MEMBER_PER_METRE: usize = 700;
pub const ROOF_PER_SQUARE_METRE: usize = 800;
/// A kit's dress (D215): triangles in a metre of a profile swept along an edge, its ends
/// and joints included, and in one corner stone.
pub const TRIM_PER_METRE: usize = 600;
pub const CORNER_STONE: usize = 320;
/// A module (D258): triangles in a metre of it near, and in its cap.
pub const MODULE_PER_METRE: usize = 800;
pub const MODULE_CAP: usize = 600;

/// What a kit drew on the checker's cabin: triangles a metre of its long members, and a
/// square metre of its roof.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Measured {
    pub per_metre: Option<usize>,
    pub per_square_metre: usize,
    pub triangles: usize,
    /// What its dress adds a cube of its kind on the cabin, if it dresses any: its trims,
    /// corner stones and members by role, and a framed kind's frame.
    pub dress_per_cube: Option<usize>,
}

/// What the checker found of a kit: its problems, none if it passes, and what it measured.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Report {
    pub problems: Vec<String>,
    pub measured: Measured,
}

/// The materials a kit laying in a pattern cannot do without: what its walls and their
/// trims are drawn in, or a roof's frame, covering, boards and flashing.
pub fn needs(kit: &Kit) -> Vec<Material> {
    let mut needs = match kit.pattern {
        Pattern::Logs => vec![Material::Log, Material::LogEnd, Material::Board],
        Pattern::Masonry => vec![Material::Masonry, Material::Log, Material::LogEnd],
        Pattern::Roof => vec![
            Material::Covering,
            Material::Board,
            Material::Flashing,
            Material::Log,
            Material::LogEnd,
        ],
    };
    if kit.pattern == Pattern::Logs && kit.chinking {
        needs.push(Material::Daub);
    }
    // A framed kind's frame, and its core (D236).
    if let Some(framing) = &kit.framing {
        needs.push(framing.material);
        needs.extend(framing.core);
    }
    needs.dedup();
    needs
}

/// Checks a kit: `exists` says whether a file under `assets/` is there, `items` names the
/// items there are.
pub fn check(kit: &Kit, exists: &dyn Fn(&str) -> bool, items: &[String]) -> Report {
    let mut problems = contract(kit);
    let mut fail = |problem: String| problems.push(problem);
    if kit.kind() == Kind::PLAIN || kit.kind() > Kind::LAST {
        fail(format!("its kind {} is not a kit's", kit.id));
    }
    // A roof's kind is numbered as one (D241), and only a roof's.
    if (kit.pattern == Pattern::Roof) != kit.kind().is_roof() {
        fail(format!(
            "its kind {} is {}a roof's, but it lays {:?}",
            kit.id,
            if kit.kind().is_roof() { "" } else { "not " },
            kit.pattern
        ));
    }
    if let Some(roof) = kit.roof_kind
        && !Kind(roof).is_roof()
    {
        fail(format!("its roofs' kind {roof} is not a roof's"));
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
        let cored = kit
            .framing
            .as_ref()
            .is_some_and(|framing| framing.core == Some(*material) && framing.core_relief > 0.0);
        let relief = cored
            || match (kit.pattern, material) {
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
    // Life size, and numbers in range (D14: lightly stylised, true proportions).
    let t = &kit.timber;
    let r = &kit.roof;
    let ranges: [(&str, f32, f32, f32); 12] = [
        ("diameter", kit.diameter, 0.12, CELL_M),
        ("eaves", kit.eaves, 0.0, 1.0),
        ("frame", kit.frame, 0.0, 0.2),
        ("bevel", kit.bevel, 0.0, CELL_M * 0.5),
        ("relief", kit.relief, 0.0, 0.12),
        ("timber taper", t.taper, 0.0, 0.5),
        ("timber uneven", t.uneven, 0.0, 0.3),
        ("timber relief", t.relief, 0.0, 0.08),
        ("roof thickness", r.thick, 0.005, 0.4),
        ("roof relief", r.relief, 0.0, 0.15),
        ("rafters", r.rafter, 0.06, 0.3),
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
    let colours = [c.light, c.chinking, t.tint, t.end_tint, r.tint];
    if !colours.iter().flatten().all(|v| (0.0..=1.0).contains(v)) {
        fail("a colour out of 0 to 1".into());
    }
    // What it costs, in items there are and amounts a base may keep.
    let steps = kit.cost.steps;
    if steps == 0 || !crate::cubes::FULL.is_multiple_of(steps) {
        fail(format!("{steps} steps do not fill a cube"));
    }
    if kit.pattern == Pattern::Roof && steps != crate::layout::roofs::ROOF_STAGES {
        fail(format!(
            "a roof goes up in {} stages, not {steps} steps",
            crate::layout::roofs::ROOF_STAGES
        ));
    }
    for (thing, num, den) in &kit.cost.step {
        if !items.iter().any(|item| item == thing) {
            fail(format!("costs {thing}, which is no item"));
        }
        if *den == 0 || !crate::cost::Amount::new(*num, *den).in_bounds() {
            fail(format!(
                "costs {num}/{den} of {thing}, which a base cannot keep"
            ));
        }
    }
    dress(kit, &mut fail);
    // Its dress of the raw materials it is built from (D215): a piece of another material
    // puts that material in its cost.
    for (what, material) in dressed_in(kit) {
        if let Some(item) = raw(material)
            && !kit.cost.step.iter().any(|(thing, ..)| thing == item)
        {
            fail(format!(
                "dresses {what} in {material:?}, of {item}, which it does not cost"
            ));
        }
    }
    parts(kit, &mut fail);
    if let Some(framing) = &kit.framing {
        if kit.pattern != Pattern::Masonry {
            fail("is framed, which only a block kind is".into());
        }
        // Life size, and standing out no more than an eighth of a cube (D192).
        let sizes: [(&str, f32, f32, f32); 6] = [
            ("its frame's members", framing.wide, 0.08, 0.3),
            ("its frame's standing out", framing.proud, 0.0, CELL_M / 8.0),
            (
                "its frame's depth",
                framing.deep,
                framing.proud + 0.02,
                CELL_M * 0.5,
            ),
            ("its corner posts", framing.corner, framing.wide, 0.3),
            ("its braces", framing.brace, 0.06, framing.wide),
            ("its core's relief", framing.core_relief, 0.0, 0.06),
        ];
        for (what, value, low, high) in sizes {
            if !value.is_finite() || value < low || value > high {
                fail(format!("{what}, {value} m, not from {low} to {high} m"));
            }
        }
        if framing.bay < 2 {
            fail(format!(
                "posts every {} cubes, closer than a metre",
                framing.bay
            ));
        }
        if framing
            .rails
            .iter()
            .any(|&rail| rail <= 0 || rail >= crate::STOREY_CUBES)
        {
            fail("a rail outside its storey".into());
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

/// What a kit's dress is made of (D215): each role's piece's material, and its frame's
/// members and core, by what it is.
fn dressed_in(kit: &Kit) -> Vec<(String, Material)> {
    use crate::kit::Dress;
    let mut out: Vec<(String, Material)> = kit
        .dress
        .roles
        .iter()
        .map(|(role, dress)| {
            let material = match dress {
                Dress::Profile { material, .. } => material.unwrap_or(Material::Dressed),
                Dress::Corners { .. } => Material::Dressed,
                Dress::Member { material, .. } => *material,
                Dress::Pillar { .. } => Material::Masonry,
            };
            (format!("{role:?}"), material)
        })
        .collect();
    if let Some(framing) = &kit.framing {
        out.push(("its frame".into(), framing.material));
        if let Some(core) = framing.core {
            out.push(("its frame's core".into(), core));
        }
    }
    // A roof's lines and frame, each a module of its pieces in its own material (D258).
    for (part, made) in &kit.roof.parts {
        out.push((format!("{part:?}"), made.material));
    }
    out
}

/// The raw material a piece drawn in a material is made of, as the items name it; none for
/// the kind's own surface, which is what its cubes are.
fn raw(material: Material) -> Option<&'static str> {
    match material {
        Material::Dressed | Material::Stone => Some("Stone"),
        Material::Hewn
        | Material::Log
        | Material::LogEnd
        | Material::Board
        | Material::Floor
        | Material::Wattle => Some("Wood"),
        Material::Plain
        | Material::Daub
        | Material::Covering
        | Material::Masonry
        | Material::Flashing
        | Material::Piece
        | Material::Trim => None,
    }
}

/// A kit's dress (D215): every role it dresses has its piece among the kit's pieces, or
/// for a member a material it has; and each piece keeps to its budget.
fn dress(kit: &Kit, fail: &mut dyn FnMut(String)) {
    use crate::kit::Dress;
    let pieces = kit.dress.made.as_deref();
    if kit.dress.pieces.is_some() && pieces.is_none() {
        fail("its dressed pieces are not read".into());
    }
    for (role, dress) in &kit.dress.roles {
        match dress {
            Dress::Profile {
                name,
                extend,
                material,
                ..
            } => {
                if let Some(material) = material
                    && !kit.textures.contains_key(material)
                {
                    fail(format!(
                        "dresses {role:?} in {material:?}, which it has no texture for"
                    ));
                }
                let Some(profile) = pieces.and_then(|p| p.profiles.get(name)) else {
                    fail(format!(
                        "dresses {role:?} with the profile {name}, which it has not"
                    ));
                    continue;
                };
                if profile.len() < 3 {
                    fail(format!("its profile {name} has too few points"));
                }
                // Out from its face; a floor's border lies on the floor, so up from it.
                let axis = usize::from(*role == crate::layout::Role::Border);
                let out = profile.iter().map(|p| p[axis]).fold(f32::MIN, f32::max);
                if out > CELL_M / 8.0 + 1e-3 {
                    fail(format!(
                        "its profile {name} stands out {out} m, past an eighth of a cube (D192)"
                    ));
                }
                if !(0.0..=CELL_M).contains(extend) {
                    fail(format!("{role:?} runs on {extend} m past its ends"));
                }
                let per_metre = crate::dress::trim_triangles(profile) * 2;
                if per_metre > TRIM_PER_METRE {
                    fail(format!(
                        "its profile {name}: {per_metre} triangles a metre, over the {TRIM_PER_METRE} its budget allows"
                    ));
                }
            }
            Dress::Corners { set } => {
                let Some(stones) = pieces.and_then(|p| p.corners.get(set)) else {
                    fail(format!(
                        "dresses {role:?} with the corner set {set}, which it has not"
                    ));
                    continue;
                };
                if stones.long.is_empty() {
                    fail(format!("its corner set {set} has no stones"));
                }
                for stone in stones.long.iter().chain(&stones.short) {
                    if stone.triangles.len() > CORNER_STONE {
                        fail(format!(
                            "a stone of its corner set {set}: {} triangles, over the {CORNER_STONE} its budget allows",
                            stone.triangles.len()
                        ));
                    }
                    let out = stone
                        .positions
                        .iter()
                        .map(|p| p[0].max(p[2]))
                        .fold(f32::MIN, f32::max);
                    if out > CELL_M / 8.0 + 1e-3 {
                        fail(format!(
                            "a stone of its corner set {set} stands out {out} m, past an eighth of a cube (D192)"
                        ));
                    }
                }
            }
            Dress::Member {
                material, scale, ..
            } => {
                if !kit.textures.contains_key(material) {
                    fail(format!(
                        "dresses {role:?} in {material:?}, which it has no texture for"
                    ));
                }
                if !(0.2..=2.0).contains(scale) {
                    fail(format!("{role:?}'s member is {scale} times a log"));
                }
            }
            Dress::Pillar { width } => {
                if *role != crate::layout::Role::Post {
                    fail(format!(
                        "dresses {role:?} with a pillar, which only posts are"
                    ));
                }
                if !(0.1..=CELL_M).contains(width) {
                    fail(format!("its pillars are {width} m wide, not within a cube"));
                }
            }
        }
    }
}

/// A kit's modules (D257): every part of its roof and its frame names a module it has, in a
/// material it has a texture for, and each module keeps to its budget, near a metre and in
/// its cap.
fn parts(kit: &Kit, fail: &mut dyn FnMut(String)) {
    let pieces = kit.dress.made.as_deref();
    let mut named: Vec<(String, &str, Material, Option<Material>)> = kit
        .roof
        .parts
        .iter()
        .map(|(part, p)| (format!("{part:?}"), p.module.as_str(), p.material, p.ends))
        .collect();
    if let Some(framing) = &kit.framing {
        named.push((
            "its frame".into(),
            framing.module.as_str(),
            framing.material,
            Some(Material::LogEnd),
        ));
    }
    if let Some(module) = &kit.timber.module {
        named.push((
            "its logs".into(),
            module.as_str(),
            Material::Log,
            Some(Material::LogEnd),
        ));
    }
    if kit.pattern == Pattern::Roof {
        for part in [
            RoofPart::Ridge,
            RoofPart::Hip,
            RoofPart::Verge,
            RoofPart::Rafter,
            RoofPart::Batten,
        ] {
            if !kit.roof.parts.contains_key(&part) {
                fail(format!("its roof has no piece for {part:?}"));
            }
        }
    }
    for (what, name, material, ends) in named {
        for material in [Some(material), ends].into_iter().flatten() {
            if !kit.textures.contains_key(&material) {
                fail(format!(
                    "draws {what} in {material:?}, which it has no texture for"
                ));
            }
        }
        let Some(module) = pieces.and_then(|p| p.modules.get(name)) else {
            fail(format!("{what} is the module {name}, which it has not"));
            continue;
        };
        if module.near.is_empty() || module.length <= 0.0 {
            fail(format!("its module {name} has nothing in it"));
            continue;
        }
        let atlas = if module.shared {
            Material::Trim
        } else {
            Material::Piece
        };
        if crate::modules::modelled(module) && !kit.textures.contains_key(&atlas) {
            fail(format!(
                "its module {name} is modelled, and it has no {atlas:?} atlas to draw it in"
            ));
        }
        let per_metre = (module.triangles() as f32 / module.length).round() as usize;
        if per_metre > MODULE_PER_METRE {
            fail(format!(
                "its module {name}: {per_metre} triangles a metre, over the {MODULE_PER_METRE} its budget allows"
            ));
        }
        if let Some(cap) = &module.cap
            && cap.triangles.len() > MODULE_CAP
        {
            fail(format!(
                "its module {name}'s cap: {} triangles, over the {MODULE_CAP} its budget allows",
                cap.triangles.len()
            ));
        }
        if module.near.iter().any(|mesh| !faces_out(mesh)) {
            fail(format!("its module {name} faces in"));
        }
        let far = module
            .far
            .iter()
            .map(|m| m.triangles.len())
            .max()
            .unwrap_or(0);
        if far * 2 > module.triangles() {
            fail(format!("its module {name} is no lighter far off"));
        }
    }
}

/// Whether a module's mesh faces out, away from the middle of its section along it, as it
/// is wound: what a module seen from outside needs, or its faces are culled.
fn faces_out(mesh: &crate::dress::Stone) -> bool {
    use glam::Vec3;
    let at = |i: u32| Vec3::from_array(mesh.positions[i as usize]);
    let count = mesh.positions.len().max(1) as f32;
    let middle = mesh
        .positions
        .iter()
        .map(|p| Vec3::from_array(*p))
        .sum::<Vec3>()
        / count;
    let out: f32 = mesh
        .triangles
        .iter()
        .map(|t| {
            let [a, b, c] = t.map(at);
            let centre = (a + b + c) / 3.0;
            let away = (centre - middle).with_x(0.0);
            (b - a).cross(c - a).dot(away)
        })
        .sum();
    out > 0.0
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
    // Each model's contract, as the first piece made from it has it (D290, D303).
    let mut models: BTreeMap<String, (String, [f32; 3])> = BTreeMap::new();
    for (name, kit) in kits {
        let mut report = check(kit, exists, items);
        if let Some(other) = kinds.insert(kit.id, name) {
            report
                .problems
                .push(format!("lays kind {}, as {other} does", kit.id));
        }
        // A model is made to one size wherever it is used: no material fits it to its own
        // box, which is the per-material offset the contract does away with.
        for (piece, module) in kit.dress.made.iter().flat_map(|made| &made.modules) {
            let Some(model) = &module.model else {
                continue;
            };
            match models.get(model) {
                Some((first, dims)) if !same_model_size(*dims, module.dims) => {
                    report.problems.push(format!(
                        "its piece {piece} makes {model} {:?}, where {first} makes it {dims:?}: one model, one size",
                        module.dims
                    ));
                }
                Some(_) => {}
                None => {
                    models.insert(model.clone(), (format!("{name}'s {piece}"), module.dims));
                }
            }
        }
        reports.insert(name.clone(), report);
    }
    reports
}

/// Whether two sizes agree within the contract's tolerance: 2% or a centimetre, whichever
/// is more (D303).
fn same_size(a: [f32; 3], b: [f32; 3]) -> bool {
    (0..3).all(|i| (a[i] - b[i]).abs() <= (a[i].abs() * 0.02).max(0.01))
}

/// Whether two pieces are one model at one size, however each lies: a board on its edge
/// along a verge and flat as flashing is one board.
fn same_model_size(a: [f32; 3], b: [f32; 3]) -> bool {
    let sorted = |mut d: [f32; 3]| {
        d.sort_by(f32::total_cmp);
        d
    };
    same_size(sorted(a), sorted(b))
}

/// A kit's pieces against their contract (D290, D303): each modelled module states its real
/// size and its model, and is that size from its origin; and every module names its ends.
pub fn contract(kit: &Kit) -> Vec<String> {
    let mut problems = Vec::new();
    for (name, module) in kit.dress.made.iter().flat_map(|made| &made.modules) {
        if !module.sockets.contains_key("start") || !module.sockets.contains_key("end") {
            problems.push(format!("its piece {name} has no start and end sockets"));
        }
        if !crate::modules::modelled(module) {
            continue;
        }
        if module.model.is_none() || module.dims.iter().any(|d| *d <= 0.0) {
            problems.push(format!("its modelled piece {name} states no model or size"));
            continue;
        }
        let mut low = [f32::MAX; 3];
        let mut high = [f32::MIN; 3];
        for p in module.near.iter().flat_map(|mesh| &mesh.positions) {
            for i in 0..3 {
                low[i] = low[i].min(p[i]);
                high[i] = high[i].max(p[i]);
            }
        }
        let size = [0, 1, 2].map(|i| high[i] - low[i]);
        if !same_size(size, module.dims) {
            problems.push(format!(
                "its piece {name} is {size:?}, not the {:?} its contract says",
                module.dims
            ));
        }
        if low[0].abs() > 0.01 {
            problems.push(format!(
                "its piece {name} starts {:.3} m from its origin along it, not at it",
                low[0]
            ));
        }
    }
    // Its corner stones, quoins and jamb stones: each states its real size, the box of its
    // dressed faces, and is that size, its origin the wall's corner it is laid at, from the
    // course's foot.
    for (set, corners) in kit.dress.made.iter().flat_map(|made| &made.corners) {
        for (which, stones) in [("long", &corners.long), ("short", &corners.short)] {
            for (variant, stone) in stones.iter().enumerate() {
                let what = format!("its corner stone {set} {which} {variant}");
                if stone.dims.iter().any(|d| *d <= 0.0) {
                    problems.push(format!("{what} states no size"));
                    continue;
                }
                let mut low = [f32::MAX; 3];
                let mut high = [f32::MIN; 3];
                for p in &stone.positions {
                    for i in 0..3 {
                        low[i] = low[i].min(p[i]);
                        high[i] = high[i].max(p[i]);
                    }
                }
                let size = [0, 1, 2].map(|i| high[i] - low[i]);
                if !same_size(size, stone.dims) {
                    problems.push(format!(
                        "{what} is {size:?}, not the {:?} its contract says",
                        stone.dims
                    ));
                }
                if high[0].abs() > STONE_PROUD_M || high[2].abs() > STONE_PROUD_M || low[1] < -0.01
                {
                    problems.push(format!(
                        "{what} is not seated at its corner: its faces {:.3} and {:.3} m out, its foot {:.3} m up",
                        high[0], high[2], low[1]
                    ));
                }
            }
        }
    }
    problems
}

/// The most a corner stone's dressed face stands out of the wall, pillowed and pitched.
const STONE_PROUD_M: f32 = 0.05;

/// What a kit draws on a cabin five metres by four with a door and a window, of its kind,
/// or for a roof kind, of plain walls under a gable of it: triangles a metre of its long
/// members and a square metre of its roof.
pub fn measure(kit: &Kit) -> Measured {
    let roof = kit.pattern == Pattern::Roof;
    let walls = if roof { Kind::PLAIN } else { kit.kind() };
    let mut base = Base::default();
    base.apply(BuildOp::AddRoom {
        min: [0, 0, 0],
        max: [9, 0, 7],
        kind: walls,
    });
    for x in 3..=4 {
        base.apply(BuildOp::SetWall {
            tile: [x, 0, 7],
            panel: Some(crate::Panel::Door),
            kind: walls,
        });
    }
    base.apply(BuildOp::SetWall {
        tile: [0, 0, 3],
        panel: Some(crate::Panel::WINDOW),
        kind: walls,
    });
    if roof {
        let footprint = (0..10).flat_map(|x| (0..8).map(move |z| [x, z])).collect();
        let plan = crate::roofer::over(&footprint, 6, crate::roofer::Style::Gable, None);
        for (at, shape) in plan.roof {
            base.apply(BuildOp::Fill {
                min: at.to_array(),
                max: at.to_array(),
                cube: Some(Cube {
                    shape,
                    ..Cube::new(kit.kind(), Fill::Solid)
                }),
            });
        }
    }
    let layout = layout::lay(&base, kit);
    let (mut logs, mut length, mut roof, mut squares, mut all) = (0, 0.0, 0, 0, 0);
    let mut dress = 0;
    for piece in &layout.pieces {
        let mut surfaces = Surfaces::default();
        crate::draw::draw(piece, kit, None, &mut surfaces);
        let count: usize = surfaces.0.values().map(|s| s.triangle_count()).sum();
        all += count;
        match *piece {
            Piece::Trim { .. }
            | Piece::Corner { .. }
            | Piece::Beam { .. }
            | Piece::Log { role: Some(_), .. } => {
                dress += count;
            }
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
        dress_per_cube: (dress > 0).then(|| {
            dress
                / base
                    .cubes()
                    .filter(|(_, c)| c.kind == kit.kind())
                    .count()
                    .max(1)
        }),
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
        let kits: Vec<(String, Kit)> = Kit::read_folder(&assets)
            .unwrap()
            .into_iter()
            .map(|(name, kit)| (format!("{name}.ron"), kit))
            .collect();
        (
            kits,
            items.kinds.into_iter().map(|item| item.name).collect(),
        )
    }

    #[test]
    fn a_model_made_to_two_sizes_or_off_its_size_fails_its_contract() {
        // D290, D303: one model is one size wherever it is used, and a piece is that size
        // from its origin; no material fits a shared model to a box of its own.
        let piece = |dims: [f32; 3], high: f32| crate::dress::Module {
            length: dims[0],
            dims,
            model: Some("art/kits/boards/board/model.glb".into()),
            sockets: [
                ("start".to_string(), [0.0; 3]),
                ("end".to_string(), [dims[0], 0.0, 0.0]),
            ]
            .into_iter()
            .collect(),
            near: vec![crate::dress::Stone {
                dims: [0.0; 3],
                positions: vec![[0.0, 0.0, 0.0], [dims[0], high, dims[2]]],
                normals: vec![[0.0, 1.0, 0.0]; 2],
                triangles: Vec::new(),
                uvs: vec![[0.0, 0.0]; 2],
            }],
            ..Default::default()
        };
        let kit = |id: u16, dims: [f32; 3], high: f32| {
            let mut kit = Kit {
                id,
                ..Kit::default()
            };
            let mut pieces = crate::dress::Pieces::default();
            pieces.modules.insert("verge".into(), piece(dims, high));
            kit.dress.made = Some(std::sync::Arc::new(pieces));
            kit
        };
        assert!(contract(&kit(1, [2.0, 0.22, 0.03], 0.22)).is_empty());
        assert!(
            !contract(&kit(1, [2.0, 0.22, 0.03], 0.3)).is_empty(),
            "off its size"
        );
        let kits = [
            ("a".to_string(), kit(1, [2.0, 0.22, 0.03], 0.22)),
            ("b".to_string(), kit(3, [2.0, 0.03, 0.22], 0.03)),
            ("c".to_string(), kit(7, [2.0, 0.27, 0.03], 0.27)),
        ];
        let reports = check_all(&kits, &|_| true, &[]);
        let model = |name: &str| {
            reports[name]
                .problems
                .iter()
                .any(|p| p.contains("one model, one size"))
        };
        assert!(!model("a") && !model("b"), "a board flat is the same board");
        assert!(model("c"), "{:?}", reports["c"].problems);
    }

    #[test]
    fn a_corner_stone_states_its_size_and_is_it_seated_at_its_corner() {
        let stone = |dims: [f32; 3], lift: f32| crate::dress::Stone {
            dims,
            positions: vec![[-0.42, lift, -0.23], [0.02, 0.236 + lift, 0.02]],
            ..Default::default()
        };
        let kit = |stone: crate::dress::Stone| {
            let mut pieces = crate::dress::Pieces::default();
            pieces.corners.insert(
                "quoin".into(),
                crate::dress::Corners {
                    long: vec![stone],
                    short: Vec::new(),
                },
            );
            let mut kit = Kit::default();
            kit.dress.made = Some(std::sync::Arc::new(pieces));
            kit
        };
        assert!(contract(&kit(stone([0.44, 0.236, 0.25], 0.0))).is_empty());
        for (wrong, why) in [
            (stone([0.0; 3], 0.0), "states no size"),
            (stone([0.3, 0.236, 0.25], 0.0), "not the"),
            (stone([0.44, 0.236, 0.25], -0.1), "not seated"),
        ] {
            let problems = contract(&kit(wrong));
            assert!(problems.iter().any(|p| p.contains(why)), "{problems:?}");
        }
    }

    #[test]
    fn every_shipped_kit_passes_the_checker() {
        let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        let (kits, items) = shipped();
        assert!(kits.len() >= 3, "{kits:?}");
        let exists = |path: &str| assets.join(path).is_file();
        for (name, report) in check_all(&kits, &exists, &items) {
            assert!(report.problems.is_empty(), "{name}: {:?}", report.problems);
            let roof = kits
                .iter()
                .any(|(n, kit)| *n == name && kit.pattern == Pattern::Roof);
            assert_eq!(report.measured.per_square_metre > 0, roof, "{name}");
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
        kit.textures.remove(&Material::Daub);
        kit.diameter = 2.0;
        kit.cost.step = vec![("Gold".into(), 1, 3)];
        kit.roof_kind = Some(2);
        let report = check(&kit, &|_| true, &items);
        let said = report.problems.join("; ");
        for words in [
            "no Daub material",
            "diameter 2",
            "Gold, which is no item",
            "its roofs' kind 2 is not a roof's",
        ] {
            assert!(said.contains(words), "{said}");
        }
        // Earth with oak lintels, costing no wood.
        let mut earth = kits
            .iter()
            .find(|(name, _)| name == "earth.ron")
            .map(|(_, kit)| kit.clone())
            .unwrap();
        assert!(check(&earth, &|_| true, &items).problems.is_empty());
        earth.cost.step.retain(|(thing, ..)| thing != "Wood");
        let said = check(&earth, &|_| true, &items).problems.join("; ");
        assert!(
            said.contains("dresses Head in Hewn, of Wood, which it does not cost"),
            "{said}"
        );
        // A roof kind numbered as a wall's, in three steps, without its covering.
        let mut thatch = kits
            .iter()
            .find(|(name, _)| name == "thatch.ron")
            .map(|(_, kit)| kit.clone())
            .unwrap();
        thatch.id = 7;
        thatch.cost.steps = 2;
        thatch.textures.remove(&Material::Covering);
        let said = check(&thatch, &|_| true, &items).problems.join("; ");
        for words in ["is not a roof's", "not 2 steps", "no Covering material"] {
            assert!(said.contains(words), "{said}");
        }
        // The stone kit dressing a role with a piece it has not, and one past its budget.
        let mut stone = kits
            .iter()
            .find(|(name, _)| name == "stone.ron")
            .map(|(_, kit)| kit.clone())
            .unwrap();
        stone.dress.roles.insert(
            crate::layout::Role::Inside,
            crate::kit::Dress::Profile {
                name: "cornice".into(),
                extend: 0.0,
                joints: true,
                material: None,
            },
        );
        let mut pieces = (**stone.dress.made.as_ref().unwrap()).clone();
        let quoin = pieces.corners.get_mut("quoin").unwrap();
        let stone_of = &mut quoin.long[0];
        let more = stone_of.triangles.clone();
        stone_of.triangles.extend(more);
        stone.dress.made = Some(std::sync::Arc::new(pieces));
        let said = check(&stone, &|_| true, &items).problems.join("; ");
        for words in ["the profile cornice, which it has not", "corner set quoin"] {
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
