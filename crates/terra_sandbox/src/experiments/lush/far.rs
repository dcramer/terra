//! The far forest (D273): the same forest carried on past the meadow round the walkable one,
//! out to where the planet's curve hides the ground even from a height, so the forest is not
//! an island. As SpeedTree games draw their farthest trees, each is a billboard: one card
//! turned to the eye about its upright, its crown drawn on it by the shader as the species'
//! silhouette, rounded and lit by Bevy's own lighting, its leaves' grain from the spike's oak
//! leaf picture (research/far-forest.md). The cards are merged into a mesh a square, so a few
//! hundred draws hold tens of thousands of trees, and none casts a shadow. Where they stand,
//! and which kind each is, follow the near forest's own rules (`species_at`, `glade_at`), so
//! its groves of pines and hollows of spruce run on past its edge.
//!
//! Beyond them, the far land in the sky carries the forest to the horizon (`terra_world::sky`).

use super::{FOREST_M, Species, floor::MEADOW_M, glade_at, species_at};
use bevy::{
    camera::visibility::NoAutoAabb, light::NotShadowCaster, platform::collections::HashMap,
    prelude::*, render::storage::ShaderBuffer,
};
use terra_gen::{Noise, Seed};
use terra_world::{
    planet::Planet,
    woods::{FarCard, FarExtension, FarWoods, far_mesh},
};

/// The far forest's material is the world's (`terra_world::woods`, D396): one definition of
/// far_forest.wesl's uniform, so a field the shader gains reaches this forest too.
pub(super) type FarForest = FarWoods;

/// The far forest starts this far past the meadow's edge, give or take its own noise, and
/// ends where nothing standing on the ground is seen any more from the overlook, 45 m up at
/// the forest's edge: a 30 m tree 770 m off is under its horizon on a planet 2 km round.
const FROM_M: f32 = 20.0;
const RAGGED_M: f32 = 45.0;
const TO_M: f32 = 1_000.0;
/// A tree to a square this wide, jittered within it; some squares are left empty.
const CELL_M: f32 = 6.5;
/// The cards are merged in squares this wide.
const CHUNK_M: f32 = 160.0;
/// Each kind's height range, as the near forest grows its grown trees (`KINDS`), and how wide
/// its crown is for its height.
const HEIGHTS: [(f32, f32); 3] = [(18.0, 27.0), (24.0, 34.0), (22.0, 32.0)];
const CROWN: [f32; 3] = [0.78, 0.42, 0.36];

/// Where the far forest is laid, and what with.
pub(super) struct Lay<'a> {
    pub planet: &'a Planet,
    pub start: Vec3,
    pub east: Vec3,
    pub south: Vec3,
    pub seed: Seed,
    pub glades: &'a Noise,
    pub groves: &'a Noise,
    pub clumps: &'a Noise,
    /// The bushes' leaf picture, for the crowns' grain.
    pub leaves: Handle<Image>,
    /// The near crowns' leaves' colours, oak, pine and spruce, linear.
    pub crowns: [Vec3; 3],
    /// The plants' shared buffer (terra_world's parting), whose look turns the far crowns
    /// with the near ones.
    pub parting: Handle<ShaderBuffer>,
}

