//! Debris drawn (D193, `loss`): a pile of what collapsed, in the art style (D14), from the
//! kinds that fell into it and their own materials, so a pile of logs is logs and a pile
//! of stone is that stone, with no art made for it. Every pattern breaks its own way:
//!
//! - **Coursed** (logs, hewn timber): lengths of its members, drawn as the kit draws a
//!   course's log, fallen across one another, and splinters of them.
//! - **Block** (stone, brick, earth): rubble, rough lumps wearing the wall's own texture.
//! - **Frame** (wattle and daub): lumps of its infill and lengths of its squared frame.
//! - **Roof** (shakes, slate, thatch): slabs of its covering and a rafter or two.
//! - **Doors and windows**: boards.
//!
//! A pile spreads and heaps with what fell into it, and is the same on every machine: its
//! pieces and where they lie follow from where it lies.

use crate::kit::{Kit, Kits, Pattern};
use crate::layout::{Piece, Place, Random, Split};
use crate::loss::Pile;
use crate::{CELL_M, Kind, Material, Surface, Surfaces};
use glam::{IVec3, Quat, Vec3};
use std::collections::BTreeMap;

/// The most lumps, and logs, one kind lays in a pile, however much fell into it.
const MOST_LUMPS: u32 = 40;
const MOST_LOGS: u32 = 5;

/// What a pile is drawn as, in the base's own space, by the kind and material each surface
/// is drawn in, as a section's are; doors' boards in [`Kind::PLAIN`]'s.
pub fn draw(at: IVec3, pile: &Pile, kits: &Kits) -> BTreeMap<(Kind, Material), Surface> {
    let mut out: BTreeMap<(Kind, Material), Surface> = BTreeMap::new();
    let seed = crate::layout::hash(&[at.x as u32, at.y as u32, at.z as u32, 0xdeb]);
    let mut random = Random(seed);
    let floor = pile.floor_cm as f32 / 100.0;
    let middle = Vec3::new(
        (at.x as f32 + 0.5) * CELL_M,
        floor,
        (at.z as f32 + 0.5) * CELL_M,
    );
    let cubes = pile.cubes() + pile.props;
    // How far it spreads and how high it heaps, with how much fell into it.
    let spread = 0.4 + 0.1 * (cubes as f32).sqrt();
    let heap = (0.12 + 0.05 * cubes as f32).min(0.7);
    let place = |random: &mut Random| -> Vec3 {
        let angle = random.unit() * std::f32::consts::TAU;
        let out = spread * random.unit().sqrt();
        let (sin, cos) = (libm::sinf(angle), libm::cosf(angle));
        let rise = heap * (1.0 - out / spread);
        middle + Vec3::new(cos * out, rise * random.unit(), sin * out)
    };
    for &(kind, count) in &pile.kinds {
        let Some(kit) = kits.of(kind) else {
            continue;
        };
        let mut surfaces = Surfaces::sharing();
        match kit.pattern {
            Pattern::Logs => {
                for _ in 0..count.div_ceil(2).clamp(1, MOST_LOGS) {
                    fallen_log(kit, place(&mut random), &mut random, &mut surfaces);
                }
                let splinters = (count * 2).min(MOST_LUMPS / 2);
                for _ in 0..splinters {
                    let at = place(&mut random);
                    let size = Vec3::new(0.35, 0.06, 0.08) * (0.6 + 0.8 * random.unit());
                    lump(
                        kit,
                        Material::Log,
                        at,
                        size,
                        0.15,
                        &mut random,
                        &mut surfaces,
                    );
                }
            }
            Pattern::Masonry => {
                let framed = kit.framing.is_some();
                for _ in 0..(4 + count * 5).min(MOST_LUMPS) {
                    let at = place(&mut random);
                    let size = Vec3::new(0.24, 0.16, 0.2) * (0.55 + 0.7 * random.unit());
                    lump(
                        kit,
                        Material::Masonry,
                        at,
                        size,
                        0.4,
                        &mut random,
                        &mut surfaces,
                    );
                }
                if framed {
                    for _ in 0..count.div_ceil(2).clamp(1, MOST_LOGS) {
                        let at = place(&mut random);
                        let size = Vec3::new(0.9 + 0.6 * random.unit(), 0.14, 0.14);
                        lump(
                            kit,
                            Material::Hewn,
                            at,
                            size,
                            0.04,
                            &mut random,
                            &mut surfaces,
                        );
                    }
                }
            }
            Pattern::Roof => {
                for _ in 0..(3 + count * 4).min(MOST_LUMPS) {
                    let at = place(&mut random);
                    let size = Vec3::new(0.32, 0.035, 0.22) * (0.7 + 0.6 * random.unit());
                    let material = if kit.textures.contains_key(&Material::Covering) {
                        Material::Covering
                    } else {
                        Material::Board
                    };
                    lump(kit, material, at, size, 0.1, &mut random, &mut surfaces);
                }
                for _ in 0..count.div_ceil(3).min(2) {
                    let at = place(&mut random);
                    let size = Vec3::new(1.2, 0.09, 0.07);
                    lump(
                        kit,
                        Material::Log,
                        at,
                        size,
                        0.04,
                        &mut random,
                        &mut surfaces,
                    );
                }
            }
        }
        for (material, surface) in surfaces.0 {
            out.entry((kind, material)).or_default().append(surface);
        }
    }
    // Doors and windows broken into boards, in the first kit's.
    if pile.props > 0
        && let Some(kit) = kits.0.first()
    {
        let mut surfaces = Surfaces::sharing();
        for _ in 0..(pile.props * 4).min(MOST_LUMPS) {
            let at = place(&mut random);
            let size = Vec3::new(0.7 + 0.5 * random.unit(), 0.03, 0.14);
            lump(
                kit,
                Material::Board,
                at,
                size,
                0.04,
                &mut random,
                &mut surfaces,
            );
        }
        for (material, surface) in surfaces.0 {
            out.entry((Kind::PLAIN, material))
                .or_default()
                .append(surface);
        }
    }
    out
}

