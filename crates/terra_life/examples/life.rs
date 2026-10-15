//! The life check (D326), headless: a market town from its seed (`terra_build::town`), its
//! people from `assets/data/life.ron`, run through a game day at a fast clock on the CPU,
//! watched step by step (`terra_life::check`). It prints the hours' counts, every fault, and
//! a verdict, and fails if anything went wrong: anyone stuck, in a wall, inside another,
//! sliding their feet, or late for their day.
//!
//!   cargo xtask life [--seed N | --seeds A-B] [--ground flat|hillside|rolling|all]
//!       [--from HOUR] [--hours N]
//!
//! The viewer stands on the square, so everyone in the town is ticked; people beyond
//! `terra_life::REDUCED_M` would be placed from the clock instead.

use glam::Vec2;
use std::process::ExitCode;
use std::time::Instant;
use terra_build::Kind;
use terra_build::town::Mark;
use terra_build::village::Kinds;
use terra_life::check::Watch;
use terra_life::{Life, Plan, Shows};

const KINDS: Kinds = Kinds {
    timber: Kind(1),
    masonry: Kind(2),
    beams: Kind(3),
    cob: Kind(4),
    framed: Kind(5),
    timber_roof: Kind(0x800),
    masonry_roof: Kind(0x801),
    beams_roof: Kind(0x802),
};

/// The game's clock: a day is thirty minutes of play (`terra_world::daylight::DAY_SECONDS`).
const SECONDS_AN_HOUR: f32 = 30.0 * 60.0 / 24.0;
/// The game's fixed step, in seconds.
const STEP_S: f32 = 0.1;

fn main() -> ExitCode {
    let mut seeds = (1u32, 1u32);
    let mut ground = "hillside".to_string();
    let mut from = 0.0f64;
    let mut span = 24.0f64;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut number = |name: &str| -> Option<f64> {
            let value = args.next().and_then(|v| v.parse().ok());
            if value.is_none() {
                eprintln!("life: {name} needs a number");
            }
            value
        };
        match arg.as_str() {
            "--seed" => match number("--seed") {
                Some(n) => seeds = (n as u32, n as u32),
                None => return ExitCode::FAILURE,
            },
            "--seeds" => {
                let range = args.next().unwrap_or_default();
                let (a, b) = range.split_once('-').unwrap_or((&range, &range));
                seeds = (a.parse().unwrap_or(0), b.parse().unwrap_or(0));
            }
            "--ground" => ground = args.next().unwrap_or(ground),
            "--from" => match number("--from") {
                Some(n) => from = n,
                None => return ExitCode::FAILURE,
            },
            "--hours" => match number("--hours") {
                Some(n) => span = n,
                None => return ExitCode::FAILURE,
            },
            other => {
                eprintln!(
                    "life: what is {other}? --seed N | --seeds A-B, --ground flat|hillside|rolling|all, --from HOUR, --hours N"
                );
                return ExitCode::FAILURE;
            }
        }
    }
    let file = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/data/life.ron");
    let plan = match std::fs::read_to_string(file)
        .map_err(|e| e.to_string())
        .and_then(|t| Plan::parse(&t))
    {
        Ok(plan) => plan,
        Err(why) => {
            eprintln!("life: {why}");
            return ExitCode::FAILURE;
        }
    };
    let grounds: Vec<&str> = if ground == "all" {
        vec!["flat", "hillside", "rolling"]
    } else {
        vec![ground.as_str()]
    };
    let mut failed = 0;
    for &ground in &grounds {
        let heights = heights(ground);
        for seed in seeds.0..=seeds.1 {
            if !day(seed, ground, &*heights, plan.clone(), from, span) {
                failed += 1;
            }
        }
    }
    if failed > 0 {
        println!("LIFE failed in {failed} town(s)");
        ExitCode::FAILURE
    } else {
        println!("LIFE ok");
        ExitCode::SUCCESS
    }
}

