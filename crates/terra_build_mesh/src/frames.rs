//! A door's or shutters' frame drawn (D192, D225): its jambs, head and threshold or sill as
//! boxes in the kit's squared timber, where `terra_build::fitting` stands them in the wall.

use crate::mesh::Surfaces;
use glam::Vec3;
use terra_build::Prop;
use terra_build::kit::Kit;

/// A frame drawn, on a wall whose face stands `face` out from its middle: its boxes in a
/// kit's material (squared timber's, `fragments`'), about `origin`, each face's texture laid
/// along the member's length.
pub fn frame_surfaces(prop: &Prop, face: f32, kit: &Kit, origin: Vec3) -> Surfaces {
    let material = crate::fragments::material(kit);
    let metres = kit
        .textures
        .get(&material)
        .map_or([1.0, 1.0], |texture| texture.metres);
    let mut surfaces = Surfaces::sharing();
    for (min, max) in prop.frame(face) {
        let (min, max) = (min - origin, max - origin);
        let size = max - min;
        // Its length, the way its grain runs: the texture's first axis along it.
        let long = if size.y >= size.x.max(size.z) {
            Vec3::Y
        } else if size.x >= size.z {
            Vec3::X
        } else {
            Vec3::Z
        };
        let middle = (min + max) * 0.5;
        for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
            for side in [-1.0, 1.0] {
                let normal = axis * side;
                let (u, v) = {
                    let a = if axis == long {
                        if axis == Vec3::Y { Vec3::X } else { Vec3::Y }
                    } else {
                        long
                    };
                    (a, normal.cross(a))
                };
                let centre = middle + normal * (size * 0.5);
                let (hu, hv) = (u * (size * 0.5).dot(u).abs(), v * (size * 0.5).dot(v).abs());
                let corners = [
                    centre - hu - hv,
                    centre + hu - hv,
                    centre + hu + hv,
                    centre - hu + hv,
                ];
                let uv = |p: Vec3| [(p - min).dot(u) / metres[0], (p - min).dot(v) / metres[1]];
                for triangle in [
                    [corners[0], corners[1], corners[2]],
                    [corners[0], corners[2], corners[3]],
                ] {
                    let facing = (triangle[1] - triangle[0]).cross(triangle[2] - triangle[0]);
                    let triangle = if facing.dot(normal) < 0.0 {
                        [triangle[0], triangle[2], triangle[1]]
                    } else {
                        triangle
                    };
                    surfaces.of(material).textured(
                        triangle,
                        [normal; 3],
                        triangle.map(uv),
                        [1.0; 3],
                    );
                }
            }
        }
    }
    surfaces
}
