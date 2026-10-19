//! Pieces made into surfaces by material, ready to draw: long members and a deck's planks by
//! [`timber`], the boards round openings as boxes their texture is mapped onto, a kit's
//! dress by [`dress`](crate::dress). Doors and windows are props,
//! and block kinds are drawn as their cubes' surface (`surface.rs`), so neither is here.
//!
//! [`timber`]: crate::timber

use crate::mesh::{Material, Surface, Surfaces};
use crate::relief::Relief;
use glam::Vec3;
use terra_build::kit::Kit;
use terra_build_layout::layout::Piece;

/// The size in metres of a kit's texture for a material, or a metre square if it has none.
fn metres(kit: &Kit, material: Material) -> [f32; 2] {
    kit.textures
        .get(&material)
        .map_or([1.0, 1.0], |texture| texture.metres)
}

/// Draws a piece, each part in its material. `face` is the height map of the kit's `Log`
/// material, which its logs' bark stands out by, once it has been read.
pub fn draw(piece: &Piece, kit: &Kit, face: Option<&Relief>, surfaces: &mut Surfaces) {
    draw_as(piece, kit, face, false, surfaces);
}

/// Draws a piece as [`draw`] does, or, `plain`, with less of its detail, as a ghost is drawn:
/// a roof's covering as a sheet without its courses.
pub fn draw_as(
    piece: &Piece,
    kit: &Kit,
    face: Option<&Relief>,
    plain: bool,
    surfaces: &mut Surfaces,
) {
    draw_harmed(piece, kit, face, plain, None, surfaces);
}

/// As [`draw_as`], a member broken where it has lost so much of its health that chunks of
/// it have gone (`timber::draw_harmed`, D305).
pub fn draw_harmed(
    piece: &Piece,
    kit: &Kit,
    face: Option<&Relief>,
    plain: bool,
    harm: Option<crate::timber::Harm>,
    surfaces: &mut Surfaces,
) {
    match *piece {
        Piece::Log { .. } if log_module(piece, kit, plain, surfaces) => {}
        Piece::Log { .. } => crate::timber::draw_harmed(piece, kit, harm, surfaces),
        Piece::Board { min, max } => cuboid(
            surfaces.of(Material::Board),
            min.metres(),
            max.metres(),
            metres(kit, Material::Board),
            [1.0; 3],
        ),
        Piece::Blocking { min, max, top } => wedge(
            surfaces.of(Material::Board),
            (min.metres(), max.metres()),
            top.map(terra_build_layout::layout::metres),
            metres(kit, Material::Board),
            [1.0; 3],
        ),
        Piece::Curb { min, max, under } => curb(
            surfaces.of(Material::Board),
            (min.metres(), max.metres()),
            &under,
            metres(kit, Material::Board),
            [1.0; 3],
        ),
        Piece::Plank { .. } => crate::timber::plank(piece, kit, surfaces),
        Piece::Beam { .. } => beam(piece, kit, plain, surfaces),
        Piece::Trim { .. } | Piece::Band { .. } | Piece::Corner { .. } => {
            crate::dress::draw(piece, kit, face, surfaces)
        }
        Piece::Covering { .. } | Piece::Roof { .. } => {
            crate::roofing::draw_as(piece, kit, face, plain, surfaces)
        }
        Piece::Block { .. } | Piece::Gable { .. } | Piece::Wattle { .. } => {}
    }
}

