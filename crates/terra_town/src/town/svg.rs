//! A town's plan as a picture to look at.

use super::*;

/// Something to mark on a plan: a place, a colour, how big, and a word by it.
#[derive(Clone, Debug, PartialEq)]
pub struct Mark {
    pub at: Vec2,
    pub colour: &'static str,
    pub size: f32,
    pub label: Option<String>,
}

/// A town's plan as an SVG to look at (`cargo xtask town`, and the life check's map): the
/// ground's height in metre bands, the square, the streets, every building's ground, the
/// yards, the things a day is done at, which way each looks, the path graph with anything cut
/// off from it in red, a legend, and marks laid over it all.
pub fn svg(town: &Town, heights: &dyn Fn(Vec2) -> f32, marks: &[Mark]) -> String {
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
            ThingKind::Barrel => "#6b4a2b",
            ThingKind::Cart => "#9c7a4a",
            ThingKind::Goods => "#c9a227",
            ThingKind::Site => "#ffffff",
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
    for mark in marks {
        let s = (mark.at - low) * scale;
        svg.push_str(&format!(
            "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"{:.1}\" fill=\"{}\" stroke=\"#111\" stroke-width=\"0.6\"/>\n",
            s.x, s.y, mark.size, mark.colour
        ));
        if let Some(label) = &mark.label {
            svg.push_str(&format!(
                "<text x=\"{:.1}\" y=\"{:.1}\" font-size=\"9\">{label}</text>\n",
                s.x + mark.size + 1.0,
                s.y + 3.0
            ));
        }
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
