//! Corner stones (`Piece::Corner`): dressed blocks up an upright edge, cut from the
//! material's own wall (D215 as amended).

use super::*;

/// The share of a wall's height map its joints are where its kit does not say (`Breaks`).
pub(super) const STONE_SHARE: f32 = 0.22;

/// A corner stone's piece drawn into its wall's own material (`Masonry`), cut from `wall`,
/// its height map, where it is given.
pub(super) fn draw(
    cube: IVec3,
    (role, faces, courses, shared, seed): (
        crate::layout::Role,
        [Dir; 2],
        [bool; 2],
        [bool; 2],
        u32,
    ),
    kit: &Kit,
    wall: Option<&crate::relief::Relief>,
    surfaces: &mut Surfaces,
) {
    let Some(Dress::Corners {
        long,
        short,
        proud,
        bevel,
        joint,
    }) = kit.dress.roles.get(&role)
    else {
        return;
    };
    let material = if kit.textures.contains_key(&Material::Masonry) {
        Material::Masonry
    } else {
        Material::Dressed
    };
    let metres = kit
        .textures
        .get(&material)
        .map_or([1.2, 1.2], |texture| texture.metres);
    let share = match kit.damage.get(&Material::Masonry) {
        Some(crate::kit::Breaks::Joints { share, .. }) => *share,
        _ => STONE_SHARE,
    };
    let stones = wall.map(|wall| wall.stones(share));
    let mut made = Indexed::default();
    corner(
        cube,
        (faces, shared),
        courses,
        &CornerStones {
            long,
            short,
            proud: *proud,
            bevel: *bevel,
            joint: *joint,
        },
        (
            stones
                .as_deref()
                .map(Vec::as_slice)
                .filter(|s| !s.is_empty()),
            metres,
            kit.tint,
        ),
        seed,
        &mut made,
    );
    surfaces.of(material).add(made);
}

/// A kind's corner stones, as its pattern gives their numbers (`Dress::Corners`).
pub(super) struct CornerStones<'a> {
    long: &'a [[f32; 2]],
    short: &'a [[f32; 2]],
    proud: f32,
    bevel: f32,
    joint: f32,
}

