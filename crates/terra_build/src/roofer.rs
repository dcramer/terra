//! The roofer (D241): "add a roof with this material" over a footprint, as the developers
//! asked (D226), laying roof cubes as anything else is laid, so a roof it lays is changed,
//! extended or built over by hand like one built a cube at a time.
//!
//! **The surface.** A roof over a footprint of columns is a height field over it, rising a
//! cube for every cube it runs in from the walls' outer faces, as every roof of cubes cut to
//! slopes does (45°):
//!
//! - a **hip** roof's height is how far a point is from the footprint's edge, square on or
//!   corner to corner, whichever is nearer (the chessboard distance to what is outside it):
//!   hips at its outer corners, valleys at its inner ones, a ridge along the middle of every
//!   arm, the straight skeleton of a plan whose walls run the two ways (research on roofs,
//!   docs/building-kits.md);
//! - a **gable** roof's is, over each of the largest rectangles the footprint holds, how far
//!   a point is from the nearer of its long sides, the highest of them where they overlap,
//!   so each arm of an L, T or U has its own ridge and they cross where the arms meet;
//! - a **lean-to** rises from its low edge across the footprint.
//!
//! **The cubes.** Each column's cube sits on the layer its lowest corner's height says, cut
//! to the shape its corners' heights make: a slope where two corners along a side are high,
//! an outer corner (a hip's) where one is, an inner corner (a valley's) where three are; a
//! whole cube a layer down where none is and its middle is higher, so a ridge a cube wide is
//! flat on top. Where the footprint's edge has roof over it, at a gable end, the wall under
//! it is carried up to the roof as gable cubes, whose tops the roof cuts (`layout::roofs`).

use crate::{Facing, Shape};
use glam::{IVec3, Vec2};
use std::collections::{BTreeMap, BTreeSet};

/// How a roof the roofer lays slopes.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    serde::Serialize,
    serde::Deserialize,
)]
pub enum Style {
    /// Up from every side to a ridge or a point.
    Hip,
    /// Up from the long sides of each arm to its ridge, with upright gable ends.
    #[default]
    Gable,
    /// One slope, rising toward the south, or toward the east if `along_x` is false.
    LeanTo,
}

/// A roof laid out: its cubes, each with its shape, and the cubes that carry the walls under
/// its gable ends up to it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Roofed {
    pub roof: Vec<(IVec3, Shape)>,
    pub gables: Vec<IVec3>,
}

/// The roof of a style over a footprint of columns, counted in cubes east and south, its
/// lowest cubes on `layer`, the first over the walls' tops. `along_x` says which way a
/// gable's ridges run, or a lean-to's high side, where the footprint does not; none, along
/// each arm's length.
pub fn over(
    footprint: &BTreeSet<[i32; 2]>,
    layer: i32,
    style: Style,
    along_x: Option<bool>,
) -> Roofed {
    let height = field(footprint, style, along_x);
    cubes(footprint, layer, &height)
}

/// The roof's height over a point of a footprint, in cubes over its walls' tops, the point
/// in cubes east and south.
pub fn field(
    footprint: &BTreeSet<[i32; 2]>,
    style: Style,
    along_x: Option<bool>,
) -> impl Fn(Vec2) -> f32 + '_ {
    let ring = outside_ring(footprint);
    let rectangles = match style {
        Style::Gable => largest_rectangles(footprint),
        _ => Vec::new(),
    };
    move |p: Vec2| match style {
        Style::Hip => ring
            .iter()
            .map(|&[x, z]| {
                let dx = (x as f32 - p.x).max(p.x - (x + 1) as f32).max(0.0);
                let dz = (z as f32 - p.y).max(p.y - (z + 1) as f32).max(0.0);
                dx.max(dz)
            })
            .fold(f32::MAX, f32::min),
        Style::Gable => rectangles
            .iter()
            .filter(|r| r.holds(p))
            .map(|r| r.gable(p, along_x))
            .fold(0.0, f32::max),
        Style::LeanTo => {
            // How far the point is from the start of the run of footprint it is in, the way
            // the roof rises: from the edge behind it, the run either side if it is on one.
            let rising_x = along_x == Some(false);
            let (along, across) = if rising_x { (p.x, p.y) } else { (p.y, p.x) };
            let inside =
                |c: i32, k: i32| footprint.contains(&if rising_x { [k, c] } else { [c, k] });
            let mut best: f32 = 0.0;
            for c in [libm::floorf(across - 1e-4), libm::floorf(across + 1e-4)] {
                let c = c as i32;
                let mut k = libm::floorf(along - 1e-4) as i32;
                if !inside(c, k) {
                    continue;
                }
                while inside(c, k - 1) {
                    k -= 1;
                }
                best = best.max(along - k as f32);
            }
            best
        }
    }
}

