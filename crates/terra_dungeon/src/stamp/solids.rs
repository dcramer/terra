//! What of a theme's pieces a walker cannot pass (D53, `Theme::solids`): each piece's boxes,
//! placed as the piece stands, for the game to stop a body at as it does a chest's
//! (`terra_walker::bodies::Solid`). A mine's timber posts stand where no cube does (the cube
//! sets give way to them, D559), and a pier is wider than the column of cubes under it.

use super::Stamp;
use crate::theme::Theme;
use glam::Vec3;

/// A box a walker cannot pass, in the dungeon's frame: the middle of its foot, the side its
/// front faces (its piece's `facing`), and its half size along its own axes, in metres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Blocking {
    pub foot: Vec3,
    pub facing: usize,
    pub half: Vec3,
}

/// The boxes of every piece the theme names a solid for, each placed as its piece stands:
/// scaled along the piece's axes, then turned to its facing.
pub fn blocks(stamp: &Stamp, theme: &Theme) -> Vec<Blocking> {
    (stamp.pieces.iter())
        .flat_map(|piece| {
            let boxes = theme.solids.get(&piece.model).into_iter().flatten();
            boxes.map(move |block| Blocking {
                foot: piece.at + turned(Vec3::from(block.foot) * piece.scale, piece.facing),
                facing: piece.facing,
                half: Vec3::from(block.half) * piece.scale,
            })
        })
        .collect()
}

/// A point of a piece's frame turned as the piece is to face `facing` (0..4 = north, east,
/// south, west; unturned, its front +Z faces south): a quarter turn about Y, +Z to +X, for
/// each side round from south.
fn turned(p: Vec3, facing: usize) -> Vec3 {
    let mut p = p;
    for _ in 0..(6 - facing % 4) % 4 {
        p = Vec3::new(p.z, p.y, -p.x);
    }
    p
}

#[cfg(test)]
mod tests {
    use super::super::tests::kind;
    use super::super::{CUBE, stamp};
    use super::*;
    use crate::theme::assets;
    use crate::{ModuleSet, generate};
    use terra_gen::Seed;

    /// Whether a point is inside a box, in plan and up from its foot.
    fn inside(b: &Blocking, p: Vec3) -> bool {
        let local = turned(p - b.foot, (6 - b.facing % 4) % 4 + 2);
        local.x.abs() < b.half.x
            && local.z.abs() < b.half.z
            && (0.0..2.0 * b.half.y).contains(&local.y)
    }

    #[test]
    fn a_turn_takes_the_front_to_its_side() {
        let front = Vec3::Z;
        let sides = [Vec3::NEG_Z, Vec3::X, Vec3::Z, Vec3::NEG_X];
        for (facing, side) in sides.into_iter().enumerate() {
            assert!(
                turned(front, facing).distance(side) < 1e-6,
                "facing {facing}"
            );
        }
        // And back: the inverse turn the test's `inside` uses.
        for facing in 0..4 {
            let p = Vec3::new(0.3, 1.0, -0.7);
            let back = turned(turned(p, facing), (6 - facing % 4) % 4 + 2);
            assert!(back.distance(p) < 1e-6, "facing {facing}");
        }
    }

    /// A timber set's posts stop a walker a hand in from the drift's walls, either side,
    /// from the floor to head height, and the drift's middle is clear between them; the
    /// tour never walks into any piece's box.
    #[test]
    fn a_mines_posts_stand_in_the_way_and_the_tour_is_clear() {
        for theme in Theme::all(&assets()).unwrap() {
            let set = ModuleSet::new(&theme).unwrap();
            let mut posts = 0;
            for s in 0..12 {
                let Ok(dungeon) = generate(&theme, &set, Seed(s)) else {
                    continue;
                };
                let laid = stamp(&dungeon, &theme, &kind);
                let boxes = blocks(&laid, &theme);
                let blocked = |p: Vec3| boxes.iter().any(|b| inside(b, p));
                for p in &laid.route {
                    for up in [0.1, 1.0, 1.7] {
                        let at = *p + Vec3::Y * up;
                        assert!(
                            !blocked(at),
                            "{} seed {s}: the tour walks into a piece at {at}",
                            theme.name
                        );
                    }
                }
                let Some(timber) = &theme.timber else {
                    continue;
                };
                for piece in laid.pieces.iter().filter(|p| p.model == timber.set) {
                    // Across the drift: the set's X, its walls 1.5 m out (6 cubes clear).
                    let across = turned(Vec3::X, piece.facing);
                    let wall = 3.0 * CUBE;
                    for up in [0.05, 1.0, 1.8] {
                        let at = piece.at + Vec3::Y * up;
                        for side in [-1.0, 1.0] {
                            let post = at + across * side * (wall - 0.23);
                            assert!(blocked(post), "{} seed {s}: no post at {post}", theme.name);
                        }
                        assert!(
                            !blocked(at),
                            "{} seed {s}: the drift's middle blocked",
                            theme.name
                        );
                        assert!(
                            !blocked(at + across * 0.9),
                            "{} seed {s}: the way blocked",
                            theme.name
                        );
                    }
                    posts += 2;
                }
            }
            if theme.timber.is_some() {
                assert!(posts > 0, "{}: no sets", theme.name);
            }
        }
    }
}
