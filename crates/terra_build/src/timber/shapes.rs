//! The two profiles a member is made in (D200): a round log in its bark, and a beam squared
//! by the axe or a sawn board.

use super::*;

/// A: a round log, tapered and bowed as a tree grows, its section a little uneven and
/// swelling here and there, its bark displaced by the bark's height.
pub(super) fn round(
    frame: &Frame,
    kit: &Kit,
    split: Split,
    walled: bool,
    weather: Vec3,
    finer: u32,
    random: &mut Random,
) -> Member {
    let t = &kit.timber;
    let metres = kit
        .textures
        .get(&Material::Log)
        .map_or([1.0, 1.0], |texture| texture.metres);
    let radius = kit.radius() * 1.03 * (1.0 + t.variation.thickness * random.signed());
    let thick_first = random.unit() < 0.5;
    let taper = t.taper * (0.6 + 0.8 * random.unit());
    let bow = random.signed() * t.bend;
    let sag = random.signed() * t.bend * 0.5;
    let phases = [0; 4].map(|_| random.unit() * TAU);
    let shift = [random.unit() * 3.0, random.unit() * 3.0];
    let chamfer = match t.notch {
        Notch::Saddle => 0.03,
        Notch::Flush => 0.012,
    };
    let columns = (t.sides * finer) as usize;
    let around = TAU * radius;
    // A face weathered where a log is exposed (`textures.ts blend --exposed`) is laid once
    // round a lying log from underneath, its middle on top and its first quarter to the
    // weather, so the bark it has lost lies there; any other face, wherever its seed says.
    let exposed = t.exposed && frame.axis.y.abs() < 0.5;
    let to_weather = frame.side.dot(weather) >= 0.0;
    // Its bark worn to bare wood point by point, where its texture names what it wears to:
    // near its sawn ends, on its top and on the face the weather beats on, in patches.
    let worn = exposed
        && kit
            .textures
            .get(&Material::Log)
            .is_some_and(|texture| texture.worn.is_some());
    let weather_flat = weather.with_y(0.0).normalize_or_zero();
    let uv = |t_along: f32, j: usize| {
        let along = shift[1] + t_along * frame.length / metres[1];
        if exposed {
            let round = j as f32 / columns as f32;
            [if to_weather { round } else { 1.0 - round }, along]
        } else {
            [
                shift[0] + j as f32 / columns as f32 * around / metres[0],
                along,
            ]
        }
    };
    // What each point round the section needs that does not change along the log, worked
    // out once: its angle's sine and cosine, and its unevenness's.
    let round: Vec<[f32; 5]> = (0..columns)
        .map(|j| {
            // From underneath, where the texture's seam is out of sight.
            let angle = -PI * 0.5 + TAU * j as f32 / columns as f32;
            [
                libm::sinf(angle),
                libm::cosf(angle),
                libm::sinf(2.0 * angle + phases[1]),
                libm::cosf(2.0 * angle + phases[1]),
                libm::sinf(3.0 * angle + phases[2]),
            ]
        })
        .collect();
    // Its shade: drawn from a seed of its own, so its shape is as it was without it.
    let mut own = Random(random.0 ^ 0x5bd1_e995);
    let mottle_phase = [own.unit() * TAU, own.unit() * TAU, own.unit() * TAU];
    let level = frame.axis.y.abs() < 0.5;
    let rests = walled && level;
    let rest = kit.course * 0.5 + SCRIBE_M;
    // Knots where branches were trimmed off: each a low swell, dark at its heart, along it
    // and round it where its seed says, none in the sawn ends.
    let knots = knots(frame, t, &mut own);
    // Its rings where its shape bends and no closer (D284): the bark's relief is its
    // material's normal map and parallax, not its points, so between its chamfered ends a
    // ring every `SHAPE_RING_M` follows its bow, taper and swells, and three across each
    // knot raise it.
    let at = round_rings(frame, t.step.max(SHAPE_RING_M), chamfer, &knots);
    let last = at.len() - 1;
    let mut grid = Vec::with_capacity(at.len());
    let mut uvs = Vec::with_capacity(at.len());
    // What its shade round it is, the same all along it: toward the groove over it, where the
    // log above overhangs it; its top bleached by the sun a little, as its underside is not.
    // Its underside's shade, where the sky does not reach, is every kit's (`dress::underside`,
    // D320): shaded here too, the two stacked with the screen's occlusion and crushed a log
    // wall's lower halves to black in the shade (D373).
    let shade = round_shade(&round, t, level, walled);
    let mut tone = Vec::with_capacity(at.len());
    let mut wear = Vec::with_capacity(if worn { at.len() } else { 0 });
    for (i, &along) in at.iter().enumerate() {
        let (shift_sin, shift_cos) = (libm::sinf(along * 1.3), libm::cosf(along * 1.3));
        let metres = along * frame.length;
        let mut ring_tone = ring_tone(
            &shade,
            t,
            (frame.length, metres),
            mottle_phase,
            exposed && !worn,
        );
        let arc = libm::sinf(PI * along);
        let centre = frame.from
            + frame.axis * (frame.length * along)
            + frame.side * (bow * arc)
            + frame.up * (if rests { 0.0 } else { sag } * arc);
        let butt = if thick_first {
            1.0 - 2.0 * along
        } else {
            2.0 * along - 1.0
        };
        let swell = 1.0 + t.swell * libm::sinf(along * frame.length * 2.1 + phases[0]);
        // The chamfer round a sawn end, the bark's relief fading into it.
        let (end, fade) = match (t.notch, i.min(last - i)) {
            (Notch::Saddle, 0) => (0.86, 0.0),
            (Notch::Saddle, 1) => (0.95, 0.4),
            (Notch::Flush, 0) => (0.95, 0.0),
            (Notch::Flush, 1) => (0.99, 0.5),
            _ => (1.0, 1.0),
        };
        let mut ring = Vec::with_capacity(columns);
        let mut ring_uv = Vec::with_capacity(columns + 1);
        for (j, &[sin, cos, sin2, cos2, sin3]) in round.iter().enumerate() {
            ring_uv.push(uv(along, j));
            // sin(2a + phase + along * 1.3), by the sum of angles.
            let wave = sin2 * shift_cos + cos2 * shift_sin;
            let uneven = 1.0 + t.uneven * wave + t.uneven * 0.6 * sin3;
            let angle = -PI * 0.5 + TAU * j as f32 / columns as f32;
            let knot = knotted(&knots, (metres, angle, radius), fade, &mut ring_tone[j]);
            let body = radius * (1.0 + taper * butt) * swell * uneven;
            // A wall's log rests on the one below and carries the one above (D178 as
            // amended): never thinner there than the kit's log, so it reaches them for all
            // its taper, and scribed flat where it meets them, a course apart.
            let r = if rests { body.max(kit.radius()) } else { body } * end + knot;
            let height = if rests {
                (sin * r).clamp(-rest, rest)
            } else {
                // Scribed a little flatter top and bottom, to sit on the log below.
                sin * r * 0.94
            };
            let (across, height) = if on_cut(height, split) {
                let plain = radius * (1.0 + taper * butt) * end;
                (cos * plain, 0.0)
            } else {
                (cos * r, height)
            };
            ring.push(centre + frame.side * across + frame.up * height);
        }
        // The seam's last column, where the texture comes round to its start.
        ring_uv.push(uv(along, columns));
        if worn {
            wear.push(worn_ring(
                &round,
                frame,
                (weather_flat, metres),
                mottle_phase,
            ));
        }
        tone.push(ring_tone);
        grid.push(ring);
        uvs.push(ring_uv);
    }
    Member {
        grid,
        uvs,
        grain: [0.47, 0.47],
        daub: (daub_depth(kit), t.daub.1),
        tone,
        wear,
        hue: Vec::new(),
        cut: Vec::new(),
    }
}

