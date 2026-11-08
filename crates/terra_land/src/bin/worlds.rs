//! The seed sweep (v0 seam 2, D596): the world each of many seeds makes, every core at once
//! (`terra_gen::par::map`), each found valid or not by its survey (`terra_land::survey`: a
//! level start, forest and tundra reached on foot) and fingerprinted (`terra_land::fingerprint`)
//! so another run, in another process or on another machine, is seen to make the same worlds
//! bit for bit (`--against` its table). `cargo xtask worlds` runs it; `--help` says what it
//! takes.

use std::fmt::Write as _;
use std::path::PathBuf;
use std::time::Instant;
use terra_core::{PlanetSpec, Seed};
use terra_land::fingerprint::{self, Fingerprint};
use terra_land::survey::{self, Biome, Survey};
use terra_land::{Land, PLANET_START, cave, gate, ground, spawn, start_climate};

const USAGE: &str = "cargo xtask worlds [--seeds 1-128] [--radius 30000] [--out target/worlds] [--against <an earlier worlds.tsv>]
  --seeds    the seeds swept, first to last
  --radius   the worlds' radius in metres (D13's 30 km world)
  --out      the folder the table, worlds.tsv, is written to
  --against  an earlier run's worlds.tsv: fail on any seed whose fingerprint differs from it";

struct Args {
    seeds: (u64, u64),
    radius_m: f32,
    out: PathBuf,
    against: Option<PathBuf>,
}

fn parse() -> Result<Args, String> {
    let mut args = Args {
        seeds: (1, 128),
        radius_m: 30_000.0,
        out: PathBuf::from("target/worlds"),
        against: None,
    };
    let mut all = std::env::args().skip(1);
    while let Some(arg) = all.next() {
        let mut value = || all.next().ok_or(format!("{arg} needs a value\n\n{USAGE}"));
        match arg.as_str() {
            "--seeds" => {
                let text = value()?;
                let (a, b) = text.split_once('-').unwrap_or((&text, &text));
                let whole = |t: &str| {
                    t.parse::<u64>()
                        .map_err(|_| format!("--seeds wants A-B, whole numbers: {text}"))
                };
                args.seeds = (whole(a)?, whole(b)?);
                if args.seeds.0 > args.seeds.1 {
                    return Err(format!("--seeds {text}: the first is past the last"));
                }
            }
            "--radius" => {
                args.radius_m = value()?
                    .parse()
                    .map_err(|_| "--radius wants metres".to_string())?
            }
            "--out" => args.out = PathBuf::from(value()?),
            "--against" => args.against = Some(PathBuf::from(value()?)),
            "--help" | "-h" => return Err(USAGE.into()),
            other => return Err(format!("unknown option {other}\n\n{USAGE}")),
        }
    }
    Ok(args)
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(true) => std::process::ExitCode::SUCCESS,
        Ok(false) => std::process::ExitCode::FAILURE,
        Err(problem) => {
            eprintln!("✗ worlds: {problem}");
            std::process::ExitCode::FAILURE
        }
    }
}

/// One seed's world.
struct World {
    seed: u64,
    print: Fingerprint,
    /// The start the search found (`spawn`), in km east and north of the planet's own.
    start_km: (f32, f32),
    search_s: f32,
    tried: Option<spawn::Tried>,
    /// The start's direction, its bits hashed, for another run to compare.
    start_print: u64,
    survey: Survey,
    /// What is wrong with the world's gate (`terra_land::gate`): none found from its start,
    /// or one a walker does not reach.
    gate_faults: Vec<String>,
    den_faults: Vec<String>,
}

