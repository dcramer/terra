//! Profiles (`Piece::Trim`, `Piece::Band`): a section swept along an edge, mitred, jointed
//! and capped, and where a piece set into a wall is bedded.

use super::*;

/// Where a piece set into a wall is bedded: a lintel or a sill, swept along an opening's
/// head or sill and on past it into the wall either side. The wall's face is sunk behind
/// it and built up a little round its edges (`surface::Heights::bed`), so the piece sits
/// all but flush, its ends and its edge away from the opening lost in the wall, as cob or
/// rubble is built up round a timber, rather than standing on the face like a board.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bed {
    /// Along the edge, from one end of the piece to the other, on the wall's face.
    pub from: Vec3,
    pub to: Vec3,
    /// Out of the face, and up the face square to the edge.
    pub out: Vec3,
    pub up: Vec3,
    /// How far the piece reaches up the face from the edge, lowest and highest, in metres.
    pub low: f32,
    pub high: f32,
    /// How far its face stands out of the wall's.
    pub face: f32,
    /// Whether each end, at `from` and at `to`, is the piece's end, which the wall laps
    /// onto, rather than running on into the next cube's.
    pub ends: [bool; 2],
    /// Whether the opening is under it (a lintel) rather than over it (a sill): the edge
    /// along the opening is the one the wall does not lap onto.
    pub over_opening: bool,
    /// How far it reaches into the wall behind the face, in metres.
    pub back: f32,
}

impl Bed {
    /// How far a point of the wall's face is inside the piece's outline on the face, in
    /// metres, less than nothing outside it; none if the point is not on this face.
    pub fn inside(&self, p: Vec3) -> Option<f32> {
        let rel = p - self.from;
        // Only the face itself, not the opening's reveal behind it.
        if rel.dot(self.out).abs() > 1e-3 {
            return None;
        }
        let length = self.from.distance(self.to);
        let along = rel.dot((self.to - self.from) / length.max(1e-6));
        let up = rel.dot(self.up);
        // Past an end running on into the next cube's, that one's bed takes over; along
        // the opening's side, the piece's own edge shows.
        let edge = |lapped: bool, inset: f32| {
            if lapped || inset < 0.0 {
                inset
            } else {
                f32::MAX
            }
        };
        let inside = edge(self.ends[0], along)
            .min(edge(self.ends[1], length - along))
            .min(edge(!self.over_opening, up - self.low))
            .min(edge(self.over_opening, self.high - up));
        Some(inside)
    }

    /// As [`Bed::inside`], for a point of the opening's reveal the piece's underside (a
    /// lintel's) or top (a sill's) lies in: how far inside the piece's depth into the wall
    /// it is, with how far the piece stands out of it, nothing. The reveal is sunk behind
    /// the piece as the face is, else the two are drawn in one plane and fight.
    pub fn reveal(&self, p: Vec3) -> Option<f32> {
        let rel = p - self.from;
        let flush = if self.over_opening {
            self.low
        } else {
            self.high
        };
        if flush.abs() > 1e-3 || rel.dot(self.up).abs() > 1e-3 {
            return None;
        }
        let deep = -rel.dot(self.out);
        if deep < -1e-3 {
            return None;
        }
        let length = self.from.distance(self.to);
        let along = rel.dot((self.to - self.from) / length.max(1e-6));
        // Sunk through the cube it is set into, so the head is behind the lintel on each
        // face of a wall a cube thick, and in a thicker one behind each lintel all the way
        // to the next cube's, never in the plane of either; not on the wall's other face.
        // Along it only its own ends bound it, not where it runs on into the next cube's,
        // whose bed takes over past it.
        if (!self.ends[0] && along < 0.0) || (!self.ends[1] && along > length) {
            return None;
        }
        let end = |own: bool, inset: f32| if own { inset } else { f32::MAX };
        Some(
            (CELL_M - REVEAL_SHORT_M - deep)
                .min(end(self.ends[0], along))
                .min(end(self.ends[1], length - along)),
        )
    }

    /// Where a point of a wall's surface is against the piece, on its face or its reveal:
    /// how far inside the piece's outline, how far the piece stands out there, and whether
    /// the wall laps over its edges there: on its face, not on a reveal, where its edge
    /// away from the opening is inside the wall.
    pub fn at(&self, p: Vec3) -> Option<(f32, f32, bool)> {
        self.inside(p)
            .map(|inside| (inside, self.face, true))
            .or_else(|| self.reveal(p).map(|inside| (inside, 0.0, false)))
    }
}

