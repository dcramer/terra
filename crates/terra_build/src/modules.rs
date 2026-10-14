//! Modules laid along lines (D251, D258, docs/building-plan.md#one-material-system-d251-d258):
//! a length of mesh modelled in Blender (`tools/kits`), repeated along a line the layout
//! gives, as Unreal's spline meshes tile a modular kit's trim along a spline. Code only
//! places, repeats, stretches and turns them (D177): a line takes as many whole copies as
//! fit it best, each stretched along to make them fit, and is closed at each end it stops by
//! the module's cap.
//!
//! **Seats.** A module is modelled along +X from 0 to its length, Y up from what it sits on
//! and Z across. It is seated on a line in one of two ways:
//!
//! - **On a plane**: a member along its middle, or a piece along a roof's edge, its Y the
//!   plane's normal and its Z the way out of it (a verge's bundle out past the verge, a
//!   frame's post out of the wall).
//! - **Over a crease**: a capping over a ridge or a hip, or a lining in a valley, draped over
//!   the two planes that meet there, its -Z half laid on the first and its +Z half on the
//!   second, each its Y from its own plane, so one capping fits a ridge's right angle and a
//!   hip's flatter one alike.
//!
//! Each copy's texture is mapped in its own frame, along it as timber's grain runs, its
//! faces across it from the side; faces looking along it, a member's sawn ends, can take
//! another material, and each copy is a little lighter or darker by its seed.

use crate::dress::{Module, Stone};
use crate::layout::{self, hash};
use crate::mesh::{Indexed, Material, Surfaces};
use glam::Vec3;

/// How a module sits on its line.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Seat {
    /// On one plane: `up` its normal, `out` square to the line in it.
    Plane { up: Vec3, out: Vec3 },
    /// Over the crease where two planes meet: each plane's normal, and the way into each
    /// from the line, square to it, in it.
    Crease { up: [Vec3; 2], into: [Vec3; 2] },
}

/// What a module is drawn in, and how.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub material: Material,
    /// The material of its faces that look along it, if not its own: a member's end grain.
    pub ends: Option<Material>,
    /// How many metres one copy of each's texture covers.
    pub metres: [f32; 2],
    pub end_metres: [f32; 2],
    /// What its colour is multiplied by, and how much lighter or darker a copy may be.
    pub tint: [f32; 3],
    pub shade: f32,
}

/// Where a module is laid: from one end of a line to the other, seated so, its section
/// scaled across to `across` metres (Y, Z) if given, and capped at the ends that stop.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Line {
    pub from: Vec3,
    pub to: Vec3,
    pub seat: Seat,
    pub across: Option<[f32; 2]>,
    pub capped: [bool; 2],
    pub seed: u32,
    /// Over a crease, how far either side of it, in metres, the module's up turns from one
    /// plane's to the other's, so what stands high over it, a ridge's crown, stays whole.
    pub blend: f32,
}

/// Lays a modelled module along a line as a modular kit's piece is laid: copies at the length
/// they were modelled, end to end from an offset along the model the line's seed picks, cut
/// where the line stops (or where its caps begin) and closed there, so a short rail is a
/// length of the timber, not the whole timber squeezed, and no two lines start alike.
fn lay_cut(
    module: &Module,
    variants: &[Stone],
    line: &Line,
    look: &Look,
    (lo, hi): (f32, f32),
    scale: [f32; 2],
    (body, ends): (&mut Indexed, &mut Indexed),
) {
    let length = line.from.distance(line.to);
    let axis = (line.to - line.from) / length;
    let long = module.length.max(0.05);
    let into = 0.1 + 0.8 * layout::unit(hash(&[line.seed, 13]));
    let first = lo - into * long;
    let mut cuts: [Vec<(Vec3, Vec3, [f32; 2])>; 2] = [Vec::new(), Vec::new()];
    let mut copy = 0;
    while first + copy as f32 * long < hi {
        let seed = hash(&[line.seed, copy as u32]);
        let mesh = &variants[hash(&[seed, 7]) as usize % variants.len()];
        let start = first + copy as f32 * long;
        // Every other copy turned end for end, so its repeat reads half as often.
        let (offset, along) = if hash(&[seed, 11]) % 2 == 1 {
            (start + long, -1.0)
        } else {
            (start, 1.0)
        };
        clipped(
            mesh,
            (offset, Vec3::new(along, scale[0], scale[1]), long),
            (lo, hi),
            line,
            axis,
            look,
            seed,
            body,
            &mut cuts,
        );
        copy += 1;
    }
    // Each cut closed, from its middle, looking along the line out of it.
    let shade = look.tint;
    for (end, segments) in cuts.into_iter().enumerate() {
        if segments.is_empty() {
            continue;
        }
        let outward = if end == 0 { -axis } else { axis };
        let count = segments.len() as f32 * 2.0;
        let middle = segments.iter().map(|s| s.0 + s.1).sum::<Vec3>() / count;
        let uv = segments
            .iter()
            .fold([0.0, 0.0], |a, s| [a[0] + s.2[0], a[1] + s.2[1]]);
        let uv = [uv[0] / (count * 0.5), uv[1] / (count * 0.5)];
        for (a, b, _) in segments {
            let face = (a - middle).cross(b - middle);
            if face.length_squared() < 1e-12 {
                continue;
            }
            let (a, b) = if face.dot(outward) >= 0.0 {
                (a, b)
            } else {
                (b, a)
            };
            let corners = [middle, a, b].map(|p| ends.corner(p, outward, uv, shade));
            ends.indices.extend(corners);
        }
    }
}

