//! What a renderer would show as junk in what the kits draw, found from the meshes
//! (`terra_build::inspect`), `cargo xtask artifacts`: every roof over every plan in
//! each wall kind and every two kinds mixed, on sloping ground, at every level of detail.
//! Prints how many of each fault it found at each detail and the first few of each (every
//! one with `--all`), and fails if there is any. A name given keeps only the bases whose
//! names hold it, as `"1: rectangle"`.
//!
//! It is also a ratchet (D69): the faults found of each kind in each kit's bases, and each
//! two kits' mixed, are kept in `artifacts-baseline.tsv` beside this file. `--ratchet` fails
//! if any count is above its baseline, so no change adds junk unnoticed, and says which have
//! dropped; `--record` writes the counts as the new baseline, to commit as fixes land.
//! `--quick` inspects each kit's and each two kits' rectangle under a gable, and each kit's
//! QA cabin, junctions and roofs and walls, the bases the voxel contract's rows cite, which
//! `cargo xtask check` ratchets before every push; the whole set, with every roof over every
//! plan and the castle, is `cargo xtask artifacts --ratchet`.

use std::path::Path;
use std::process::ExitCode;
use std::sync::Arc;
use terra_build::inspect::{self, Fault};
use terra_build::kit::{Kit, Kits};
use terra_build::relief::{Maps, Relief, Swatch, cored_by, shaped_by, topped_by};
use terra_build::sections::Detail;

fn main() -> ExitCode {
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let Some((kits, names)) = read_kits(&assets) else {
        return ExitCode::FAILURE;
    };
    let maps = stand_in_maps(&kits);
    let args: Vec<String> = std::env::args().skip(1).collect();
    let all = args.iter().any(|a| a == "--all");
    let quick = args.iter().any(|a| a == "--quick");
    let only = args
        .iter()
        .find(|a| !a.starts_with("--"))
        .cloned()
        .filter(|_| !quick);
    // The bases of the building baseline's materials (D333), those the settlement is built
    // of and the roofs they take, unless a name asks for others; a frozen kit is still read,
    // so a base of it can be asked for by name.
    let named = only.is_some();
    let Some(scope) = (if named {
        Some(kits.clone())
    } else {
        baseline_kits(&assets, &kits, &names)
    }) else {
        return ExitCode::FAILURE;
    };
    let mut cases: Vec<(String, terra_build::Base)> = if quick {
        QUICK
            .iter()
            .flat_map(|family| inspect::cases(&scope, Some(family)))
            .collect()
    } else {
        inspect::cases(&scope, only.as_deref())
    };
    // And one of each sort of the village's buildings, in the settlement's own materials, in
    // every set: the shipped scene the game builds most of.
    let Some(village) = village(&assets, &kits, &names) else {
        return ExitCode::FAILURE;
    };
    cases.extend(
        village
            .into_iter()
            .filter(|(name, _)| quick || only.as_ref().is_none_or(|only| name.contains(only))),
    );
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
        Fault::Black,
        Fault::Inward,
        Fault::Coplanar,
        Fault::Doubled,
        Fault::SeeThrough,
        Fault::UnderRoof,
        Fault::PokesOut,
        Fault::Buried,
    ];
    let details = [Detail::Near, Detail::Mid, Detail::Far, Detail::Proxy];
    let mut counts = [[0usize; 4]; 9];
    let (mut triangles, mut rays) = ([0usize; 4], 0usize);
    // The first few of each fault at each detail, or every one with --all.
    let mut shown = [[0usize; 4]; 9];
    let mut lines = Vec::new();
    // The faults of each kind in each kit's bases, or each two kits' mixed.
    let mut by_kit: std::collections::BTreeMap<(String, String), usize> = Default::default();
    for (i, report) in &reports {
        let group = group(&cases[*i].0, &names);
        for finding in &report.findings {
            *by_kit
                .entry((group.clone(), format!("{:?}", finding.fault)))
                .or_default() += 1;
        }
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
        "a surface drawn black whatever the light",
        "a surface drawn from inside, dark or see-through",
        "two surfaces flickering in one plane (z-fighting)",
        "a piece drawn twice",
        "sky or sun through a wall or a roof",
        "a gap between a wall's top and the roof",
        "a piece poking through a roof, past its edge or out of a wall",
        "a piece sunk under a surface's relief, showing in dashes",
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
    let set = if quick {
        "quick"
    } else if only.is_none() {
        "all"
    } else {
        ""
    };
    let ratchet = args.iter().any(|a| a == "--ratchet");
    let record = args.iter().any(|a| a == "--record");
    if (ratchet || record) && set.is_empty() {
        println!("artifacts: the baseline is of every base or of --quick's, not of a name");
        return ExitCode::FAILURE;
    }
    if record {
        return match baseline::record(set, &by_kit) {
            Ok(()) => {
                println!("artifacts: baseline of {set} recorded, {total} faults");
                ExitCode::SUCCESS
            }
            Err(why) => {
                println!("artifacts: {why}");
                ExitCode::FAILURE
            }
        };
    }
    if ratchet {
        return baseline::ratchet(set, &by_kit);
    }
    if total == 0 {
        println!("artifacts: meshes clean");
        ExitCode::SUCCESS
    } else {
        println!("artifacts: FAIL: {total} faults in the meshes");
        ExitCode::FAILURE
    }
}

