//! The floor's dead wood: fallen logs and stumps, built from rings of bark with their cut and
//! broken ends.

use super::*;

/// A rock's share of its height that is under the ground, applied once its mesh has
/// loaded and its height is known (`footings`).
#[derive(Component)]
pub(in crate::experiments::lush) struct Sink(pub f32);

/// How many kinds of fallen log and of stump there are; each is drawn many times.
pub(super) const LOG_KINDS: u64 = 6;
pub(super) const STUMP_KINDS: u64 = 6;

/// A log or stump: its bark, and its cut end, of end grain, and how thick it is.
pub(in crate::experiments::lush) struct Wood {
    pub bark: Build,
    pub end: Option<Build>,
    pub radius: f32,
}

/// A fallen log, lying along its x: its bark and cut end, how long and how thick at its
/// foot.
pub(super) struct Log {
    pub(super) bark: Build,
    pub(super) end: Option<Build>,
    pub(super) length: f32,
    pub(super) radius: f32,
}

/// A ring of points round a line, `sides` round, its radius shaped by `shape` at each
/// angle; and the texture places round it, `wraps` repeats of the bark's picture.
#[allow(clippy::too_many_arguments)]
pub(super) fn ring(
    build: &mut Build,
    middle: Vec3,
    forward: Vec3,
    side: Vec3,
    sides: u32,
    radius: impl Fn(f32) -> f32,
    v: f32,
    wraps: f32,
    sky: impl Fn(Vec3) -> f32,
    colour: Vec3,
) {
    let other = forward.cross(side);
    for j in 0..=sides {
        let angle = j as f32 / sides as f32 * TAU;
        let out = side * libm::cosf(angle) + other * libm::sinf(angle);
        let round = side * -libm::sinf(angle) + other * libm::cosf(angle);
        let r = radius(angle);
        let slope = (radius(angle + 0.01) - radius(angle - 0.01)) / 0.02;
        let normal = (out * r - round * slope).normalize_or(out);
        let index = build.vertex(
            middle + out * r,
            normal,
            Vec2::new(j as f32 / sides as f32 * wraps, v),
            Vec2::ZERO,
            colour,
        );
        build.colours[index as usize][3] = sky(normal);
    }
}

/// Joins rings of `sides + 1` points from `first`, `count` of them, facing out.
pub(super) fn join_rings(build: &mut Build, first: u32, count: u32, sides: u32) {
    let ring = sides + 1;
    for i in 0..count - 1 {
        for j in 0..sides {
            let a = first + i * ring + j;
            let b = a + ring;
            build.indices.extend([a, a + 1, b, a + 1, b + 1, b]);
        }
    }
}

/// A broken end: from the last ring, `sides + 1` points from `last`, splinters of wood
/// jutting on along `forward`, each its own length, closing to a point, paler where the
/// wood shows.
pub(super) fn splinters(
    build: &mut Build,
    dice: &mut Rng,
    last: u32,
    sides: u32,
    middle: Vec3,
    forward: Vec3,
    radius: f32,
    v: f32,
) {
    let tip_v = v + 0.3;
    let mut teeth = Vec::new();
    let lengths: Vec<f32> = (0..sides)
        .map(|_| {
            let u = dice.unit();
            radius * (0.15 + 1.6 * u * u)
        })
        .collect();
    for j in 0..=sides {
        let from = Vec3::from(build.positions[(last + j) as usize]);
        let out = (from - middle) - forward * (from - middle).dot(forward);
        let length = lengths[j as usize % sides as usize];
        let at = middle + forward * length + out * 0.45;
        let index = build.vertex(
            at,
            (out.normalize_or(forward) * 0.6 + forward * 0.4).normalize(),
            Vec2::new(build.uvs[(last + j) as usize][0], tip_v),
            Vec2::ZERO,
            Vec3::new(1.35, 1.2, 1.0),
        );
        build.colours[index as usize][3] = 0.6;
        teeth.push(index);
    }
    let centre = build.vertex(
        middle + forward * radius * 0.3,
        forward,
        Vec2::new(0.5, tip_v),
        Vec2::ZERO,
        Vec3::new(1.4, 1.25, 1.0),
    );
    build.colours[centre as usize][3] = 0.6;
    for j in 0..sides {
        let (a, b) = (last + j, last + j + 1);
        let (ta, tb) = (teeth[j as usize], teeth[j as usize + 1]);
        build.indices.extend([a, b, ta, b, tb, ta]);
        build.indices.extend([ta, tb, centre]);
    }
}

