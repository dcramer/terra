//! A member made into triangles: its rings swept into a tube, and its sawn ends capped with
//! the end grain's picture.

use super::*;

/// A sawn end's colour: the kit's end tint, lighter or darker and warmer or cooler with its
/// member as the member's face is, and then each end its own, a little lighter or darker and
/// greyed by the weather from hardly at all to nearly half, as a wall's ends are where some
/// were cut later or face the rain: one tint at every end of a wall read as a row of the same
/// sticker (research/kit-review-2026-10-14.md).
pub(super) fn end_colour(t: &crate::kit::Timber, seed: u32, shade: f32, warmth: f32) -> [f32; 3] {
    let mut random = Random(layout::hash(&[seed, 0x5a3d]));
    let own = 1.0 + END_SHADE * random.signed();
    let grey = END_GREY * random.unit() * random.unit();
    let tinted = [
        t.end_tint[0] * shade * own * (1.0 + warmth),
        t.end_tint[1] * shade * own,
        t.end_tint[2] * shade * own * (1.0 - warmth),
    ];
    let luma = (tinted[0] + tinted[1] + tinted[2]) / 3.0;
    tinted.map(|v| (v + (luma - v) * grey).clamp(0.0, 1.0))
}

/// How much lighter or darker each sawn end is than its member's, and the most it is
/// greyed toward its own lightness.
pub(super) const END_SHADE: f32 = 0.1;
pub(super) const END_GREY: f32 = 0.6;

/// A member's rings as a closed tube of triangles, each corner shaded smooth across its
/// neighbours but at a crease, its texture as its rings say, the seam's last column its own.
pub(super) fn sweep(out: &mut Surface, member: &Member, colour: [f32; 3]) {
    let grid = &member.grid;
    let rows = grid.len();
    let columns = grid[0].len();
    let colour_at = |r: usize, c: usize| {
        let k = member
            .tone
            .get(r)
            .and_then(|ring| ring.get(c))
            .copied()
            .unwrap_or(1.0);
        let hue = member
            .hue
            .get(r)
            .and_then(|ring| ring.get(c))
            .copied()
            .unwrap_or([1.0; 3]);
        [0, 1, 2].map(|i| (colour[i] * k * hue[i]).clamp(0.0, 1.0))
    };
    let wear_at = |r: usize, c: usize| {
        member
            .wear
            .get(r)
            .and_then(|ring| ring.get(c))
            .copied()
            .unwrap_or(0.0)
    };
    // A triangle on a break is shaded flat, its own face, so a break's edges are hard.
    let cut_at = |r: usize, c: usize| {
        member
            .cut
            .get(r)
            .and_then(|ring| ring.get(c))
            .copied()
            .unwrap_or(false)
    };
    // Each point's normal: the sum of the faces round it. The rings go round the way that
    // makes each quad's first triangle face out, as `round` and `hewn` lay them.
    let face = |a: Vec3, b: Vec3, c: Vec3| (b - a).cross(c - a);
    let mut normals = vec![vec![Vec3::ZERO; columns]; rows];
    for i in 0..rows - 1 {
        for j in 0..columns {
            let k = (j + 1) % columns;
            let (a, b, c, d) = (grid[i][j], grid[i + 1][j], grid[i + 1][k], grid[i][k]);
            let (n1, n2) = (face(a, b, c), face(a, c, d));
            for (r, col, n) in [
                (i, j, n1 + n2),
                (i + 1, j, n1),
                (i + 1, k, n1 + n2),
                (i, k, n2),
            ] {
                normals[r][col] += n;
            }
        }
    }
    if out.sharing {
        // The same triangles, each corner shaded smooth shared by all that meet at it.
        let mut smooth: Vec<u32> = vec![u32::MAX; rows * (columns + 1)];
        for i in 0..rows - 1 {
            for j in 0..columns {
                let k = (j + 1) % columns;
                let quad = [(i, j), (i + 1, j), (i + 1, k), (i, k)];
                for tri in [[0, 1, 2], [0, 2, 3]] {
                    let corners = tri.map(|index| grid[quad[index].0][quad[index].1]);
                    let own = face(corners[0], corners[1], corners[2]);
                    if own.length_squared() < 1e-14 {
                        continue;
                    }
                    let own = own.normalize();
                    let flat = tri
                        .iter()
                        .any(|&index| cut_at(quad[index].0, quad[index].1));
                    let indices = tri.map(|index| {
                        let (r, c) = quad[index];
                        let column = if index >= 2 && k == 0 { columns } else { c };
                        let normal = normals[r][c].normalize_or_zero();
                        let uv = member.uvs[r][column];
                        if flat || normal.dot(own) < CREASE {
                            return out.shared.worn_corner(
                                grid[r][c],
                                own,
                                uv,
                                colour_at(r, c),
                                wear_at(r, c),
                            );
                        }
                        let slot = &mut smooth[r * (columns + 1) + column];
                        if *slot == u32::MAX {
                            *slot = out.shared.worn_corner(
                                grid[r][c],
                                normal,
                                uv,
                                colour_at(r, c),
                                wear_at(r, c),
                            );
                        }
                        *slot
                    });
                    out.shared.indices.extend(indices);
                }
            }
        }
        return;
    }
    for i in 0..rows - 1 {
        for j in 0..columns {
            let k = (j + 1) % columns;
            let quad = [(i, j), (i + 1, j), (i + 1, k), (i, k)];
            for tri in [[0, 1, 2], [0, 2, 3]] {
                let corners = tri.map(|index| grid[quad[index].0][quad[index].1]);
                let own = face(corners[0], corners[1], corners[2]);
                if own.length_squared() < 1e-14 {
                    continue;
                }
                let own = own.normalize();
                let flat = tri
                    .iter()
                    .any(|&index| cut_at(quad[index].0, quad[index].1));
                for index in tri {
                    let (r, c) = quad[index];
                    let smooth = if flat {
                        own
                    } else {
                        normals[r][c].normalize_or_zero()
                    };
                    if wear_at(r, c) > 0.0 {
                        crate::mesh::worn(&mut out.wear, out.positions.len());
                        out.wear.push(wear_at(r, c));
                    }
                    out.positions.push(grid[r][c]);
                    out.normals.push(if smooth.dot(own) < CREASE {
                        own
                    } else {
                        smooth
                    });
                    // The seam's last column takes its own place on the texture.
                    let column = if index >= 2 && k == 0 { columns } else { c };
                    out.uvs.push(member.uvs[r][column]);
                    out.colours.push(colour_at(r, c));
                }
            }
        }
    }
}

