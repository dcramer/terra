//! How evenly each kit's textures lie on what it draws, found from the meshes with no GPU
//! (D69, D71), `cargo xtask texels`: a texture stretched across a rounded edge, squeezed
//! along a module, or smeared down a face mapped from the wrong side shows as streaks a
//! player sees at two metres, and no mesh fault catches it.
//!
//! For every triangle drawn near, the texture's metres on the world are measured along its
//! two directions (the Jacobian from texture to world, divided by the metres a copy covers
//! as the kit names them): a texture drawn twice as large as made along one of them, as a
//! plank's one column smeared down its side or a top's mapping carried round a rounded edge
//! is, is stretched 2:1. A texture drawn smaller than made, squeezed across a narrow face,
//! is not counted: it is finer, not smeared. Where a piece's own UVs place it (a modelled
//! piece's atlas, a sawn end), there are no metres to measure against, so its two directions
//! are compared with each other. Prints, for each kit and material, its area, the share of
//! it stretched past 1.5:1 and past 2.5:1, and where the worst of it is, in the base's own
//! metres, with which way those triangles face; then each modelled module as baked, before
//! it is laid, so a stretch is told apart from one the laying makes. Every roof over every
//! plan in each wall kind and every two kinds mixed, as `cargo xtask artifacts` lays them;
//! a name given keeps only the bases whose names hold it, as `"1: rectangle"`. `--worst N`
//! lists N places.
//!
//! Glossary: a stretch of 1 is a texture drawn as it was made; real stone or timber drawn
//! past about 1.5 reads as smeared.

use glam::{IVec3, Vec3};
use std::collections::BTreeMap;
use std::path::Path;
use std::process::ExitCode;
use std::sync::Arc;
use terra_build::kit::{Kit, Kits};
use terra_build::relief::{Maps, Relief, Swatch, cored_by, shaped_by, topped_by};
use terra_build::sections::{self, Progress, Reach, Section};
use terra_build::{Material, Surface};

/// Stretched past this reads as smeared; past the second, as a streak.
const SMEARED: f32 = 1.5;
const STREAKED: f32 = 2.5;

#[derive(Default)]
struct Tally {
    area: f32,
    smeared: f32,
    streaked: f32,
    /// Streaked area by half-metre cell, with the sum of its triangles' normals.
    places: BTreeMap<(String, [i32; 3]), (f32, Vec3, f32)>,
}

