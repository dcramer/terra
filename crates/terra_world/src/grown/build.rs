//! The pieces grown plants are made of, as mesh data: tubes of bark, cards of leaves cut to
//! their outline, and rounded lumps for far crowns and shadows.

use super::Card;
use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use std::f32::consts::TAU;

/// A mesh being made: where each point is, which way it faces, where it is in its picture,
/// how it moves in the wind (`foliage.wesl`: how far it bends with the plant, how much it
/// flutters on its own) and its colour, which the picture's is multiplied by.
#[derive(Default)]
pub struct Build {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub sway: Vec<[f32; 2]>,
    pub colours: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
}

impl Build {
    pub fn vertex(&mut self, at: Vec3, normal: Vec3, uv: Vec2, sway: Vec2, colour: Vec3) -> u32 {
        self.positions.push(at.into());
        self.normals.push(normal.into());
        self.uvs.push(uv.into());
        self.sway.push(sway.into());
        self.colours.push([colour.x, colour.y, colour.z, 1.0]);
        self.positions.len() as u32 - 1
    }

    /// Says how much of the sky each point sees, 0 to 1, in its colour's alpha, which the
    /// foliage shader dims the sky's light by (D216).
    pub fn see_sky(&mut self, sky: impl Fn(Vec3) -> f32) {
        for (colour, at) in self.colours.iter_mut().zip(&self.positions) {
            colour[3] = sky(Vec3::from(*at)).clamp(0.0, 1.0);
        }
    }

    pub fn triangles(&self) -> usize {
        self.indices.len() / 3
    }

    /// The highest point, from the foot.
    pub fn height(&self) -> f32 {
        self.positions.iter().map(|p| p[1]).fold(0.01, f32::max)
    }

    /// The mesh, with tangents for a normal map where `tangents`.
    pub fn mesh(self, tangents: bool) -> Mesh {
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, self.sway)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.colours)
        .with_inserted_indices(Indices::U32(self.indices));
        if tangents && let Err(problem) = mesh.generate_tangents() {
            warn!("No tangents for a grown mesh: {problem}");
        }
        mesh
    }
}

/// A limb of a plant: a line of points, with how thick it is at each.
pub struct Limb {
    pub points: Vec<Vec3>,
    pub radii: Vec<f32>,
    pub depth: u32,
}

/// A limb as a tube with bark round it, `sides` round, its picture `bark_m` metres across
/// and as tall, so bark is the same size on every limb.
pub fn tube(build: &mut Build, limb: &Limb, sides: u32, height: f32, bark_m: f32, tint: Vec3) {
    let start = build.positions.len() as u32;
    let mut along = 0.0;
    let mut side = (limb.points[1] - limb.points[0]).any_orthonormal_vector();
    for (i, (&point, &radius)) in limb.points.iter().zip(&limb.radii).enumerate() {
        if i > 0 {
            along += point.distance(limb.points[i - 1]);
        }
        let forward = if i + 1 < limb.points.len() {
            limb.points[i + 1] - point
        } else {
            point - limb.points[i - 1]
        }
        .normalize();
        // Carry the ring's orientation along the limb, so it does not twist.
        side = (side - forward * side.dot(forward)).normalize_or(forward.any_orthonormal_vector());
        let other = forward.cross(side);
        let wraps = (TAU * radius / bark_m).round().max(1.0);
        let bend = (point.y / height).clamp(0.0, 1.0);
        let flutter = if limb.depth >= 2 { 0.15 } else { 0.0 };
        for j in 0..=sides {
            let angle = j as f32 / sides as f32 * TAU;
            let normal = side * libm::cosf(angle) + other * libm::sinf(angle);
            let uv = Vec2::new(j as f32 / sides as f32 * wraps, along / bark_m);
            build.vertex(
                point + normal * radius,
                normal,
                uv,
                Vec2::new(bend * bend, flutter),
                tint,
            );
        }
    }
    let ring = sides + 1;
    for i in 0..(limb.points.len() as u32 - 1) {
        for j in 0..sides {
            let a = start + i * ring + j;
            let b = a + ring;
            build.indices.extend([a, b, a + 1, a + 1, b, b + 1]);
        }
    }
}

