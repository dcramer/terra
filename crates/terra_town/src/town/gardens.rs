//! A walled town's gardens (D381): the ground within its wall that no house, yard, street or
//! work stands on laid out in plots, as a walled town's was. Its wall was drawn round its
//! streets' houses and the castle, and between the streets the meadow lay empty, a dark field
//! inside the circuit (the review of round 13); a town grown within its walls had its
//! burgage plots' gardens there, beds of vegetables in rows, greens kept for bleaching and
//! drying, and ground fresh dug, in long plots square to its streets, a balk of grass
//! between each.

use super::*;

/// How big a garden's plot is, along the town's contour (`Square::along`) and across it:
/// long plots running back from the streets along the contour, as burgage plots run.
const PLOT_M: Vec2 = Vec2::new(8.0, 14.0);
/// How wide the path between two plots is, half each side (`terra_folk::town_gardens` treads it pale).
pub const BALK_M: f32 = 0.6;
/// How far a plot keeps from the wall's line (its towers and gatehouses reach in to six
/// metres), from anything built, from a street's edge, the square, a yard, a lane and a
/// thing.
const FROM_WALL_M: f32 = 7.0;
const FROM_BUILT_M: f32 = 1.5;
const FROM_STREET_M: f32 = 1.5;
const FROM_SQUARE_M: f32 = 1.5;
const FROM_YARD_M: f32 = 0.5;
const FROM_LANE_M: f32 = 1.0;

/// What a garden holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GardenKind {
    /// Beds of vegetables in rows along the plot, earth between them.
    Beds,
    /// A green kept short and lush, for bleaching linen and drying.
    Green,
    /// Ground fresh dug, a few weeds through it.
    Dug,
}

/// One garden: its plot on the town's grid, the home whose it is (it is laid out when that
/// house is finished), and what it holds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Garden {
    pub cell: [i32; 2],
    pub home: usize,
    pub kind: GardenKind,
}

/// A town's gardens, on a grid square to its contour so its plots run side by side.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Gardens {
    /// The grid's corner, and the way along the contour its cells' first side runs.
    pub origin: Vec2,
    pub along: Vec2,
    pub plots: Vec<Garden>,
}

impl Gardens {
    /// The cell a point stands in, and where in it, 0 to 1 each way.
    pub fn cell_at(&self, p: Vec2) -> ([i32; 2], Vec2) {
        let d = p - self.origin;
        let across = self.along.perp();
        let u = Vec2::new(d.dot(self.along) / PLOT_M.x, d.dot(across) / PLOT_M.y);
        let cell = u.floor();
        ([cell.x as i32, cell.y as i32], u - cell)
    }

    /// A plot's size, along the contour and across it, in metres.
    pub fn plot_m() -> Vec2 {
        PLOT_M
    }

    /// A cell's ground, its corners in order round it, inset by `inset` metres.
    pub fn quad(&self, cell: [i32; 2], inset: f32) -> [Vec2; 4] {
        let across = self.along.perp();
        let corner = |x: f32, y: f32| self.origin + self.along * x + across * y;
        let (x0, y0) = (cell[0] as f32 * PLOT_M.x, cell[1] as f32 * PLOT_M.y);
        let (x1, y1) = (x0 + PLOT_M.x, y0 + PLOT_M.y);
        [
            corner(x0 + inset, y0 + inset),
            corner(x1 - inset, y0 + inset),
            corner(x1 - inset, y1 - inset),
            corner(x0 + inset, y1 - inset),
        ]
    }
}