/// A rectangle of columns, from one to another inclusive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rectangle {
    pub min: [i32; 2],
    pub max: [i32; 2],
}

impl Rectangle {
    /// Whether a point, in cubes, is on it or its edge.
    pub fn holds(&self, p: Vec2) -> bool {
        p.x >= self.min[0] as f32 - 1e-4
            && p.x <= (self.max[0] + 1) as f32 + 1e-4
            && p.y >= self.min[1] as f32 - 1e-4
            && p.y <= (self.max[1] + 1) as f32 + 1e-4
    }

    /// A gable's height over a point of it: how far it is from the nearer of its sides
    /// along its ridge, which runs along its length, or as `along_x` says.
    pub fn gable(&self, p: Vec2, along_x: Option<bool>) -> f32 {
        let (w, d) = (self.max[0] - self.min[0] + 1, self.max[1] - self.min[1] + 1);
        if along_x.unwrap_or(w >= d) {
            (p.y - self.min[1] as f32).min((self.max[1] + 1) as f32 - p.y)
        } else {
            (p.x - self.min[0] as f32).min((self.max[0] + 1) as f32 - p.x)
        }
    }

    /// A roof's height over a point of it in a style, its gable's ridge or its lean-to's
    /// rise along X, or not, as a section of a roof over several is (D209's): its hip's, as
    /// far as the nearest side; its gable's, as far as the nearer long side; its lean-to's,
    /// as far as its low side. None off it.
    pub fn height(&self, style: Style, along_x: bool, p: Vec2) -> Option<f32> {
        if !self.holds(p) {
            return None;
        }
        let (west, east) = (p.x - self.min[0] as f32, (self.max[0] + 1) as f32 - p.x);
        let (north, south) = (p.y - self.min[1] as f32, (self.max[1] + 1) as f32 - p.y);
        Some(match (style, along_x) {
            (Style::Hip, _) => west.min(east).min(north).min(south),
            (Style::Gable, true) => north.min(south),
            (Style::Gable, false) => west.min(east),
            (Style::LeanTo, true) => north,
            (Style::LeanTo, false) => west,
        })
    }

    /// Its columns.
    pub fn columns(&self) -> impl Iterator<Item = [i32; 2]> + '_ {
        (self.min[0]..=self.max[0])
            .flat_map(move |x| (self.min[1]..=self.max[1]).map(move |z| [x, z]))
    }
}

/// The columns just outside a footprint, square on or corner to corner: those nearest to
/// any point of it of all that are outside.
fn outside_ring(footprint: &BTreeSet<[i32; 2]>) -> Vec<[i32; 2]> {
    let mut ring = BTreeSet::new();
    for &[x, z] in footprint {
        for dx in -1..=1 {
            for dz in -1..=1 {
                let at = [x + dx, z + dz];
                if !footprint.contains(&at) {
                    ring.insert(at);
                }
            }
        }
    }
    ring.into_iter().collect()
}