/// A rough ball: rings of six round, four rings high, in a colour, its surface pushed in
/// and out a little so it reads as a mass of leaves rather than a ball. Its normals lean
/// out from `crown`, the middle of the crown it is part of, so a crown of many lumps
/// shades as one mass, as its cards do, rather than as a heap of balls.
pub fn lump(build: &mut Build, centre: Vec3, radii: Vec3, crown: Vec3, colour: Vec3) {
    let base = build.positions.len() as u32;
    let (around, rings) = (6, 4);
    // A hash of the point, from the centre, the same every time it is grown.
    let bump = |normal: Vec3| {
        let p = (normal * 3.7 + centre * 0.31).to_array();
        let h = libm::sinf(p[0] * 12.99 + p[1] * 78.23 + p[2] * 37.72) * 43_758.547;
        1.0 + 0.22 * ((h - libm::floorf(h)) - 0.5)
    };
    for ring in 0..=rings {
        let tilt = ring as f32 / rings as f32 * std::f32::consts::PI;
        for step in 0..around {
            let turn = step as f32 / around as f32 * TAU;
            let normal = Vec3::new(
                libm::sinf(tilt) * libm::cosf(turn),
                libm::cosf(tilt),
                libm::sinf(tilt) * libm::sinf(turn),
            );
            // The poles are one point each, so they do not split.
            let pushed = if ring == 0 || ring == rings {
                1.0
            } else {
                bump(normal)
            };
            let at = centre + normal * radii * pushed;
            let out = (at - crown).normalize_or(normal);
            build.vertex(
                at,
                (normal * 0.3 + out * 0.7).normalize_or(normal),
                Vec2::ZERO,
                Vec2::ZERO,
                colour,
            );
        }
    }
    for ring in 0..rings {
        for step in 0..around {
            let a = base + ring * around + step;
            let b = base + ring * around + (step + 1) % around;
            build
                .indices
                .extend([a, a + around, b, b, a + around, b + around]);
        }
    }
}

/// Where a card goes: the point of its picture that is pinned in place (`pin`, in the
/// card's own square, 0 to 1 from its top left), where that point is, and the directions in
/// the world the picture's right and down edges run, each as long as the picture.
#[derive(Clone, Copy)]
pub struct Placing {
    pub pin: Vec2,
    pub at: Vec3,
    pub right: Vec3,
    pub down: Vec3,
    pub normal: Vec3,
    pub sway: Vec2,
    pub colour: Vec3,
}

/// A card cut to its outline: six triangles round its leaves rather than a square.
pub fn card(build: &mut Build, card: &Card, place: &Placing) {
    let base = build.positions.len() as u32;
    for &(u, v) in &card.outline {
        let atlas = Vec2::new(u, v);
        let own = (atlas - card.cell) / card.across;
        let corner =
            place.at + place.right * (own.x - place.pin.x) + place.down * (own.y - place.pin.y);
        build.vertex(corner, place.normal, atlas, place.sway, place.colour);
    }
    for i in 1..card.outline.len() as u32 - 1 {
        build.indices.extend([base, base + i, base + i + 1]);
    }
}

