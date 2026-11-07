//! The marsh's ground cover and debris, the kit's scatter (kit/scatter.rs) over the marsh's
//! ground: what kinds, how many and where are biomes/marsh.ron's `scatter`; this offers its
//! maps (terrain.rs `SITES`, land.rs `cover`), the baked rules (Mud, Wet by the channels),
//! the water's face, and keeps the path, the cast, the fire and the stones clear.

use super::dress::{FIRE, PROPS};
use super::land::{cover, path, shape};
use super::{FOE, KNIGHT, LEVEL, TARGET};
use crate::kit::biome::Biome;
use crate::kit::scatter::{self, Budget, Ground};
use crate::terrain::{Land, SITES};
use bevy::prelude::*;

/// The ground strewn (world x, z: the frame and a margin).
const LO: Vec2 = Vec2::new(-14.0, -13.0);
const HI: Vec2 = Vec2::new(15.0, 8.0);
/// How far the scatter keeps from each of the cast and the fire (m).
const CAST: f32 = 2.0;

pub fn plugin(app: &mut App) {
    app.add_plugins(scatter::plugin).add_systems(Startup, spawn);
}

/// Whether `p` is kept clear: the path, the cast's ground, the fire, the stones and stumps.
pub fn kept(p: Vec2) -> bool {
    let props = PROPS.iter().any(|&(path, at, _, scale, _)| {
        let keep = if path.contains("rock") {
            scale * 1.2
        } else {
            0.4
        };
        at.distance(p) < keep
    });
    path(p) > 0.02
        || [KNIGHT, FOE, TARGET].iter().any(|c| c.distance(p) < CAST)
        || FIRE.distance(p) < 1.2
        || props
}

/// The marsh's map `k` of `SITES` at `p`.
fn site(k: usize, p: Vec2) -> f32 {
    let h = shape(p);
    let (a, b, _) = cover(p, Vec3::Y, h);
    [a[0], a[1], a[2], a[3], b[0], b[1]][k]
}

#[allow(clippy::too_many_arguments)]
fn spawn(
    mut commands: Commands,
    asset: Res<AssetServer>,
    land: Res<Land>,
    biome: Res<Biome>,
    mut budget: ResMut<Budget>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<crate::foliage::FoliageMaterial>>,
    mut mats: ResMut<crate::foliage::Mats>,
) {
    let shade = |_: Vec2| 0.0;
    let place = crate::kit::ground::Place {
        land: &land,
        sites: SITES,
        water: Some(LEVEL),
        shade: &shade,
    };
    let rules = crate::kit::ground::bake(&place, &biome.ground, biome.weather.as_ref());
    let height = |p: Vec2| land.height(p);
    let g = Ground {
        sites: &SITES,
        site: &site,
        height: &height,
        water: Some(LEVEL),
        rules: Some(&rules),
        keep: &kept,
        span: (LO, HI),
    };
    let name = biome.refs.clone();
    scatter::spawn(
        &mut commands,
        &asset,
        &mut budget,
        &name,
        &biome.scatter,
        &g,
    );
    let clumps = scatter::plants(&mut budget, &name, &biome.scatter, &g);
    scatter::draw_plants(
        &mut commands,
        &asset,
        &mut budget,
        &land,
        &mut meshes,
        &mut materials,
        &mut mats,
        &clumps,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The marsh's scatter as its file says: none on the path or the cast; what stands by
    /// the water (the bank's mat and debris) within its band of the face, and the dry
    /// cover never in the water.
    #[test]
    fn the_marsh_scatter_keeps_the_fight_clear_and_the_water_open() {
        let biome = Biome::load("marsh").unwrap();
        assert!(
            biome.scatter.len() >= 4,
            "the marsh strews {} sets",
            biome.scatter.len()
        );
        let land = Land::from_shape(shape);
        let shade = |_: Vec2| 0.0;
        let place = crate::kit::ground::Place {
            land: &land,
            sites: SITES,
            water: Some(LEVEL),
            shade: &shade,
        };
        let rules = crate::kit::ground::bake(&place, &biome.ground, None);
        let height = |p: Vec2| land.height(p);
        let g = Ground {
            sites: &SITES,
            site: &site,
            height: &height,
            water: Some(LEVEL),
            rules: Some(&rules),
            keep: &kept,
            span: (LO, HI),
        };
        for (i, s) in biome.scatter.iter().enumerate() {
            let all = scatter::place(s, i, &g).unwrap();
            assert!(all.len() >= 10, "set {i} places only {}", all.len());
            for p in &all {
                let q = p.at.xz();
                assert!(
                    path(q) <= 0.02 && KNIGHT.distance(q) >= CAST,
                    "set {i} at {q}"
                );
                let over = height(q) - LEVEL;
                let (from, to) = s.over.unwrap_or((-1e3, 1e3));
                assert!(
                    (from..=to).contains(&over),
                    "set {i} at {q} stands {over} over"
                );
            }
        }
    }
}