/// The bed of a piece set into a wall, if it is one: a lintel or a sill swept along an
/// opening's head or sill (`Piece::Trim`, `Role::Head` or `Role::Sill`).
pub fn bed(piece: &Piece, kit: &Kit) -> Option<Bed> {
    let Piece::Trim {
        cube,
        role,
        side,
        other,
        ends,
        ..
    } = *piece
    else {
        return None;
    };
    if !matches!(role, layout::Role::Head | layout::Role::Sill) {
        return None;
    }
    let Some(Dress::Profile { name, extend, .. }) = kit.dress.roles.get(&role) else {
        return None;
    };
    let profile = kit.dress.made.as_deref()?.profiles.get(name)?;
    let low = profile.iter().map(|p| p[1]).fold(f32::MAX, f32::min);
    let high = profile.iter().map(|p| p[1]).fold(f32::MIN, f32::max);
    let face = profile.iter().map(|p| p[0]).fold(0.0f32, f32::max);
    let edge = layout::roles::Edge { role, side, other };
    let (from, to) = layout::roles::line(cube, edge);
    let at = |p: [i32; 3]| IVec3::from_array(p).as_vec3() * (CELL_M / TICKS as f32);
    let (from, to) = (at(from), at(to));
    let along = (to - from).normalize_or(Vec3::X);
    let reach = ends.map(|end| if end == End::Square { *extend } else { 0.0 });
    Some(Bed {
        from: from - along * reach[0],
        to: to + along * reach[1],
        out: unit(side),
        up: Vec3::Y,
        low,
        high,
        face,
        ends: ends.map(|end| !matches!(end, End::On | End::Step)),
        over_opening: role == layout::Role::Head,
        back: -profile.iter().map(|p| p[0]).fold(0.0f32, f32::min),
    })
}

/// A stone's shade and where its texture falls, from its seed: each a part of the texture
/// of its own, a little lighter or darker and warmer or cooler, as stones from one bed are.
pub(super) fn stone_look(seed: u32, tint: [f32; 3]) -> ([f32; 3], [f32; 2]) {
    let shade = 0.86 + 0.16 * layout::unit(hash(&[seed, 3]));
    let warm = 0.06 * (layout::unit(hash(&[seed, 4])) - 0.5);
    (
        [
            tint[0] * shade * (1.0 + warm),
            tint[1] * shade,
            tint[2] * shade * (1.0 - warm),
        ],
        [
            layout::unit(hash(&[seed, 1])),
            layout::unit(hash(&[seed, 2])),
        ],
    )
}

/// A profile swept along one cube's edge.
pub(super) struct Sweep {
    pub(super) from: Vec3,
    pub(super) to: Vec3,
    /// Out from the face it looks out of, and up from the line, square to both it and the
    /// way out: straight up along a level edge, square to a slope along a raking one.
    pub(super) side: Vec3,
    pub(super) up: Vec3,
    pub(super) ends: [End; 2],
    pub(super) extend: f32,
    pub(super) joints: bool,
    pub(super) lift: f32,
    pub(super) sink: f32,
    pub(super) tint: [f32; 3],
    /// Whether its texture runs along it, as timber's grain does, rather than from the
    /// world's planes, as stone's is.
    pub(super) grain: bool,
    /// Whether its part set into the wall is all out of sight, as a plinth's is: at a
    /// square end it is cut back square to the corner's diagonal, so two plinths meeting
    /// at an outside corner from its two faces, each ended square where the ground steps,
    /// keep each to its own side of it rather than lying in one another in the wall.
    pub(super) buried: bool,
}

/// How a sweep's two ends are cut, worked out once from them: run on, mitred out or back,
/// shared with a cube corner to corner, bent where a raking line meets a level one, carried
/// on past the line, set back into the wall, jointed, or cut plumb.
pub(super) struct Cut {
    mitre: [f32; 2],
    shared: [bool; 2],
    bend: [f32; 2],
    extend: [f32; 2],
    recess: [f32; 2],
    joint: [bool; 2],
    plumb: [f32; 2],
    /// Whether the line slopes, and which way it runs level.
    raking: bool,
    level: Vec3,
    lift: f32,
    length: f32,
}

