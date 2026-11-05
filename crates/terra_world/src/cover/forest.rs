//! The forest floor's map (D246, D619): what grows and lies under trees, by the light their
//! canopy lets through, as a real floor's does. The lush forest's rules, the world's since its
//! woods have the forest walk's floor (D619): the forest walk lays its paths over it, the round
//! world's woods lay it round their own trees (`woods`), and the layer above lays what stands
//! on it, logs, rocks and the undergrowth, telling it where so the grass keeps clear.
//!
//! - **Grass follows the light.** Thick and tall in the clearings, along the paths and in
//!   the gaps between crowns; under a closed canopy it gives out to fallen leaves, to
//!   needles under the pines and spruces, to moss, so the floor is always full but never
//!   one lawn.
//! - **Feet in the ground.** Grass is pushed back from every trunk, a ring of duff, roots
//!   and moss round its foot; fallen logs and rocks lie in duff of their own; a path's edge
//!   frays into dirt, roots and leaves.

use super::{CoverSource, Growing};
use crate::planet::Planet;
use crate::trees::Species;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use std::sync::Arc;
use terra_gen::{Noise, Rng, Seed, smoothstep};

/// How much grass grows under a closed canopy, as a share of a lit lawn's, before its tufts
/// (D293): Sons of the Forest's floor under its trees is grass and ferns, not bare litter.
const SHADE_GRASS: f32 = 0.62;
/// Trees, logs and rocks are filed for the floor in squares this wide.
const FLOOR_SQUARE_M: f32 = 8.0;
/// How far round a place the canopy over it is looked for: the widest crown.
const CROWN_REACH: i32 = 2;
/// How far a log's or rock's snow well reaches past its edge, metres.
const LOG_WELL_M: f32 = 0.8;
/// How far out from a shrub's or young tree's foot its well reaches: as snow lies round a
/// stool of stems or a young spruce, hollowed under its boughs and lipped where the wind
/// eddies, so a stem stands in the snow rather than out of a flat white sheet.
const FEET_WELL_M: f32 = 0.5;
/// How far the nearest bark is said to be, at most: none nearer is none at all.
pub const NO_BARK_M: f32 = 8.0;

/// The square of a grid this wide a place on the map is in.
fn square(at: Vec2, side: f32) -> (i32, i32) {
    ((at.x / side).floor() as i32, (at.y / side).floor() as i32)
}

/// The ways through a floor, its paths: how far a place is from the nearest one's edge.
pub trait Ways: Send + Sync + 'static {
    /// How far a place is outside the nearest path's edge, negative on it; a large number
    /// far from every path.
    fn edge_distance(&self, at: Vec2) -> f32;

    /// A place just outside a path's edge, up to so far out, where what lines a path is
    /// tried; none where there are no paths.
    fn beside(&self, dice: &mut Rng, out_m: f32) -> Option<Vec2>;
}

/// A floor with no paths through it: a wild wood's.
pub struct Wild;

impl Ways for Wild {
    fn edge_distance(&self, _at: Vec2) -> f32 {
        1_000.0
    }

    fn beside(&self, _dice: &mut Rng, _out_m: f32) -> Option<Vec2> {
        None
    }
}

/// A tree as the floor sees it: where its foot is, how far its bark is from its middle
/// there, how far its crown spreads and how much of the sky it hides under it, and whether
/// it drops needles.
#[derive(Clone, Copy, Debug)]
pub struct Stand {
    pub at: Vec2,
    pub trunk_m: f32,
    pub crown_at: Vec2,
    pub crown_m: f32,
    pub closes: f32,
    pub conifer: bool,
    pub spruce: bool,
}

