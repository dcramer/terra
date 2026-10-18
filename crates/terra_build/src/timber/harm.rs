//! A member harmed: chunks broken out of it along the grain where it has lost most, its
//! fresh wood bared, and its sawn ends splintered.

use super::*;

/// How deep a kit's timber loses chunks where it is harmed most (`Breaks::Grain`), in
/// metres: none if its kit does not say.
pub(super) fn chunk_depth(kit: &Kit) -> f32 {
    [Material::Log, Material::Hewn]
        .iter()
        .find_map(|material| match kit.damage.get(material) {
            Some(crate::kit::Breaks::Grain { chunks, .. }) => Some(*chunks),
            _ => None,
        })
        .unwrap_or(0.0)
}

/// From how much lost a member starts to lose chunks, and the share of the places a chunk
/// could break from that do where it has lost everything.
pub(super) const CHUNK_FROM: f32 = 0.45;
pub(super) const CHUNK_MOST: f32 = 0.8;
/// How far apart along a member the places a chunk could break from are, in metres, and how
/// long a chunk is along the grain, the least and the most.
pub(super) const CHUNK_EVERY_M: f32 = 0.16;
pub(super) const CHUNK_LONG_M: [f32; 2] = [0.14, 0.32];
/// How far apart a member's rings are where it is broken, so a chunk's ends are its own.
pub(super) const CHUNK_RING_M: f32 = 0.025;
/// How far back a sawn end splinters where it is harmed most, at its longest splinter.
pub(super) const SPLINTER_M: f32 = 0.14;

/// Whether a member has lost enough anywhere along it to have lost chunks: read every tenth
/// of a metre along it, on each of its four sides.
pub(super) fn harmed_along(frame: &Frame, radius: f32, harm: Harm) -> bool {
    let count = (frame.length / 0.1).ceil().max(1.0) as usize;
    (0..=count).any(|i| {
        let at = frame.from + frame.axis * (frame.length * i as f32 / count as f32);
        [frame.side, -frame.side, frame.up, -frame.up]
            .into_iter()
            .any(|out| harm(at + out * radius, out) > CHUNK_FROM)
    })
}