/// Knots where branches were trimmed off a round log: each a low swell, dark at its heart,
/// along it and round it where its seed says, none in the sawn ends: where along it, round
/// it, how big and how high.
fn knots(frame: &Frame, t: &crate::kit::Timber, own: &mut Random) -> Vec<[f32; 4]> {
    let count = (frame.length * t.knots + own.unit()).floor() as usize;
    (0..count)
        .map(|_| {
            let size = 0.03 + 0.025 * own.unit();
            // Clear of the sawn ends and their chamfer by three times its size.
            let clear = (3.0 * size).min(frame.length * 0.5);
            let along = clear + (frame.length - 2.0 * clear) * own.unit();
            let angle = own.unit() * TAU;
            let high = 0.006 + 0.008 * own.unit();
            [along, angle, size, high]
        })
        .collect()
}

/// A round log's rings where its shape bends and no closer (D284): the bark's relief is its
/// material's normal map and parallax, not its points, so between its chamfered ends a ring
/// every `step` follows its bow, taper and swells, and three across each knot raise it.
fn round_rings(frame: &Frame, step: f32, chamfer: f32, knots: &[[f32; 4]]) -> Vec<f32> {
    let mut at = stations(frame.length, step, chamfer);
    let ends = (at[2], at[at.len() - 3]);
    for &[along, _, size, _] in knots {
        for k in [-1.0, 0.0, 1.0] {
            let s = (along + k * size) / frame.length;
            if s > ends.0 && s < ends.1 {
                at.push(s);
            }
        }
    }
    at.sort_by(f32::total_cmp);
    at.dedup_by(|a, b| (*a - *b) * frame.length < 0.01);
    at
}

