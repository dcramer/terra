//! Decks on posts and the stairs up to them, laid in the shipped kits (D306,
//! docs/building-kits.md): framed as a carpenter frames one, so its geometry is checked here
//! without a picture. Every post stands on a footing and ends under a bearer, inside the
//! deck's footprint; the joists sit on the bearers; nothing of the frame stands above the
//! deck; and a flight's stringers bear on the ground at their foot and on the deck's rim at
//! their head. Each case is a deck as a player lays one: on four, six and nine posts, an L,
//! against a wall, a storey and two up, over ground falling away, with stairs up each side,
//! in logs and hewn timber and on stone piers.

use glam::{IVec3, Vec3};
use std::collections::BTreeSet;
use terra_build::kit::{Kit, Kits};
use terra_build::layout::{self, Piece, Role};
use terra_build::{Base, BuildOp, CELL_M, Cube, Facing, Fill, Kind, Shape};

const LOGS: Kind = Kind(1);
const STONE: Kind = Kind(2);
const HEWN: Kind = Kind(3);

fn kits() -> Kits {
    let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    Kits(
        Kit::read_folder(&assets)
            .unwrap()
            .into_iter()
            .map(|(_, kit)| kit)
            .collect(),
    )
}

fn put(base: &mut Base, min: [i32; 3], max: [i32; 3], kind: Kind, shape: Shape) {
    base.apply(BuildOp::Fill {
        min,
        max,
        cube: Some(Cube {
            shape,
            ..Cube::new(kind, Fill::Solid)
        }),
    });
}

/// A deck as the hammer lays one, on a layer, over a footprint of columns, with a post of
/// `posts` kind at each of `at` from its foot (a footing of stone under it, down to the
/// layer under the ground) up to the deck's layer, as the hammer's post a storey high is,
/// or to the layer under it, as one laid by hand under a deck is.
struct Scene {
    base: Base,
    deck: Kind,
    layer: i32,
    footprint: BTreeSet<[i32; 2]>,
}

impl Scene {
    fn new(deck: Kind, layer: i32, footprint: impl IntoIterator<Item = [i32; 2]>) -> Scene {
        Scene {
            base: Base::default(),
            deck,
            layer,
            footprint: footprint.into_iter().collect(),
        }
    }

    fn rect(deck: Kind, layer: i32, min: [i32; 2], max: [i32; 2]) -> Scene {
        Scene::new(
            deck,
            layer,
            (min[0]..=max[0]).flat_map(|x| (min[1]..=max[1]).map(move |z| [x, z])),
        )
    }

    /// Posts, each from its foot on a footing, to the deck's layer or under it.
    fn posts(mut self, kind: Kind, at: &[[i32; 3]], into: bool) -> Scene {
        for &[x, foot, z] in at {
            put(
                &mut self.base,
                [x, -3, z],
                [x, foot - 1, z],
                STONE,
                Shape::Whole,
            );
            let top = if into { self.layer } else { self.layer - 1 };
            put(&mut self.base, [x, foot, z], [x, top, z], kind, Shape::Post);
        }
        self
    }

    /// The deck over its footprint, where no post stands in its layer already.
    fn laid(mut self) -> Scene {
        for &[x, z] in &self.footprint.clone() {
            let at = [x, self.layer, z];
            if self.base.cube(IVec3::from_array(at)).is_none() {
                put(&mut self.base, at, at, self.deck, Shape::Whole);
            }
        }
        self
    }

    /// A flight a cube wide, `wide` columns across from `foot`, rising toward `facing` from
    /// the layer on the footing up to the deck's, as `plans::flight` lays one.
    fn flight(mut self, foot: [i32; 2], facing: Facing, wide: i32) -> Scene {
        let (forward, across) = (facing.step(), facing.turned(1).step());
        let rise = self.layer + 1;
        for i in 0..rise {
            for w in 0..wide {
                let at = IVec3::new(foot[0], 0, foot[1]) + forward * i + across * w;
                put(
                    &mut self.base,
                    [at.x, -3, at.z],
                    [at.x, -1, at.z],
                    STONE,
                    Shape::Whole,
                );
                if i > 0 {
                    put(
                        &mut self.base,
                        [at.x, 0, at.z],
                        [at.x, i - 1, at.z],
                        self.deck,
                        Shape::Whole,
                    );
                }
                put(
                    &mut self.base,
                    [at.x, i, at.z],
                    [at.x, i, at.z],
                    self.deck,
                    Shape::Stair(facing),
                );
            }
        }
        self
    }
}

