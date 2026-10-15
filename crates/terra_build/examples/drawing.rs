//! The drawing audit, `cargo xtask rules` (research/building-audit.md): every building of
//! the village (D229), in every kit the game ships with its dressed pieces, laid and drawn
//! section by section as the game's workers do, checked for what a picture shows only by
//! luck:
//!
//! - **Laid twice, the same**: every section drawn twice at each detail gives the same
//!   triangles, bit for bit.
//! - **No z-fighting**: no two triangles of a section's near meshes face the same way in the
//!   same plane (within 2 mm) and overlap by more than a square centimetre, whatever kinds
//!   or materials they are of. Reported by the pair of materials that meet.
//! - **One edit, only what it must**: a cube taken out of a wall marks its section and its
//!   neighbours on an edge; the sections set waiting (`sections::affected`), those whose
//!   pieces change and must be merged again, and that no other section changed.
//!
//! Each piece in one section, sections merged exactly as the base laid whole, and seams at
//! section edges are `terra_build`'s own tests (`sections::tests`). Exits non-zero when a
//! check fails.

use glam::{IVec3, Vec2, Vec3};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::ExitCode;
use std::time::Instant;
use terra_build::kit::{Kit, Kits};
use terra_build::relief::Maps;
use terra_build::sections::{self, Progress, Reach, Section};
use terra_build::{Base, BuildOp, Kind, Surface};