/// A flat cut end, in its own mesh of end grain: a disc of the section's picture over the
/// last ring, `sides + 1` points from `last`, facing `normal`.
pub(super) fn cut_end(
    bark: &Build,
    last: u32,
    sides: u32,
    middle: Vec3,
    normal: Vec3,
    side: Vec3,
) -> Build {
    let mut end = Build::default();
    let other = normal.cross(side);
    let centre = end.vertex(middle, normal, Vec2::splat(0.5), Vec2::ZERO, Vec3::ONE);
    let radius = (0..sides)
        .map(|j| Vec3::from(bark.positions[(last + j) as usize]).distance(middle))
        .fold(0.0, f32::max)
        .max(0.01);
    for j in 0..=sides {
        let at = Vec3::from(bark.positions[(last + j) as usize]);
        let local = at - middle;
        let uv = Vec2::new(local.dot(side), local.dot(other)) / radius * 0.46 + 0.5;
        end.vertex(at, normal, uv, Vec2::ZERO, Vec3::ONE);
    }
    for j in 0..sides {
        end.indices.extend([centre, centre + 1 + j, centre + 2 + j]);
    }
    // Facing out along the normal, whichever way the ring ran.
    let (a, b, c) = (
        Vec3::from(end.positions[0]),
        Vec3::from(end.positions[1]),
        Vec3::from(end.positions[2]),
    );
    if (b - a).cross(c - a).dot(normal) < 0.0 {
        for tri in end.indices.chunks_mut(3) {
            tri.swap(1, 2);
        }
    }
    end
}

/// A log fallen from the forest's own trees: a trunk's length tapering toward its crown,
/// a little bent and sagging, lumpy, its foot sawn or broken and its crown's end broken
/// off in splinters, stubs of branches on it, sunk a little into the ground. The bark
/// above sees the sky and grows moss; below it is dark and wet.
pub(super) fn fallen_log(dice: &mut Rng) -> Log {
    let length = dice.range(4.0, 10.0);
    let radius = dice.range(0.22, 0.45);
    let bend = dice.range(-0.35, 0.35);
    let sides = 14;
    let steps = (length / 0.45).ceil() as u32;
    let phase = dice.range(0.0, TAU);
    let mut bark = Build::default();
    let wraps = (TAU * radius / BARK_M.x).round().max(1.0);
    let point = |t: f32| {
        Vec3::new(
            (t - 0.5) * length,
            radius * (1.0 - 0.3 * t) * 0.85 - 0.04 * libm::sinf(t * PI),
            bend * libm::sinf(t * PI),
        )
    };
    let radius_at = |t: f32| radius * (1.0 - 0.45 * t);
    // The sky it sees: most on top.
    let sky = |normal: Vec3| 0.3 + 0.25 * normal.y.max(0.0);
    let mut side = Vec3::Y;
    for k in 0..=steps {
        let t = k as f32 / steps as f32;
        let forward = (point((t + 0.01).min(1.0)) - point((t - 0.01).max(0.0))).normalize();
        side = (side - forward * side.dot(forward)).normalize();
        let r = radius_at(t);
        let along = t * length;
        ring(
            &mut bark,
            point(t),
            forward,
            side,
            sides,
            |angle| {
                // Lumpy, burred here and there, and a little flattened where it lies.
                let burr = libm::powf(0.5 + 0.5 * libm::sinf(along * 2.3 + phase * 3.0), 6.0)
                    * libm::powf(0.5 + 0.5 * libm::cosf(angle * 2.0 + along), 2.0);
                r * (1.0
                    + 0.1
                        * libm::sinf(3.0 * angle + along * 1.3 + phase)
                        * libm::sinf(along * 0.7 + 2.0 * angle)
                    + 0.04 * libm::cosf(7.0 * angle + phase + along * 0.4)
                    + 0.12 * burr
                    - 0.08 * libm::powf(0.5 + 0.5 * libm::sinf(angle), 3.0))
            },
            along / BARK_M.y,
            wraps,
            sky,
            Vec3::ONE,
        );
    }
    join_rings(&mut bark, 0, steps + 1, sides);
    // The crown's end broken off.
    let last = steps * (sides + 1);
    let tip_forward = (point(1.0) - point(0.98)).normalize();
    splinters(
        &mut bark,
        dice,
        last,
        sides,
        point(1.0),
        tip_forward,
        radius_at(1.0),
        length / BARK_M.y,
    );
    // Stubs of branches, mostly on its upper side, broken short.
    for _ in 0..2 + dice.below(4) {
        let t = dice.range(0.3, 0.92);
        let around = dice.range(-1.3, 1.3) + if dice.unit() < 0.5 { 0.0 } else { PI * 0.5 };
        let out = (Vec3::Y * libm::cosf(around) + Vec3::Z * libm::sinf(around)).normalize();
        let r = radius_at(t) * dice.range(0.22, 0.38);
        let reach = dice.range(0.15, 0.55);
        let foot = point(t) + out * radius_at(t) * 0.6;
        let lean = Vec3::X * dice.range(0.1, 0.6);
        let stub = Limb {
            points: vec![
                foot,
                foot + (out + lean).normalize() * (radius_at(t) * 0.4 + reach * 0.5),
                foot + (out + lean).normalize() * (radius_at(t) * 0.4 + reach),
            ],
            radii: vec![r, r * 0.8, r * 0.45],
            depth: 1,
            pointed: false,
        };
        let first = bark.positions.len();
        tube(&mut bark, &stub, 6, 1.0e4, BARK_M);
        for colour in &mut bark.colours[first..] {
            colour[3] = 0.5;
        }
    }
    // Its foot sawn, or broken too.
    let foot_forward = (point(0.0) - point(0.02)).normalize();
    let end = if dice.unit() < 0.6 {
        let up = (Vec3::Y - foot_forward * foot_forward.y).normalize();
        Some(cut_end(&bark, 0, sides, point(0.0), foot_forward, up))
    } else {
        // Splintered back along the log from its first ring.
        splinters(
            &mut bark,
            dice,
            0,
            sides,
            point(0.0),
            foot_forward,
            radius,
            0.0,
        );
        None
    };
    // Broken ends face out; the foot's splinters were wound for the other way round.
    fix_winding(&mut bark);
    Log {
        bark,
        end,
        length,
        radius,
    }
}