/// A wall's whole log drawn as the kit's hand-made log module (D251, D258), if its timber
/// names one: laid along its middle and scaled across to its thickness, its sawn ends in end
/// grain. Split and slope-cut logs and role members stay generated. Whether it was.
fn log_module(piece: &Piece, kit: &Kit, plain: bool, surfaces: &mut Surfaces) -> bool {
    let Piece::Log {
        from,
        to,
        split: terra_build_layout::layout::Split::Whole,
        seed,
        lean: [0, 0],
        lift: [0, 0],
        role: None,
        ..
    } = *piece
    else {
        return false;
    };
    let Some(module) = kit.timber.module.as_ref().and_then(|name| {
        kit.dress
            .made
            .as_deref()
            .and_then(|pieces| pieces.modules.get(name))
    }) else {
        return false;
    };
    let (from, to) = (from.metres(), to.metres());
    let axis = (to - from).normalize_or(Vec3::X);
    let line = crate::modules::Line {
        from,
        to,
        seat: crate::modules::Seat::Plane {
            up: Vec3::Y,
            out: axis.cross(Vec3::Y).normalize_or(Vec3::Z),
        },
        across: Some([kit.diameter; 2]),
        capped: [true; 2],
        seed,
        blend: 0.0,
        plumb: 0.0,
        crest: None,
        lift: 0.0,
        mitre: [None; 2],
    };
    let look = crate::modules::Look {
        material: Material::Log,
        ends: Some(Material::LogEnd),
        metres: metres(kit, Material::Log),
        end_metres: metres(kit, Material::LogEnd),
        tint: kit.timber.tint,
        shade: kit.timber.variation.shade,
        repeat: kit.dress.repeat(),
    };
    crate::modules::lay(module, &line, &look, plain || kit.dress.far, surfaces);
    true
}

/// A framed kind's member (D236, D258): the kit's frame module laid along its middle, scaled
/// across to its width on the wall's face and its depth into the wall, its sawn ends in
/// end grain; `plain`, at its far budget.
fn beam(piece: &Piece, kit: &Kit, plain: bool, surfaces: &mut Surfaces) {
    let Piece::Beam {
        from,
        to,
        face,
        wide,
        deep,
        seed,
    } = *piece
    else {
        return;
    };
    let Some(framing) = &kit.framing else {
        return;
    };
    let Some(module) = kit
        .dress
        .made
        .as_deref()
        .and_then(|pieces| pieces.modules.get(&framing.module))
    else {
        return;
    };
    let (from, to) = (from.metres(), to.metres());
    let axis = (to - from).normalize_or(Vec3::X);
    // Its depth along the way its face looks, its width across that on the wall's face.
    let normal = face.step().as_vec3();
    let out = (normal - axis * normal.dot(axis)).normalize_or(Vec3::Y);
    let t = &kit.timber;
    let (wide, deep) = (
        terra_build_layout::layout::metres(wide),
        terra_build_layout::layout::metres(deep),
    );
    // A member through the wall, lining an opening, is far deeper than the timber was
    // modelled: stretched to it, the model's waves would swell to centimetres and break
    // through the daub. So it is two timbers of the model's own depth, one at each face, as
    // a frame's jambs are pegged to the face either side.
    let modelled = module.size()[1].max(0.02);
    let lines: Vec<(Vec3, f32)> = if deep > modelled * 2.0 {
        let shift = (deep - modelled) * 0.5;
        vec![(out * shift, modelled), (-out * shift, modelled)]
    } else {
        vec![(Vec3::ZERO, deep)]
    };
    let look = crate::modules::Look {
        material: framing.material,
        ends: Some(Material::LogEnd),
        metres: metres(kit, framing.material),
        end_metres: metres(kit, Material::LogEnd),
        tint: t.tint,
        shade: t.variation.shade,
        repeat: kit.dress.repeat(),
    };
    for (k, (shift, depth)) in lines.into_iter().enumerate() {
        let line = crate::modules::Line {
            from: from + shift,
            to: to + shift,
            seat: crate::modules::Seat::Plane {
                up: out.cross(axis),
                out,
            },
            across: Some([wide, depth]),
            capped: [true; 2],
            seed: terra_gen::hash::of(&[seed, k as u32]),
            blend: 0.0,
            plumb: 0.0,
            crest: None,
            lift: 0.0,
            mitre: [None; 2],
        };
        crate::modules::lay(module, &line, &look, plain || kit.dress.far, surfaces);
    }
}