impl Stand {
    /// How deep in this tree's well `at` is, 1 at its bark to 0 at the well's edge (winter
    /// round 12): a crown intercepts the snow over its foot and the trunk shelters it, so the
    /// well reaches about as far as the thickest boughs, near a canopy wide under a big
    /// conifer, a hand or two round a sapling, and further one way than another, as the
    /// boughs hang, as the tree wells round a snowy forest's trunks lie.
    fn well(&self, at: Vec2) -> f32 {
        let off = at - self.at;
        let d = off.length() - self.trunk_m;
        let held = if self.conifer { 0.45 } else { 0.2 };
        let reach = (0.25 + 1.2 * self.trunk_m + held * self.crown_m).clamp(0.35, 3.2);
        if d > reach * 1.4 {
            return 0.0;
        }
        let turn = libm::atan2f(off.y, off.x);
        let phase = (self.at.x * 0.731 + self.at.y * 1.917).rem_euclid(core::f32::consts::TAU);
        let lobes = 1.0
            + 0.22 * libm::sinf(2.0 * turn + phase)
            + 0.12 * libm::sinf(3.0 * turn + 2.3 * phase);
        1.0 - smoothstep(0.0, reach * lobes, d)
    }

    /// A tree of a species standing at `at`, its crown's middle at `crown_at` reaching
    /// `crown_m` round it: an oak's crown hides most of the sky under it, a Scots pine's a
    /// little less, a spruce's nearly all; a young tree's less.
    pub fn of(
        at: Vec2,
        crown_at: Vec2,
        crown_m: f32,
        species: Species,
        young: bool,
        trunk_m: f32,
    ) -> Self {
        let closes = match species {
            Species::Oak => 0.85,
            // A Scots pine's crown of pads hides three quarters of the sky straight up through
            // it, measured from its cards (D318); its old whorls of spray cards hid less.
            Species::Pine => 0.78,
            Species::Spruce => 0.92,
        };
        Self {
            at,
            // The root flare spreads past the trunk.
            trunk_m: trunk_m * 1.3,
            crown_at,
            crown_m,
            closes: closes * if young { 0.7 } else { 1.0 },
            conifer: species != Species::Oak,
            spruce: species == Species::Spruce,
        }
    }

    /// A stump's foot, in duff and roots as a tree's, with no crown over it.
    pub fn stump(at: Vec2, trunk_m: f32) -> Self {
        Self {
            at,
            trunk_m,
            crown_at: at,
            crown_m: 0.0,
            closes: 0.0,
            conifer: false,
            spruce: false,
        }
    }

    /// How much of the crown is over a place, 0 to 1: all of it under its middle, none past
    /// its edge.
    fn under(&self, at: Vec2) -> f32 {
        1.0 - smoothstep(
            0.4 * self.crown_m,
            1.0 * self.crown_m,
            self.crown_at.distance(at),
        )
    }
}

/// A crown's middle, level with the foot, and how far its leaves reach from it, from where
/// its clumps of leaves are and how big.
pub fn crown_of(tips: &[(Vec3, f32)]) -> (Vec3, f32) {
    let count = tips.len().max(1) as f32;
    let middle = tips.iter().fold(Vec3::ZERO, |sum, (tip, _)| {
        sum + Vec3::new(tip.x, 0.0, tip.z)
    }) / count;
    let reach = tips
        .iter()
        .map(|(tip, size)| Vec2::new(tip.x - middle.x, tip.z - middle.z).length() + size * 0.5)
        .fold(0.5, f32::max);
    (middle, reach)
}

/// The trees over the floor, filed by square.
#[derive(Default)]
pub struct Crowns {
    filed: HashMap<(i32, i32), Vec<Stand>>,
}

/// What the canopy makes of a place: how much of the sky it hides, how much of what falls
/// is needles, how much of the canopy over it is spruce, and how far the nearest bark is.
struct Over {
    shade: f32,
    needles: f32,
    spruce: f32,
    /// How much of what falls here is a broadleaf's leaves, from the crowns near, whatever
    /// light comes through them (D294).
    leaves: f32,
    bark: f32,
    /// How deep in a tree well this is, 1 at the bark to 0 at the well's edge (`Stand::well`).
    well: f32,
}

impl Crowns {
    pub fn add(&mut self, stand: Stand) {
        self.filed
            .entry(square(stand.at, FLOOR_SQUARE_M))
            .or_default()
            .push(stand);
    }

