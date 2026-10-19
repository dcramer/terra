use super::*;
use crate::draw::tests::facing_out;
use terra_build::kit::Timber;
use terra_build::testing as layouts;
use terra_build::{CELL_M, Panel};
use terra_build_layout::layout::roles::Dir;
use terra_build_layout::layout::{Place, lay};

fn log(seed: u32, split: Split) -> Piece {
    Piece::Log {
        from: Place::near(Vec3::new(0.0, 0.125, 0.25)),
        to: Place::near(Vec3::new(3.0, 0.125, 0.25)),
        split,
        seed,
        chinking: Some((0, terra_build_layout::layout::ticks(3.0))),
        lean: [0, 0],
        lift: [0, 0],
        role: None,
        outside: None,
    }
}

fn corners(piece: &Piece, kit: &Kit) -> Vec<Vec3> {
    let mut surfaces = Surfaces::default();
    draw(piece, kit, &mut surfaces);
    surfaces
        .0
        .values()
        .flat_map(|s| s.positions.clone())
        .collect()
}

/// FNV-1a over the bits of every corner.
fn checksum(points: &[Vec3]) -> u64 {
    points
        .iter()
        .flat_map(|p| p.to_array())
        .fold(0xcbf2_9ce4_8422_2325_u64, |sum, v| {
            (sum ^ u64::from(v.to_bits())).wrapping_mul(0x0100_0000_01b3)
        })
}

/// A log's own numbers, written here, so retuning pine does not move it.
fn pinned(profile: Profile) -> Kit {
    Kit {
        diameter: 0.3,
        course: 0.25,
        timber: Timber {
            profile,
            ..Timber::default()
        },
        ..Kit::default()
    }
}

/// The most of a line between two courses that shows anything behind the logs, in
/// metres (D178 as amended): what a builder's chinking or a scribe's moss would be.
const MOST_GAP_M: f32 = 0.02;

/// Where a ray from `origin` along `way` first meets a triangle, as how far along it.
fn hit(origin: Vec3, way: Vec3, [a, b, c]: [Vec3; 3]) -> Option<f32> {
    let (e1, e2) = (b - a, c - a);
    let p = way.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-12 {
        return None;
    }
    let s = origin - a;
    let u = s.dot(p) / det;
    let q = s.cross(e1);
    let v = way.dot(q) / det;
    let t = e2.dot(q) / det;
    (u >= -1e-6 && v >= -1e-6 && u + v <= 1.0 + 1e-6 && t > 0.0).then_some(t)
}