/// A member broken where it is harmed (D305), as wood breaks: chunks split or chopped out
/// of it, each cut by flat faces, an impact's one slanting into it along the grain or an
/// axe's two meeting in a V across it, their edges hard; the fresh wood under them bared
/// (its texture's worn set, `Texture::worn`, its grain running along the member) and pale;
/// its sawn ends splintered back into long thin spikes along the grain. Its rings are drawn
/// close where it has lost enough, so a cut has its own edges; the rest of it, between
/// chunks, keeps its round. Points move only in toward its middle or back from its end, so
/// it stays closed and faces out.
pub(super) fn break_up(made: &mut Member, frame: &Frame, harm: Harm, depth: f32, seed: u32) {
    let middle_of = |ring: &[Vec3]| ring.iter().copied().sum::<Vec3>() / ring.len().max(1) as f32;
    let along = |ring: &[Vec3]| (middle_of(ring) - frame.from).dot(frame.axis);
    let out_at = |p: Vec3, middle: Vec3| {
        let d = p - middle;
        (d - frame.axis * d.dot(frame.axis)).normalize_or_zero()
    };
    let unit = |a: u32, b: i32, c: i32| terra_gen::hash::unit3(seed, a, b as u32, c as u32);
    let length = frame.length;
    let radius = made.grid[0]
        .iter()
        .map(|p| out_at(*p, middle_of(&made.grid[0])).dot(*p - middle_of(&made.grid[0])))
        .fold(0.0f32, f32::max)
        .max(0.05);
    // The chunks: at places along it, each round it where its seed says, broken out as
    // more is lost there.
    struct Chunk {
        from: f32,
        to: f32,
        /// The flat faces it is cut by, each a point on it and its normal out of the wood.
        faces: [(Vec3, Vec3); 2],
    }
    let mut chunks: Vec<Chunk> = Vec::new();
    let places = (length / CHUNK_EVERY_M).floor().max(1.0) as i32;
    for k in 0..places {
        let r = |salt: u32| unit(salt, k, 7);
        let middle_along = (k as f32 + 0.2 + 0.6 * r(1)) * length / places as f32;
        let turn = r(2) * TAU;
        let out = (frame.side * libm::cosf(turn) + frame.up * libm::sinf(turn)).normalize();
        let centre = frame.from + frame.axis * middle_along;
        let share =
            ((harm(centre + out * radius, out) - CHUNK_FROM) / (1.0 - CHUNK_FROM)).clamp(0.0, 1.0);
        if r(3) >= share * CHUNK_MOST {
            continue;
        }
        let long = CHUNK_LONG_M[0] + (CHUNK_LONG_M[1] - CHUNK_LONG_M[0]) * r(4);
        let (from, to) = (
            (middle_along - long * 0.5).max(0.0),
            (middle_along + long * 0.5).min(length),
        );
        let deep = depth * (0.5 + 0.5 * r(5)) * (0.6 + 0.4 * share);
        let floor = centre + out * (radius - deep);
        // An axe's V across the grain, or an impact's one face slanting in along it from
        // one end, its other end broken off square.
        let (a, b) = if r(6) < 0.5 {
            let slope = 0.9 + 0.8 * r(7);
            (
                (floor, (out + frame.axis * slope).normalize()),
                (floor, (out - frame.axis * slope).normalize()),
            )
        } else {
            let sign = if r(7) < 0.5 { 1.0 } else { -1.0 };
            let slope = 0.25 + 0.3 * r(8);
            let tilt = (out + frame.axis * sign * slope).normalize();
            ((floor, tilt), (floor, tilt))
        };
        chunks.push(Chunk {
            from,
            to,
            faces: [a, b],
        });
    }
    // Rings close where a chunk is, and over the ends where they splinter.
    let end_harm = [frame.from, frame.from + frame.axis * length].map(|end| {
        [frame.side, -frame.side, frame.up, -frame.up]
            .into_iter()
            .map(|out| harm(end + out * radius, out))
            .fold(0.0f32, f32::max)
    });
    let splinters = end_harm.map(|h| ((h - CHUNK_FROM) / (1.0 - CHUNK_FROM)).clamp(0.0, 1.0));
    let mut spans: Vec<(f32, f32)> = chunks
        .iter()
        .map(|c| (c.from - 0.03, c.to + 0.03))
        .collect();
    if splinters[0] > 0.0 {
        spans.push((0.0, SPLINTER_M + 0.04));
    }
    if splinters[1] > 0.0 {
        spans.push((length - SPLINTER_M - 0.04, length));
    }
    let mut places = Vec::new();
    for (from, to) in spans {
        let count = ((to - from) / CHUNK_RING_M).ceil() as usize;
        places.extend((0..=count).map(|k| from + (to - from) * k as f32 / count.max(1) as f32));
    }
    places.sort_by(f32::total_cmp);
    places.dedup_by(|a, b| (*a - *b).abs() < 0.004);
    rings_at(made, frame.from, frame.axis, &places);
    let rows = made.grid.len();
    let columns = made.grid[0].len();
    if made.wear.is_empty() {
        made.wear = vec![vec![0.0; columns]; rows];
    }
    made.cut = vec![vec![false; columns]; rows];
    for i in 0..rows {
        let at = along(&made.grid[i]).clamp(0.0, length);
        for j in 0..columns {
            let mut p = made.grid[i][j];
            let mut cut = false;
            for chunk in &chunks {
                if at < chunk.from || at > chunk.to {
                    continue;
                }
                // Out of the wood past both faces: onto the nearer.
                let past = chunk.faces.map(|(on, normal)| (p - on).dot(normal));
                if past[0] > 0.0 && past[1] > 0.0 {
                    let k = if past[0] < past[1] { 0 } else { 1 };
                    p -= chunk.faces[k].1 * past[k];
                    cut = true;
                }
            }
            // A sawn end splintered back, a spike left standing here and there.
            for (end, share) in splinters.into_iter().enumerate() {
                let from_end = if end == 0 { at } else { length - at };
                if share <= 0.0 || from_end > SPLINTER_M + 0.03 {
                    continue;
                }
                let spike = unit(4 + end as u32, j as i32, 0);
                let reach = if spike < 0.3 {
                    SPLINTER_M * share * 0.08
                } else {
                    SPLINTER_M * share * (0.35 + 0.65 * unit(6 + end as u32, j as i32, 1))
                };
                let inward = if end == 0 { frame.axis } else { -frame.axis };
                let lift = reach * (1.0 - from_end / (SPLINTER_M + 0.03)).clamp(0.0, 1.0);
                if lift > 1e-4 {
                    p += inward * lift;
                    cut = true;
                }
            }
            if cut {
                made.grid[i][j] = p;
                made.cut[i][j] = true;
                made.wear[i][j] = 1.0;
                if let Some(tone) = made.tone.get_mut(i).and_then(|ring| ring.get_mut(j)) {
                    *tone *= FRESH_WOOD;
                }
            }
        }
    }
}

/// How much paler the fresh wood under a chunk is than the face round it.
pub(super) const FRESH_WOOD: f32 = 1.18;