/// A corner stone for each course of the cube's upright edge that shows one: a dressed
/// block of the wall's own stone, its foot at the course's, its corner the edge's, standing
/// `proud` of both faces, its long side along the first face and its short along the second,
/// turned each course where it has no `short` stones (quoins) or long and short by turns
/// (jamb stones). Where it shares a face with the cube's other edge's stones it reaches
/// along it no further than the face's middle, short of a joint. Each of its faces wears the
/// inside of one of the wall's own largest stones (`stones`, middles and sizes on its
/// texture), else the wall's texture at its own size; each a little lighter or darker.
pub(super) fn corner(
    cube: IVec3,
    (faces, shared): ([Dir; 2], [bool; 2]),
    courses: [bool; 2],
    set: &CornerStones,
    (stones, metres, tint): (Option<&[crate::cells::Stone]>, [f32; 2], [f32; 3]),
    seed: u32,
    out: &mut Indexed,
) {
    if set.long.is_empty() {
        return;
    }
    // The edge's foot: the cube's corner on both faces.
    let mut base = cube.as_vec3() * CELL_M;
    for face in faces {
        if face.sign() > 0 {
            base[face.axis()] += CELL_M;
        }
    }
    for (course, shows) in courses.into_iter().enumerate() {
        if !shows {
            continue;
        }
        let turn = (cube.y * 2 + course as i32).rem_euclid(2) == 1;
        let stone_seed = hash(&[seed, course as u32]);
        let pick = |sizes: &[[f32; 2]]| -> Option<[f32; 2]> {
            (!sizes.is_empty()).then(|| sizes[hash(&[stone_seed, 9]) as usize % sizes.len()])
        };
        let (size, [a, b]) = if set.short.is_empty() {
            (
                pick(set.long),
                if turn { [faces[1], faces[0]] } else { faces },
            )
        } else {
            (pick(if turn { set.short } else { set.long }), faces)
        };
        let Some([along_a, along_b]) = size else {
            continue;
        };
        let shares = |face: Dir| faces.iter().zip(shared).any(|(f, s)| *f == face && s);
        let reach = |shares: bool, length: f32| {
            if shares {
                length.min(SHARED_REACH_M + set.joint)
            } else {
                length
            }
        };
        // In its own frame: its +Z face on the first face (a), running back along -X; its +X
        // face on the second (b), running back along -Z; a course high less its joints.
        let (along_a, along_b) = (reach(shares(a), along_a), reach(shares(b), along_b));
        let low = Vec3::new(-along_a + set.joint, set.joint, -along_b + set.joint);
        let high = Vec3::new(set.proud, COURSE_M - set.joint, set.proud);
        let turned = Mat3::from_cols(unit(b), Vec3::Y, unit(a));
        let mirrored = turned.determinant() < 0.0;
        let at = base + Vec3::Y * (course as f32 * COURSE_M);
        let (colour, shift) = stone_look(stone_seed, tint);
        // Each of its faces cut from a stone of its own, across the face and down it (up
        // it, its top and bottom), its middle at the stone's, as large as the stone allows
        // without a joint showing; else the wall's texture at its own size.
        let face_of = |n: Vec3| -> usize {
            let n = n.abs();
            if n.y >= n.x && n.y >= n.z {
                2
            } else if n.x > n.z {
                1
            } else {
                0
            }
        };
        let flat = |face: usize, p: Vec3| -> [f32; 2] {
            match face {
                0 => [p.x, -p.y],
                1 => [-p.z, -p.y],
                _ => [p.x, p.z],
            }
        };
        // Each face at the wall texture's own scale, never larger (D362): the inside of
        // one of the wall's larger stones, laid again mirrored beside itself as far as the
        // face reaches, so a face larger than the stone is the same rock carried on, not
        // one stone blown up to it (two and a half times, before).
        let centre_of = (low + high) * 0.5;
        let scale = 1.0 / metres[0].max(0.01);
        let cut = |face: usize| -> Tiling {
            let middle = flat(face, centre_of);
            let (a, b) = (flat(face, low), flat(face, high));
            let origin = [a[0].min(b[0]), a[1].min(b[1])];
            match stones {
                Some(stones) => {
                    let (centre, radius) =
                        pick_stone(stones, hash(&[stone_seed, 11 + face as u32]));
                    let half = (radius * STONE_INSIDE).max(MIN_TILE_M * 0.5 * scale);
                    Tiling {
                        centre,
                        middle,
                        origin,
                        scale,
                        tile: 2.0 * half / scale,
                    }
                }
                None => Tiling {
                    centre: shift,
                    middle,
                    origin,
                    scale,
                    tile: f32::INFINITY,
                },
            }
        };
        let cuts = [cut(0), cut(1), cut(2)];
        let first = out.positions.len() as u32;
        let triangles = bevelled_box(low, high, set.bevel);
        for corners in triangles.chunks_exact(3) {
            let face = face_of(corners[0].1);
            let tiling = &cuts[face];
            for (p, n, uv) in tiling.lay(corners, |p| flat(face, p)) {
                out.corner(
                    at + turned * p,
                    (turned * n).normalize_or(Vec3::Y),
                    uv,
                    colour,
                );
            }
        }
        let count = out.positions.len() as u32 - first;
        for t in (0..count).step_by(3) {
            let [x, y, z] = [first + t, first + t + 1, first + t + 2];
            if mirrored {
                out.indices.extend([x, z, y]);
            } else {
                out.indices.extend([x, y, z]);
            }
        }
    }
}