#[test]
fn courses_of_a_wall_sit_tight_showing_no_more_than_a_thin_line_between() {
    // Two courses of a wall, many seeds each, round logs and hewn beams, looked at square
    // on from both sides: every 5 cm along, the tallest run of rays a millimetre apart
    // up the line between them that meet the daub, or nothing, before either member. "why do the logs have big gaps between them? thats not how you'd
    // build w/ logs irl" (the developers, 2026-10-15).
    // Every shipped coursed material, and pine scribed full.
    let logs = terra_build::testing::shipped("logs.ron");
    let mut scribed = logs.clone();
    scribed.timber.joint = 0.0;
    for kit in [logs, terra_build::testing::shipped("hewn.ron"), scribed] {
        let (profile, joint) = (kit.timber.profile, kit.timber.joint);
        let mut worst = 0.0f32;
        for seed in 0..16 {
            let mut surfaces = Surfaces::default();
            for (n, y) in [0.125, 0.375].into_iter().enumerate() {
                let piece = Piece::Log {
                    from: Place::near(Vec3::new(0.0, y, 0.25)),
                    to: Place::near(Vec3::new(3.0, y, 0.25)),
                    split: Split::Whole,
                    seed: seed * 7 + n as u32,
                    chinking: Some((0, terra_build_layout::layout::ticks(3.0))),
                    lean: [0, 0],
                    lift: [0, 0],
                    role: None,
                    outside: None,
                };
                draw(&piece, &kit, &mut surfaces);
            }
            // Each triangle, and whether it is the daub's.
            let triangles: Vec<([Vec3; 3], bool)> = [Material::Log, Material::Daub]
                .into_iter()
                .flat_map(|material| {
                    let all = surfaces.0[&material].indexed();
                    all.indices
                        .chunks_exact(3)
                        .map(|t| {
                            (
                                [0, 1, 2].map(|k| all.positions[t[k] as usize]),
                                material == Material::Daub,
                            )
                        })
                        .collect::<Vec<_>>()
                })
                .collect();
            for step in 0..=48 {
                let x = 0.3 + step as f32 * 0.05;
                let near: Vec<&([Vec3; 3], bool)> = triangles
                    .iter()
                    .filter(|(t, _)| {
                        let lo = t.iter().map(|p| p.x).fold(f32::MAX, f32::min);
                        let hi = t.iter().map(|p| p.x).fold(f32::MIN, f32::max);
                        lo <= x && x <= hi
                    })
                    .collect();
                for side in [1.0f32, -1.0] {
                    let mut run = 0.0f32;
                    for k in 0..=250 {
                        let y = 0.125 + k as f32 * 0.001;
                        let origin = Vec3::new(x, y, 0.25 + side);
                        let way = Vec3::new(0.0, 0.0, -side);
                        // What it meets first: a member, or the daub between, or nothing.
                        let first = near
                            .iter()
                            .filter_map(|(t, daub)| hit(origin, way, *t).map(|d| (d, *daub)))
                            .min_by(|a, b| a.0.total_cmp(&b.0));
                        let stopped = matches!(first, Some((_, false)));
                        run = if stopped { 0.0 } else { run + 0.001 };
                        worst = worst.max(run);
                    }
                }
            }
        }
        assert!(
            worst <= MOST_GAP_M.max(joint) + 0.002,
            "{profile:?} with a joint of {joint} m: a line {worst} m tall shows between courses"
        );
    }
}

#[test]
fn a_seed_gives_the_same_log_on_every_machine_and_another_seed_another() {
    let (round, hewn) = (pinned(Profile::Round), pinned(Profile::Hewn));
    let sums = [
        checksum(&corners(&log(7, Split::Whole), &round)),
        checksum(&corners(&log(7, Split::Top), &round)),
        checksum(&corners(&log(7, Split::Whole), &hewn)),
    ];
    assert_eq!(
        sums,
        [
            0x8534_2bbd_ef45_a122,
            0xcb16_b69c_89d4_cf4c,
            0x5ece_288a_9a84_250d
        ],
        "drawn differently from the machine these were taken on (D30)"
    );
    // The same log with another seed is another log, of the same kind.
    for kit in [&round, &hewn] {
        let (a, b) = (
            corners(&log(7, Split::Whole), kit),
            corners(&log(8, Split::Whole), kit),
        );
        assert_ne!(a, b);
        assert_eq!(a, corners(&log(7, Split::Whole), kit));
    }
}

#[test]
fn logs_vary_in_thickness_and_shade_by_seed_within_the_kits_bounds() {
    let mut kit = pinned(Profile::Round);
    // Its shade alone varying by seed, not its warmth, mottle or weathering.
    let v = &mut kit.timber.variation;
    (v.warmth, v.mottle, v.age, v.stain, v.lichen) = (0.0, 0.0, 0.0, 0.0, 0.0);
    let mut thickest: f32 = 0.0;
    let mut thinnest = f32::MAX;
    let mut shades = Vec::new();
    for seed in 0..64 {
        let mut surfaces = Surfaces::default();
        draw(&log(seed, Split::Whole), &kit, &mut surfaces);
        let face = &surfaces.0[&Material::Log];
        // How far its middle ring reaches from the log's line.
        let middle = face
            .positions
            .iter()
            .filter(|p| (p.x - 1.5).abs() < 0.05)
            .map(|p| ((p.y - 0.125).powi(2) + (p.z - 0.25).powi(2)).sqrt())
            .fold(0.0, f32::max);
        thickest = thickest.max(middle);
        thinnest = thinnest.min(middle);
        shades.push(face.colours[0][1]);
    }
    let v = &kit.timber.variation;
    let r = kit.radius();
    // Thicker and thinner by about the variation and the bow, never by much more.
    assert!(
        thickest - thinnest > r * v.thickness,
        "{thinnest} to {thickest}"
    );
    assert!(
        thickest < r * 1.45 && thinnest > r * 0.8,
        "{thinnest} to {thickest}"
    );
    let (light, dark) = shades
        .iter()
        .fold((0.0f32, 1.0f32), |(l, d), s| (l.max(*s), d.min(*s)));
    // Darker by up to the kit's shade, as a share of the lightest, the same corner of each.
    let spread = 1.0 - dark / light;
    assert!(
        spread > v.shade * 0.5 && spread <= v.shade + 1e-3,
        "{dark} to {light}"
    );
}