/// Every piece of a base, with the kit it is of.
fn laid(base: &Base, kits: &Kits) -> Vec<(Kind, Piece)> {
    kits.0
        .iter()
        .flat_map(|kit| {
            layout::lay_among(base, kit, kits)
                .pieces
                .into_iter()
                .map(move |piece| (kit.kind(), piece))
        })
        .collect()
}

fn ends(piece: &Piece) -> Option<(Vec3, Vec3)> {
    match *piece {
        Piece::Log { from, to, .. } | Piece::Plank { from, to, .. } => {
            Some((from.metres(), to.metres()))
        }
        _ => None,
    }
}

fn role(piece: &Piece) -> Option<Role> {
    match *piece {
        Piece::Log { role, .. } => role,
        _ => None,
    }
}

/// How far a point is from a segment, level.
fn level_distance(point: Vec3, (a, b): (Vec3, Vec3)) -> f32 {
    let (p, a, b) = (point.with_y(0.0), a.with_y(0.0), b.with_y(0.0));
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared().max(1e-9)).clamp(0.0, 1.0);
    (a + ab * t).distance(p)
}

/// Checks a scene's deck and flights against how a deck is framed, and says what it found.
fn check(name: &str, scene: &Scene, kits: &Kits) {
    let base = &scene.base;
    let pieces = laid(base, kits);
    let deck_kit = kits.of(scene.deck).unwrap();
    let foot = scene.layer as f32 * CELL_M;
    let top = foot + CELL_M;
    let (_, bearer_high) = terra_build::timber::section(deck_kit, Some(Role::Crest));
    let bearers: Vec<(Vec3, Vec3)> = pieces
        .iter()
        .filter(|(kind, piece)| *kind == scene.deck && role(piece) == Some(Role::Crest))
        .filter_map(|(_, piece)| ends(piece))
        .filter(|(a, b)| (a.y - b.y).abs() < 1e-3 && (a.y - foot - bearer_high * 0.5).abs() < 1e-3)
        .collect();
    let posts: Vec<(Kind, Vec3, Vec3)> = pieces
        .iter()
        .filter(|(_, piece)| role(piece) == Some(Role::Post))
        .filter_map(|(kind, piece)| ends(piece).map(|(a, b)| (*kind, a.min(b), a.max(b))))
        .collect();
    // The posts' columns, of any kind: every one under the deck carries a bearer.
    let columns: BTreeSet<[i32; 2]> = base
        .cubes()
        .filter(|(_, cube)| cube.shape == Shape::Post)
        .map(|(at, _)| [at.x, at.z])
        .collect();
    assert!(!columns.is_empty(), "{name}: no posts");
    for &[x, z] in &columns {
        let middle = Vec3::new((x as f32 + 0.5) * CELL_M, 0.0, (z as f32 + 0.5) * CELL_M);
        // Its footprint inside the deck's.
        assert!(
            scene.footprint.contains(&[x, z]),
            "{name}: the post at {x},{z} is outside the deck"
        );
        // A bearer over it, its underside on the deck's layer's foot.
        assert!(
            bearers
                .iter()
                .any(|&line| level_distance(middle, line) < 0.01),
            "{name}: no bearer over the post at {x},{z}: {bearers:?}"
        );
        // Its top at the bearer's underside, whatever its kind: a timber post's shaft, or a
        // stone pillar's drums up to the layer under the deck, none in it.
        let top_cube = (-3..=scene.layer)
            .rev()
            .find(|&y| {
                base.cube(IVec3::new(x, y, z))
                    .is_some_and(|cube| cube.shape == Shape::Post)
            })
            .unwrap();
        let kind = base.cube(IVec3::new(x, top_cube, z)).unwrap().kind;
        if kind == STONE {
            let drums: Vec<i32> = pieces
                .iter()
                .filter_map(|(_, piece)| match piece {
                    Piece::Block { cube } if cube.x == x && cube.z == z => Some(cube.y),
                    _ => None,
                })
                .filter(|&y| y >= 0)
                .collect();
            let highest = drums.iter().copied().max().unwrap();
            assert_eq!(
                highest,
                scene.layer - 1,
                "{name}: the pillar at {x},{z} rises to {highest}"
            );
        } else {
            let (_, low, high) = posts
                .iter()
                .find(|(_, low, _)| level_distance(*low, (middle, middle)) < 0.01)
                .unwrap_or_else(|| panic!("{name}: no post drawn at {x},{z}"));
            assert!(
                (high.y - foot).abs() < 0.005,
                "{name}: the post at {x},{z} ends at {}, not under the bearer at {foot}",
                high.y
            );
            // On its footing: the cube under its foot is solid, its pad between.
            let foot_cube = (low.y / CELL_M + 1e-3).floor() as i32;
            assert!(
                low.y - foot_cube as f32 * CELL_M < 0.07
                    && base
                        .cube(IVec3::new(x, foot_cube - 1, z))
                        .is_some_and(|cube| cube.fill == Fill::Solid),
                "{name}: the post at {x},{z} stands at {} on nothing",
                low.y
            );
            // Within the deck's footprint, all round.
            let (wide, _) = terra_build::timber::section(kits.of(kind).unwrap(), Some(Role::Post));
            for corner in [Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z] {
                let p = middle + corner * wide * 0.5;
                let column = [(p.x / CELL_M).floor() as i32, (p.z / CELL_M).floor() as i32];
                assert!(
                    scene.footprint.contains(&column),
                    "{name}: the post at {x},{z} reaches out of the deck"
                );
            }
        }
    }
    // Nothing of the deck's frame above its top, nor any post up through it.
    for (kind, piece) in &pieces {
        let Some((a, b)) = ends(piece) else { continue };
        let column = [
            ((a.x + b.x) * 0.5 / CELL_M).floor() as i32,
            ((a.z + b.z) * 0.5 / CELL_M).floor() as i32,
        ];
        if !scene.footprint.contains(&column) || a.y.max(b.y) < foot + 0.01 {
            continue;
        }
        if role(piece) == Some(Role::Post) {
            panic!("{name}: a post of {kind:?} stands up into the deck: {a} {b}");
        }
    }
    // The joists sit on the bearers: every plank crossing a bearer, low in the deck, rests
    // on its top.
    let joists: Vec<(Vec3, Vec3, f32)> = pieces
        .iter()
        .filter(|(kind, _)| *kind == scene.deck)
        .filter_map(|(_, piece)| match *piece {
            Piece::Plank { from, to, high, .. } => {
                Some((from.metres(), to.metres(), layout::metres(high)))
            }
            _ => None,
        })
        .filter(|(a, b, high)| {
            (a.y - b.y).abs() < 1e-3 && a.y - high * 0.5 > foot - 0.01 && a.y < top - 0.05
        })
        .collect();
    for &(a, b) in &bearers {
        let along = (b - a).normalize();
        let across = Vec3::Y.cross(along);
        let crossing: Vec<_> = joists
            .iter()
            .filter(|(c, d, _)| (*d - *c).normalize().dot(along).abs() < 0.1)
            .filter(|(c, d, _)| {
                // Over the bearer, level: within its length, and across its line.
                let at = (*c - a).dot(along);
                let (from, to) = ((*c - a).dot(across), (*d - a).dot(across));
                at > 0.0 && at < (b - a).length() && from.min(to) < 0.0 && from.max(to) > 0.0
            })
            .collect();
        assert!(
            !crossing.is_empty(),
            "{name}: no joist over the bearer {a} {b}"
        );
        for (c, _, high) in crossing {
            let under = c.y - high * 0.5;
            assert!(
                (under - (foot + bearer_high)).abs() < 0.005,
                "{name}: a joist's underside at {under} is not on the bearer's top at {}",
                foot + bearer_high
            );
        }
    }
    // Each flight's stringers: seated on the floor at their foot, and at their head bearing
    // on the deck's rim at the face of the top step's back.
    let stringers: Vec<(Vec3, Vec3, f32)> = pieces
        .iter()
        .filter_map(|(_, piece)| match *piece {
            Piece::Plank { from, to, high, .. } if (from.0[1] - to.0[1]).abs() > 16 => {
                let (a, b) = (from.metres(), to.metres());
                Some(if a.y < b.y {
                    (a, b, layout::metres(high))
                } else {
                    (b, a, layout::metres(high))
                })
            }
            _ => None,
        })
        .collect();
    for &(low, high, deep) in &stringers {
        let axis = (high - low).normalize();
        let normal = (Vec3::Y - axis * axis.y).normalize();
        let (seat, head) = (low - normal * deep * 0.5, high - normal * deep * 0.5);
        assert!(
            seat.y.abs() < 0.01,
            "{name}: a stringer's foot is at {}, not on the floor",
            seat.y
        );
        // The deck's face it bears on: the side of the footprint nearest the head.
        let rim = joists_and_rims(&pieces, scene.deck, foot)
            .into_iter()
            .any(|(c, d, h)| {
                let face = level_distance(head, (c, d));
                face < JOIST_HALF + 0.01
                    && head.y > c.y - h * 0.5 - 0.01
                    && head.y < c.y + h * 0.5 + 0.01
            });
        assert!(rim, "{name}: a stringer's head at {head} bears on no rim");
    }
    // Posts under a flight carry a stringer: each top meets the underside of one over it;
    // a flight a storey high stands on one under each stringer, halfway up.
    let flight_posts = pieces
        .iter()
        .filter(|(_, piece)| role(piece) == Some(Role::Jamb))
        .count();
    assert!(
        flight_posts >= stringers.len(),
        "{name}: {flight_posts} posts under {} stringers",
        stringers.len()
    );
    for (_, piece) in &pieces {
        if role(piece) != Some(Role::Jamb) {
            continue;
        }
        let (a, b) = ends(piece).unwrap();
        let (low, high) = (a.min(b), a.max(b));
        let carried = stringers.iter().any(|&(c, d, deep)| {
            let axis = (d - c).normalize();
            let normal = (Vec3::Y - axis * axis.y).normalize();
            let t =
                (high - c).with_y(0.0).dot(axis.with_y(0.0)) / axis.with_y(0.0).length_squared();
            let centre = c + axis * t;
            let under = centre.y - deep * 0.5 / normal.y;
            level_distance(high, (c, d)) < 0.01 && (high.y - under).abs() < 0.1
        });
        assert!(
            carried,
            "{name}: a post under a flight at {low}..{high} carries no stringer"
        );
        assert!(
            low.y.abs() < 0.01,
            "{name}: a post under a flight stands at {}",
            low.y
        );
    }
}