/// Which picture of the kit's sawn ends an end draws, by a seed, where its `LogEnd` texture
/// is an atlas of several: the corner of its cell on the texture, and the cell's size.
pub(crate) fn end_cell(kit: &Kit, seed: u32) -> ([f32; 2], f32) {
    let cells = kit
        .textures
        .get(&Material::LogEnd)
        .map_or(1, |texture| texture.cells.max(1));
    if cells == 1 {
        return ([0.0, 0.0], 1.0);
    }
    let which = crate::layout::hash(&[seed, 0x0e4d]) % (cells * cells);
    let size = 1.0 / cells as f32;
    (
        [(which % cells) as f32 * size, (which / cells) as f32 * size],
        size,
    )
}

pub(super) fn smoothstep(from: f32, to: f32, x: f32) -> f32 {
    let t = ((x - from) / (to - from)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// A flat cap over a ring, facing `outward`, the end grain's picture across it, turned by
/// `spin`, as much of it as `grain` says, from the atlas's cell `cell` gives.
#[allow(clippy::too_many_arguments)]
pub(super) fn cap(
    out: &mut Surface,
    ring: &[Vec3],
    outward: Vec3,
    frame: &Frame,
    spin: f32,
    grain: [f32; 2],
    colour: [f32; 3],
    cell: ([f32; 2], f32),
) {
    let centre = ring.iter().copied().sum::<Vec3>() / ring.len() as f32;
    let reach = ring
        .iter()
        .map(|p| {
            let d = *p - centre;
            d.dot(frame.side).abs().max(d.dot(frame.up).abs())
        })
        .fold(1e-3, f32::max);
    let (s, c) = (libm::sinf(spin), libm::cosf(spin));
    let place = |p: Vec3| {
        let d = p - centre;
        let (x, y) = (
            d.dot(frame.side) / reach * grain[0],
            d.dot(frame.up) / reach * grain[1],
        );
        let (x, y) = (x * c - y * s, x * s + y * c);
        let (corner, size) = cell;
        [corner[0] + size * (0.5 + x), corner[1] + size * (0.5 - y)]
    };
    for j in 0..ring.len() {
        let k = (j + 1) % ring.len();
        let mut t = [centre, ring[j], ring[k]];
        let own = (t[1] - t[0]).cross(t[2] - t[0]);
        if own.length_squared() < 1e-14 {
            continue;
        }
        if own.dot(outward) < 0.0 {
            t.swap(1, 2);
        }
        for p in t {
            out.positions.push(p);
            out.normals.push(outward);
            out.uvs.push(place(p));
            out.colours.push(colour);
        }
    }
}