fn run() -> Result<bool, String> {
    let args = parse()?;
    let started = Instant::now();
    let climate = start_climate()?;
    let seeds: Vec<u64> = (args.seeds.0..=args.seeds.1).collect();
    let worlds = terra_gen::par::map(&seeds, |&seed| {
        let spec = PlanetSpec {
            seed: Seed(seed),
            radius_m: args.radius_m,
        };
        let land = Land::new(ground::world(spec), climate.clone());
        let searching = Instant::now();
        let (start, tried) = spawn::find_tried(&land);
        let search_s = searching.elapsed().as_secs_f32();
        let (east, north) = (
            land.frame.east(PLANET_START),
            land.frame.north(PLANET_START),
        );
        World {
            seed,
            print: fingerprint::of(Seed(seed), args.radius_m, climate.clone()),
            start_km: (
                start.dot(east) * args.radius_m / 1000.0,
                start.dot(north) * args.radius_m / 1000.0,
            ),
            search_s,
            tried,
            start_print: [start.x, start.y, start.z].iter().fold(0, |sum, x| {
                terra_gen::hash::splitmix64(sum ^ u64::from(x.to_bits()))
            }),
            survey: survey::of(&land, start),
            gate_faults: match gate::find(&land, start) {
                Some(gate) => gate::faults(&land, start, gate),
                None => vec![format!(
                    "no gate: no ground {} to {} m from the start is level, dry and reached on foot (terra_land::gate)",
                    gate::NEAR_M,
                    gate::FARTHEST_M
                )],
            },
            den_faults: match cave::find(&land, start) {
                Some(den) => cave::faults(&land, start, &den),
                None => vec![format!(
                    "no den: no ground {} to {} m from the start that a walk reaches holds a cave with {} m of rock over it and its floor over the sea (terra_land::cave, D591)",
                    cave::NEAR_M,
                    cave::FARTHEST_M,
                    cave::COVER_M
                )],
            },
        }
    });
    let slowest = worlds.iter().map(|w| w.search_s).fold(0.0, f32::max);
    let moved = worlds.iter().filter(|w| w.start_km != (0.0, 0.0)).count();
    let table = table(&worlds);
    std::fs::create_dir_all(&args.out).map_err(|e| format!("{}: {e}", args.out.display()))?;
    let path = args.out.join("worlds.tsv");
    std::fs::write(&path, &table).map_err(|e| format!("{}: {e}", path.display()))?;

    let mut ok = true;
    let mut invalid = 0;
    for world in &worlds {
        let mut faults = world.survey.faults();
        faults.extend(world.gate_faults.iter().cloned());
        faults.extend(world.den_faults.iter().cloned());
        if !faults.is_empty() {
            invalid += 1;
            ok = false;
            let tried = match world.tried {
                Some(t) if world.start_km == (0.0, 0.0) => format!(
                    " (the start's search found none: of {} places within {} km, {} walked far enough, {} of them level; spawn.rs)",
                    t.places,
                    spawn::SEARCH_M[1] / 1000.0,
                    t.walked,
                    t.level
                ),
                Some(_) => String::new(),
                None => " (the planet's own start, kept: terra_land::spawn::find)".into(),
            };
            println!("✗ seed {}: {}{tried}", world.seed, faults.join("; "));
        }
    }
    let against = match &args.against {
        Some(earlier) => {
            let text = std::fs::read_to_string(earlier)
                .map_err(|e| format!("--against {}: {e}", earlier.display()))?;
            let differ = differences(&text, &worlds);
            for line in &differ {
                println!("✗ {line}");
            }
            ok &= differ.is_empty();
            format!(
                ", {} of {} the same bit for bit as {}",
                worlds.len() - differ.len(),
                worlds.len(),
                earlier.display()
            )
        }
        None => String::new(),
    };
    let digest = worlds.iter().fold(0u64, |sum, w| {
        terra_gen::hash::splitmix64(sum ^ w.print.whole())
    });
    let starts = worlds.iter().fold(0u64, |sum, w| {
        terra_gen::hash::splitmix64(sum ^ w.start_print)
    });
    println!(
        "{} worlds: seeds {}-{} on {} km worlds, {} valid, in {:.0} s; {moved} starts moved by the search, the slowest in {slowest:.1} s; their fingerprints' digest {digest:016x}, their starts' {starts:016x}{against}; the table in {}",
        if ok { "✓" } else { "✗" },
        args.seeds.0,
        args.seeds.1,
        args.radius_m / 1000.0,
        worlds.len() - invalid,
        started.elapsed().as_secs_f32(),
        path.display(),
    );
    Ok(ok)
}

/// The table: a row a seed, its fingerprint's parts and what its survey found.
fn table(worlds: &[World]) -> String {
    let mut out = String::from(
        "seed\tfingerprint\tground\tstands\ttrees\tweather\tstart\tstart_east_km\tstart_north_km\tsearch_s\tstart_rise_m\treached_share\treached_land_km2",
    );
    for biome in Biome::ALL {
        let _ = write!(out, "\t{}_km2", biome.name());
    }
    out.push_str("\tfaults\n");
    for w in worlds {
        let p = &w.print;
        let _ = write!(
            out,
            "{}\t{:016x}\t{:016x}\t{:016x}\t{:016x}\t{:016x}\t{:016x}\t{:.3}\t{:.3}\t{:.1}\t{}\t{:.3}\t{:.2}",
            w.seed,
            p.whole(),
            p.ground,
            p.stands,
            p.trees,
            p.weather,
            w.start_print,
            w.start_km.0,
            w.start_km.1,
            w.search_s,
            w.survey
                .start_rise_m
                .map_or("-".into(), |r| format!("{r:.2}")),
            w.survey.walk.share(),
            w.survey.walk.land_m2.0 / 1e6,
        );
        for biome in Biome::ALL {
            let _ = write!(out, "\t{:.2}", w.survey.reached(biome) / 1e6);
        }
        let _ = writeln!(out, "\t{}", w.survey.faults().join("; "));
    }
    out
}

/// Each seed whose fingerprint differs from an earlier table's, and which parts.
fn differences(earlier: &str, worlds: &[World]) -> Vec<String> {
    let mut found = Vec::new();
    let starts_kept = earlier
        .lines()
        .next()
        .is_some_and(|header| header.split('\t').nth(6) == Some("start"));
    for w in worlds {
        let Some(row) = earlier
            .lines()
            .skip(1)
            .map(|line| line.split('\t').collect::<Vec<_>>())
            .find(|cells| cells.first() == Some(&w.seed.to_string().as_str()))
        else {
            found.push(format!("seed {}: not in the earlier table", w.seed));
            continue;
        };
        let parts = w.print.parts();
        let mut differ: Vec<&str> = parts
            .iter()
            .enumerate()
            .filter(|(i, (_, hash))| row.get(i + 2) != Some(&format!("{hash:016x}").as_str()))
            .map(|(_, (name, _))| *name)
            .collect();
        // A table from before the start was searched for has no start to compare.
        if starts_kept && row.get(6) != Some(&format!("{:016x}", w.start_print).as_str()) {
            differ.push("start");
        }
        if !differ.is_empty() {
            found.push(format!(
                "seed {}: its {} differ from the earlier run's",
                w.seed,
                differ.join(", ")
            ));
        }
    }
    found
}