/// Half a joist's thickness: a stringer's head touches a rim joist's face.
const JOIST_HALF: f32 = 0.035;

/// The deck's planks laid level and deep, its joists and rims.
fn joists_and_rims(pieces: &[(Kind, Piece)], deck: Kind, foot: f32) -> Vec<(Vec3, Vec3, f32)> {
    pieces
        .iter()
        .filter(|(kind, _)| *kind == deck)
        .filter_map(|(_, piece)| match *piece {
            Piece::Plank { from, to, high, .. }
                if from.0[1] == to.0[1] && layout::metres(high) > 0.09 =>
            {
                Some((from.metres(), to.metres(), layout::metres(high)))
            }
            _ => None,
        })
        .filter(|(a, _, _)| a.y > foot)
        .collect()
}

#[test]
fn a_deck_stands_on_its_posts_through_bearers_and_joists_in_every_kit() {
    let kits = kits();
    let corners = |x: [i32; 2], z: [i32; 2], foot: i32| {
        [
            [x[0], foot, z[0]],
            [x[1], foot, z[0]],
            [x[0], foot, z[1]],
            [x[1], foot, z[1]],
        ]
    };
    for deck in [LOGS, HEWN] {
        for (posts, into) in [(deck, true), (deck, false), (STONE, false), (STONE, true)] {
            let label = |what: &str| {
                format!(
                    "{what} in {deck:?} on {posts:?}{}",
                    if into { ", posts into its layer" } else { "" }
                )
            };
            // Four posts a storey up, a flight up its south side.
            let four = Scene::rect(deck, 5, [0, 0], [5, 4])
                .posts(posts, &corners([0, 5], [0, 4], 0), into)
                .laid()
                .flight([1, 10], Facing::North, 2);
            check(&label("four posts"), &four, &kits);
            // Six: three along each long side.
            let mut six_at = corners([0, 5], [0, 9], 0).to_vec();
            six_at.extend([[0, 0, 5], [5, 0, 5]]);
            let six = Scene::rect(deck, 5, [0, 0], [5, 9])
                .posts(posts, &six_at, into)
                .laid();
            check(&label("six posts"), &six, &kits);
            // Nine, three by three.
            let nine_at: Vec<[i32; 3]> = [0, 4, 7]
                .into_iter()
                .flat_map(|x| [0, 4, 7].map(|z| [x, 0, z]))
                .collect();
            let nine = Scene::rect(deck, 5, [0, 0], [7, 7])
                .posts(posts, &nine_at, into)
                .laid();
            check(&label("nine posts"), &nine, &kits);
        }
    }
}

