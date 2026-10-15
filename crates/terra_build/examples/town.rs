//! A market town's plan (`terra_build::town`), drawn as an SVG to look at: the square, the
//! streets, every building's ground, the yards, the things a day is done at, and the path
//! graph its people walk, with a legend and the counts. CPU only, in a moment.
//!
//!   cargo run -p terra_build --example town -- [--seed N] [--ground flat|hillside|rolling]
//!       [--out shots/town.svg]
//!
//! With `--seeds A-B` and `--ground all` it lays every one and prints a line for each, and
//! whatever on any is cut off from its paths, without drawing.

use glam::Vec2;
use terra_build::Kind;
use terra_build::town::{self, On, ThingKind, Town};
use terra_build::village::Kinds;

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

fn main() {
    let mut seeds = (1u32, 1u32);
    let mut ground = "hillside".to_string();
    let mut out = "shots/town.svg".to_string();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--seed" => {
                let seed = args.next().and_then(|s| s.parse().ok()).unwrap_or(1);
                seeds = (seed, seed);
            }
            "--seeds" => {
                let range = args.next().unwrap_or_default();
                let (a, b) = range.split_once('-').unwrap_or((&range, &range));
                seeds = (a.parse().unwrap_or(0), b.parse().unwrap_or(0));
            }
            "--ground" => ground = args.next().unwrap_or(ground),
            "--out" => out = args.next().unwrap_or(out),
            other => {
                eprintln!(
                    "town: what is {other}? --seed N | --seeds A-B, --ground flat|hillside|rolling|all, --out file.svg"
                );
                std::process::exit(2);
            }
        }
    }
    let grounds: Vec<&str> = if ground == "all" {
        vec!["flat", "hillside", "rolling"]
    } else {
        vec![ground.as_str()]
    };
    let one = grounds.len() == 1 && seeds.0 == seeds.1;
    for &ground in &grounds {
        let heights = heights(ground);
        for seed in seeds.0..=seeds.1 {
            let town = town::town(seed, KINDS, &*heights);
            if let Some(folder) = std::path::Path::new(&out).parent() {
                let _ = std::fs::create_dir_all(folder);
            }
            if one && let Err(why) = std::fs::write(&out, draw(&town, &*heights)) {
                eprintln!("town: cannot write {out}: {why}");
                std::process::exit(1);
            }
            report(seed, ground, &town, one.then_some(out.as_str()));
        }
    }
}

/// The grounds a town is tried on: level, a hillside rising 6 in 100 to the north-east, and
/// rolling swells on a gentle slope.
fn heights(ground: &str) -> Box<dyn Fn(Vec2) -> f32> {
    match ground {
        "flat" => Box::new(|_| 0.0),
        "rolling" => {
            Box::new(|p: Vec2| 4.0 * libm::sinf(p.x / 45.0) * libm::cosf(p.y / 60.0) + p.y * 0.03)
        }
        _ => Box::new(|p: Vec2| p.dot(Vec2::new(1.0, -1.0).normalize()) * 0.06),
    }
}

fn report(seed: u32, ground: &str, town: &Town, drawn: Option<&str>) {
    let parts = town.paths.parts();
    for thing in &town.things {
        if parts[thing.node as usize] != parts[0] {
            println!(
                "  cut off: the {:?} at {:.1}, {:.1}",
                thing.kind, thing.at.x, thing.at.y
            );
        }
    }
    let count = |kind| town.things.iter().filter(|t| t.kind == kind).count();
    println!(
        "TOWN seed {seed} on {ground}: a {:?} square, {} streets, {} homes, {} buildings in all, {} yards, {} stalls, {} pens, {} hen yards, {} chopping blocks, {} path nodes and {} edges{}",
        town.square.shape,
        town.streets.len(),
        town.homes.len(),
        town.buildings.len(),
        town.yards.len(),
        count(ThingKind::Stall),
        count(ThingKind::Pen),
        count(ThingKind::Yard),
        count(ThingKind::Block),
        town.paths.nodes.len(),
        town.paths.edges.len(),
        drawn.map_or(String::new(), |out| format!("; drawn to {out}")),
    );
}

