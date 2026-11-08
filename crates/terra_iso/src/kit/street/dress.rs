//! The street's dressing (kit/street.rs): the building fronts with their windows or facade
//! bays, the street lamps and the signs.

use super::paving::Sheets;
use super::{POLE, Street};
use crate::forest::Rng;
use crate::kit::water::Glint;
use crate::scene::material;
use bevy::light::NotShadowCaster;
use bevy::prelude::*;

/// The fronts: each a block of its scan with a roof, and its storeys of windows.
pub(super) fn fronts(
    commands: &mut Commands,
    asset: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    st: &Street,
) {
    let (_, _, wall) = st.edges();
    let flat = |_: Vec3| [0.0, 0.0, 0.0, 0.0];
    let glass = materials.add(StandardMaterial {
        base_color: Color::srgb(0.02, 0.025, 0.03),
        perceptual_roughness: 0.08,
        reflectance: 0.6,
        ..default()
    });
    let pane = meshes.add(Rectangle::new(1.0, 1.0));
    let mut rng = Rng::new(0x5_7ee7);
    for f in &st.fronts {
        let s = f.side.z();
        let (x0, x1, h) = (f.along.0, f.along.1, f.height);
        let (near, back) = (s * wall, s * (wall + f.depth));
        let mut faces = Sheets::default();
        // The face to the road (set back dark behind a facade kit's bays), then its ends
        // from the wall line; each wound so its normal looks out.
        let (z_a, z_b) = if s < 0.0 { (near, back) } else { (back, near) };
        let mut inner = Sheets::default();
        let (road, set) = match &f.bays {
            Some(b) => (&mut inner, s * (wall + b.back)),
            None => (&mut faces, near),
        };
        if s < 0.0 {
            road.add(
                Vec3::new(x0, 0.0, set),
                Vec3::X * (x1 - x0),
                Vec3::Y * h,
                f.metres,
                &flat,
            );
        } else {
            road.add(
                Vec3::new(x1, 0.0, set),
                Vec3::X * (x0 - x1),
                Vec3::Y * h,
                f.metres,
                &flat,
            );
        }
        faces.add(
            Vec3::new(x0, 0.0, z_b),
            Vec3::Z * (z_a - z_b),
            Vec3::Y * h,
            f.metres,
            &flat,
        );
        faces.add(
            Vec3::new(x1, 0.0, z_a),
            Vec3::Z * (z_b - z_a),
            Vec3::Y * h,
            f.metres,
            &flat,
        );
        let (r, g, b) = f.tint;
        commands.spawn((
            Mesh3d(meshes.add(faces.mesh())),
            MeshMaterial3d(materials.add(material(asset, &f.material, Color::srgb(r, g, b)))),
        ));
        let mut roof = Sheets::default();
        roof.add(
            Vec3::new(x0, h, z_a),
            Vec3::X * (x1 - x0),
            Vec3::Z * (z_b - z_a),
            3.0,
            &flat,
        );
        let top = f.roof.as_deref().unwrap_or(&f.material);
        commands.spawn((
            Mesh3d(meshes.add(roof.mesh())),
            MeshMaterial3d(materials.add(material(asset, top, Color::srgb(0.45, 0.45, 0.45)))),
        ));
        if let Some(b) = &f.bays {
            // The rooms behind the openings: near black, rough.
            commands.spawn((
                Mesh3d(meshes.add(inner.mesh())),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(0.015, 0.014, 0.013),
                    perceptual_roughness: 0.95,
                    ..default()
                })),
            ));
            crate::kit::facade::spawn(
                commands, asset, meshes, materials, f.side, f.along, h, wall, b, &mut rng,
            );
            continue;
        }
        let Some(w) = &f.windows else { continue };
        let lit: Vec<Handle<StandardMaterial>> = w
            .colours
            .iter()
            .map(|c| {
                materials.add(StandardMaterial {
                    base_color: Color::BLACK,
                    emissive: c.color().to_linear() * w.nits,
                    perceptual_roughness: 0.2,
                    ..default()
                })
            })
            .collect();
        let mut y = w.from + w.size.1 / 2.0;
        while y + w.size.1 / 2.0 < h - 0.5 {
            let mut x = x0 + w.every / 2.0;
            while x < x1 - w.every / 2.0 + 0.01 {
                let on = rng.f() < w.lit && !lit.is_empty();
                let m = if on {
                    lit[(rng.f() * lit.len() as f32) as usize % lit.len()].clone()
                } else {
                    glass.clone()
                };
                commands.spawn((
                    Mesh3d(pane.clone()),
                    MeshMaterial3d(m),
                    Transform::from_xyz(x, y, near - s * 0.02)
                        .with_rotation(Quat::from_rotation_y(f.side.facing()))
                        .with_scale(Vec3::new(w.size.0, w.size.1, 1.0)),
                ));
                x += w.every;
            }
            y += w.storey;
        }
    }
}