/// One copy of a modelled mesh placed on the line and cut to `lo`..`hi` along it, in the
/// line's own frame; its faces closing it at either end of the model left out, and the
/// segments where it is cut gathered, at the low cut and the high, to close it there.
#[allow(clippy::too_many_arguments)]
fn clipped(
    mesh: &Stone,
    (offset, axes, long): (f32, Vec3, f32),
    (lo, hi): (f32, f32),
    line: &Line,
    axis: Vec3,
    look: &Look,
    seed: u32,
    out: &mut Indexed,
    cuts: &mut [Vec<(Vec3, Vec3, [f32; 2])>; 2],
) {
    let shade = 1.0 - look.shade * layout::unit(hash(&[seed, 3]));
    let colour = look.tint.map(|c| c * shade);
    // Each corner: along the line, in the world, its normal and its place in the atlas.
    type Corner = (f32, Vec3, Vec3, [f32; 2]);
    let corners: Vec<Corner> = mesh
        .positions
        .iter()
        .zip(&mesh.normals)
        .enumerate()
        .map(|(i, (p, n))| {
            let (p, n) = (Vec3::from_array(*p), Vec3::from_array(*n));
            let q = Vec3::new(offset, 0.0, 0.0) + p * axes;
            let n = (n / axes).normalize_or(Vec3::Y);
            let (world, normal) = seat(line, axis, q, n);
            (
                q.x,
                world,
                normal,
                mesh.uvs.get(i).copied().unwrap_or([0.0; 2]),
            )
        })
        .collect();
    let lerp = |a: &Corner, b: &Corner, at: f32| -> Corner {
        let t = ((at - a.0) / (b.0 - a.0)).clamp(0.0, 1.0);
        (
            at,
            a.1.lerp(b.1, t),
            a.2.lerp(b.2, t).normalize_or(a.2),
            [
                a.3[0] + (b.3[0] - a.3[0]) * t,
                a.3[1] + (b.3[1] - a.3[1]) * t,
            ],
        )
    };
    for t in &mesh.triangles {
        let [a, b, c] = t.map(|i| i as usize);
        if a >= corners.len() || b >= corners.len() || c >= corners.len() {
            continue;
        }
        // The model's own ends, which copies meet at, never drawn.
        let x = |i: usize| mesh.positions[i][0];
        let at_end = |at: f32| [a, b, c].iter().all(|&i| (x(i) - at).abs() < 1e-3);
        if at_end(0.0) || at_end(long) {
            continue;
        }
        let mut polygon: Vec<Corner> = vec![corners[a], corners[b], corners[c]];
        for (end, (plane, keep_above)) in [(lo, true), (hi, false)].into_iter().enumerate() {
            let inside = |c: &Corner| {
                if keep_above {
                    c.0 >= plane
                } else {
                    c.0 <= plane
                }
            };
            let mut kept: Vec<Corner> = Vec::new();
            let mut on_plane: Vec<Corner> = Vec::new();
            for i in 0..polygon.len() {
                let (p, q) = (&polygon[i], &polygon[(i + 1) % polygon.len()]);
                if inside(p) {
                    kept.push(*p);
                }
                if inside(p) != inside(q) {
                    let cut = lerp(p, q, plane);
                    kept.push(cut);
                    on_plane.push(cut);
                }
            }
            if on_plane.len() == 2 {
                cuts[end].push((on_plane[0].1, on_plane[1].1, on_plane[0].3));
            }
            polygon = kept;
            if polygon.len() < 3 {
                break;
            }
        }
        if polygon.len() < 3 {
            continue;
        }
        let indices: Vec<u32> = polygon
            .iter()
            .map(|c| out.corner(c.1, c.2, c.3, colour))
            .collect();
        for k in 1..indices.len() - 1 {
            let [i, j, l] = [indices[0], indices[k], indices[k + 1]];
            let (pa, pb, pc) = (polygon[0].1, polygon[k].1, polygon[k + 1].1);
            let face = (pb - pa).cross(pc - pa);
            let normal = polygon[0].2 + polygon[k].2 + polygon[k + 1].2;
            if face.dot(normal) >= 0.0 {
                out.indices.extend([i, j, l]);
            } else {
                out.indices.extend([i, l, j]);
            }
        }
    }
}