/// A box from one corner to the other, each face's texture mapped from its plane.
pub(crate) fn cuboid(
    surface: &mut Surface,
    min: Vec3,
    max: Vec3,
    metres: [f32; 2],
    colour: [f32; 3],
) {
    let corner = |x: bool, y: bool, z: bool| {
        Vec3::new(
            if x { max.x } else { min.x },
            if y { max.y } else { min.y },
            if z { max.z } else { min.z },
        )
    };
    let faces = [
        (
            Vec3::X,
            [
                (true, false, true),
                (true, false, false),
                (true, true, false),
                (true, true, true),
            ],
        ),
        (
            Vec3::NEG_X,
            [
                (false, false, false),
                (false, false, true),
                (false, true, true),
                (false, true, false),
            ],
        ),
        (
            Vec3::Y,
            [
                (false, true, true),
                (true, true, true),
                (true, true, false),
                (false, true, false),
            ],
        ),
        (
            Vec3::NEG_Y,
            [
                (false, false, false),
                (true, false, false),
                (true, false, true),
                (false, false, true),
            ],
        ),
        (
            Vec3::Z,
            [
                (false, false, true),
                (true, false, true),
                (true, true, true),
                (false, true, true),
            ],
        ),
        (
            Vec3::NEG_Z,
            [
                (true, false, false),
                (false, false, false),
                (false, true, false),
                (true, true, false),
            ],
        ),
    ];
    for (normal, corners) in faces {
        let corners = corners.map(|(x, y, z)| corner(x, y, z));
        surface.planar_quad(corners, normal, metres, colour);
    }
}

/// A box from `min` to `max` across, its top cut to heights at its corners west-north,
/// east-north, east-south and west-south: blocking under a roof (`Piece::Blocking`). Its
/// top is split along the diagonal that leaves it lower, so where it crosses a valley's or
/// a hip's crease it is cut along it, never standing over the roof.
pub(crate) fn wedge(
    surface: &mut Surface,
    (min, max): (Vec3, Vec3),
    top: [f32; 4],
    metres: [f32; 2],
    colour: [f32; 3],
) {
    let foot = [
        Vec3::new(min.x, min.y, min.z),
        Vec3::new(max.x, min.y, min.z),
        Vec3::new(max.x, min.y, max.z),
        Vec3::new(min.x, min.y, max.z),
    ];
    let head = [0, 1, 2, 3].map(|i| foot[i].with_y(top[i].max(min.y)));
    let mut tri = |t: [Vec3; 3]| {
        let n = (t[1] - t[0]).cross(t[2] - t[0]);
        if n.length_squared() < 1e-12 {
            return;
        }
        let n = n.normalize();
        surface.textured(
            t,
            [n; 3],
            t.map(|p| crate::mesh::planar(p, n, metres)),
            colour,
        );
    };
    // The foot, looking down.
    tri([foot[0], foot[1], foot[2]]);
    tri([foot[0], foot[2], foot[3]]);
    // The top, looking up, along its lower diagonal.
    if head[0].y + head[2].y <= head[1].y + head[3].y {
        tri([head[0], head[2], head[1]]);
        tri([head[0], head[3], head[2]]);
    } else {
        tri([head[1], head[0], head[3]]);
        tri([head[1], head[3], head[2]]);
    }
    // Each side, looking out.
    for i in 0..4 {
        let j = (i + 1) % 4;
        tri([foot[j], foot[i], head[i]]);
        tri([foot[j], head[i], head[j]]);
    }
}

