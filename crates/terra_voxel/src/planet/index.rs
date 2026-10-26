//! The edits by place: which of a planet's edits can reach a point, so a sample lays over the
//! generated ground only those, not every edit made. The edits are still kept whole and in
//! order (D21, the saved truth); this only finds them.
//!
//! A uniform grid hashed by cell (Ericson, Real-Time Collision Detection, 7.1 "Uniform grids"
//! and 7.1.6 "Hashed storage"), kept as Godot 3's `BroadPhase2DHashGrid` (MIT) keeps its
//! objects: each in every cell its bounds overlap, those too big for the grid in a list of
//! their own looked at every time. Edits are laid in the order made, as Voxel Tools'
//! `VoxelModifierStack` (MIT) lays its modifiers in stack order, culled by bounds: each is a
//! stamp, numbered as laid, and a cell keeps its stamps in that order (a list of links, so the
//! grid copies without an allocation a cell). Adding an edit touches its cells alone; a bridge
//! taken away is crossed out where it is kept; nothing is built again (D127).
//!
//! The answer is the same, bit for bit, as laying every edit in order (test
//! `an_indexed_sample_is_every_edit_laid_in_order`): a dig reaches only so far, and the cell a
//! point is in holds every edit whose reach takes it in; a fill counts everywhere, but only
//! lowers the ground, so one whose sphere is no nearer a point than the most the ground there
//! can be (what the field says raised by the digs that reach it) changes nothing, and only the
//! fills nearer than that are looked for, in the cells within that much more than their reach.

use super::{EDIT_MARGIN_M, Edit};
use crate::field::Sample;
use glam::Vec3;
use terra_gen::hash::QuickMap;

/// A cell's side, m: an edit's reach (a metre or so past its 2 m margin) spans a few.
const CELL_M: f32 = 4.0;
/// An edit's bounds are taken this much wider than its reach, so a point the reach's own test
/// (in f32) takes in is always in a cell it is kept in, m.
const SLACK_M: f32 = 0.25;
/// The most cells an edit is kept in, or a point's far fills looked for in, before the edit
/// goes in the list looked at every time, or every fill is looked at.
const MOST_CELLS: i64 = 512;
/// The end of a cell's list.
const END: u32 = u32::MAX;

type Key = (i32, i32, i32);

/// The cell a point is in.
fn cell(point: Vec3) -> Key {
    let at = (point / CELL_M).floor();
    (at.x as i32, at.y as i32, at.z as i32)
}

/// A cell's place in the map: its coordinates' low 21 bits each, mixed (SplitMix64's
/// finaliser) so the map's buckets spread. Two cells 8,000 km apart share one, which costs
/// only edits passed over by their reach.
fn slot(key: Key) -> u64 {
    let bits = |n: i32| u64::from(n as u32) & 0x1F_FFFF;
    terra_gen::hash::splitmix64(bits(key.0) | bits(key.1) << 21 | bits(key.2) << 42)
}

/// The cells a box overlaps, low and high corners, and how many.
fn cells(low: Vec3, high: Vec3) -> (Key, Key, i64) {
    let (a, b) = (cell(low), cell(high));
    let count = (i64::from(b.0) - i64::from(a.0) + 1)
        * (i64::from(b.1) - i64::from(a.1) + 1)
        * (i64::from(b.2) - i64::from(a.2) + 1);
    (a, b, count)
}

/// Every cell from one corner to the other.
fn each(a: Key, b: Key) -> impl Iterator<Item = Key> {
    (a.0..=b.0)
        .flat_map(move |x| (a.1..=b.1).flat_map(move |y| (a.2..=b.2).map(move |z| (x, y, z))))
}

/// What the ground is at a point, as a distance: how a fill and a dig compare with it.
pub(super) trait Level: Copy {
    fn level(&self) -> f32;
}

impl Level for f32 {
    fn level(&self) -> f32 {
        *self
    }
}

impl Level for Sample {
    fn level(&self) -> f32 {
        self.distance
    }
}