fn main() -> ExitCode {
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let (kits, names) = read_kits(&assets);
    let kind = |name: &str| *names.get(name).unwrap_or_else(|| panic!("no kit {name}"));
    let read = |path: &str| {
        std::fs::read_to_string(assets.join(path)).map_err(|why| format!("{path}: {why}"))
    };
    let kinds = terra_build::village::Palette::read(&read, &|name| Some(kind(name)), &kits)
        .expect("the settlement's materials");
    let only: Option<String> = std::env::args().nth(1);
    let maps = Maps::default();
    let mut ok = true;
    let (mut sections_drawn, mut triangles) = (0usize, 0usize);
    let mut fights: BTreeMap<String, (usize, f32, String)> = BTreeMap::new();
    let mut edits: Vec<(String, usize, usize, usize, f32)> = Vec::new();
    let mut operations: Vec<f32> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for building in terra_build::village::village(7, &kinds) {
        // One of each sort of building: the village repeats them.
        let sort = format!("{:?} {:?}", building.plan, building.scheme);
        if !seen.insert(sort)
            || only
                .as_ref()
                .is_some_and(|o| !building.name.contains(o.as_str()))
        {
            continue;
        }
        let mut base = Base::default();
        for op in &building.ops {
            base.apply(*op);
        }
        base.lay_footings(kinds.first().masonry, |column| {
            -((column[0] + column[1]).rem_euclid(3) as i16) * 20
        });
        let progress = Progress::Whole;
        let whole = sections::Whole::lay(&base, &kits);
        for at in sections::drawable(&base) {
            let here = section_of(&base, &kits, &maps, &progress, at);
            let laid = sections::lay(&here, &whole);
            for reach in [Reach::Near, Reach::Mid, Reach::Far] {
                let (a, b) = (
                    sections::draw(&here, &laid, reach),
                    sections::draw(&here, &laid, reach),
                );
                if bits(&a) != bits(&b) {
                    println!(
                        "FAIL  {}: section {at:?} drawn twice at {reach:?} differs",
                        building.name
                    );
                    ok = false;
                }
            }
            let near = sections::draw(&here, &laid, Reach::Near);
            sections_drawn += 1;
            let mut labelled: Vec<(String, &Surface)> = near
                .near
                .iter()
                .map(|((kind, material), surface)| {
                    (format!("{}:{material:?}", name(&names, *kind)), surface)
                })
                .collect();
            labelled.push(("plain".into(), &near.plain));
            let (count, found) = z_fights(&labelled);
            triangles += count;
            for (pair, area, place) in found {
                let entry = fights.entry(pair).or_insert((0, 0.0, String::new()));
                entry.0 += 1;
                if area > entry.1 {
                    *entry = (
                        entry.0,
                        area,
                        format!("{}, section {at:?}, at {place}", building.name),
                    );
                }
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
                    sections::lay(&section_of(&base, &kits, &maps, &progress, at), &whole).key(),
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
        let after_whole = sections::Whole::lay(&base, &kits);
        let mut changed = 0;
        for &at in &waiting {
            if before.get(&at)
                != Some(
                    &sections::lay(
                        &section_of(&base, &kits, &maps, &progress, at),
                        &after_whole,
                    )
                    .key(),
                )
            {
                changed += 1;
                let laid = sections::lay(
                    &section_of(&base, &kits, &maps, &progress, at),
                    &after_whole,
                );
                let _ = sections::draw(
                    &section_of(&base, &kits, &maps, &progress, at),
                    &laid,
                    Reach::Near,
                );
            }
        }
        let ms = started.elapsed().as_secs_f32() * 1000.0;
        for (&at, key) in &before {
            if !waiting.contains(&at)
                && sections::lay(
                    &section_of(&base, &kits, &maps, &progress, at),
                    &after_whole,
                )
                .key()
                    != *key
            {
                println!(
                    "FAIL  {}: section {at:?} changed by an edit that did not set it waiting",
                    building.name
                );
                ok = false;
            }
        }
        edits.push((
            building.name.clone(),
            before.len(),
            waiting.len(),
            changed,
            ms,
        ));
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
                    building.name
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

/// Every kit under `assets/data/kits` with its dressed pieces, and each one's kind by its
/// file's name.
fn read_kits(assets: &Path) -> (Kits, BTreeMap<String, Kind>) {
    let read = |path: &str| {
        std::fs::read_to_string(assets.join(path)).map_err(|why| format!("{path}: {why}"))
    };
    let mut paths: Vec<_> = std::fs::read_dir(assets.join("data/kits"))
        .expect("the kits")
        .flatten()
        .map(|entry| entry.path())
        .filter(|p| p.extension().is_some_and(|e| e == "ron"))
        .collect();
    paths.sort();
    let (mut kits, mut names) = (Vec::new(), BTreeMap::new());
    for path in paths {
        let stem = path.file_stem().unwrap().to_string_lossy().to_string();
        let kit = Kit::read(
            &format!("{}/{stem}.ron", terra_build::kit::MATERIALS),
            &read,
        )
        .unwrap_or_else(|why| panic!("{stem}: {why}"));
        names.insert(stem, kit.kind());
        kits.push(kit);
    }
    (Kits(kits), names)
}

fn name(names: &BTreeMap<String, Kind>, kind: Kind) -> String {
    names
        .iter()
        .find(|(_, k)| **k == kind)
        .map_or(format!("{kind:?}"), |(n, _)| n.clone())
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

/// A triangle, its label, its plane and its corners.
struct Tri {
    label: usize,
    normal: Vec3,
    d: f32,
    corners: [Vec3; 3],
}

/// Pairs of triangles facing the same way in the same plane that overlap: how many
/// triangles were looked at, and each pair's materials, overlap in square metres and where.
fn z_fights(surfaces: &[(String, &Surface)]) -> (usize, Vec<(String, f32, String)>) {
    let mut tris = Vec::new();
    for (label, (_, surface)) in surfaces.iter().enumerate() {
        let all = surface.indexed();
        for t in all.indices.chunks_exact(3) {
            let corners = [0, 1, 2].map(|i| all.positions[t[i] as usize]);
            let cross = (corners[1] - corners[0]).cross(corners[2] - corners[0]);
            if cross.length() < 1e-7 {
                continue;
            }
            let normal = cross.normalize();
            tris.push(Tri {
                label,
                normal,
                d: normal.dot(corners[0]),
                corners,
            });
        }
    }
    // Into bins by plane, then by where they are in it, a quarter metre a cell.
    let mut bins: BTreeMap<([i32; 3], i32), Vec<usize>> = BTreeMap::new();
    for (i, tri) in tris.iter().enumerate() {
        let n = (tri.normal * 50.0).round().as_ivec3().to_array();
        bins.entry((n, (tri.d / 0.002).round() as i32))
            .or_default()
            .push(i);
    }
    let mut found = Vec::new();
    let mut checked: BTreeSet<(usize, usize)> = BTreeSet::new();
    for ((n, d), members) in &bins {
        // Their neighbours a step of the plane's offset away too.
        let mut near: Vec<usize> = members.clone();
        for step in [-1, 1] {
            if let Some(more) = bins.get(&(*n, d + step)) {
                near.extend(more);
            }
        }
        let axis = tris[members[0]].normal.abs().max_position();
        let flat = |p: Vec3| match axis {
            0 => Vec2::new(p.y, p.z),
            1 => Vec2::new(p.x, p.z),
            _ => Vec2::new(p.x, p.y),
        };
        let mut cells: BTreeMap<[i32; 2], Vec<usize>> = BTreeMap::new();
        for &i in &near {
            let pts = tris[i].corners.map(flat);
            let (lo, hi) = (
                pts[0].min(pts[1]).min(pts[2]),
                pts[0].max(pts[1]).max(pts[2]),
            );
            for x in (lo.x / 0.25).floor() as i32..=(hi.x / 0.25).floor() as i32 {
                for y in (lo.y / 0.25).floor() as i32..=(hi.y / 0.25).floor() as i32 {
                    cells.entry([x, y]).or_default().push(i);
                }
            }
        }
        for list in cells.values() {
            for (k, &a) in list.iter().enumerate() {
                for &b in &list[k + 1..] {
                    let pair = (a.min(b), a.max(b));
                    if a == b || !checked.insert(pair) {
                        continue;
                    }
                    let (ta, tb) = (&tris[a], &tris[b]);
                    if ta.normal.dot(tb.normal) < 0.9995 || (ta.d - tb.d).abs() > 0.002 {
                        continue;
                    }
                    let area = overlap(ta.corners.map(flat), tb.corners.map(flat));
                    if area > 1e-4 {
                        let mut labels =
                            [surfaces[ta.label].0.clone(), surfaces[tb.label].0.clone()];
                        labels.sort();
                        let middle = (ta.corners[0] + ta.corners[1] + ta.corners[2]) / 3.0;
                        found.push((
                            format!("{} / {}", labels[0], labels[1]),
                            area,
                            format!("{middle:.2}"),
                        ));
                    }
                }
            }
        }
    }
    (tris.len(), found)
}

/// The area two triangles in a plane share: one clipped by the other.
fn overlap(a: [Vec2; 3], b: [Vec2; 3]) -> f32 {
    let wind = |t: [Vec2; 3]| (t[1] - t[0]).perp_dot(t[2] - t[0]);
    let b = if wind(b) < 0.0 { [b[0], b[2], b[1]] } else { b };
    let mut poly: Vec<Vec2> = a.to_vec();
    for i in 0..3 {
        let (p, q) = (b[i], b[(i + 1) % 3]);
        let inside = |v: Vec2| (q - p).perp_dot(v - p) >= -1e-6;
        let mut next = Vec::new();
        for j in 0..poly.len() {
            let (s, e) = (poly[j], poly[(j + 1) % poly.len()]);
            match (inside(s), inside(e)) {
                (true, true) => next.push(e),
                (true, false) => next.push(cut(s, e, p, q)),
                (false, true) => {
                    next.push(cut(s, e, p, q));
                    next.push(e);
                }
                (false, false) => {}
            }
        }
        poly = next;
        if poly.len() < 3 {
            return 0.0;
        }
    }
    let mut area = 0.0;
    for j in 0..poly.len() {
        area += poly[j].perp_dot(poly[(j + 1) % poly.len()]);
    }
    (area * 0.5).abs()
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

/// Where the segment from `s` to `e` crosses the line through `p` and `q`.
fn cut(s: Vec2, e: Vec2, p: Vec2, q: Vec2) -> Vec2 {
    let (r, d) = (e - s, q - p);
    let t = (p - s).perp_dot(d) / r.perp_dot(d);
    s + r * t
}