    /// Whether a place is among the trees, as the woods' floor is laid (D619): within
    /// `NO_BARK_M` of a trunk's bark, less the half metre where the floor gives out.
    pub fn among(&self, at: Vec2) -> bool {
        let (i, j) = square(at, FLOOR_SQUARE_M);
        (-CROWN_REACH..=CROWN_REACH).any(|di| {
            (-CROWN_REACH..=CROWN_REACH).any(|dj| {
                self.filed
                    .get(&(i + di, j + dj))
                    .into_iter()
                    .flatten()
                    .any(|stand| stand.at.distance(at) - stand.trunk_m < NO_BARK_M - 0.5)
            })
        })
    }

    fn over(&self, at: Vec2) -> Over {
        let (i, j) = square(at, FLOOR_SQUARE_M);
        let (mut light, mut needles_miss, mut bark, mut well) = (1.0f32, 1.0f32, NO_BARK_M, 0.0f32);
        // Under a broadleaf crown its own leaves fall over the needles.
        let mut leaves_miss = 1.0f32;
        let mut spruce_miss = 1.0f32;
        // And its leaves fall a little past its crown, as needles do, sun or shade.
        let mut fallen_miss = 1.0f32;
        for di in -CROWN_REACH..=CROWN_REACH {
            for dj in -CROWN_REACH..=CROWN_REACH {
                for stand in self.filed.get(&(i + di, j + dj)).into_iter().flatten() {
                    bark = bark.min(stand.at.distance(at) - stand.trunk_m);
                    well = well.max(stand.well(at));
                    light *= 1.0 - stand.under(at) * stand.closes;
                    if stand.conifer {
                        // Needles fall a little past the crown.
                        let d = stand.crown_at.distance(at);
                        let fall = 1.0 - smoothstep(0.8 * stand.crown_m, 2.4 * stand.crown_m, d);
                        needles_miss *= 1.0 - fall * 0.95;
                    } else {
                        leaves_miss *= 1.0 - stand.under(at) * 0.9;
                        let d = stand.crown_at.distance(at);
                        let fall = 1.0 - smoothstep(0.8 * stand.crown_m, 2.2 * stand.crown_m, d);
                        fallen_miss *= 1.0 - fall * 0.9;
                    }
                    if stand.spruce {
                        spruce_miss *= 1.0 - stand.under(at) * 0.9;
                    }
                }
            }
        }
        Over {
            shade: 1.0 - light,
            needles: (1.0 - needles_miss) * (0.25 + 0.75 * leaves_miss),
            spruce: 1.0 - spruce_miss,
            leaves: 1.0 - fallen_miss,
            bark,
            well,
        }
    }
}

/// Circles of what lies on the floor, logs and rocks, filed by square.
#[derive(Default)]
pub struct Lying {
    filed: HashMap<(i32, i32), Vec<(Vec2, f32)>>,
}

impl Lying {
    pub fn add(&mut self, at: Vec2, radius: f32) {
        self.filed
            .entry(square(at, FLOOR_SQUARE_M))
            .or_default()
            .push((at, radius));
    }

    /// How far a place is from the nearest edge of what lies there, up to 8 m; negative
    /// under it.
    fn edge(&self, at: Vec2) -> f32 {
        let (i, j) = square(at, FLOOR_SQUARE_M);
        let mut nearest = 8.0f32;
        for di in -1..=1 {
            for dj in -1..=1 {
                for (middle, radius) in self.filed.get(&(i + di, j + dj)).into_iter().flatten() {
                    nearest = nearest.min(middle.distance(at) - radius);
                }
            }
        }
        nearest
    }
}

/// A forest's map, on a flat patch laid over the ground, in metres east and south of the
/// start, through which `W`'s ways run. Asked by the ground cover from other threads.
pub struct Floor<W: Ways = Wild> {
    pub ways: W,
    pub canopy: Crowns,
    /// What lies on the floor, logs and rocks, which the placement of the rest reads.
    pub lying: Lying,
    /// The feet of the middle layer's shrubs and young trees, for their wells alone: apart
    /// from `lying`, which placement reads, so no other plant moves for them.
    pub feet: Lying,
    noise: Noise,
}

