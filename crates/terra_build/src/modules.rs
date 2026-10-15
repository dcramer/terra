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
    /// How many times its length each copy is drawn: 1 as modelled, `FAR_REPEAT` far off.
    pub repeat: f32,
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
    /// How much further off what it lies on the module stands than modelled, square to it, in
    /// metres: where its anchor is seated (a roof's covering, D290).
    pub lift: f32,
    /// Over a crease, at each end that meets the edges of the planes it lies on, as a hip
    /// and a valley meet the eaves at a corner: the way out of each plane's edge, level, for
    /// the module's half on that plane. Each half is cut along its plane's edge through the
    /// line's end and closed there, mitred as a hip's capping is to the eaves, rather than
    /// standing out past them square to the line.
    pub mitre: [Option<[Vec3; 2]>; 2],
    /// How far along it a modelled piece's cut ends lean for each metre up it, so they stand
    /// plumb where it lies along a slope square to it: a verge's board seated square to its
    /// roof, unsheared, its ends still cut plumb at the eave and the apex. 0 cuts them square.
    pub plumb: f32,
    /// Over a crease, a wider `blend` for its body, and how far from each end it narrows
    /// back to `blend` (none: wide to that end): a hip's capping turned over its own
    /// thickness's arc but where it meets the ridge or the other hips at its top, where the
    /// pieces' cuts are made for the turn as modelled.
    pub crest: Option<(f32, [f32; 2])>,
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
    (lo, hi, capped): (f32, f32, [bool; 2]),
    scale: [f32; 2],
    (body, ends): (&mut Indexed, &mut Indexed),
) {
    let length = line.from.distance(line.to);
    let axis = (line.to - line.from) / length;
    // Each copy drawn `repeat` times its length, so a line far off has that many
    // times fewer: what repeats along it is a few pixels apart either way.
    let model = module.length.max(0.05);
    let stretch = look.repeat.max(1.0);
    let long = model * stretch;
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
            (
                offset,
                Vec3::new(along * stretch, scale[0], scale[1]),
                model,
            ),
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
    // Each cut closed, looking along the line out of it, but where a cap is laid on it: the
    // cap closes it, and a face between the two would be drawn twice over in one plane.
    let shade = look.tint;
    for (end, segments) in cuts.into_iter().enumerate() {
        if segments.is_empty() || capped[end] {
            continue;
        }
        // Square to the cut's own plane: a plumb cut along a slope leans off square to it.
        let cut = match line.seat {
            Seat::Plane { up, .. } if line.plumb != 0.0 => {
                (axis + up.normalize_or_zero() * line.plumb).normalize_or(axis)
            }
            _ => axis,
        };
        let outward = if end == 0 { -cut } else { cut };
        let count = segments.len() as f32;
        let uv = segments
            .iter()
            .fold([0.0, 0.0], |a, s| [a[0] + s.2[0], a[1] + s.2[1]]);
        let uv = [uv[0] / count, uv[1] / count];
        let lines: Vec<(Vec3, Vec3)> = segments.iter().map(|s| (s.0, s.1)).collect();
        for [a, b, c] in close(&lines, outward) {
            let corners = [a, b, c].map(|p| ends.corner(p, outward, uv, shade));
            ends.indices.extend(corners);
        }
    }
}

/// The faces that close a cut through a mesh, from the segments its triangles were cut along,
/// all in one plane square to `outward`: the segments joined end to end into outlines, an
/// open one closed straight across, each filled by ear clipping (as Blender's and every
/// engine's mesh slicing caps a cut), so a section that is not convex, a ridge's roll or a
/// bundle's lumpy face, is filled once and never folded over itself as a fan from its middle
/// would be. Each triangle looks the outward way.
pub(crate) fn close(segments: &[(Vec3, Vec3)], outward: Vec3) -> Vec<[Vec3; 3]> {
    const SAME_M: f32 = 1e-5;
    let u = outward.any_orthonormal_vector();
    let v = outward.cross(u);
    let flat = |p: Vec3| glam::Vec2::new(p.dot(u), p.dot(v));
    let mut used = vec![false; segments.len()];
    let mut filled: Vec<[glam::Vec2; 3]> = Vec::new();
    let mut all: Vec<Vec3> = Vec::new();
    for first in 0..segments.len() {
        if used[first] {
            continue;
        }
        used[first] = true;
        let mut outline = vec![segments[first].0, segments[first].1];
        // Onward from its end, then back from its start.
        for forward in [true, false] {
            loop {
                let tip = if forward {
                    outline[outline.len() - 1]
                } else {
                    outline[0]
                };
                let next = (0..segments.len()).find_map(|k| {
                    if used[k] {
                        return None;
                    }
                    let (a, b) = segments[k];
                    if a.distance(tip) < SAME_M {
                        Some((k, b))
                    } else if b.distance(tip) < SAME_M {
                        Some((k, a))
                    } else {
                        None
                    }
                });
                let Some((k, p)) = next else {
                    break;
                };
                used[k] = true;
                if forward {
                    outline.push(p);
                } else {
                    outline.insert(0, p);
                }
            }
        }
        // The same point twice in a row, or the outline's end on its start, once.
        outline.dedup_by(|a, b| a.distance(*b) < SAME_M);
        if outline.len() > 1 && outline[0].distance(outline[outline.len() - 1]) < SAME_M {
            outline.pop();
        }
        if outline.len() < 3 {
            continue;
        }
        let points: Vec<glam::Vec2> = outline.iter().map(|&p| flat(p)).collect();
        // An outline every point of which its middle sees, a member's or a bundle's, a fan
        // from its middle, which keeps its triangles whole; any other by its ears.
        let middle = points.iter().sum::<glam::Vec2>() / points.len() as f32;
        let turn =
            |i: usize| (points[i] - middle).perp_dot(points[(i + 1) % points.len()] - middle);
        let way = (0..points.len()).map(turn).sum::<f32>().signum();
        if (0..points.len()).all(|i| turn(i) * way > 0.0) {
            for i in 0..points.len() {
                filled.push([middle, points[i], points[(i + 1) % points.len()]]);
            }
        } else {
            for [i, j, k] in ear_clip(&points) {
                filled.push([points[i], points[j], points[k]]);
            }
        }
        all.extend(outline);
    }
    // The plane the cut lies in, which a sheared seat leans off square to the line: what is
    // filled in the square plane is put back on it, along the line.
    let Some(&first) = all.first() else {
        return Vec::new();
    };
    let far = all
        .iter()
        .copied()
        .max_by(|a, b| a.distance(first).total_cmp(&b.distance(first)))
        .unwrap_or(first);
    let plane = all
        .iter()
        .map(|&p| (far - first).cross(p - first))
        .max_by(|a, b| a.length_squared().total_cmp(&b.length_squared()))
        .unwrap_or(outward)
        .normalize_or(outward);
    let lift = |q: glam::Vec2| {
        let on = u * q.x + v * q.y;
        let along = plane.dot(outward);
        let t = if along.abs() < 1e-4 {
            first.dot(outward)
        } else {
            plane.dot(first - on) / along
        };
        on + outward * t
    };
    // Outlines that overlap, or one crossing itself, filled once: each triangle cut back to
    // what the triangles before it do not cover.
    let mut out = Vec::new();
    let mut done: Vec<[glam::Vec2; 3]> = Vec::new();
    for triangle in filled {
        let mut pieces = vec![triangle.to_vec()];
        for before in &done {
            pieces = pieces
                .into_iter()
                .flat_map(|piece| {
                    if polygon_area(&clip_inside(&piece, before)) < 1e-9 {
                        vec![piece]
                    } else {
                        outside(&piece, before)
                    }
                })
                .collect();
        }
        for piece in pieces {
            for k in 1..piece.len().saturating_sub(1) {
                let t = [piece[0], piece[k], piece[k + 1]];
                if (t[1] - t[0]).perp_dot(t[2] - t[0]).abs() < 1e-10 {
                    continue;
                }
                let [a, b, c] = t.map(lift);
                let face = (b - a).cross(c - a);
                out.push(if face.dot(outward) >= 0.0 {
                    [a, b, c]
                } else {
                    [a, c, b]
                });
            }
        }
        done.push(triangle);
    }
    out
}