impl Cut {
    fn mitre_at(&self, which: usize, o: f32) -> f32 {
        if self.shared[which] {
            if o > 0.0 { -1.0 } else { 1.0 }
        } else {
            self.mitre[which]
        }
    }

    /// Where along the line the section's point `(o, u)` starts.
    fn start(&self, o: f32, u: f32) -> f32 {
        -self.extend[0] - o * self.mitre_at(0, o) + (-o).max(0.0) * self.recess[0]
            - (u - self.lift) * self.bend[0]
            + (u - self.lift) * self.plumb[0]
    }

    /// Where along the line the section's point `(o, u)` ends.
    fn end(&self, o: f32, u: f32) -> f32 {
        self.length + self.extend[1] + o * self.mitre_at(1, o) - (-o).max(0.0) * self.recess[1]
            + (u - self.lift) * self.bend[1]
            + (u - self.lift) * self.plumb[1]
    }

    /// `at` along the line, kept between where the point `(o, u)` starts and ends; where a
    /// short line is mitred at both ends, so a point far in from the face would pass itself,
    /// where it meets its mirror in the middle.
    fn within(&self, [o, u]: [f32; 2], at: f32) -> f32 {
        let (low, high) = (self.start(o, u), self.end(o, u));
        if low <= high {
            at.clamp(low, high)
        } else {
            (low + high) * 0.5
        }
    }
}

/// A sweep made ready to draw: its section's points and their normals, how its ends are
/// cut, and the rings along it.
pub(super) struct Swept<'a> {
    sweep: &'a Sweep,
    points: Vec<[f32; 2]>,
    normals: Vec<[f32; 2]>,
    cut: Cut,
    along: Vec3,
    /// Each ring's place along the line, and whether it is pulled in as a joint.
    rings: Vec<(f32, bool)>,
}

impl Swept<'_> {
    /// A point of the section, at a ring, pulled in where the ring is a joint.
    fn point(&self, ring: usize, i: usize, seg: usize) -> Vec3 {
        let (at, inset) = self.rings[ring];
        let [mut o, mut u] = self.points[i];
        if inset {
            let nn = vertex_normal(&self.normals, seg, if i == seg { 0 } else { 1 });
            o -= nn[0] * JOINT_DEPTH_M;
            u -= nn[1] * JOINT_DEPTH_M;
        }
        let a = self.cut.within(self.points[i], at);
        let s = self.sweep;
        s.from + self.along * a + s.side * o + s.up * u
    }
}

impl Sweep {
    /// How its ends are cut, along a line `length` long running `along`.
    fn cut(&self, length: f32, along: Vec3) -> Cut {
        // How each end is cut: run on, mitred out or back, or square.
        let mitre = self.ends.map(|end| match end {
            End::Out => 1.0,
            End::Back => -1.0,
            _ => 0.0,
        });
        // Shared with a cube corner to corner: out to the turn in the wall, back to the
        // plane halfway between the two out of it.
        let shared = self.ends.map(|end| end == End::Shared);
        // Mitred in its own plane where a ramp's raking coping meets a level one: on the
        // line halving the 45 degrees between them, over or under the bend.
        let bend = self.ends.map(|end| match end {
            End::Over => BEND_MITRE,
            End::Under => -BEND_MITRE,
            _ => 0.0,
        });
        let extend = self.ends.map(|end| match end {
            End::Square => self.extend,
            _ => 0.0,
        });
        // A square end's part set into the wall cut back from its line, so it is not in the
        // plane of the wall's end or an opening's reveal there, nor of the next piece's;
        // and a joint's or a step's, at a cube's side, so it is not in the plane of a step
        // in the footing or the wall under it there.
        let recess = self.ends.map(|end| match end {
            End::Square | End::Free if (self.extend == 0.0 || end == End::Free) && self.buried => {
                BURIED_RECESS
            }
            End::Square | End::Free if self.extend == 0.0 || end == End::Free => SQUARE_RECESS,
            End::On | End::Step => SQUARE_RECESS,
            _ => 0.0,
        });
        let joint = self
            .ends
            .map(|end| self.joints && matches!(end, End::On | End::Step));
        // A raking band's square end cut plumb, as a raking coping's is, so nothing of its top
        // leans out past the cube's face it ends at: along it as far, for each metre up
        // square to it, as its up leans back over its own line.
        let level = Vec3::new(along.x, 0.0, along.z);
        let raking = along.y.abs() > 1e-3 && level.length() > 1e-3;
        let lean = if raking {
            let h = level.normalize();
            -self.up.dot(h) / along.dot(h)
        } else {
            0.0
        };
        let plumb = self.ends.map(|end| {
            if matches!(end, End::Square | End::Free) {
                lean
            } else {
                0.0
            }
        });
        Cut {
            mitre,
            shared,
            bend,
            extend,
            recess,
            joint,
            plumb,
            raking,
            level,
            lift: self.lift,
            length,
        }
    }