/// How many copies of a module so long fit a line so long, and how much each is stretched.
pub fn fit(length: f32, module: f32) -> (usize, f32) {
    let module = module.max(0.05);
    let count = libm::roundf(length / module).max(1.0) as usize;
    (count, length / (count as f32 * module))
}

/// Lays a module along a line, its near meshes or, `far`, its far ones.
pub fn lay(module: &Module, line: &Line, look: &Look, far: bool, surfaces: &mut Surfaces) {
    let length = line.from.distance(line.to);
    if length < 1e-3 {
        return;
    }
    let variants = if far && !module.far.is_empty() {
        &module.far
    } else {
        &module.near
    };
    if variants.is_empty() {
        return;
    }
    let axis = (line.to - line.from) / length;
    // A modelled piece is closed by its cap within the line, or, with none, stops a little
    // short where the line stops, so its cut end is not in the plane of what it meets.
    let made = modelled(module);
    let cap_length = module.cap.as_ref().map_or(0.0, |cap| {
        cap.positions.iter().map(|p| p[0]).fold(0.0, f32::max)
    });
    let lead = line.capped.map(|capped| match (made, capped) {
        (true, true) if cap_length > 0.0 => cap_length.min(length * 0.3),
        (true, true) => END_INSET_M.min(length * 0.1),
        _ => 0.0,
    });
    let span = (length - lead[0] - lead[1]).max(length * 0.2);
    let (count, stretch) = fit(span, module.length);
    // Scaled across to the member's size, from the size it was modelled at.
    let scale = line.across.map_or([1.0, 1.0], |[y, z]| {
        let size = module.size();
        [y / size[0].max(1e-3), z / size[1].max(1e-3)]
    });
    let mut body = Indexed::default();
    let mut ends = Indexed::default();
    // A modelled piece is cut to length, never squeezed; one made from numbers fits whole
    // copies, which it can, being uniform along it.
    let count = if made {
        let lo = lead[0].min(length * 0.45);
        let hi = (length - lead[1]).max(lo + length * 0.1);
        lay_cut(
            module,
            variants,
            line,
            look,
            (lo, hi),
            scale,
            (&mut body, &mut ends),
        );
        0
    } else {
        count
    };
    for copy in 0..count {
        let seed = hash(&[line.seed, copy as u32]);
        let mesh = &variants[hash(&[seed, 7]) as usize % variants.len()];
        let start = copy as f32 * module.length;
        // A modelled piece is one model, so every other copy is turned end for end (mirrored
        // along it) and its repeat reads half as often.
        place(
            mesh,
            (
                lead[0] + start * stretch,
                Vec3::new(stretch, scale[0], scale[1]),
            ),
            line,
            axis,
            look,
            seed,
            (&mut body, &mut ends),
            None,
        );
    }
    // Caps where it stops: the end's as modelled, the start's mirrored along it.
    if let Some(cap) = &module.cap {
        for (end, capped) in line.capped.into_iter().enumerate() {
            if !capped {
                continue;
            }
            let seed = hash(&[line.seed, 99, end as u32]);
            let at = if end == 1 { length - lead[1] } else { lead[0] };
            let sign = if end == 1 { 1.0 } else { -1.0 };
            place(
                cap,
                (at, Vec3::new(sign, scale[0], scale[1])),
                line,
                axis,
                look,
                seed,
                (&mut body, &mut ends),
                made.then_some((cap_length, [false, true])),
            );
        }
    }
    // A modelled piece is in its kit's atlas, whatever it stands for; one made from
    // numbers in its material, its ends in theirs.
    if modelled(module) {
        put(surfaces, Material::Piece, body);
        put(surfaces, Material::Piece, ends);
    } else {
        put(surfaces, look.material, body);
        if let Some(material) = look.ends {
            put(surfaces, material, ends);
        }
    }
}

