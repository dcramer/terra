//! The street's build (kit/street.rs): the camera and light, the paving (road, kerbs and
//! pavements as laid floors over the street's maps), the puddles, then the fronts, cables,
//! lamps, signs and props.

use super::dress::{fronts, lamps, signs};
use super::{CELL, SITES, Side};
use crate::Args;
use crate::kit::biome::Biome;
use crate::kit::contact::Grounded;
use crate::kit::laid::{LaidMaterial, Place};
use crate::kit::water::KitWater;
use crate::scene::model;
use crate::terrain::fbm;
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

/// A mesh being built of flat sheets, each a grid over a rectangle, carrying the laid
/// floor's maps.
#[derive(Default)]
pub(super) struct Sheets {
    pos: Vec<[f32; 3]>,
    normal: Vec<[f32; 3]>,
    uv: Vec<[f32; 2]>,
    sites: Vec<[f32; 2]>,
    colour: Vec<[f32; 4]>,
    idx: Vec<u32>,
}

impl Sheets {
    /// A sheet from `o` along `u` and `v` (its sides), in cells about `CELL` across, its
    /// texture at `metres` along the sheet's own axes, `maps` giving each vertex's maps.
    pub(super) fn add(
        &mut self,
        o: Vec3,
        u: Vec3,
        v: Vec3,
        metres: f32,
        maps: &dyn Fn(Vec3) -> [f32; 4],
    ) {
        let n = u.cross(v).normalize();
        let (nu, nv) = (
            (u.length() / CELL).ceil().max(1.0) as u32,
            (v.length() / CELL).ceil().max(1.0) as u32,
        );
        let (du, dv) = (u.normalize(), v.normalize());
        let base = self.pos.len() as u32;
        for j in 0..=nv {
            for i in 0..=nu {
                let p = o + u * (i as f32 / nu as f32) + v * (j as f32 / nv as f32);
                self.pos.push(p.to_array());
                self.normal.push(n.to_array());
                self.uv.push([p.dot(du) / metres, -p.dot(dv) / metres]);
                let m = maps(p);
                self.sites.push([m[0], m[1]]);
                let g = 0.82 * (0.9 + 0.2 * fbm(Vec2::new(p.x, p.z) * 0.6, 404));
                self.colour.push(
                    Color::srgba(g, g * 0.98, g * 0.96, m[2])
                        .to_linear()
                        .to_f32_array(),
                );
            }
        }
        let row = nu + 1;
        for j in 0..nv {
            for i in 0..nu {
                let a = base + j * row + i;
                // Counter-clockwise seen from the normal's side: u then v.
                self.idx
                    .extend([a, a + 1, a + row + 1, a, a + row + 1, a + row]);
            }
        }
    }