#[test]
fn a_wall_log_is_shaded_toward_its_grooves_and_its_ends_draw_their_own_pictures() {
    let mut kit = pinned(Profile::Round);
    kit.timber.groove = 0.4;
    kit.timber.knots = 0.0;
    kit.timber.variation.mottle = 0.0;
    kit.textures.insert(
        Material::LogEnd,
        terra_build::kit::Texture {
            path: "ends".into(),
            metres: [1.0, 1.0],
            depth: 0.0,
            reflectance: terra_build::kit::matte(),
            strips: None,
            cells: 2,
            worn: None,
        },
    );
    let mut surfaces = Surfaces::default();
    draw(&log(5, Split::Whole), &kit, &mut surfaces);
    let face = &surfaces.0[&Material::Log];
    // Its middle's corners, by how high on it they are: under it darkest, its side
    // lightest, its top between.
    let lightness = |from: f32, to: f32| {
        let picked: Vec<f32> = face
            .positions
            .iter()
            .zip(&face.colours)
            .filter(|(p, _)| (p.x - 1.5).abs() < 0.3 && (from..to).contains(&(p.y - 0.125)))
            .map(|(_, c)| c[1])
            .collect();
        picked.iter().sum::<f32>() / picked.len().max(1) as f32
    };
    let (under, side, top) = (
        lightness(-1.0, -0.12),
        lightness(-0.03, 0.03),
        lightness(0.12, 1.0),
    );
    assert!(under < top && top < side, "{under} {top} {side}");
    // Its underside shaded by the sun not bleaching it, no more: the shade where the sky
    // does not reach is every kit's (`dress::underside`), and stacked crushed it (D373).
    assert!(under > side * 0.8, "{under} {side}");
    // Each end within one cell of the atlas, and across seeds every cell drawn.
    let mut cells = std::collections::BTreeSet::new();
    for seed in 0..16 {
        let mut surfaces = Surfaces::default();
        draw(&log(seed, Split::Whole), &kit, &mut surfaces);
        let ends = &surfaces.0[&Material::LogEnd];
        for corner in ends.uvs.chunks(3 * 18) {
            let cell = |uv: &[f32; 2]| ((uv[0] * 2.0) as i32, (uv[1] * 2.0) as i32);
            let first = cell(&corner[0]);
            assert!(
                corner.iter().all(|uv| cell(uv) == first),
                "an end across cells"
            );
            cells.insert(first);
        }
    }
    assert_eq!(cells.len(), 4, "{cells:?}");
}