/// A round log's shade round it, the same all along it, for each point round its section
/// (`round`, each its angle's sine and cosine first): toward the groove over it, where the
/// log above overhangs it; its top bleached by the sun a little, as its underside is not;
/// with the sine and cosine its mottling turns by.
fn round_shade(
    round: &[[f32; 5]],
    t: &crate::kit::Timber,
    level: bool,
    walled: bool,
) -> Vec<[f32; 3]> {
    round
        .iter()
        .map(|&[sin, cos, ..]| {
            let mut k = 1.0;
            if level {
                if walled {
                    let over = smoothstep(0.5, 0.95, sin);
                    k *= 1.0 - t.groove * 0.6 * over;
                }
                k *= 1.0 + 0.5 * t.groove * sin;
            }
            // A share of the light, laid on a colour as on screen (D373).
            let k = crate::light_share(k);
            [k, libm::sinf(2.0 * cos), libm::cosf(2.0 * cos)]
        })
        .collect()
}

/// A round log's tone round a ring `metres` along it, of `length`: lighter and darker in
/// long patches along it, and round it a little; weathered paler toward its ends, where the
/// rain and sun get at it most, where it is `weathered`.
fn ring_tone(
    shade: &[[f32; 3]],
    t: &crate::kit::Timber,
    (length, metres): (f32, f32),
    mottle_phase: [f32; 3],
    weathered: bool,
) -> Vec<f32> {
    let patch = t.variation.mottle
        * (0.6 * libm::sinf(metres * 1.9 + mottle_phase[0])
            + 0.4 * libm::sinf(metres * 4.7 + mottle_phase[1]))
        + if weathered {
            END_WEATHER * (1.0 - smoothstep(0.0, 0.45, metres.min(length - metres)))
        } else {
            0.0
        };
    // sin(2 cos a + x), by the sum of angles, a little lighter or darker round it too.
    let (x_sin, x_cos) = (
        libm::sinf(metres * 3.1 + mottle_phase[2]),
        libm::cosf(metres * 3.1 + mottle_phase[2]),
    );
    shade
        .iter()
        .map(|&[k, turn_sin, turn_cos]| {
            let round = 0.3 * t.variation.mottle * (turn_sin * x_cos + turn_cos * x_sin);
            ((1.0 + patch + round) * k).max(0.0)
        })
        .collect()
}

/// How far the knots near a point of a round log raise it, `metres` along it and at `angle`
/// round it on a log of `radius`, their swell faded by `fade`; each darkens its `tone`.
fn knotted(
    knots: &[[f32; 4]],
    (metres, angle, radius): (f32, f32, f32),
    fade: f32,
    tone: &mut f32,
) -> f32 {
    let mut knot = 0.0;
    for &[at_m, at_angle, size, high] in knots {
        let dx = metres - at_m;
        if dx.abs() > 3.0 * size {
            continue;
        }
        let turn = (angle - at_angle + PI).rem_euclid(TAU) - PI;
        let da = turn * radius;
        let w = libm::expf(-(dx * dx + da * da) / (size * size));
        knot += high * w * fade;
        *tone *= 1.0 - 0.5 * w * w;
    }
    knot
}

/// How worn to bare wood each point round a ring of a round log is, `metres` along it: near
/// its sawn ends, its face to the weather most, its top on that side some, the top on its
/// sheltered side a little, underneath not at all, in patches.
fn worn_ring(
    round: &[[f32; 5]],
    frame: &Frame,
    (weather_flat, metres): (Vec3, f32),
    mottle_phase: [f32; 3],
) -> Vec<f32> {
    let end = 1.0 - smoothstep(0.0, END_WEAR_M, metres.min(frame.length - metres));
    round
        .iter()
        .map(|&[sin, cos, ..]| {
            let out = frame.side * cos + frame.up * sin;
            let to = out.dot(weather_flat);
            let facing = (0.8 * to.max(0.0) + 0.4 * sin.max(0.0) * (to + 1.0) * 0.5).min(1.0);
            let patchy = 0.5
                + 0.3 * libm::sinf(metres * 1.3 + mottle_phase[1] + cos * 0.9)
                + 0.2 * libm::sinf(metres * 3.7 + mottle_phase[2]);
            (end + facing * (0.2 + 0.45 * patchy)).clamp(0.0, 1.0)
        })
        .collect()
}

