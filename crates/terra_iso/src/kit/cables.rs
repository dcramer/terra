//! The kit's strung cables and run pipes (D634, iso-kit j46): power and phone wires hung
//! across a street from front to front and along the fronts, downpipes and conduits up their
//! faces: the dense overhead clutter of a real city street (the refs' Shibuya and Ascent
//! frames), each a thin tube swept along its hanging curve, the way Unreal's Cable component
//! draws a cable at rest (its docs: a tube mesh along a simulated rope), here the rope's
//! rest shape alone: a parabola, within a few per cent of a catenary at a wire's sag of a
//! tenth of its span (Irvine, "Cable Structures", 1981). Wet, so each catches the signs' and
//! lamps' light in a thin line. A cable is data: two ends, a sag, a thickness, a colour,
//! repeated `count` times `every` metres apart (a bundle of wires, a row of downpipes).

use super::light::Colour;
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use serde::Deserialize;

/// A cable or pipe from `from` to `to` (world x, y, z), hanging `sag` metres below its
/// chord at its middle (0: a straight pipe), `radius` metres thick, of `colour` and
/// `rough`ness, drawn `count` times `every` metres (x, y, z) apart.
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Cable {
    pub from: (f32, f32, f32),
    pub to: (f32, f32, f32),
    #[serde(default)]
    pub sag: f32,
    pub radius: f32,
    #[serde(default = "black")]
    pub colour: Colour,
    #[serde(default = "wet")]
    pub rough: f32,
    #[serde(default = "once")]
    pub count: u32,
    #[serde(default)]
    pub every: (f32, f32, f32),
}

fn black() -> Colour {
    Colour::Srgb(0.03, 0.03, 0.03)
}

fn wet() -> f32 {
    0.35
}

fn once() -> u32 {
    1
}

/// Rings along a cable and sides around it.
const ALONG: usize = 24;
const SIDES: usize = 6;

impl Cable {
    /// The cable as given, or what is wrong and the fix (D556).
    pub fn check(&self) -> Result<(), String> {
        let span = (self.end() - self.start()).length();
        if !(self.radius > 0.0 && self.radius < 0.5) || self.sag < 0.0 || self.count == 0 {
            return Err(format!(
                "cable from {:?} to {:?}: radius {} m, sag {} m, count {}: a cable is 0 to \
                 0.5 m thick, hangs 0 or more metres and is drawn at least once. Fix: its \
                 `radius`, `sag` or `count` (kit/cables.rs, D634)",
                self.from, self.to, self.radius, self.sag, self.count
            ));
        }
        if span < 0.05 {
            return Err(format!(
                "cable from {:?} to {:?} has no length. Fix: move its `to` (kit/cables.rs, D634)",
                self.from, self.to
            ));
        }
        Ok(())
    }

    fn start(&self) -> Vec3 {
        Vec3::new(self.from.0, self.from.1, self.from.2)
    }

    fn end(&self) -> Vec3 {
        Vec3::new(self.to.0, self.to.1, self.to.2)
    }

    /// Where the cable's centre lies at `t` (0 at `from`, 1 at `to`): on its chord, hung
    /// `sag` lower at the middle along a parabola.
    pub fn point(&self, t: f32) -> Vec3 {
        self.start().lerp(self.end(), t) - Vec3::Y * (4.0 * self.sag * t * (1.0 - t))
    }

    /// The tube's mesh, rings of `SIDES` about the curve, `ALONG` of them.
    fn mesh(&self) -> Mesh {
        let mut pos = Vec::with_capacity((ALONG + 1) * (SIDES + 1));
        let mut normal = Vec::with_capacity(pos.capacity());
        let mut uv = Vec::with_capacity(pos.capacity());
        let mut idx = Vec::with_capacity(ALONG * SIDES * 6);
        for i in 0..=ALONG {
            let t = i as f32 / ALONG as f32;
            let dt = 1.0 / ALONG as f32;
            let tangent =
                (self.point((t + dt).min(1.0)) - self.point((t - dt).max(0.0))).normalize();
            let up = if tangent.y.abs() > 0.9 {
                Vec3::X
            } else {
                Vec3::Y
            };
            let n = tangent.cross(up).normalize();
            let b = tangent.cross(n);
            let c = self.point(t);
            for s in 0..=SIDES {
                let a = s as f32 / SIDES as f32 * std::f32::consts::TAU;
                let r = n * libm::cosf(a) + b * libm::sinf(a);
                pos.push((c + r * self.radius).to_array());
                normal.push(r.to_array());
                uv.push([s as f32 / SIDES as f32, t]);
            }
        }
        let ring = (SIDES + 1) as u32;
        for i in 0..ALONG as u32 {
            for s in 0..SIDES as u32 {
                let a = i * ring + s;
                let b = a + ring;
                idx.extend_from_slice(&[a, b, a + 1, a + 1, b, b + 1]);
            }
        }
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normal)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
        .with_inserted_indices(Indices::U32(idx))
    }
}

/// Every cable, each repeat its own tube sharing its cable's mesh and material.
pub fn spawn(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    cables: &[Cable],
) {
    for c in cables {
        let mesh = meshes.add(c.mesh());
        let material = materials.add(StandardMaterial {
            base_color: c.colour.color(),
            perceptual_roughness: c.rough,
            ..default()
        });
        let every = Vec3::new(c.every.0, c.every.1, c.every.2);
        for k in 0..c.count {
            commands.spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material.clone()),
                Transform::from_translation(every * k as f32),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wire(sag: f32) -> Cable {
        Cable {
            from: (-2.0, 6.0, -7.0),
            to: (3.0, 4.0, 7.0),
            sag,
            radius: 0.015,
            colour: black(),
            rough: wet(),
            count: 1,
            every: (0.0, 0.0, 0.0),
        }
    }

    /// A wire meets both its ends and hangs its sag below its chord's middle; a pipe runs
    /// straight.
    #[test]
    fn a_wire_hangs_its_sag_between_its_ends() {
        let w = wire(0.8);
        assert!(w.point(0.0).distance(Vec3::new(-2.0, 6.0, -7.0)) < 1e-5);
        assert!(w.point(1.0).distance(Vec3::new(3.0, 4.0, 7.0)) < 1e-5);
        let chord = (Vec3::new(-2.0, 6.0, -7.0) + Vec3::new(3.0, 4.0, 7.0)) / 2.0;
        assert!((chord.y - w.point(0.5).y - 0.8).abs() < 1e-5);
        let p = wire(0.0);
        assert!((p.point(0.5) - chord).length() < 1e-5);
    }

    /// The tube's every vertex lies its radius from the curve.
    #[test]
    fn the_tube_is_its_radius_about_the_curve() {
        let w = wire(0.8);
        let m = w.mesh();
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(pos)) =
            m.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("a tube has positions");
        };
        for (k, p) in pos.iter().enumerate() {
            let t = (k / (SIDES + 1)) as f32 / ALONG as f32;
            let d = Vec3::from_array(*p).distance(w.point(t));
            assert!((d - 0.015).abs() < 1e-4, "vertex {k} lies {d} m off");
        }
    }

    /// A bad cable says what is wrong and the fix.
    #[test]
    fn a_bad_cable_says_the_fix() {
        let mut w = wire(0.5);
        assert!(w.check().is_ok());
        w.radius = 0.0;
        assert!(w.check().unwrap_err().contains("Fix:"));
        let mut w = wire(0.5);
        w.to = w.from;
        assert!(w.check().unwrap_err().contains("Fix:"));
    }
}
