//! How long a base's sections take to lay and merge, `cargo xtask sections`
//! (docs/building-plan.md, checking it): the village's fifty buildings, every section of
//! each laid and merged at every detail as the game's workers do, on one thread, timed.
//! The bar is a section laid and merged in under 2 ms. Height maps are stand-ins of the
//! real ones' size, as the cost of shaping by them does not depend on what is in them.

use std::path::Path;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Instant;
use terra_build::Base;
use terra_build::kit::{Kit, Kits, Pattern, Profile};
use terra_build::relief::{Maps, Relief, Swatch, shaped_by, topped_by};
use terra_build::sections::{
    self, NEAR_MIB_A_BASE, NEAR_TRIANGLES_A_CUBE, Progress, Reach, Section,
};

/// The bar for a section laid and merged on a worker, in milliseconds.
const BAR_MS: f32 = 2.0;
/// What a corner of a textured mesh takes on the GPU, as the game uploads it: position,
/// normal, tangent, colour and texture place.
const NEAR_CORNER_BYTES: usize = 12 + 12 + 16 + 16 + 8;

fn main() -> ExitCode {
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let Some(kits) = read_kits(&assets) else {
        return ExitCode::FAILURE;
    };
    let find = |pattern: Pattern, profile: Profile| {
        kits.0
            .iter()
            .find(|kit| {
                kit.pattern == pattern
                    && (pattern == Pattern::Masonry || kit.timber.profile == profile)
            })
            .map(Kit::kind)
    };
    let (Some(logs), Some(stone), Some(hewn)) = (
        find(Pattern::Logs, Profile::Round),
        kits.footing().map(Kit::kind),
        find(Pattern::Logs, Profile::Hewn),
    ) else {
        println!("sections: the village needs a log, a stone and a hewn kit");
        return ExitCode::FAILURE;
    };
    let maps = stand_in_maps(&kits);
    let only: Option<String> = std::env::args().nth(1);
    let mut times: [Vec<(f32, f32)>; 3] = Default::default();
    let mut slowest: Vec<(f32, String)> = Vec::new();
    let mut whole_ms: Vec<f32> = Vec::new();
    let mut edits: Vec<Edit> = Vec::new();
    // Triangles of every section at each detail: near, mid, far and proxy.
    let mut triangles = [0usize; 4];
    let mut by_kind: std::collections::BTreeMap<String, (usize, usize)> = Default::default();
    // Near detail's triangles and bytes by pattern and material, and the cubes drawn.
    let mut near_by: std::collections::BTreeMap<String, (usize, usize)> = Default::default();
    let mut block_cubes = 0usize;
    let roof = |walls: terra_build::Kind| kits.roof_over(walls);
    for (name, ops) in terra_build::bench_village(logs, stone, hewn, &roof) {
        if only
            .as_ref()
            .is_some_and(|only| !name.contains(only.as_str()))
        {
            continue;
        }
        let base = Base::from_shortcuts(&ops);
        block_cubes += base
            .cubes()
            .filter(|(_, c)| {
                kits.of(c.kind)
                    .is_some_and(|k| k.pattern == Pattern::Masonry)
            })
            .count();
        let progress = Progress::Whole;
        // Laid whole once, as the game's first job for a change does.
        let started = Instant::now();
        let whole = sections::Whole::lay(&base, &kits);
        whole_ms.push(started.elapsed().as_secs_f32() * 1000.0);
        edits.extend(time_edit(&name, &base, &kits, &maps));
        for at in sections::drawable(&base) {
            let section = Section {
                base: &base,
                kits: &kits,
                maps: &maps,
                at: glam::IVec3::from_array(at),
                progress: &progress,
            };
            for (slot, reach) in [Reach::Near, Reach::Mid, Reach::Far]
                .into_iter()
                .enumerate()
            {
                let started = Instant::now();
                let laid = sections::lay(&section, &whole);
                let lay_ms = started.elapsed().as_secs_f32() * 1000.0;
                if laid.pieces.is_empty() {
                    continue;
                }
                let drawn = sections::draw(&section, &laid, reach);
                let total_ms = started.elapsed().as_secs_f32() * 1000.0;
                let count = |surfaces: &std::collections::BTreeMap<_, terra_build::Surface>| {
                    surfaces
                        .values()
                        .map(terra_build::Surface::triangle_count)
                        .sum::<usize>()
                };
                let finest = match reach {
                    Reach::Close | Reach::Near => count(&drawn.near),
                    Reach::Mid => count(&drawn.mid),
                    Reach::Far => drawn.far.triangle_count(),
                    Reach::Hull => drawn.proxy.triangle_count(),
                };
                triangles[slot] += finest;
                if reach == Reach::Near {
                    for ((kind, material), surface) in &drawn.near {
                        let pattern = kits.of(*kind).map_or(Pattern::Masonry, |k| k.pattern);
                        let all = surface.indexed();
                        let entry = near_by
                            .entry(format!("{pattern:?} {material:?}"))
                            .or_default();
                        entry.0 += all.triangle_count();
                        // As the game uploads it: position, normal, colour, tangent and
                        // texture place a corner, and an index each.
                        entry.1 += all.positions.len() * NEAR_CORNER_BYTES + all.indices.len() * 4;
                    }
                }
                if reach == Reach::Far {
                    triangles[3] += sections::proxy(&section).triangle_count();
                    if std::env::var_os("BY_KIND").is_some() {
                        let mut kinds: std::collections::BTreeSet<String> = Default::default();
                        for p in &laid.pieces {
                            kinds.insert(
                                format!("{:?}", p.1)
                                    .split([' ', '{', '('])
                                    .next()
                                    .unwrap()
                                    .to_string(),
                            );
                        }
                        for kind in kinds {
                            let mut only = laid.clone();
                            only.pieces
                                .retain(|p| format!("{:?}", p.1).starts_with(&kind));
                            let d = sections::draw(&section, &only, Reach::Far);
                            let e = by_kind.entry(kind).or_insert((0, 0));
                            e.0 += d.far.triangle_count();
                            e.1 += sections::proxy(&section).triangle_count();
                        }
                    }
                }
                times[slot].push((lay_ms, total_ms));
                slowest.push((total_ms, format!("{name}, section {at:?}, {reach:?}")));
            }
        }
    }
    let median = |values: &mut Vec<f32>| {
        values.sort_by(f32::total_cmp);
        values.get(values.len() / 2).copied().unwrap_or(0.0)
    };
    let mut met = true;
    let buildings = whole_ms.len();
    println!(
        "{buildings} buildings laid whole in a median {:.2} ms",
        median(&mut whole_ms)
    );
    println!("Sections with anything in them, cut from them and merged on one thread, in ms:");
    for (slot, name) in ["near", "mid", "far"].into_iter().enumerate() {
        let mut lay: Vec<f32> = times[slot].iter().map(|t| t.0).collect();
        let mut total: Vec<f32> = times[slot].iter().map(|t| t.1).collect();
        let count = total.len();
        let (lay, total) = (median(&mut lay), median(&mut total));
        let p90 = total_at(&times[slot], 0.9);
        println!(
            "  {name:5} {count:4} sections: cut in a median {lay:.2}, cut and merged {total:.2}, 90th percentile {p90:.2}"
        );
        met &= total < BAR_MS;
    }
    println!(
        "Triangles of them all, near {}, mid {}, far {}, proxy {}",
        triangles[0], triangles[1], triangles[2], triangles[3]
    );
    for (kind, (far, proxy)) in &by_kind {
        println!("  {kind:12} far {far:9} proxy {proxy:9}");
    }
    // Near detail, as dense as it is drawn where a player stands beside it, by what it is
    // drawn in; and the block kinds' surface and dress against their budget (D284).
    println!("Near, by pattern and material, triangles and MiB:");
    for (what, (tri, bytes)) in &near_by {
        println!(
            "  {what:22} {tri:9} {:7.1} MiB",
            *bytes as f32 / (1024.0 * 1024.0)
        );
    }
    let (block_triangles, block_bytes) = near_by
        .iter()
        .filter(|(what, _)| what.starts_with("Masonry"))
        .fold((0, 0), |(t, b), (_, &(tri, bytes))| (t + tri, b + bytes));
    let per_cube = block_triangles as f32 / block_cubes.max(1) as f32;
    let per_base = block_bytes as f32 / buildings.max(1) as f32 / (1024.0 * 1024.0);
    let dense = per_cube <= NEAR_TRIANGLES_A_CUBE && per_base <= NEAR_MIB_A_BASE;
    println!(
        "Block kinds near, surface and dress: {per_cube:.0} triangles a cube of them, {per_base:.2} MiB a building; the budget {NEAR_TRIANGLES_A_CUBE} and {NEAR_MIB_A_BASE} MiB: {}",
        if dense { "met" } else { "MISSED" }
    );
    report_edits(&mut edits);
    slowest.sort_by(|a, b| b.0.total_cmp(&a.0));
    println!("The slowest:");
    for (ms, what) in slowest.iter().take(6) {
        println!("  {ms:6.2}  {what}");
    }
    println!(
        "The bar: a section laid and merged in under {BAR_MS} ms (median): {}",
        if met { "met" } else { "MISSED" }
    );
    if met && dense {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// An edit timed: a wall's cube taken out of a building, and the section it is in drawn
/// again near, afresh and from how it was drawn before, on one thread, in ms.
struct Edit {
    what: String,
    whole: f32,
    afresh: f32,
    again: f32,
    taken: usize,
    faces: usize,
}

/// Takes a cube out of the middle of a wall a metre up, as a player's hammer does, and
/// times laying the base whole again and drawing the cube's section near again, both
/// afresh and, as the game does, only its parts the edit reached, from how they were drawn
/// before it (`sections::draw_parts`).
fn time_edit(name: &str, base: &Base, kits: &Kits, maps: &Maps) -> Option<Edit> {
    let walls: Vec<glam::IVec3> = base
        .cubes()
        .map(|(at, _)| at)
        .filter(|&at| at.y == 2 && base.in_wall(at))
        .collect();
    let cube = *walls.get(walls.len() / 2)?;
    let progress = Progress::Whole;
    let at = terra_build::cubes::section_of(cube);
    let section = |base| Section {
        base,
        kits,
        maps,
        at,
        progress: &progress,
    };
    let mut last = sections::Drawing::default();
    let before = section(base);
    let whole = sections::Whole::lay(base, kits);
    let laid_before = sections::lay(&before, &whole);
    let keys_before = sections::cluster_keys(&before, &laid_before, Reach::Near);
    let all = vec![true; keys_before.len()];
    sections::draw_parts(
        &before,
        &laid_before,
        Reach::Near,
        None,
        Some(&mut last),
        Some(&all),
    );
    let mut edited = base.clone();
    edited.apply(terra_build::BuildOp::Fill {
        min: cube.to_array(),
        max: cube.to_array(),
        cube: None,
    });
    let after = section(&edited);
    let started = Instant::now();
    let whole = sections::Whole::lay(&edited, kits);
    let whole_ms = started.elapsed().as_secs_f32() * 1000.0;
    let laid = sections::lay(&after, &whole);
    let started = Instant::now();
    std::hint::black_box(sections::draw(&after, &laid, Reach::Near));
    let afresh = started.elapsed().as_secs_f32() * 1000.0;
    // Drawn again as the game does: cut from the base laid whole, each part's key, and
    // only the parts whose key changed drawn, from how they were drawn.
    let mut now = sections::Drawing::default();
    let started = Instant::now();
    sections::remembered(|| {
        let laid = sections::lay(&after, &whole);
        let keys = sections::cluster_keys(&after, &laid, Reach::Near);
        let only: Vec<bool> = keys.iter().zip(&keys_before).map(|(a, b)| a != b).collect();
        std::hint::black_box(sections::draw_parts(
            &after,
            &laid,
            Reach::Near,
            Some(&last),
            Some(&mut now),
            Some(&only),
        ));
    });
    let again = started.elapsed().as_secs_f32() * 1000.0;
    Some(Edit {
        what: format!("{name}, section {:?}", at.to_array()),
        whole: whole_ms,
        afresh,
        again,
        taken: now.faces_taken(),
        faces: now.faces(),
    })
}

fn report_edits(edits: &mut [Edit]) {
    if edits.is_empty() {
        return;
    }
    let median = |values: &mut Vec<f32>| {
        values.sort_by(f32::total_cmp);
        values[values.len() / 2]
    };
    let mut whole: Vec<f32> = edits.iter().map(|e| e.whole).collect();
    let mut afresh: Vec<f32> = edits.iter().map(|e| e.afresh).collect();
    let mut again: Vec<f32> = edits.iter().map(|e| e.again).collect();
    println!(
        "An edit, a wall's cube taken out of each of {} buildings: laid whole again in a median {:.2} ms; its section drawn near afresh in {:.2}, from how it was drawn in {:.2}",
        edits.len(),
        median(&mut whole),
        median(&mut afresh),
        median(&mut again)
    );
    edits.sort_by(|a, b| b.afresh.total_cmp(&a.afresh));
    println!("  The slowest, afresh and again, and the faces kept:");
    for edit in edits.iter().take(4) {
        println!(
            "  {:6.2} {:6.2}  {} of {}  {}",
            edit.afresh, edit.again, edit.taken, edit.faces, edit.what
        );
    }
}

/// The time below which this share of sections were laid and merged.
fn total_at(times: &[(f32, f32)], share: f32) -> f32 {
    let mut total: Vec<f32> = times.iter().map(|t| t.1).collect();
    total.sort_by(f32::total_cmp);
    total
        .get(((total.len().max(1) - 1) as f32 * share) as usize)
        .copied()
        .unwrap_or(0.0)
}

/// Every kit under `assets/data/kits`, with its dressed pieces, as the game reads them.
fn read_kits(assets: &Path) -> Option<Kits> {
    let read = |path: &Path| {
        std::fs::read_to_string(path).map_err(|why| format!("{}: {why}", path.display()))
    };
    let mut paths: Vec<_> = std::fs::read_dir(assets.join("data/kits"))
        .map(|entries| entries.flatten().map(|entry| entry.path()).collect())
        .unwrap_or_default();
    paths.sort();
    let mut kits = Vec::new();
    for path in paths
        .into_iter()
        .filter(|p| p.extension().is_some_and(|e| e == "ron"))
    {
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let kit = Kit::read(
            &format!("{}/{name}", terra_build::kit::MATERIALS),
            &|file: &str| read(&assets.join(file)),
        );
        match kit {
            Ok(kit) => kits.push(kit),
            Err(why) => {
                println!("sections: {} does not read: {why}", path.display());
                return None;
            }
        }
    }
    Some(Kits(kits))
}

/// Height maps and swatches of the real ones' sizes, in a pattern, for every kit.
fn stand_in_maps(kits: &Kits) -> Maps {
    const SIDE: usize = 1024;
    let heights: Vec<u8> = (0..SIDE * SIDE)
        .map(|i| ((i * 2_654_435_761) >> 13) as u8)
        .collect();
    let relief = Arc::new(Relief::new(SIDE, SIDE, heights).expect("a full map"));
    let swatch = Arc::new(Swatch::new(16, 16, vec![[0.4, 0.35, 0.3]; 256]).expect("a swatch"));
    let mut maps = Maps::default();
    for kit in &kits.0 {
        for path in [shaped_by(kit), topped_by(kit)].into_iter().flatten() {
            maps.heights.insert(path.to_string(), relief.clone());
        }
        for texture in kit.textures.values() {
            maps.colours.insert(texture.path.clone(), swatch.clone());
        }
    }
    maps
}