fn main() -> ExitCode {
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let Some((kits, names)) = read_kits(&assets) else {
        return ExitCode::FAILURE;
    };
    let maps = stand_in_maps(&kits);
    let args: Vec<String> = std::env::args().skip(1).collect();
    let worst = args
        .iter()
        .position(|a| a == "--worst")
        .and_then(|i| args.get(i + 1))
        .and_then(|n| n.parse().ok())
        .unwrap_or(3usize);
    let only = args
        .iter()
        .enumerate()
        .find(|(i, a)| !a.starts_with("--") && (*i == 0 || args[i - 1] != "--worst"))
        .map(|(_, a)| a.clone());
    let cases = terra_build::inspect::cases(&kits, only.as_deref());
    let name = |kind: terra_build::Kind| names.get(&kind.0).cloned().unwrap_or(kind.0.to_string());
    let mut tallies: BTreeMap<(String, String), Tally> = BTreeMap::new();
    for (case, base) in &cases {
        let whole = sections::Whole::lay(base, &kits);
        let progress = Progress::Whole;
        for at in sections::drawable(base) {
            let section = Section {
                base,
                kits: &kits,
                maps: &maps,
                at: IVec3::from_array(at),
                progress: &progress,
            };
            let laid = sections::lay(&section, &whole);
            let near = sections::draw(&section, &laid, Reach::Near);
            for ((kind, material), surface) in &near.near {
                let metres = kits.of(*kind).and_then(|kit| expected(kit, *material));
                let tally = tallies
                    .entry((name(*kind), format!("{material:?}")))
                    .or_default();
                measure(surface, metres, case, tally);
            }
        }
    }
    println!(
        "{:<16} {:<10} {:>9} {:>9} {:>9}",
        "kit", "material", "area m²", ">1.5:1", ">2.5:1"
    );
    for ((kit, material), tally) in &tallies {
        if tally.area < 1e-3 {
            continue;
        }
        println!(
            "{kit:<16} {material:<10} {:>9.1} {:>8.1}% {:>8.1}%",
            tally.area,
            100.0 * tally.smeared / tally.area,
            100.0 * tally.streaked / tally.area
        );
        let mut places: Vec<_> = tally.places.iter().collect();
        places.sort_by(|a, b| b.1.0.total_cmp(&a.1.0));
        for ((case, cell), (area, normal, stretch)) in places.into_iter().take(worst) {
            let n = normal.normalize_or_zero();
            println!(
                "    {:.2} m² at {:.1},{:.1},{:.1} facing {:.1},{:.1},{:.1}, up to {:.1}:1, in {case}",
                area,
                cell[0] as f32 * 0.5,
                cell[1] as f32 * 0.5,
                cell[2] as f32 * 0.5,
                n.x,
                n.y,
                n.z,
                stretch
            );
        }
    }
    println!("{} bases", cases.len());
    // The modelled modules as they were baked, before any is laid: a stretch here is in the
    // piece's own UVs, not in how it is laid.
    println!("\nmodules as modelled (each kit's own pieces and its pattern's):");
    let mut seen = std::collections::BTreeSet::new();
    for kit in &kits.0 {
        let Some(pieces) = kit.dress.made.as_deref() else {
            continue;
        };
        for (module_name, module) in &pieces.modules {
            let Some(stone) = module.near.first().filter(|s| !s.uvs.is_empty()) else {
                continue;
            };
            if !seen.insert((module_name.clone(), stone.positions.len(), stone.uvs.len())) {
                continue;
            }
            let (mut area, mut smeared) = (0.0, 0.0);
            for t in &stone.triangles {
                let [p, q, r] = t.map(|i| Vec3::from_array(stone.positions[i as usize]));
                let [u, v, w] = t.map(|i| stone.uvs[i as usize]);
                let a = 0.5 * (q - p).cross(r - p).length();
                area += a;
                if stretch(p, q, r, u, v, w) > SMEARED {
                    smeared += a;
                }
            }
            println!(
                "{:<16} {module_name:<14} {:>6.2} m² {:>6.1}% past 1.5:1",
                name(kit.kind()),
                area,
                100.0 * smeared / area.max(1e-6)
            );
        }
    }
    ExitCode::SUCCESS
}

/// How far a triangle's texture is drawn larger than made along its most stretched way, and
/// along its least: the singular values of the map from its texture to the world.
fn spread(p: Vec3, q: Vec3, r: Vec3, s: [f32; 2], u: [f32; 2], v: [f32; 2]) -> (f32, f32) {
    let (d1, d2) = ([u[0] - s[0], u[1] - s[1]], [v[0] - s[0], v[1] - s[1]]);
    let det = d1[0] * d2[1] - d1[1] * d2[0];
    if det.abs() < 1e-12 {
        // Every corner on one line of the texture: as stretched as can be.
        return (f32::INFINITY, 0.0);
    }
    // World along the texture's two directions: e = J d, so J = E D⁻¹.
    let (e1, e2) = (q - p, r - p);
    let ju = (e1 * d2[1] - e2 * d1[1]) / det;
    let jv = (e2 * d1[0] - e1 * d2[0]) / det;
    // Singular values of [ju jv], from its Gram matrix.
    let (aa, bb, ab): (f32, f32, f32) = (ju.dot(ju), jv.dot(jv), ju.dot(jv));
    let mean = 0.5 * (aa + bb);
    let gap = (0.25 * (aa - bb) * (aa - bb) + ab * ab).sqrt();
    ((mean + gap).sqrt(), (mean - gap).max(0.0).sqrt())
}

/// How much longer its texture is drawn one way than the other.
fn stretch(p: Vec3, q: Vec3, r: Vec3, s: [f32; 2], u: [f32; 2], v: [f32; 2]) -> f32 {
    let (big, small) = spread(p, q, r, s, u, v);
    if small < 1e-9 {
        f32::INFINITY
    } else {
        big / small
    }
}

