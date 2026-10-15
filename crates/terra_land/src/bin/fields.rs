//! The field maps (D326): a region of a world's land on a date as small maps, its rules seen
//! before anything is drawn, on the CPU alone. `cargo xtask fields` runs it; `--help` says
//! what it takes.
//!
//! It writes, into its folder: a map each of the ground's height, steepness and facing, the
//! growing season's mean temperature with the tree line's isotherm drawn, snow lying on the
//! date, and shelter from the prevailing wind; `fields.json`, their numbers and a table of what
//! the climate puts at each height and latitude; and `index.html`, the maps with their legends.

use glam::{Vec2, Vec3};
use std::fmt::Write as _;
use std::path::PathBuf;
use std::time::Instant;
use terra_climate::{Date, TREE_LINE_SEASON_C};
use terra_core::{PlanetSpec, Seed};
use terra_land::maps::{Map, Ramp, hillshade, par_map};
use terra_land::region::Region;
use terra_land::{Land, PLANET_START, ground, start_climate};

const USAGE: &str = "cargo xtask fields [--radius 30000] [--seed 1] [--at <east km>,<north km>] [--size 8000] [--cell 32] [--date <month-day|day>] [--out shots/fields]

A region of the world's land on a date as maps of its rules, on the CPU (docs/demos.md, D326):
the ground's height, steepness and facing, the growing season's mean temperature with the tree
line's isotherm, snow lying on the date, shelter from the prevailing wind; fields.json with the
numbers and what the climate puts at each height and latitude; index.html to look at them.

  --radius   the world's radius in metres; 30000 is the starting world's (D13)
  --seed     the world's seed
  --at       the region's middle, kilometres east and north of the planet's own start
  --size     how wide the region is, in metres
  --cell     how wide each cell of the maps is, in metres; the past's own cells are 32 m
  --date     the date, as the sandbox takes it: 06-20 for midsummer's eve, or a day of the
             clock; by default the clock's first day, in late August
  --out      the folder to write into";

/// How far upwind shelter is looked for, as Winstral's Sx takes it for drifting snow.
const SHELTER_REACH_M: f32 = 100.0;
/// Snow this deep counts as lying, for the snow line.
const LYING_M: f32 = 0.1;

struct Args {
    radius_m: f32,
    seed: u64,
    at_km: Vec2,
    size_m: f32,
    cell_m: f32,
    date: Option<Date>,
    out: PathBuf,
}