/// Every rectangle a footprint holds that cannot be made longer either way.
pub fn largest_rectangles(footprint: &BTreeSet<[i32; 2]>) -> Vec<Rectangle> {
    let Some(&first) = footprint.iter().next() else {
        return Vec::new();
    };
    let (mut low, mut high) = (first, first);
    for &[x, z] in footprint {
        low = [low[0].min(x), low[1].min(z)];
        high = [high[0].max(x), high[1].max(z)];
    }
    let row_holds = |z: i32, x0: i32, x1: i32| (x0..=x1).all(|x| footprint.contains(&[x, z]));
    let mut found = Vec::new();
    for z0 in low[1]..=high[1] {
        for z1 in z0..=high[1] {
            // The columns every row from z0 to z1 holds, in runs.
            let mut x = low[0];
            while x <= high[0] {
                if !(z0..=z1).all(|z| footprint.contains(&[x, z])) {
                    x += 1;
                    continue;
                }
                let x0 = x;
                while x < high[0] && (z0..=z1).all(|z| footprint.contains(&[x + 1, z])) {
                    x += 1;
                }
                let x1 = x;
                x += 1;
                // Longer north or south, it is not the largest.
                if row_holds(z0 - 1, x0, x1) || row_holds(z1 + 1, x0, x1) {
                    continue;
                }
                found.push(Rectangle {
                    min: [x0, z0],
                    max: [x1, z1],
                });
            }
        }
    }
    found
}

/// A roof over several rectangles, each of a style and its ridge or rise along X or not, the
/// highest of them where they overlap, so a wing's roof meets the main one's in a valley:
/// its footprint, and its height over each point.
#[allow(clippy::type_complexity)]
pub fn sections(
    parts: &[(Rectangle, Style, bool)],
) -> (BTreeSet<[i32; 2]>, impl Fn(Vec2) -> f32 + '_) {
    let footprint = parts.iter().flat_map(|(r, _, _)| r.columns()).collect();
    let height = move |p: Vec2| {
        parts
            .iter()
            .filter_map(|(r, style, along_x)| r.height(*style, *along_x, p))
            .fold(0.0, f32::max)
    };
    (footprint, height)
}

/// The shortcuts that put a roof up built at once over a footprint of a base on a storey
/// (`BuildOp`, as the yard, the kit sheet and a village build): in the place of the flat roof
/// the room tool laid inside its walls, its cubes in `kind` as `height` gives them, and the
/// walls under its gable ends carried up to it in the kind of each wall's top.
pub fn put_up(
    base: &crate::Base,
    footprint: &BTreeSet<[i32; 2]>,
    storey: i32,
    kind: crate::Kind,
    height: &dyn Fn(Vec2) -> f32,
) -> Vec<crate::BuildOp> {
    use crate::{BuildOp, Cube, Fill, STOREY_CUBES};
    let layer = crate::wall_layer(storey) + STOREY_CUBES;
    let mut ops = Vec::new();
    for &[x, z] in footprint {
        if base.filling(IVec3::new(x, storey, z)).is_none() {
            ops.push(BuildOp::Fill {
                min: [x, layer - 1, z],
                max: [x, layer - 1, z],
                cube: None,
            });
        }
    }
    let plan = cubes(footprint, layer, height);
    for (at, shape) in plan.roof {
        ops.push(BuildOp::Fill {
            min: at.to_array(),
            max: at.to_array(),
            cube: Some(Cube {
                shape,
                ..Cube::new(kind, Fill::Solid)
            }),
        });
    }
    for at in plan.gables {
        let wall = (layer - STOREY_CUBES..layer)
            .rev()
            .find_map(|y| base.cube(IVec3::new(at.x, y, at.z)));
        if let Some(wall) = wall.filter(|wall| !wall.kind.is_roof()) {
            ops.push(BuildOp::Fill {
                min: at.to_array(),
                max: at.to_array(),
                cube: Some(Cube::new(wall.kind, Fill::Solid)),
            });
        }
    }
    ops
}