/// What the floor is like at a place, for the undergrowth to choose by.
#[derive(Clone, Copy, Debug)]
pub struct Here {
    /// How much of the sky's light reaches it, with the light down a path's corridor.
    pub light: f32,
    pub needles: f32,
    /// How much of the canopy over it is spruce.
    pub spruce: f32,
    pub leaves: f32,
    /// How far the nearest bark, the nearest log or rock's edge, and the nearest path's
    /// edge are.
    pub bark: f32,
    pub well: f32,
    pub lying: f32,
    /// How far the nearest shrub's foot is.
    pub feet: f32,
    pub path: f32,
    /// Damp hollows, where ferns and moss grow best.
    pub wet: f32,
    /// 1 where the sun reaches it past the crowns and 0 where not, for the kinds that grow
    /// in its patches; the sky's light where not asked.
    pub sun: f32,
}

impl<W: Ways> Floor<W> {
    /// A floor under `canopy` with `ways` through it, its patches from `seed`.
    pub fn new(ways: W, canopy: Crowns, seed: Seed) -> Self {
        Self {
            ways,
            canopy,
            lying: Lying::default(),
            feet: Lying::default(),
            noise: Noise::new(seed),
        }
    }

    fn noise(&self, at: Vec2, height: f32, across: f32) -> f32 {
        0.5 + 0.5 * self.noise.sample(Vec3::new(at.x, height, at.y) / across)
    }

    /// How far outside a path's edge a place is, the edge wandering and fraying.
    fn path_edge(&self, at: Vec2) -> f32 {
        self.ways.edge_distance(at)
            + (self.noise(at, 71.0, 2.6) - 0.5) * 1.3
            + (self.noise(at, 73.0, 0.6) - 0.5) * 0.4
    }

    pub fn here(&self, at: Vec2) -> Here {
        let over = self.canopy.over(at);
        let path = self.path_edge(at);
        // Gaps in the canopy, where a crown is thin or a tree has fallen.
        let gap = smoothstep(0.58, 0.82, self.noise(at, 19.0, 10.0));
        let shade = over.shade * (1.0 - 0.45 * gap);
        // A path's corridor lets the light down to its edges.
        let corridor = 1.0 - smoothstep(0.5, 5.0, path.max(0.0));
        let light = (1.0 - shade + 0.45 * corridor).min(1.0);
        Here {
            light,
            sun: light,
            needles: over.needles,
            spruce: over.spruce,
            leaves: over.leaves,
            bark: over.bark,
            well: over.well,
            lying: self.lying.edge(at),
            feet: self.feet.edge(at),
            path,
            wet: smoothstep(0.35, 0.75, self.noise(at, 61.0, 13.0)),
        }
    }

    /// What grows at a place: grass by the light, thick in the open and in gaps and along
    /// paths, giving out under a closed canopy to leaves, needles under conifers, moss in
    /// the damp; pushed back from trunks, logs and rocks by a ring of duff and roots; none
    /// on the paths, whose edges fray into dirt, roots and leaves.
    pub fn growing(&self, at: Vec2) -> Growing {
        self.growing_here(at, &self.here(at))
    }

