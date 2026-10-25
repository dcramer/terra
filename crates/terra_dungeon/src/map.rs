//! The map sheet: dungeons drawn top down, a panel a dungeon and a column a level, with the
//! graph over the grid. Rooms are tinted by role, corridors grey, stairs striped yellow, water
//! blue, collapses hatched brown, a module other than the plain floor a dot, a placed feature
//! a square; walls dark, doors coloured by their link; each room's node a disc of its role's
//! colour with lines to its doors, red pips for mobs and gold pips for loot.

use crate::font;
use crate::graph::Link;
use crate::place::{STEPS, Side, Space};
use crate::theme::{Role, Theme};
use crate::{Dungeon, ModuleSet};
use std::path::Path;

/// Pixels a cell (3 m): two a building cube.
const CELL: i32 = 12;
const PAD: i32 = 16;
const TEXT: i32 = 2;
type Rgb = [u8; 3];

const BACK: Rgb = [22, 22, 26];
const WALL: Rgb = [8, 8, 10];
const CORRIDOR: Rgb = [92, 92, 98];
const STAIR: Rgb = [214, 196, 110];
const WATER: Rgb = [52, 96, 178];
const RUBBLE: Rgb = [112, 82, 52];
const INK: Rgb = [230, 230, 230];
const MOB: Rgb = [235, 50, 45];
const LOOT: Rgb = [250, 205, 60];

pub fn role_colour(role: Role) -> Rgb {
    match role {
        Role::Entrance => [80, 180, 90],
        Role::Passage => [140, 140, 150],
        Role::Branch => [110, 150, 190],
        Role::Guard => [200, 90, 60],
        Role::Goal => [240, 200, 50],
        Role::Puzzle => [70, 140, 240],
        Role::Shortcut => [170, 120, 200],
        Role::Secret => [150, 60, 170],
        Role::Rest => [240, 140, 50],
        Role::Treasure => [210, 170, 100],
    }
}

pub fn link_colour(link: Link) -> Rgb {
    match link {
        Link::Corridor => [170, 170, 170],
        Link::Door => [160, 110, 60],
        Link::Locked(_) => [240, 30, 30],
        Link::OneWay => [250, 150, 30],
        Link::Secret => [190, 70, 230],
    }
}

/// Pixels, three bytes each.
pub struct Canvas {
    pub width: i32,
    pub height: i32,
    pub rgb: Vec<u8>,
}

fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    std::array::from_fn(|i| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * t) as u8)
}

impl Canvas {
    pub fn new(width: i32, height: i32) -> Canvas {
        Canvas {
            width,
            height,
            rgb: BACK.repeat((width * height) as usize),
        }
    }

    pub fn put(&mut self, x: i32, y: i32, c: Rgb) {
        if x >= 0 && y >= 0 && x < self.width && y < self.height {
            let i = ((y * self.width + x) * 3) as usize;
            self.rgb[i..i + 3].copy_from_slice(&c);
        }
    }

    pub fn fill(&mut self, x: i32, y: i32, w: i32, h: i32, c: Rgb) {
        for py in y..y + h {
            for px in x..x + w {
                self.put(px, py, c);
            }
        }
    }

    pub fn line(&mut self, (x0, y0): (i32, i32), (x1, y1): (i32, i32), c: Rgb) {
        let steps = (x1 - x0).abs().max((y1 - y0).abs()).max(1);
        for k in 0..=steps {
            self.put(x0 + (x1 - x0) * k / steps, y0 + (y1 - y0) * k / steps, c);
        }
    }

    pub fn disc(&mut self, (cx, cy): (i32, i32), r: i32, c: Rgb) {
        for y in -r - 1..=r + 1 {
            for x in -r - 1..=r + 1 {
                let d = x * x + y * y;
                if d <= r * r {
                    self.put(cx + x, cy + y, c);
                } else if d <= (r + 1) * (r + 1) {
                    self.put(cx + x, cy + y, WALL);
                }
            }
        }
    }

    /// Text at a scale of `TEXT` pixels a font pixel; returns the width drawn.
    pub fn text(&mut self, x: i32, y: i32, text: &str, c: Rgb) -> i32 {
        for (i, ch) in text.chars().enumerate() {
            for gy in 0..5 {
                for gx in 0..3 {
                    if font::lit(ch, gx, gy) {
                        let px = x + (i as i32 * 4 + gx as i32) * TEXT;
                        self.fill(px, y + gy as i32 * TEXT, TEXT, TEXT, c);
                    }
                }
            }
        }
        text.chars().count() as i32 * 4 * TEXT
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut encoder = png::Encoder::new(
            std::io::BufWriter::new(file),
            self.width as u32,
            self.height as u32,
        );
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|e| format!("{}: {e}", path.display()))?;
        writer
            .write_image_data(&self.rgb)
            .map_err(|e| format!("{}: {e}", path.display()))
    }
}

