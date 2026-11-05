//! The floor's small builds: dead branches, twigs and bracken.

use super::*;

/// Tangents for a mesh of bark, which its normal map needs.
pub(super) fn ensure_tangents(mesh: &mut Mesh) {
    if !mesh.contains_attribute(Mesh::ATTRIBUTE_TANGENT)
        && let Err(problem) = mesh.generate_tangents()
    {
        warn!("No tangents for the floor's wood: {problem}");
    }
}

/// Dead branches fallen from the canopy: one to three limbs, each lying along the floor sunk a
/// little into the litter and bending as it lies, thinning to its broken tip, forked into side
/// branches, some reaching down to the ground and propping the limb's far end up off it,
/// others sticking up and broken short; in damp brown bark, darker below.
pub(super) fn branches(dice: &mut Rng) -> Build {
    let mut build = Build::default();
    for _ in 0..1 + dice.below(3) {
        let angle = dice.range(0.0, TAU);
        let along = Vec3::new(libm::cosf(angle), 0.0, libm::sinf(angle));
        let across = along.cross(Vec3::Y);
        let length = dice.range(1.2, 3.2);
        let radius = dice.range(0.035, 0.09);
        let start =
            Vec3::new(dice.range(-0.4, 0.4), 0.0, dice.range(-0.4, 0.4)) - along * length * 0.5;
        // Where the forks prop it, its thin end stands off the ground.
        let propped = if dice.unit() < 0.6 {
            dice.range(0.05, 0.3)
        } else {
            0.0
        };
        let bend = dice.range(-0.25, 0.25);
        let kink = dice.range(-0.08, 0.08);
        let radius_at = |t: f32| radius * (1.0 - 0.65 * t);
        let at = |t: f32| {
            start
                + along * t * length
                + across * (bend * libm::sinf(t * PI) + kink * libm::sinf(t * 3.0 * PI))
                // Sunk a third of its thickness into the litter where it lies on it.
                + Vec3::Y * (radius_at(t) * 0.65 + propped * t * t)
        };
        let points: Vec<Vec3> = (0..=6).map(|i| at(i as f32 / 6.0)).collect();
        let radii: Vec<f32> = (0..=6).map(|i| radius_at(i as f32 / 6.0)).collect();
        tube(
            &mut build,
            &Limb {
                points,
                radii,
                depth: 1,
                pointed: false,
            },
            7,
            1.0e4,
            BARK_M * 0.5,
        );
        for _ in 0..2 + dice.below(4) {
            let t = dice.range(0.25, 0.9);
            let from = at(t);
            let side = if dice.unit() < 0.5 { -1.0 } else { 1.0 };
            let turn = dice.range(0.45, 1.1);
            let way = along * libm::cosf(turn) + across * side * libm::sinf(turn);
            let fork = length * dice.range(0.15, 0.4);
            let r = radius_at(t) * dice.range(0.45, 0.65);
            // Down to the ground where it props the limb up, else up off it, broken short.
            let reaching = from.y > 0.12 && dice.unit() < 0.7;
            let end_y = if reaching {
                r * 0.6
            } else {
                from.y + fork * dice.range(0.05, 0.35)
            };
            let end = from + way * fork;
            let end = Vec3::new(end.x, end_y, end.z);
            let middle = from.lerp(end, 0.5)
                + Vec3::Y * dice.range(0.0, 0.06)
                + across * dice.range(-0.04, 0.04);
            tube(
                &mut build,
                &Limb {
                    points: vec![from, middle, end],
                    radii: vec![r, r * 0.75, r * 0.45],
                    depth: 1,
                    pointed: false,
                },
                5,
                1.0e4,
                BARK_M * 0.5,
            );
        }
    }
    // Brown, as damp dead bark is, rather than the living trunks' grey.
    for colour in &mut build.colours {
        colour[0] *= 0.95;
        colour[1] *= 0.8;
        colour[2] *= 0.62;
        colour[3] = 0.5;
    }
    build
}

/// Twig litter: a dozen to thirty thin dead twigs scattered over a couple of metres, lying
/// every way, some crooked, as the canopy drops them in every wind; paler than living bark.
pub(super) fn twigs(dice: &mut Rng) -> Build {
    let mut build = Build::default();
    for _ in 0..12 + dice.below(19) {
        let at = Vec3::new(dice.range(-1.1, 1.1), 0.0, dice.range(-1.1, 1.1));
        let angle = dice.range(0.0, TAU);
        let along = Vec3::new(libm::cosf(angle), 0.0, libm::sinf(angle));
        let across = along.cross(Vec3::Y);
        let length = dice.range(0.2, 0.9);
        let radius = dice.range(0.006, 0.02);
        let kink = dice.range(-0.06, 0.06);
        let points = (0..=2)
            .map(|i| {
                let t = i as f32 / 2.0;
                at + along * (t - 0.5) * length
                    + Vec3::Y * radius
                    + across * kink * libm::sinf(t * PI)
            })
            .collect();
        tube(
            &mut build,
            &Limb {
                points,
                radii: vec![radius, radius * 0.8, radius * 0.5],
                depth: 1,
                pointed: false,
            },
            3,
            1.0e4,
            BARK_M * 0.3,
        );
    }
    for colour in &mut build.colours {
        colour[0] *= 0.95;
        colour[1] *= 0.84;
        colour[2] *= 0.7;
        colour[3] = 0.45;
    }
    build
}

/// Bracken: a few fronds, each rising on its stalk and arching out nearly level, its tip
/// drooping, as two cards of the frond's picture, the lower steep, the upper laid out from
/// its end.
pub(super) fn bracken(
    dice: &mut Rng,
    leaf: &terra_world::grown::Card,
    tint: Vec3,
) -> grown_build::Build {
    let mut build = grown_build::Build::default();
    let stalk = Vec2::new(0.03, 0.97);
    let tip = Vec2::new(0.97, 0.03);
    let axis = tip - stalk;
    let reach = axis.length();
    let count = 3 + dice.below(4);
    let turn = dice.range(0.0, TAU);
    for i in 0..count {
        let around = turn + i as f32 / count as f32 * TAU + dice.range(-0.4, 0.4);
        let out = Vec3::new(libm::cosf(around), 0.0, libm::sinf(around));
        let across = out.cross(Vec3::Y).normalize();
        let foot = out * dice.range(0.0, 0.2);
        let mut at = foot;
        let height = dice.range(0.75, 1.2);
        for (part, (tilt, length)) in [
            (dice.range(55.0f32, 75.0), height * 0.75),
            (dice.range(-20.0f32, 5.0), height * 0.85),
        ]
        .into_iter()
        .enumerate()
        {
            let tilt = tilt.to_radians();
            let run = (out * libm::cosf(tilt) + Vec3::Y * libm::sinf(tilt)).normalize();
            let facing = across.cross(run).normalize();
            let scale = length / reach;
            let (ax, ay) = (axis.x / reach, axis.y / reach);
            let right = (run * ax + across * ay) * scale;
            let down = (run * ay - across * ax) * scale;
            let shade = if part == 0 { 0.8 } else { 1.0 } * dice.range(0.9, 1.08);
            grown_build::card(
                &mut build,
                leaf,
                &Placing {
                    pin: stalk,
                    at,
                    right,
                    down,
                    normal: (facing * 0.4 + Vec3::Y * 0.6).normalize(),
                    sway: Vec2::new(0.4 + 0.3 * part as f32, 0.6),
                    colour: tint * shade,
                },
            );
            at += run * length * 0.9;
        }
    }
    build
}