    fn growing_here(&self, at: Vec2, here: &Here) -> Growing {
        let path = here.path;
        // The path's edge: no grass on it, short and broken grass in a fraying band beside
        // it, dirt, roots and leaves in that band.
        // Grass comes up close to a path's edge, as it does beside a trodden track (D293).
        let off_path = smoothstep(-0.1, 0.7, path);
        let fray = (1.0 - smoothstep(0.0, 1.5, path)) * smoothstep(-0.35, 0.1, path);
        let patch = 0.6
            + 0.4
                * self
                    .noise
                    .fbm(Vec3::new(at.x, 0.0, at.y) / 9.0, 2, 2.0, 0.5);
        // In the shade grass still covers most of the floor, as shade grasses and sedges do
        // under Sons of the Forest's canopy (D293), thickest where a fleck of light falls, in
        // tufts, and giving way to bare litter at the trees' feet and on the paths.
        let tufts = smoothstep(0.45, 0.85, self.noise(at, 23.0, 2.6));
        let lit = smoothstep(0.22, 0.72, here.light);
        // Pushed back from the bark, the logs and the rocks.
        let wander = (self.noise(at, 53.0, 1.3) - 0.5) * 1.2;
        let foot = smoothstep(0.2, 0.85, here.bark + wander * 0.4);
        let beside = smoothstep(0.0, 0.55, here.lying + wander * 0.3);
        // How thick the light lets it grow, thinner and drier on the needles under pines.
        let light_grows = (1.0 - 0.15 * smoothstep(0.2, 0.7, here.needles))
            * (lit + (1.0 - lit) * (SHADE_GRASS + 0.3 * tufts));
        // Thin grass grows in clumps, not as a stubble of lone blades spread evenly.
        let clumps = smoothstep(0.35, 0.65, self.noise(at, 89.0, 1.6));
        let clumped = clumps + (1.0 - clumps) * smoothstep(0.3, 0.75, light_grows);
        // A path little walked keeps tussocks along it, more toward its edges (D293).
        let trodden = off_path.max(
            (0.45 + 0.4 * smoothstep(-1.2, 0.0, path))
                * smoothstep(0.45, 0.75, self.noise(at, 29.0, 1.1)),
        );
        let grass = trodden * foot * beside * patch * light_grows * clumped;
        // Faint grass is none.
        let grass = grass * smoothstep(0.04, 0.2, grass);
        let floor = 1.0 - lit;
        let canopied = floor.max(0.75 * smoothstep(0.04, 0.3, 1.0 - here.light));
        let on_floor = smoothstep(-0.3, 0.3, path);
        // Needles carpet a conifer stand, between its trees as well as under them.
        let needles = smoothstep(0.1, 0.5, here.needles) * off_path.max(0.4);
        let mossy = self.noise(at, 31.0, 3.5);
        // Duff and roots round a tree's foot, its edge wandering; along logs and rocks.
        let ring = 1.0 - smoothstep(0.0, 1.5, here.bark + wander);
        let along = 1.0 - smoothstep(0.0, 0.7, here.lying + wander * 0.4);
        // Dark duff in the damp hollows under the canopy.
        let damp = floor * smoothstep(0.6, 0.85, here.wet + (mossy - 0.5) * 0.4) * 0.65;
        let roots = (ring * 0.95)
            .max(along * 0.7)
            .max(damp)
            .max(fray * (0.35 + 0.5 * self.noise(at, 79.0, 0.9)));
        // Moss round a foot only on some sides of it, in broken patches, never a halo; along
        // logs and rocks; on the floor only in the dampest hollows, in cushions.
        let broken = smoothstep(0.4, 0.7, self.noise(at, 37.0, 0.9));
        let moss = (ring * broken * (0.2 + 0.6 * mossy))
            .max(along * 0.5)
            .max(floor * smoothstep(0.5, 0.82, mossy * 0.7 + here.wet * 0.4) * 0.75)
            * off_path.max(0.3);
        Growing {
            grass,
            // Grass in the shade is a deeper green; in the open it dries in patches.
            lush: self.noise(at, 7.0, 20.0).max(floor * 0.8),
            path: path < 0.0,
            // Fallen leaves cover the floor wherever trees stand near, not only in their deep shade:
            // off the paths no forest floor is bare; on them a few drift into the hollows. Leaves
            // fall where broadleaves stand, sun or shade, as needles do where pines stand (D294).
            litter: (canopied.max(0.9 * smoothstep(0.1, 0.5, here.leaves)) * (1.0 - needles))
                .max(fray * 0.7)
                * on_floor
                + (1.0 - on_floor) * 0.35 * smoothstep(0.45, 0.8, self.noise(at, 97.0, 1.7)),
            moss,
            needles,
            roots,
            shade: 1.0 - here.light,
            // A forest's paths are its own floor worn, not a settlement's packed earth.
            trodden: 0.0,
            heath: 0.0,
            shrub: 0.0,
            lichen: 0.0,
            heath_moss: 0.0,
            // Litter heaps against trunks, logs and rocks, its edge wandering (D294).
            heap: 1.0 - smoothstep(0.0, 0.9, here.bark.min(here.lying) + wander * 0.25),
            stands: here
                .well
                .max(1.0 - smoothstep(0.0, LOG_WELL_M, here.lying))
                .max(1.0 - smoothstep(0.0, FEET_WELL_M, here.feet)),
            // Tall meadow grass in patches where the sun reaches; in the shade, clumps of
            // taller sedge among the short grass, so it is not one height (D293).
            tall: (smoothstep(0.7, 0.95, here.light)
                * smoothstep(0.4, 0.7, self.noise(at, 41.0, 7.0)))
            .max(0.6 * floor * smoothstep(0.6, 0.8, self.noise(at, 43.0, 1.9))),
        }
    }
}