/// A sheet of a theme's dungeons, two panels a row, under a title and a legend.
pub fn sheet(theme: &Theme, set: &ModuleSet, dungeons: &[(u64, Dungeon)]) -> Canvas {
    let levels = theme.levels as i32;
    let (mut w, mut h) = (1, 1);
    for (_, d) in dungeons {
        let (lo, hi) = d.layout.bounds();
        w = w.max(hi.x - lo.x + 1);
        h = h.max(hi.z - lo.z + 1);
    }
    let panel = (levels * (w * CELL + PAD) + PAD, h * CELL + PAD * 3);
    let columns = 2;
    let rows = dungeons.len().div_ceil(columns as usize) as i32;
    let top = 26 * TEXT;
    let mut canvas = Canvas::new((panel.0 * columns).max(900), top + panel.1 * rows);
    let title = format!(
        "{}: {}  ({} LEVELS, {}-{} ROOMS)",
        theme.name, theme.title, levels, theme.rooms.0, theme.rooms.1
    );
    canvas.text(PAD, PAD / 2, &title.replace('_', " "), INK);
    legend(&mut canvas, PAD, PAD / 2 + 8 * TEXT);
    for (i, (seed, dungeon)) in dungeons.iter().enumerate() {
        let x = (i as i32 % columns) * panel.0;
        let y = top + (i as i32 / columns) * panel.1;
        draw(&mut canvas, (x, y), (w, h), *seed, dungeon, set, theme);
    }
    canvas
}

fn legend(canvas: &mut Canvas, x: i32, y: i32) {
    let roles = [
        Role::Entrance,
        Role::Passage,
        Role::Branch,
        Role::Puzzle,
        Role::Guard,
        Role::Goal,
        Role::Shortcut,
        Role::Secret,
        Role::Rest,
        Role::Treasure,
    ];
    let mut at = x;
    for role in roles {
        canvas.disc((at + 4, y + 4), 4, role_colour(role));
        at += 12 + canvas.text(at + 12, y, &format!("{role:?}"), INK) + 8;
    }
    let mut at = x;
    let y = y + 8 * TEXT;
    let links = [
        (Link::Corridor, "ARCH"),
        (Link::Door, "DOOR"),
        (Link::Locked(0), "LOCKED"),
        (Link::OneWay, "ONE WAY"),
        (Link::Secret, "SECRET"),
    ];
    for (link, name) in links {
        canvas.fill(at, y + 3, 14, 4, link_colour(link));
        at += 18 + canvas.text(at + 18, y, name, INK) + 8;
    }
    let swatches = [
        (CORRIDOR, "CORRIDOR"),
        (STAIR, "STAIR"),
        (WATER, "WATER"),
        (RUBBLE, "COLLAPSE"),
        (MOB, "MOB"),
        (LOOT, "LOOT"),
    ];
    for (c, name) in swatches {
        canvas.fill(at, y, 10, 10, c);
        at += 14 + canvas.text(at + 14, y, name, INK) + 8;
    }
}

/// One dungeon's panel: a column a level.
fn draw(
    canvas: &mut Canvas,
    (x, y): (i32, i32),
    (w, _h): (i32, i32),
    seed: u64,
    d: &Dungeon,
    set: &ModuleSet,
    theme: &Theme,
) {
    let (lo, _) = d.layout.bounds();
    let fell = if d.dressing.fallback {
        ", PLAIN FLOORS"
    } else {
        ""
    };
    let label = format!(
        "SEED {seed}: {} ROOMS, {} CELLS, PLACED IN {}, DRESSED IN {}{fell}",
        d.graph.rooms.len(),
        d.layout.cells.len(),
        d.layout.attempts,
        d.dressing.tries
    );
    canvas.text(x + PAD, y + 4, &label, INK);
    let origin = |level: i32| (x + PAD + level * (w * CELL + PAD), y + PAD * 2);
    for level in 0..theme.levels as i32 {
        let (ox, oy) = origin(level);
        canvas.text(ox, oy - 6 * TEXT, &format!("L{level}"), [150, 150, 150]);
    }
    let px = |at: glam::IVec3| {
        let (ox, oy) = origin(at.y);
        (ox + (at.x - lo.x) * CELL, oy + (at.z - lo.z) * CELL)
    };
    for (i, cell) in d.layout.cells.iter().enumerate() {
        let (cx, cy) = px(cell.at);
        let v = d.dressing.picks[i];
        let module = set.variants[v].module;
        let mut floor = match cell.space {
            Space::Room(r) => mix(role_colour(d.graph.rooms[r].role), BACK, 0.45),
            Space::Corridor(_) => CORRIDOR,
            Space::Stair(_) => STAIR,
        };
        if set.wet(v) {
            floor = mix(floor, WATER, 0.75);
        }
        canvas.fill(cx, cy, CELL, CELL, floor);
        if set.blocks(v) {
            for k in 0..CELL {
                canvas.put(cx + k, cy + k, RUBBLE);
                canvas.put(cx + CELL - 1 - k, cy + k, RUBBLE);
            }
        }
        if let Space::Stair(s) = cell.space {
            let up = STEPS[d.layout.stairs[s].up];
            // Treads across the way it rises.
            for k in (2..CELL).step_by(3) {
                if up.x != 0 {
                    canvas.line((cx + k, cy), (cx + k, cy + CELL - 1), [120, 100, 40]);
                } else {
                    canvas.line((cx, cy + k), (cx + CELL - 1, cy + k), [120, 100, 40]);
                }
            }
        }
        let m = &theme.modules[module];
        if m.fixed && !m.spaces.contains(&crate::theme::Space::Stair) {
            canvas.fill(cx + 3, cy + 3, CELL - 6, CELL - 6, [250, 250, 250]);
            let role = match cell.space {
                Space::Room(r) => d.graph.rooms[r].role,
                _ => Role::Goal,
            };
            canvas.fill(cx + 4, cy + 4, CELL - 8, CELL - 8, role_colour(role));
        } else if !m.safe && !set.wet(v) && !m.blocks {
            let hue = palette(module);
            canvas.fill(cx + CELL / 2 - 1, cy + CELL / 2 - 1, 3, 3, hue);
        }
    }
    for cell in &d.layout.cells {
        let (cx, cy) = px(cell.at);
        for (side, kind) in cell.sides.iter().enumerate() {
            let (a, b) = edge(cx, cy, side);
            match kind {
                Side::Wall => canvas.line(a, b, WALL),
                Side::Door(e) => {
                    canvas.line(a, b, WALL);
                    let c = link_colour(d.graph.edges[*e].link);
                    let mid = ((a.0 + b.0) / 2, (a.1 + b.1) / 2);
                    canvas.fill(mid.0 - 2, mid.1 - 2, 5, 5, c);
                }
                Side::Way | Side::Open => {}
            }
        }
    }
    overlay(canvas, d, &px);
}