/// How far short of a line's stopped end a modelled piece with no cap stops, in metres.
const END_INSET_M: f32 = 0.015;

/// One copy of a mesh placed on the line: in the line's own frame (along it, up, across),
/// so far along it and scaled by so much each way, along by less than nothing to mirror it.
#[allow(clippy::too_many_arguments)]
fn place(
    mesh: &Stone,
    (offset, axes): (f32, Vec3),
    line: &Line,
    axis: Vec3,
    look: &Look,
    seed: u32,
    (body, ends): (&mut Indexed, &mut Indexed),
    // For a modelled piece: its length as modelled, and whether to keep the faces that close
    // it at its start (x = 0) and its end.
    closing: Option<(f32, [bool; 2])>,
) {
    let shade = 1.0 - look.shade * layout::unit(hash(&[seed, 3]));
    let colour = look.tint.map(|c| c * shade);
    let shift = [
        layout::unit(hash(&[seed, 1])),
        layout::unit(hash(&[seed, 2])),
    ];
    // Each corner in the world, with its normal, and where it falls on its texture.
    let corners: Vec<(Vec3, Vec3, [f32; 2], bool)> = mesh
        .positions
        .iter()
        .zip(&mesh.normals)
        .enumerate()
        .map(|(i, (p, n))| {
            let (p, n) = (Vec3::from_array(*p), Vec3::from_array(*n));
            let q = Vec3::new(offset, 0.0, 0.0) + p * axes;
            // A normal scaled with its mesh leans the other way, and turns with a mirror.
            let n = (n / axes).normalize_or(Vec3::Y);
            let (world, normal) = seat(line, axis, q, n);
            let a = n.abs();
            let end = a.x >= a.y && a.x >= a.z;
            // A modelled piece is read by its own UVs, in its kit's atlas.
            if let Some(&uv) = mesh.uvs.get(i) {
                return (world, normal, uv, false);
            }
            let uv = if end {
                [q.z / look.end_metres[0], -q.y / look.end_metres[1]]
            } else if a.y >= a.z {
                [q.z / look.metres[0], q.x / look.metres[1]]
            } else {
                [-q.y / look.metres[0], q.x / look.metres[1]]
            };
            (world, normal, [uv[0] + shift[0], uv[1] + shift[1]], end)
        })
        .collect();
    let mut map_body: Vec<Option<u32>> = vec![None; corners.len()];
    let mut map_ends: Vec<Option<u32>> = vec![None; corners.len()];
    for t in &mesh.triangles {
        let [a, b, c] = t.map(|i| i as usize);
        if a >= corners.len() || b >= corners.len() || c >= corners.len() {
            continue;
        }
        if let Some((long, keep)) = closing {
            let x = |i: usize| mesh.positions[i][0];
            let near = |at: f32| [a, b, c].iter().all(|&i| (x(i) - at).abs() < 1e-3);
            if (near(0.0) && !keep[0]) || (near(long) && !keep[1]) {
                continue;
            }
        }
        // A face looking along the module is an end, if it has a material of its own.
        let to_ends = look.ends.is_some() && corners[a].3 && corners[b].3 && corners[c].3;
        let (out, map) = if to_ends {
            (&mut *ends, &mut map_ends)
        } else {
            (&mut *body, &mut map_body)
        };
        let mut index = |i: usize| {
            *map[i].get_or_insert_with(|| {
                let (p, n, uv, _) = corners[i];
                out.corner(p, n, uv, colour)
            })
        };
        let tri = [index(a), index(b), index(c)];
        // Wound the way its normals look, whatever the seat turned or mirrored.
        let (pa, pb, pc) = (corners[a].0, corners[b].0, corners[c].0);
        let face = (pb - pa).cross(pc - pa);
        let normal = corners[a].1 + corners[b].1 + corners[c].1;
        if face.dot(normal) >= 0.0 {
            out.indices.extend(tri);
        } else {
            out.indices.extend([tri[0], tri[2], tri[1]]);
        }
    }
}

