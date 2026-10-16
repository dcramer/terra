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
use terra_land::lakes;
use terra_land::maps::{Map, Ramp, hillshade, par_map};
use terra_land::region::Region;
use terra_land::site::MountainLake;
use terra_land::woods::{self, Stand};
use terra_land::{Land, PLANET_START, ground, start_climate};
use terra_voxel::{Field, material};

const USAGE: &str = "cargo xtask fields [--radius 30000] [--seed 1] [--at <east km>,<north km>] [--size 8000] [--cell 32] [--date <month-day|day>] [--site lake|tundra|town] [--out shots/fields]

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
  --site     the region round a place the land's rules make, found by its query instead of
             --at: lake, a basin under the tree line with peaks a kilometre over it; tundra, level
             ground past the trees over permafrost, the forest's edge in sight; town, level ground
             for a market town below the tree line, the forest past its fields and peaks over
             it (docs/demos.md)
  --out      the folder to write into";

/// How far upwind shelter is looked for, as Winstral's Sx takes it for drifting snow.
const SHELTER_REACH_M: f32 = terra_land::drift::REACH_M;
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
    site: Option<String>,
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
        site: None,
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
            "--site" => args.site = Some(value()?),
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
    // Ground a settlement found by its query has cleared of the forest for its fields.
    let mut clearings: Vec<terra_land::town::Clearing> = Vec::new();
    let site = match args.site.as_deref() {
        None => None,
        Some("lake") => {
            let (site, searched) = land.mountain_lake_searched(PLANET_START, &MountainLake::DEMO);
            match site {
                Some(site) => {
                    println!(
                        "FIELDS site lake: {:.2} km east, {:.2} km north of the start, its water {:.0} m over the sea, {:.2} km² and {:.0} m deep, the tree line at {:.0} m, peaks to {:.0} m within 3 km ({} lakes looked at, {} fit)",
                        site.at_km.x,
                        site.at_km.y,
                        site.level_m,
                        site.area_m2 / 1e6,
                        site.deepest_m,
                        site.tree_line_m,
                        site.highest_m,
                        searched.lakes,
                        searched.fit
                    );
                    // The walk round its shore from where the sandbox starts by it, for a
                    // film's and a bench's route (places.ts), on the game's own lake grid.
                    let waters =
                        terra_land::water::Waters::round(&land, site.toward, 10_000.0, 10.0);
                    let shore = waters.shore_of(site.toward).unwrap_or(site.shore);
                    let walk =
                        terra_land::site::shore_walk(&land, &waters, shore, site.toward, 25.0);
                    let through: Vec<String> = walk
                        .iter()
                        .map(|p| format!("{:.0},{:.0}", p.x, p.y))
                        .collect();
                    println!(
                        "FIELDS shore walk, metres east and south of the start: {}",
                        through.join(";")
                    );
                    Some(site.at_km)
                }
                None => return Err(format!("no mountain lake on this world: {searched:?}")),
            }
        }
        Some("tundra") => {
            let want = terra_land::tundra::OpenTundra::DEMO;
            let (found, searched) = land.tundra_searched(PLANET_START, &want);
            let found =
                found.ok_or_else(|| format!("no open tundra on this world: {searched:?}"))?;
            let forest_km = land.east_north_km(found.toward, found.forest);
            println!(
                "FIELDS site tundra: {:.2} km east, {:.2} km north of the start, {:.0} m over the sea, its growing season {:.1} °C and its year {:.1} °C, the closed forest {:.0} m off ({:.2} km east, {:.2} km north of it), {:.1}% of the ground round it pools ({} cells looked at, {} fit)",
                found.at_km.x,
                found.at_km.y,
                found.height_m,
                found.growing_mean_c,
                found.year_mean_c,
                found.forest_m,
                forest_km.x,
                forest_km.y,
                found.pools * 100.0,
                searched.cells,
                searched.fit
            );
            Some(found.at_km)
        }
        Some("town") => {
            let want = terra_land::town::MarketTown::DEMO;
            let (found, searched) = land.market_town_searched(PLANET_START, &want);
            let found =
                found.ok_or_else(|| format!("no place for a town on this world: {searched:?}"))?;
            let peak_km = land.east_north_km(found.toward, found.peak);
            let start_km = land.east_north_km(found.toward, found.start);
            println!(
                "FIELDS site town: {:.2} km east, {:.2} km north of the start, {:.0} m over the sea, its growing season {:.1} °C, the tree line at {:.0} m; {:.1} m of rise and fall within {:.0} m, {:.1}° at its middle; closed forest past its fields on {:.0}% of the ways out; the highest ground {:.0} m over the sea {:.1} km off ({:.2} km east, {:.2} km north of it), the skyline up to {:.1}° and over 2° on {:.0}% of the ways round; the world's start {:.2} km east, {:.2} km north of it ({} cells looked at, {} fit)",
                found.at_km.x,
                found.at_km.y,
                found.height_m,
                found.growing_mean_c,
                found.tree_line_m,
                found.relief_m,
                want.reach_m,
                found.slope_deg,
                found.forest * 100.0,
                found.peak_m,
                found.peak_off_m / 1000.0,
                peak_km.x,
                peak_km.y,
                found.skyline_deg,
                found.ringed * 100.0,
                start_km.x,
                start_km.y,
                searched.cells,
                searched.fit
            );
            clearings = want.clearings(&land, &found);
            Some(found.at_km)
        }
        Some(other) => {
            return Err(format!(
                "no site called {other}: the sites are lake, tundra and town"
            ));
        }
    };
    let at_km = site.unwrap_or(args.at_km);
    let toward = land.along(PLANET_START, at_km * 1000.0);
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
    // Snow as the ground's shelter from the wind catches it (D323): more in the lee.
    let snow_m: Vec<f32> = par_map(&cells, |&k| {
        let place = terra_climate::Place {
            catch: terra_land::drift::catch(shelter[k]),
            ..places[k]
        };
        terra_climate::past(&land.fields, place, day).snow_depth_m()
    });
    let shade = hillshade(&region);
    // What the ground is made of just under its surface, as digging finds it, and the sea.
    let sea = land.sea_radius_m();
    let lying: Vec<Option<[u8; 3]>> = par_map(&cells, |&k| {
        let point = points[k];
        if point.length() < sea {
            return Some([40, 70, 120]);
        }
        let made_of = land.ground.sample(point - point.normalize() * 0.3).material;
        Some(match made_of {
            material::STONE => [128, 126, 122],
            material::SCREE => [196, 194, 188],
            material::GRAVEL => [190, 160, 110],
            material::SAND => [220, 200, 140],
            material::ORE => [150, 70, 40],
            _ => [92, 120, 64],
        })
    });
    // Lakes: the region's hollows filled to their spill points (D322), water leaving by its
    // edge; the sea is not a lake.
    let sea_level = land.sea_radius_m() - land.radius_m();
    let lakes = lakes::fill(&region, land.ground.sea_radius_m().map(|_| sea_level));
    let lake_cells: Vec<Option<[u8; 3]>> = lakes
        .lake
        .iter()
        .zip(&lakes.level)
        .zip(&region.heights)
        .map(|((lake, level), ground)| {
            lake.map(|_| {
                let deep = ((level - ground) / 20.0).clamp(0.0, 1.0);
                [
                    (60.0 - 40.0 * deep) as u8,
                    (120.0 - 60.0 * deep) as u8,
                    (190.0 - 60.0 * deep) as u8,
                ]
            })
        })
        .collect();
    // The trees each cell holds (`woods`): how close and how tall, from its season, its
    // steepness and its footing.
    let microsites = woods::Microsites::new(Seed(args.seed));
    let stands: Vec<Stand> = par_map(&cells, |&k| {
        let water = lakes.lake[k].map(|_| land.radius_m() + lakes.level[k]);
        let footing = land.footing(points[k], water);
        let stand = woods::stand_in(
            &seasons[k],
            slope[k],
            footing,
            microsites.at(points[k], shelter[k]),
        );
        let kept: f32 = clearings.iter().map(|c| c.keeps(points[k])).product();
        Stand {
            density: stand.density * kept,
            ..stand
        }
    });
    // What covers the open ground past the trees (terra_land::heath), community by community.
    let drainage = terra_land::heath::Drainage::new(Seed(args.seed));
    let wet: Vec<f32> = par_map(&cells, |&k| {
        let above_water = if lakes.lake[k].is_some() { -1.0 } else { 50.0 };
        let frozen = ((-1.0 - seasons[k].year_mean_c) / 3.0).clamp(0.0, 1.0);
        (terra_land::heath::wetness(&region, k % n, k / n, above_water, frozen)
            + drainage.at(points[k]))
        .clamp(0.0, 1.0)
    });
    let heath_cells: Vec<Option<[u8; 3]>> = par_map(&cells, |&k| {
        if lakes.lake[k].is_some() {
            return Some([40, 80, 150]);
        }
        let h = terra_land::heath::heath(terra_land::heath::Ground {
            colder_c: TREE_LINE_SEASON_C - seasons[k].growing_mean_c,
            trees: stands[k].density,
            shelter_deg: shelter[k],
            wet: wet[k],
            slope_deg: slope[k],
        });
        let parts = [
            (h.fellfield, [150.0, 148.0, 142.0]),
            (h.lichen, [205.0, 204.0, 176.0]),
            (h.shrub, [150.0, 72.0, 42.0]),
            (h.sedge, [178.0, 156.0, 92.0]),
            (h.moss, [88.0, 128.0, 48.0]),
            (stands[k].density * (1.0 - h.open), [24.0, 60.0, 30.0]),
            (
                (1.0 - h.open) * (1.0 - stands[k].density),
                [96.0, 136.0, 60.0],
            ),
        ];
        let mut c = [0.0f32; 3];
        let mut sum = 0.0;
        for (share, colour) in parts {
            sum += share;
            for i in 0..3 {
                c[i] += share * colour[i];
            }
        }
        let s = shade[k] / sum.max(1e-3);
        Some(c.map(|v| (v * s).min(255.0) as u8))
    });
    // Trees standing: closer for denser, and a tree at its full height green, krummholz brown.
    let tree_cells: Vec<Option<[u8; 3]>> = stands
        .iter()
        .zip(&shade)
        .map(|(stand, s)| {
            (stand.density > 0.02).then(|| {
                let tall = [24.0, 70.0, 34.0];
                let low = [120.0, 96.0, 50.0];
                let t = stand.stature;
                let c: Vec<u8> = (0..3)
                    .map(|i| {
                        ((low[i] + (tall[i] - low[i]) * t) * (1.2 - 0.6 * stand.density) * s)
                            .min(255.0) as u8
                    })
                    .collect();
                [c[0], c[1], c[2]]
            })
        })
        .collect();
    let density: Vec<f32> = stands.iter().map(|s| s.density).collect();
    // The tree line as the map has it, measured: the height under which nine in ten of the
    // cells holding trees of three metres and more stand, on ground leaning toward the
    // equator, level, and toward the pole.
    let measured = |keep: &dyn Fn(f32) -> bool| {
        let mut heights: Vec<f32> = (0..n * n)
            .filter(|&k| stands[k].density > 0.3 && !stands[k].krummholz() && keep(facing[k]))
            .map(|k| over_sea[k])
            .collect();
        heights.sort_by(f32::total_cmp);
        heights.get((heights.len() as f32 * 0.9) as usize).copied()
    };
    let measured_sunny = measured(&|f| f > 10.0);
    let measured_level = measured(&|f| f.abs() <= 10.0);
    let measured_shaded = measured(&|f| f < -10.0);
    let mut by_size = lakes.lakes.clone();
    by_size.sort_by(|a, b| b.area_m2.total_cmp(&a.area_m2));
    let shaded_lying: Vec<Option<[u8; 3]>> = lying
        .iter()
        .zip(&shade)
        .map(|(c, s)| c.map(|c| c.map(|ch| (ch as f32 * s).min(255.0) as u8)))
        .collect();

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
            "Height over the sea, shaded by the relief, a line every 250 m; lakes, filled to their spill points, blue",
            Map {
                cells: n,
                values: &over_sea,
                ramp: Ramp::LAND,
                low,
                high: high.max(low + 1.0),
                shade: Some(&shade),
                lines: &height_lines,
                over: Some(&lake_cells),
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
            "Snow lying on the date, from the folded past, as the ground's shelter from the wind catches it (D323); a line where it is 10 cm deep",
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
            "ground",
            "What the ground is made of at its surface: soil green, bare rock grey, scree pale grey, gravel tan, sand yellow, the sea blue",
            Map {
                cells: n,
                values: &slope,
                ramp: Ramp::AMOUNT,
                low: 0.0,
                high: 60.0,
                shade: None,
                lines: &[],
                over: Some(&shaded_lying),
            },
            "by colour".into(),
        ),
        (
            "trees",
            "Trees as the land's rules grow them: green at their full height, brown as krummholz, darker for closer; a line at half a closed forest",
            Map {
                cells: n,
                values: &density,
                ramp: Ramp::SNOW,
                low: 0.0,
                high: 1.0,
                shade: Some(&shade),
                lines: &[0.5],
                over: Some(&tree_cells),
            },
            "none to a closed forest".into(),
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
        (
            "heath",
            "What covers open ground past the trees, by exposure and wetness (terra_land::heath): fellfield's stones grey, lichen heath pale, dwarf-shrub heath rust, tussock sedge straw, moss carpet green; the forest dark green, a meadow's grass green; lakes blue",
            Map {
                cells: n,
                values: &density,
                ramp: Ramp::SNOW,
                low: 0.0,
                high: 1.0,
                shade: Some(&shade),
                lines: &[],
                over: Some(&heath_cells),
            },
            "by colour".into(),
        ),
        (
            "wetness",
            "How wet the ground is (terra_land::heath::wetness): hollows, level ground, the shore and permafrost's held water",
            Map {
                cells: n,
                values: &wet,
                ramp: Ramp::AMOUNT,
                low: 0.0,
                high: 1.0,
                shade: Some(&shade),
                lines: &[0.45, 0.75],
                over: None,
            },
            "dry to soaked".into(),
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
                catch: 1.0,
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
        at_km.x,
        at_km.y,
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
    // How steep the high ground is, a kilometre and more over the sea: the shares of it under
    // 20 degrees, to 30, to 40, and over, which sets what can grow on it and what is bare.
    let high_ground: Vec<f32> = (0..n * n)
        .filter(|&k| over_sea[k] > 1000.0)
        .map(|k| slope[k])
        .collect();
    let share = |lo: f32, hi: f32| {
        high_ground.iter().filter(|&&s| s >= lo && s < hi).count() as f32
            / high_ground.len().max(1) as f32
    };
    let _ = writeln!(
        json,
        "  \"high_ground_slopes\": {{ \"under_20\": {:.3}, \"20_to_30\": {:.3}, \"30_to_40\": {:.3}, \"over_40\": {:.3} }},",
        share(0.0, 20.0),
        share(20.0, 30.0),
        share(30.0, 40.0),
        share(40.0, 90.0)
    );
    let _ = writeln!(json, "  \"tree_line_m\": {},", opt(here_line));
    let _ = writeln!(json, "  \"share_above_tree_line\": {above_line:.3},");
    let _ = writeln!(json, "  \"share_with_snow\": {snowy:.3},");
    let _ = writeln!(
        json,
        "  \"tree_line_measured_m\": {{ \"facing_the_equator\": {}, \"level\": {}, \"facing_the_pole\": {} }},",
        opt(measured_sunny),
        opt(measured_level),
        opt(measured_shaded)
    );
    let _ = writeln!(json, "  \"lakes\": [");
    let listed: Vec<String> = by_size
        .iter()
        .take(12)
        .map(|lake| {
            let at = region.offset(lake.deepest.0, lake.deepest.1);
            format!(
                "    {{ \"level_m\": {:.1}, \"area_m2\": {:.0}, \"deepest_m\": {:.1}, \"deepest_at_m\": [{:.0}, {:.0}] }}",
                lake.level_m - sea_level,
                lake.area_m2,
                lake.deepest_m,
                at.x,
                at.y
            )
        })
        .collect();
    let _ = writeln!(json, "{}", listed.join(",\n"));
    let _ = writeln!(json, "  ],");
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
        at_km.x,
        at_km.y,
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

/// Heights every `every` metres between two.
fn contours(low: f32, high: f32, every: f32) -> Vec<f32> {
    let first = (low / every).ceil() as i32;
    let last = (high / every).floor() as i32;
    (first..=last).map(|k| k as f32 * every).collect()
}