/// The street lamps: a dark pole by the kerb, an arm over the road, a glowing head and its
/// spot straight down.
pub(super) fn lamps(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    st: &Street,
) {
    let Some(lp) = &st.lamps else { return };
    let steel = materials.add(StandardMaterial {
        base_color: Color::srgb(0.08, 0.085, 0.09),
        metallic: 0.7,
        perceptual_roughness: 0.45,
        ..default()
    });
    let glow = materials.add(StandardMaterial {
        base_color: Color::BLACK,
        emissive: lp.colour.color().to_linear() * 400.0,
        ..default()
    });
    let pole = meshes.add(Cylinder::new(POLE, lp.height));
    let arm = meshes.add(Cuboid::new(0.06, 0.06, lp.reach));
    let head = meshes.add(Cuboid::new(0.32, 0.08, 0.6));
    for (side, foot) in st.poles().0 {
        let s = side.z();
        commands.spawn((
            Mesh3d(pole.clone()),
            MeshMaterial3d(steel.clone()),
            Transform::from_translation(foot + Vec3::Y * lp.height / 2.0),
        ));
        let top = foot + Vec3::Y * lp.height;
        commands.spawn((
            Mesh3d(arm.clone()),
            MeshMaterial3d(steel.clone()),
            Transform::from_translation(top - Vec3::Z * s * lp.reach / 2.0),
        ));
        let lamp = top - Vec3::Z * s * lp.reach - Vec3::Y * 0.06;
        commands.spawn((
            Mesh3d(head.clone()),
            MeshMaterial3d(glow.clone()),
            Transform::from_translation(lamp),
        ));
        commands.spawn((
            SpotLight {
                color: lp.colour.color(),
                intensity: lp.lumens,
                range: lp.range,
                radius: 0.15,
                inner_angle: lp.angle.to_radians() * 0.55,
                outer_angle: lp.angle.to_radians(),
                shadow_maps_enabled: lp.shadows,
                ..default()
            },
            Transform::from_translation(lamp - Vec3::Y * 0.1)
                .looking_at(lamp - Vec3::Y * 5.0, Vec3::Z),
            Glint,
        ));
    }
}

/// The signs: a dark box on the wall, its face the texture glowing, and a light of its
/// colour half a metre out so it lights the wet ground.
pub(super) fn signs(
    commands: &mut Commands,
    asset: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    st: &Street,
) {
    let (_, _, wall) = st.edges();
    let backing = materials.add(StandardMaterial {
        base_color: Color::srgb(0.03, 0.03, 0.035),
        metallic: 0.5,
        perceptual_roughness: 0.5,
        ..default()
    });
    for sg in &st.signs {
        let s = sg.side.z();
        let (w, h) = sg.size;
        let face = materials.add(StandardMaterial {
            base_color: Color::BLACK,
            emissive: LinearRgba::rgb(sg.nits, sg.nits, sg.nits),
            emissive_texture: Some(asset.load(sg.texture.clone())),
            perceptual_roughness: 0.3,
            ..default()
        });
        // Out from the wall toward the road, and the card's turn: facing the road, or
        // edge-on to it (a blade, seen from up and down the street). A flat sign's box runs
        // from the wall to its card, out past a facade's frames (0.2 m) and under its crown.
        let (out, turn, depth) = if sg.blade {
            (
                0.15 + w / 2.0,
                std::f32::consts::FRAC_PI_2,
                Vec3::new(0.08, h, w),
            )
        } else {
            (0.25, sg.side.facing(), Vec3::new(w, h, 0.24))
        };
        let mid = Vec3::new(sg.at, sg.height, s * (wall - out));
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(depth.x, depth.y, depth.z))),
            MeshMaterial3d(backing.clone()),
            Transform::from_translation(mid + Vec3::Z * s * 0.125 * (!sg.blade as u8 as f32)),
            // Its own light sits in it (a blade's) or just before it: a shadowed one would
            // black out its own pool, as a brazier's bowl did (traps.md).
            NotShadowCaster,
        ));
        let to_road = Vec3::Z * -s;
        // A blade shows a card on each face of its box, a flat sign one toward the road.
        let card = meshes.add(Rectangle::new(w, h));
        let cards: Vec<Transform> = if sg.blade {
            [1.0f32, -1.0]
                .map(|k| {
                    Transform::from_translation(mid + Vec3::X * k * 0.045)
                        .with_rotation(Quat::from_rotation_y(turn * k))
                })
                .to_vec()
        } else {
            vec![
                Transform::from_translation(mid + to_road * 0.005)
                    .with_rotation(Quat::from_rotation_y(turn)),
            ]
        };
        for t in cards {
            commands.spawn((
                Mesh3d(card.clone()),
                MeshMaterial3d(face.clone()),
                t,
                NotShadowCaster,
            ));
        }
        let lamp = mid + to_road * if sg.blade { 0.4 } else { 0.6 };
        let mut glow = commands.spawn((
            PointLight {
                color: sg.colour.color(),
                intensity: sg.lumens,
                range: sg.range,
                radius: (w.max(h) * 0.3).min(0.6),
                shadow_maps_enabled: sg.shadows && sg.pool.is_none(),
                ..default()
            },
            Transform::from_translation(lamp),
        ));
        let Some((cone, reach, lumens)) = sg.pool else {
            glow.insert(Glint);
            continue;
        };
        // The pool: down from the sign onto the pavement `reach` out from the wall, its
        // cone's edge grazing the wall a little under the sign, so the face lights only
        // below it and falls off with the distance (the ARPGs' pool under a light).
        let foot = Vec3::new(sg.at, st.kerb.1, s * (wall - reach));
        commands.spawn((
            SpotLight {
                color: sg.colour.color(),
                intensity: lumens,
                range: (lamp - foot).length() + 3.0,
                radius: (w.max(h) * 0.3).min(0.6),
                inner_angle: cone.to_radians() * 0.5,
                outer_angle: cone.to_radians(),
                shadow_maps_enabled: sg.shadows,
                ..default()
            },
            Transform::from_translation(lamp).looking_at(foot, Vec3::X),
            Glint,
        ));
    }
}