/// A card folded along its stalk and drooping toward its tip, as a spray of leaves is held
/// near: the picture's two halves either side of its pin turned up by `fold` radians about the
/// line down the card through the pin, toward the side its normal is on, and its tip, the
/// part above the pin, bent down by `droop` of its length at the end. A flat card a stride off
/// is one plane every leaf on it lies in; SpeedTree's and Unreal's foliage fold and bend
/// their near cards so the leaves on them face apart (D359). Drawn as a grid of three by three
/// over the card's outline's bounds, eight triangles to the flat card's six.
pub fn folded_card(build: &mut Build, card: &Card, place: &Placing, fold: f32, droop: f32) {
    let own = |&(u, v): &(f32, f32)| (Vec2::new(u, v) - card.cell) / card.across;
    let (low, high) = card.outline.iter().map(own).fold(
        (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN)),
        |(low, high), p| (low.min(p), high.max(p)),
    );
    let size = place.right.length().max(1e-6);
    let across = place.right / size;
    let mut facing = place.right.cross(place.down).normalize_or(place.normal);
    if facing.dot(place.normal) < 0.0 {
        facing = -facing;
    }
    let length = place.down.length();
    let (us, vs) = (
        [low.x, place.pin.x.clamp(low.x, high.x), high.x],
        [low.y, ((low.y + high.y) * 0.5).min(place.pin.y), high.y],
    );
    let base = build.positions.len() as u32;
    for &v in &vs {
        for &u in &us {
            let x = (u - place.pin.x) * size;
            let y = v - place.pin.y;
            // Toward the tip, above the pin, the bend grows with the square of the way out.
            let out = (-y / place.pin.y.max(1e-3)).max(0.0);
            let at = place.at
                + across * x * libm::cosf(fold)
                + facing * x.abs() * libm::sinf(fold)
                + place.down * y
                - Vec3::Y * droop * length * out * out;
            let uv = card.cell + Vec2::new(u, v) * card.across;
            build.vertex(at, place.normal, uv, place.sway, place.colour);
        }
    }
    for row in 0..2u32 {
        for col in 0..2u32 {
            let a = base + row * 3 + col;
            build.indices.extend([a, a + 3, a + 1, a + 1, a + 3, a + 4]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square() -> Card {
        Card {
            cell: Vec2::ZERO,
            across: 0.25,
            outline: vec![(0.0, 0.0), (0.25, 0.0), (0.25, 0.25), (0.0, 0.25)],
            colour: Vec3::ONE,
            leaf: None,
        }
    }

    fn place() -> Placing {
        Placing {
            pin: Vec2::new(0.5, 0.75),
            at: Vec3::ZERO,
            right: Vec3::X * 0.4,
            down: Vec3::Z * 0.4,
            normal: Vec3::Y,
            sway: Vec2::ZERO,
            colour: Vec3::ONE,
        }
    }

    fn faces(build: &Build) -> Vec<Vec3> {
        build
            .indices
            .chunks(3)
            .map(|t| {
                let p = |i: u32| Vec3::from(build.positions[i as usize]);
                (p(t[1]) - p(t[0])).cross(p(t[2]) - p(t[0])).normalize()
            })
            .collect()
    }

    /// Folded, a card's two halves face apart by twice the fold, each turned up toward the
    /// side its normal is on; unfolded it is the flat card's plane.
    #[test]
    fn a_folded_cards_halves_face_apart() {
        let mut flat = Build::default();
        folded_card(&mut flat, &square(), &place(), 0.0, 0.0);
        assert!(
            faces(&flat).iter().all(|f| f.y.abs() > 0.999),
            "a flat card is not flat"
        );
        let mut build = Build::default();
        folded_card(&mut build, &square(), &place(), 0.4, 0.0);
        let faces = faces(&build);
        let up = |f: &Vec3| if f.y < 0.0 { -*f } else { *f };
        let left = faces.iter().map(up).filter(|f| f.x > 0.01).count();
        let right = faces.iter().map(up).filter(|f| f.x < -0.01).count();
        assert!(left > 0 && right > 0, "{faces:?}");
        for f in faces.iter().map(up) {
            assert!((f.angle_between(Vec3::Y) - 0.4).abs() < 1e-3, "{f}");
        }
        // Turned up: the edges stand above the stalk.
        let edge = build
            .positions
            .iter()
            .map(|p| p[1])
            .fold(f32::MIN, f32::max);
        assert!(edge > 0.05, "edges at {edge}");
    }

    /// A drooping card's tip hangs below its pin, and the part behind the pin does not.
    #[test]
    fn a_drooping_cards_tip_hangs() {
        let mut build = Build::default();
        folded_card(&mut build, &square(), &place(), 0.0, 0.2);
        // The tip is the picture's top, v = 0, which lies toward -Z of the pin.
        let tip = build.positions.iter().filter(|p| p[2] < -0.2).map(|p| p[1]);
        let back = build.positions.iter().filter(|p| p[2] > 0.0).map(|p| p[1]);
        assert!(tip.fold(f32::MAX, f32::min) < -0.05);
        assert!(back.fold(f32::MIN, f32::max).abs() < 1e-5);
    }
}