fn draw(town: &Town, heights: &dyn Fn(Vec2) -> f32) -> String {
    let mut points: Vec<Vec2> = town.paths.nodes.iter().map(|n| n.0).collect();
    points.extend(town.footprints.iter().flatten().flatten());
    let low = points
        .iter()
        .copied()
        .reduce(Vec2::min)
        .unwrap_or(Vec2::ZERO)
        - 12.0;
    let high = points
        .iter()
        .copied()
        .reduce(Vec2::max)
        .unwrap_or(Vec2::ZERO)
        + 12.0;
    let size = high - low;
    let scale = 4.0;
    let p = |v: Vec2| {
        let s = (v - low) * scale;
        format!("{:.1},{:.1}", s.x, s.y)
    };
    let poly = |quad: &[Vec2], style: &str| {
        let pts: Vec<String> = quad.iter().map(|&v| p(v)).collect();
        format!("<polygon points=\"{}\" {style}/>\n", pts.join(" "))
    };
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{:.0}\" height=\"{:.0}\" font-family=\"sans-serif\">\n<rect width=\"100%\" height=\"100%\" fill=\"#e9e4d4\"/>\n",
        size.x * scale,
        size.y * scale + 70.0
    );
    // Contours every metre, as dots, so the slope reads.
    let mut y = low.y;
    while y < high.y {
        let mut x = low.x;
        while x < high.x {
            let h = heights(Vec2::new(x, y));
            let shade = (h.rem_euclid(2.0) < 1.0) as u8;
            if shade == 1 {
                svg.push_str(&format!(
                    "<rect x=\"{:.0}\" y=\"{:.0}\" width=\"{:.0}\" height=\"{:.0}\" fill=\"#dcd5bf\"/>\n",
                    (x - low.x) * scale,
                    (y - low.y) * scale,
                    2.0 * scale,
                    2.0 * scale
                ));
            }
            x += 2.0;
        }
        y += 2.0;
    }
    svg.push_str(&poly(
        &town.square.outline,
        "fill=\"#cdbf9c\" stroke=\"#8a7a55\"",
    ));
    for street in &town.streets {
        let pts: Vec<String> = street.points.iter().map(|&v| p(v)).collect();
        svg.push_str(&format!(
            "<polyline points=\"{}\" fill=\"none\" stroke=\"#cdbf9c\" stroke-width=\"{:.0}\" stroke-linejoin=\"round\"/>\n",
            pts.join(" "),
            street.wide * scale
        ));
    }
    for (_, yard) in &town.yards {
        svg.push_str(&poly(
            yard,
            "fill=\"#b9c99a\" stroke=\"#7d8f5c\" stroke-dasharray=\"3 2\"",
        ));
    }
    for (index, quads) in town.footprints.iter().enumerate() {
        let home = town.homes.contains(&index);
        let fill = if home { "#8c6b4f" } else { "#6f6a63" };
        for quad in quads {
            svg.push_str(&poly(quad, &format!("fill=\"{fill}\" stroke=\"#3b2f25\"")));
        }
    }
    for &[a, b] in &town.paths.edges {
        let (pa, pb) = (
            town.paths.nodes[a as usize].0,
            town.paths.nodes[b as usize].0,
        );
        svg.push_str(&format!(
            "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#2f6db5\" stroke-width=\"0.8\"/>\n",
            ((pa - low) * scale).x,
            ((pa - low) * scale).y,
            ((pb - low) * scale).x,
            ((pb - low) * scale).y
        ));
    }
    let parts = town.paths.parts();
    for (index, (at, on)) in town.paths.nodes.iter().enumerate() {
        let colour = if parts[index] != parts[0] {
            "#e0201b"
        } else {
            match on {
                On::Street(_) => "#2f6db5",
                On::Square => "#2f6db5",
                On::Thing(_) => "#111",
                On::Lane(_) => "#5a8f2c",
            }
        };
        let s = (*at - low) * scale;
        svg.push_str(&format!(
            "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"1.6\" fill=\"{colour}\"/>\n",
            s.x, s.y
        ));
    }
    for thing in &town.things {
        let colour = match thing.kind {
            ThingKind::Door => "#f2c14e",
            ThingKind::Well => "#3a8fd9",
            ThingKind::Stall => "#d9573a",
            ThingKind::Forge => "#1c1c1c",
            ThingKind::Block => "#a0522d",
            ThingKind::Bench => "#7a5c3a",
            ThingKind::Trough => "#3a8fd9",
            ThingKind::Post => "#7b2fbe",
            ThingKind::Gathering => "#e08bd1",
            ThingKind::Gate => "#7b2fbe",
            ThingKind::Pen => "#d6c7a1",
            ThingKind::Yard => "#e3d26a",
        };
        match thing.outline() {
            Some(quad) => svg.push_str(&poly(&quad, &format!("fill=\"{colour}\" stroke=\"#222\""))),
            None => {
                let s = (thing.at - low) * scale;
                svg.push_str(&format!(
                    "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"4\" fill=\"{colour}\" stroke=\"#222\"/>\n",
                    s.x, s.y
                ));
            }
        }
        // Which way it looks.
        let (from, to) = (
            (thing.at - low) * scale,
            (thing.at + thing.front() * 1.5 - low) * scale,
        );
        svg.push_str(&format!(
            "<line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" stroke=\"#222\" stroke-width=\"1\"/>\n",
            from.x, from.y, to.x, to.y
        ));
    }
    let legend = [
        ("#8c6b4f", "home"),
        ("#6f6a63", "shed, wall"),
        ("#b9c99a", "yard"),
        ("#f2c14e", "door"),
        ("#d9573a", "stall"),
        ("#3a8fd9", "well, trough"),
        ("#1c1c1c", "forge"),
        ("#a0522d", "block"),
        ("#7a5c3a", "bench"),
        ("#e08bd1", "gathering"),
        ("#7b2fbe", "gate, post"),
        ("#e3d26a", "hens"),
        ("#d6c7a1", "pen"),
        ("#2f6db5", "paths"),
        ("#e0201b", "cut off"),
    ];
    let base = size.y * scale + 10.0;
    for (index, (colour, name)) in legend.iter().enumerate() {
        let x = 10.0 + (index % 8) as f32 * 110.0;
        let y = base + (index / 8) as f32 * 24.0;
        svg.push_str(&format!(
            "<rect x=\"{x}\" y=\"{y}\" width=\"14\" height=\"14\" fill=\"{colour}\" stroke=\"#222\"/><text x=\"{}\" y=\"{}\" font-size=\"13\">{name}</text>\n",
            x + 20.0,
            y + 12.0
        ));
    }
    svg.push_str(&format!(
        "<text x=\"10\" y=\"{}\" font-size=\"12\">North is up; each ground band is a metre of height. 10 m = {:.0} px.</text>\n",
        base + 56.0,
        10.0 * scale
    ));
    svg.push_str("</svg>\n");
    svg
}