#[test]
fn a_walls_sawn_ends_differ_from_one_another_and_some_are_greyed() {
    let kit = pinned(Profile::Round);
    let mut ends: Vec<[f32; 3]> = Vec::new();
    for seed in 0..24 {
        let mut surfaces = Surfaces::default();
        draw(&log(seed, Split::Whole), &kit, &mut surfaces);
        let drawn = &surfaces.0[&Material::LogEnd];
        // Each end one colour across it, its two ends their own.
        let half = drawn.colours.len() / 2;
        for colours in [&drawn.colours[..half], &drawn.colours[half..]] {
            assert!(colours.iter().all(|c| c == &colours[0]));
            ends.push(colours[0]);
        }
    }
    let lightness = |c: &[f32; 3]| (c[0] + c[1] + c[2]) / 3.0;
    let (dark, light) = ends
        .iter()
        .map(lightness)
        .fold((f32::MAX, f32::MIN), |(l, h), v| (l.min(v), h.max(v)));
    assert!(
        light - dark > 0.08,
        "every end the same lightness: {dark} {light}"
    );
    // As warm as the tint at most, and greyer: red against blue spread out.
    let warmth = |c: &[f32; 3]| c[0] - c[2];
    let tint = kit.timber.end_tint;
    let (cool, warm) = ends
        .iter()
        .map(warmth)
        .fold((f32::MAX, f32::MIN), |(l, h), v| (l.min(v), h.max(v)));
    assert!(
        cool < (tint[0] - tint[2]) * 0.7,
        "none greyed: {cool} {warm}"
    );
    assert!(warm - cool > 0.05, "{cool} {warm}");
}

#[test]
fn a_stringers_head_is_cut_level_under_the_top_of_the_cube_it_reaches() {
    // A flight's stringer, up from the ground to the top of a storey: its head cut level
    // under the top of its highest cube, out of the plane of a tread laid flush with that
    // top, as the village's outside stair to an upper door has; cut in that plane, the
    // two fought for 58 cm².
    let kit = pinned(Profile::Hewn);
    let ticks = |m: f32| terra_build_layout::layout::ticks(m);
    let stringer = Piece::Plank {
        from: Place::near(Vec3::new(0.0, 0.1, 0.0)),
        to: Place::near(Vec3::new(0.0, 3.0, 2.9)),
        wide: ticks(0.07),
        high: ticks(0.32),
        seed: 5,
    };
    let mut surfaces = Surfaces::default();
    plank(&stringer, &kit, &mut surfaces);
    let all = surfaces.0[&Material::Floor].indexed();
    let top = all.positions.iter().fold(f32::MIN, |m, p| m.max(p.y));
    assert!(top < 3.0 - 0.002, "its head at {top}");
    assert!(top > 3.0 - 0.02, "its head at {top}, short of the deck");
}

#[test]
fn a_plank_drawn_from_one_board_of_its_texture_is_not_smeared_down_its_sides() {
    let mut kit = pinned(Profile::Hewn);
    kit.textures.insert(
        Material::Floor,
        terra_build::kit::Texture {
            path: "planks".into(),
            metres: [2.0, 2.0],
            depth: 0.0,
            reflectance: terra_build::kit::matte(),
            strips: Some([0.06, 0.11]),
            cells: 1,
            worn: None,
        },
    );
    // A joist, 5 cm wide and 20 cm deep, three metres long.
    let ticks = |m: f32| terra_build_layout::layout::ticks(m);
    let joist = Piece::Plank {
        from: Place::near(Vec3::new(0.0, 0.0, 0.0)),
        to: Place::near(Vec3::new(3.0, 0.0, 0.0)),
        wide: ticks(0.05),
        high: ticks(0.2),
        seed: 3,
    };
    let mut surfaces = Surfaces::default();
    plank(&joist, &kit, &mut surfaces);
    let all = surfaces.0[&Material::Floor].indexed();
    let mut sides = 0;
    for t in all.indices.chunks_exact(3) {
        let [a, b, c] = [t[0], t[1], t[2]].map(|i| i as usize);
        let normal = (all.positions[b] - all.positions[a])
            .cross(all.positions[c] - all.positions[a])
            .normalize_or_zero();
        let ys = [a, b, c].map(|i| all.positions[i].y);
        let high =
            ys.iter().fold(f32::MIN, |m, &y| m.max(y)) - ys.iter().fold(f32::MAX, |m, &y| m.min(y));
        if normal.z.abs() < 0.9 || high < 0.05 {
            continue;
        }
        sides += 1;
        // Up the side, its texture runs across the plank as it does across its top,
        // about as many metres of it a metre: not one column of it all the way down.
        let us = [a, b, c].map(|i| all.uvs[i][0]);
        let spread =
            us.iter().fold(f32::MIN, |m, &u| m.max(u)) - us.iter().fold(f32::MAX, |m, &u| m.min(u));
        let metres_a_metre = spread * 2.0 / high;
        assert!(
            (0.5..2.0).contains(&metres_a_metre),
            "a side's triangle stretched: {us:?} over {high} m"
        );
    }
    assert!(sides > 0);
}