/// A stamp in a cell's list, where its edit is and its size (so an edit out of reach is
/// passed over without looking it up), and the next.
#[derive(Clone, Copy)]
struct Link {
    stamp: u32,
    next: u32,
    centre: Vec3,
    radius: f32,
}

impl Link {
    fn of(stamp: u32, edit: &Edit) -> Link {
        let (centre, radius) = edit.centre_and_radius();
        Link {
            stamp,
            next: END,
            centre,
            radius,
        }
    }

    /// [`Edit::reaches`], from what the link keeps.
    fn reaches(&self, point: Vec3) -> bool {
        let reach = self.radius + EDIT_MARGIN_M;
        point.distance_squared(self.centre) <= reach * reach
    }
}

/// How much nearer than the margin a ground must be for the fills out of reach to be left
/// out without looking for them, m: more than an f32 distance's error a few km out.
const FAR_SLACK_M: f32 = 0.05;

/// The edits laid over the ground, by place.
#[derive(Clone, Default)]
pub(super) struct Index {
    /// Each stamp's edit, in the order laid; none once taken away.
    stamps: Vec<Option<Edit>>,
    /// Each cell's first and last link.
    cells: QuickMap<u64, (u32, u32)>,
    links: Vec<Link>,
    /// Stamps over more than [`MOST_CELLS`], in order.
    large: Vec<u32>,
    /// Every fill's stamp, in order.
    fills: Vec<u32>,
}

impl Index {
    /// Lays an edit over those laid before it, and returns its stamp.
    pub fn add(&mut self, edit: Edit) -> u32 {
        let stamp = self.stamps.len() as u32;
        self.stamps.push(Some(edit));
        if edit.is_fill() {
            self.fills.push(stamp);
        }
        let (centre, radius) = edit.centre_and_radius();
        let bound = Vec3::splat(radius + EDIT_MARGIN_M + SLACK_M);
        let (a, b, count) = cells(centre - bound, centre + bound);
        if count > MOST_CELLS {
            self.large.push(stamp);
            return stamp;
        }
        for key in each(a, b) {
            let link = self.links.len() as u32;
            self.links.push(Link::of(stamp, &edit));
            let ends = self.cells.entry(slot(key)).or_insert((link, link));
            if ends.1 != link {
                self.links[ends.1 as usize].next = link;
                ends.1 = link;
            }
        }
        stamp
    }

    /// Takes a stamp's edit away: it is no longer laid anywhere.
    pub fn remove(&mut self, stamp: u32) {
        self.stamps[stamp as usize] = None;
        self.large.retain(|&s| s != stamp);
    }

    /// The edit a stamp is, if it is still laid.
    fn edit(&self, stamp: u32) -> Option<&Edit> {
        self.stamps[stamp as usize].as_ref()
    }