/// How much of a stone's inside, from its middle to its nearest joint, half a corner stone's
/// face's copy of it spans: a square whose corners stay within the stone (under 1/sqrt 2),
/// so no joint shows in the face.
pub(super) const STONE_INSIDE: f32 = 0.68;
/// The least a face's copy of its stone covers, in metres: a smaller stone is drawn larger
/// than its wall's, so a face is not cut into slivers.
pub(super) const MIN_TILE_M: f32 = 0.12;

/// One of a wall's larger stones for a seed: among those at least half the largest's size.
pub(super) fn pick_stone(stones: &[crate::cells::Stone], seed: u32) -> crate::cells::Stone {
    let largest = stones.iter().map(|s| s.1).fold(0.0, f32::max);
    let large: Vec<_> = stones.iter().filter(|s| s.1 >= largest * 0.5).collect();
    *large[seed as usize % large.len().max(1)]
}

/// How a corner stone's face wears its stone: at the wall's own scale (`scale`, texture a
/// metre), the stone's inside, `tile` metres across, laid from the face's corner and again
/// mirrored beside itself, so the texture runs on unbroken and nothing is drawn larger than
/// the wall draws it.
pub(super) struct Tiling {
    centre: [f32; 2],
    middle: [f32; 2],
    /// Where the face's copies start, its corner: as few as cover it.
    origin: [f32; 2],
    scale: f32,
    tile: f32,
}

impl Tiling {
    /// A face's triangle, cut where its copies of the stone meet, each piece's corners
    /// with their texture places: positions, normals and places, three a triangle.
    fn lay(
        &self,
        corners: &[(Vec3, Vec3)],
        flat: impl Fn(Vec3) -> [f32; 2],
    ) -> Vec<(Vec3, Vec3, [f32; 2])> {
        let n = corners[0].1;
        // Each corner's place in tiles, from the face's corner.
        let at = |p: Vec3| -> [f32; 2] {
            let t = flat(p);
            [0, 1].map(|i| (t[i] - self.origin[i]) / self.tile)
        };
        let mut out = Vec::new();
        if !self.tile.is_finite() {
            // No stones: the wall's texture at its own size, as it runs.
            for &(p, _) in corners {
                let t = flat(p);
                let uv = [0, 1].map(|i| self.centre[i] + (t[i] - self.middle[i]) * self.scale);
                out.push((p, n, uv));
            }
            return out;
        }
        let points: Vec<(Vec3, [f32; 2])> = corners.iter().map(|&(p, _)| (p, at(p))).collect();
        let lo = [0, 1].map(|i| {
            points
                .iter()
                .map(|q| q.1[i])
                .fold(f32::MAX, f32::min)
                .floor() as i32
        });
        let hi = [0, 1].map(|i| {
            points
                .iter()
                .map(|q| q.1[i])
                .fold(f32::MIN, f32::max)
                .ceil() as i32
        });
        for i in lo[0]..hi[0] {
            for j in lo[1]..hi[1] {
                let mut poly = points.clone();
                for (axis, edge, keep_above) in [
                    (0, i, true),
                    (0, i + 1, false),
                    (1, j, true),
                    (1, j + 1, false),
                ] {
                    poly = clip(&poly, axis, edge as f32, keep_above);
                    if poly.len() < 3 {
                        break;
                    }
                }
                if poly.len() < 3 {
                    continue;
                }
                for k in 1..poly.len() - 1 {
                    for &(p, t) in [&poly[0], &poly[k], &poly[k + 1]] {
                        out.push((p, n, self.place(t, [i, j])));
                    }
                }
            }
        }
        out
    }

    /// The texture's place for a point `t` tiles in, in tile `cell`: mirrored in every
    /// other tile each way, so neighbouring copies meet on the same texels.
    fn place(&self, t: [f32; 2], cell: [i32; 2]) -> [f32; 2] {
        let half = self.tile * 0.5 * self.scale;
        [0, 1].map(|i| {
            let mut f = t[i] - cell[i] as f32;
            if cell[i].rem_euclid(2) == 1 {
                f = 1.0 - f;
            }
            self.centre[i] - half + f * 2.0 * half
        })
    }
}