fn parse() -> Result<Args, String> {
    let mut args = Args {
        radius_m: 30_000.0,
        seed: 1,
        at_km: Vec2::ZERO,
        size_m: 8000.0,
        cell_m: 32.0,
        date: None,
        out: PathBuf::from("shots/fields"),
    };
    let mut all = std::env::args().skip(1);
    while let Some(arg) = all.next() {
        let mut value = || all.next().ok_or(format!("{arg} needs a value"));
        let number = |text: String| {
            text.parse::<f32>()
                .map_err(|_| format!("not a number: {text}"))
        };
        match arg.as_str() {
            "--radius" => args.radius_m = number(value()?)?,
            "--seed" => {
                args.seed = value()?
                    .parse()
                    .map_err(|_| "--seed wants a whole number")?
            }
            "--at" => {
                let text = value()?;
                let (e, n) = text
                    .split_once(',')
                    .ok_or("--at wants <east km>,<north km>")?;
                args.at_km = Vec2::new(number(e.into())?, number(n.into())?);
            }
            "--size" => args.size_m = number(value()?)?,
            "--cell" => args.cell_m = number(value()?)?,
            "--date" => args.date = Some(Date::parse(&value()?)?),
            "--out" => args.out = PathBuf::from(value()?),
            "--help" | "-h" => return Err(USAGE.into()),
            other => return Err(format!("unknown option {other}\n\n{USAGE}")),
        }
    }
    Ok(args)
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(problem) => {
            eprintln!("{problem}");
            std::process::ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let args = parse()?;
    let started = Instant::now();
    let climate = start_climate()?;
    let day = args.date.map_or(0, |date| date.day(&climate.year));
    let land = Land::new(
        ground::world(PlanetSpec {
            seed: Seed(args.seed),
            radius_m: args.radius_m,
        }),
        climate,
    );
    let toward = toward(&land, args.at_km);
    let region = Region::sample(&land, toward, args.size_m, args.cell_m);
    let n = region.cells;
    let cells: Vec<usize> = (0..n * n).collect();
    let points: Vec<Vec3> = par_map(&cells, |&k| region.point(&land, k % n, k / n));
    let places = par_map(&cells, |&k| {
        land.place(points[k], region.normal(k % n, k / n))
    });
    let over_sea: Vec<f32> = places.iter().map(|p| p.height_m).collect();
    let slope: Vec<f32> = par_map(&cells, |&k| region.slope_deg(k % n, k / n));
    let facing: Vec<f32> = places.iter().map(|p| p.sunward.to_degrees()).collect();
    let wind_from = {
        let from = land.fields.climate().wind.prevailing_from_deg.to_radians();
        Vec2::new(libm::sinf(from), libm::cosf(from))
    };
    let shelter: Vec<f32> = par_map(&cells, |&k| {
        region.shelter_deg(k % n, k / n, wind_from, SHELTER_REACH_M)
    });
    let seasons = par_map(&places, |&place| land.fields.season(place));
    let season_c: Vec<f32> = seasons.iter().map(|s| s.growing_mean_c).collect();
    let snow_m: Vec<f32> = par_map(&places, |&place| {
        terra_climate::past(&land.fields, place, day).snow_depth_m()
    });
    let shade = hillshade(&region);

    std::fs::create_dir_all(&args.out).map_err(|e| format!("{}: {e}", args.out.display()))?;
    let scale = (768 / n).max(1);
    let range = |v: &[f32]| {
        v.iter()
            .fold((f32::MAX, f32::MIN), |(lo, hi), &x| (lo.min(x), hi.max(x)))
    };
    let (low, high) = range(&over_sea);
    let snow_top = range(&snow_m).1.max(0.5);
    let height_lines = contours(low, high, 250.0);
    let maps: Vec<(&str, &str, Map, String)> = vec![
        (
            "height",
            "Height over the sea, shaded by the relief, a line every 250 m",
            Map {
                cells: n,
                values: &over_sea,
                ramp: Ramp::LAND,
                low,
                high: high.max(low + 1.0),
                shade: Some(&shade),
                lines: &height_lines,
                over: None,
            },
            format!("{low:.0} m to {high:.0} m"),
        ),
        (
            "slope",
            "Steepness; lines at 30, 37 and 40 degrees, where scree rests and rock is bare",
            Map {
                cells: n,
                values: &slope,
                ramp: Ramp::AMOUNT,
                low: 0.0,
                high: 60.0,
                shade: None,
                lines: &[30.0, 37.0, 40.0],
                over: None,
            },
            "0 to 60 degrees".into(),
        ),
        (
            "facing",
            "How far the ground leans toward the equator (warm) or the pole (cold)",
            Map {
                cells: n,
                values: &facing,
                ramp: Ramp::COLD_WARM,
                low: -40.0,
                high: 40.0,
                shade: Some(&shade),
                lines: &[],
                over: None,
            },
            "40 degrees toward the pole to 40 toward the equator".into(),
        ),
        (
            "season",
            "The growing season's mean temperature; the black line the tree line's 6.4 °C (Paulsen and Körner)",
            Map {
                cells: n,
                values: &season_c,
                ramp: Ramp::COLD_WARM,
                low: 0.0,
                high: 14.0,
                shade: Some(&shade),
                lines: &[TREE_LINE_SEASON_C],
                over: None,
            },
            "0 °C to 14 °C".into(),
        ),
        (
            "snow",
            "Snow lying on the date, from the folded past; a line where it is 10 cm deep",
            Map {
                cells: n,
                values: &snow_m,
                ramp: Ramp::SNOW,
                low: 0.0,
                high: snow_top,
                shade: Some(&shade),
                lines: &[LYING_M],
                over: None,
            },
            format!("none to {snow_top:.2} m"),
        ),
        (
            "shelter",
            "Shelter from the prevailing wind (Winstral's Sx over 100 m): red exposed, blue in the lee",
            Map {
                cells: n,
                values: &shelter,
                ramp: Ramp::SHELTER,
                low: -20.0,
                high: 20.0,
                shade: Some(&shade),
                lines: &[],
                over: None,
            },
            "20 degrees exposed to 20 sheltered".into(),
        ),
    ];
    for (name, _, map, _) in &maps {
        map.save(&args.out.join(format!("{name}.png")), scale)?;
    }

    // What the climate puts at each height and latitude, on level ground and on slopes of
    // 20 degrees facing the equator and the pole: the tree line, and the snow line on the date.
    let latitude = land.frame.latitude(toward);
    let mut table = Vec::new();
    for degrees in (30..=70).step_by(5) {
        let lat = (degrees as f32).to_radians();
        let line = |sunward: f32| land.fields.tree_line_m(lat, sunward, 6000.0);
        let snow_line = (0..=60).map(|k| k as f32 * 100.0).find(|&h| {
            let place = terra_climate::Place {
                at: toward * (land.sea_radius_m() + h),
                height_m: h,
                latitude: lat,
                sunward: 0.0,
            };
            terra_climate::past(&land.fields, place, day).snow_depth_m() >= LYING_M
        });
        table.push((
            degrees,
            line(0.0),
            line(20f32.to_radians()),
            line(-20f32.to_radians()),
            snow_line,
        ));
    }
    let here_line = land.fields.tree_line_m(latitude, 0.0, 6000.0);
    let above_line = places
        .iter()
        .zip(&seasons)
        .filter(|(_, s)| !s.holds_trees())
        .count() as f32
        / places.len() as f32;
    let snowy = snow_m.iter().filter(|&&s| s >= LYING_M).count() as f32 / snow_m.len() as f32;

    let opt = |v: Option<f32>| v.map_or("null".to_string(), |m| format!("{m:.0}"));
    let mut json = String::new();
    let _ = writeln!(json, "{{");
    let _ = writeln!(
        json,
        "  \"region\": {{ \"radius_m\": {}, \"seed\": {}, \"at_km\": [{}, {}], \"size_m\": {}, \"cell_m\": {}, \"cells\": {n}, \"latitude_deg\": {:.2} }},",
        args.radius_m,
        args.seed,
        args.at_km.x,
        args.at_km.y,
        args.size_m,
        args.cell_m,
        latitude.to_degrees()
    );
    let _ = writeln!(json, "  \"day\": {day},");
    let _ = writeln!(
        json,
        "  \"height_m\": {{ \"lowest\": {low:.1}, \"highest\": {high:.1} }},"
    );
    let _ = writeln!(json, "  \"steepest_deg\": {:.1},", range(&slope).1);
    let _ = writeln!(json, "  \"tree_line_m\": {},", opt(here_line));
    let _ = writeln!(json, "  \"share_above_tree_line\": {above_line:.3},");
    let _ = writeln!(json, "  \"share_with_snow\": {snowy:.3},");
    let _ = writeln!(json, "  \"by_latitude\": [");
    for (k, (deg, level, sunny, shaded, snow)) in table.iter().enumerate() {
        let _ = writeln!(
            json,
            "    {{ \"latitude_deg\": {deg}, \"tree_line_m\": {}, \"tree_line_sunny_20_m\": {}, \"tree_line_shaded_20_m\": {}, \"snow_line_m\": {} }}{}",
            opt(*level),
            opt(*sunny),
            opt(*shaded),
            opt(*snow),
            if k + 1 < table.len() { "," } else { "" }
        );
    }
    let _ = writeln!(json, "  ]");
    let _ = writeln!(json, "}}");
    std::fs::write(args.out.join("fields.json"), &json).map_err(|e| e.to_string())?;

    let mut html = String::from(
        "<!doctype html><meta charset=utf-8><title>Field maps</title><style>body{font:14px system-ui;background:#1b1d22;color:#ddd;margin:16px}figure{display:inline-block;margin:8px;vertical-align:top;width:400px}img{width:400px;image-rendering:pixelated}figcaption{margin-top:4px}td,th{padding:2px 10px;text-align:right}</style>",
    );
    let _ = write!(
        html,
        "<h1>Field maps</h1><p>{:.1} km by {:.1} km at {:.1} km east, {:.1} km north of the planet's start, latitude {:.2}°, cells of {} m, day {day} of the clock. Tree line here {} m over the sea; {:.0}% of the region above it; {:.0}% under 10 cm of snow or more. North is up.</p>",
        args.size_m / 1000.0,
        args.size_m / 1000.0,
        args.at_km.x,
        args.at_km.y,
        latitude.to_degrees(),
        args.cell_m,
        opt(here_line),
        above_line * 100.0,
        snowy * 100.0
    );
    for (name, caption, _, legend) in &maps {
        let _ = write!(
            html,
            "<figure><img src={name}.png><figcaption><b>{name}</b>: {caption}. Bar: {legend}.</figcaption></figure>"
        );
    }
    let _ = write!(
        html,
        "<h2>What the climate puts at each latitude</h2><table><tr><th>latitude</th><th>tree line, level</th><th>on 20° facing the equator</th><th>facing the pole</th><th>snow line on the date</th></tr>"
    );
    for (deg, level, sunny, shaded, snow) in &table {
        let _ = write!(
            html,
            "<tr><td>{deg}°</td><td>{} m</td><td>{} m</td><td>{} m</td><td>{} m</td></tr>",
            opt(*level),
            opt(*sunny),
            opt(*shaded),
            opt(*snow)
        );
    }
    html.push_str("</table>");
    std::fs::write(args.out.join("index.html"), html).map_err(|e| e.to_string())?;
    println!(
        "FIELDS {n} by {n} cells of {} m at latitude {:.2}°, day {day}: height {low:.0} to {high:.0} m, tree line {} m, {:.0}% above it, {:.0}% under snow; {:.1} s; {}",
        args.cell_m,
        latitude.to_degrees(),
        opt(here_line),
        above_line * 100.0,
        snowy * 100.0,
        started.elapsed().as_secs_f32(),
        args.out.join("index.html").display()
    );
    Ok(())
}

/// The direction from the centre so many kilometres east and north of the planet's start,
/// along the ground.
fn toward(land: &Land, at_km: Vec2) -> Vec3 {
    let start = PLANET_START;
    let (east, north) = (land.frame.east(start), land.frame.north(start));
    let along = east * at_km.x + north * at_km.y;
    let distance_m = along.length() * 1000.0;
    if distance_m < 1e-3 {
        return start;
    }
    let angle = distance_m / land.radius_m();
    start * libm::cosf(angle) + along.normalize() * libm::sinf(angle)
}

/// Heights every `every` metres between two.
fn contours(low: f32, high: f32, every: f32) -> Vec<f32> {
    let first = (low / every).ceil() as i32;
    let last = (high / every).floor() as i32;
    (first..=last).map(|k| k as f32 * every).collect()
}