    /// A cell's links, in order.
    fn listed(&self, key: Key) -> impl Iterator<Item = Link> + '_ {
        let mut at = self.cells.get(&slot(key)).map_or(END, |ends| ends.0);
        std::iter::from_fn(move || {
            let link = *self.links.get(at as usize)?;
            at = link.next;
            Some(link)
        })
    }

    /// The large edits' links, in order.
    fn large(&self) -> impl Iterator<Item = Link> + '_ {
        (self.large.iter()).filter_map(|&stamp| Some(Link::of(stamp, self.edit(stamp)?)))
    }

    /// The edits that reach a point, in order: its cell's and the large ones, still laid.
    fn reaching(&self, point: Vec3, home: Key) -> impl Iterator<Item = (Link, &Edit)> + '_ {
        let mut cell = self
            .listed(home)
            .filter(move |l| l.reaches(point))
            .peekable();
        let mut large = self.large().filter(move |l| l.reaches(point)).peekable();
        std::iter::from_fn(move || {
            loop {
                let link = match (cell.peek(), large.peek()) {
                    (Some(c), Some(l)) if l.stamp < c.stamp => large.next(),
                    (Some(_), _) => cell.next(),
                    (None, _) => large.next(),
                }?;
                if let Some(edit) = self.edit(link.stamp) {
                    return Some((link, edit));
                }
            }
        })
    }

    /// The fills out of reach of a point but nearer it than `most`: those that could lower
    /// a ground there that is at most that far. Looked for in the cells within that much
    /// more than their reach, or among every fill when that is fewer.
    fn far_fills(&self, point: Vec3, most: f32) -> Vec<u32> {
        let nearer =
            |link: &Link| !link.reaches(point) && point.distance(link.centre) - link.radius < most;
        let fill = |link: &Link| self.edit(link.stamp).is_some_and(Edit::is_fill);
        let wide = (most - EDIT_MARGIN_M).max(0.0) + FAR_SLACK_M;
        let (a, b, count) = cells(point - wide, point + wide);
        let mut found: Vec<u32> = if !most.is_finite() || count > self.fills.len() as i64 {
            (self.fills.iter())
                .filter_map(|&stamp| Some(Link::of(stamp, self.edit(stamp)?)))
                .filter(nearer)
                .map(|link| link.stamp)
                .collect()
        } else {
            let large = self.large().filter(|l| nearer(l) && fill(l));
            each(a, b)
                .flat_map(|key| self.listed(key).filter(|l| nearer(l) && fill(l)))
                .chain(large)
                .map(|link| link.stamp)
                .collect()
        };
        found.sort_unstable();
        found.dedup();
        found
    }

    /// The ground at a point with the edits laid over it in order, each that can change it
    /// stepped by `step`: the same as stepping every edit that reaches it and every fill.
    ///
    /// The edits that reach the point are stepped in order. A fill out of reach changes the
    /// ground only where it is nearer than the ground, and the ground is at most what the
    /// field says raised by the digs that reach the point, each by at most its size. When
    /// that is within the margin no fill out of reach can matter; otherwise the fills out of
    /// reach nearer than that are looked for, and if there are any, laid in order with the
    /// rest.
    pub fn shaped<T: Level>(&self, point: Vec3, ground: T, step: impl Fn(&Edit, T) -> T) -> T {
        if self.stamps.is_empty() {
            return ground;
        }
        let home = cell(point);
        // A ground of no number is lowered by every fill.
        let mut most = match ground.level() {
            level if level.is_nan() => f32::INFINITY,
            level => level,
        };
        let mut shaped = ground;
        for (link, edit) in self.reaching(point, home) {
            if !edit.is_fill() {
                most = most.max(link.radius);
            }
            shaped = step(edit, shaped);
        }
        if self.fills.is_empty() || most <= EDIT_MARGIN_M - FAR_SLACK_M {
            return shaped;
        }
        let far = self.far_fills(point, most);
        if far.is_empty() {
            return shaped;
        }
        let mut stamps: Vec<u32> = self.reaching(point, home).map(|(l, _)| l.stamp).collect();
        stamps.extend(far);
        stamps.sort_unstable();
        let mut ground = ground;
        for stamp in stamps {
            if let Some(edit) = self.edit(stamp) {
                ground = step(edit, ground);
            }
        }
        ground
    }
}

#[cfg(test)]
mod tests {
    use super::super::VoxelPlanet;
    use crate::field::{Field, material};
    use crate::{GeneratedPlanet, HeightMemo};
    use glam::{DVec3, Quat, Vec3};
    use terra_core::{MaterialId, Operation, PlanetSpec};
    use terra_gen::Rng;
    use terra_gen::Seed;