/// The bases `--quick` inspects, the ratchet `cargo xtask check` runs before a push (D287):
/// each kit's and each two kits' rectangle under a gable; and in each kit, the QA cabin, the
/// junctions family and the roofs-and-walls family, whose bases are the proof of rows of the
/// voxel contract's junction matrix (docs/building-voxels.md); and the village's buildings,
/// which every set holds (`village`). Not the castle, nor every roof over every plan, which
/// `cargo xtask artifacts --ratchet` inspects.
const QUICK: [&str; 4] = [
    "rectangle, gable roof",
    ": QA cabin",
    ": junction, ",
    ": roofs and walls: ",
];

/// The kits a base is of, by their files' names, from its name: "stone", "logs with stone".
fn group(case: &str, names: &std::collections::BTreeMap<u16, String>) -> String {
    let ids = case.split(':').next().unwrap_or(case);
    ids.split(" with ")
        .map(|id| {
            id.trim()
                .parse::<u16>()
                .ok()
                .and_then(|id| names.get(&id).cloned())
                .unwrap_or_else(|| id.to_string())
        })
        .collect::<Vec<_>>()
        .join(" with ")
}

/// The ratchet's baseline: a line a set, kits and fault, its count, tab-separated.
mod baseline {
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    use std::process::ExitCode;

    fn path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/artifacts-baseline.tsv")
    }

    type Counts = BTreeMap<(String, String), usize>;

    /// Every set's counts as recorded.
    fn read() -> BTreeMap<String, Counts> {
        let mut out: BTreeMap<String, Counts> = BTreeMap::new();
        let text = std::fs::read_to_string(path()).unwrap_or_default();
        for line in text
            .lines()
            .filter(|l| !l.starts_with('#') && !l.is_empty())
        {
            let cells: Vec<&str> = line.split('\t').collect();
            if let [set, kits, fault, count] = cells[..]
                && let Ok(count) = count.parse()
            {
                out.entry(set.to_string())
                    .or_default()
                    .insert((kits.to_string(), fault.to_string()), count);
            }
        }
        out
    }

    pub fn record(set: &str, counts: &Counts) -> Result<(), String> {
        let mut all = read();
        all.insert(set.to_string(), counts.clone());
        let mut text = String::from(
            "# The faults `cargo xtask artifacts` finds in building's meshes, of each kind in each kit's\n# bases: the ratchet's baseline. `cargo xtask artifacts --record` (every base) and\n# `--quick --record` (the bases `cargo xtask check` inspects) write it.\n# set\tkits\tfault\tcount\n",
        );
        for (set, counts) in &all {
            for ((kits, fault), count) in counts {
                text.push_str(&format!("{set}\t{kits}\t{fault}\t{count}\n"));
            }
        }
        std::fs::write(path(), text).map_err(|why| format!("{}: {why}", path().display()))
    }

    /// Fails if any count is above its baseline; says which are below it.
    pub fn ratchet(set: &str, counts: &Counts) -> ExitCode {
        let recorded = read().remove(set).unwrap_or_default();
        let (mut rose, mut fell) = (Vec::new(), Vec::new());
        let keys: std::collections::BTreeSet<&(String, String)> =
            counts.keys().chain(recorded.keys()).collect();
        for key in keys {
            let now = counts.get(key).copied().unwrap_or(0);
            let was = recorded.get(key).copied().unwrap_or(0);
            let line = format!("{} {}: {was} to {now}", key.0, key.1);
            if now > was {
                rose.push(line);
            } else if now < was {
                fell.push(line);
            }
        }
        for line in &fell {
            println!("  fewer: {line}");
        }
        for line in &rose {
            println!("  MORE:  {line}");
        }
        let flag = if set == "quick" { " --quick" } else { "" };
        if !rose.is_empty() {
            println!(
                "artifacts: FAIL: {} counts of junk in building's meshes rose past the baseline (examples/artifacts-baseline.tsv); `cargo xtask artifacts{flag}` lists them. Fix them, or if the rise is meant, `cargo xtask artifacts{flag} --record` and say why in the commit",
                rose.len()
            );
            return ExitCode::FAILURE;
        }
        if !fell.is_empty() {
            println!(
                "artifacts: {} counts fell: lower the baseline with `cargo xtask artifacts{flag} --record` and commit it",
                fell.len()
            );
        }
        println!("artifacts: ratchet holds ({set})");
        ExitCode::SUCCESS
    }
}