/// A simple polygon's triangles by ear clipping, as indices into its points; what is left
/// when no ear is found, as of a polygon that crosses itself, a fan from its first point.
fn ear_clip(points: &[glam::Vec2]) -> Vec<[usize; 3]> {
    let twice_area: f32 = (0..points.len())
        .map(|i| points[i].perp_dot(points[(i + 1) % points.len()]))
        .sum();
    let mut left: Vec<usize> = (0..points.len()).collect();
    if twice_area < 0.0 {
        left.reverse();
    }
    let cross =
        |a: usize, b: usize, c: usize| (points[b] - points[a]).perp_dot(points[c] - points[a]);
    let inside = |p: glam::Vec2, [a, b, c]: [usize; 3]| {
        let (pa, pb, pc) = (points[a], points[b], points[c]);
        (pb - pa).perp_dot(p - pa) >= 0.0
            && (pc - pb).perp_dot(p - pb) >= 0.0
            && (pa - pc).perp_dot(p - pc) >= 0.0
    };
    let mut out = Vec::new();
    while left.len() > 3 {
        let n = left.len();
        let ear = (0..n).find(|&i| {
            let t = [left[(i + n - 1) % n], left[i], left[(i + 1) % n]];
            cross(t[0], t[1], t[2]) > 1e-12
                && left.iter().filter(|&&k| !t.contains(&k)).all(|&k| {
                    !inside(points[k], t) || points[k] == points[t[0]] || points[k] == points[t[2]]
                })
        });
        let Some(i) = ear else {
            // Collinear points left, or one crossing itself: a flat corner first, else a fan.
            if let Some(i) = (0..n)
                .find(|&i| cross(left[(i + n - 1) % n], left[i], left[(i + 1) % n]).abs() <= 1e-12)
            {
                left.remove(i);
                continue;
            }
            for k in 1..n - 1 {
                out.push([left[0], left[k], left[k + 1]]);
            }
            return out;
        };
        out.push([left[(i + n - 1) % n], left[i], left[(i + 1) % n]]);
        left.remove(i);
    }
    if left.len() == 3 && cross(left[0], left[1], left[2]).abs() > 1e-12 {
        out.push([left[0], left[1], left[2]]);
    }
    out
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
    // A modelled piece carries its own colour: copies of it vary only a little, or a line
    // of them reads in bands.
    let shade = 1.0 - look.shade * 0.3 * layout::unit(hash(&[seed, 3]));
    let colour = look.tint.map(|c| c * shade);
    // Each corner in the line's own frame (along it, up, across), its normal there and its
    // place in the atlas: cut there, and only then seated, so a cut over a crease is cut
    // where the module is, not across the bend.
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
            Corner {
                q,
                n,
                uv: mesh.uvs.get(i).copied().unwrap_or([0.0; 2]),
                world,
                normal,
            }
        })
        .collect();
    let crease = valley(&line.seat);
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
            // Plumb along a slope: through the line's end on its deck, leaning as it rises.
            let by = Vec3::new(1.0, line.plumb, 0.0);
            let (kept, on_plane) = cut_by(&polygon, by, plane - line.plumb * line.lift, keep_above);
            if on_plane.len() == 2 {
                cuts[end].push((on_plane[0].world, on_plane[1].world, on_plane[0].uv));
            }
            polygon = kept;
            if polygon.len() < 3 {
                break;
            }
        }
        if polygon.len() < 3 {
            continue;
        }
        // In a valley, each half on its own plane: a face across the crease is split along
        // it, as each half is seated by its own plane.
        let halves = if crease {
            vec![
                cut_frame(&polygon, 2, 0.0, false).0,
                cut_frame(&polygon, 2, 0.0, true).0,
            ]
        } else {
            vec![polygon]
        };
        for polygon in halves.into_iter().filter(|p| p.len() >= 3) {
            // Seated again from the frame in a valley, whose halves bend at the crease; else
            // as cut, along the seated faces.
            let placed: Vec<(Vec3, Vec3, [f32; 2])> = polygon
                .iter()
                .map(|c| {
                    let (p, n) = if crease {
                        seat(line, axis, c.q, c.n)
                    } else {
                        (c.world, c.normal)
                    };
                    (p, n, c.uv)
                })
                .collect();
            let indices: Vec<u32> = placed
                .iter()
                .map(|c| out.corner(c.0, c.1, c.2, colour))
                .collect();
            for k in 1..indices.len() - 1 {
                let [i, j, l] = [indices[0], indices[k], indices[k + 1]];
                let (pa, pb, pc) = (placed[0].0, placed[k].0, placed[k + 1].0);
                let face = (pb - pa).cross(pc - pa);
                if face.length_squared() < 1e-14 {
                    continue;
                }
                let normal = placed[0].1 + placed[k].1 + placed[k + 1].1;
                if face.dot(normal) >= 0.0 {
                    out.indices.extend([i, j, l]);
                } else {
                    out.indices.extend([i, l, j]);
                }
            }
        }
    }
}

/// A corner of a modelled piece as it is laid: in the line's own frame (along it, up,
/// across) with its normal there, its place in the atlas, and where it is seated.
#[derive(Clone, Copy, Debug)]
struct Corner {
    q: Vec3,
    n: Vec3,
    uv: [f32; 2],
    world: Vec3,
    normal: Vec3,
}

/// A polygon of corners cut by the plane where one of the frame's axes is `at`, keeping what
/// is above it or below; and the corners made where it is cut, each between two corners in
/// the frame and as seated.
fn cut_frame(
    polygon: &[Corner],
    axis: usize,
    at: f32,
    keep_above: bool,
) -> (Vec<Corner>, Vec<Corner>) {
    let mut by = Vec3::ZERO;
    by[axis] = 1.0;
    cut_by(polygon, by, at, keep_above)
}