/// How wooded a place is, 1 among trees to 0 where no trunk stands within `NO_BARK_M`: where
/// a wood's floor gives way to the land's own cover beyond it.
fn wooded(here: &Here) -> f32 {
    1.0 - smoothstep(NO_BARK_M - 3.0, NO_BARK_M - 0.5, here.bark)
}

/// A wood's floor laid round the world's own trees (`woods`), before the layer above lays
/// what stands on it (in `LayingFloor`) and the ground cover is grown from it, which takes
/// it: the land's cover where no tree stands, the forest's under and between them.
#[derive(Resource)]
pub struct WoodsFloor(pub Option<Floor<Wild>>);

/// Where the layer above lays what stands on the woods' floor, logs, rocks and undergrowth,
/// on entering a world: after the woods are planted, before the ground cover is grown.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct LayingFloor;

/// The ground cover of a world whose woods have a floor: the forest floor's where trees
/// stand, blended into the land's own cover (`land`) where they give out.
pub(super) struct WoodsCover {
    pub floor: Floor<Wild>,
    pub land: Arc<dyn CoverSource>,
}

impl CoverSource for WoodsCover {
    fn growing(&self, planet: &Planet, ground: Vec3, at: Vec2) -> Growing {
        let land = self.land.growing(planet, ground, at);
        if super::heath::drowned(planet, ground) || planet.dug(ground) {
            return land;
        }
        let here = self.floor.here(at);
        let wood = wooded(&here);
        if wood <= 0.0 {
            return land;
        }
        let forest = self.floor.growing_here(at, &here);
        mix(&land, &forest, wood)
    }
}

/// `a` with `b` laid over it, `t` of the way.
fn mix(a: &Growing, b: &Growing, t: f32) -> Growing {
    if t <= 0.0 {
        return *a;
    }
    if t >= 1.0 {
        return *b;
    }
    let m = |x: f32, y: f32| x + (y - x) * t;
    Growing {
        grass: m(a.grass, b.grass),
        lush: m(a.lush, b.lush),
        path: if t < 0.5 { a.path } else { b.path },
        litter: m(a.litter, b.litter),
        moss: m(a.moss, b.moss),
        needles: m(a.needles, b.needles),
        roots: m(a.roots, b.roots),
        shade: m(a.shade, b.shade),
        tall: m(a.tall, b.tall),
        heap: m(a.heap, b.heap),
        stands: m(a.stands, b.stands),
        trodden: m(a.trodden, b.trodden),
        heath: m(a.heath, b.heath),
        shrub: m(a.shrub, b.shrub),
        lichen: m(a.lichen, b.lichen),
        heath_moss: m(a.heath_moss, b.heath_moss),
    }
}

#[cfg(test)]
#[path = "forest_tests.rs"]
mod tests;