    pub(super) fn mesh(self) -> Mesh {
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normal)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uv)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, self.sites)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.colour)
        .with_inserted_indices(Indices::U32(self.idx))
        .with_generated_tangents()
        .expect("a street sheet has normals and UVs")
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn spawn(
    mut commands: Commands,
    asset: Res<AssetServer>,
    args: Res<Args>,
    biome: Res<Biome>,
    light: Res<crate::kit::light::Light>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut laid: ResMut<Assets<LaidMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut waters: ResMut<Assets<KitWater>>,
) {
    fn stop(refs: &str, e: String) -> ! {
        panic!("biome {refs:?}: {e}")
    }
    let refs = biome.refs.clone();
    let st = biome.street.clone().unwrap_or_else(|| {
        stop(
            &refs,
            "its layout is Street but it has no `street: (...)`. Fix: add one as \
              biomes/neon-street.ron does (kit/street.rs, D634)"
                .into(),
        )
    });
    st.check().unwrap_or_else(|e| stop(&refs, e));
    let walk = biome.ground.laid.clone().unwrap_or_else(|| {
        stop(
            &refs,
            "a street's pavements are its `ground.laid`. Fix: give its ground a \
              `laid: (material: ..., metres: ...)` (kit/laid.rs, D634)"
                .into(),
        )
    });

    // The camera and the light: the ARPG's view on the focus, the preset's key and fill.
    let focus = Vec3::new(st.focus.0, 1.2, st.focus.1);
    let cam = crate::ruin::camera::arpg(&mut commands, &args, &mut images, focus, light.ev100);
    light.camera(&mut commands, cam, crate::ruin::camera::DISTANCE);
    light.spawn(&mut commands);

    // The paving: road, kerbs and pavements, each a laid floor over the same maps.
    let place = Place { sites: SITES };
    let maps = |p: Vec3| {
        let q = Vec2::new(p.x, p.z);
        [st.gutter(q), st.puddled(q), st.walls(q), 0.0]
    };
    let (half, kerb, wall) = st.edges();
    let (l, rise) = (st.length, st.kerb.1);
    let x0 = -l / 2.0;
    let mut road = Sheets::default();
    road.add(
        Vec3::new(x0, 0.0, half),
        Vec3::X * l,
        Vec3::Z * -st.road,
        st.asphalt.metres,
        &maps,
    );
    let (mut kerbs, mut walks) = (Sheets::default(), Sheets::default());
    for side in [Side::North, Side::South] {
        let s = side.z();
        // Each sheet's u then v runs so its normal faces up, or toward the road.
        // A band from |z| p to q, its origin the larger z and v toward -z, so it faces up.
        let band = |p: f32, q: f32| {
            (
                Vec3::new(x0, rise, if s < 0.0 { -p } else { q }),
                Vec3::Z * -(q - p),
            )
        };
        let (o, v) = band(half, kerb);
        kerbs.add(o, Vec3::X * l, v, st.kerbs.metres, &maps);
        let face = Vec3::new(x0, 0.0, s * half);
        if s < 0.0 {
            kerbs.add(face, Vec3::X * l, Vec3::Y * rise, st.kerbs.metres, &maps);
        } else {
            kerbs.add(
                face + Vec3::X * l,
                Vec3::X * -l,
                Vec3::Y * rise,
                st.kerbs.metres,
                &maps,
            );
        }
        let (o, v) = band(kerb, wall);
        walks.add(o, Vec3::X * l, v, walk.metres, &maps);
    }
    for (sheets, floor) in [(road, &st.asphalt), (kerbs, &st.kerbs), (walks, &walk)] {
        commands.spawn((
            Mesh3d(meshes.add(sheets.mesh())),
            MeshMaterial3d(laid.add(crate::kit::laid::material(&asset, floor, &place))),
        ));
    }

    // The puddles: the biome's water over the road and each pavement where `puddled` says.
    // One mirror serves every face, at the first's level (the road's): the pavements' sit
    // the kerb's rise over it, a few centimetres' error in what they mirror.
    if let Some(water) = &biome.water {
        let wind = biome
            .weather
            .as_ref()
            .map_or(Vec2::ZERO, |w| Vec2::new(w.wind.0, w.wind.1));
        for (z0, z1, floor) in [(-half, half, 0.0), (-wall, -kerb, rise), (kerb, wall, rise)] {
            let level = floor + water.level;
            let face = crate::kit::water::surface(
                Vec2::new(x0, z0),
                Vec2::new(-x0, z1),
                CELL,
                level,
                |p| level - st.standing(p),
            );
            crate::kit::water::spawn_face(
                &mut commands,
                &mut meshes,
                &mut waters,
                water,
                wind,
                Some(&*light),
                face,
            );
        }
    }

    fronts(&mut commands, &asset, &mut meshes, &mut materials, &st);
    crate::kit::cables::spawn(&mut commands, &mut meshes, &mut materials, &st.cables);
    lamps(&mut commands, &mut meshes, &mut materials, &st);
    signs(&mut commands, &asset, &mut meshes, &mut materials, &st);
    props(&mut commands, &asset, &args, &st);
}

/// The street's props: each at its spots, grounded by its bounds, its feet or not at all,
/// rimmed, posed and walked in as its entry says; a film's own left out of a still or play,
/// the staged fight's out of play (`Prop::stands`).
fn props(commands: &mut Commands, asset: &AssetServer, args: &Args, st: &super::Street) {
    for p in &st.props {
        if !p.stands(args.film > 0, args.play) {
            continue;
        }
        for x in &p.at {
            let at = st.spot(p.side, p.line, *x, p.out) + Vec3::Y * p.up;
            let yaw = p.side.facing() + p.turn.to_radians();
            let body = model(commands, asset, &p.model, at, yaw, p.scale);
            // A rigged body's bounds are its bind pose's, a square far wider than its feet
            // (j40's walker stood in a 1.5 m black square): a still prop is grounded by its
            // bounds, a posed body by its feet (kit/contact.rs); a hung one not at all.
            if p.up > 0.0 {
                // Hung on the wall: nothing on the floor under it.
            } else if p.pose.is_none() {
                commands.entity(body).insert(Grounded { floor: at.y });
            } else {
                commands
                    .entity(body)
                    .insert(crate::kit::contact::Feet { floor: at.y });
            }
            if p.rim > 0.0 {
                // Darker than the ground about it as the ruin's fighters (an ogre at half).
                let (albedo, _) = crate::ruin::fight::value(&p.model);
                commands
                    .entity(body)
                    .insert(crate::rim::Fighter::valued(albedo, p.rim));
            }
            if let Some((clip, time)) = &p.pose {
                // The pose names its clip for the life of the app: a few names, once, at
                // startup.
                commands.entity(body).insert(crate::ruin::fight::Pose {
                    gltf: asset.load(p.model.clone()),
                    clip: Box::leak(clip.clone().into_boxed_str()),
                    at: *time,
                    once: false,
                });
            }
            if let Some(w) = &p.walk {
                let off = |(x, z): (f32, f32)| at.xz() + Vec2::new(x, z);
                commands.entity(body).insert(crate::ruin::walk::WalkIn {
                    from: off(w.from),
                    to: if w.fight { at.xz() } else { off(w.to) },
                    floor: crate::ruin::walk::Floor::Flat(at.y),
                    fight: w.fight,
                });
            }
        }
    }
}
