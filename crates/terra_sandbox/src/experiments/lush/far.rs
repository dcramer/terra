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
    asset::RenderAssetUsages,
    camera::{primitives::Aabb, visibility::NoAutoAabb},
    light::NotShadowCaster,
    mesh::{Indices, PrimitiveTopology},
    pbr::{ExtendedMaterial, MaterialExtension},
    platform::collections::HashMap,
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
};
use terra_gen::{Noise, Seed};
use terra_world::planet::Planet;

pub(super) type FarForest = ExtendedMaterial<StandardMaterial, FarExtension>;

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

pub(super) fn plugin(app: &mut App) {
    app.add_plugins(MaterialPlugin::<FarForest>::default());
}

/// What the far forest's shader is told. Field order must match far_forest.wesl.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub(super) struct FarExtension {
    /// Each kind's crown colour, oak, pine and spruce, linear: its leaves' as the near
    /// crowns show them.
    #[uniform(100)]
    crowns: [Vec4; 3],
    /// Within x of the eye a card is gone, from y on whole: walked into, the far forest
    /// gives way rather than show its cards.
    #[uniform(100)]
    near: Vec4,
}

impl MaterialExtension for FarExtension {
    fn vertex_shader() -> ShaderRef {
        "shaders/far_forest.wesl".into()
    }
    fn prepass_vertex_shader() -> ShaderRef {
        "shaders/far_forest.wesl".into()
    }
    fn fragment_shader() -> ShaderRef {
        "shaders/far_forest.wesl".into()
    }
    fn prepass_fragment_shader() -> ShaderRef {
        "shaders/far_forest.wesl".into()
    }
}

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
}

/// One far tree's card, in the mesh of its square.
struct Card {
    foot: Vec3,
    width: f32,
    height: f32,
    species: Species,
    /// 0 to 1, for the shader's own variation.
    luck: f32,
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
        extension: FarExtension {
            crowns: lay.crowns.map(|c| c.extend(1.0)),
            near: Vec4::new(25.0, 45.0, 0.0, 0.0),
        },
    });
    let edge = Noise::new(lay.seed.derive("far edge"));
    let cells = (TO_M / CELL_M).ceil() as i32;
    let mut squares: HashMap<(i32, i32), Vec<Card>> = HashMap::default();
    let mut count = [0usize; 3];
    for i in -cells..=cells {
        for j in -cells..=cells {
            let cell = Vec2::new(i as f32, j as f32);
            let luck = |salt: u32| hash(i, j, salt);
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
            if glade_at(lay.glades, at) < -0.3 || luck(4) > stands {
                continue;
            }
            let (low, high) = HEIGHTS[species as usize];
            let height = low + (high - low) * luck(5);
            let width = height * CROWN[species as usize] * (0.85 + 0.3 * luck(6));
            let foot = lay
                .planet
                .ground_at(lay.start + lay.east * at.x + lay.south * at.y);
            count[species as usize] += 1;
            squares
                .entry((
                    (at.x / CHUNK_M).floor() as i32,
                    (at.y / CHUNK_M).floor() as i32,
                ))
                .or_default()
                .push(Card {
                    foot,
                    width,
                    height,
                    species,
                    luck: luck(7),
                });
        }
    }
    let squares_count = squares.len();
    for (_, cards) in squares {
        let (mesh, middle, aabb) = merged(&cards);
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

/// A square's cards as one mesh about its middle: each card's four corners all at its foot,
/// the shader standing them up and turning them to the eye. The first texture coordinate is
/// the corner, the second the card's width and height, and the colour its kind, its luck
/// and how far up its crown starts. Its bounds hold the cards turned any way.
fn merged(cards: &[Card]) -> (Mesh, Vec3, Aabb) {
    let middle = cards.iter().map(|c| c.foot).sum::<Vec3>() / cards.len() as f32;
    let up = middle.normalize();
    let mut positions = Vec::with_capacity(cards.len() * 4);
    let mut normals = Vec::with_capacity(cards.len() * 4);
    let mut corners = Vec::with_capacity(cards.len() * 4);
    let mut sizes = Vec::with_capacity(cards.len() * 4);
    let mut colours = Vec::with_capacity(cards.len() * 4);
    let mut indices = Vec::with_capacity(cards.len() * 6);
    let (mut low, mut high) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for card in cards {
        let at = card.foot - middle;
        let reach = Vec3::splat(card.width * 0.5);
        let top = up * card.height;
        for p in [at - reach, at + reach, at - reach + top, at + reach + top] {
            low = low.min(p);
            high = high.max(p);
        }
        let first = positions.len() as u32;
        for corner in [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]] {
            positions.push(at.to_array());
            normals.push(up.to_array());
            corners.push(corner);
            sizes.push([card.width, card.height]);
            colours.push([card.species as u32 as f32 / 2.0, card.luck, 0.0, 1.0]);
        }
        indices.extend([first, first + 1, first + 2, first, first + 2, first + 3]);
    }
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, corners);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_1, sizes);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colours);
    mesh.insert_indices(Indices::U32(indices));
    (mesh, middle, Aabb::from_min_max(low, high))
}

/// A number from 0 to 1 for a square of the grid, the same every run.
fn hash(i: i32, j: i32, salt: u32) -> f32 {
    let x = terra_gen::hash::lowbias32(
        (i as u32).wrapping_mul(0x9E37_79B9)
            ^ (j as u32).wrapping_mul(0x85EB_CA6B)
            ^ salt.wrapping_mul(0xC2B2_AE35),
    );
    (x & 0xFF_FFFF) as f32 / 16_777_215.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_squares_bounds_hold_its_cards_turned_any_way() {
        let up = Vec3::Y * 2000.0;
        let cards = [
            Card {
                foot: up + Vec3::new(3.0, 0.0, -4.0),
                width: 10.0,
                height: 25.0,
                species: Species::Oak,
                luck: 0.3,
            },
            Card {
                foot: up + Vec3::new(-50.0, 0.4, 20.0),
                width: 6.0,
                height: 30.0,
                species: Species::Pine,
                luck: 0.9,
            },
        ];
        let (mesh, middle, aabb) = merged(&cards);
        assert_eq!(mesh.count_vertices(), 8);
        let (low, high) = (Vec3::from(aabb.min()), Vec3::from(aabb.max()));
        for card in &cards {
            for turn in 0..16 {
                let angle = turn as f32 / 16.0 * std::f32::consts::TAU;
                let across = Vec3::new(libm::cosf(angle), 0.0, libm::sinf(angle));
                for corner in [-0.5f32, 0.5] {
                    for rise in [0.0, 1.0] {
                        let p = card.foot - middle
                            + across * corner * card.width
                            + Vec3::Y * rise * card.height;
                        assert!(
                            p.cmpge(low - 1e-3).all() && p.cmple(high + 1e-3).all(),
                            "{p} outside {low} to {high}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn the_grids_luck_is_spread_and_repeatable() {
        let draws: Vec<f32> = (0..2000).map(|k| hash(k % 50 - 25, k / 50, 3)).collect();
        assert!(draws.iter().all(|d| (0.0..=1.0).contains(d)));
        let mean = draws.iter().sum::<f32>() / draws.len() as f32;
        assert!((mean - 0.5).abs() < 0.03, "{mean}");
        assert_eq!(hash(7, -3, 1), hash(7, -3, 1));
        assert_ne!(hash(7, -3, 1), hash(7, -3, 2));
    }
}