/// The graph over the grid: nodes at room centres, a line from each to its doors.
fn overlay(canvas: &mut Canvas, d: &Dungeon, px: &dyn Fn(glam::IVec3) -> (i32, i32)) {
    let centre = |room: usize| {
        let r = d.layout.rooms[room];
        let (x, y) = px(r.min);
        (x + r.size.x * CELL / 2, y + r.size.y * CELL / 2)
    };
    for cell in &d.layout.cells {
        let Space::Room(room) = cell.space else {
            continue;
        };
        let (cx, cy) = px(cell.at);
        for (side, kind) in cell.sides.iter().enumerate() {
            if let Side::Door(e) = kind {
                let (a, b) = edge(cx, cy, side);
                let mid = ((a.0 + b.0) / 2, (a.1 + b.1) / 2);
                canvas.line(centre(room), mid, link_colour(d.graph.edges[*e].link));
            }
        }
    }
    for (room, r) in d.graph.rooms.iter().enumerate() {
        let (x, y) = centre(room);
        canvas.disc((x, y), 4, role_colour(r.role));
        let mobs: u32 = r.mobs.iter().map(|m| m.count).sum();
        let loot = r.loot.len() as i32;
        for k in 0..mobs.min(8) as i32 {
            canvas.fill(x - 8 + k * 3, y + 7, 2, 2, MOB);
        }
        for k in 0..loot.min(6) {
            canvas.fill(x - 8 + k * 3, y + 10, 2, 2, LOOT);
        }
    }
}

/// The two ends of a cell's side, in pixels.
fn edge(x: i32, y: i32, side: usize) -> ((i32, i32), (i32, i32)) {
    let e = CELL - 1;
    match side {
        0 => ((x, y), (x + e, y)),
        1 => ((x + e, y), (x + e, y + e)),
        2 => ((x, y + e), (x + e, y + e)),
        _ => ((x, y), (x, y + e)),
    }
}

/// A colour a module, spread round the wheel.
pub fn palette(module: usize) -> Rgb {
    const WHEEL: [Rgb; 8] = [
        [230, 120, 120],
        [120, 230, 140],
        [120, 170, 240],
        [240, 220, 120],
        [210, 130, 230],
        [120, 230, 230],
        [240, 170, 110],
        [190, 190, 190],
    ];
    WHEEL[module % WHEEL.len()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::assets;

    #[test]
    fn a_sheet_draws_its_dungeons() {
        let theme = Theme::all(&assets()).unwrap().remove(0);
        let set = ModuleSet::new(&theme).unwrap();
        let dungeons: Vec<(u64, Dungeon)> = (0..2)
            .filter_map(|s| {
                crate::generate(&theme, &set, terra_gen::Seed(s))
                    .ok()
                    .map(|d| (s, d))
            })
            .collect();
        let canvas = sheet(&theme, &set, &dungeons);
        assert_eq!(
            canvas.rgb.len(),
            (canvas.width * canvas.height * 3) as usize
        );
        let lit = canvas
            .rgb
            .chunks(3)
            .filter(|p| *p == role_colour(Role::Goal))
            .count();
        assert!(lit > 0, "the goal's node is drawn");
    }
}