/// The cubes of a roof over a footprint whose height over each point `height` gives, its
/// lowest on `layer`; and those carrying the walls under its edges up to it.
pub fn cubes(footprint: &BTreeSet<[i32; 2]>, layer: i32, height: &dyn Fn(Vec2) -> f32) -> Roofed {
    let mut corners: BTreeMap<[i32; 2], i32> = BTreeMap::new();
    let mut at = |x: i32, z: i32| -> i32 {
        *corners
            .entry([x, z])
            .or_insert_with(|| libm::roundf(height(Vec2::new(x as f32, z as f32))) as i32)
    };
    let mut out = Roofed::default();
    for &[x, z] in footprint {
        // North-west, north-east, south-east, south-west.
        let c = [at(x, z), at(x + 1, z), at(x + 1, z + 1), at(x, z + 1)];
        let low = *c.iter().min().expect("four corners");
        let high = c.map(|h| h > low);
        let middle = height(Vec2::new(x as f32 + 0.5, z as f32 + 0.5));
        let (rise, shape) = shape_of(high, middle > low as f32 + 0.25);
        // A ridge's flat top over the walls themselves, where the footprint is a cube wide:
        // on them, not in them.
        let y = layer + (low + rise).max(0);
        out.roof.push((IVec3::new(x, y, z), shape));
        // The wall under an edge of the footprint carried up to the roof over it.
        let edge = [[1, 0], [-1, 0], [0, 1], [0, -1]]
            .iter()
            .any(|[dx, dz]| !footprint.contains(&[x + dx, z + dz]));
        if edge {
            out.gables.extend((layer..y).map(|g| IVec3::new(x, g, z)));
        }
    }
    out
}

/// The shape of a cube whose corners, north-west, north-east, south-east and south-west,
/// are high or low, and how far up from the low corners' layer it sits: down one where no
/// corner is high but its middle is, a ridge's whole cube under its flat top.
/// The heights of a roof cube's top corners, north-west, north-east, south-east and
/// south-west, in cubes over `layer`, as [`cubes`] laid it: none for a shape it never lays.
pub fn corner_heights(at: IVec3, shape: Shape, layer: i32) -> Option<[i32; 4]> {
    let low = at.y - layer;
    if shape == Shape::Whole {
        return Some([low + 1; 4]);
    }
    (0..16u8)
        .map(|bits| [0, 1, 2, 3].map(|i| bits & (1 << i) != 0))
        .find(|&high| shape_of(high, false) == (0, shape))
        .map(|high| high.map(|h| low + i32::from(h)))
}

