//! The drawing audit, `cargo xtask rules` (research/building-audit.md): every scene the
//! registry holds before a push (`terra_build::scenes::registry`, `Gate::Push`): one of each
//! sort of the village's buildings (D229), the QA cabins, the junctions, roofs and walls and
//! the rectangles (the kit sheet's only by name while gap 19 is open), in the building
//! baseline's kits with their dressed pieces, laid and drawn section by section as the
//! game's workers do, checked for what a picture shows only by luck (a name given keeps only
//! the scenes whose names hold it):
//!
//! - **Laid twice, the same**: every section drawn twice at each detail gives the same
//!   triangles, bit for bit.
//! - **No z-fighting**: no two triangles of a section's near meshes face the same way in one
//!   plane and overlap, whatever kinds or materials they are of, measured as the mesh
//!   checker measures it (`inspect::fights`, within a millimetre, never pressed against a
//!   face turned to meet them). Reported by the pair of materials that meet, and fails.
//! - **One edit, only what it must**: a cube taken out of a wall marks its section and its
//!   neighbours on an edge; the sections set waiting (`sections::affected`), those whose
//!   pieces change and must be merged again, and that no other section changed.
//!
//! Each piece in one section, sections merged exactly as the base laid whole, and seams at
//! section edges are `terra_build`'s own tests (`sections::tests`). Exits non-zero when a
//! check fails.

use glam::IVec3;
use std::collections::BTreeMap;
use std::path::Path;
use std::process::ExitCode;
use std::time::Instant;
use terra_build::inspect;
use terra_build::kit::Kits;
use terra_build::scenes::registry::{Family, Gate, Materials, ratchet};

use terra_build::relief::Maps;
use terra_build::sections::{self, Progress, Reach, Section};
use terra_build::{Base, BuildOp, Kind, Material, Surface};

/// A mesh a section draws near: its name, its kind and material where it has one, and it.
type Mesh = (String, Option<(Kind, Material)>, Surface);

