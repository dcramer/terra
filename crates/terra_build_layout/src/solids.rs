//! A base's solid parts near a point as boxes, in the base's own space: what a character
//! controller sweeps a body against (D53, docs/controller.md "What stops a body"), where
//! [`crate::walk`] answers a point walker by columns. The same layers `walk` stands a walker on
//! and stops it by: each cube to its fill, a roof's cube only its shell, a post as wide as
//! `walk::POST_M`; a cube whose top or underside is not level (a stair, a ramp, a corner, an
//! inverted one) cut into upright boxes, a stair's steps exactly and a ramp's in steps. Door
//! and shutter leaves are boxes where they hang (`fitting::hung`), shut or open.

use crate::walk::{POST_M, ROOF_SHELL};
use glam::{IVec3, Vec3};
use terra_build::cubes::{Cube, FULL};
use terra_build::kit::Kits;
use terra_build::{Base, CELL_M, Shape};

/// A box turned in the base's space: its middle, its three axes (orthonormal) and half its size
/// along each.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Slab {
    pub centre: Vec3,
    pub axes: [Vec3; 3],
    pub half: Vec3,
}

impl Slab {
    /// An upright box from one corner to the other.
    fn between(low: Vec3, high: Vec3) -> Slab {
        Slab {
            centre: (low + high) * 0.5,
            axes: [Vec3::X, Vec3::Y, Vec3::Z],
            half: (high - low) * 0.5,
        }
    }
}

/// How many cuts a side a cube whose top or underside is not level is cut into: a stair's
/// steps (at its half) exactly, a ramp in steps of an eighth of a cube, 6 cm, each its middle's
/// height.
const CUTS: usize = 4;

/// The boxes of a base, as far as it stands, within a reach of a point (its own space): its
/// cubes' solid layers and its doors' and shutters' leaves, with the kits it is laid by (on
/// their wall's face, as drawn).
pub fn near(base: &Base, at: Vec3, reach: f32, kits: Option<&Kits>) -> Vec<Slab> {
    let mut out = Vec::new();
    let column = |v: f32| (v / CELL_M).floor() as i32;
    let (low, high) = (at - Vec3::splat(reach), at + Vec3::splat(reach));
    for x in column(low.x)..=column(high.x) {
        for z in column(low.z)..=column(high.z) {
            for layer in base.column(x, z) {
                let foot = layer as f32 * CELL_M;
                if foot > high.y || foot + CELL_M < low.y {
                    continue;
                }
                if let Some(cube) = base.cube(IVec3::new(x, layer, z)) {
                    cube_slabs(cube, IVec3::new(x, layer, z), &mut out);
                }
            }
        }
    }
    let near = |prop: &&terra_build::Prop| {
        let low = IVec3::from_array(prop.min).as_vec3() * CELL_M - reach;
        let high = (IVec3::from_array(prop.max) + IVec3::ONE).as_vec3() * CELL_M + reach;
        at.cmpgt(low).all() && at.cmplt(high).all()
    };
    for prop in base.props().filter(near) {
        let open = if prop.open { 1.0 } else { 0.0 };
        for leaf in crate::fitting::hung(base, prop, kits) {
            let pose = leaf.pose(open);
            out.push(Slab {
                centre: leaf.middle(open),
                axes: [pose.across, Vec3::Y, pose.through],
                half: Vec3::new(leaf.width, leaf.height, leaf.thick) * 0.5,
            });
        }
    }
    out
}