#[test]
fn a_weathered_face_is_laid_with_its_top_up_and_its_weather_face_to_the_weather() {
    let mut kit = pinned(Profile::Round);
    kit.timber.exposed = true;
    // Both ways along the same line: the face looking +Z, toward the weather, takes the
    // texture's first quarter, the top its middle.
    for (from, to) in [(0.0, 3.0), (3.0, 0.0)] {
        let piece = Piece::Log {
            from: Place::near(Vec3::new(from, 0.125, 0.25)),
            to: Place::near(Vec3::new(to, 0.125, 0.25)),
            split: Split::Whole,
            seed: 4,
            chinking: None,
            lean: [0, 0],
            lift: [0, 0],
            role: None,
            outside: None,
        };
        let mut surfaces = Surfaces::default();
        draw(&piece, &kit, &mut surfaces);
        let face = &surfaces.0[&Material::Log];
        let all = face
            .positions
            .iter()
            .zip(&face.uvs)
            .chain(face.shared.positions.iter().zip(&face.shared.uvs));
        let middle: Vec<_> = all.filter(|(p, _)| (p.x - 1.5).abs() < 0.1).collect();
        let most = |key: &dyn Fn(&Vec3) -> f32| {
            middle
                .iter()
                .max_by(|a, b| key(a.0).total_cmp(&key(b.0)))
                .map(|(_, uv)| uv[0].rem_euclid(1.0))
                .unwrap()
        };
        let (top, weather) = (most(&|p| p.y), most(&|p| p.z));
        assert!((top - 0.5).abs() < 0.08, "{from}: top at {top}");
        assert!(
            (weather - 0.25).abs() < 0.08,
            "{from}: weather face at {weather}"
        );
    }
}

#[test]
fn a_log_wears_to_bare_wood_at_its_ends_and_on_its_face_out_of_its_room() {
    let mut kit = pinned(Profile::Round);
    kit.timber.exposed = true;
    kit.textures.insert(
        Material::Log,
        terra_build::kit::Texture {
            path: "bark".into(),
            metres: [1.0, 2.0],
            depth: 0.0,
            reflectance: terra_build::kit::matte(),
            strips: None,
            cells: 1,
            worn: Some("bare".into()),
        },
    );
    // Along X; its room to the north (-Z), so its outside is south, against the way the
    // weather comes for a log walling nothing (from the north-west).
    for (outside, out_z) in [(Some(Dir::South), 1.0), (Some(Dir::North), -1.0)] {
        let piece = Piece::Log {
            from: Place::near(Vec3::new(0.0, 0.125, 0.25)),
            to: Place::near(Vec3::new(3.0, 0.125, 0.25)),
            split: Split::Whole,
            seed: 4,
            chinking: None,
            lean: [0, 0],
            lift: [0, 0],
            role: None,
            outside,
        };
        let mut surfaces = Surfaces::default();
        draw(&piece, &kit, &mut surfaces);
        let face = surfaces.0[&Material::Log].indexed();
        assert_eq!(face.wear.len(), face.positions.len());
        let mean = |keep: &dyn Fn(&Vec3) -> bool| {
            let picked: Vec<f32> = face
                .positions
                .iter()
                .zip(&face.wear)
                .filter(|(p, _)| keep(p))
                .map(|(_, w)| *w)
                .collect();
            picked.iter().sum::<f32>() / picked.len().max(1) as f32
        };
        let middle = |p: &Vec3| (p.x - 1.5).abs() < 0.4;
        let side = |sign: f32| move |p: &Vec3| middle(p) && (p.z - 0.25) * sign > 0.08;
        let (out, inside) = (mean(&side(out_z)), mean(&side(-out_z)));
        assert!(out > inside + 0.15, "{outside:?}: out {out}, in {inside}");
        // Its sawn ends lose their bark whichever way it faces; its middle underside keeps it.
        let ends = mean(&|p: &Vec3| p.x < 0.1 || p.x > 2.9);
        let under = mean(&|p: &Vec3| middle(p) && p.y < 0.05);
        assert!(ends > 0.7, "ends {ends}");
        assert!(under < 0.2, "underside {under}");
    }
}

