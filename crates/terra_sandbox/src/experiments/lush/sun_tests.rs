//! Where the floor's sun test (`SkyOccluders::sun_reaches`) puts the sun under a stand of the
//! forest's own pines, against where the shadow pass's own casters let it through: the
//! crowns' shadow lumps, as their meshes are drawn (`crown_clumps`), cast by a ray to the sun
//! through each triangle.

use super::*;

/// The forest walk's sun at 07:30: 23.6 degrees up, toward the east and south (forest fix
/// round 3's log).
fn morning_sun() -> Vec3 {
    let up = libm::sinf(23.6f32.to_radians());
    let level = libm::cosf(23.6f32.to_radians());
    Vec3::new(0.79 * level, up, 0.62 * level).normalize()
}

/// Whether a ray from `eye` along `way` passes through the triangle, within `far`
/// (Möller and Trumbore's test).
fn crosses(eye: Vec3, way: Vec3, far: f32, [a, b, c]: [Vec3; 3]) -> bool {
    let (ab, ac) = (b - a, c - a);
    let p = way.cross(ac);
    let det = ab.dot(p);
    if det.abs() < 1e-9 {
        return false;
    }
    let t0 = eye - a;
    let u = t0.dot(p) / det;
    if !(0.0..=1.0).contains(&u) {
        return false;
    }
    let q = t0.cross(ab);
    let v = way.dot(q) / det;
    if v < 0.0 || u + v > 1.0 {
        return false;
    }
    let t = ac.dot(q) / det;
    t > 0.0 && t < far
}

/// A stand of the forest's grown Scots pines about 4.6 m apart, as its pine stands are
/// (`PINES_APART`), each turned and placed at random: their crowns as the sky's light and
/// the floor see them, and their shadow lumps' triangles in the world.
#[allow(clippy::type_complexity)]
fn pine_stand() -> (SkyOccluders, Vec<(Vec3, f32, Vec<[Vec3; 3]>)>) {
    let pines: Vec<usize> = (0..KINDS.len())
        .filter(|&k| KINDS[k].0 == Species::Pine && !KINDS[k].2)
        .collect();
    let grown: Vec<_> = pines
        .iter()
        .map(|&kind| {
            let (species, form, young, (low, high)) = KINDS[kind];
            let mut dice = Seed(7).derive("tree").index(kind as u64).rng();
            let height = dice.range(low, high);
            let tree = species::conifer(&mut dice, height, species, form, young);
            let build = crown_clumps(&tree, SHADOW_LUMP, true);
            let triangles: Vec<[Vec3; 3]> = build
                .indices
                .chunks(3)
                .map(|t| t.iter().map(|&i| Vec3::from(build.positions[i as usize])))
                .map(|mut t| [t.next().unwrap(), t.next().unwrap(), t.next().unwrap()])
                .collect();
            let lumps = Arc::new(sky_occlusion::Lumps::new(lumps_of(&tree, SHADOW_LUMP)));
            (height, floor::crown_of(&tree.tips), triangles, lumps)
        })
        .collect();
    let mut dice = Seed(11).rng();
    let (mut crowns, mut shadows, mut casters) = (Vec::new(), Vec::new(), Vec::new());
    let apart = 4.6;
    for i in -16..=16 {
        for j in -16..=16 {
            let foot = Vec3::new(
                i as f32 * apart + dice.range(-1.2, 1.2),
                0.0,
                j as f32 * apart + dice.range(-1.2, 1.2),
            );
            // A path's corridor, 7 m wide, runs through the stand toward the sun, as the
            // walk does.
            if foot.dot(Vec3::new(-0.62, 0.0, 0.79)).abs() < 3.5 {
                continue;
            }
            let (height, (middle, reach), triangles, lumps) =
                &grown[dice.below(grown.len() as u32) as usize];
            let turn = Quat::from_rotation_y(dice.range(0.0, TAU));
            crowns.push(crown_seen(
                foot + turn * *middle,
                Vec3::Y,
                *height,
                *reach,
                Species::Pine,
                false,
            ));
            shadows.push(sky_occlusion::CrownShadow {
                foot,
                turn,
                scale: Vec3::ONE,
                lumps: lumps.clone(),
            });
            let placed = triangles
                .iter()
                .map(|t| t.map(|p| foot + turn * p))
                .collect();
            casters.push((
                foot + turn * *middle + Vec3::Y * height * 0.82,
                height * 0.5,
                placed,
            ));
        }
    }
    let occluders = SkyOccluders {
        middle: Vec3::ZERO,
        up: Vec3::Y,
        east: Vec3::X,
        half_m: 80.0,
        below_m: 15.0,
        above_m: 45.0,
        crowns,
        shadows,
    };
    (occluders, casters)
}

/// The sun at a frond's height under the stand by the floor's test and by the shadow
/// lumps, on a grid over its middle: how much of the floor each says is lit, and how much of
/// it they agree on.
fn sun_under_the_stand() -> (f32, f32, f32) {
    let (occluders, casters) = pine_stand();
    let sun = morning_sun();
    let (mut by_test, mut by_lumps, mut agree, mut n, mut by_thick) =
        (0u32, 0u32, 0u32, 0u32, 0u32);
    // The path's verges, from 3.6 to 5.5 m off its middle, along 60 m of it.
    let (along, across) = (Vec3::new(0.79, 0.0, 0.62), Vec3::new(-0.62, 0.0, 0.79));
    for i in -30..30 {
        for j in [-5.5f32, -4.8, -4.2, -3.6, 3.6, 4.2, 4.8, 5.5] {
            let eye = along * (i as f32 + 0.37) + across * j + Vec3::Y * 0.5;
            let test = occluders.sun_reaches(eye, sun, 120.0);
            let lumps = !casters.iter().any(|(middle, reach, triangles)| {
                let off = *middle - eye;
                let along = off.dot(sun);
                along > 0.0
                    && (off - sun * along).length() < *reach + 8.0
                    && triangles.iter().any(|t| crosses(eye, sun, 160.0, *t))
            });
            n += 1;
            by_thick += !occluders.hides(eye, eye + sun * 120.0) as u32;
            by_test += test as u32;
            by_lumps += lumps as u32;
            agree += (test == lumps) as u32;
        }
    }
    let share = |v: u32| v as f32 / n as f32;
    eprintln!("lit by the crowns' thick alone {:.2}", share(by_thick));
    (share(by_test), share(by_lumps), share(agree))
}

#[test]
fn the_floors_sun_falls_where_the_crowns_shadow_lets_it_through() {
    let (test, lumps, agree) = sun_under_the_stand();
    eprintln!(
        "lit by the floor's test {test:.2}, by the shadow lumps {lumps:.2}, agree {agree:.2}"
    );
    assert!(
        (test - lumps).abs() < 0.05,
        "the floor's test lights {test:.2} of the floor, the crowns' shadow {lumps:.2}"
    );
    assert!(agree > 0.9, "they agree on only {agree:.2} of it");
}