    /// Digs, fills and boxes over one another near a spot, in an order that matters (a fill
    /// dug through, a dig filled in, boxes side by side that bridge and bridges taken away),
    /// with a few large ones and fills far off.
    fn edit(rng: &mut Rng, spot: Vec3, up: Vec3, spread: f32) -> Operation {
        let near = spot + rng.unit_vector() * rng.range(0.0, spread);
        let size = rng.range(0.3, 1.5);
        match rng.below(20) {
            // Too large for the grid (a large dig is laid the same way, and measured slowly).
            0 => Operation::Fill {
                centre: near,
                radius: rng.range(15.0, 40.0),
                material: material::STONE,
            },
            1 => Operation::Fill {
                centre: spot + rng.unit_vector() * rng.range(50.0, 1500.0),
                radius: rng.range(0.5, 30.0),
                material: MaterialId(2),
            },
            2..=6 => Operation::Fill {
                centre: near,
                radius: size,
                material: material::SAND,
            },
            7..=12 => Operation::Dig {
                centre: near,
                radius: size,
            },
            _ => {
                // Boxes on a lattice square to the ground, so many share faces.
                let rotation = Quat::from_rotation_arc(Vec3::Y, up);
                let step = Vec3::new(
                    rng.below(8) as f32 - 4.0,
                    rng.below(4) as f32 - 3.0,
                    rng.below(8) as f32 - 4.0,
                );
                Operation::DigBox {
                    centre: spot + rotation * step,
                    half: Vec3::splat(0.5) + Vec3::X * rng.below(2) as f32 * 0.5,
                    rotation,
                }
            }
        }
    }

    /// A point to ask about: near the edits, high in the air over them, or deep under them.
    fn point(rng: &mut Rng, spot: Vec3, up: Vec3, spread: f32) -> Vec3 {
        match rng.below(10) {
            0 => spot + up * rng.range(5.0, 400.0) + rng.unit_vector() * 20.0,
            1 => spot - up * rng.range(5.0, 60.0) + rng.unit_vector() * 20.0,
            _ => spot + rng.unit_vector() * rng.range(0.0, spread + 8.0),
        }
    }

    /// Edits made `spread` metres round a spot, and points asked about there compared.
    fn compare(spread: f32) {
        let mut planet = VoxelPlanet::new(GeneratedPlanet::new(
            PlanetSpec {
                seed: Seed(7),
                radius_m: 2000.0,
            },
            150.0,
            Some(-20.0),
        ));
        let up = Vec3::Y;
        let spot = up * (2000.0 + planet.field().height_m(up));
        let mut rng = Rng::new(Seed(11));
        for _ in 0..4 {
            for _ in 0..150 {
                planet.apply(&edit(&mut rng, spot, up, spread));
            }
            for _ in 0..1500 {
                let at = point(&mut rng, spot, up, spread);
                let memo = &mut HeightMemo::default();
                let ground = planet.field().distance_in(at, memo);
                let by_index = planet.shaped(at, ground, |e, before| e.distance(at, before));
                let by_all = planet.scanned(at, ground, |e, before| e.distance(at, before));
                assert_eq!(by_index.to_bits(), by_all.to_bits(), "distance at {at}");
                let ground = planet.field().sample_in(at, memo);
                let by_index = planet.shaped(at, ground, |e, before| e.apply(at, before));
                let by_all = planet.scanned(at, ground, |e, before| e.apply(at, before));
                assert_eq!(
                    by_index.distance.to_bits(),
                    by_all.distance.to_bits(),
                    "{at}"
                );
                assert_eq!(by_index.material, by_all.material, "material at {at}");
                let precise = DVec3::from(at) + 0.25;
                let near = planet.distance_near(precise, memo);
                let ground = planet.field().distance_near(precise, memo);
                let low = precise.as_vec3();
                let by_all = planet.scanned(low, ground, |e, before| e.distance(low, before));
                assert_eq!(
                    near.to_bits(),
                    by_all.to_bits(),
                    "near distance at {precise}"
                );
            }
        }
        let crossed = planet.index.stamps.iter().filter(|s| s.is_none()).count();
        let laid = planet.index.stamps.len() - crossed;
        assert_eq!(
            laid,
            planet.edits.len() + planet.bridges.len(),
            "laid as kept"
        );
        assert!(!planet.bridges.is_empty(), "boxes bridged");
        assert!(crossed > 0, "a bridge was taken away");
        assert!(!planet.index.large.is_empty(), "a large edit");
    }

    #[test]
    fn an_indexed_sample_is_every_edit_laid_in_order() {
        // Close together, where the edits that reach a point decide it, and far apart,
        // where fills out of reach over the ground in the air do.
        compare(12.0);
        compare(60.0);
    }
}