/// A curb under what stands on a roof (`Piece::Curb`): a box from `min` to `max` across, its
/// top level at `max`'s height, its bottom on the roof cube's deck (`under`), following it
/// exactly as a gable's cut does, over a hip's or a valley's crease too: the deck is flat on
/// each of eight triangles of the cube, its quarters split along the diagonals through its
/// middle, so the bottom is those clipped to the curb and its sides break where they cross
/// them.
pub(crate) fn curb(
    surface: &mut Surface,
    (min, max): (Vec3, Vec3),
    under: &[[i32; 3]; 3],
    metres: [f32; 2],
    colour: [f32; 3],
) {
    use glam::{IVec3, Vec2};
    use terra_build::CELL_M;
    let cube = IVec3::new(
        libm::floorf((min.x + max.x) * 0.5 / CELL_M) as i32,
        libm::roundf(max.y / CELL_M) as i32 - 1,
        libm::floorf((min.z + max.z) * 0.5 / CELL_M) as i32,
    );
    let deck = |p: Vec2| {
        terra_build_layout::layout::roofs::gable_cut(cube, under, Vec3::new(p.x, 0.0, p.y))
            .min(max.y)
    };
    let mut face = |t: [Vec3; 3], out: Vec3| {
        let n = (t[1] - t[0]).cross(t[2] - t[0]);
        if n.length_squared() < 1e-12 {
            return;
        }
        let (t, n) = if n.dot(out) >= 0.0 {
            (t, n.normalize())
        } else {
            ([t[0], t[2], t[1]], -n.normalize())
        };
        surface.textured(
            t,
            [n; 3],
            t.map(|p| crate::mesh::planar(p, n, metres)),
            colour,
        );
    };
    let up = |p: Vec2, y: f32| Vec3::new(p.x, y, p.y);
    // The top, looking up.
    let corners = [
        Vec2::new(min.x, min.z),
        Vec2::new(max.x, min.z),
        Vec2::new(max.x, max.z),
        Vec2::new(min.x, max.z),
    ];
    face(
        [
            up(corners[0], max.y),
            up(corners[1], max.y),
            up(corners[2], max.y),
        ],
        Vec3::Y,
    );
    face(
        [
            up(corners[0], max.y),
            up(corners[2], max.y),
            up(corners[3], max.y),
        ],
        Vec3::Y,
    );
    // The bottom, looking down: each triangle the deck is flat on, clipped to the curb.
    let at = |u: f32, v: f32| Vec2::new((cube.x as f32 + u) * CELL_M, (cube.z as f32 + v) * CELL_M);
    for (i, k) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
        let (u, v) = (i as f32 * 0.5, k as f32 * 0.5);
        let [q00, q10, q11, q01] = [
            at(u, v),
            at(u + 0.5, v),
            at(u + 0.5, v + 0.5),
            at(u, v + 0.5),
        ];
        let halves = if (i == 0) == (k == 0) {
            [[q00, q10, q11], [q00, q11, q01]]
        } else {
            [[q00, q10, q01], [q10, q11, q01]]
        };
        for half in halves {
            let clipped = clip(&half, Vec2::new(min.x, min.z), Vec2::new(max.x, max.z));
            for j in 1..clipped.len().saturating_sub(1) {
                let t = [clipped[0], clipped[j], clipped[j + 1]].map(|p| up(p, deck(p)));
                face(t, Vec3::NEG_Y);
            }
        }
    }
    // Each side, looking out, broken where it crosses the lines the deck bends along.
    let middle = (min + max) * 0.5;
    for s in 0..4 {
        let (a, b) = (corners[s], corners[(s + 1) % 4]);
        let local = |p: Vec2| (p.x / CELL_M - cube.x as f32, p.y / CELL_M - cube.z as f32);
        let mut ts = vec![0.0, 1.0];
        let (ua, va) = local(a);
        let (ub, vb) = local(b);
        let mut cross = |fa: f32, fb: f32| {
            if (fa > 0.0) != (fb > 0.0) && (fa - fb).abs() > 1e-6 {
                ts.push(fa / (fa - fb));
            }
        };
        cross(ua - 0.5, ub - 0.5);
        cross(va - 0.5, vb - 0.5);
        cross(ua - va, ub - vb);
        cross(ua + va - 1.0, ub + vb - 1.0);
        ts.sort_by(f32::total_cmp);
        ts.dedup_by(|x, y| (*x - *y).abs() < 1e-5);
        let out = (a + b) * 0.5 - Vec2::new(middle.x, middle.z);
        let out = if (b - a).x.abs() > (b - a).y.abs() {
            Vec3::new(0.0, 0.0, out.y.signum())
        } else {
            Vec3::new(out.x.signum(), 0.0, 0.0)
        };
        for w in ts.windows(2) {
            let (p, q) = (a.lerp(b, w[0]), a.lerp(b, w[1]));
            let (pb, qb) = (up(p, deck(p)), up(q, deck(q)));
            let (pt, qt) = (up(p, max.y), up(q, max.y));
            face([pb, qb, qt], out);
            face([pb, qt, pt], out);
        }
    }
}