/// A hewn member's half height: half a course less its joint, so the daub between courses
/// shows as a line `joint` tall.
pub(super) fn hewn_high(kit: &Kit) -> f32 {
    ((kit.course - kit.timber.joint.max(0.0)) * 0.5).max(0.01)
}

/// How far out from a wall's middle the daub in a round log's groove stands: where the
/// groove between two logs of the kit's size, a course apart, is `joint` tall, so that much
/// of it shows; the kit's own `daub` share of the log where the logs do not meet.
pub(super) fn daub_depth(kit: &Kit) -> f32 {
    let r = kit.radius();
    let half = (kit.course - kit.timber.joint.max(0.0)) * 0.5;
    if half < r {
        libm::sqrtf(r * r - half * half)
    } else {
        r * kit.timber.daub.0
    }
}

/// B: a hewn beam, squared by the axe: flat faces a little wavy, scalloped where the adze
/// bit, its edges rounded and uneven, a little less tall than a course so daub shows.
pub(super) fn hewn(frame: &Frame, kit: &Kit, split: Split, random: &mut Random) -> Member {
    let t = &kit.timber;
    let metres = kit
        .textures
        .get(&Material::Log)
        .map_or([1.0, 1.0], |texture| texture.metres);
    let thickness = 1.0 + t.variation.thickness * random.signed();
    let half_wide = kit.diameter * HEWN_WIDE * thickness;
    // Every course the same height, its beds fitted to the next (D178 as amended); the
    // draw kept, so the seed's other choices are as they were.
    let _ = random.unit();
    let half_high = hewn_high(kit);
    let chamfer = match t.notch {
        Notch::Saddle => 0.02,
        Notch::Flush => 0.008,
    };
    let look = Squared {
        half_wide,
        half_high,
        round: HEWN_EDGE_M,
        metres,
        strip: None,
        bend: t.bend * 0.25,
        adzed: true,
        chamfer,
        step: t.step,
        // Rounded edges only as finely as the kit draws a round log: fewer far off.
        corner_steps: if t.sides < 12 { 1 } else { 3 },
        tight: false,
        bedded: true,
    };
    let mut member = squared(frame, &look, split, random);
    // Its daub set back from the face of the thinnest beam its seed could give, and as tall
    // as the line between this beam and the next, so it shows there and nowhere in front of
    // either.
    member.daub = (
        kit.diameter * HEWN_WIDE * (1.0 - t.variation.thickness) * t.daub.0,
        kit.course - 2.0 * half_high + 0.004,
    );
    member
}

/// How a squared member is made: its half width and height, how round its edges, its face's
/// texture's size, the strip of it across the member if its texture is of boards side by
/// side, how far it bows at most, whether an adze scalloped its faces, and how deep the
/// chamfer at its ends.
pub(super) struct Squared {
    pub(super) half_wide: f32,
    pub(super) half_high: f32,
    pub(super) round: f32,
    pub(super) metres: [f32; 2],
    pub(super) strip: Option<[f32; 2]>,
    pub(super) bend: f32,
    pub(super) adzed: bool,
    pub(super) chamfer: f32,
    /// How far apart its rings are along it, and how many steps round each edge.
    pub(super) step: f32,
    pub(super) corner_steps: usize,
    /// Laid side by side with others, as a deck's boards are: its sides straight, neither
    /// waved nor drawn in at its ends, so they meet the next's along their length.
    pub(super) tight: bool,
    /// Laid in courses, one on the next, as a wall's beams are: its top and bottom flat, so
    /// it beds on the one below with only its joint between (D178 as amended).
    pub(super) bedded: bool,
}