/// Lays the far forest: a card for each tree, merged a square at a time.
pub(super) fn lay(
    lay: Lay,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<FarForest>,
) {
    let material = materials.add(FarForest {
        base: StandardMaterial {
            base_color_texture: Some(lay.leaves.clone()),
            alpha_mode: AlphaMode::Mask(0.5),
            perceptual_roughness: 0.85,
            reflectance: 0.2,
            diffuse_transmission: 0.3,
            ..default()
        },
        extension: FarExtension::new(
            lay.crowns.map(|c| c.extend(1.0)),
            Vec4::new(25.0, 45.0, 0.0, 0.0),
            lay.parting.clone(),
        ),
    });
    let edge = Noise::new(lay.seed.derive("far edge"));
    let cells = (TO_M / CELL_M).ceil() as i32;
    // A row of the grid at a time, on every core (D406): each tree is its own square's, by
    // the seed, and asks the ground where it stands; the rows are gathered in their order,
    // so the cards are as one thread would lay them.
    let seed = far_seed(lay.seed);
    let rows: Vec<i32> = (-cells..=cells).collect();
    let laid = terra_gen::par::map(&rows, |&i| {
        let mut row: Vec<((i32, i32), FarCard)> = Vec::new();
        for j in -cells..=cells {
            let cell = Vec2::new(i as f32, j as f32);
            let luck = |salt: u32| hash(seed, i, j, salt);
            let at = (cell + Vec2::new(luck(1), luck(2))) * CELL_M;
            let distance = at.length();
            // The meadow's far edge wanders, as a wood's edge does.
            let direction = at / distance.max(1.0);
            let ragged =
                0.5 + 0.5 * edge.fbm(Vec3::new(direction.x, 0.0, direction.y) * 6.0, 3, 2.0, 0.5);
            let from = FOREST_M + MEADOW_M + FROM_M + RAGGED_M * ragged;
            if !(from..TO_M).contains(&distance) {
                continue;
            }
            // Glades open, and trees stand in clumps with gaps, as in the near forest.
            let clumped = 0.5
                + 0.5
                    * lay
                        .clumps
                        .fbm(Vec3::new(at.x, 0.0, at.y) / 22.0, 2, 2.0, 0.5);
            let species = species_at(lay.groves, at, (luck(3) - 0.5) * 0.24);
            let stands = match species {
                Species::Oak => 0.55 + 0.4 * clumped,
                _ => 0.75 + 0.25 * clumped,
            };
            if glade_at(lay.glades, at) < -0.3 || luck(4) > stands * lay.planet.trees() {
                continue;
            }
            let (low, high) = HEIGHTS[species as usize];
            let height = low + (high - low) * luck(5);
            let width = height * CROWN[species as usize] * (0.85 + 0.3 * luck(6));
            let foot = lay
                .planet
                .ground_at(lay.start + lay.east * at.x + lay.south * at.y);
            let square = (
                (at.x / CHUNK_M).floor() as i32,
                (at.y / CHUNK_M).floor() as i32,
            );
            row.push((
                square,
                FarCard {
                    foot,
                    width,
                    height,
                    kind: species as usize,
                    luck: luck(7),
                    comes_in: 0.0,
                },
            ));
        }
        row
    });
    let mut squares: HashMap<(i32, i32), Vec<FarCard>> = HashMap::default();
    let mut count = [0usize; 3];
    for (square, card) in laid.into_iter().flatten() {
        count[card.kind] += 1;
        squares.entry(square).or_default().push(card);
    }
    let squares_count = squares.len();
    // The world's far cards, cut to their trees' outlines (D396: one primitive), a square's
    // mesh a core.
    let squares: Vec<Vec<FarCard>> = squares.into_values().collect();
    for (mesh, middle, aabb) in terra_gen::par::map(&squares, |cards| far_mesh(cards)) {
        commands.spawn((
            Name::new("Far forest"),
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(middle),
            aabb,
            NoAutoAabb,
            NotShadowCaster,
        ));
    }
    info!(
        "lush: far forest of {} oaks, {} pines and {} spruces, out to {TO_M:.0} m, in {squares_count} squares",
        count[0], count[1], count[2]
    );
}

/// The word the far trees' squares are drawn by: the forest's seed, derived for them.
fn far_seed(forest: Seed) -> u32 {
    forest.derive("far trees").0 as u32
}

/// A number from 0 to 1 for a square of the grid, the same every run of a world: the far
/// trees stand by the forest's seed, the world's (D596).
fn hash(seed: u32, i: i32, j: i32, salt: u32) -> f32 {
    let x = terra_gen::hash::lowbias32(
        seed ^ (i as u32).wrapping_mul(0x9E37_79B9)
            ^ (j as u32).wrapping_mul(0x85EB_CA6B)
            ^ salt.wrapping_mul(0xC2B2_AE35),
    );
    (x & 0xFF_FFFF) as f32 / 16_777_215.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_grids_luck_is_spread_and_repeatable() {
        let draws: Vec<f32> = (0..2000).map(|k| hash(9, k % 50 - 25, k / 50, 3)).collect();
        assert!(draws.iter().all(|d| (0.0..=1.0).contains(d)));
        let mean = draws.iter().sum::<f32>() / draws.len() as f32;
        assert!((mean - 0.5).abs() < 0.03, "{mean}");
        assert_eq!(hash(9, 7, -3, 1), hash(9, 7, -3, 1));
        assert_ne!(hash(9, 7, -3, 1), hash(9, 7, -3, 2));
    }

    /// D596: the far trees stand by the world's seed: two worlds' far forests differ square
    /// by square.
    #[test]
    fn two_worlds_far_trees_stand_apart() {
        let grid = |seed: Seed| -> Vec<f32> {
            let word = far_seed(seed);
            (0..400)
                .map(|k| hash(word, k % 20 - 10, k / 20, 1))
                .collect()
        };
        let (one, two) = (grid(Seed(1)), grid(Seed(2)));
        assert_eq!(one, grid(Seed(1)));
        let apart = one
            .iter()
            .zip(&two)
            .filter(|(a, b)| (*a - *b).abs() > 0.01)
            .count();
        assert!(apart > 380, "{apart} of 400 squares differ");
    }
}