    pub(super) fn draw(
        &self,
        profile: &[[f32; 2]],
        metres: [f32; 2],
        seed: u32,
        out: &mut Indexed,
    ) {
        if profile.len() < 2 {
            return;
        }
        // Raised to step with the ground, its lowest points run on down to reach it.
        let low = profile.iter().map(|p| p[1]).fold(f32::MAX, f32::min);
        let points: Vec<[f32; 2]> = profile
            .iter()
            .map(|&[o, u]| {
                let u = u + self.lift;
                [
                    o,
                    if u - self.lift <= low + 1e-4 {
                        u - self.sink
                    } else {
                        u
                    },
                ]
            })
            .collect();
        let normals = segment_normals(&points);
        let length = self.from.distance(self.to);
        let along = (self.to - self.from) / length;
        let cut = self.cut(length, along);
        // Rings along it: at each end, and in from a joint by its chamfer.
        let mut rings: Vec<(f32, bool)> = vec![(f32::MIN, cut.joint[0])];
        if cut.joint[0] {
            rings.push((JOINT_M, false));
        }
        if cut.joint[1] {
            rings.push((length - JOINT_M, false));
        }
        rings.push((f32::MAX, cut.joint[1]));
        let swept = Swept {
            sweep: self,
            points,
            normals,
            cut,
            along,
            rings,
        };
        let (colour, shift) = stone_look(seed, self.tint);
        let shift = if self.joints { shift } else { [0.0; 2] };
        swept.faces(metres, shift, colour, out);
        swept.close(metres, shift, colour, out);
    }
}