/// The grounds a town is tried on, as `cargo xtask town` draws them.
fn heights(ground: &str) -> Box<dyn Fn(Vec2) -> f32> {
    match ground {
        "flat" => Box::new(|_| 0.0),
        "rolling" => {
            Box::new(|p: Vec2| 4.0 * libm::sinf(p.x / 45.0) * libm::cosf(p.y / 60.0) + p.y * 0.03)
        }
        _ => Box::new(|p: Vec2| p.dot(Vec2::new(1.0, -1.0).normalize()) * 0.06),
    }
}

/// One town through its day; whether nothing went wrong.
fn day(
    seed: u32,
    ground: &str,
    heights: &dyn Fn(Vec2) -> f32,
    plan: Plan,
    from: f64,
    span: f64,
) -> bool {
    let town = terra_build::town::town(seed, KINDS, heights);
    let viewer =
        town.square.outline.iter().copied().sum::<Vec2>() / town.square.outline.len() as f32;
    let mut life = Life::new(town, plan, seed, SECONDS_AN_HOUR);
    println!(
        "\nLIFE town {seed} on {ground}: {} people in {} homes, {} spots; from {from:.1} h for {span:.1} h at a step of {:.0} ms",
        life.people.len(),
        life.town.homes.len(),
        life.spots.len(),
        STEP_S * 1000.0
    );
    let mut roles: std::collections::BTreeMap<&str, usize> = Default::default();
    for person in 0..life.people.len() {
        *roles.entry(life.role(person)).or_default() += 1;
    }
    let roles: Vec<String> = roles.iter().map(|(r, n)| format!("{n} {r}")).collect();
    println!("  roles: {}", roles.join(", "));
    life.start(from);
    let mut watch = Watch::default();
    let steps = (span * SECONDS_AN_HOUR as f64 / STEP_S as f64) as u64;
    let started = Instant::now();
    let mut slowest = 0.0f32;
    // Who is where on market morning, for the map.
    let mut morning: Vec<Mark> = Vec::new();
    for n in 0..steps {
        let hours = from + n as f64 * STEP_S as f64 / SECONDS_AN_HOUR as f64;
        let before = Instant::now();
        life.step(hours, STEP_S, &[viewer]);
        slowest = slowest.max(before.elapsed().as_secs_f32() * 1000.0);
        watch.see(&life, hours, STEP_S);
        if morning.is_empty() && hours >= MORNING_H {
            morning = (0..life.people.len())
                .filter_map(|p| {
                    let colour = match life.shows(p) {
                        Shows::Hidden => return None,
                        Shows::Walk(_) => "#ffffff",
                        Shows::Run(_) => "#9be7ff",
                        Shows::Clip(_) => "#3cb043",
                    };
                    Some(Mark {
                        at: life.people[p].at,
                        colour,
                        size: 2.2,
                        label: None,
                    })
                })
                .collect();
        }
    }
    watch.finish();
    let took = started.elapsed().as_secs_f32();
    for hour in &watch.hours {
        let mut doing: Vec<(&String, &f32)> = hour.doing.iter().collect();
        doing.sort_by(|a, b| b.1.total_cmp(a.1));
        let doing: Vec<String> = doing
            .iter()
            .filter(|(_, n)| **n >= 0.5)
            .map(|(a, n)| format!("{n:.0} {}", a.to_lowercase()))
            .collect();
        println!(
            "  {:02}:00  out {:5.1}  walking {:5.1}  on the square {:5.1}  standing in their way {:4.1}  {}",
            hour.hour.rem_euclid(24),
            hour.out,
            hour.walking,
            hour.on_square,
            hour.aimless,
            doing.join(", ")
        );
    }
    // By day, the share of those out standing in their way, held up by others, idling
    // without a purpose to look at: the aim is none.
    let (standing, out) = watch
        .hours
        .iter()
        .filter(|h| (7..18).contains(&h.hour.rem_euclid(24)))
        .fold((0.0, 0.0), |(s, o), h| (s + h.aimless, o + h.out));
    println!(
        "  standing in their way by day: {:.1}% of those out",
        100.0 * standing / f32::max(out, 1.0)
    );
    let faults = &watch.faults;
    let hhmm = |hours: f64| {
        let minutes = (hours.rem_euclid(24.0) * 60.0) as i64;
        format!("{:02}:{:02}", minutes / 60, minutes % 60)
    };
    let who = |person: usize| format!("{} {person}", life.role(person));
    for (p, at, place, going) in faults.stuck.iter().take(12) {
        println!(
            "  STUCK {} at {} by {:.1}, {:.1}, {going}",
            who(*p),
            hhmm(*at),
            place.x,
            place.y
        );
    }
    for &(p, at, place) in faults.in_walls.iter().take(12) {
        println!(
            "  IN A WALL {} at {} at {:.1}, {:.1}",
            who(p),
            hhmm(at),
            place.x,
            place.y
        );
    }
    for &(a, b, at, place) in faults.through.iter().take(12) {
        println!(
            "  THROUGH {} and {} at {} at {:.1}, {:.1}",
            who(a),
            who(b),
            hhmm(at),
            place.x,
            place.y
        );
    }
    for &(p, at, place) in faults.sliding.iter().take(12) {
        println!(
            "  SLIDING {} at {} at {:.1}, {:.1}",
            who(p),
            hhmm(at),
            place.x,
            place.y
        );
    }
    for late in faults.late.iter().take(12) {
        println!(
            "  LATE {} for {} by {:.0} min: set off at {}, a walk of {:.0} s that took {:.0} s",
            who(late.person),
            hhmm(late.window),
            late.late,
            hhmm(late.set_off),
            late.route_s,
            late.walked_s
        );
    }
    let arrivals = &life.record.arrivals;
    let mut lateness: Vec<f32> = arrivals.iter().map(|a| a.late).collect();
    lateness.sort_by(f32::total_cmp);
    let at = |share: f32| {
        lateness
            .get(((lateness.len().max(1) - 1) as f32 * share) as usize)
            .copied()
            .unwrap_or(0.0)
    };
    println!(
        "  {} arrivals for their day: early by a median {:.0} min, the latest {:+.0} min; {:.1} s for the day, the slowest step {:.2} ms",
        arrivals.len(),
        -at(0.5),
        at(1.0),
        took,
        slowest
    );
    println!(
        "  faults: {} stuck, {} in walls, {} through each other, {} sliding, {} late",
        faults.stuck.len(),
        faults.in_walls.len(),
        faults.through.len(),
        faults.sliding.len(),
        faults.late.len()
    );
    // The map: the town at market morning, and where each fault was.
    let mut marks = morning;
    let fault = |at: Vec2, colour: &'static str, person: usize| Mark {
        at,
        colour,
        size: 4.0,
        label: Some(format!("{}", person)),
    };
    marks.extend(
        faults
            .stuck
            .iter()
            .map(|&(p, _, at, _)| fault(at, "#e0201b", p)),
    );
    marks.extend(
        faults
            .in_walls
            .iter()
            .map(|&(p, _, at)| fault(at, "#111111", p)),
    );
    marks.extend(
        faults
            .through
            .iter()
            .map(|&(p, _, _, at)| fault(at, "#a020f0", p)),
    );
    marks.extend(
        faults
            .sliding
            .iter()
            .map(|&(p, _, at)| fault(at, "#ff9a1f", p)),
    );
    let out = format!("shots/life-{seed}-{ground}.svg");
    let _ = std::fs::create_dir_all("shots");
    match std::fs::write(&out, terra_build::town::svg(&life.town, heights, &marks)) {
        Ok(()) => println!(
            "  drawn to {out}: people at {MORNING_H:.0}:00 (white walking, blue running, green at a spot) and faults (red stuck, black in a wall, purple through another, orange sliding)"
        ),
        Err(why) => println!("  could not draw {out}: {why}"),
    }
    faults.count() == 0
}

/// The hour the map shows who is where: market morning.
const MORNING_H: f64 = 10.0;
