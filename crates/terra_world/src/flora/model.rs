//! What each kind of plant is drawn from: its modelled meshes, read and coloured, or its
//! grown variants (`grown`), with their sizes.

use super::super::flora_kinds::{Flora, FloraDef, Sway};
use super::super::grown;
use super::super::models::{Part, read_model};
use bevy::{
    asset::RenderAssetUsages, mesh::Indices, platform::collections::HashMap, prelude::*,
    render::render_resource::PrimitiveTopology,
};
use terra_gen::Seed;

/// A model ready to place: its mesh, or what a grown one is drawn from, and its size as
/// modelled or grown.
#[derive(Clone)]
pub(super) struct Model {
    pub(super) look: Drawn,
    pub(super) height: f32,
    /// Half its width at the ground, more or less.
    pub(super) spread: f32,
}

/// How a model is drawn: one mesh in the flora material, or a grown plant's levels of
/// detail and shadow stand-ins (`grown`).
#[derive(Clone)]
pub(super) enum Drawn {
    Mesh(Handle<Mesh>),
    Grown(Box<grown::Variant>),
}

/// Every kind's models, or its grown variants, by its place in `assets/data/flora.ron`.
pub(super) fn of_kinds(
    flora: &Flora,
    seed: Seed,
    cards: &grown::Cards,
    looks: &mut grown::Looks,
    assets: &AssetServer,
    (meshes, foliage): (&mut Assets<Mesh>, &mut Assets<grown::Foliage>),
) -> Vec<Vec<Model>> {
    flora
        .kinds()
        .iter()
        .map(|kind| {
            if let Some(how) = &kind.grown {
                let grew = grown::grow(
                    how,
                    &kind.name,
                    seed.derive("grown").derive(&kind.name),
                    kind.variants,
                    kind.size.unwrap_or((1.0, 1.0)),
                    cards,
                    looks,
                    assets,
                    meshes,
                    foliage,
                );
                return grew
                    .into_iter()
                    .map(|variant| Model {
                        height: variant.height,
                        spread: variant.spread,
                        look: Drawn::Grown(Box::new(variant)),
                    })
                    .collect();
            }
            kind.models
                .iter()
                .filter_map(|name| match read_model(&format!("nature/{name}.glb")) {
                    Ok(parts) => Some(build_model(kind, &parts, meshes)),
                    Err(problem) => {
                        error!("Could not read a model of {}. {problem}", kind.name);
                        None
                    }
                })
                .collect()
        })
        .collect()
}

/// Turns a model's parts into one mesh in our colours, carrying how each vertex moves in
/// the wind.
fn build_model(kind: &FloraDef, parts: &[Part], meshes: &mut Assets<Mesh>) -> Model {
    let all = || parts.iter().flat_map(|part| part.positions.iter());
    let height = all().map(|p| p.y).fold(0.01, f32::max);
    let spread = all()
        .filter(|p| p.y < height * 0.15)
        .map(|p| p.xz().length())
        .fold(0.05, f32::max);
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut colours = Vec::new();
    let mut sways = Vec::new();
    let mut indices = Vec::new();
    for part in parts {
        let look = kind.look(&part.material);
        let foliage = look.foliage;
        let colour = look.colour.to_linear();
        let low = part.positions.iter().map(|p| p.y).fold(f32::MAX, f32::min);
        let high = part.positions.iter().map(|p| p.y).fold(f32::MIN, f32::max);
        let first = positions.len() as u32;
        // The free models are flat-shaded, every triangle its own plane. Drawn in a real
        // material they read as cut facets, so their normals are made smooth across gentle
        // curves and kept split at creases (docs/art-style.md): a crown or a bush as soft
        // lobes, a trunk round, a rock still angular at its edges.
        let crease = if foliage { 75.0 } else { 50.0 };
        let part_normals = smooth_normals(&part.positions, &part.indices, &part.normals, crease);
        // The material is told to the shader in the colour's alpha: a layer of
        // `materials/flora` in eighths, a sixteenth more for one with ore's rust in it, or
        // whole for plain colour.
        let rust = if kind.rust { 0.5 } else { 0.0 };
        let alpha = look
            .material
            .map_or(1.0, |layer| (layer as f32 + rust) / 8.0);
        for (position, normal) in part.positions.iter().zip(&part_normals) {
            let up_plant = (position.y / height).clamp(0.0, 1.0);
            // Foliage is darker underneath and inside, which gives a crown its depth.
            let up_part = ((position.y - low) / (high - low).max(1e-3)).clamp(0.0, 1.0);
            let shade = if foliage {
                0.66 + 0.42 * up_part
            } else {
                0.85 + 0.15 * up_plant
            };
            let (bend, flutter) = match kind.sway {
                Sway::Tree => (1.7 * up_plant * up_plant, if foliage { 0.7 } else { 0.0 }),
                Sway::Bush => (0.5 * up_plant, 0.5),
                Sway::Plant => (0.9 * up_plant, 0.4 * up_plant),
                Sway::Still => (0.0, 0.0),
            };
            positions.push(*position);
            normals.push(*normal);
            colours.push([
                colour.red * shade,
                colour.green * shade,
                colour.blue * shade,
                alpha,
            ]);
            sways.push([bend, flutter]);
        }
        indices.extend(part.indices.iter().map(|index| first + index));
    }
    let count = positions.len();
    let mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colours)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, sways)
    // Room for the shader to carry where each point is on the plant, unmoved by the wind,
    // which is where its material is projected from (wind.wesl).
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, vec![[0.0f32; 2]; count])
    .with_inserted_indices(Indices::U32(indices));
    Model {
        look: Drawn::Mesh(meshes.add(mesh)),
        height,
        spread,
    }
}