fn shape_of(high: [bool; 4], peak: bool) -> (i32, Shape) {
    use Facing::{East, North, South, West};
    match high {
        [false, false, false, false] if peak => (-1, Shape::Whole),
        [false, false, false, false] => (-1, Shape::Whole),
        [true, true, false, false] => (0, Shape::Slope(North)),
        [false, true, true, false] => (0, Shape::Slope(East)),
        [false, false, true, true] => (0, Shape::Slope(South)),
        [true, false, false, true] => (0, Shape::Slope(West)),
        [false, true, false, false] => (0, Shape::OuterCorner(North)),
        [false, false, true, false] => (0, Shape::OuterCorner(East)),
        [false, false, false, true] => (0, Shape::OuterCorner(South)),
        [true, false, false, false] => (0, Shape::OuterCorner(West)),
        [true, true, true, false] => (0, Shape::InnerCorner(North)),
        [false, true, true, true] => (0, Shape::InnerCorner(East)),
        [true, false, true, true] => (0, Shape::InnerCorner(South)),
        [true, true, false, true] => (0, Shape::InnerCorner(West)),
        // Two corners across from each other, a saddle: flat at the higher.
        _ => (0, Shape::Whole),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rectangle(w: i32, d: i32) -> BTreeSet<[i32; 2]> {
        (0..w).flat_map(|x| (0..d).map(move |z| [x, z])).collect()
    }

    fn at(roofed: &Roofed, x: i32, z: i32) -> (i32, Shape) {
        roofed
            .roof
            .iter()
            .find(|(at, _)| at.x == x && at.z == z)
            .map(|(at, shape)| (at.y, *shape))
            .unwrap()
    }

    #[test]
    fn a_gable_rises_a_cube_a_cube_to_its_ridge_with_gables_at_its_ends() {
        // Five metres by four, the ridge along X.
        let roofed = over(&rectangle(10, 8), 6, Style::Gable, None);
        assert_eq!(roofed.roof.len(), 80);
        for x in 0..10 {
            for z in 0..4 {
                assert_eq!(at(&roofed, x, z), (6 + z, Shape::Slope(Facing::South)));
                assert_eq!(at(&roofed, x, 7 - z), (6 + z, Shape::Slope(Facing::North)));
            }
        }
        // Each gable end carried up under its slopes: 0, 1, 2, 3, 3, 2, 1, 0 cubes.
        assert_eq!(roofed.gables.len(), 2 * (1 + 2 + 3) * 2);
        assert!(roofed.gables.iter().all(|g| g.x == 0 || g.x == 9));
    }

    #[test]
    fn an_odd_span_has_a_flat_ridge_a_cube_wide() {
        let roofed = over(&rectangle(10, 9), 6, Style::Gable, None);
        // The middle row: whole cubes level with the slopes' tops.
        assert_eq!(at(&roofed, 3, 4), (6 + 3, Shape::Whole));
        assert_eq!(at(&roofed, 3, 3), (6 + 3, Shape::Slope(Facing::South)));
        assert_eq!(at(&roofed, 3, 5), (6 + 3, Shape::Slope(Facing::North)));
    }

    #[test]
    fn a_hip_has_outer_corners_at_its_corners_and_slopes_all_round() {
        let roofed = over(&rectangle(10, 8), 6, Style::Hip, None);
        use Facing::*;
        assert_eq!(at(&roofed, 0, 0), (6, Shape::OuterCorner(East)));
        assert_eq!(at(&roofed, 9, 0), (6, Shape::OuterCorner(South)));
        assert_eq!(at(&roofed, 9, 7), (6, Shape::OuterCorner(West)));
        assert_eq!(at(&roofed, 0, 7), (6, Shape::OuterCorner(North)));
        assert_eq!(at(&roofed, 4, 0), (6, Shape::Slope(South)));
        assert_eq!(at(&roofed, 0, 3), (6, Shape::Slope(East)));
        // Its ridge: two metres long between the hips, slopes meeting either side.
        assert_eq!(at(&roofed, 4, 3), (9, Shape::Slope(South)));
        assert_eq!(at(&roofed, 3, 3), (9, Shape::OuterCorner(East)));
        assert!(roofed.gables.is_empty());
    }

    #[test]
    fn an_l_hipped_has_a_valley_where_its_arms_meet() {
        // Main block ten by four, a wing four by six off its west end, south.
        let mut ell = rectangle(10, 4);
        ell.extend((0..4).flat_map(|x| (4..10).map(move |z| [x, z])));
        let roofed = over(&ell, 6, Style::Hip, None);
        // The inner corner's cube, against the corner where the arms meet.
        assert_eq!(at(&roofed, 4, 4 - 1).1, Shape::Slope(Facing::North));
        let valley: Vec<_> = roofed
            .roof
            .iter()
            .filter(|(_, shape)| matches!(shape, Shape::InnerCorner(_)))
            .collect();
        assert!(!valley.is_empty(), "{:?}", roofed.roof);
        // A gable L has its arms' ridges crossing where they meet, and gables at their ends.
        let gabled = over(&ell, 6, Style::Gable, None);
        assert!(gabled.gables.iter().any(|g| g.z == 9));
        assert!(gabled.gables.iter().any(|g| g.x == 9));
        assert!(
            gabled
                .roof
                .iter()
                .any(|(_, shape)| matches!(shape, Shape::InnerCorner(_)))
        );
    }

    #[test]
    fn the_largest_rectangles_of_an_l_are_its_two_arms_run_through() {
        let mut ell = rectangle(10, 4);
        ell.extend((0..4).flat_map(|x| (4..10).map(move |z| [x, z])));
        let mut found = largest_rectangles(&ell);
        found.sort_by_key(|r| (r.min, r.max));
        assert_eq!(
            found,
            vec![
                Rectangle {
                    min: [0, 0],
                    max: [3, 9]
                },
                Rectangle {
                    min: [0, 0],
                    max: [9, 3]
                },
            ]
        );
    }

    #[test]
    fn a_lean_to_rises_across_its_footprint() {
        let roofed = over(&rectangle(6, 4), 6, Style::LeanTo, Some(true));
        for z in 0..4 {
            assert_eq!(at(&roofed, 2, z), (6 + z, Shape::Slope(Facing::South)));
        }
        // Its high side and both ends carried up to it.
        assert!(roofed.gables.iter().any(|g| g.z == 3 && g.y == 8));
    }
}