/// A convex polygon cut by a line of tiles, keeping the side above or below it.
pub(super) fn clip(
    poly: &[(Vec3, [f32; 2])],
    axis: usize,
    edge: f32,
    keep_above: bool,
) -> Vec<(Vec3, [f32; 2])> {
    let inside = |q: &(Vec3, [f32; 2])| {
        if keep_above {
            q.1[axis] >= edge
        } else {
            q.1[axis] <= edge
        }
    };
    let mut out = Vec::new();
    for k in 0..poly.len() {
        let (a, b) = (&poly[k], &poly[(k + 1) % poly.len()]);
        if inside(a) {
            out.push(*a);
        }
        if inside(a) != inside(b) {
            let s = (edge - a.1[axis]) / (b.1[axis] - a.1[axis]);
            let mut t = [
                a.1[0] + (b.1[0] - a.1[0]) * s,
                a.1[1] + (b.1[1] - a.1[1]) * s,
            ];
            t[axis] = edge;
            out.push((a.0 + (b.0 - a.0) * s, t));
        }
    }
    out
}

/// A box from `low` to `high` with its twelve edges bevelled by `bevel` and its eight
/// corners cut, as triangles, three corners each, each with its own face's normal, wound to
/// face out: a dressed block's hard arrises.
pub(super) fn bevelled_box(low: Vec3, high: Vec3, bevel: f32) -> Vec<(Vec3, Vec3)> {
    let half = (high - low) * 0.5;
    let middle = (high + low) * 0.5;
    let b = bevel.min(half.min_element() * 0.45);
    let inner = half - Vec3::splat(b);
    let point = |sign: Vec3, out: [bool; 3]| -> Vec3 {
        middle
            + Vec3::new(
                sign.x * if out[0] { half.x } else { inner.x },
                sign.y * if out[1] { half.y } else { inner.y },
                sign.z * if out[2] { half.z } else { inner.z },
            )
    };
    let mut triangles = Vec::new();
    let mut add = |corners: &[Vec3]| {
        let centre = corners.iter().copied().sum::<Vec3>() / corners.len() as f32;
        for i in 1..corners.len() - 1 {
            let (mut p, q, mut r) = (corners[0], corners[i], corners[i + 1]);
            let mut normal = (q - p).cross(r - p).normalize_or_zero();
            if normal.dot(centre - middle) < 0.0 {
                std::mem::swap(&mut p, &mut r);
                normal = -normal;
            }
            triangles.extend([(p, normal), (q, normal), (r, normal)]);
        }
    };
    let signs = [-1.0f32, 1.0];
    for axis in 0..3 {
        for s in signs {
            let mut quad = Vec::new();
            for (a, b2) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                let mut sign = Vec3::ZERO;
                sign[axis] = s;
                sign[(axis + 1) % 3] = a;
                sign[(axis + 2) % 3] = b2;
                let mut out = [false; 3];
                out[axis] = true;
                quad.push(point(sign, out));
            }
            add(&quad);
        }
    }
    for axis in 0..3 {
        let (p, q) = ((axis + 1) % 3, (axis + 2) % 3);
        for sp in signs {
            for sq in signs {
                let mut quad = Vec::new();
                for (along, outer) in [(-1.0, p), (1.0, p), (1.0, q), (-1.0, q)] {
                    let mut sign = Vec3::ZERO;
                    sign[axis] = along;
                    sign[p] = sp;
                    sign[q] = sq;
                    let mut out = [false; 3];
                    out[outer] = true;
                    quad.push(point(sign, out));
                }
                add(&quad);
            }
        }
    }
    for sx in signs {
        for sy in signs {
            for sz in signs {
                let sign = Vec3::new(sx, sy, sz);
                add(&[0, 1, 2].map(|axis| {
                    let mut out = [false; 3];
                    out[axis] = true;
                    point(sign, out)
                }));
            }
        }
    }
    triangles
}