impl Swept<'_> {
    /// A point's place in its texture, from the world's planes, as stone's is.
    fn uv(p: Vec3, normal: Vec3, metres: [f32; 2], shift: [f32; 2]) -> [f32; 2] {
        let t = planar(p, normal, metres);
        [t[0] + shift[0], t[1] + shift[1]]
    }

    /// Its faces along it, from ring to ring round its section.
    fn faces(&self, metres: [f32; 2], shift: [f32; 2], colour: [f32; 3], out: &mut Indexed) {
        let (points, normals, rings) = (&self.points, &self.normals, &self.rings);
        let s = self.sweep;
        let n = points.len();
        // How far round the section each point is, for a texture that runs along it.
        let mut round = vec![0.0f32; n];
        for i in 1..n {
            let (a, b) = (points[i - 1], points[i]);
            round[i] = round[i - 1] + ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
        }
        let along_uv = |ring: usize, i: usize| {
            let at = self.cut.within(points[i], rings[ring].0);
            [round[i] / metres[0] + shift[0], at / metres[1] + shift[1]]
        };
        for ring in 0..rings.len() - 1 {
            for seg in 0..n - 1 {
                let quad = [
                    (self.point(ring, seg, seg), vertex_normal(normals, seg, 0)),
                    (
                        self.point(ring + 1, seg, seg),
                        vertex_normal(normals, seg, 0),
                    ),
                    (
                        self.point(ring + 1, seg + 1, seg),
                        vertex_normal(normals, seg, 1),
                    ),
                    (
                        self.point(ring, seg + 1, seg),
                        vertex_normal(normals, seg, 1),
                    ),
                ];
                let grained = [
                    along_uv(ring, seg),
                    along_uv(ring + 1, seg),
                    along_uv(ring + 1, seg + 1),
                    along_uv(ring, seg + 1),
                ];
                let face = (quad[2].0 - quad[0].0).cross(quad[3].0 - quad[1].0);
                if face.length_squared() < 1e-12 {
                    continue;
                }
                let world_normal =
                    |nn: [f32; 2]| (s.side * nn[0] + s.up * nn[1]).normalize_or(s.up);
                let flat = (s.side * normals[seg][0] + s.up * normals[seg][1]).normalize_or(s.up);
                let mut k = 0;
                let corners = quad.map(|(p, nn)| {
                    let at = if s.grain {
                        grained[k]
                    } else {
                        Self::uv(p, flat, metres, shift)
                    };
                    k += 1;
                    out.corner(p, world_normal(nn), at, colour)
                });
                // Wound to face out.
                let outward = face.dot(flat) >= 0.0;
                for [a, b, c] in [[0, 1, 2], [0, 2, 3]] {
                    if outward {
                        out.indices.extend([corners[a], corners[b], corners[c]]);
                    } else {
                        out.indices.extend([corners[a], corners[c], corners[b]]);
                    }
                }
            }
        }
    }

    /// Each end closed, so a piece whose neighbour is not yet in, or never will be, is
    /// never seen hollow: across its joint's chamfer, or its whole section. A plumb end
    /// looks level along it.
    fn close(&self, metres: [f32; 2], shift: [f32; 2], colour: [f32; 3], out: &mut Indexed) {
        let (cut, along, s) = (&self.cut, self.along, self.sweep);
        let n = self.points.len();
        let ahead = if cut.raking {
            cut.level.normalize()
        } else {
            along
        };
        for (which, ring, outward) in [(0, 0, -along), (1, self.rings.len() - 1, along)] {
            if cut.mitre[which] != 0.0 || cut.bend[which] != 0.0 || cut.shared[which] {
                continue;
            }
            let outward = if cut.plumb[which] != 0.0 {
                ahead * outward.dot(along).signum()
            } else {
                outward
            };
            let ring_points: Vec<Vec3> =
                (0..n).map(|i| self.point(ring, i, i.min(n - 2))).collect();
            let flat: Vec<[f32; 2]> = ring_points
                .iter()
                .map(|p| {
                    let rel = *p - s.from;
                    [rel.dot(s.side), rel.dot(s.up)]
                })
                .collect();
            let points: Vec<glam::Vec2> = flat.iter().map(|&p| p.into()).collect();
            for tri in crate::mesh::ear_clip(&points) {
                // None across points in a line in the section, a sliver with no area there:
                // where its end is cut back off the line, as a square end's part set into
                // the wall is, it would lie in a face of the piece's own, its underside.
                let [p, q, r] = tri.map(|i| flat[i]);
                if ((q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0])).abs() < 1e-7 {
                    continue;
                }
                let c = tri.map(|i| ring_points[i]);
                let face = (c[1] - c[0]).cross(c[2] - c[0]);
                let tri = if face.dot(outward) >= 0.0 {
                    tri
                } else {
                    [tri[0], tri[2], tri[1]]
                };
                for i in tri {
                    let p = ring_points[i];
                    let index = out.corner(p, outward, Self::uv(p, outward, metres, shift), colour);
                    out.indices.push(index);
                }
            }
        }
    }
}

/// A section's outward normal at each of its segments, in (out, up): the side away from
/// the cube, the solid being on the side of the cube's inside.
pub(super) fn segment_normals(points: &[[f32; 2]]) -> Vec<[f32; 2]> {
    // Which side is solid: by the winding of the section closed on itself.
    let area: f32 = (0..points.len())
        .map(|i| {
            let (a, b) = (points[i], points[(i + 1) % points.len()]);
            a[0] * b[1] - b[0] * a[1]
        })
        .sum();
    points
        .windows(2)
        .map(|w| {
            let d = [w[1][0] - w[0][0], w[1][1] - w[0][1]];
            let l = libm::sqrtf(d[0] * d[0] + d[1] * d[1]).max(1e-9);
            if area > 0.0 {
                [d[1] / l, -d[0] / l]
            } else {
                [-d[1] / l, d[0] / l]
            }
        })
        .collect()
}

/// A section's point's normal at either end of a segment: shared with the next segment's
/// where they meet at less than a crease, its own where they meet at more.
pub(super) fn vertex_normal(normals: &[[f32; 2]], seg: usize, end: usize) -> [f32; 2] {
    let own = normals[seg];
    let other = if end == 0 {
        seg.checked_sub(1).map(|s| normals[s])
    } else {
        normals.get(seg + 1).copied()
    };
    match other {
        Some(o) if own[0] * o[0] + own[1] * o[1] > CREASE => {
            let s = [own[0] + o[0], own[1] + o[1]];
            let l = libm::sqrtf(s[0] * s[0] + s[1] * s[1]).max(1e-9);
            [s[0] / l, s[1] / l]
        }
        _ => own,
    }
}