/// One of each sort of the village's buildings in the settlement's materials
/// (`village::each_sort`), each named by the wall kinds it is built of, as a kit's bases are:
/// "2 with 3: village, a masonry and timber house of two storeys under a gable".
fn village(
    assets: &Path,
    kits: &Kits,
    names: &std::collections::BTreeMap<u16, String>,
) -> Option<Vec<(String, terra_build::Base)>> {
    let read = |path: &str| {
        std::fs::read_to_string(assets.join(path)).map_err(|why| format!("{path}: {why}"))
    };
    let named = |name: &str| {
        names
            .iter()
            .find(|(_, file)| file.as_str() == name)
            .and_then(|(&id, _)| kits.0.iter().find(|kit| kit.id == id))
            .map(|kit| kit.kind())
    };
    let palette = match terra_build::village::Palette::read(&read, &named, kits) {
        Ok(palette) => palette,
        Err(why) => {
            println!("artifacts: the village's materials: {why}");
            return None;
        }
    };
    Some(
        terra_build::village::each_sort(&palette)
            .into_iter()
            .map(|(building, base)| {
                let walls: std::collections::BTreeSet<u16> = base
                    .kinds()
                    .filter(|kind| !kind.is_roof())
                    .map(|kind| kind.0)
                    .collect();
                let ids: Vec<String> = walls.iter().map(u16::to_string).collect();
                (
                    format!("{}: village, {}", ids.join(" with "), building.name),
                    base,
                )
            })
            .collect(),
    )
}

/// The building baseline's kits (D333): the wall materials the settlement is built of
/// (`data/settlement.ron`) and the roofs over them.
fn baseline_kits(
    assets: &Path,
    kits: &Kits,
    names: &std::collections::BTreeMap<u16, String>,
) -> Option<Kits> {
    let text = std::fs::read_to_string(assets.join(terra_build::village::SETTLEMENT)).ok()?;
    let settlement: terra_build::village::Settlement = match ron::from_str(&text) {
        Ok(settlement) => settlement,
        Err(why) => {
            println!("artifacts: {}: {why}", terra_build::village::SETTLEMENT);
            return None;
        }
    };
    let walls: std::collections::BTreeSet<&String> = [
        &settlement.timber,
        &settlement.masonry,
        &settlement.beams,
        &settlement.cob,
        &settlement.framed,
    ]
    .into_iter()
    .flatten()
    .collect();
    let wall = |kit: &Kit| names.get(&kit.id).is_some_and(|name| walls.contains(name));
    let roofs: std::collections::BTreeSet<_> = kits
        .0
        .iter()
        .filter(|kit| wall(kit))
        .filter_map(|kit| kits.roof_for(kit.kind()))
        .collect();
    Some(Kits(
        kits.0
            .iter()
            .filter(|kit| wall(kit) || roofs.contains(&kit.kind()))
            .cloned()
            .collect(),
    ))
}

/// Every kit under `assets/data/kits`, with its dressed pieces, as the game reads them, and
/// each one's file name by its id.
fn read_kits(assets: &Path) -> Option<(Kits, std::collections::BTreeMap<u16, String>)> {
    let read = |path: &Path| {
        std::fs::read_to_string(path).map_err(|why| format!("{}: {why}", path.display()))
    };
    let mut paths: Vec<_> = std::fs::read_dir(assets.join("data/kits"))
        .map(|entries| entries.flatten().map(|entry| entry.path()).collect())
        .unwrap_or_default();
    paths.sort();
    let mut kits = Vec::new();
    let mut names = std::collections::BTreeMap::new();
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
            Ok(kit) => {
                if let Some(stem) = path.file_stem() {
                    names.insert(kit.id, stem.to_string_lossy().to_string());
                }
                kits.push(kit);
            }
            Err(why) => {
                println!("artifacts: {} does not read: {why}", path.display());
                return None;
            }
        }
    }
    Some((Kits(kits), names))
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