fn main() -> ExitCode {
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let materials = Materials::read(&assets, true).expect("the baseline's materials");
    let (kits, names) = (&materials.kits, &materials.names);
    let only: Option<String> = std::env::args().nth(1);
    let maps = Maps::default();
    let mut ok = true;
    let (mut sections_drawn, mut triangles) = (0usize, 0usize);
    let mut fights: BTreeMap<String, (usize, f32, String)> = BTreeMap::new();
    let mut edits: Vec<(String, usize, usize, usize, f32)> = Vec::new();
    let mut operations: Vec<f32> = Vec::new();
    for scene in ratchet(&materials, Gate::Push) {
        // The kit sheet's rows have faults the ratchet's baseline holds while the voxel
        // contract's gap 19 is open; asked for by name, they are audited too.
        let sheet = scene.family == Family::Sheet && only.is_none();
        if sheet
            || only
                .as_ref()
                .is_some_and(|o| !scene.name.contains(o.as_str()))
        {
            continue;
        }
        let mut base = scene.base();
        let progress = Progress::Whole;
        let whole = sections::Whole::lay(&base, kits);
        let mut meshes: Vec<Mesh> = Vec::new();
        for at in sections::drawable(&base) {
            let here = section_of(&base, kits, &maps, &progress, at);
            let laid = sections::lay(&here, &whole);
            for reach in [Reach::Near, Reach::Mid, Reach::Far] {
                let (a, b) = (
                    sections::draw(&here, &laid, reach),
                    sections::draw(&here, &laid, reach),
                );
                if bits(&a) != bits(&b) {
                    println!(
                        "FAIL  {}: section {at:?} drawn twice at {reach:?} differs",
                        scene.name
                    );
                    ok = false;
                }
            }
            let near = sections::draw(&here, &laid, Reach::Near);
            sections_drawn += 1;
            for (&(kind, material), surface) in &near.near {
                meshes.push((
                    format!("{}:{material:?}", name(names, kind)),
                    Some((kind, material)),
                    surface.clone(),
                ));
            }
            meshes.push(("plain".into(), None, near.plain));
        }
        triangles += meshes
            .iter()
            .map(|(_, _, s)| s.triangle_count())
            .sum::<usize>();
        // Measured over the building's sections together, as the mesh checker measures it
        // (`inspect::fights`), so the two agree: a piece pressed against a face in the next
        // section is as unseen as one pressed against a face in its own.
        let all = meshes
            .iter()
            .map(|(name, kind, surface)| (name.clone(), *kind, surface));
        for fight in inspect::fights(all) {
            let mut pair = fight.meshes.clone();
            pair.sort();
            let what = if fight.doubled { ", drawn twice" } else { "" };
            let entry = fights
                .entry(format!("{} / {}{what}", pair[0], pair[1]))
                .or_insert((0, 0.0, String::new()));
            entry.0 += fight.pairs;
            if fight.area > entry.1 {
                entry.1 = fight.area;
                entry.2 = format!("{}, at {:.2}", scene.name, fight.at);
            }
        }
        // One cube out of the middle of the front wall, a storey's middle up.
        let wall = base
            .cubes()
            .filter(|(at, cube)| {
                at.y == 2
                    && cube.is_built()
                    && cube.fill == terra_build::Fill::Solid
                    && !cube.kind.is_roof()
                    && base.in_wall(*at)
            })
            .map(|(at, _)| at)
            .min_by_key(|at| (at.x.abs() + at.z.abs(), at.x, at.z));
        let Some(cube) = wall else { continue };
        let before: BTreeMap<[i32; 3], u64> = sections::drawable(&base)
            .into_iter()
            .map(|at| {
                (
                    at,
                    sections::lay(&section_of(&base, kits, &maps, &progress, at), &whole).key(),
                )
            })
            .collect();
        let _ = base.take_dirty();
        let started = Instant::now();
        base.apply(BuildOp::Fill {
            min: cube.to_array(),
            max: cube.to_array(),
            cube: None,
        });
        let marked = base.take_dirty();
        let waiting = sections::affected(&base, &marked);
        let after_whole = sections::Whole::lay(&base, kits);
        let mut changed = 0;
        for &at in &waiting {
            if before.get(&at)
                != Some(
                    &sections::lay(&section_of(&base, kits, &maps, &progress, at), &after_whole)
                        .key(),
                )
            {
                changed += 1;
                let laid =
                    sections::lay(&section_of(&base, kits, &maps, &progress, at), &after_whole);
                let _ = sections::draw(
                    &section_of(&base, kits, &maps, &progress, at),
                    &laid,
                    Reach::Near,
                );
            }
        }
        let ms = started.elapsed().as_secs_f32() * 1000.0;
        for (&at, key) in &before {
            if !waiting.contains(&at)
                && sections::lay(&section_of(&base, kits, &maps, &progress, at), &after_whole).key()
                    != *key
            {
                println!(
                    "FAIL  {}: section {at:?} changed by an edit that did not set it waiting",
                    scene.name
                );
                ok = false;
            }
        }
        edits.push((scene.name.clone(), before.len(), waiting.len(), changed, ms));
        // An operation checked and applied: the top of a wall's column out and in again,
        // timed over many.
        let rules = kits.rules();
        let cx = terra_build::ops::Context::free(&rules);
        let top = base
            .column(cube.x + 1, cube.z)
            .max()
            .map(|y| [cube.x + 1, y, cube.z]);
        if let Some(top) = top {
            let started = Instant::now();
            let mut count = 0;
            for _ in 0..100 {
                for op in [
                    terra_build::ops::Op::Empty { cubes: vec![top] },
                    terra_build::ops::Op::Fill { cubes: vec![top] },
                ] {
                    count += usize::from(base.act(&op, &cx).is_ok());
                }
            }
            operations.push(started.elapsed().as_secs_f32() * 1000.0 / 200.0);
            if count == 0 {
                println!(
                    "  ({}: its wall's top could not come out to time an operation)",
                    scene.name
                );
            }
        }
    }
    operations.sort_by(f32::total_cmp);
    if let (Some(median), Some(worst)) = (operations.get(operations.len() / 2), operations.last()) {
        println!(
            "An operation checked and applied (a wall's top out and in), per building: median {median:.3} ms, slowest {worst:.3} ms"
        );
    }
    println!(
        "Laid twice: {sections_drawn} sections drawn twice at three details, {}",
        if ok { "the same" } else { "NOT the same" }
    );
    println!(
        "Z-fighting: {triangles} triangles near; coplanar overlaps facing the same way, by what meets:"
    );
    if fights.is_empty() {
        println!("  none");
    } else {
        ok = false;
    }
    for (pair, (count, area, place)) in &fights {
        println!(
            "  {pair:44} {count:6} pairs, the largest {:.0} cm² ({place})",
            area * 1e4
        );
    }
    println!(
        "One cube out of a wall (sections drawn, set waiting, merged again, ms on one thread to lay whole, cut and merge what changed):"
    );
    for (name, all, waiting, changed, ms) in &edits {
        println!("  {name:58} {all:3} {waiting:3} {changed:3} {ms:8.1}");
    }
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// A kind by its file's name.
fn name(names: &BTreeMap<Kind, String>, kind: Kind) -> String {
    names.get(&kind).cloned().unwrap_or(format!("{kind:?}"))
}

/// Every triangle of a section's drawing at a detail, as bits, to compare two drawings.
fn bits(drawn: &sections::Drawn) -> Vec<u32> {
    let mut out = Vec::new();
    let mut add = |surface: &Surface| {
        let all = surface.indexed();
        out.extend(all.indices.iter().copied());
        out.extend(
            all.positions
                .iter()
                .flat_map(|p| p.to_array().map(f32::to_bits)),
        );
        out.extend(
            all.normals
                .iter()
                .flat_map(|p| p.to_array().map(f32::to_bits)),
        );
    };
    for surface in drawn.near.values().chain(drawn.mid.values()) {
        add(surface);
    }
    for surface in [&drawn.plain, &drawn.shell, &drawn.far, &drawn.proxy] {
        add(surface);
    }
    for surface in drawn.next.0.values() {
        add(surface);
    }
    out
}

/// A section of a base to lay and draw.
fn section_of<'a>(
    base: &'a Base,
    kits: &'a Kits,
    maps: &'a Maps,
    progress: &'a Progress,
    at: [i32; 3],
) -> Section<'a> {
    Section {
        base,
        kits,
        maps,
        at: IVec3::from_array(at),
        progress,
    }
}
