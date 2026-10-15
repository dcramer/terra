//! What covers open cold ground, by the land's rules (docs/demos.md §2, D321): past the trees,
//! on the tundra and above a mountain's tree line alike, the ground is a mosaic of plant
//! communities chosen by two things the land works out, how exposed a place is to the wind
//! and how wet it is, as real tundra is (Walker et al., the Circumpolar Arctic Vegetation Map;
//! the EUNIS shrub tundra habitats):
//!
//! | Community | Where | What |
//! |---|---|---|
//! | Fellfield | windward, exposed rises | stones, cushion plants |
//! | Lichen heath | dry ground | pale *Cladonia* lichen |
//! | Dwarf-shrub heath | moderate, sheltered ground, under winter snow | dwarf birch, crowberry, bilberry |
//! | Tussock sedge | wet ground | sedge tussocks, cotton grass |
//! | Moss carpet | pool edges, soaked hollows | moss, *Sphagnum* |
//!
//! Colder ground loses its shrubs first and then its sedges to lichen and stones, as the
//! low arctic gives way to the high. A place's share of each is worked out on the land's own
//! grid round a place ([`HeathMap`]), and drawn by the ground and the cover as their layers and
//! plants; a new cold realm is new species in the same communities, not new code.

use crate::region::Region;
use glam::{Vec2, Vec3};

/// How much of each community covers a place, as shares of the ground, and how much of the
/// ground is the open heath's at all (the rest the forest's floor or the meadow's grass).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Heath {
    pub fellfield: f32,
    pub lichen: f32,
    pub shrub: f32,
    pub sedge: f32,
    pub moss: f32,
    /// How much of the ground the heath holds: none in a closed forest or a warm meadow, all
    /// past the trees.
    pub open: f32,
    /// How wet the ground is, 0 dry to 1 soaked.
    pub wet: f32,
}

/// What a place offers the heath.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ground {
    /// How much colder than the tree line's its growing season is, in degrees Celsius:
    /// below 0 trees grow, above 3 the polar desert begins.
    pub colder_c: f32,
    /// How close the trees there stand, 0 none to 1 a closed forest (`woods::Stand`).
    pub trees: f32,
    /// Winstral's Sx against the prevailing wind, in degrees: above 0 in a lee, below exposed.
    pub shelter_deg: f32,
    /// How wet it is, 0 dry to 1 soaked ([`wetness`]).
    pub wet: f32,
    /// How steep, in degrees.
    pub slope_deg: f32,
}