/// Whether a module is modelled, read by its own UVs in its kit's atlas (`Material::Piece`),
/// rather than made from numbers and mapped by where it lies.
pub fn modelled(module: &Module) -> bool {
    module.near.first().is_some_and(|mesh| !mesh.uvs.is_empty())
}

/// A point and a normal in a line's frame (along it, up, across) in the world.
fn seat(line: &Line, axis: Vec3, q: Vec3, n: Vec3) -> (Vec3, Vec3) {
    let along = line.from + axis * q.x;
    match line.seat {
        Seat::Plane { up, out } => (
            along + up * q.y + out * q.z,
            (axis * n.x + up * n.y + out * n.z).normalize_or(up),
        ),
        Seat::Crease { up, into } => {
            // Its -Z half on the first plane, its +Z half on the second, its up turning
            // from one plane's to the other's within `blend` of the crease.
            let on = if q.z < 0.0 {
                into[0] * -q.z
            } else {
                into[1] * q.z
            };
            let t = if line.blend > 0.0 {
                let t = ((q.z / line.blend) * 0.5 + 0.5).clamp(0.0, 1.0);
                t * t * (3.0 - 2.0 * t)
            } else if q.z < 0.0 {
                0.0
            } else {
                1.0
            };
            let up = up[0].lerp(up[1], t).normalize_or(Vec3::Y);
            let across = (-into[0]).lerp(into[1], t).normalize_or(into[1]);
            (
                along + on + up * q.y,
                (axis * n.x + up * n.y + across * n.z).normalize_or(up),
            )
        }
    }
}

