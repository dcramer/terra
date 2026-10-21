//! The heap a fall leaves (D305): what lands and breaks raises the ground under it by its
//! volume, loosened as broken masonry is, and the heap rolls down to its angle of repose, as
//! a sandpile automaton's grains do, so what comes down after lands on what came down before and a
//! tower sinks into its own rubble rather than every block reaching the ground. The heap goes
//! as its pieces fade.

use super::Fragment;
use glam::{IVec3, Vec3};
use std::collections::BTreeMap;
use terra_build::kit::Kit;
use terra_build::{CELL_M, Cube, Kind};

/// How much more room broken masonry takes than it did standing (rubble's bulking, 1.3 to
/// 1.6), less the share of it that lands again whole and is laid again then.
const BULKS: f32 = 1.0;
/// The steepest a heap stands, as a rise a metre: rubble's angle of repose, about 37°.
pub const REPOSE: f32 = 0.75;
/// How long a heap lies after it was last laid on, and how long it takes to go, in seconds:
/// it is drawn (`Rubble::surfaces`), the volume of what fell lying at the foot of what
/// stands as a mound of its stone, as a collapse's rubble does, long after the pieces on it
/// have gone. Lying 9 s, as the pieces on it, a keep's top that fell left a heap a fraction
/// of its volume (castle film round 4).
const LIES_S: f32 = 120.0;
const GOES_S: f32 = 2.0;
/// How high a grain of rubble is, in metres of a cell, and the most cells it rolls over.
const GRAIN_M: f32 = 0.1;
const MOST_ROLLS: usize = 48;

/// A heap's cell: the floor it lies on, how high it stands over it, in metres, and when it
/// was last laid on, in seconds on the heap's clock.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Cell {
    floor: f32,
    high: f32,
    laid_s: f32,
    kind: Kind,
}

/// The rubble of a base's falls, by column of cubes, in the base's own space.
#[derive(Clone, Debug, Default)]
pub struct Rubble {
    cells: BTreeMap<[i32; 2], Cell>,
}

fn column(at: Vec3) -> [i32; 2] {
    [
        (at.x / CELL_M).floor() as i32,
        (at.z / CELL_M).floor() as i32,
    ]
}

/// What came down, by column of cubes: how low, in metres, the lowest of it in each was.
/// What lay on it, a heap or a fragment at rest, comes down with it ([`Rubble::fell`],
/// [`Fragment::wake`]).
pub type Gone = BTreeMap<[i32; 2], f32>;

/// The columns `fell` came down from, each with the bottom of its lowest cube.
pub fn gone(fell: &[(IVec3, Cube)]) -> Gone {
    let mut gone = Gone::new();
    for (at, _) in fell {
        let low = at.y as f32 * CELL_M;
        gone.entry([at.x, at.z])
            .and_modify(|was| *was = was.min(low))
            .or_insert(low);
    }
    gone
}

impl Fragment {
    /// Sets it falling again if it lay on what has now come down: a piece at rest on a
    /// keep's roof falls with the keep rather than hanging where the roof was. Whether it
    /// did.
    pub fn wake(&mut self, gone: &Gone) -> bool {
        if !self.still {
            return false;
        }
        let low = gone.get(&column(self.at));
        if low.is_none_or(|low| self.at.y < *low) {
            return false;
        }
        self.still = false;
        true
    }
}

impl Rubble {
    /// What came down takes the heap lying on it, or over it, with it: a heap laid on a
    /// keep's roof by what fell off it as it burned, left standing when the keep fell, held
    /// the keep's own blocks up at its roof line for seconds (castle film round 2).
    pub fn fell(&mut self, gone: &Gone) {
        self.cells
            .retain(|at, cell| gone.get(at).is_none_or(|low| cell.floor < *low));
    }

    /// How high the heap stands over a point at `now`, if there is one there: the top of it.
    pub fn top(&self, at: Vec3, now: f32) -> Option<f32> {
        let cell = self.cells.get(&column(at))?;
        let gone = ((now - cell.laid_s - LIES_S) / GOES_S).clamp(0.0, 1.0);
        let high = cell.high * (1.0 - gone);
        (high > 0.01).then_some(cell.floor + high)
    }