/// A cube's solid layers as boxes: as `walk`'s `solid_layers` has them, from its underside (a
/// roof's shell under its top) to its top or its fill, whichever is lower.
fn cube_slabs(cube: Cube, at: IVec3, out: &mut Vec<Slab>) {
    if !cube.is_in() {
        return;
    }
    let corner = at.as_vec3() * CELL_M;
    let fill = f32::from(cube.level()) / f32::from(FULL);
    let layer = |u: f32, v: f32| {
        let top = fill.min(cube.shape.top(u, v));
        let mut bottom = cube.shape.bottom(u, v);
        if cube.kind.is_roof() {
            bottom = bottom.max(cube.shape.top(u, v) - ROOF_SHELL);
        }
        (top > bottom).then_some((bottom * CELL_M, top * CELL_M))
    };
    let level = matches!(
        cube.shape,
        Shape::Whole | Shape::Half | Shape::UpperHalf | Shape::Post
    );
    if level {
        let Some((bottom, top)) = layer(0.5, 0.5) else {
            return;
        };
        // A post as wide as walking has it, square about its column's middle.
        let side = if cube.shape == Shape::Post {
            POST_M
        } else {
            CELL_M
        };
        let inset = (CELL_M - side) * 0.5;
        out.push(Slab::between(
            corner + Vec3::new(inset, bottom, inset),
            corner + Vec3::new(inset + side, top, inset + side),
        ));
        return;
    }
    let cut = CELL_M / CUTS as f32;
    for i in 0..CUTS {
        for j in 0..CUTS {
            let (u, v) = (
                (i as f32 + 0.5) / CUTS as f32,
                (j as f32 + 0.5) / CUTS as f32,
            );
            if let Some((bottom, top)) = layer(u, v) {
                let (x, z) = (i as f32 * cut, j as f32 * cut);
                out.push(Slab::between(
                    corner + Vec3::new(x, bottom, z),
                    corner + Vec3::new(x + cut, top, z + cut),
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::walk::Inner;
    use terra_build::{BuildOp, Fill, Kind, Panel};

    /// Every shape on a row of cubes, a post, a cube filled to half, and a gable of roof cubes
    /// over a room with a door: what each layer of `walk` is.
    fn every_shape() -> Base {
        let mut base = Base::default();
        let put = |base: &mut Base, at: [i32; 3], cube: Cube| {
            base.apply(BuildOp::Fill {
                min: at,
                max: at,
                cube: Some(cube),
            });
        };
        for (i, shape) in Shape::ALL.into_iter().enumerate() {
            let cube = Cube {
                shape,
                ..Cube::new(Kind::PLAIN, Fill::Solid)
            };
            put(&mut base, [i as i32 * 2, 0, -4], cube);
        }
        put(
            &mut base,
            [0, 0, -8],
            Cube::new(Kind::PLAIN, Fill::Solid).at_level(FULL / 2),
        );
        base.apply(BuildOp::AddRoom {
            min: [0, 0, 0],
            max: [7, 0, 5],
            kind: Kind::PLAIN,
        });
        base.apply(BuildOp::Fill {
            min: [1, 5, 1],
            max: [6, 5, 4],
            cube: None,
        });
        let footprint = (0..8).flat_map(|x| (0..6).map(move |z| [x, z])).collect();
        let plan =
            terra_build::roofer::over(&footprint, 6, terra_build::roofer::Style::Gable, None);
        for (at, shape) in plan.roof {
            let cube = Cube {
                shape,
                ..Cube::new(Kind::FIRST_ROOF, Fill::Solid)
            };
            put(&mut base, at.to_array(), cube);
        }
        for x in 3..5 {
            base.apply(BuildOp::SetWall {
                tile: [x, 0, 5],
                panel: Some(Panel::Door),
                kind: Kind::PLAIN,
            });
        }
        base
    }

    #[test]
    fn a_bases_boxes_are_the_layers_walking_stands_on_and_is_stopped_by() {
        let base = every_shape();
        let slabs = near(&base, Vec3::new(14.0, 2.0, 0.0), 40.0, None);
        // Points off every cut's edge, over the whole base.
        let points = (0..1200).flat_map(|i| {
            (0..240).map(move |j| Vec3::new(i as f32 * 0.0471 - 0.6, 0.0, j as f32 * 0.0473 - 4.6))
        });
        // A ramp's box is its middle's height: off by up to half a cut's rise.
        let ramp = CELL_M / CUTS as f32 * 0.5 + 1e-4;
        let mut compared = 0;
        for at in points {
            let column = IVec3::new(
                (at.x / CELL_M).floor() as i32,
                0,
                (at.z / CELL_M).floor() as i32,
            );
            let mut want: Vec<(f32, f32)> = base.solid_layers(at).collect();
            let mut got: Vec<(f32, f32)> = slabs
                .iter()
                .filter(|s| s.axes[0] == Vec3::X)
                .filter(|s| {
                    let d = at - s.centre;
                    d.x.abs() < s.half.x && d.z.abs() < s.half.z
                })
                .map(|s| (s.centre.y - s.half.y, s.centre.y + s.half.y))
                .collect();
            want.sort_by(|a, b| a.0.total_cmp(&b.0));
            got.sort_by(|a, b| a.0.total_cmp(&b.0));
            assert_eq!(
                want.len(),
                got.len(),
                "at {at} ({column}): {want:?} vs {got:?}"
            );
            for (w, g) in want.iter().zip(&got) {
                let off = (w.0 - g.0).abs().max((w.1 - g.1).abs());
                assert!(off <= ramp, "at {at}: {w:?} vs {g:?}");
            }
            compared += want.len();
        }
        assert!(compared > 10_000, "{compared}");
    }

    #[test]
    fn a_doors_leaves_are_boxes_where_they_hang_shut_and_open() {
        let base = every_shape();
        let leaves = |base: &Base| -> Vec<Slab> {
            near(base, Vec3::new(2.0, 1.0, 2.5), 4.0, None)
                .into_iter()
                // A cube's boxes are half a cube high at most; a leaf is a door's height.
                .filter(|s| s.half.y > CELL_M)
                .collect()
        };
        let mut base = base;
        let rules = terra_build::Rules::default();
        let act = |base: &mut Base, op| {
            base.act(&op, &terra_build::ops::Context::free(&rules))
                .expect("done")
        };
        let door = terra_build::Prop {
            min: [3, 0, 5],
            max: [4, 3, 5],
            opens: [0, 0, -1],
            ..terra_build::Prop::default()
        };
        assert!(leaves(&base).is_empty(), "no door fitted, no leaf");
        act(&mut base, terra_build::ops::Op::FitProp(door));
        let hung = crate::fitting::hung(&base, &door, None);
        let shut = leaves(&base);
        assert!(!hung.is_empty() && shut.len() == hung.len(), "{shut:?}");
        for (slab, leaf) in shut.iter().zip(&hung) {
            assert!((slab.centre - leaf.middle(0.0)).length() < 1e-5);
            let pose = leaf.pose(0.0);
            assert_eq!(slab.axes, [pose.across, Vec3::Y, pose.through]);
            // Its box's foot is the hinge's: as low and as high as the leaf.
            assert!((slab.centre.y - slab.half.y - leaf.hinge.y).abs() < 1e-5);
        }
        act(
            &mut base,
            terra_build::ops::Op::SetOpen {
                at: door.min,
                open: true,
            },
        );
        let swung = leaves(&base);
        assert_eq!(swung.len(), hung.len());
        for (slab, leaf) in swung.iter().zip(&hung) {
            assert!((slab.centre - leaf.middle(1.0)).length() < 1e-5);
            assert!(slab.centre.distance(leaf.middle(0.0)) > 0.2, "it swung");
        }
    }
}