/// A squared member along a frame: a rounded rectangle swept along it, its faces gently
/// waved, scalloped by the adze if it was hewn, its edges wandering where the axe or saw
/// met the tree's round.
pub(super) fn squared(frame: &Frame, look: &Squared, split: Split, random: &mut Random) -> Member {
    let (half_wide, half_high, round) = (look.half_wide, look.half_high, look.round);
    let phases = [0; 4].map(|_| random.unit() * TAU);
    let shift = [random.unit() * 3.0, random.unit() * 3.0];
    let bow = random.signed() * look.bend;
    // The section: a rounded rectangle, walked round from under its middle, with a point
    // in the middle of each face so the face can wave.
    let mut section: Vec<(Vec3, Vec3)> = Vec::new();
    let corner_steps = look.corner_steps.max(1);
    for (cx, cy, start) in [
        (1.0f32, -1.0f32, -PI * 0.5),
        (1.0, 1.0, 0.0),
        (-1.0, 1.0, PI * 0.5),
        (-1.0, -1.0, PI),
    ] {
        for s in 0..=corner_steps {
            let a = start + PI * 0.5 * s as f32 / corner_steps as f32;
            let centre = Vec3::new(cx * (half_wide - round), cy * (half_high - round), 0.0);
            let normal = Vec3::new(libm::cosf(a), libm::sinf(a), 0.0);
            section.push((centre + normal * round, normal));
        }
        let a = start + PI * 0.5;
        let normal = Vec3::new(libm::cosf(a).round(), libm::sinf(a).round(), 0.0);
        section.push((normal * Vec3::new(half_wide, half_high, 0.0), normal));
    }
    let columns = section.len();
    let mut perimeter = vec![0.0];
    for j in 1..=columns {
        let step = section[j % columns].0.distance(section[j - 1].0);
        perimeter.push(perimeter[j - 1] + step);
    }
    // How far across the texture each point of the section falls. Drawn from one plank, each
    // face takes the plank across it, running on round the section from face to face (the
    // top left to right, the right side up, the underside right to left, the left side
    // down), so every edge meets its neighbour: a joist's side or a rim beam's took one
    // column of the texture smeared down it.
    let across = |j: usize| match look.strip {
        Some([from, wide]) => {
            let (p, n) = section[j % columns];
            let x = (p.x / (2.0 * half_wide) + 0.5).clamp(0.0, 1.0);
            let y = (p.y / (2.0 * half_high) + 0.5).clamp(0.0, 1.0);
            let k = match (n.y.abs() >= n.x.abs(), n.y > 0.0, n.x > 0.0) {
                (true, true, _) => x,
                (true, false, _) => 1.0 - x,
                (false, _, true) => y,
                (false, _, false) => 1.0 - y,
            };
            from + wide * k
        }
        None => shift[0] + perimeter[j] / look.metres[0],
    };
    let at = stations(frame.length, look.step, look.chamfer);
    let last = at.len() - 1;
    let (wave_m, scallop_m, edge_m) = if look.adzed {
        (1.0, 0.003, 0.01)
    } else {
        (0.4, 0.0, 0.002)
    };
    let mut grid = Vec::with_capacity(at.len());
    let mut uvs = Vec::with_capacity(at.len());
    for (i, &along_t) in at.iter().enumerate() {
        let along = along_t * frame.length;
        let centre =
            frame.from + frame.axis * along + frame.side * (bow * libm::sinf(PI * along_t));
        let end = match i.min(last - i) {
            0 => 0.94,
            1 => 0.98,
            _ => 1.0,
        };
        let ring = section
            .iter()
            .map(|&(p, n)| {
                // The face waves gently along the member, and an adze's scallops cut
                // across it, deeper on the sides than on top and below.
                let wave = wave_m
                    * (0.004 * libm::sinf(along * 1.7 + phases[0] + n.x * 2.0)
                        + 0.003 * libm::sinf(along * 4.3 + phases[1] + n.y * 3.0));
                let bite =
                    libm::sinf(along * TAU / 0.13 + phases[2] + 0.8 * libm::sinf(along * 2.3));
                let scallop = -scallop_m * bite.abs() * n.x.abs();
                // The edges, where the axe or saw met the round of the tree, wander.
                let edge = (1.0 - n.x.abs().max(n.y.abs()))
                    * edge_m
                    * libm::sinf(along * 3.1 + phases[3] + p.x * 9.0);
                let q = p + n * (wave + scallop - edge.abs());
                let (x, y) = if on_cut(p.y, split) {
                    (p.x, 0.0)
                } else {
                    (q.x, q.y)
                };
                let x = if look.tight { p.x } else { x * end };
                let y = if look.bedded && !on_cut(p.y, split) {
                    p.y
                } else {
                    y
                };
                centre + frame.side * x + frame.up * (y * end)
            })
            .collect();
        grid.push(ring);
        uvs.push(
            (0..=columns)
                .map(|j| [across(j), shift[1] + along / look.metres[1]])
                .collect(),
        );
    }
    Member {
        grid,
        uvs,
        grain: [0.3, 0.24],
        daub: (0.0, 0.0),
        tone: Vec::new(),
        wear: Vec::new(),
        hue: Vec::new(),
        cut: Vec::new(),
    }
}