/// A length of a coursed kind's member lying where it fell, drawn as the kit draws a log.
fn fallen_log(kit: &Kit, at: Vec3, random: &mut Random, surfaces: &mut Surfaces) {
    let length = 0.8 + 0.9 * random.unit();
    let angle = random.unit() * std::f32::consts::TAU;
    let along = Vec3::new(libm::cosf(angle), 0.18 * random.signed(), libm::sinf(angle));
    let lift = Vec3::Y * kit.diameter * 0.5;
    let piece = Piece::Log {
        from: Place::near(at + lift - along * length * 0.5),
        to: Place::near(at + lift + along * length * 0.5),
        split: Split::Whole,
        seed: crate::layout::hash(&[(random.unit() * 65535.0) as u32]),
        chinking: None,
        lean: [0, 0],
        lift: [0, 0],
        role: None,
        outside: None,
    };
    crate::draw::draw_as(&piece, kit, None, false, surfaces);
}

/// A rough lump of a material, about so big, tumbled, its corners pushed in and out as a
/// broken stone's or a split timber's are, rounded by `round`; flat faced, wearing its
/// material's texture.
fn lump(
    kit: &Kit,
    material: Material,
    at: Vec3,
    size: Vec3,
    round: f32,
    random: &mut Random,
    surfaces: &mut Surfaces,
) {
    let turn = Quat::from_rotation_y(random.unit() * std::f32::consts::TAU)
        * Quat::from_rotation_x(0.5 * random.signed())
        * Quat::from_rotation_z(0.5 * random.signed());
    let metres = kit
        .textures
        .get(&material)
        .map_or([1.0, 1.0], |texture| texture.metres);
    let tint = if material == Material::Masonry {
        kit.tint
    } else {
        [1.0; 3]
    };
    let shade = 0.78 + 0.2 * random.unit();
    let colour = tint.map(|channel| channel * shade);
    let jitter = random.unit().to_bits();
    // A box's faces, each a grid of two by two, pulled toward a ball and pushed about.
    let corner = |p: Vec3| -> Vec3 {
        let key = (p * 2.0).round().as_ivec3();
        let push = crate::layout::hash(&[key.x as u32, key.y as u32, key.z as u32, jitter]);
        let push = 0.82 + 0.3 * (push & 0xFFFF) as f32 / 65535.0;
        let ball = p.normalize_or(Vec3::Y) * 1.15;
        let shaped = p.lerp(ball, round) * push;
        at + turn * (shaped * size * 0.5)
    };
    let surface = surfaces.of(material);
    for axis in [
        Vec3::X,
        Vec3::NEG_X,
        Vec3::Y,
        Vec3::NEG_Y,
        Vec3::Z,
        Vec3::NEG_Z,
    ] {
        let (u, v) = (
            axis.any_orthonormal_vector(),
            axis.cross(axis.any_orthonormal_vector()),
        );
        let point = |i: i32, j: i32| corner(axis + u * i as f32 + v * j as f32);
        for (i, j) in [(-1, -1), (0, -1), (-1, 0), (0, 0)] {
            let quad = [
                point(i, j),
                point(i + 1, j),
                point(i + 1, j + 1),
                point(i, j + 1),
            ];
            for [a, b, c] in [[quad[0], quad[1], quad[2]], [quad[0], quad[2], quad[3]]] {
                let mut normal = (b - a).cross(c - a).normalize_or(axis);
                let mut corners = [a, b, c];
                // Every face outward, whichever way round its corners came.
                if normal.dot((a + b + c) / 3.0 - at) < 0.0 {
                    normal = -normal;
                    corners = [a, c, b];
                }
                let uvs = corners.map(|p| crate::mesh::planar(p, normal, metres));
                surface.textured(corners, [normal; 3], uvs, colour);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kits() -> Kits {
        let logs = Kit::default();
        let stone = Kit {
            id: 2,
            pattern: Pattern::Masonry,
            ..Kit::default()
        };
        Kits(vec![logs, stone])
    }

    #[test]
    fn a_pile_is_drawn_from_what_fell_into_it_and_the_same_every_time() {
        let kits = kits();
        let pile = Pile {
            kinds: vec![(Kind(1), 3), (Kind(2), 4)],
            floor_cm: 20,
            ..Pile::default()
        };
        let at = IVec3::new(3, 0, 5);
        let drawn = draw(at, &pile, &kits);
        // Logs as logs, stone as its own surface's material.
        assert!(drawn.contains_key(&(Kind(1), Material::Log)));
        assert!(drawn.contains_key(&(Kind(2), Material::Masonry)));
        assert_eq!(drawn, draw(at, &pile, &kits));
        // Lying on what it fell onto, and about its cube.
        for surface in drawn.values() {
            for point in surface.indexed().positions {
                assert!(point.y > 0.0 && point.y < 1.6, "{point}");
                assert!((point.x - 1.75).abs() < 1.5 && (point.z - 2.75).abs() < 1.5);
            }
        }
        // More fallen, more drawn.
        let more = Pile {
            kinds: vec![(Kind(2), 12)],
            ..pile.clone()
        };
        let count = |drawn: &BTreeMap<(Kind, Material), Surface>| -> usize {
            drawn.values().map(Surface::triangle_count).sum()
        };
        let less = Pile {
            kinds: vec![(Kind(2), 1)],
            ..pile
        };
        assert!(count(&draw(at, &more, &kits)) > count(&draw(at, &less, &kits)));
    }
}
