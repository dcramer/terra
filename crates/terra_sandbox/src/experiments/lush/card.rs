//! A leaf card in a crown and the picture it wears: drawn near as the octagon round its
//! leaves, far off as the rectangle round that octagon.

use super::Build;
use bevy::prelude::{Vec2, Vec3};
use terra_world::grown::Cards;

/// A card's picture: its cell in its atlas, how far across the cell is, and the octagon
/// round its leaves in the atlas's coordinates.
#[derive(Clone, Copy)]
pub(super) struct Leaf {
    pub(super) cell: Vec2,
    pub(super) across: f32,
    pub(super) outline: [(f32, f32); 8],
}

impl Leaf {
    /// A card of the shared atlas grown plants use (`assets/data/foliage.ron`, D222).
    pub(super) fn of(cards: &Cards, name: &str) -> Self {
        let card = cards.get(name);
        let mut outline = [(0.0, 0.0); 8];
        for (to, from) in outline.iter_mut().zip(&card.outline) {
            *to = *from;
        }
        Self {
            cell: card.cell,
            across: card.across,
            outline,
        }
    }

    /// The rectangle round its octagon, wound as the octagon is.
    fn boxed(&self) -> [(f32, f32); 4] {
        let (mut low, mut high) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
        let mut area = 0.0;
        for (i, &(u, v)) in self.outline.iter().enumerate() {
            let at = Vec2::new(u, v);
            (low, high) = (low.min(at), high.max(at));
            let (nu, nv) = self.outline[(i + 1) % self.outline.len()];
            area += u * nv - nu * v;
        }
        let rectangle = [
            (low.x, low.y),
            (high.x, low.y),
            (high.x, high.y),
            (low.x, high.y),
        ];
        if area < 0.0 {
            [rectangle[0], rectangle[3], rectangle[2], rectangle[1]]
        } else {
            rectangle
        }
    }
}

/// One leaf card in a crown: where it is, which way it faces, how big, its picture, and
/// how it is lit and moves.
#[derive(Clone, Copy)]
pub(super) struct Card {
    pub(super) middle: Vec3,
    pub(super) right: Vec3,
    pub(super) up: Vec3,
    pub(super) half: f32,
    pub(super) normal: Vec3,
    pub(super) tint: Vec3,
    pub(super) sway: Vec2,
    pub(super) flip: f32,
    /// How much of the sky it sees, least deep in the crown (D216).
    pub(super) sky: f32,
    pub(super) leaf: Leaf,
}

impl Card {
    /// Draws it, `grow` times its size; `rank`, its place and its clump's count, for a crown
    /// whose cards drop out with distance.
    pub(super) fn draw(&self, build: &mut Build, grow: f32) {
        self.draw_ranked(build, grow, None);
    }

    pub(super) fn draw_ranked(
        &self,
        build: &mut Build,
        grow: f32,
        rank: Option<(usize, usize, usize)>,
    ) {
        self.draw_outline(build, grow, rank, &self.leaf.outline);
    }

    /// Draws it far off, `grow` times its size, as the rectangle round its octagon: two
    /// triangles for six. A far card covers a few pixels, so its triangles cost more than
    /// the transparent corners the alpha test discards, and the test cuts the same leaves,
    /// as foliage levels of detail trade a cutout's vertices for fill with distance. The
    /// far crowns of pines were 8,600 triangles each and a third of the forest walk's
    /// (forest perf round 2).
    pub(super) fn draw_far(&self, build: &mut Build, grow: f32) {
        self.draw_outline(build, grow, None, &self.leaf.boxed());
    }

    fn draw_outline(
        &self,
        build: &mut Build,
        grow: f32,
        rank: Option<(usize, usize, usize)>,
        outline: &[(f32, f32)],
    ) {
        let half = self.half * grow;
        let base = build.positions.len() as u32;
        for &(u, v) in outline {
            let atlas = Vec2::new(u, v);
            let own = (atlas - self.leaf.cell) / self.leaf.across;
            let corner = self.middle
                + self.right * (own.x * 2.0 - 1.0) * half * self.flip
                + self.up * (1.0 - own.y * 2.0) * half;
            let index = build.vertex(corner, self.normal, atlas, self.sway, self.tint);
            build.colours[index as usize][3] = self.sky;
            if build.offset_in_colour {
                let offset = corner - self.middle;
                build.colours[index as usize] = [offset.x, offset.y, offset.z, 1.0];
            }
            if let Some((k, n, keep)) = rank {
                let offset = corner - self.middle;
                build.dropping.push([
                    offset.x,
                    offset.y,
                    offset.z,
                    k as f32 + n as f32 / 100.0 + keep as f32 / 1000.0,
                ]);
            }
        }
        for i in 1..outline.len() as u32 - 1 {
            build.indices.extend([base, base + i, base + i + 1]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A far card is two triangles covering every point of its leaves' octagon, facing the
    /// way the octagon's own triangles do, so the alpha test cuts the same leaves from it
    /// and back-face culling keeps it as it kept them.
    #[test]
    fn a_far_card_covers_its_octagon_in_two_triangles_facing_as_it_does() {
        // An octagon from the atlas, and the same wound the other way.
        let octagon = [
            (0.1557, 0.0337),
            (0.2026, 0.0337),
            (0.2478, 0.0789),
            (0.2478, 0.1738),
            (0.2036, 0.2180),
            (0.0303, 0.2180),
            (0.0022, 0.1900),
            (0.0022, 0.1872),
        ];
        let mut reversed = octagon;
        reversed.reverse();
        for outline in [octagon, reversed] {
            let card = Card {
                middle: Vec3::new(1.0, 2.0, 3.0),
                right: Vec3::X,
                up: Vec3::Y,
                half: 0.4,
                normal: Vec3::Z,
                tint: Vec3::ONE,
                sway: Vec2::ZERO,
                flip: 1.0,
                sky: 1.0,
                leaf: Leaf {
                    cell: Vec2::ZERO,
                    across: 0.25,
                    outline,
                },
            };
            let (mut near, mut far) = (Build::default(), Build::default());
            card.draw(&mut near, 1.0);
            card.draw_far(&mut far, 1.0);
            assert_eq!(far.indices.len(), 6, "two triangles");
            // Every triangle's facing, by its winding seen down the card's normal.
            let facing = |build: &Build| -> Vec<f32> {
                build
                    .indices
                    .chunks(3)
                    .map(|t| {
                        let [a, b, c] =
                            [t[0], t[1], t[2]].map(|i| Vec3::from(build.positions[i as usize]));
                        (b - a).cross(c - a).dot(Vec3::Z)
                    })
                    .collect()
            };
            let near_way = facing(&near)[0].signum();
            assert!(facing(&far).iter().all(|f| f.signum() == near_way));
            // Every corner of the octagon lies within the rectangle.
            let bounds = |build: &Build| {
                build
                    .positions
                    .iter()
                    .map(|p| Vec3::from(*p))
                    .fold((Vec3::MAX, Vec3::MIN), |(l, h), p| (l.min(p), h.max(p)))
            };
            let (low, high) = bounds(&far);
            for p in near.positions.iter().map(|p| Vec3::from(*p)) {
                assert!(p.cmpge(low - 1.0e-5).all() && p.cmple(high + 1.0e-5).all());
            }
            // And the rectangle is no bigger than the octagon's own bounds.
            let (near_low, near_high) = bounds(&near);
            assert!((near_low - low).length() < 1.0e-5 && (near_high - high).length() < 1.0e-5);
        }
    }
}
