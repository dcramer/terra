use super::*;

/// A hand laid out as the base body's is about its socket (metres, the socket's frame: +X out
/// of the back of the hand, +Y along the handle out of the thumb's side, +Z toward the wrist):
/// four straight fingers from knuckles 4.5 cm over the handle's line, pointing away from the
/// wrist, and a thumb from the heel of the hand pointing along the handle.
fn hand() -> [[Transform; 4]; 5] {
    let finger = |y: f32, z: f32, long: [f32; 3]| {
        [
            Transform::from_xyz(0.045, y, z),
            Transform::from_xyz(0.0, 0.0, -long[0]),
            Transform::from_xyz(0.0, 0.0, -long[1]),
            Transform::from_xyz(0.0, 0.0, -long[2]),
        ]
    };
    let thumb = [
        Transform::from_xyz(0.038, 0.009, 0.066),
        Transform::from_xyz(-0.011, 0.033, -0.024),
        Transform::from_xyz(-0.009, 0.026, -0.025),
        Transform::from_xyz(-0.02, 0.018, -0.035),
    ];
    [
        thumb,
        finger(0.039, -0.007, [0.027, 0.025, 0.032]),
        finger(0.019, -0.028, [0.027, 0.024, 0.037]),
        finger(-0.003, -0.036, [0.022, 0.018, 0.035]),
        finger(-0.026, -0.024, [0.022, 0.017, 0.031]),
    ]
}

/// Each closed bone's distance from the handle's surface padded by a finger, metres: the
/// thumb's two outer bones and each finger's three.
fn gaps(chains: &[[Transform; 4]; 5], pose: &[[Option<Quat>; 3]; 5], line: &Line) -> Vec<f32> {
    let mut out = Vec::new();
    for (k, chain) in chains.iter().enumerate() {
        let mut at = Transform::IDENTITY;
        let mut joints = Vec::new();
        for j in 0..4 {
            let rotation = if j < 3 {
                pose[k][j].unwrap_or(chain[j].rotation)
            } else {
                chain[j].rotation
            };
            at = at.mul_transform(Transform {
                rotation,
                ..chain[j]
            });
            joints.push(at.translation);
        }
        for j in usize::from(k == 0)..3 {
            out.push(line.to_segment(joints[j], joints[j + 1], 0.0) - line.reach);
        }
    }
    out
}

/// A look whose handle is `r` round, off its origin, seated (`seat`) and tipped `tilt` degrees
/// about the socket's X as a carried look is.
fn line(r: f32, tilt: f32) -> Line {
    line_seated(r, tilt, seat)
}

/// `line`, the look seated by `seat`.
fn line_seated(r: f32, tilt: f32, seat: impl Fn(&Handle, Quat) -> Vec3) -> Line {
    let handle = Handle {
        at: Vec3::new(0.004, 0.0, -0.003),
        way: Vec3::Y,
        r,
    };
    let rotation = Quat::from_rotation_x(tilt.to_radians());
    let look = Transform {
        translation: seat(&handle, rotation),
        rotation,
        ..default()
    };
    Line::of(handle, &Transform::IDENTITY, &look)
}

/// The seat before it followed the handle's radius: mid palm whatever the handle.
fn mid_palm(handle: &Handle, turn: Quat) -> Vec3 {
    Vec3::new(PALM_M - handle.r, 0.0, 0.0) - turn * handle.at
}

#[test]
fn a_seated_handle_lies_on_the_palm_however_it_is_tipped() {
    for (r, tilt) in [(0.012, 0.0), (0.02, -45.0), (0.03, 60.0)] {
        let line = line(r, tilt);
        let at = palmed(r);
        assert!((at.x - (PALM_M - r)).abs() < 1e-6, "{r}");
        assert!(line.to_segment(at, at, 0.0) < 1e-5, "{r} {tilt}");
        let way = Quat::from_rotation_x(f32::to_radians(tilt)) * Vec3::Y;
        assert!(line.d.dot(way) > 0.9999, "{r} {tilt}");
    }
}

#[test]
fn a_thinner_handle_is_seated_toward_the_fingers_by_what_it_lacks() {
    // The side of the handle toward the fingers (-Z) stays where the fitted handle's is; a
    // handle as thick as the fist is placed for, or thicker, stays mid palm.
    for r in [0.012, 0.016, FIT_M] {
        assert!((palmed(r).z - r + FIT_M).abs() < 1e-6, "{r}");
    }
    assert_eq!(palmed(0.025).z, 0.0);
}