#[test]
fn an_l_a_deck_against_a_wall_and_one_two_storeys_up_are_framed_the_same() {
    let kits = kits();
    for deck in [LOGS, HEWN] {
        // An L: a post at every corner of it and where its arms meet.
        let footprint: Vec<[i32; 2]> = (0..=9)
            .flat_map(|x| (0..=3).map(move |z| [x, z]))
            .chain((0..=3).flat_map(|x| (4..=9).map(move |z| [x, z])))
            .collect();
        let ell = Scene::new(deck, 5, footprint)
            .posts(
                deck,
                &[
                    [0, 0, 0],
                    [9, 0, 0],
                    [9, 0, 3],
                    [3, 0, 3],
                    [3, 0, 9],
                    [0, 0, 9],
                ],
                true,
            )
            .laid()
            .flight([15, 1], Facing::West, 2);
        check(&format!("an L in {deck:?}"), &ell, &kits);
        // Against a log wall two storeys high to the north: posts on its open side only.
        let mut against = Scene::rect(deck, 5, [0, 1], [7, 4]);
        put(&mut against.base, [0, 0, 0], [7, 11, 0], LOGS, Shape::Whole);
        let against = against
            .posts(deck, &[[0, 0, 4], [7, 0, 4]], true)
            .laid()
            .flight([-6, 2], Facing::East, 2);
        check(&format!("against a wall in {deck:?}"), &against, &kits);
        // Two storeys up, its stairs from the north.
        let high = Scene::rect(deck, 11, [0, 0], [5, 4])
            .posts(deck, &[[0, 0, 0], [5, 0, 0], [0, 0, 4], [5, 0, 4]], true)
            .laid()
            .flight([2, -12], Facing::South, 2);
        check(&format!("two storeys up in {deck:?}"), &high, &kits);
        // Over ground falling away: posts from feet a cube and two lower, longer, their
        // footings stepping down.
        let slope = Scene::rect(deck, 5, [0, 0], [5, 4])
            .posts(deck, &[[0, 0, 0], [5, 0, 0], [0, -1, 4], [5, -2, 4]], true)
            .laid();
        check(&format!("over a slope in {deck:?}"), &slope, &kits);
    }
}

#[test]
fn stairs_up_each_side_bear_on_the_rim_and_the_ground() {
    let kits = kits();
    for deck in [LOGS, HEWN] {
        let scene = Scene::rect(deck, 5, [0, 0], [5, 5])
            .posts(deck, &[[0, 0, 0], [5, 0, 0], [0, 0, 5], [5, 0, 5]], true)
            .laid()
            .flight([2, 11], Facing::North, 2)
            .flight([3, -6], Facing::South, 2)
            .flight([-6, 3], Facing::East, 2)
            .flight([11, 2], Facing::West, 2);
        check(&format!("stairs each side in {deck:?}"), &scene, &kits);
    }
}
