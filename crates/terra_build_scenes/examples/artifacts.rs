//! What a renderer would show as junk in what the kits draw, found from the meshes
//! (`terra_build_mesh::inspect`), `cargo xtask artifacts`: every scene of the registry
//! (`terra_build_scenes::registry`) in the building baseline's materials (D333), on sloping
//! ground, at every level of detail. Prints how many of each fault it found at each detail
//! and the first few of each (every one with `--all`), and fails if there is any. A name
//! given keeps only the scenes whose names hold it, in every material, as `"logs: rectangle"`.
//!
//! It is also a ratchet (D69): the faults found of each kind in each group of scenes (a
//! name's kits, or its family's, "sheet", "village") are kept in `artifacts-baseline/`
//! beside this file, a file a set and group (D414). `--ratchet` fails if any count is above
//! its baseline, so no change adds junk unnoticed; a count that falls passes and is left for
//! the full run on main (`cargo xtask nightly`) to lower (`--lower`, D414). `--raise "<why>"` writes the counts that
//! rose, with why the rise is meant. `--quick` inspects the scenes the registry gates before
//! a push (`Gate::Push`: some of every family but the castles, the states, the previews and
//! the example base),
//! which `cargo xtask check` ratchets before every push; the whole set is
//! `cargo xtask artifacts --ratchet`, which holds every scene registered.

use std::path::Path;
use std::process::ExitCode;
use terra_build_mesh::inspect::{self, Fault};
use terra_build_mesh::relief::Maps;
use terra_build_mesh::sections::Detail;
use terra_build_scenes::registry::{Gate, Materials, ratchet};

fn main() -> ExitCode {
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let args: Vec<String> = std::env::args().skip(1).collect();
    let all = args.iter().any(|a| a == "--all");
    let quick = args.iter().any(|a| a == "--quick");
    // A name, not a flag's number.
    let only = args
        .iter()
        .enumerate()
        .find(|&(i, a)| {
            !a.starts_with("--")
                && (i == 0 || !["--seeds", "--from", "--raise"].contains(&args[i - 1].as_str()))
        })
        .map(|(_, a)| a.clone())
        .filter(|_| !quick);
    // Every kit, which the bases are drawn by, and the scenes of the building baseline's
    // materials (D333), those the settlement is built of and the roofs they take, unless a
    // name asks for others: a frozen kit is still read, so a base of it can be asked for.
    let (every, scope) = match (
        Materials::read(&assets, false),
        Materials::read(&assets, only.is_none()),
    ) {
        (Ok(every), Ok(scope)) => (every, scope),
        (Err(why), _) | (_, Err(why)) => {
            println!("artifacts: {why}");
            return ExitCode::FAILURE;
        }
    };
    let kits = &every.kits;
    let maps = Maps::worst(kits, std::env::var("FLAT").is_ok());
    if args.iter().any(|a| a == "--fuzz") {
        let number = |flag: &str, or: u64| {
            args.iter()
                .position(|a| a == flag)
                .and_then(|i| args.get(i + 1))
                .and_then(|n| n.parse().ok())
                .unwrap_or(or)
        };
        let from = number("--from", 0);
        let seeds = from..from + number("--seeds", FUZZ_SEEDS);
        let hold = baseline::Hold::from(&args);
        if hold.is_some() && seeds != (0..FUZZ_SEEDS) {
            println!("fuzz: the baseline is of the check's seeds, 0 to {FUZZ_SEEDS}, not others");
            return ExitCode::FAILURE;
        }
        return fuzz(seeds, (&scope, &every), &maps, hold.as_ref());
    }
    // The scene registry's (`terra_build_scenes::registry`): before a push those it gates
    // so, else every one, or those whose names hold the name given.
    let gate = if quick { Gate::Push } else { Gate::Full };
    let cases: Vec<_> = ratchet(&scope, gate)
        .into_iter()
        .filter(|scene| only.as_ref().is_none_or(|only| scene.name.contains(only)))
        .collect();
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
                        let Some(scene) = cases.get(i) else {
                            break;
                        };
                        mine.push((i, inspect::inspect(&scene.base(), kits, &maps)));
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
        let group = group(&cases[*i].name);
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
                    cases[*i].name,
                    finding.at.x,
                    finding.at.y,
                    finding.at.z,
                    finding.what
                ));
            }
        }
    }
    println!(
        "artifacts: the meshes of {} scenes, {}, {}, {} and {} triangles near, mid, far and proxy, {rays} lines cast, in {:.1} s",
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
    let hold = baseline::Hold::from(&args);
    if hold.is_some() && set.is_empty() {
        println!("artifacts: the baseline is of every base or of --quick's, not of a name");
        return ExitCode::FAILURE;
    }
    if let Some(hold) = hold {
        return baseline::hold(set, &by_kit, &hold);
    }
    if total == 0 {
        println!("artifacts: meshes clean");
        ExitCode::SUCCESS
    } else {
        println!("artifacts: FAIL: {total} faults in the meshes");
        ExitCode::FAILURE
    }
}