/// How many triangles a profile swept along a cube's edge has at most: along it in three
/// lengths, between its joints' chamfers, and its two ends closed.
pub fn trim_triangles(profile: &[[f32; 2]]) -> usize {
    let segments = profile.len().saturating_sub(1);
    segments * 3 * 2 + 2 * profile.len().saturating_sub(2)
}

/// A trim swept along one cube's edge, as its role's profile; false if the kit has none.
pub(super) fn trim(
    piece: &Piece,
    kit: &Kit,
    pieces: &Pieces,
    (grain, metres): (bool, [f32; 2]),
    made: &mut Indexed,
) -> bool {
    let Piece::Trim {
        cube,
        role,
        side,
        other,
        ends,
        lift,
        sink,
        seed,
    } = *piece
    else {
        return false;
    };
    let Some(Dress::Profile {
        name,
        extend,
        joints,
        ..
    }) = kit.dress.roles.get(&role)
    else {
        return false;
    };
    let Some(profile) = pieces.profiles.get(name) else {
        return false;
    };
    let edge = layout::roles::Edge { role, side, other };
    let (from, to) = layout::roles::line(cube, edge);
    let at = |p: [i32; 3]| IVec3::from_array(p).as_vec3() * (CELL_M / TICKS as f32);
    let (mut from, mut to) = (at(from), at(to));
    // A border sunk into the floor it lies on, ended square where that floor ends
    // in a face, stops a hair short of it, its end not in that face's plane.
    if role == layout::Role::Border {
        let along = (to - from).normalize_or_zero();
        if matches!(ends[0], End::Square | End::Free) {
            from += along * BORDER_SHORT_M;
        }
        if matches!(ends[1], End::Square | End::Free) {
            to -= along * BORDER_SHORT_M;
        }
    }
    let sweep = Sweep {
        from,
        to,
        side: unit(side),
        up: Vec3::Y,
        ends,
        extend: *extend,
        joints: *joints,
        // A soffit hangs a little under the underside it runs along, so its own
        // underside is not in that face's plane.
        lift: layout::metres(lift)
            - if role == layout::Role::Soffit {
                SOFFIT_DROP_M
            } else {
                0.0
            },
        sink: layout::metres(sink),
        tint: kit.dress.tint,
        grain,
        buried: role == layout::Role::Foot,
    };
    sweep.draw(profile, metres, seed, made);
    true
}

/// A band swept along a line of a cube cut to a shape, as its role's profile; false if the
/// kit has none.
pub(super) fn band(
    piece: &Piece,
    kit: &Kit,
    pieces: &Pieces,
    (grain, metres): (bool, [f32; 2]),
    made: &mut Indexed,
) -> bool {
    let Piece::Band {
        role,
        from,
        to,
        side,
        ends,
        seed,
        ..
    } = *piece
    else {
        return false;
    };
    let Some(Dress::Profile {
        name,
        extend,
        joints,
        ..
    }) = kit.dress.roles.get(&role)
    else {
        return false;
    };
    let Some(profile) = pieces.profiles.get(name) else {
        return false;
    };
    // On a shape cut in half cubes, a step a half cube deep: a nose modelled for a
    // step a cube deep reaches back no further than its own and a hair into what
    // rises behind it, not on into the stone of the stair over it, through whose
    // face its back would show, nor ending in the plane of the face behind it.
    let back = -CELL_M * 0.5 - BAND_PAST_M;
    let profile: Vec<[f32; 2]> = profile.iter().map(|&[x, y]| [x.max(back), y]).collect();
    let profile = &profile;
    let (from, to) = (from.metres(), to.metres());
    let along = (to - from).normalize_or(Vec3::X);
    let side = unit(side);
    let sweep = Sweep {
        from,
        to,
        side,
        up: along.cross(side).normalize_or(Vec3::Y) * along.cross(side).y.signum(),
        ends,
        extend: *extend,
        joints: *joints,
        lift: 0.0,
        sink: 0.0,
        tint: kit.dress.tint,
        grain,
        buried: false,
    };
    sweep.draw(profile, metres, seed, made);
    true
}