#[test]
fn a_log_harmed_past_half_loses_chunks_out_of_its_outline_and_splinters_at_its_end() {
    let mut kit = pinned(Profile::Round);
    kit.damage.insert(
        Material::Log,
        terra_build::kit::Breaks::Grain {
            splits: 5.0,
            chunks: 0.06,
        },
    );
    // Its west half all but gone, its east half whole.
    let harm = |p: Vec3, _: Vec3| if p.x < 1.5 { 0.95 } else { 0.0 };
    let piece = log(3, Split::Whole);
    let (mut whole, mut harmed) = (Surfaces::default(), Surfaces::default());
    draw(&piece, &kit, &mut whole);
    draw_harmed(&piece, &kit, Some(&harm), &mut harmed);
    // How near its middle line it comes between two places along it.
    let nearest = |surfaces: &Surfaces, from: f32, to: f32| {
        surfaces.0[&Material::Log]
            .positions
            .iter()
            .filter(|p| p.x > from && p.x < to)
            .map(|p| glam::Vec2::new(p.y - 0.125, p.z - 0.25).length())
            .fold(f32::MAX, f32::min)
    };
    let lost = nearest(&whole, 0.2, 1.3) - nearest(&harmed, 0.2, 1.3);
    assert!(lost > 0.025, "chunks out of its outline: {lost}");
    let kept = nearest(&whole, 1.7, 2.8) - nearest(&harmed, 1.7, 2.8);
    assert!(kept.abs() < 0.004, "whole where not harmed: {kept}");
    // Broken in places, round between them: not the whole of its harmed half eaten.
    let round = nearest(&whole, 0.2, 1.3) - 0.004;
    let points: Vec<f32> = harmed.0[&Material::Log]
        .positions
        .iter()
        .filter(|p| p.x > 0.2 && p.x < 1.3)
        .map(|p| glam::Vec2::new(p.y - 0.125, p.z - 0.25).length())
        .collect();
    let broken = points.iter().filter(|r| **r < round).count();
    assert!(
        broken * 50 > points.len() && broken * 2 < points.len(),
        "{broken} of {} points broken",
        points.len()
    );
    // Its west end splintered back unevenly, its east end sawn square.
    let spread = |surfaces: &Surfaces, near: f32| {
        let xs: Vec<f32> = surfaces.0[&Material::LogEnd]
            .positions
            .iter()
            .map(|p| p.x)
            .filter(|x| (x - near).abs() < 0.2)
            .collect();
        xs.iter().copied().fold(f32::MIN, f32::max) - xs.iter().copied().fold(f32::MAX, f32::min)
    };
    assert!(spread(&harmed, 0.0) > 0.02, "{}", spread(&harmed, 0.0));
    assert!(spread(&whole, 0.0) < 0.005 && spread(&harmed, 3.0) < 0.005);
    // And still facing out from its middle line, clear of its ends, where splinters
    // turn its faces every way.
    let log = &harmed.0[&Material::Log];
    let mut inward = 0;
    for corners in log.positions.chunks(3) {
        if corners.iter().all(|p| p.x > 0.2 && p.x < 2.8) {
            let own = (corners[1] - corners[0]).cross(corners[2] - corners[0]);
            let centre = (corners[0] + corners[1] + corners[2]) / 3.0;
            let out = Vec3::new(0.0, centre.y - 0.125, centre.z - 0.25);
            if own.dot(out) <= 0.0 {
                inward += 1;
            }
        }
    }
    assert_eq!(inward, 0, "triangles facing in");
    for material in [Material::Log, Material::LogEnd] {
        assert!(harmed.0[&material].positions.iter().all(|p| p.is_finite()));
    }
}