/// A convex polygon in plan clipped to a rectangle from `low` to `high`.
fn clip(polygon: &[glam::Vec2], low: glam::Vec2, high: glam::Vec2) -> Vec<glam::Vec2> {
    let mut out = polygon.to_vec();
    for (axis, bound, keep_above) in [
        (0, low.x, true),
        (0, high.x, false),
        (1, low.y, true),
        (1, high.y, false),
    ] {
        let inside = |p: glam::Vec2| {
            if keep_above {
                p[axis] >= bound - 1e-6
            } else {
                p[axis] <= bound + 1e-6
            }
        };
        let input = std::mem::take(&mut out);
        for i in 0..input.len() {
            let (p, q) = (input[i], input[(i + 1) % input.len()]);
            if inside(p) {
                out.push(p);
            }
            if inside(p) != inside(q) {
                let t = (bound - p[axis]) / (q[axis] - p[axis]);
                out.push(p.lerp(q, t));
            }
        }
        if out.len() < 3 {
            return Vec::new();
        }
    }
    out
}

/// A long member far off, as a plain box from one end to the other along its middle, so
/// wide across and so high, its texture mapped from each face's plane: what a log or a
/// deck's plank comes to a few pixels across, where its round or chamfered section and its
/// rings cannot be seen.
pub(crate) fn member_box(
    surface: &mut Surface,
    from: Vec3,
    to: Vec3,
    half: [f32; 2],
    metres: [f32; 2],
    colour: [f32; 3],
) {
    let along = (to - from).normalize_or(Vec3::X);
    let across = along.cross(Vec3::Y).normalize_or(Vec3::Z) * half[0];
    let up = across
        .normalize_or(Vec3::Z)
        .cross(along)
        .normalize_or(Vec3::Y)
        * half[1];
    let at = |end: Vec3, a: f32, b: f32| end + across * a + up * b;
    let side = |a: f32, b: f32, c: f32, d: f32| {
        [at(from, a, b), at(to, a, b), at(to, c, d), at(from, c, d)]
    };
    let faces = [
        (up, side(1.0, 1.0, -1.0, 1.0)),
        (-up, side(-1.0, -1.0, 1.0, -1.0)),
        (across, side(1.0, -1.0, 1.0, 1.0)),
        (-across, side(-1.0, 1.0, -1.0, -1.0)),
        (
            -along,
            [
                at(from, 1.0, -1.0),
                at(from, 1.0, 1.0),
                at(from, -1.0, 1.0),
                at(from, -1.0, -1.0),
            ],
        ),
        (
            along,
            [
                at(to, -1.0, -1.0),
                at(to, -1.0, 1.0),
                at(to, 1.0, 1.0),
                at(to, 1.0, -1.0),
            ],
        ),
    ];
    for (normal, corners) in faces {
        let normal = normal.normalize_or(Vec3::Y);
        // Wound to face out, whichever way the frame turned.
        let wound = (corners[1] - corners[0])
            .cross(corners[2] - corners[0])
            .dot(normal)
            > 0.0;
        let corners = if wound {
            corners
        } else {
            [corners[0], corners[3], corners[2], corners[1]]
        };
        surface.planar_quad(corners, normal, metres, colour);
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use terra_build::Panel;
    use terra_build::testing::{cabin, textured, wall};
    use terra_build_layout::layout::lay;

    /// Each triangle of a shape with no hollows in it faces away from its middle, and its
    /// corners wind the way its normals point; how many there were.
    /// A long shape, such as a log, is judged from its axis level with each triangle, short
    /// of its ends by its thickness, so a knot's swell far from its middle is not taken for
    /// a hollow.
    pub(crate) fn facing_out(surface: &Surface, what: &str) -> usize {
        let middle = surface.positions.iter().sum::<Vec3>() / surface.positions.len() as f32;
        let (low, high) = surface.positions.iter().fold(
            (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
            |(low, high), p| (low.min(*p), high.max(*p)),
        );
        let extent = high - low;
        let long = extent.max_position();
        let thick = (0..3)
            .filter(|&k| k != long)
            .map(|k| extent[k])
            .fold(0.0, f32::max);
        for (corners, normals) in surface.positions.chunks(3).zip(surface.normals.chunks(3)) {
            let own = (corners[1] - corners[0]).cross(corners[2] - corners[0]);
            let centre = (corners[0] + corners[1] + corners[2]) / 3.0;
            let mut middle = middle;
            if extent[long] > 2.0 * thick {
                middle[long] = centre[long].clamp(low[long] + thick, high[long] - thick);
            }
            assert!(
                own.dot(centre - middle) > 0.0,
                "{what}: {corners:?} faces in"
            );
            assert!(
                normals.iter().all(|n| own.dot(*n) > 0.0),
                "{what}: {corners:?} is wound against its normals {normals:?}"
            );
        }
        surface.triangle_count()
    }

    #[test]
    fn every_piece_is_drawn_facing_out() {
        let kit = textured();
        let mut base = cabin();
        wall(&mut base, [[0, 0, 2]], Panel::WINDOW);
        let layout = lay(&base, &kit);
        let mut drawn: BTreeMap<String, usize> = BTreeMap::new();
        for piece in &layout.pieces {
            let mut surfaces = Surfaces::default();
            draw(piece, &kit, None, &mut surfaces);
            let what = format!("{piece:?}");
            let kind = what.split([' ', '{']).next().unwrap_or("").to_string();
            for (material, surface) in &surfaces.0 {
                let count = facing_out(surface, &what);
                *drawn.entry(format!("{kind} {material:?}")).or_default() += count;
            }
        }
        for each in [
            "Log Log",
            "Log LogEnd",
            "Log Daub",
            "Board Board",
            "Plank Floor",
            "Plank LogEnd",
        ] {
            assert!(
                drawn.get(each).is_some_and(|&n| n > 0),
                "no {each}: {drawn:?}"
            );
        }
    }

    #[test]
    fn a_log_kit_can_take_a_hand_made_log_module() {
        // The frame's member, from wattle and daub's pieces, as a log kit's logs: its whole
        // logs are that module along them, its split ones still generated.
        let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        let mut kit = textured();
        kit.dress.pieces = Some("data/kits/pieces/wattle.ron".into());
        kit.dress
            .read(&|f| std::fs::read_to_string(assets.join(f)).map_err(|e| e.to_string()))
            .unwrap();
        let module = kit.dress.made.as_ref().unwrap().modules["member"].clone();
        let log = |split| Piece::Log {
            from: terra_build_layout::layout::Place::near(Vec3::ZERO),
            to: terra_build_layout::layout::Place::near(Vec3::X * 2.0),
            split,
            seed: 1,
            chinking: None,
            lean: [0, 0],
            lift: [0, 0],
            role: None,
            outside: None,
        };
        let count = |kit: &Kit, piece: &Piece| {
            let mut surfaces = Surfaces::default();
            draw(piece, kit, None, &mut surfaces);
            surfaces
                .0
                .get(&Material::Log)
                .map_or(0, |s| s.triangle_count())
        };
        let generated = count(&kit, &log(terra_build_layout::layout::Split::Whole));
        kit.timber.module = Some("member".into());
        let made = count(&kit, &log(terra_build_layout::layout::Split::Whole));
        assert_ne!(made, generated);
        // Two copies of the module along two metres, with nothing generated.
        let cap = module.cap.as_ref().map_or(0, |c| c.triangles.len());
        assert!(made <= (module.triangles() + cap) * 2, "{made}");
        assert_eq!(
            count(&kit, &log(terra_build_layout::layout::Split::Top)),
            count(
                &Kit {
                    timber: terra_build::kit::Timber {
                        module: None,
                        ..kit.timber.clone()
                    },
                    ..kit.clone()
                },
                &log(terra_build_layout::layout::Split::Top)
            )
        );
    }
}
