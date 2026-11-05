//! The world's trees (D198): the lush forest's oaks, Scots pines and Norway spruces, grown by
//! rules as SpeedTree grows them, a trunk splitting into limbs, branches and twigs, each a
//! tube with bark on it, and leaves as cards in clumps at the ends of the branches. What a
//! tree is (`Tree`, its `Species` and `Form`) and how its shape is made into meshes: its wood
//! at each level of detail (`wood`), its crown near and far and the stand-in its shadow is
//! cast from (`crown`), each card (`card`) and the solid lumps far off (`lump`), every vertex
//! carrying its limb's pivot for wind in levels (D597, `crate::grown::pivots`).
//!
//! Moved here from the sandbox's lush forest (`terra_sandbox::experiments::lush`), whose
//! species and kinds still grow them there, toward every world's woods drawing them
//! (core-game-woods, D366).

pub mod build;
pub mod card;
pub mod crown;
pub mod lump;
pub mod wood;

use crate::sky_occlusion;
use bevy::prelude::*;

/// What a tree is: its habit, bark and leaves.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Species {
    Oak,
    Pine,
    Spruce,
}

/// How a tree grew. An oak: tall and narrow among others, broad where it had room, or
/// forked low into two stems. A Scots pine (D276): its leader running on to a pointed top;
/// broad, an old pine's flat umbrella of long level branches; forked high into two leaders;
/// or broken, its top snapped off and a branch below turned up to lead in its place.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Form {
    Tall,
    Broad,
    Forked,
    Broken,
}

/// A limb of a tree: a line of points, with how thick it is at each.
pub struct Limb {
    pub points: Vec<Vec3>,
    pub radii: Vec<f32>,
    pub depth: u32,
    /// Whether it runs out to a point, as a branch does; a cut log does not.
    pub pointed: bool,
}

/// How far a tree's limbs and leaves reach from its trunk within 1.8 m of the ground.
pub fn skirt(tree: &Tree) -> f32 {
    let low = |p: &Vec3| p.y < 1.8;
    let limbs = tree
        .limbs
        .iter()
        .filter(|limb| limb.depth > 0)
        .flat_map(|limb| {
            limb.points
                .iter()
                .filter(|p| low(p))
                .map(|p| Vec2::new(p.x, p.z).length())
        });
    let tips = tree
        .tips
        .iter()
        .filter(|(p, _)| low(p))
        .map(|(p, size)| Vec2::new(p.x, p.z).length() + size * 0.5);
    limbs.chain(tips).fold(0.0, f32::max)
}

/// A tree's shape, before it is made into meshes.
pub struct Tree {
    pub limbs: Vec<Limb>,
    /// Where each clump of leaves is, and how big.
    pub tips: Vec<(Vec3, f32)>,
    /// Which branch each tip grows on, counted from the great limbs out.
    pub branch_of: Vec<usize>,
    pub branches: usize,
    pub trunk_m: f32,
    /// Which way each clump's twig runs, outward: its cards' stalks are toward it.
    pub along: Vec<Vec3>,
    /// Which mass of the crown each clump is in, and each mass's middle, which its cards'
    /// normals lean out of, so the crown shades as masses with gaps, not as one ball.
    pub mass_of: Vec<usize>,
    pub masses: Vec<Vec3>,
    pub species: Species,
    /// Which limbs are dead wood, grey where the bark has gone (D276).
    pub dead: Vec<usize>,
    /// A Scots pine's bark: how far up its bole, as a share of its height, the grey-brown
    /// fissured bark gives way to thin orange plates, and how orange they are (D276).
    pub plates: Vec2,
}

impl Tree {
    pub fn new(species: Species, trunk_m: f32) -> Self {
        Self {
            limbs: Vec::new(),
            tips: Vec::new(),
            branch_of: Vec::new(),
            branches: 0,
            trunk_m,
            along: Vec::new(),
            mass_of: Vec::new(),
            masses: Vec::new(),
            species,
            dead: Vec::new(),
            plates: Vec2::new(0.3, 1.0),
        }
    }

    /// A limb of dead wood, bark gone grey, as a pine's shed branches and broken top are.
    pub fn dead_limb(&mut self, points: Vec<Vec3>, radii: Vec<f32>, depth: u32) {
        self.dead.push(self.limbs.len());
        self.limb(points, radii, depth);
    }

    pub fn limb(&mut self, points: Vec<Vec3>, radii: Vec<f32>, depth: u32) {
        self.limbs.push(Limb {
            points,
            radii,
            depth,
            pointed: depth > 0,
        });
    }

    /// A clump of leaves at a point, its twig running `along`, in a branch's lump (for
    /// the far crown and its shadow) and a mass of the crown (for its light).
    pub fn clump(&mut self, at: Vec3, size: f32, along: Vec3, branch: usize, mass: usize) {
        self.tips.push((at, size));
        self.along.push(along.normalize_or(Vec3::Y));
        self.branch_of.push(branch);
        self.mass_of.push(mass);
        self.branches = self.branches.max(branch + 1);
    }

    /// Takes away every clump lower than `y`.
    pub fn bare_below(&mut self, y: f32) {
        let keep: Vec<bool> = self.tips.iter().map(|(at, _)| at.y >= y).collect();
        let mut k = keep.iter();
        self.tips.retain(|_| *k.next().unwrap_or(&true));
        let mut k = keep.iter();
        self.along.retain(|_| *k.next().unwrap_or(&true));
        let mut k = keep.iter();
        self.branch_of.retain(|_| *k.next().unwrap_or(&true));
        let mut k = keep.iter();
        self.mass_of.retain(|_| *k.next().unwrap_or(&true));
    }

    /// Each mass's middle, from its clumps.
    pub fn settle(&mut self) {
        let count = self.mass_of.iter().copied().max().map_or(0, |m| m + 1);
        let mut sums = vec![(Vec3::ZERO, 0.0f32); count];
        for ((at, _), &mass) in self.tips.iter().zip(&self.mass_of) {
            sums[mass].0 += *at;
            sums[mass].1 += 1.0;
        }
        self.masses = sums.into_iter().map(|(sum, n)| sum / n.max(1.0)).collect();
    }
}
/// A tree's crown as the sky's light sees it (D270): an ellipsoid round its leaves, from
/// where it stands, how tall it is and how far its leaves reach; an oak's crown fills its
/// upper two thirds, a Scots pine's its top fifth, a spruce's nearly all of it. How much of
/// the sky it hides straight up through it is the floor's own measure (`floor::Stand`).
pub fn crown_seen(
    middle_at_foot: Vec3,
    up: Vec3,
    height: f32,
    reach: f32,
    species: Species,
    young: bool,
) -> sky_occlusion::Crown {
    let (mid, half, wide, closes) = match species {
        Species::Oak => (0.68, 0.32, 1.0, 0.85),
        Species::Pine => (0.82, 0.16, 1.0, 0.78),
        // A spruce of the tall stand: its crown from a third of its height (D318).
        Species::Spruce => (0.66, 0.34, 0.75, 0.92),
    };
    sky_occlusion::Crown {
        middle: middle_at_foot + up * height * mid,
        half: Vec3::new(reach * wide, height * half, reach * wide),
        closes: closes * if young { 0.7 } else { 1.0 },
    }
}