    /// A whole fragment broken where it landed, at `now`: its volume laid on the cells under
    /// it as grains, each rolling down the heap while it stands steeper than its repose, as a
    /// sandpile's do. `under` says what a point stands on without the heap, the ground or
    /// what is built: a grain rolling off a wall's top into the gap beside it drops to what
    /// is under the gap. Taking the floor it landed on for every column it rolled to, grains
    /// laid on the keep's standing walls spread out over its gap at the walls' height, and
    /// what fell after lay on them, an arch of blocks over nothing (castle film round 3).
    pub fn lay(&mut self, fragment: &Fragment, under: &dyn Fn(Vec3) -> f32, now: f32) {
        self.cells
            .retain(|_, cell| now - cell.laid_s < LIES_S + GOES_S);
        let side = ((fragment.size.x * fragment.size.z).sqrt() / CELL_M)
            .round()
            .max(1.0) as i32;
        let area = (side as f32 * CELL_M).powi(2);
        let rise = fragment.size.x * fragment.size.y * fragment.size.z * BULKS / area;
        let grains = (rise / GRAIN_M).ceil().max(1.0);
        let wide = side as f32 * CELL_M;
        let [x, z] = column(fragment.at - Vec3::new(wide, 0.0, wide) * 0.5 + CELL_M * 0.5);
        for dx in 0..side {
            for dz in 0..side {
                for _ in 0..grains as u32 {
                    self.roll(
                        [x + dx, z + dz],
                        rise / grains,
                        (fragment.at.y, fragment.kind),
                        under,
                        now,
                    );
                }
            }
        }
    }

    /// A grain `high` metres of a cell laid at `at`, from `from` metres up, rolled down to
    /// where it rests. A column with no heap is as high as what is under it at the height of
    /// the heap beside it: a cell's floor is its own, never that of the cell the grain came
    /// from, which may be a wall's top over a drop.
    fn roll(
        &mut self,
        mut at: [i32; 2],
        high: f32,
        (from, kind): (f32, Kind),
        under: &dyn Fn(Vec3) -> f32,
        now: f32,
    ) {
        let steepest = REPOSE * CELL_M;
        let floor_of = |at: [i32; 2], height: f32| {
            under(Vec3::new(
                (at[0] as f32 + 0.5) * CELL_M,
                height,
                (at[1] as f32 + 0.5) * CELL_M,
            ))
        };
        let top = |cells: &BTreeMap<[i32; 2], Cell>, at: [i32; 2], height: f32| {
            cells
                .get(&at)
                .map_or_else(|| floor_of(at, height), |cell| cell.floor + cell.high)
        };
        let mut here = top(&self.cells, at, from);
        for _ in 0..MOST_ROLLS {
            let lowest = [[1, 0], [-1, 0], [0, 1], [0, -1]]
                .map(|[dx, dz]| [at[0] + dx, at[1] + dz])
                .into_iter()
                .map(|next| (next, top(&self.cells, next, here + high)))
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .expect("four neighbours");
            if here + high - lowest.1 <= steepest {
                break;
            }
            at = lowest.0;
            here = lowest.1;
        }
        let floor = self.cells.get(&at).map_or(here, |cell| cell.floor);
        let cell = self.cells.entry(at).or_insert(Cell {
            floor,
            high: 0.0,
            laid_s: now,
            kind,
        });
        cell.high += high;
        cell.laid_s = now;
        cell.kind = kind;
    }

    /// How much is in the heap at `now`, in cubic metres.
    pub fn volume(&self, now: f32) -> f32 {
        self.cells
            .values()
            .map(|cell| cell.high * Self::left(cell, now))
            .sum::<f32>()
            * CELL_M
            * CELL_M
    }

    /// How much of a cell is left at `now`, 0 to 1.
    fn left(cell: &Cell, now: f32) -> f32 {
        1.0 - ((now - cell.laid_s - LIES_S) / GOES_S).clamp(0.0, 1.0)
    }

