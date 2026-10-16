//! A plan's pieces drawn as ghosts: a box or a prism a piece, for the game to draw
//! instanced.

use super::*;

/// A plan's piece as a ghost, if it is drawn as one: a box or a prism stretched over it.
pub(super) fn ghost(piece: &Piece, kit: &Kit, shade: bool) -> Option<Ghost> {
    // A box from one point to another along its middle, so wide and so high, its height
    // along `up` as far as that is square to it.
    let along = |from: Vec3, to: Vec3, up: Vec3, high: f32, wide: f32, shape: Shape| {
        let length = from.distance(to);
        if length < 1e-3 {
            return None;
        }
        let x = (to - from) / length;
        let y = (up - x * up.dot(x)).normalize_or(if x.y.abs() < 0.9 { Vec3::Y } else { Vec3::X });
        let z = x.cross(y);
        Some(Ghost {
            shape,
            centre: (from + to) * 0.5,
            rotation: Quat::from_mat3(&Mat3::from_cols(x, y, z)),
            size: Vec3::new(length, high, wide),
            shade,
        })
    };
    let boxed = |min: Vec3, max: Vec3| Ghost {
        shape: Shape::Box,
        centre: (min + max) * 0.5,
        rotation: Quat::IDENTITY,
        size: (max - min).max(Vec3::splat(1e-3)),
        shade,
    };
    match *piece {
        Piece::Log {
            from, to, split, ..
        } => {
            let (from, to) = (from.metres(), to.metres());
            let d = kit.diameter;
            let round = kit.timber.profile == Profile::Round;
            let (high, lift) = match split {
                layout::Split::Whole => (if round { d } else { kit.course * 0.8 }, 0.0),
                layout::Split::Top => (d * 0.5, d * 0.25),
                layout::Split::Bottom => (d * 0.5, -d * 0.25),
            };
            let shape = if round && split == layout::Split::Whole {
                Shape::Round
            } else {
                Shape::Box
            };
            let lift = Vec3::Y * lift;
            along(from + lift, to + lift, Vec3::Y, high, d * 0.92, shape)
        }
        Piece::Plank {
            from,
            to,
            wide,
            high,
            ..
        } => along(
            from.metres(),
            to.metres(),
            Vec3::Y,
            layout::metres(high),
            layout::metres(wide),
            Shape::Box,
        ),
        Piece::Board { min, max } => Some(boxed(min.metres(), max.metres())),
        Piece::Blocking { .. } | Piece::Curb { .. } => {
            piece.bounds().map(|(min, max)| boxed(min, max))
        }
        Piece::Beam {
            from,
            to,
            face,
            wide,
            deep,
            ..
        } => along(
            from.metres(),
            to.metres(),
            face.step().as_vec3(),
            layout::metres(deep),
            layout::metres(wide),
            Shape::Box,
        ),
        // A plan's trims as a band along their edge, standing out of its face.
        Piece::Trim {
            cube,
            role,
            side,
            other,
            ..
        } => {
            let (from, to) = layout::roles::line(cube, layout::roles::Edge { role, side, other });
            let at = |p: [i32; 3]| {
                glam::IVec3::from_array(p).as_vec3() * (CELL_M / layout::TICKS as f32)
            };
            let (from, to) = (at(from), at(to));
            let out = side.step().as_vec3();
            let up = if other == layout::Dir::Up { -0.1 } else { 0.25 };
            let (a, b) = (from - out * 0.05, to + out * 0.06 + Vec3::Y * up);
            Some(boxed(a.min(b), a.max(b)))
        }
        Piece::Corner { .. } => None,
        // A cut cube's band along its line, out of its face and up from it.
        Piece::Band { from, to, side, .. } => {
            let (from, to) = (from.metres(), to.metres());
            let out = side.step().as_vec3();
            let up = (to - from).normalize_or(Vec3::X).cross(out);
            let up = up * up.y.signum();
            along(
                from + out * 0.01,
                to + out * 0.01,
                up,
                0.1,
                0.12,
                Shape::Box,
            )
        }
        Piece::Covering {
            corners, middle, ..
        } => {
            let c = corners.map(layout::Place::metres);
            let x = ((c[1] - c[0]) + (c[2] - c[3])) * 0.5;
            let z = ((c[3] - c[0]) + (c[2] - c[1])) * 0.5;
            let y = z.cross(x).normalize_or(Vec3::Y);
            let thick = (kit.roof.thick + kit.roof.relief).max(0.03);
            let (width, depth) = (x.length(), (z - y * z.dot(y)).length());
            let mut tile = along(
                Vec3::ZERO,
                x.normalize_or(Vec3::X),
                y,
                thick,
                depth * 0.9,
                Shape::Box,
            )?;
            tile.centre = middle.metres() + y * (thick * 0.5);
            tile.size.x = width * 0.9;
            Some(tile)
        }
        Piece::Roof {
            part,
            from,
            to,
            sides,
            ..
        } => {
            let (from, to) = (from.metres(), to.metres());
            let up = layout::roofs::planar_normal(from, to, sides[0].metres());
            // As big as the module laid for it, up and across.
            let module = kit.roof.parts.get(&part).and_then(|p| {
                kit.dress
                    .made
                    .as_deref()
                    .and_then(|pieces| pieces.modules.get(&p.module))
            });
            let [high, wide] = module.map_or([0.08, 0.2], |m| m.size().map(|s| s.max(0.02)));
            along(from, to, up, high, wide, Shape::Box)
        }
        Piece::Block { .. } | Piece::Gable { .. } | Piece::Wattle { .. } => None,
    }
}