/// The fuzz pass's seeds the full check holds to the baseline, 0 up to this: about 20 s on
/// a desktop's 32 threads.
const FUZZ_SEEDS: u64 = 200;

/// The fuzz pass (`terra_build_scenes::fuzz`, `cargo xtask fuzz`): a small base built at
/// random from each seed in the baseline's materials (`scope`), drawn by every kit, each
/// inspected on a thread of its own. A base with a fault, a sort of fault at a time, is
/// shrunk to the fewest elements still showing it and printed as a test to paste: every
/// one found, or, held to the baseline (`hold`, set "fuzz", a seed's faults of each kind),
/// only those of seeds whose counts rose, which fail it.
fn fuzz(
    seeds: std::ops::Range<u64>,
    (scope, every): (&Materials, &Materials),
    maps: &Maps,
    hold: Option<&baseline::Hold>,
) -> ExitCode {
    use terra_build_scenes::fuzz::{as_rust, build, elements, shrink};
    let kits = &every.kits;
    let started = std::time::Instant::now();
    let count = seeds.end - seeds.start;
    let next = std::sync::atomic::AtomicU64::new(seeds.start);
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let mut found: Vec<(u64, Vec<inspect::Finding>)> = std::thread::scope(|threads_scope| {
        let workers: Vec<_> = (0..threads)
            .map(|_| {
                threads_scope.spawn(|| {
                    let mut mine = Vec::new();
                    loop {
                        let seed = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        if seed >= seeds.end {
                            break;
                        }
                        let base = build(&scope.kits, &elements(seed, &scope.kits));
                        let report = inspect::inspect(&base, kits, maps);
                        if !report.findings.is_empty() {
                            mine.push((seed, report.findings));
                        }
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
    found.sort_by_key(|(seed, _)| *seed);
    println!(
        "fuzz: {count} seeds from {}, {} with faults, in {:.1} s",
        seeds.start,
        found.len(),
        started.elapsed().as_secs_f32()
    );
    let mut counts: std::collections::BTreeMap<(String, String), usize> = Default::default();
    for (seed, findings) in &found {
        for finding in findings {
            *counts
                .entry((format!("seed {seed}"), format!("{:?}", finding.fault)))
                .or_default() += 1;
        }
    }
    let recorded = if hold.is_some() {
        baseline::counts("fuzz")
    } else {
        Default::default()
    };
    let rose = |seed: u64| {
        counts.iter().any(|((at, fault), &now)| {
            *at == format!("seed {seed}")
                && now
                    > recorded
                        .get(&(at.clone(), fault.clone()))
                        .copied()
                        .unwrap_or(0)
        })
    };
    // Each sort of fault shrunk once, at the first seed showing it, the rest listed.
    let mut shrunk = std::collections::BTreeSet::new();
    let name = |kind: terra_build::Kind| every.name(kind);
    for (seed, findings) in found.iter().filter(|(seed, _)| rose(*seed)) {
        let first = &findings[0];
        println!(
            "  seed {seed}: {} faults, the first {:?}, {:?}: at ({:.2}, {:.2}, {:.2}): {}",
            findings.len(),
            first.fault,
            first.detail,
            first.at.x,
            first.at.y,
            first.at.z,
            first.what
        );
        // A sort is its fault and what it is in, the numbers left out: "Coplanar section Logs
        // LogEnd and section Logs Floor".
        let sort: String = format!("{:?} {}", first.fault, first.what)
            .split(':')
            .next()
            .unwrap_or_default()
            .chars()
            .filter(|c| !c.is_ascii_digit() && !"[],-".contains(*c))
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if !shrunk.insert(sort) {
            continue;
        }
        let fault = first.fault;
        let fails = |base: &terra_build::Base| {
            inspect::inspect(base, kits, maps)
                .findings
                .iter()
                .any(|f| f.fault == fault)
        };
        let all = elements(*seed, &scope.kits);
        let (kept, tries) = shrink(&all, &scope.kits, &fails);
        let base = build(&scope.kits, &kept);
        let left: Vec<String> = inspect::inspect(&base, kits, maps)
            .findings
            .iter()
            .filter(|f| f.fault == fault)
            .take(4)
            .map(|f| {
                format!(
                    "    //   {:?}, {:?}: at ({:.2}, {:.2}, {:.2}): {}",
                    f.fault, f.detail, f.at.x, f.at.y, f.at.z, f.what
                )
            })
            .collect();
        println!(
            "\n  shrunk from {} elements to {} in {tries} bases; as a test (crates/terra_build_scenes/src/fuzz/tests.rs):\n\n#[test]\nfn fuzz_seed_{seed}_is_drawn_without_{}() {{\n    // Found by `cargo xtask fuzz` at seed {seed}, shrunk:\n{}\n    let materials = baseline();\n    let kind = |name: &str| materials.names.iter().find(|(_, n)| n.as_str() == name).map(|(&k, _)| k).unwrap();\n    let base = build(&materials.kits, &{});\n    let report = terra_build_mesh::inspect::inspect(&base, &materials.kits, &terra_build_mesh::relief::Maps::worst(&materials.kits, false));\n    assert_eq!(report.count(terra_build_mesh::inspect::Fault::{fault:?}), 0);\n}}\n",
            all.len(),
            kept.len(),
            format!("{fault:?}").to_lowercase(),
            left.join("\n"),
            as_rust(&kept, &name).replace('\n', "\n    "),
        );
    }
    if let Some(hold) = hold {
        return baseline::hold("fuzz", &counts, hold);
    }
    if found.is_empty() {
        return ExitCode::SUCCESS;
    }
    println!(
        "fuzz: FAIL: {} of {count} seeds build a base with a fault",
        found.len()
    );
    ExitCode::FAILURE
}

/// What a scene's faults are counted under in the baseline: its name's kits, "stone" or
/// "logs with stone", or its family where it is of many, "sheet" or "village".
fn group(case: &str) -> String {
    case.split(':').next().unwrap_or(case).trim().to_string()
}

/// The ratchet's baseline (D414): the folder `artifacts-baseline/<set>/<group>.tsv` beside
/// this file, a file a set and group of scenes, so changes to different groups never touch one
/// file. A row a group and fault, tab-separated: group, fault, count and why the count is
/// what it is (`-` for none said). `--ratchet` holds a set to it; `--raise "<why>"` writes the
/// counts that rose or are new, with the why; `--lower` writes those that fell, and is the
/// full run on main's alone: `cargo xtask check` refuses a lowering made anywhere else.
mod baseline {
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::PathBuf;
    use std::process::ExitCode;

    type Counts = BTreeMap<(String, String), usize>;

    /// A set's rows: group and fault to count and why.
    type Rows = BTreeMap<(String, String), (usize, String)>;

    /// How a run ends with the baseline: held to it, or writing it first.
    #[derive(Clone, PartialEq)]
    pub enum Hold {
        Ratchet,
        Raise(String),
        Lower,
    }

    impl Hold {
        /// `--ratchet`, `--raise "<why>"` or `--lower` from the arguments, or none.
        pub fn from(args: &[String]) -> Option<Hold> {
            if let Some(i) = args.iter().position(|a| a == "--raise") {
                let why = args.get(i + 1).filter(|a| !a.starts_with("--"));
                return Some(Hold::Raise(why.cloned().unwrap_or_default()));
            }
            if args.iter().any(|a| a == "--lower") {
                return Some(Hold::Lower);
            }
            args.iter()
                .any(|a| a == "--ratchet")
                .then_some(Hold::Ratchet)
        }
    }

    fn folder(set: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("examples/artifacts-baseline")
            .join(set)
    }

    /// A group's file: "seed 103" is `seed-103.tsv`, "logs with stone" `logs-with-stone.tsv`.
    fn file(set: &str, group: &str) -> PathBuf {
        let slug = group
            .to_lowercase()
            .split(|c: char| !c.is_ascii_alphanumeric())
            .filter(|word| !word.is_empty())
            .collect::<Vec<_>>()
            .join("-");
        folder(set).join(format!("{slug}.tsv"))
    }

    /// A set's rows as recorded.
    fn read(set: &str) -> Rows {
        let mut rows = Rows::new();
        let Ok(entries) = std::fs::read_dir(folder(set)) else {
            return rows;
        };
        for entry in entries.flatten() {
            let text = std::fs::read_to_string(entry.path()).unwrap_or_default();
            for line in text
                .lines()
                .filter(|l| !l.starts_with('#') && !l.is_empty())
            {
                if let [group, fault, count, why] = line.split('\t').collect::<Vec<_>>()[..]
                    && let Ok(count) = count.parse()
                {
                    rows.insert((group.into(), fault.into()), (count, why.into()));
                }
            }
        }
        rows
    }

    /// A set's counts as recorded.
    pub fn counts(set: &str) -> Counts {
        read(set)
            .into_iter()
            .map(|(key, (count, _))| (key, count))
            .collect()
    }

    /// Writes the files of `groups` from `rows`, removing one left with no rows.
    fn write(set: &str, rows: &Rows, groups: &BTreeSet<&str>) -> Result<(), String> {
        std::fs::create_dir_all(folder(set)).map_err(|why| format!("{set}: {why}"))?;
        for group in groups {
            let path = file(set, group);
            let text: String = rows
                .iter()
                .filter(|((g, _), _)| file(set, g) == path)
                .map(|((g, fault), (count, why))| format!("{g}\t{fault}\t{count}\t{why}\n"))
                .collect();
            let done = match (text.is_empty(), path.exists()) {
                (true, true) => std::fs::remove_file(&path),
                (true, false) => Ok(()),
                (false, _) => std::fs::write(&path, text),
            };
            done.map_err(|why| format!("{}: {why}", path.display()))?;
        }
        Ok(())
    }

    /// A count that changed: group and fault, what it was and what it is.
    type Change = ((String, String), usize, usize);

    /// Holds `counts` to the set's baseline and fails if any rose past it, after `--raise`
    /// has written those that rose, or `--lower` those that fell.
    pub fn hold(set: &str, counts: &Counts, how: &Hold) -> ExitCode {
        let mut rows = read(set);
        let (mut rose, mut fell): (Vec<Change>, Vec<Change>) = Default::default();
        let keys: BTreeSet<(String, String)> = counts.keys().chain(rows.keys()).cloned().collect();
        for key in keys {
            let now = counts.get(&key).copied().unwrap_or(0);
            let was = rows.get(&key).map_or(0, |(count, _)| *count);
            if now > was {
                rose.push((key, was, now));
            } else if now < was {
                fell.push((key, was, now));
            }
        }
        let command = match set {
            "quick" => "cargo xtask artifacts --quick",
            "fuzz" => "cargo xtask fuzz",
            _ => "cargo xtask artifacts",
        };
        let written = match how {
            Hold::Ratchet => Vec::new(),
            Hold::Raise(why) => {
                let why = why.split_whitespace().collect::<Vec<_>>().join(" ");
                if why.is_empty() || why == "-" {
                    println!(
                        "artifacts: FAIL: a raise says why: `{command} --raise \"<why the rise is meant>\"`"
                    );
                    return ExitCode::FAILURE;
                }
                if rose.is_empty() {
                    println!("artifacts: FAIL: nothing rose past the baseline ({set}) to raise");
                    return ExitCode::FAILURE;
                }
                for (key, _, now) in &rose {
                    rows.insert(key.clone(), (*now, why.clone()));
                }
                std::mem::take(&mut rose)
            }
            Hold::Lower => {
                for (key, _, now) in &fell {
                    match rows.get_mut(key) {
                        Some(row) if *now > 0 => row.0 = *now,
                        _ => {
                            rows.remove(key);
                        }
                    }
                }
                std::mem::take(&mut fell)
            }
        };
        if !written.is_empty() {
            let groups: BTreeSet<&str> = written.iter().map(|((g, _), _, _)| g.as_str()).collect();
            if let Err(why) = write(set, &rows, &groups) {
                println!("artifacts: {why}");
                return ExitCode::FAILURE;
            }
            for ((group, fault), was, now) in &written {
                println!("  written: {group} {fault}: {was} to {now}");
            }
            println!(
                "artifacts: {} counts of {set} written to examples/artifacts-baseline/{set}",
                written.len()
            );
        }
        for ((group, fault), was, now) in &rose {
            println!("  MORE:  {group} {fault}: {was} to {now}");
        }
        if !rose.is_empty() {
            println!(
                "artifacts: FAIL: {} counts of junk in building's meshes rose past the baseline (examples/artifacts-baseline/{set}); `{command}` lists them. Fix them, or if the rise is meant, `{command} --raise \"<why>\"`",
                rose.len()
            );
            return ExitCode::FAILURE;
        }
        if !fell.is_empty() {
            println!(
                "artifacts: {} counts fell; the baseline is left as it is, the full run on main (`cargo xtask nightly`) lowers it (D414)",
                fell.len()
            );
        }
        println!("artifacts: ratchet holds ({set})");
        ExitCode::SUCCESS
    }
}