/// A polygon of corners cut by the plane in the frame where `q · by` is `at`, as
/// [`cut_frame`] cuts by one axis: a plumb end along a slope leans along it as it goes up.
fn cut_by(polygon: &[Corner], by: Vec3, at: f32, keep_above: bool) -> (Vec<Corner>, Vec<Corner>) {
    let f = |c: &Corner| c.q.dot(by);
    let inside = |c: &Corner| {
        if keep_above { f(c) >= at } else { f(c) <= at }
    };
    let mut kept = Vec::new();
    let mut on_plane = Vec::new();
    for i in 0..polygon.len() {
        let (p, q) = (&polygon[i], &polygon[(i + 1) % polygon.len()]);
        if inside(p) {
            kept.push(*p);
        }
        if inside(p) != inside(q) {
            let t = ((at - f(p)) / (f(q) - f(p))).clamp(0.0, 1.0);
            let place = p.q.lerp(q.q, t);
            let cut = Corner {
                q: place,
                n: p.n.lerp(q.n, t).normalize_or(p.n),
                uv: [
                    p.uv[0] + (q.uv[0] - p.uv[0]) * t,
                    p.uv[1] + (q.uv[1] - p.uv[1]) * t,
                ],
                world: p.world.lerp(q.world, t),
                normal: p.normal.lerp(q.normal, t).normalize_or(p.normal),
            };
            kept.push(cut);
            on_plane.push(cut);
        }
    }
    (kept, on_plane)
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
    let k = look.repeat.max(1.0);
    let (count, stretch) = fit(span, module.length * k);
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
            (lo, hi, line.capped.map(|capped| capped && cap_length > 0.0)),
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
        let start = copy as f32 * module.length * k;
        // A modelled piece is one model, so every other copy is turned end for end (mirrored
        // along it) and its repeat reads half as often.
        place(
            mesh,
            (
                lead[0] + start * stretch,
                Vec3::new(stretch * k, scale[0], scale[1]),
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
    if line.mitre.iter().any(Option::is_some) {
        for (end, mitre) in line.mitre.into_iter().enumerate() {
            if let Some(out) = mitre {
                let at = if end == 0 { line.from } else { line.to };
                mitred(&mut body, &mut ends, line, at, out, look.tint);
            }
        }
        // Only the corners the faces left use.
        for mesh in [&mut body, &mut ends] {
            let mut keep: Vec<Option<u32>> = vec![None; mesh.positions.len()];
            let mut kept = Indexed::default();
            for k in 0..mesh.indices.len() {
                let i = mesh.indices[k] as usize;
                let at = *keep[i].get_or_insert_with(|| {
                    kept.corner(
                        mesh.positions[i],
                        mesh.normals[i],
                        mesh.uvs[i],
                        mesh.colours[i],
                    )
                });
                kept.indices.push(at);
            }
            *mesh = kept;
        }
    }
    // Seated over a crease, a modelled piece's faces bend with it: two that meet at a
    // corner, nearly in one plane as modelled, may come to lie in one, the one over the other.
    if made && matches!(line.seat, Seat::Crease { .. }) {
        unfold(&mut body);
    }
    // A modelled piece is in its kit's atlas, whatever it stands for; one made from
    // numbers in its material, its ends in theirs.
    if modelled(module) {
        let atlas = if module.shared {
            Material::Trim
        } else {
            Material::Piece
        };
        put(surfaces, atlas, body);
        put(surfaces, atlas, ends);
    } else {
        put(surfaces, look.material, body);
        if let Some(material) = look.ends {
            put(surfaces, material, ends);
        }
    }
}

/// Faces of a laid piece that meet at a corner and lie one over the other in one plane,
/// looking the same way: the smaller cut back to what the larger leaves of it, its new
/// corners' normals, atlas places and colours taken from its own.
fn unfold(mesh: &mut Indexed) {
    let key = |p: Vec3| (p * 1e4).round().as_ivec3().to_array();
    let face = |m: &Indexed, t: usize| -> [Vec3; 3] {
        [0, 1, 2].map(|k| m.positions[m.indices[t * 3 + k] as usize])
    };
    let count = mesh.indices.len() / 3;
    let mut at: std::collections::HashMap<[i32; 3], Vec<usize>> = Default::default();
    for t in 0..count {
        for p in face(mesh, t) {
            let list = at.entry(key(p)).or_default();
            if list.last() != Some(&t) {
                list.push(t);
            }
        }
    }
    let mut corners: Vec<_> = at.into_iter().collect();
    corners.sort_by_key(|(k, _)| *k);
    let mut gone = vec![false; count];
    let mut added: Vec<u32> = Vec::new();
    for (_, faces) in corners {
        for x in 0..faces.len() {
            for y in x + 1..faces.len() {
                let (i, j) = (faces[x], faces[y]);
                if gone[i] || gone[j] {
                    continue;
                }
                let (fi, fj) = (face(mesh, i), face(mesh, j));
                let normal = |f: &[Vec3; 3]| (f[1] - f[0]).cross(f[2] - f[0]);
                let (ni, nj) = (normal(&fi), normal(&fj));
                let (ai, aj) = (ni.length(), nj.length());
                if ai < 1e-12 || aj < 1e-12 || ni.dot(nj) < 0.999 * ai * aj {
                    continue;
                }
                // The larger as the plane, the smaller the one over it.
                let (big, small, s) = if ai >= aj { (fi, fj, j) } else { (fj, fi, i) };
                let n = normal(&big).normalize();
                let d = n.dot(big[0]);
                if small.iter().any(|p| (n.dot(*p) - d).abs() > COPLANAR_M) {
                    continue;
                }
                let flat = flatten(n);
                let (big_flat, small_flat) = (big.map(&flat), small.map(&flat));
                if polygon_area(&clip_inside(&small_flat, &big_flat)) < 1e-6 {
                    continue;
                }
                gone[s] = true;
                let ids = [0, 1, 2].map(|k| mesh.indices[s * 3 + k] as usize);
                let whole = (small_flat[1] - small_flat[0]).perp_dot(small_flat[2] - small_flat[0]);
                for piece in outside(&small_flat, &big_flat) {
                    let new: Vec<u32> = piece
                        .iter()
                        .map(|&q| {
                            let wb =
                                (q - small_flat[0]).perp_dot(small_flat[2] - small_flat[0]) / whole;
                            let wc =
                                (small_flat[1] - small_flat[0]).perp_dot(q - small_flat[0]) / whole;
                            let w = [1.0 - wb - wc, wb, wc];
                            let mix = |v: [Vec3; 3]| v[0] * w[0] + v[1] * w[1] + v[2] * w[2];
                            let p = mix(ids.map(|c| mesh.positions[c]));
                            let nn = mix(ids.map(|c| mesh.normals[c])).normalize_or(n);
                            let uv =
                                mix(ids.map(|c| Vec3::new(mesh.uvs[c][0], mesh.uvs[c][1], 0.0)));
                            let colour = mix(ids.map(|c| Vec3::from_array(mesh.colours[c])));
                            mesh.corner(p, nn, [uv.x, uv.y], colour.to_array())
                        })
                        .collect();
                    for k in 1..new.len().saturating_sub(1) {
                        let t = [new[0], new[k], new[k + 1]];
                        let [a, b, c] = t.map(|v| mesh.positions[v as usize]);
                        let f = (b - a).cross(c - a);
                        if f.length_squared() < 1e-14 {
                            continue;
                        }
                        let along: Vec3 = t.iter().map(|&v| mesh.normals[v as usize]).sum();
                        if f.dot(along) >= 0.0 {
                            added.extend(t);
                        } else {
                            added.extend([t[0], t[2], t[1]]);
                        }
                    }
                }
            }
        }
    }
    if !gone.iter().any(|&g| g) {
        return;
    }
    let old = std::mem::take(&mut mesh.indices);
    for (t, chunk) in old.chunks_exact(3).enumerate() {
        if !gone[t] {
            mesh.indices.extend_from_slice(chunk);
        }
    }
    mesh.indices.extend(added);
}

/// How far apart two faces of a laid piece may be and still lie in one plane, in metres.
const COPLANAR_M: f32 = 0.0015;

/// How far inside the edges a crease's module is mitred to it is cut, in metres: off the
/// plane of the covering's own cut edge, which would fight its closing face.
const MITRE_INSET_M: f32 = 0.01;

/// A module laid over a crease cut at an end along the edges of its planes ([`Line::mitre`]):
/// each triangle on a plane's half kept to the inside of that plane's edge through `at`,
/// `out` the way out of each, and each cut closed.
fn mitred(
    body: &mut Indexed,
    ends: &mut Indexed,
    line: &Line,
    at: Vec3,
    out: [Vec3; 2],
    colour: [f32; 3],
) {
    let Seat::Crease { into, .. } = line.seat else {
        return;
    };
    // Which half a point is on: toward the second plane from the first.
    let toward = into[1] - into[0];
    let mut cuts: [Vec<(Vec3, Vec3)>; 2] = [Vec::new(), Vec::new()];
    // Over a hip the roof is inside both edges, so all of it is cut by both; in a valley
    // it is inside either, so each half by its own.
    let passes: &[Option<usize>] = if valley(&line.seat) {
        &[None]
    } else {
        &[Some(0), Some(1)]
    };
    for (pass, in_body) in passes
        .iter()
        .flat_map(|&pass| [(pass, true), (pass, false)])
    {
        let (mesh, keep_cuts): (&mut Indexed, bool) = if in_body {
            (&mut *body, true)
        } else {
            (&mut *ends, false)
        };
        let old = std::mem::take(&mut mesh.indices);
        for t in old.chunks_exact(3) {
            let corners = [t[0], t[1], t[2]];
            let middle = corners
                .iter()
                .map(|&i| mesh.positions[i as usize])
                .sum::<Vec3>()
                / 3.0;
            let half = pass.unwrap_or(usize::from((middle - line.from).dot(toward) > 0.0));
            let way = out[half];
            let sides = corners.map(|i| (mesh.positions[i as usize] - at).dot(way) + MITRE_INSET_M);
            if sides.iter().all(|&s| s <= 0.0) {
                mesh.indices.extend(corners);
                continue;
            }
            if sides.iter().all(|&s| s >= 0.0) {
                continue;
            }
            // Cut along the edge: the corners inside kept, a new one where each side crosses.
            let mut kept: Vec<u32> = Vec::new();
            let mut on: Vec<Vec3> = Vec::new();
            for k in 0..3 {
                let (a, b) = (corners[k], corners[(k + 1) % 3]);
                let (sa, sb) = (sides[k], sides[(k + 1) % 3]);
                if sa <= 0.0 {
                    kept.push(a);
                }
                if (sa <= 0.0) != (sb <= 0.0) {
                    let f = sa / (sa - sb);
                    let (a, b) = (a as usize, b as usize);
                    let p = mesh.positions[a].lerp(mesh.positions[b], f);
                    let n = mesh.normals[a]
                        .lerp(mesh.normals[b], f)
                        .normalize_or(mesh.normals[a]);
                    let uv = [
                        mesh.uvs[a][0] + (mesh.uvs[b][0] - mesh.uvs[a][0]) * f,
                        mesh.uvs[a][1] + (mesh.uvs[b][1] - mesh.uvs[a][1]) * f,
                    ];
                    let c = mesh.colours[a];
                    kept.push(mesh.corner(p, n, uv, c));
                    on.push(p);
                }
            }
            for k in 1..kept.len().saturating_sub(1) {
                // Wound the way its corners' normals look, as every laid face is.
                let t = [kept[0], kept[k], kept[k + 1]];
                let [pa, pb, pc] = t.map(|i| mesh.positions[i as usize]);
                let normal: Vec3 = t.iter().map(|&i| mesh.normals[i as usize]).sum();
                if (pb - pa).cross(pc - pa).dot(normal) >= 0.0 {
                    mesh.indices.extend(t);
                } else {
                    mesh.indices.extend([t[0], t[2], t[1]]);
                }
            }
            if keep_cuts && on.len() == 2 {
                cuts[half].push((on[0], on[1]));
            }
        }
    }
    for (half, segments) in cuts.iter().enumerate() {
        let uv = [0.5, 0.5];
        for face in close(segments, out[half]) {
            // Over a hip, the face closing one cut kept inside the other edge too.
            let mut polygon = face.to_vec();
            if passes.len() > 1 {
                let way = out[1 - half];
                let side = |p: Vec3| (p - at).dot(way) + MITRE_INSET_M;
                let mut kept = Vec::new();
                for k in 0..polygon.len() {
                    let (p, q) = (polygon[k], polygon[(k + 1) % polygon.len()]);
                    if side(p) <= 0.0 {
                        kept.push(p);
                    }
                    if (side(p) <= 0.0) != (side(q) <= 0.0) {
                        kept.push(p.lerp(q, side(p) / (side(p) - side(q))));
                    }
                }
                polygon = kept;
            }
            for k in 1..polygon.len().saturating_sub(1) {
                let [a, b, c] = [polygon[0], polygon[k], polygon[k + 1]];
                let face = (b - a).cross(c - a);
                if face.length_squared() < 1e-12 {
                    continue;
                }
                let (b, c) = if face.dot(out[half]) >= 0.0 {
                    (b, c)
                } else {
                    (c, b)
                };
                let corners = [a, b, c].map(|p| ends.corner(p, out[half], uv, colour));
                ends.indices.extend(corners);
            }
        }
    }
}

/// How far short of a line's stopped end a modelled piece with no cap stops, in metres.
const END_INSET_M: f32 = 0.015;

/// How many times its length each copy of a module is drawn far off (a ridge's caps, an
/// eave's butts, a hip's): a ridge tile 30 cm long is a few pixels at 60 m and more, so
/// copies twice as long read the same and a line has half as many.
pub const FAR_REPEAT: f32 = 2.0;

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
    // Each corner in the world, with its normal, and where it falls on its texture; and in
    // the line's frame, to split a face across a crease.
    let mut frame: Vec<(Vec3, Vec3)> = Vec::with_capacity(mesh.positions.len());
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
            frame.push((q, n));
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
        // In a valley, a face across it split along it, each half seated on its own plane.
        let across = [a, b, c].map(|i| frame[i].0.z);
        if valley(&line.seat)
            && across.iter().any(|&z| z < -1e-5)
            && across.iter().any(|&z| z > 1e-5)
        {
            let polygon: Vec<Corner> = [a, b, c]
                .iter()
                .map(|&i| Corner {
                    q: frame[i].0,
                    n: frame[i].1,
                    uv: corners[i].2,
                    world: corners[i].0,
                    normal: corners[i].1,
                })
                .collect();
            for keep_above in [false, true] {
                let half = cut_frame(&polygon, 2, 0.0, keep_above).0;
                let placed: Vec<(Vec3, Vec3)> =
                    half.iter().map(|c| seat(line, axis, c.q, c.n)).collect();
                for k in 1..half.len().saturating_sub(1) {
                    let tri = [0, k, k + 1];
                    let face = (placed[tri[1]].0 - placed[tri[0]].0)
                        .cross(placed[tri[2]].0 - placed[tri[0]].0);
                    if face.length_squared() < 1e-14 {
                        continue;
                    }
                    let normal: Vec3 = tri.iter().map(|&i| placed[i].1).sum();
                    let order = if face.dot(normal) >= 0.0 {
                        tri
                    } else {
                        [tri[0], tri[2], tri[1]]
                    };
                    for i in order {
                        let corner = out.corner(placed[i].0, placed[i].1, half[i].uv, colour);
                        out.indices.push(corner);
                    }
                }
            }
            continue;
        }
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

/// A kit's modules made fit to lay, as an engine's import cleans a mesh before building it
/// (Unreal's mesh build drops degenerate and duplicate triangles): every mesh of each
/// ([`clean`]). The kit does it once, as it reads its pieces.
pub fn clean_all(pieces: &mut crate::dress::Pieces) {
    for module in pieces.modules.values_mut() {
        for mesh in module
            .near
            .iter_mut()
            .chain(module.far.iter_mut())
            .chain(module.cap.iter_mut())
        {
            clean(mesh);
        }
    }
}

/// How nearly in one plane a sliver folded back over its neighbour lies: within 45 degrees
/// of it, as no surface of a model folds back on itself with its normals smooth across.
const FOLD: f32 = 1.0;

/// A modelled mesh made fit to lay: without triangles of no area, a triangle drawn twice (a
/// face made double-sided, both copies of which a layer turns to look one way), or a sliver
/// folded back over its neighbour across their shared edge, looking the same way by its
/// corners' normals, which laid lies in its neighbour's plane over it and flickers. The
/// smaller of a fold goes; its neighbour covers where it was.
pub fn clean(mesh: &mut Stone) {
    let key = |p: [f32; 3]| p.map(|v| libm::roundf(v * 1e5) as i64);
    let point = |i: u32| Vec3::from_array(mesh.positions[i as usize]);
    let normal = |t: &[u32; 3]| -> Vec3 {
        t.iter()
            .filter_map(|&i| mesh.normals.get(i as usize))
            .map(|n| Vec3::from_array(*n))
            .sum()
    };
    let area = |t: &[u32; 3]| {
        let [a, b, c] = t.map(point);
        (b - a).cross(c - a).length() * 0.5
    };
    let in_range = |t: &[u32; 3]| t.iter().all(|&i| (i as usize) < mesh.positions.len());
    // Each triangle once, by its corners' places, and with some area.
    let mut seen = std::collections::HashSet::new();
    let mut keep: Vec<bool> = mesh
        .triangles
        .iter()
        .map(|t| {
            if !in_range(t) || area(t) < 1e-10 {
                return false;
            }
            let mut corners = t.map(|i| key(mesh.positions[i as usize]));
            corners.sort();
            seen.insert(corners)
        })
        .collect();
    // Folds: two triangles across an edge, the one's far corner on the same side of the edge
    // as the other's and nearly in its plane.
    let mut edges: std::collections::HashMap<[[i64; 3]; 2], Vec<usize>> = Default::default();
    for (k, t) in mesh.triangles.iter().enumerate() {
        if !keep[k] {
            continue;
        }
        for e in 0..3 {
            let mut edge = [t[e], t[(e + 1) % 3]].map(|i| key(mesh.positions[i as usize]));
            edge.sort();
            edges.entry(edge).or_default().push(k);
        }
    }
    let far_corner = |t: &[u32; 3], edge: &[[i64; 3]; 2]| {
        t.iter()
            .copied()
            .find(|&i| !edge.contains(&key(mesh.positions[i as usize])))
    };
    let mut edges: Vec<_> = edges.into_iter().collect();
    edges.sort_by_key(|edge| edge.0);
    for (edge, on) in edges {
        for x in 0..on.len() {
            for y in x + 1..on.len() {
                let (i, j) = (on[x], on[y]);
                if !keep[i] || !keep[j] {
                    continue;
                }
                let (ti, tj) = (mesh.triangles[i], mesh.triangles[j]);
                // The larger as the plane, the smaller the one that may fold over it.
                let (big, small, gone) = if area(&ti) >= area(&tj) {
                    (ti, tj, j)
                } else {
                    (tj, ti, i)
                };
                let (Some(a), Some(b)) = (far_corner(&big, &edge), far_corner(&small, &edge))
                else {
                    continue;
                };
                let ends: Vec<Vec3> = big.iter().filter(|&&k| k != a).map(|&k| point(k)).collect();
                let [e0, e1] = [ends[0], ends[1]];
                let n = (e1 - e0).cross(point(a) - e0).normalize_or_zero();
                let mut side = n.cross(e1 - e0).normalize_or_zero();
                if side.dot(point(a) - e0) < 0.0 {
                    side = -side;
                }
                let (h, z) = (side.dot(point(b) - e0), n.dot(point(b) - e0));
                if h > 0.0 && z.abs() < h * FOLD && normal(&big).dot(normal(&small)) > 0.0 {
                    keep[gone] = false;
                }
            }
        }
    }
    let mut k = 0;
    mesh.triangles.retain(|_| {
        k += 1;
        keep[k - 1]
    });
    unlap(mesh);
}

/// How far apart two faces of a modelled mesh may be and still lie in one plane, in metres,
/// and how nearly they must look the same way.
const PLANE_M: f32 = 0.0015;
const ONE_WAY: f32 = 0.999;

/// Each triangle of a modelled mesh that lies over a larger one in its plane, looking the same
/// way by its corners' normals, cut back to what of it the larger does not cover, as two
/// faces of a model meet where they should only touch: a cap's end face triangulated across
/// itself, two layers of a model run into one. What is left of it is cut into convex pieces,
/// its new corners' normals and texture places taken from the triangle's own.
fn unlap(mesh: &mut Stone) {
    let point = |m: &Stone, i: u32| Vec3::from_array(m.positions[i as usize]);
    let facing = |m: &Stone, t: &[u32; 3]| {
        let [a, b, c] = t.map(|i| point(m, i));
        let n = (b - a).cross(c - a);
        let along: Vec3 = t
            .iter()
            .filter_map(|&i| m.normals.get(i as usize))
            .map(|n| Vec3::from_array(*n))
            .sum();
        let n = n.normalize_or_zero();
        if n.dot(along) < 0.0 { -n } else { n }
    };
    let area = |m: &Stone, t: &[u32; 3]| {
        let [a, b, c] = t.map(|i| point(m, i));
        (b - a).cross(c - a).length() * 0.5
    };
    for _ in 0..4 {
        let mut changed = false;
        let mut k = 0;
        while k < mesh.triangles.len() {
            let small = mesh.triangles[k];
            let n = facing(mesh, &small);
            let small_area = area(mesh, &small);
            let cover = (0..mesh.triangles.len()).find(|&j| {
                if j == k {
                    return false;
                }
                let big = mesh.triangles[j];
                let big_area = area(mesh, &big);
                if big_area < small_area || (big_area == small_area && j > k) {
                    return false;
                }
                let m = facing(mesh, &big);
                if m.dot(n) < ONE_WAY {
                    return false;
                }
                let d = m.dot(point(mesh, big[0]));
                if small
                    .iter()
                    .any(|&i| (m.dot(point(mesh, i)) - d).abs() > PLANE_M)
                {
                    return false;
                }
                let flat = flatten(m);
                let tri = |t: [u32; 3]| t.map(|i| flat(point(mesh, i)));
                polygon_area(&clip_inside(&tri(small), &tri(big))) > 1e-7
            });
            let Some(j) = cover else {
                k += 1;
                continue;
            };
            let big = mesh.triangles[j];
            let m = facing(mesh, &big);
            let flat = flatten(m);
            let big_flat = big.map(|i| flat(point(mesh, i)));
            let small_flat = small.map(|i| flat(point(mesh, i)));
            let pieces = outside(&small_flat, &big_flat);
            // The new corners, each from the small triangle's by where it falls in it.
            let corners = small.map(|i| i as usize);
            let weights = |p: glam::Vec2| {
                let [a, b, c] = small_flat;
                let whole = (b - a).perp_dot(c - a);
                if whole.abs() < 1e-14 {
                    return [1.0, 0.0, 0.0];
                }
                let wb = (p - a).perp_dot(c - a) / whole;
                let wc = (b - a).perp_dot(p - a) / whole;
                [1.0 - wb - wc, wb, wc]
            };
            let mut added = Vec::new();
            for piece in pieces {
                let ids: Vec<u32> = piece
                    .iter()
                    .map(|&p| {
                        let w = weights(p);
                        let mix3 = |v: &Vec<[f32; 3]>| {
                            let mut out = Vec3::ZERO;
                            for (c, w) in corners.iter().zip(w) {
                                out += Vec3::from_array(v.get(*c).copied().unwrap_or([0.0; 3])) * w;
                            }
                            out
                        };
                        let position = mix3(&mesh.positions);
                        let normal = mix3(&mesh.normals).normalize_or(n);
                        mesh.positions.push(position.to_array());
                        mesh.normals.push(normal.to_array());
                        if !mesh.uvs.is_empty() {
                            let mut uv = [0.0; 2];
                            for (c, w) in corners.iter().zip(w) {
                                let at = mesh.uvs.get(*c).copied().unwrap_or([0.0; 2]);
                                uv[0] += at[0] * w;
                                uv[1] += at[1] * w;
                            }
                            mesh.uvs.push(uv);
                        }
                        (mesh.positions.len() - 1) as u32
                    })
                    .collect();
                for i in 1..ids.len().saturating_sub(1) {
                    let t = [ids[0], ids[i], ids[i + 1]];
                    if area(mesh, &t) < 1e-10 {
                        continue;
                    }
                    // Wound as the triangle it is cut from.
                    let [a, b, c] = t.map(|i| point(mesh, i));
                    let [sa, sb, sc] = small.map(|i| point(mesh, i));
                    let same = (b - a).cross(c - a).dot((sb - sa).cross(sc - sa)) >= 0.0;
                    added.push(if same { t } else { [t[0], t[2], t[1]] });
                }
            }
            mesh.triangles.swap_remove(k);
            mesh.triangles.extend(added);
            changed = true;
        }
        if !changed {
            break;
        }
    }
}

/// A point in a plane square to `n`, by two axes in it.
fn flatten(n: Vec3) -> impl Fn(Vec3) -> glam::Vec2 {
    let u = n.any_orthonormal_vector();
    let v = n.cross(u);
    move |p| glam::Vec2::new(p.dot(u), p.dot(v))
}

fn polygon_area(polygon: &[glam::Vec2]) -> f32 {
    if polygon.len() < 3 {
        return 0.0;
    }
    (0..polygon.len())
        .map(|i| polygon[i].perp_dot(polygon[(i + 1) % polygon.len()]))
        .sum::<f32>()
        .abs()
        * 0.5
}

/// A convex polygon clipped to one side of a line through `a` and `b`: the left, or the right.
fn clip_side(polygon: &[glam::Vec2], a: glam::Vec2, b: glam::Vec2, left: bool) -> Vec<glam::Vec2> {
    let side = |p: glam::Vec2| {
        let s = (b - a).perp_dot(p - a);
        if left { s } else { -s }
    };
    let mut out = Vec::with_capacity(polygon.len() + 2);
    for i in 0..polygon.len() {
        let (p, q) = (polygon[i], polygon[(i + 1) % polygon.len()]);
        let (sp, sq) = (side(p), side(q));
        if sp >= 0.0 {
            out.push(p);
        }
        if (sp >= 0.0) != (sq >= 0.0) {
            out.push(p + (q - p) * (sp / (sp - sq)));
        }
    }
    out
}

/// A triangle wound counter-clockwise.
fn counter_clockwise(t: &[glam::Vec2; 3]) -> [glam::Vec2; 3] {
    if (t[1] - t[0]).perp_dot(t[2] - t[0]) < 0.0 {
        [t[0], t[2], t[1]]
    } else {
        *t
    }
}

/// What of a convex polygon lies inside a triangle.
fn clip_inside(polygon: &[glam::Vec2], triangle: &[glam::Vec2; 3]) -> Vec<glam::Vec2> {
    let t = counter_clockwise(triangle);
    let mut rest = polygon.to_vec();
    for e in 0..3 {
        rest = clip_side(&rest, t[e], t[(e + 1) % 3], true);
        if rest.len() < 3 {
            return Vec::new();
        }
    }
    rest
}

/// What of a convex polygon lies outside a triangle, as convex pieces: beyond its first edge,
/// then within that and beyond its second, then within both and beyond its third.
fn outside(polygon: &[glam::Vec2], triangle: &[glam::Vec2; 3]) -> Vec<Vec<glam::Vec2>> {
    let t = counter_clockwise(triangle);
    let mut rest = polygon.to_vec();
    let mut out = Vec::new();
    for e in 0..3 {
        let beyond = clip_side(&rest, t[e], t[(e + 1) % 3], false);
        if polygon_area(&beyond) > 1e-9 {
            out.push(beyond);
        }
        rest = clip_side(&rest, t[e], t[(e + 1) % 3], true);
        if rest.len() < 3 {
            break;
        }
    }
    out
}

/// Whether a module is modelled, read by its own UVs in its kit's atlas (`Material::Piece`),
/// rather than made from numbers and mapped by where it lies.
pub fn modelled(module: &Module) -> bool {
    module.near.first().is_some_and(|mesh| !mesh.uvs.is_empty())
}

/// Whether a module sits over a crease whose planes close over it, a valley: going into the
/// one plane from the crease rises over the other.
fn valley(seat: &Seat) -> bool {
    matches!(seat, Seat::Crease { up, into } if into[0].dot(up[1]) > 1e-3)
}

/// A point and a normal in a line's frame (along it, up, across) in the world.
fn seat(line: &Line, axis: Vec3, q: Vec3, n: Vec3) -> (Vec3, Vec3) {
    let along = line.from + axis * q.x;
    let q = Vec3::new(q.x, q.y + line.lift, q.z);
    match line.seat {
        Seat::Plane { up, out } => (
            along + up * q.y + out * q.z,
            (axis * n.x + up * n.y + out * n.z).normalize_or(up),
        ),
        Seat::Crease { up, into } if valley(&line.seat) => {
            // In a valley, where the planes close over the crease: each half on its plane,
            // lifted off it along the line between the two planes' ups, by as much as keeps
            // it as far off its plane as modelled. Lifted along each plane's own up, as over a
            // ridge, the halves would cross over the crease and fold the section on itself.
            let rise = (up[0] + up[1]).normalize_or(Vec3::Y);
            let half = usize::from(q.z >= 0.0);
            let lift = rise / rise.dot(up[half]).max(0.2);
            let across = if half == 0 { -into[0] } else { into[1] };
            // A normal by the transform's cofactors: each axis's image crossed with the next.
            let normal =
                n.x * lift.cross(across) + n.y * across.cross(axis) + n.z * axis.cross(lift);
            let det = axis.dot(lift.cross(across));
            (
                along + across * q.z + lift * q.y,
                (normal * det.signum()).normalize_or(up[half]),
            )
        }
        Seat::Crease { up, into } => {
            // Its -Z half on the first plane, its +Z half on the second, its up turning
            // from one plane's to the other's within `blend` of the crease.
            let on = if q.z < 0.0 {
                into[0] * -q.z
            } else {
                into[1] * q.z
            };
            let blend = match line.crest {
                Some((wide, clear)) => {
                    let length = line.from.distance(line.to);
                    let k = [q.x, length - q.x]
                        .iter()
                        .zip(clear)
                        .map(|(&d, c)| {
                            if c <= 0.0 {
                                1.0
                            } else {
                                let t = ((d - c * 0.5) / (c * 0.5)).clamp(0.0, 1.0);
                                t * t * (3.0 - 2.0 * t)
                            }
                        })
                        .fold(1.0f32, f32::min);
                    line.blend + (wide - line.blend).max(0.0) * k
                }
                None => line.blend,
            };
            let t = if blend > 0.0 {
                let t = ((q.z / blend) * 0.5 + 0.5).clamp(0.0, 1.0);
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
            repeat: 1.0,
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
            plumb: 0.0,
            crest: None,
            lift: 0.0,
            mitre: [None; 2],
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
            plumb: 0.0,
            crest: None,
            lift: 0.0,
            mitre: [None; 2],
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
    fn a_board_along_a_slope_is_seated_square_to_it_and_cut_plumb_at_its_ends() {
        // A board modelled a metre long and 0.2 m wide, its face to +Z, its atlas laid on it
        // a metre to a unit, as a verge's is: laid up a slope at 45 degrees, square to it.
        let mut mesh = Stone::default();
        mesh.positions = vec![
            [0.0, 0.0, 0.01],
            [1.0, 0.0, 0.01],
            [1.0, 0.2, 0.01],
            [0.0, 0.2, 0.01],
        ];
        mesh.normals = vec![[0.0, 0.0, 1.0]; 4];
        mesh.triangles = vec![[0, 1, 2], [0, 2, 3]];
        mesh.uvs = mesh.positions.iter().map(|p| [p[0], p[1]]).collect();
        let module = Module {
            length: 1.0,
            near: vec![mesh],
            ..Module::default()
        };
        let s = std::f32::consts::FRAC_1_SQRT_2;
        let (along, up) = (Vec3::new(s, s, 0.0), Vec3::new(-s, s, 0.0));
        let line = Line {
            from: Vec3::ZERO,
            to: along * 1.4,
            seat: Seat::Plane { up, out: Vec3::Z },
            across: None,
            capped: [false; 2],
            seed: 3,
            blend: 0.0,
            // Along it for each metre up it, so its ends stand plumb: up · level / along · level.
            plumb: -1.0,
            crest: None,
            lift: 0.0,
            mitre: [None; 2],
        };
        let mut surfaces = Surfaces::sharing();
        lay(&module, &line, &look(), false, &mut surfaces);
        let made = &surfaces.0[&Material::Piece].shared;
        assert!(made.triangle_count() > 0);
        // Its texture is not sheared: a metre of it is a metre each way.
        for t in made.indices.chunks(3) {
            let [a, b, c] = [t[0], t[1], t[2]].map(|i| i as usize);
            let (p, q, r) = (made.positions[a], made.positions[b], made.positions[c]);
            let (u, v, w) = (made.uvs[a], made.uvs[b], made.uvs[c]);
            let (d1, d2) = ([v[0] - u[0], v[1] - u[1]], [w[0] - u[0], w[1] - u[1]]);
            let det = d1[0] * d2[1] - d1[1] * d2[0];
            if det.abs() < 1e-6 {
                continue;
            }
            let ju = ((q - p) * d2[1] - (r - p) * d1[1]) / det;
            let jv = ((r - p) * d1[0] - (q - p) * d2[0]) / det;
            assert!(
                (ju.length() - 1.0).abs() < 0.02 && (jv.length() - 1.0).abs() < 0.02,
                "sheared: {ju} {jv}"
            );
            assert!(ju.dot(jv).abs() < 0.02, "sheared: {ju} {jv}");
        }
        // Its ends cut plumb, through the line's ends: nothing past them level, its closing
        // faces too, and its top edge reaching the plumb line at its foot.
        let (start, end) = (0.0, 1.4 * s);
        assert!(
            made.positions
                .iter()
                .all(|p| p.x > start - 1e-4 && p.x < end + 1e-4),
            "past a plumb end"
        );
        assert!(
            made.positions
                .iter()
                .any(|p| p.x < start + 1e-3 && p.y > 0.2 * s - 1e-3),
            "its foot not cut plumb"
        );
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
            plumb: 0.0,
            crest: None,
            lift: 0.0,
            mitre: [None; 2],
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

    #[test]
    fn a_lining_in_a_valley_lies_on_both_planes_without_folding_over_the_crease() {
        // A top 0.2 m across in strips, a tenth of a metre up, laid in a valley along X: the
        // north slope falls to it from -Z, the south from +Z. Lifted along an up turning from
        // one plane's to the other's over the crease, as over a ridge, it folded on itself.
        let mut mesh = Stone::default();
        let strips = 10;
        for k in 0..=strips {
            let z = -0.1 + 0.2 * k as f32 / strips as f32;
            mesh.positions.extend([[0.0, 0.1, z], [1.0, 0.1, z]]);
            mesh.normals.extend([[0.0, 1.0, 0.0]; 2]);
        }
        for k in 0..strips as u32 {
            let (a, b) = (2 * k, 2 * k + 2);
            mesh.triangles.extend([[a, b, b + 1], [a, b + 1, a + 1]]);
        }
        let module = Module {
            length: 1.0,
            near: vec![mesh],
            ..Module::default()
        };
        let s = std::f32::consts::FRAC_1_SQRT_2;
        let line = Line {
            from: Vec3::ZERO,
            to: Vec3::X * 2.0,
            seat: Seat::Crease {
                up: [Vec3::new(0.0, s, s), Vec3::new(0.0, s, -s)],
                into: [Vec3::new(0.0, s, -s), Vec3::new(0.0, s, s)],
            },
            across: None,
            capped: [false; 2],
            seed: 1,
            blend: 0.1,
            plumb: 0.0,
            crest: None,
            lift: 0.0,
            mitre: [None; 2],
        };
        let mut surfaces = Surfaces::sharing();
        lay(&module, &line, &look(), false, &mut surfaces);
        let made = &surfaces.0[&Material::Board].shared;
        // Each corner a tenth of a metre off its own plane.
        for p in &made.positions {
            let plane = if p.z < -1e-5 {
                Vec3::new(0.0, s, s)
            } else if p.z > 1e-5 {
                Vec3::new(0.0, s, -s)
            } else {
                continue;
            };
            assert!((p.dot(plane) - 0.1).abs() < 1e-4, "{p}");
        }
        // Seen from above, no part of it over another.
        let faces: Vec<[Vec3; 3]> = made
            .indices
            .chunks(3)
            .map(|t| [0, 1, 2].map(|k| made.positions[t[k] as usize]))
            .collect();
        let (twice, _) = twice_covered(&faces, Vec3::Y);
        assert!(twice < 1e-6, "{twice}");
    }

    #[test]
    fn a_crease_is_mitred_to_the_edges_of_its_planes_at_an_end() {
        // A capping 0.2 m across down a hip, its planes falling to -Z and to +X, to the
        // corner where their eaves, z = 0 and x = 0, meet: cut square across the line it
        // stood out past both.
        let s = std::f32::consts::FRAC_1_SQRT_2;
        let (n0, n1) = (
            Vec3::new(0.0, s, -s).normalize(),
            Vec3::new(s, s, 0.0).normalize(),
        );
        let from = Vec3::new(-1.0, 1.0, 1.0);
        let to = Vec3::ZERO;
        let axis = (to - from).normalize();
        // Into each plane square to the line, falling below the other plane, as over a hip.
        let into = |n: Vec3, other: Vec3| {
            let d = n.cross(axis).normalize();
            if d.dot(other) < 0.0 { d } else { -d }
        };
        let (into0, into1) = (into(n0, n1), into(n1, n0));
        let line = Line {
            from,
            to,
            seat: Seat::Crease {
                up: [n0, n1],
                into: [into0, into1],
            },
            across: None,
            capped: [true, false],
            seed: 2,
            blend: 0.0,
            plumb: 0.0,
            crest: None,
            lift: 0.0,
            mitre: [None, Some([Vec3::new(0.0, 0.0, -1.0), Vec3::X])],
        };
        let mut surfaces = Surfaces::sharing();
        lay(&bar(), &line, &look(), false, &mut surfaces);
        let made = &surfaces.0[&Material::Board].shared;
        assert!(made.triangle_count() > 0);
        // Nothing drawn past either eave.
        for &i in &made.indices {
            let p = made.positions[i as usize];
            assert!(p.x <= 1e-4 && p.z >= -1e-4, "{p}");
        }
    }

    /// How much of the plane two lists of triangles in it cover twice over, and the area of
    /// all of them, measured flat in the plane square to `n`.
    fn twice_covered(triangles: &[[Vec3; 3]], n: Vec3) -> (f32, f32) {
        let flat = flatten(n);
        let flats: Vec<[glam::Vec2; 3]> = triangles.iter().map(|t| t.map(&flat)).collect();
        let mut twice = 0.0;
        for i in 0..flats.len() {
            for j in i + 1..flats.len() {
                twice += polygon_area(&clip_inside(&flats[i], &flats[j]));
            }
        }
        let all = flats.iter().map(|t| polygon_area(t)).sum();
        (twice, all)
    }

    /// The segments a cut along X makes through an outline in Y and Z, in order round it.
    fn outline(points: &[[f32; 2]]) -> Vec<(Vec3, Vec3)> {
        (0..points.len())
            .map(|i| {
                let [a, b] = [points[i], points[(i + 1) % points.len()]];
                (Vec3::new(0.5, a[0], a[1]), Vec3::new(0.5, b[0], b[1]))
            })
            .collect()
    }

    #[test]
    fn a_cut_through_a_section_that_is_not_convex_is_closed_once() {
        // A comb of three teeth, 0.3 across and 0.2 up, whose middle sees none of its teeth'
        // tips whole: a fan from its middle folded over itself; filled, it covers its area
        // once, every face looking out of the cut.
        let comb = [
            [0.0, 0.0],
            [0.0, 0.3],
            [0.2, 0.3],
            [0.2, 0.25],
            [0.05, 0.25],
            [0.05, 0.2],
            [0.2, 0.2],
            [0.2, 0.1],
            [0.05, 0.1],
            [0.05, 0.05],
            [0.2, 0.05],
            [0.2, 0.0],
        ];
        // Its segments in any order and either way round, as a mesh's triangles give them.
        let mut segments = outline(&comb);
        segments.reverse();
        segments.swap(0, 5);
        segments[3] = (segments[3].1, segments[3].0);
        let faces = close(&segments, Vec3::X);
        let (twice, all) = twice_covered(&faces, Vec3::X);
        let area = 0.2 * 0.3 - 2.0 * 0.15 * 0.05;
        assert!(twice < 1e-7, "{twice}");
        assert!((all - area).abs() < 1e-5, "{all} against {area}");
        for [a, b, c] in &faces {
            assert!((b - a).cross(c - a).x > 0.0);
        }
        // Two outlines that overlap, as two bundles of a ridge run into one another: their
        // union, once.
        let mut two = outline(&[[0.0, 0.0], [0.0, 0.2], [0.2, 0.2], [0.2, 0.0]]);
        two.extend(outline(&[[0.1, 0.1], [0.1, 0.3], [0.3, 0.3], [0.3, 0.1]]));
        let faces = close(&two, -Vec3::X);
        let (twice, all) = twice_covered(&faces, Vec3::X);
        assert!(twice < 1e-7, "{twice}");
        assert!((all - 0.07).abs() < 1e-5, "{all}");
    }

    #[test]
    fn a_modelled_module_is_closed_where_it_is_cut_but_not_under_its_cap() {
        // A box modelled a metre long, 0.1 up and 0.2 across, with a cap 0.05 long.
        let corners = |x: f32| [[x, 0.0, -0.1], [x, 0.1, -0.1], [x, 0.1, 0.1], [x, 0.0, 0.1]];
        let box_of = |from: f32, to: f32| {
            let mut mesh = Stone::default();
            let (a, b) = (corners(from), corners(to));
            for side in 0..4 {
                let (p, q) = (side, (side + 1) % 4);
                // Each side's normal out from the box's middle.
                let mid = |c: [f32; 3]| c[1] - 0.05;
                let base = mesh.positions.len() as u32;
                mesh.positions.extend([a[p], a[q], b[q], b[p]]);
                let out = Vec3::new(0.0, mid(a[p]) + mid(a[q]), a[p][2] + a[q][2]).normalize();
                mesh.normals.extend([out.to_array(); 4]);
                mesh.uvs
                    .extend([[0.0, 0.0], [0.1, 0.0], [0.1, 0.1], [0.0, 0.1]]);
                mesh.triangles
                    .extend([[base, base + 1, base + 2], [base, base + 2, base + 3]]);
            }
            mesh
        };
        let mut cap = box_of(0.0, 0.05);
        // The cap's own end, at its far end.
        let base = cap.positions.len() as u32;
        cap.positions.extend(corners(0.05));
        cap.normals.extend([[1.0, 0.0, 0.0]; 4]);
        cap.uvs.extend([[0.0, 0.0]; 4]);
        cap.triangles
            .extend([[base, base + 1, base + 2], [base, base + 2, base + 3]]);
        let module = Module {
            length: 1.0,
            near: vec![box_of(0.0, 1.0)],
            cap: Some(cap),
            ..Module::default()
        };
        let lay_on = |capped: [bool; 2]| {
            let line = Line {
                from: Vec3::ZERO,
                to: Vec3::X * 2.5,
                seat: Seat::Plane {
                    up: Vec3::Y,
                    out: Vec3::Z,
                },
                across: None,
                capped,
                seed: 7,
                blend: 0.0,
                plumb: 0.0,
                crest: None,
                lift: 0.0,
                mitre: [None; 2],
            };
            let mut surfaces = Surfaces::sharing();
            lay(&module, &line, &look(), false, &mut surfaces);
            let made = surfaces.0[&Material::Piece].shared.clone();
            // The faces looking along the line, by where along it they are.
            let mut ends: std::collections::BTreeMap<i32, f32> = Default::default();
            for t in made.indices.chunks(3) {
                let [a, b, c] = [0, 1, 2].map(|k| made.positions[t[k] as usize]);
                let face = (b - a).cross(c - a);
                if face.x.abs() > 0.99 * face.length() && face.length() > 0.0 {
                    *ends.entry(libm::roundf(a.x * 1000.0) as i32).or_default() +=
                        face.length() * 0.5;
                }
            }
            ends
        };
        // Cut at both ends and closed there, each once.
        let open = lay_on([false; 2]);
        assert_eq!(
            open.keys().copied().collect::<Vec<_>>(),
            vec![0, 2500],
            "{open:?}"
        );
        assert!(open.values().all(|a| (a - 0.02).abs() < 1e-4), "{open:?}");
        // Capped at its end: closed by the cap's end out at the line's end, nothing where the
        // cap meets the cut, inside.
        let capped = lay_on([false, true]);
        assert_eq!(
            capped.keys().copied().collect::<Vec<_>>(),
            vec![0, 2500],
            "{capped:?}"
        );
    }

    #[test]
    fn faces_of_a_laid_piece_folded_one_over_the_other_are_cut_back_to_cover_once() {
        // Two faces meeting at a corner, the smaller lying over the larger in its plane, as
        // a crease's bend leaves a crumple in a model: what is left covers their union once.
        let mut mesh = Indexed::default();
        for p in [
            Vec3::ZERO,
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::ZERO,
            Vec3::new(0.2, 0.0, 0.6),
            Vec3::new(0.6, 0.0, 0.6),
        ] {
            mesh.corner(p, Vec3::Y, [0.0; 2], [1.0; 3]);
        }
        mesh.indices = vec![0, 1, 2, 3, 4, 5];
        unfold(&mut mesh);
        let faces: Vec<[Vec3; 3]> = mesh
            .indices
            .chunks(3)
            .map(|t| [0, 1, 2].map(|k| mesh.positions[t[k] as usize]))
            .collect();
        let (twice, all) = twice_covered(&faces, Vec3::Y);
        assert!(twice < 1e-6, "{twice}");
        // Half a square, and of the other what lies past its diagonal x + z = 1: a triangle
        // 0.2 along z = 0.6 and 0.1 off it.
        assert!((all - (0.5 + 0.01)).abs() < 1e-4, "{all}");
        for [a, b, c] in faces {
            assert!((b - a).cross(c - a).y > 0.0);
        }
    }

    #[test]
    fn a_modelled_mesh_is_cleaned_of_doubled_faces_folds_and_faces_over_one_another() {
        let up = [0.0, 1.0, 0.0];
        let mut mesh = Stone {
            positions: vec![
                // A square, as two triangles, facing up.
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [1.0, 0.0, 1.0],
                [0.0, 0.0, 1.0],
                // A sliver folded back over the second along its diagonal.
                [0.6, 0.004, 0.5],
            ],
            normals: vec![up; 5],
            triangles: vec![
                [0, 2, 1],
                [0, 3, 2],
                // The first again, wound the other way: a face made double-sided.
                [0, 1, 2],
                // The fold: across the diagonal from the second, on its side.
                [0, 2, 4],
            ],
            uvs: vec![],
        };
        clean(&mut mesh);
        assert_eq!(mesh.triangles.len(), 2, "{:?}", mesh.triangles);
        // Two triangles crossing one another in one plane, as a cap's end face triangulated
        // across itself: what is left covers their union once.
        let mut crossed = Stone {
            positions: vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0],
                [1.0, 0.0, 1.0],
                [0.0, 0.0, 0.2],
                [1.0, 0.0, 0.2],
            ],
            normals: vec![up; 6],
            triangles: vec![[0, 2, 1], [4, 3, 5]],
            uvs: vec![],
        };
        clean(&mut crossed);
        let faces: Vec<[Vec3; 3]> = crossed
            .triangles
            .iter()
            .map(|t| t.map(|i| Vec3::from_array(crossed.positions[i as usize])))
            .collect();
        let (twice, all) = twice_covered(&faces, Vec3::Y);
        assert!(twice < 1e-6, "{twice}");
        // Half a square and 0.4 of one, overlapping by a triangle 0.8 across and 0.36 high.
        let union = 0.5 + 0.4 - 0.5 * 0.8 * (0.8 * 0.8 / 1.8);
        assert!((all - union).abs() < 1e-4, "{all} against {union}");
        for [a, b, c] in faces {
            assert!((b - a).cross(c - a).y > 0.0);
        }
    }
}