#[test]
fn every_log_is_drawn_facing_out_and_whole() {
    for profile in [Profile::Round, Profile::Hewn] {
        let kit = pinned(profile);
        for split in [Split::Whole, Split::Top, Split::Bottom] {
            let mut surfaces = Surfaces::default();
            draw(&log(3, split), &kit, &mut surfaces);
            for material in [Material::Log, Material::LogEnd, Material::Daub] {
                let what = format!("{profile:?} {split:?} {material:?}");
                let surface = &surfaces.0[&material];
                assert!(facing_out(surface, &what) > 0, "{what}");
                assert_eq!(surface.uvs.len(), surface.positions.len(), "{what}");
                assert!(surface.normals.iter().all(|n| n.is_finite()), "{what}");
            }
        }
    }
    // An upright post too.
    let post = Piece::Log {
        from: Place::near(Vec3::new(0.25, 0.0, 0.25)),
        to: Place::near(Vec3::new(0.25, 3.0, 0.25)),
        split: Split::Whole,
        seed: 1,
        chinking: None,
        lean: [0, 0],
        lift: [0, 0],
        role: None,
        outside: None,
    };
    let mut surfaces = Surfaces::default();
    draw(&post, &pinned(Profile::Round), &mut surfaces);
    assert!(facing_out(&surfaces.0[&Material::Log], "post") > 0);
}

#[test]
fn a_walls_face_is_where_its_members_are_drawn() {
    // Where a frame is seated on a wall (`Kit::face`), measured on the logs and beams
    // drawn: half how wide each is across the wall level with its line, at its middle,
    // over many seeds, is the face give or take its bark and unevenness.
    for profile in [Profile::Round, Profile::Hewn] {
        let kit = pinned(profile);
        let reaches: Vec<f32> = (0..48)
            .map(|seed| {
                let mut surfaces = Surfaces::default();
                draw(&log(seed, Split::Whole), &kit, &mut surfaces);
                // Half its width across the wall, so its bow one way or the other is
                // not counted as its face.
                let (low, high) = surfaces.0[&Material::Log]
                    .positions
                    .iter()
                    .filter(|p| (p.x - 1.5).abs() < 0.1 && (p.y - 0.125).abs() < 0.03)
                    .fold((f32::MAX, f32::MIN), |(low, high), p| {
                        (low.min(p.z), high.max(p.z))
                    });
                (high - low) * 0.5
            })
            .collect();
        let mean = reaches.iter().sum::<f32>() / reaches.len() as f32;
        assert!(
            (mean - kit.face()).abs() < 0.012,
            "{profile:?}: drawn out to {mean} on average, its face {}",
            kit.face()
        );
    }
}

#[test]
fn a_runs_logs_stay_within_its_cubes_and_the_wall_across() {
    // A cabin's logs, drawn: nothing past the cubes along a run, nor out of the
    // wall's cube across it by more than a log's bark and bow.
    for profile in [Profile::Round, Profile::Hewn] {
        let kit = pinned(profile);
        let mut base = layouts::cabin();
        layouts::wall(&mut base, [[3, 0, 5], [4, 0, 5]], Panel::Door);
        for piece in lay(&base, &kit).pieces {
            let Piece::Log { from, to, .. } = piece else {
                continue;
            };
            let (from, to) = (from.metres(), to.metres());
            let frame = Frame::new(from, to);
            let mut surfaces = Surfaces::default();
            draw(&piece, &kit, &mut surfaces);
            for p in surfaces.0.values().flat_map(|s| &s.positions) {
                let along = (*p - from).dot(frame.axis);
                assert!(
                    along >= -1e-3 && along <= frame.length + 1e-3,
                    "{profile:?}: {p} past the ends of {from} {to}"
                );
                let across = (*p - from).dot(frame.side).abs();
                assert!(across <= CELL_M * 0.5, "{profile:?}: {p} out of the wall");
            }
        }
    }
}