fn smoothstep(from: f32, to: f32, x: f32) -> f32 {
    let t = ((x - from) / (to - from)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The heath a place holds.
pub fn heath(ground: Ground) -> Heath {
    // The heath takes the ground as the season cools toward and past the tree line: a
    // meadow's grass in the forest's glades and warm valleys, the heath above and beyond,
    // under the forest-tundra's islands of trees too; a closed forest's own floor in the warm.
    let warm_forest = ground.trees * (1.0 - smoothstep(-1.5, 0.0, ground.colder_c));
    let open = smoothstep(-1.5, 0.6, ground.colder_c) * (1.0 - warm_forest);
    if open <= 0.0 {
        return Heath {
            wet: ground.wet,
            ..Heath::default()
        };
    }
    let exposed = smoothstep(-1.0, -9.0, ground.shelter_deg);
    let lee = smoothstep(1.0, 9.0, ground.shelter_deg);
    let w = ground.wet;
    // Colder ground loses its shrubs first, then its sedges.
    let high_arctic = smoothstep(1.5, 3.5, ground.colder_c);
    let scoured = smoothstep(-6.0, -12.0, ground.shelter_deg);
    let fellfield = ((0.5 * exposed + 0.6 * scoured + 0.5 * high_arctic).min(1.0)
        * (1.0 - smoothstep(0.35, 0.65, w)))
    .max(smoothstep(22.0, 36.0, ground.slope_deg));
    let moss = smoothstep(0.72, 0.95, w);
    let sedge = smoothstep(0.42, 0.68, w) * (1.0 - moss) * (1.0 - 0.6 * high_arctic);
    let damp = 1.0 - smoothstep(0.5, 0.75, w);
    // Dwarf shrubs want winter's snow over them: the lee most, level ground some.
    let shrub =
        damp * smoothstep(-0.2, 0.7, lee - 0.6 * exposed + 0.1) * (1.0 - high_arctic) * 0.75;
    let lichen = (1.0 - smoothstep(0.35, 0.6, w))
        * (0.25 + 0.75 * smoothstep(-0.5, -4.0, ground.shelter_deg).max(high_arctic))
        * (1.0 - 0.5 * scoured);
    let weights = [fellfield, lichen, shrub, sedge, moss];
    let sum: f32 = weights.iter().sum::<f32>().max(1e-4);
    let [fellfield, lichen, shrub, sedge, moss] = weights.map(|w| w / sum * open);
    Heath {
        fellfield,
        lichen,
        shrub,
        sedge,
        moss,
        open,
        wet: w,
    }
}

/// How wet a region's ground is at a cell, 0 dry to 1 soaked: a hollow below the ground round
/// it holds water, level ground drains slowly, and the ground by a lake or pool is soaked; over
/// permafrost (`frozen` 1) the frozen ground under it holds the summer's water up, as tundra's
/// does.
pub fn wetness(region: &Region, column: usize, row: usize, above_water_m: f32, frozen: f32) -> f32 {
    let n = region.cells as i64;
    let here = region.height(column, row);
    let reach = ((30.0 / region.cell_m).round() as i64).max(1);
    let (mut sum, mut count) = (0.0, 0.0);
    for dr in -reach..=reach {
        for dc in -reach..=reach {
            let (c, r) = (column as i64 + dc, row as i64 + dr);
            if c < 0 || r < 0 || c >= n || r >= n || (dc == 0 && dr == 0) {
                continue;
            }
            sum += region.height(c as usize, r as usize);
            count += 1.0;
        }
    }
    let hollow = if count > 0.0 { sum / count - here } else { 0.0 };
    let slope = region.slope_deg(column, row);
    let by_water = 1.0 - smoothstep(0.2, 2.5, above_water_m);
    let pooled = smoothstep(-0.2, 1.2, hollow);
    let level = 1.0 - smoothstep(1.0, 8.0, slope);
    (0.55 * pooled + 0.2 * level + 0.25 * frozen)
        .max(by_water)
        .clamp(0.0, 1.0)
}

/// How well the ground drains, from place to place, as its soil and the stones under it vary:
/// patches some tens of metres across a little drier or wetter than the lie of the land alone
/// makes them, so a level plateau is a mosaic of lichen on its drier knolls, shrubs and sedge
/// in its damper ground, as a fell is, not one community.
#[derive(Clone, Debug)]
pub struct Drainage(terra_gen::Noise);

impl Drainage {
    pub fn new(seed: terra_gen::Seed) -> Self {
        Self(terra_gen::Noise::new(seed.derive("heath drainage")))
    }

    /// How much wetter (above 0) or drier a point in the world is than its lie makes it.
    pub fn at(&self, point: Vec3) -> f32 {
        // The noise's spread is about 0.27 a sample; a quarter of the wetness's range either way.
        (self.0.fbm(point / 60.0, 3, 2.1, 0.5) / 0.27 * 0.14).clamp(-0.3, 0.3)
    }
}

/// How many times the heath's shares are blended with their neighbours', three cells a side.
const SMOOTHING: usize = 2;

/// The heath over a region's cells, worked out once for a place's land.
#[derive(Clone, Debug)]
pub struct HeathMap {
    region: Region,
    cells: Vec<Heath>,
}

impl HeathMap {
    /// The heath over a region, each cell's from `at(column, row)`.
    pub fn over(region: &Region, at: impl Fn(usize, usize) -> Heath + Sync) -> Self {
        let n = region.cells;
        let all: Vec<usize> = (0..n * n).collect();
        let mut cells = crate::maps::par_map(&all, |&k| at(k % n, k / n));
        // The rules' shares blended over a few cells, so a community's ground is a patch tens
        // of metres across, not one cell's square: the finer mosaic inside it is the drawing's,
        // by noise.
        for _ in 0..SMOOTHING {
            cells = crate::maps::par_map(&all, |&k| {
                let (c, r) = ((k % n) as i64, (k / n) as i64);
                let mut sum = Heath::default();
                let mut count = 0.0;
                for dr in -1..=1 {
                    for dc in -1..=1 {
                        let (cc, rr) = (c + dc, r + dr);
                        if cc < 0 || rr < 0 || cc >= n as i64 || rr >= n as i64 {
                            continue;
                        }
                        let h = cells[rr as usize * n + cc as usize];
                        sum.fellfield += h.fellfield;
                        sum.lichen += h.lichen;
                        sum.shrub += h.shrub;
                        sum.sedge += h.sedge;
                        sum.moss += h.moss;
                        sum.open += h.open;
                        sum.wet += h.wet;
                        count += 1.0;
                    }
                }
                Heath {
                    fellfield: sum.fellfield / count,
                    lichen: sum.lichen / count,
                    shrub: sum.shrub / count,
                    sedge: sum.sedge / count,
                    moss: sum.moss / count,
                    open: sum.open / count,
                    wet: sum.wet / count,
                }
            });
        }
        Self {
            region: Region {
                heights: Vec::new(),
                ..region.clone()
            },
            cells,
        }
    }

    pub fn region(&self) -> &Region {
        &self.region
    }

    /// A cell's heath.
    pub fn at_cell(&self, column: usize, row: usize) -> Heath {
        self.cells[row * self.region.cells + column]
    }

    /// The heath in a direction from the centre, blended between the cells round it; none
    /// outside the region.
    pub fn at(&self, direction: Vec3) -> Heath {
        let region = &self.region;
        let d = direction.normalize();
        let along = d.dot(region.middle.normalize());
        if along <= 0.0 {
            return Heath::default();
        }
        let p = d * (region.middle.length() / along) - region.middle;
        let at = Vec2::new(p.dot(region.east), p.dot(region.north));
        let n = region.cells as f32;
        let fc = at.x / region.cell_m + n * 0.5 - 0.5;
        let fr = n * 0.5 - 0.5 - at.y / region.cell_m;
        if fc < 0.0 || fr < 0.0 || fc > n - 1.0 || fr > n - 1.0 {
            return Heath::default();
        }
        let (c, r) = (fc as usize, fr as usize);
        let last = region.cells - 1;
        let (c2, r2) = ((c + 1).min(last), (r + 1).min(last));
        let (tc, tr) = (fc - c as f32, fr - r as f32);
        let mix = |a: Heath, b: Heath, t: f32| Heath {
            fellfield: a.fellfield + (b.fellfield - a.fellfield) * t,
            lichen: a.lichen + (b.lichen - a.lichen) * t,
            shrub: a.shrub + (b.shrub - a.shrub) * t,
            sedge: a.sedge + (b.sedge - a.sedge) * t,
            moss: a.moss + (b.moss - a.moss) * t,
            open: a.open + (b.open - a.open) * t,
            wet: a.wet + (b.wet - a.wet) * t,
        };
        let top = mix(self.at_cell(c, r), self.at_cell(c2, r), tc);
        let bottom = mix(self.at_cell(c, r2), self.at_cell(c2, r2), tc);
        mix(top, bottom, tr)
    }

    /// Its cells as two layers of a picture, a byte a share, row by row from the north: the
    /// lichen, dwarf shrubs, sedge and moss in the first; the fellfield's stones, the
    /// wetness and how open the ground is in the second. What the ground shader reads.
    pub fn bytes(&self) -> Vec<u8> {
        let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
        let mut out = Vec::with_capacity(self.cells.len() * 8);
        for h in &self.cells {
            out.extend([byte(h.lichen), byte(h.shrub), byte(h.sedge), byte(h.moss)]);
        }
        for h in &self.cells {
            out.extend([byte(h.fellfield), byte(h.wet), byte(h.open), 255]);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ground(shelter_deg: f32, wet: f32) -> Ground {
        Ground {
            colder_c: 1.5,
            trees: 0.0,
            shelter_deg,
            wet,
            slope_deg: 3.0,
        }
    }

    fn most(h: Heath) -> &'static str {
        let all = [
            ("fellfield", h.fellfield),
            ("lichen", h.lichen),
            ("shrub", h.shrub),
            ("sedge", h.sedge),
            ("moss", h.moss),
        ];
        all.iter().max_by(|a, b| a.1.total_cmp(&b.1)).unwrap().0
    }

    #[test]
    fn each_community_holds_its_own_ground() {
        assert_eq!(most(heath(ground(-12.0, 0.1))), "fellfield");
        assert_eq!(most(heath(ground(-3.0, 0.2))), "lichen");
        assert_eq!(most(heath(ground(8.0, 0.3))), "shrub");
        assert_eq!(most(heath(ground(0.0, 0.6))), "sedge");
        assert_eq!(most(heath(ground(0.0, 0.97))), "moss");
    }

    #[test]
    fn the_heath_shares_the_ground_and_gives_way_to_the_forest() {
        for shelter in [-15.0, -4.0, 0.0, 5.0, 15.0] {
            for wet in [0.0, 0.3, 0.5, 0.7, 1.0] {
                let h = heath(ground(shelter, wet));
                let sum = h.fellfield + h.lichen + h.shrub + h.sedge + h.moss;
                assert!((sum - h.open).abs() < 1e-4, "{h:?}");
            }
        }
        let forest = heath(Ground {
            colder_c: -2.5,
            trees: 1.0,
            ..ground(0.0, 0.3)
        });
        assert_eq!(forest.open, 0.0);
        // Colder ground loses its shrubs.
        let low = heath(ground(8.0, 0.3));
        let high = heath(Ground {
            colder_c: 3.5,
            ..ground(8.0, 0.3)
        });
        assert!(high.shrub < low.shrub * 0.3, "{low:?} {high:?}");
    }

    #[test]
    fn a_hollow_by_a_pool_is_wetter_than_a_rise() {
        let mut region = Region {
            middle: Vec3::Y * 30_000.0,
            east: Vec3::X,
            north: Vec3::NEG_Z,
            cells: 41,
            cell_m: 10.0,
            heights: vec![0.0; 41 * 41],
        };
        for row in 0..41 {
            for column in 0..41 {
                let (x, y) = (column as f32 - 20.0, row as f32 - 20.0);
                // A dome on the west, a bowl on the east.
                let dome = 4.0 - 0.2 * ((x + 10.0) * (x + 10.0) + y * y);
                let bowl = -4.0 + 0.2 * ((x - 10.0) * (x - 10.0) + y * y);
                region.heights[row * 41 + column] = if x < 0.0 {
                    dome.max(0.0)
                } else {
                    bowl.min(0.0)
                };
            }
        }
        let rise = wetness(&region, 10, 20, 50.0, 0.0);
        let hollow = wetness(&region, 30, 20, 50.0, 0.0);
        let shore = wetness(&region, 30, 20, 0.3, 0.0);
        assert!(hollow > rise + 0.3, "{rise} {hollow}");
        assert!(shore > 0.9, "{shore}");
        assert!(wetness(&region, 10, 20, 50.0, 1.0) > rise);
    }
}