#[test]
fn a_thin_a_middle_and_a_thick_handle_each_leave_every_finger_within_a_centimetre() {
    let chains = hand();
    let worst = |line: &Line| {
        gaps(&chains, &close(&chains, line, Grip::Power), line)
            .into_iter()
            .fold(0.0_f32, |a, g| a.max(g.abs()))
    };
    // Square in the fist, as every look is held (no look is tipped in the hand, D546), and
    // tipped 30 degrees either way.
    for r in [0.012, 0.016, 0.021] {
        for tilt in [0.0, -30.0, 30.0] {
            let off = worst(&line(r, tilt));
            assert!(off <= 0.01, "{r} m, {tilt} deg: {:.2} cm off", off * 100.0);
        }
    }
    // The planted fault: the thin handle seated mid palm, as every handle was, leaves the ring's
    // and little finger's knuckle bones short of it.
    let off = worst(&line_seated(0.012, 0.0, mid_palm));
    assert!(off > 0.01, "{:.2} cm off", off * 100.0);
}

#[test]
fn a_fist_closes_until_every_finger_bone_lies_on_the_handle() {
    let chains = hand();
    // Handles from a knife's to thicker than the socket was placed for, seated in the fist
    // (`seat`), square and tipped either way. A knife's tipped 45 degrees runs 1.3 cm past the
    // index's end bone, seated mid palm or at the fingers' roots: no look is tipped in the hand
    // (D546), so it is tried at 30.
    let mut off = Vec::new();
    for (r, tilt) in [
        (0.02, 0.0),
        (0.025, 0.0),
        (0.015, 0.0),
        (0.012, 0.0),
        (0.02, -40.0),
        (0.012, -30.0),
        (0.02, 35.0),
    ] {
        let line = line(r, tilt);
        // Open, as the rest pose holds it, the fingers stand off the handle.
        let open = gaps(&chains, &[[None; 3]; 5], &line);
        assert!(open.iter().any(|g| *g > 0.03), "{r} {tilt}: {open:?}");
        let closed = gaps(&chains, &close(&chains, &line, Grip::Power), &line);
        for (i, g) in closed.iter().enumerate() {
            // The ring's and little finger's first bones too: a thin handle is seated at their
            // roots, where they reach it without the palm bones the skeleton lacks.
            if g.abs() > 0.01 {
                off.push(format!(
                    "{r} m, {tilt} deg: bone {i} {:.1} cm off",
                    g * 100.0
                ));
            }
        }
    }
    assert!(off.is_empty(), "{off:#?}");
}

#[test]
fn a_hook_grip_closes_the_fingers_and_leaves_the_thumb_to_the_clip() {
    let chains = hand();
    let pose = close(&chains, &line(0.01, 0.0), Grip::Hook);
    assert!(pose[0].iter().all(Option::is_none));
    assert!(pose[1..].iter().flatten().all(Option::is_some));
}

#[test]
fn a_handle_is_its_section_across_the_grip_and_not_a_head_beside_it() {
    // A haft 1.5 cm round, 1 cm and -0.5 cm off the grip's line, as a ring of quads, and a
    // head's block 20 cm out that also crosses y = 0.
    let (centre, r, sides) = (Vec3::new(0.01, 0.0, -0.005), 0.015, 16);
    let mut points = Vec::new();
    for y in [-0.1, 0.1] {
        for i in 0..sides {
            let a = std::f32::consts::TAU * i as f32 / sides as f32;
            points.push(centre + Vec3::new(r * libm::cosf(a), y, r * libm::sinf(a)));
        }
    }
    let mut tris = Vec::new();
    for i in 0..sides {
        let (a, b) = (i, (i + 1) % sides);
        tris.extend([a, b, sides + a, b, sides + b, sides + a]);
    }
    let block = points.len();
    points.extend([
        Vec3::new(0.2, -0.05, 0.0),
        Vec3::new(0.25, 0.05, 0.0),
        Vec3::new(0.2, 0.05, 0.05),
    ]);
    tris.extend([block, block + 1, block + 2]);
    let found = handle(&cut(&points, &tris)).expect("a handle");
    assert!((found.at - centre).length() < 0.001, "{found:?}");
    assert!((found.r - r).abs() < 0.001, "{found:?}");
}

#[test]
fn a_looks_mount_says_its_grip() {
    assert_eq!(grip_of(true, None), None);
    assert_eq!(grip_of(false, Some("hung")), Some(Grip::Hook));
    assert_eq!(grip_of(false, None), Some(Grip::Power));
}