/// Indexed triangles added to a surface, shared where it shares its corners.
pub(crate) fn put(surfaces: &mut Surfaces, material: Material, made: Indexed) {
    if made.indices.is_empty() {
        return;
    }
    let out = surfaces.of(material);
    if out.sharing {
        out.shared.append(made);
    } else {
        for &i in &made.indices {
            let i = i as usize;
            out.positions.push(made.positions[i]);
            out.normals.push(made.normals[i]);
            out.colours.push(made.colours[i]);
            out.uvs.push(made.uvs[i]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A box along +X a metre long, 0.1 m up from 0 and 0.2 m across from -0.1.
    fn bar() -> Module {
        let mut mesh = Stone::default();
        let corners = |x: f32| [[x, 0.0, -0.1], [x, 0.1, -0.1], [x, 0.1, 0.1], [x, 0.0, 0.1]];
        // Just its top, two triangles, facing up.
        let [_, a, b, _] = corners(0.0);
        let [_, c, d, _] = corners(1.0);
        mesh.positions = vec![a, b, d, c];
        mesh.normals = vec![[0.0, 1.0, 0.0]; 4];
        mesh.triangles = vec![[0, 1, 2], [0, 2, 3]];
        Module {
            length: 1.0,
            near: vec![mesh],
            ..Module::default()
        }
    }

    fn look() -> Look {
        Look {
            material: Material::Board,
            ends: None,
            metres: [1.0, 1.0],
            end_metres: [1.0, 1.0],
            tint: [1.0; 3],
            shade: 0.0,
        }
    }

    #[test]
    fn a_line_takes_whole_copies_stretched_to_fit_it() {
        // 2.6 m of a metre's module: three copies, each squeezed to 0.867 m.
        let (count, stretch) = fit(2.6, 1.0);
        assert_eq!(count, 3);
        assert!((count as f32 * stretch - 2.6).abs() < 1e-5);
        // Shorter than half a copy, still one, squeezed.
        assert_eq!(fit(0.3, 1.0).0, 1);
        // Laid, its copies reach from one end of the line to the other and no further.
        let mut surfaces = Surfaces::sharing();
        let line = Line {
            from: Vec3::new(1.0, 2.0, 0.0),
            to: Vec3::new(3.6, 2.0, 0.0),
            seat: Seat::Plane {
                up: Vec3::Y,
                out: Vec3::Z,
            },
            across: None,
            capped: [false; 2],
            seed: 5,
            blend: 0.0,
        };
        lay(&bar(), &line, &look(), false, &mut surfaces);
        let made = &surfaces.0[&Material::Board].shared;
        assert_eq!(made.triangle_count(), 6);
        let low = made.positions.iter().map(|p| p.x).fold(f32::MAX, f32::min);
        let high = made.positions.iter().map(|p| p.x).fold(f32::MIN, f32::max);
        assert!(
            (low - 1.0).abs() < 1e-4 && (high - 3.6).abs() < 1e-4,
            "{low} {high}"
        );
        // Its top a tenth of a metre up off the plane, facing up.
        assert!(made.positions.iter().all(|p| (p.y - 2.1).abs() < 1e-4));
        assert!(made.normals.iter().all(|n| n.y > 0.99));
    }

    #[test]
    fn a_modelled_module_is_cut_to_length_in_its_kits_atlas_not_squeezed() {
        // A metre's bar modelled, its atlas V running 0.2 to 0.4 along it, laid along 0.4 m.
        let mut module = bar();
        module.near[0].uvs = vec![[0.1, 0.2], [0.3, 0.2], [0.3, 0.4], [0.1, 0.4]];
        let line = Line {
            from: Vec3::ZERO,
            to: Vec3::X * 0.4,
            seat: Seat::Plane {
                up: Vec3::Y,
                out: Vec3::Z,
            },
            across: None,
            capped: [false; 2],
            seed: 3,
            blend: 0.0,
        };
        let mut surfaces = Surfaces::sharing();
        lay(&module, &line, &look(), false, &mut surfaces);
        assert!(!surfaces.0.contains_key(&Material::Board));
        let made = &surfaces.0[&Material::Piece].shared;
        assert!(made.triangle_count() > 0);
        // From one end of the line to the other and no further.
        let (low, high) = made
            .positions
            .iter()
            .fold((f32::MAX, f32::MIN), |(l, h), p| (l.min(p.x), h.max(p.x)));
        assert!(
            low.abs() < 1e-4 && (high - 0.4).abs() < 1e-4,
            "{low} {high}"
        );
        // Its texture as modelled, 0.2 of V a metre along it: 0.4 m of the bar, not the
        // whole bar squeezed into 0.4 m, which would run 0.5 a metre.
        let mut checked = 0;
        for t in made.indices.chunks(3) {
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                let (pa, pb) = (made.positions[a as usize], made.positions[b as usize]);
                let (va, vb) = (made.uvs[a as usize][1], made.uvs[b as usize][1]);
                let dx = (pa.x - pb.x).abs();
                if dx > 0.05 && (pa.y - 0.1).abs() < 1e-4 && (pb.y - 0.1).abs() < 1e-4 {
                    assert!(((va - vb).abs() / dx - 0.2).abs() < 1e-3, "{dx} {va} {vb}");
                    checked += 1;
                }
            }
        }
        assert!(checked > 0);
    }

    #[test]
    fn a_capping_over_a_crease_lies_on_both_planes() {
        // A ridge along X: the north slope rises to it from -Z, the south from +Z.
        let s = std::f32::consts::FRAC_1_SQRT_2;
        let line = Line {
            from: Vec3::ZERO,
            to: Vec3::X * 2.0,
            seat: Seat::Crease {
                up: [Vec3::new(0.0, s, -s), Vec3::new(0.0, s, s)],
                into: [Vec3::new(0.0, -s, -s), Vec3::new(0.0, -s, s)],
            },
            across: None,
            capped: [false; 2],
            seed: 1,
            blend: 0.0,
        };
        let mut surfaces = Surfaces::sharing();
        lay(&bar(), &line, &look(), false, &mut surfaces);
        let made = &surfaces.0[&Material::Board].shared;
        // Each corner a tenth of a metre off its own plane, on its own side, and below the
        // crease by as much as it is across it, as a 45° slope falls.
        for (p, n) in made.positions.iter().zip(&made.normals) {
            let plane = if p.z < 0.0 {
                Vec3::new(0.0, s, -s)
            } else {
                Vec3::new(0.0, s, s)
            };
            assert!((p.dot(plane) - 0.1).abs() < 1e-4, "{p}");
            assert!(n.dot(plane) > 0.99, "{n}");
        }
    }
}