/// Turns every triangle to face the way its corners' normals do, so pieces built in either
/// order are seen from outside.
pub(super) fn fix_winding(build: &mut Build) {
    for tri in build.indices.chunks_mut(3) {
        let [a, b, c] = [tri[0], tri[1], tri[2]].map(|i| Vec3::from(build.positions[i as usize]));
        let normal = Vec3::from(build.normals[tri[0] as usize])
            + Vec3::from(build.normals[tri[1] as usize])
            + Vec3::from(build.normals[tri[2] as usize]);
        if (b - a).cross(c - a).dot(normal) < 0.0 {
            tri.swap(1, 2);
        }
    }
}

/// A stump: a trunk's foot with its roots spreading into the ground, sawn flat with its end
/// grain showing, or broken off in splinters; mossy where it sees the sky.
pub(in crate::experiments::lush) fn stump(dice: &mut Rng, sawn: bool) -> Wood {
    let radius = dice.range(0.3, 0.55);
    let height = if sawn {
        dice.range(0.3, 0.65)
    } else {
        dice.range(0.7, 1.3)
    };
    let sides = 18;
    let phase = dice.range(0.0, TAU);
    let roots = 4.0 + dice.below(3) as f32;
    let heights: Vec<f32> = [-0.3, -0.05, 0.06, 0.15, 0.27, 0.42, 0.6, 0.85, 1.1, 1.4]
        .into_iter()
        .filter(|y| *y < height - 0.05)
        .chain([height])
        .collect();
    let wraps = (TAU * radius / BARK_M.x).round().max(1.0);
    let tilt = Vec3::new(dice.range(-0.04, 0.04), 1.0, dice.range(-0.04, 0.04)).normalize();
    let mut bark = Build::default();
    for &y in &heights {
        let flare = 1.0 - smoothstep(-0.1, 0.55, y);
        ring(
            &mut bark,
            tilt * y,
            tilt,
            Vec3::X,
            sides,
            |angle| {
                let root = libm::powf(0.5 + 0.5 * libm::cosf(roots * angle + phase), 3.0);
                radius
                    * (1.0 + flare * flare * (1.1 * root + 0.12))
                    * (1.0 + 0.05 * libm::sinf(3.0 * angle + 2.0 * y + phase))
            },
            y / BARK_M.y,
            wraps,
            |normal| {
                (0.3 + 0.2 * normal.y.max(0.0) + 0.25 * smoothstep(0.0, 1.0, (y + 0.1) / height))
                    .min(1.0)
            },
            Vec3::ONE,
        );
    }
    join_rings(&mut bark, 0, heights.len() as u32, sides);
    let last = (heights.len() as u32 - 1) * (sides + 1);
    let top = tilt * height;
    let end = if sawn {
        // Sawn a little aslant.
        let normal =
            (tilt + Vec3::new(dice.range(-0.12, 0.12), 0.0, dice.range(-0.12, 0.12))).normalize();
        for j in 0..=sides {
            let p = Vec3::from(bark.positions[(last + j) as usize]);
            let lifted = p + tilt * (top - p).dot(normal) / tilt.dot(normal);
            bark.positions[(last + j) as usize] = lifted.into();
        }
        Some(cut_end(&bark, last, sides, top, normal, Vec3::X))
    } else {
        splinters(
            &mut bark,
            dice,
            last,
            sides,
            top,
            tilt,
            radius * 0.95,
            height / BARK_M.y,
        );
        None
    };
    fix_winding(&mut bark);
    Wood { bark, end, radius }
}
