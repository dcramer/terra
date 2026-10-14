//! What a renderer would show as junk in what the kits draw, found from the meshes
//! (`terra_build::inspect`), `cargo xtask artifacts`: every roof over every plan in
//! each wall kind and every two kinds mixed, on sloping ground, at every level of detail.
//! Prints how many of each fault it found at each detail and the first few of each (every
//! one with `--all`), and fails if there is any. A name given keeps only the bases whose
//! names hold it, as `"1: rectangle"`.

use std::path::Path;
use std::process::ExitCode;
use std::sync::Arc;
use terra_build::inspect::{self, Fault};
use terra_build::kit::{Kit, Kits};
use terra_build::relief::{Maps, Relief, Swatch, cored_by, shaped_by, topped_by};
use terra_build::sections::Detail;

fn main() -> ExitCode {
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let Some(kits) = read_kits(&assets) else {
        return ExitCode::FAILURE;
    };
    let maps = stand_in_maps(&kits);
    let args: Vec<String> = std::env::args().skip(1).collect();
    let all = args.iter().any(|a| a == "--all");
    let only = args.iter().find(|a| !a.starts_with("--")).cloned();
    let cases = inspect::cases(&kits, only.as_deref());
    let started = std::time::Instant::now();
    // Every base on a thread of its own, as many at once as there are cores.
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let next = std::sync::atomic::AtomicUsize::new(0);
    let reports: Vec<(usize, inspect::Report)> = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..threads)
            .map(|_| {
                scope.spawn(|| {
                    let mut mine = Vec::new();
                    loop {
                        let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some((_, base)) = cases.get(i) else {
                            break;
                        };
                        mine.push((i, inspect::inspect(base, &kits, &maps)));
                    }
                    mine
                })
            })
            .collect();
        workers
            .into_iter()
            .flat_map(|w| w.join().unwrap_or_default())
            .collect()
    });
    let mut reports = reports;
    reports.sort_by_key(|(i, _)| *i);
    let faults = [
        Fault::NotFinite,
        Fault::Coplanar,
        Fault::Doubled,
        Fault::SeeThrough,
        Fault::UnderRoof,
        Fault::PokesOut,
    ];
    let details = [Detail::Near, Detail::Mid, Detail::Far, Detail::Proxy];
    let mut counts = [[0usize; 4]; 6];
    let (mut triangles, mut rays) = ([0usize; 4], 0usize);
    // The first few of each fault at each detail, or every one with --all.
    let mut shown = [[0usize; 4]; 6];
    let mut lines = Vec::new();
    for (i, report) in &reports {
        for (t, n) in triangles.iter_mut().zip(report.triangles) {
            *t += n;
        }
        rays += report.rays;
        for finding in &report.findings {
            let f = faults.iter().position(|&f| f == finding.fault).unwrap_or(0);
            let d = details
                .iter()
                .position(|&d| d == finding.detail)
                .unwrap_or(0);
            counts[f][d] += 1;
            shown[f][d] += 1;
            if all || shown[f][d] <= 4 {
                lines.push(format!(
                    "  {:?}, {:?}: {}: at ({:.2}, {:.2}, {:.2}): {}",
                    finding.fault,
                    finding.detail,
                    cases[*i].0,
                    finding.at.x,
                    finding.at.y,
                    finding.at.z,
                    finding.what
                ));
            }
        }
    }
    println!(
        "artifacts: the meshes of {} bases (every roof over every plan in each wall kind, and every two kinds mixed), {}, {}, {} and {} triangles near, mid, far and proxy, {rays} lines cast, in {:.1} s",
        cases.len(),
        triangles[0],
        triangles[1],
        triangles[2],
        triangles[3],
        started.elapsed().as_secs_f32()
    );
    println!(
        "  {:<12} {:>6} {:>6} {:>6} {:>6}   what it looks like",
        "found", "near", "mid", "far", "proxy"
    );
    let looks = [
        "black pixels that spread",
        "two surfaces flickering in one plane (z-fighting)",
        "a piece drawn twice",
        "sky or sun through a wall or a roof",
        "a gap between a wall's top and the roof",
        "a piece poking through a roof, past its edge or out of a wall",
    ];
    for (f, fault) in faults.iter().enumerate() {
        let c = counts[f];
        println!(
            "  {:<12} {:>6} {:>6} {:>6} {:>6}   {}",
            format!("{fault:?}"),
            c[0],
            c[1],
            c[2],
            c[3],
            looks[f]
        );
    }
    if !lines.is_empty() {
        println!(
            "The first of each{} (places in metres in the base's own space, east, up, south):",
            if all { "" } else { ", --all for every one" }
        );
        for line in &lines {
            println!("{line}");
        }
    }
    let total: usize = counts.iter().flatten().sum();
    if total == 0 {
        println!("artifacts: meshes clean");
        ExitCode::SUCCESS
    } else {
        println!("artifacts: FAIL: {total} faults in the meshes");
        ExitCode::FAILURE
    }
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
        let kit = read(&path)
            .and_then(|text| ron::from_str::<Kit>(&text).map_err(|why| why.to_string()))
            .and_then(|mut kit| {
                kit.dress.read(&|file| read(&assets.join(file)))?;
                Ok(kit)
            });
        match kit {
            Ok(kit) => kits.push(kit),
            Err(why) => {
                println!("artifacts: {} does not read: {why}", path.display());
                return None;
            }
        }
    }
    Some(Kits(kits))
}

/// Height maps at their full range everywhere, a texel's noise, so every surface is pushed
/// as far out and in as its kit lets it: the worst case for a hole.
fn stand_in_maps(kits: &Kits) -> Maps {
    const SIDE: usize = 256;
    let heights: Vec<u8> = (0..SIDE * SIDE)
        .map(|i| {
            if std::env::var("FLAT").is_ok() {
                128
            } else {
                ((i * 2_654_435_761) >> 13) as u8
            }
        })
        .collect();
    let relief = Arc::new(Relief::new(SIDE, SIDE, heights).expect("a full map"));
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