/// Normals smooth across a surface where its triangles meet at less than `crease_deg`,
/// and split where they meet at more, as a modeller's auto-smooth does: each corner takes
/// the area-weighted normals of the triangles at the same point that face within the
/// crease of its own.
fn smooth_normals(
    positions: &[Vec3],
    indices: &[u32],
    normals: &[Vec3],
    crease_deg: f32,
) -> Vec<Vec3> {
    let limit = libm::cosf(crease_deg.to_radians());
    // Corners at the same point, found by position rounded to a tenth of a millimetre.
    let key = |p: Vec3| (p * 1.0e4).round().as_ivec3();
    let mut faces_at: HashMap<IVec3, Vec<usize>> = HashMap::default();
    let faces: Vec<Vec3> = indices
        .chunks_exact(3)
        .map(|t| {
            let [a, b, c] = [t[0], t[1], t[2]].map(|i| positions[i as usize]);
            (b - a).cross(c - a)
        })
        .collect();
    for (face, t) in indices.chunks_exact(3).enumerate() {
        for &i in t {
            faces_at
                .entry(key(positions[i as usize]))
                .or_default()
                .push(face);
        }
    }
    let mut out = normals.to_vec();
    let mut done = vec![false; positions.len()];
    for (face, t) in indices.chunks_exact(3).enumerate() {
        let own = faces[face].normalize_or_zero();
        if own == Vec3::ZERO {
            continue;
        }
        for &i in t {
            let i = i as usize;
            if done[i] {
                continue;
            }
            done[i] = true;
            let sum: Vec3 = faces_at[&key(positions[i])]
                .iter()
                .map(|&other| faces[other])
                .filter(|n| n.normalize_or_zero().dot(own) >= limit)
                .sum();
            out[i] = sum.normalize_or(own);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two triangles, each its own copy of the shared edge, as flat-shaded models are,
    /// folded along it by `fold` degrees.
    fn hinge(fold: f32) -> (Vec<Vec3>, Vec<u32>, Vec<Vec3>) {
        let (s, c) = (libm::sinf(fold.to_radians()), libm::cosf(fold.to_radians()));
        let far = Vec3::new(0.0, s, c);
        let positions = vec![
            Vec3::ZERO,
            Vec3::X,
            Vec3::new(0.0, 0.0, -1.0),
            Vec3::ZERO,
            far,
            Vec3::X,
        ];
        let indices = vec![0, 1, 2, 3, 4, 5];
        let normals = vec![Vec3::Y; 6];
        (positions, indices, normals)
    }

    #[test]
    fn normals_are_smooth_across_a_gentle_bend_and_split_at_a_crease() {
        let (positions, indices, normals) = hinge(20.0);
        let smooth = smooth_normals(&positions, &indices, &normals, 50.0);
        // Where the two meet, both sides take the same normal, between their planes.
        assert!(smooth[0].distance(smooth[3]) < 1e-4, "{smooth:?}");
        let (positions, indices, normals) = hinge(70.0);
        let split = smooth_normals(&positions, &indices, &normals, 50.0);
        assert!(split[0].distance(split[3]) > 0.5, "{split:?}");
        // Away from the shared edge, each keeps its own plane.
        assert!(split[2].distance(Vec3::Y) < 1e-4, "{split:?}");
    }
}