impl Town {
    /// Lays out the gardens within the wall, where there is room: none if it is open.
    pub(super) fn gardens(&mut self, seed: u32) {
        if self.wall.len() < 3 || self.homes.is_empty() {
            return;
        }
        let along = self.square.along.normalize_or(Vec2::X);
        let (low, high) = self
            .wall
            .iter()
            .fold((Vec2::MAX, Vec2::MIN), |(l, h), &p| (l.min(p), h.max(p)));
        let mut gardens = Gardens {
            origin: Vec2::ZERO,
            along,
            plots: Vec::new(),
        };
        // The cells the wall's box covers, in the grid's terms.
        let corners = [
            low,
            Vec2::new(high.x, low.y),
            high,
            Vec2::new(low.x, high.y),
        ];
        let cells: Vec<[i32; 2]> = corners.iter().map(|&c| gardens.cell_at(c).0).collect();
        let (lo, hi) = cells
            .iter()
            .fold(([i32::MAX; 2], [i32::MIN; 2]), |(l, h), c| {
                (
                    [l[0].min(c[0]), l[1].min(c[1])],
                    [h[0].max(c[0]), h[1].max(c[1])],
                )
            });
        // What it must keep clear of, each with a middle and a reach for a quick look first.
        let built: Vec<([Vec2; 4], Vec2, f32)> = self
            .footprints
            .iter()
            .flatten()
            .map(|q| bounded(*q))
            .collect();
        let yards: Vec<([Vec2; 4], Vec2, f32)> =
            self.yards.iter().map(|(_, q)| bounded(*q)).collect();
        let things: Vec<([Vec2; 4], Vec2, f32)> = self
            .things
            .iter()
            .filter_map(Thing::outline)
            .map(bounded)
            .collect();
        let reach = PLOT_M.length() * 0.5;
        let near = |q: &[Vec2; 4], m: Vec2, list: &[([Vec2; 4], Vec2, f32)], apart: f32| {
            list.iter()
                .filter(|(_, c, r)| c.distance(m) < reach + r + apart)
                .all(|(other, _, _)| polygons(q, other) >= apart)
        };
        let ring = self.wall.clone();
        for x in lo[0]..=hi[0] {
            for y in lo[1]..=hi[1] {
                let quad = gardens.quad([x, y], 0.0);
                let middle = quad.iter().copied().sum::<Vec2>() / 4.0;
                let room = quad.iter().all(|&c| {
                    within(c, &ring)
                        && (0..ring.len()).all(|i| {
                            point_segment(c, ring[i], ring[(i + 1) % ring.len()]) >= FROM_WALL_M
                        })
                }) && near(&quad, middle, &built, FROM_BUILT_M)
                    && near(&quad, middle, &yards, FROM_YARD_M)
                    && near(&quad, middle, &things, FROM_YARD_M)
                    && self
                        .streets
                        .iter()
                        .all(|s| from_street(&quad, s) >= s.wide * 0.5 + FROM_STREET_M)
                    && polygons(&quad, &self.square.outline) >= FROM_SQUARE_M
                    && self.lanes.iter().all(|(_, lane)| {
                        lane.windows(2)
                            .all(|pair| segment_polygon(pair[0], pair[1], &quad) >= FROM_LANE_M)
                    });
                if !room {
                    continue;
                }
                // Whose: the nearest home's.
                let Some(&home) = self.homes.iter().min_by(|&&a, &&b| {
                    let d = |h: usize| Vec2::from(self.buildings[h].at).distance(middle);
                    d(a).total_cmp(&d(b))
                }) else {
                    continue;
                };
                // Two plots side by side along the contour are one holding's, of a kind.
                let roll = hash::unit(hash::of(&[seed, 0x6a7d, x.div_euclid(2) as u32, y as u32]));
                let kind = if roll < 0.5 {
                    GardenKind::Beds
                } else if roll < 0.8 {
                    GardenKind::Green
                } else {
                    GardenKind::Dug
                };
                gardens.plots.push(Garden {
                    cell: [x, y],
                    home,
                    kind,
                });
            }
        }
        self.gardens = gardens;
    }
}

/// A quad with its middle and how far its corners reach from it.
fn bounded(q: [Vec2; 4]) -> ([Vec2; 4], Vec2, f32) {
    let middle = q.iter().copied().sum::<Vec2>() / 4.0;
    let reach = q.iter().map(|c| c.distance(middle)).fold(0.0, f32::max);
    (q, middle, reach)
}

/// Whether a point is within a polygon, convex or not (by the crossings of a ray east).
fn within(at: Vec2, polygon: &[Vec2]) -> bool {
    let mut odd = false;
    for i in 0..polygon.len() {
        let (a, b) = (polygon[i], polygon[(i + 1) % polygon.len()]);
        if (a.y > at.y) != (b.y > at.y) && at.x < a.x + (at.y - a.y) / (b.y - a.y) * (b.x - a.x) {
            odd = !odd;
        }
    }
    odd
}