/// The metres one copy of a material's texture covers on a kit, where its texture is laid
/// by them; none where its own UVs place it (a modelled piece's atlas, a sawn end's cell).
fn expected(kit: &Kit, material: Material) -> Option<[f32; 2]> {
    if matches!(
        material,
        Material::Piece | Material::Trim | Material::LogEnd
    ) {
        return None;
    }
    kit.textures.get(&material).map(|texture| texture.metres)
}

fn measure(surface: &Surface, metres: Option<[f32; 2]>, case: &str, tally: &mut Tally) {
    let all = surface.indexed();
    for t in all.indices.chunks_exact(3) {
        let [a, b, c] = [t[0], t[1], t[2]].map(|i| i as usize);
        let (p, q, r) = (all.positions[a], all.positions[b], all.positions[c]);
        let area = 0.5 * (q - p).cross(r - p).length();
        if area < 1e-7 {
            continue;
        }
        let stretch = match metres {
            Some(scale) => {
                let uv = |i: usize| [all.uvs[i][0] * scale[0], all.uvs[i][1] * scale[1]];
                spread(p, q, r, uv(a), uv(b), uv(c)).0
            }
            None => stretch(p, q, r, all.uvs[a], all.uvs[b], all.uvs[c]),
        };
        tally.area += area;
        if stretch > SMEARED {
            tally.smeared += area;
        }
        if stretch > STREAKED {
            tally.streaked += area;
            let centre = (p + q + r) / 3.0;
            let cell = (centre * 2.0).floor().as_ivec3().to_array();
            let normal = (q - p).cross(r - p).normalize_or_zero();
            let place =
                tally
                    .places
                    .entry((case.to_string(), cell))
                    .or_insert((0.0, Vec3::ZERO, 0.0));
            place.0 += area;
            place.1 += normal * area;
            place.2 = place.2.max(stretch.min(99.0));
        }
    }
}

fn read_kits(assets: &Path) -> Option<(Kits, BTreeMap<u16, String>)> {
    let read = |path: &Path| {
        std::fs::read_to_string(path).map_err(|why| format!("{}: {why}", path.display()))
    };
    let mut paths: Vec<_> = std::fs::read_dir(assets.join("data/kits"))
        .map(|entries| entries.flatten().map(|entry| entry.path()).collect())
        .unwrap_or_default();
    paths.sort();
    let mut kits = Vec::new();
    let mut names = BTreeMap::new();
    for path in paths
        .into_iter()
        .filter(|p| p.extension().is_some_and(|e| e == "ron"))
    {
        let file = path.file_name()?.to_string_lossy().to_string();
        match Kit::read(
            &format!("{}/{file}", terra_build::kit::MATERIALS),
            &|file: &str| read(&assets.join(file)),
        ) {
            Ok(kit) => {
                if let Some(stem) = path.file_stem() {
                    names.insert(kit.id, stem.to_string_lossy().to_string());
                }
                kits.push(kit);
            }
            Err(why) => {
                println!("texels: {} does not read: {why}", path.display());
                return None;
            }
        }
    }
    Some((Kits(kits), names))
}

/// Flat height maps: relief moves points along their normals, which stretches no texture,
/// so it is left out to measure the mapping alone.
fn stand_in_maps(kits: &Kits) -> Maps {
    const SIDE: usize = 64;
    let relief = Arc::new(Relief::new(SIDE, SIDE, vec![128; SIDE * SIDE]).expect("a full map"));
    let swatch = Arc::new(Swatch::new(16, 16, vec![[0.4, 0.35, 0.3]; 256]).expect("a swatch"));
    let mut maps = Maps::default();
    for kit in &kits.0 {
        for path in [shaped_by(kit), topped_by(kit), cored_by(kit)]
            .into_iter()
            .flatten()
        {
            maps.heights.insert(path.to_string(), relief.clone());
        }
        for texture in kit.textures.values() {
            maps.colours.insert(texture.path.clone(), swatch.clone());
        }
    }
    maps
}