    /// The heap drawn as a mound of broken stone in the material of what was laid on it last
    /// (`kit_of`), by kind: a height field over its cells, each corner as high as the cells
    /// round it, ragged by a hash and sloping to the floor at its edges, its texture laid
    /// on it from above. Shipped destruction turns settled debris into such a static pile
    /// (Frostbite's and Red Faction's debris settle into rubble meshes) so a collapse's
    /// volume stays on the ground within the debris budget.
    pub fn surfaces<'a>(
        &self,
        now: f32,
        kit_of: &dyn Fn(Kind) -> Option<&'a Kit>,
    ) -> BTreeMap<Kind, crate::mesh::Surfaces> {
        let mut out: BTreeMap<Kind, crate::mesh::Surfaces> = BTreeMap::new();
        let top = |at: [i32; 2]| {
            self.cells.get(&at).and_then(|cell| {
                let high = cell.high * Self::left(cell, now);
                (high > 0.02).then_some((cell.floor, high))
            })
        };
        // A corner: the mean top of the cells round it, the floor where there is none.
        let corner = |x: i32, z: i32, floor: f32| -> Vec3 {
            let round = [[x - 1, z - 1], [x, z - 1], [x - 1, z], [x, z]].map(top);
            let (sum, count) = round
                .iter()
                .fold((0.0, 0), |(sum, count), cell| match cell {
                    Some((floor, high)) => (sum + floor + high, count + 1),
                    None => (sum + floor, count),
                });
            let mean = sum / 4.0;
            let ragged = if count == 4 {
                (terra_gen::hash::unit3(0x4eab, x as u32, z as u32, 0) - 0.5) * 0.12
            } else {
                -0.02
            };
            Vec3::new(x as f32 * CELL_M, mean + ragged, z as f32 * CELL_M)
        };
        for (at, cell) in &self.cells {
            let Some((floor, _)) = top(*at) else {
                continue;
            };
            let Some(kit) = kit_of(cell.kind) else {
                continue;
            };
            let material = super::material(kit);
            let metres = kit
                .textures
                .get(&material)
                .map_or([1.0, 1.0], |texture| texture.metres);
            let shade = kit.tint.map(|c| c * super::FRAGMENT_SHADE);
            let [x, z] = *at;
            let quad = [
                corner(x, z, floor),
                corner(x + 1, z, floor),
                corner(x + 1, z + 1, floor),
                corner(x, z + 1, floor),
            ];
            let surface = out
                .entry(cell.kind)
                .or_insert_with(crate::mesh::Surfaces::sharing)
                .of(material);
            for triangle in [[quad[0], quad[2], quad[1]], [quad[0], quad[3], quad[2]]] {
                let normal = (triangle[1] - triangle[0])
                    .cross(triangle[2] - triangle[0])
                    .normalize_or(Vec3::Y);
                let normal = if normal.y < 0.0 { -normal } else { normal };
                let uvs = triangle.map(|p| [p.x / metres[0], p.z / metres[1]]);
                surface.textured(triangle, [normal; 3], uvs, shade);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{chunks, cube};
    use super::super::{GRAVITY, Shape, burst};
    use super::*;
    use terra_build::Shape as CubeShape;

    #[test]
    fn blocks_landing_in_one_place_heap_up_no_steeper_than_rubble_stands_and_the_heap_goes_later() {
        let column: Vec<(IVec3, terra_build::Cube)> = (0..40)
            .flat_map(|y| (0..4).flat_map(move |x| (0..4).map(move |z| IVec3::new(x, y, z))))
            .map(|at| (at, cube(CubeShape::Whole)))
            .collect();
        let blocks = burst(&column, &|_| false, &chunks, 1.0, 5000);
        let mut rubble = Rubble::default();
        for block in blocks.iter().filter(|b| b.shape == Shape::Whole) {
            rubble.lay(block, &|_| 0.0, 0.0);
        }
        let middle = Vec3::new(1.0, 0.0, 1.0);
        let high = rubble.top(middle, 0.0).expect("a heap where they landed");
        // Higher than one block, lower than what stood.
        assert!(high > 1.5 && high < 20.0 * CELL_M, "{high} m");
        // Spread wider than the column it came from, at no more than its repose.
        let mut widest = 0.0f32;
        for step in 1..40 {
            let out = middle + Vec3::X * step as f32 * CELL_M;
            let (Some(here), Some(next)) = (
                rubble.top(out - Vec3::X * CELL_M, 0.0),
                rubble.top(out, 0.0),
            ) else {
                break;
            };
            widest = step as f32 * CELL_M;
            assert!(
                here - next <= REPOSE * CELL_M + GRAIN_M + 0.01,
                "{} over {CELL_M}",
                here - next
            );
        }
        assert!(widest > 3.0, "the heap is {widest} m out from its middle");
        assert!(
            rubble.top(middle, LIES_S + GOES_S + 1.0).is_none(),
            "the heap never goes"
        );
    }

    #[test]
    fn blocks_landing_on_their_own_heap_raise_it_no_higher_than_on_the_ground() {
        // As the game lays them: each on what it landed on, the heap laid before included.
        let column: Vec<(IVec3, terra_build::Cube)> = (0..40)
            .flat_map(|y| (0..4).flat_map(move |x| (0..4).map(move |z| IVec3::new(x, y, z))))
            .map(|at| (at, cube(CubeShape::Whole)))
            .collect();
        let blocks = burst(&column, &|_| false, &chunks, 1.0, 5000);
        let (mut on_ground, mut on_heap) = (Rubble::default(), Rubble::default());
        for block in blocks.iter().filter(|b| b.shape == Shape::Whole) {
            on_ground.lay(block, &|_| 0.0, 0.0);
            on_heap.lay(block, &|_| 0.0, 0.0);
        }
        let middle = Vec3::new(1.0, 0.0, 1.0);
        let reference = on_ground.top(middle, 0.0).expect("a heap");
        let high = on_heap.top(middle, 0.0).expect("a heap");
        assert!(
            (high - reference).abs() < 0.1 * reference,
            "{high} m on its own heap, {reference} m on the ground"
        );
    }

    #[test]
    fn what_lay_on_a_tower_comes_down_when_the_tower_does() {
        // A tower 4 cubes a side, 30 high: something broke on its roof and lies there, a
        // heap under it, and beside the tower another heap on the ground.
        let tower: Vec<(IVec3, terra_build::Cube)> = (0..30)
            .flat_map(|y| (0..4).flat_map(move |x| (0..4).map(move |z| IVec3::new(x, y, z))))
            .map(|at| (at, cube(CubeShape::Whole)))
            .collect();
        let roof = 30.0 * CELL_M;
        let mut piece = burst(&tower[..1], &|_| true, &chunks, 1.0, 1)[0];
        piece.at = Vec3::new(1.0, roof + 0.1, 1.0);
        piece.still = true;
        let mut beside = piece;
        beside.at = Vec3::new(4.0, 0.1, 1.0);
        let mut rubble = Rubble::default();
        let on_tower = |at: Vec3| {
            let side = 0.0..4.0 * CELL_M;
            let inside = side.contains(&at.x) && side.contains(&at.z);
            if inside && at.y >= roof { roof } else { 0.0 }
        };
        rubble.lay(&piece, &on_tower, 0.0);
        rubble.lay(&beside, &|_| 0.0, 0.0);
        assert!(rubble.top(piece.at, 0.0).is_some_and(|top| top > roof));
        // The tower comes down.
        let gone = gone(&tower);
        rubble.fell(&gone);
        assert!(
            piece.wake(&gone),
            "the piece on the roof hangs where the roof was"
        );
        assert!(!beside.wake(&gone), "the piece beside it stirred");
        let heap = rubble.top(piece.at, 0.0);
        assert!(
            heap.is_none_or(|top| top < 1.0),
            "a heap stands at {heap:?} m over nothing"
        );
        assert!(
            rubble.top(beside.at, 0.0).is_some(),
            "the heap beside it went too"
        );
        // Woken, it falls to the ground in the time a drop from the roof takes, not later.
        let mut fell_s = None;
        for step in 0..240 {
            let floor = |at: Vec3| rubble.top(at, 0.0).unwrap_or(0.0);
            piece.step(1.0 / 60.0, &floor, &|_| false);
            if piece.at.y < 1.0 {
                fell_s = Some(step as f32 / 60.0);
                break;
            }
        }
        let drop_s = libm::sqrtf(2.0 * roof / GRAVITY);
        assert!(
            fell_s.is_some_and(|s| s < drop_s * 1.3),
            "it came down in {fell_s:?} s; a drop from the roof takes {drop_s:.1} s"
        );
    }

    #[test]
    fn a_keeps_top_that_fell_lies_in_its_heap_at_least_most_of_its_volume_and_is_drawn() {
        // The castle film's keep top, 20 by 8 cubes and 4 courses, falls 6.5 m and breaks
        // where it lands, its pieces as few as a full budget allows (the game's least
        // share): what lies in the heap after, drawn, is most of what came down, however
        // few of the pieces on it are left (castle film round 4: the keep's heap was a
        // fraction of its top).
        let fell: Vec<(IVec3, terra_build::Cube)> = (26..30)
            .flat_map(|y| (0..20).flat_map(move |x| (0..8).map(move |z| IVec3::new(x, y, z))))
            .map(|at| (at, cube(CubeShape::Whole)))
            .collect();
        let removed = fell.len() as f32 * CELL_M.powi(3);
        let mut parts = burst(&fell, &|_| false, &chunks, 1.0, 5000);
        let mut rubble = Rubble::default();
        let dt = 1.0 / 60.0;
        for step in 0..360 {
            let now = step as f32 * dt;
            let mut born = Vec::new();
            for part in &mut parts {
                let floor = |at: Vec3| rubble.top(at, now).unwrap_or(0.0);
                if part.step(dt, &floor, &|_| false).breaks {
                    rubble.lay(part, &|_| 0.0, now);
                    born.extend(part.shatter(chunks(part.kind), 0.3));
                    part.size = Vec3::ZERO;
                }
            }
            parts.retain(|part| part.size != Vec3::ZERO);
            parts.extend(born);
        }
        // Half a minute on, as the film's last shots see it.
        let heap = rubble.volume(30.0);
        assert!(
            heap >= 0.6 * removed,
            "the heap holds {heap:.1} m3 of the {removed:.1} m3 that came down"
        );
        // Drawn as a mound over its cells, no higher than what fell stood.
        let kit = terra_build::kit::Kit {
            pattern: terra_build::kit::Pattern::Masonry,
            ..Default::default()
        };
        let drawn = rubble.surfaces(30.0, &|_| Some(&kit));
        let points: Vec<Vec3> = drawn
            .values()
            .flat_map(|surfaces| surfaces.0.values())
            .flat_map(|surface| surface.positions.iter().copied())
            .collect();
        assert!(points.len() > 6 * 160, "{} corners drawn", points.len());
        let highest = points.iter().map(|p| p.y).fold(0.0f32, f32::max);
        assert!(
            highest > 0.5 && highest < 30.0 * CELL_M,
            "the mound stands {highest:.1} m"
        );
    }

    #[test]
    fn a_keep_whose_middle_fell_between_standing_walls_hangs_no_arch_over_its_gap() {
        // The castle film's keep: two walls 4 cubes thick and 26 high stand either side of a
        // gap 12 cubes wide, and its top, 4 courses over the whole of it, comes down onto
        // them and into the gap.
        let (wall, top) = (26, 30);
        let standing = |x: i32| (0..4).contains(&x) || (16..20).contains(&x);
        let fell: Vec<(IVec3, terra_build::Cube)> = (wall..top)
            .flat_map(|y| (0..20).flat_map(move |x| (0..8).map(move |z| IVec3::new(x, y, z))))
            .map(|at| (at, cube(CubeShape::Whole)))
            .collect();
        let wall_m = wall as f32 * CELL_M;
        let across = 0.0..8.0 * CELL_M;
        let built = |at: Vec3| {
            let x = (at.x / CELL_M).floor() as i32;
            let on = standing(x) && across.contains(&at.z) && at.y >= wall_m;
            if on { wall_m } else { 0.0 }
        };
        let solid = |at: Vec3| {
            let x = (at.x / CELL_M).floor() as i32;
            standing(x) && across.contains(&at.z) && (0.0..wall_m).contains(&at.y)
        };
        let mut parts = burst(&fell, &|_| false, &chunks, 1.0, 5000);
        let mut rubble = Rubble::default();
        let dt = 1.0 / 60.0;
        for step in 0..240 {
            let now = step as f32 * dt;
            let mut born = Vec::new();
            for part in &mut parts {
                let floor = |at: Vec3| rubble.top(at, now).unwrap_or(0.0).max(built(at));
                if part.step(dt, &floor, &solid).breaks {
                    rubble.lay(part, &built, now);
                    born.extend(part.shatter(chunks(part.kind), 0.3));
                    part.size = Vec3::ZERO;
                }
            }
            parts.retain(|part| part.size != Vec3::ZERO);
            parts.extend(born);
        }
        // Four seconds on, over the gap's middle, nothing is up near the walls' tops: what
        // fell into it lies on its heap on the ground.
        let gap = 6.0 * CELL_M..14.0 * CELL_M;
        let highest = parts
            .iter()
            .filter(|part| gap.contains(&part.at.x))
            .map(|part| part.at.y)
            .fold(0.0f32, f32::max);
        let heap = (12..28)
            .map(|x| {
                let over = Vec3::new(x as f32 * 0.25, wall_m, 2.0);
                rubble.top(over, 4.0).unwrap_or(0.0)
            })
            .fold(0.0f32, f32::max);
        assert!(
            highest < 0.5 * wall_m && heap < 0.5 * wall_m,
            "over the gap a piece at {highest:.1} m, the heap at {heap:.1} m; the walls stand {wall_m} m"
        );
    }
}
