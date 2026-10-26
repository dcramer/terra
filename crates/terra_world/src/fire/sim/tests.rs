use super::*;

fn placed(offset: f32, scale: f32) -> Placement {
    Placement {
        offset,
        turn: 0.0,
        mirror: 1.0,
        scale,
    }
}

#[test]
fn the_sidecar_and_the_volume_agree_on_its_size() {
    let sim = Sim::read();
    let bytes = std::fs::read(file(&format!("{SET}.dds"))).expect("the bake is committed");
    assert_eq!(dds_size(&bytes), Some(sim.size));
}

#[test]
fn a_fire_shows_the_frame_its_time_is_in_and_comes_round_to_its_start() {
    let sim = Sim::read();
    // A bake's frame on the clock: it plays `PLAY` times faster than baked.
    let step = 1.0 / (sim.fps * PLAY);
    let still = placed(0.0, 1.0);
    // A quarter of the way through frame 7, then frame 7's last, which blends toward 8.
    let (a, b, t) = sim.frames_at(7.25 * step, still);
    assert_eq!((a, b), (7, 8));
    assert!((t - 0.25).abs() < 1.0e-3, "{t}");
    // The last frame blends into the first; a whole loop later it is the same frames again.
    let (a, b, _) = sim.frames_at((sim.frames as f32 - 0.5) * step, still);
    assert_eq!((a, b), (sim.frames - 1, 0));
    let (a, b, t) = sim.frames_at(3.6, still);
    let (a2, b2, t2) = sim.frames_at(3.6 + sim.frames as f32 * step, still);
    assert_eq!((a, b), (a2, b2));
    assert!((t - t2).abs() < 1.0e-3);
    // Its offset moves it on so many of the bake's seconds; four times the bake's size plays
    // half as fast.
    assert_eq!(
        sim.frames_at(0.0, placed(2.0, 1.0)).0,
        sim.frames_at(2.0 / PLAY, still).0
    );
    let (a, _, _) = sim.frames_at(2.0, placed(0.0, 4.0));
    assert_eq!(a, sim.frames_at(1.0, still).0);
}

#[test]
fn a_frame_is_read_from_its_own_tile_and_no_other() {
    let sim = Sim::read();
    let cells = UVec3::from(sim.cells).as_vec3();
    let lo = Vec3::from(sim.box_min);
    let hi = lo + Vec3::from(sim.box_size);
    for frame in [
        0,
        1,
        sim.columns - 1,
        sim.columns,
        sim.frames / 2,
        sim.frames - 1,
    ] {
        let (col, row) = (frame % sim.columns, frame / sim.columns);
        let corner = UVec2::new(col * sim.cells[0], row * sim.cells[1]).as_vec2();
        // The box's corners and middle, all read within the frame's tile, half a cell in.
        for at in [lo, hi, (lo + hi) * 0.5] {
            let t = sim.texel(at, frame).expect("inside the box") - corner.extend(0.0);
            assert!(
                t.min_element() >= 0.5 && (cells - t).min_element() >= 0.5,
                "frame {frame}: {t} outside its tile"
            );
        }
        let high = sim.tile(frame) + UVec2::new(sim.cells[0], sim.cells[1]);
        assert!(
            high.x <= sim.size[0] && high.y <= sim.size[1],
            "frame {frame}"
        );
    }
    assert_eq!(sim.texel(lo - Vec3::X * 0.01, 0), None);
    assert_eq!(sim.texel(hi + Vec3::Z * 0.01, 0), None);
}

#[test]
fn the_frames_are_tiled_as_the_baker_tiled_them() {
    // Read from the committed bake: a frame is much more like the next than like one far off in
    // the loop, which holds only if the tiles are found in the order they were laid down.
    let sim = Sim::read();
    let bytes = std::fs::read(file(&format!("{SET}.dds"))).expect("the bake is committed");
    let data = &bytes[DDS_HEAD..];
    let flame = |texel: UVec3| {
        let (w, h) = (sim.size[0] / 4, sim.size[1] / 4);
        let block = ((texel.z * h + texel.y / 4) * w + texel.x / 4) as usize * 16;
        decode_bc4(&data[block..block + 8])[((texel.y % 4) * 4 + texel.x % 4) as usize]
    };
    // The flame in 4-cell cubes, as a tongue lives a frame or two and a cube's flame longer.
    let cubes = |f: u32| {
        let n = UVec3::from(sim.cells) / 4;
        let mut sums = vec![0u32; (n.x * n.y * n.z) as usize];
        for z in 0..n.z * 4 {
            for y in 0..n.y * 4 {
                for x in 0..n.x * 4 {
                    let at = UVec3::new(x, y, z);
                    let c = at / 4;
                    sums[((c.z * n.y + c.y) * n.x + c.x) as usize] +=
                        u32::from(flame(at + sim.tile(f).extend(0)));
                }
            }
        }
        sums
    };
    let all: Vec<Vec<u32>> = (0..sim.frames).map(cubes).collect();
    // How much of the two frames' flame is not shared, so two small flames are not alike for it.
    let change = |a: u32, b: u32| {
        let (a, b) = (&all[a as usize], &all[b as usize]);
        let apart: u32 = a.iter().zip(b).map(|(a, b)| a.abs_diff(*b)).sum();
        let both: u32 = a.iter().chain(b).sum();
        u64::from(apart) * 1_000_000 / u64::from(both.max(1))
    };
    // Of every other frame, the one most like a frame is nearly always one beside it in time
    // (the baker's own reader finds frame 200 most like 21: a gust comes round); read in another
    // order, a frame's neighbours would be its nearest by chance, 2 in 100.
    let beside = (0..sim.frames)
        .step_by(4)
        .filter(|&frame| {
            let nearest = (0..sim.frames)
                .filter(|&g| g != frame)
                .min_by_key(|&g| change(frame, g))
                .expect("other frames");
            nearest
                .abs_diff(frame)
                .min(sim.frames - nearest.abs_diff(frame))
                <= 2
        })
        .count();
    assert!(
        beside * 10 >= sim.frames as usize / 4 * 7,
        "{beside} of {}",
        sim.frames / 4
    );
}

#[test]
fn a_bc4_block_decodes_as_direct3d_says() {
    // Indices 0-7 in order and then again: 3 bits each, little end first.
    let indices: u64 = (0..16).fold(0, |bits, t| bits | ((t % 8) << (3 * t)));
    let mut block = [0u8; 8];
    block[2..8].copy_from_slice(&indices.to_le_bytes()[..6]);
    // Eight values: the endpoints, then six in sevenths from the first to the second.
    block[0] = 210;
    block[1] = 70;
    let eight = [210, 70, 190, 170, 150, 130, 110, 90];
    assert_eq!(decode_bc4(&block)[..8], eight);
    assert_eq!(decode_bc4(&block)[8..], eight);
    // Six: the endpoints, four in fifths, then 0 and 255.
    block[0] = 50;
    block[1] = 100;
    assert_eq!(decode_bc4(&block)[..8], [50, 100, 60, 70, 80, 90, 0, 255]);
}

#[test]
fn the_fallback_lays_each_block_where_its_texels_are() {
    // Two blocks across, one down, two slices: block b's red all 10 + b, its green 100 + b.
    let (w, h, d) = (8u32, 4u32, 2u32);
    let data: Vec<u8> = (0..4u8)
        .flat_map(|b| {
            [
                10 + b,
                10 + b,
                0,
                0,
                0,
                0,
                0,
                0,
                100 + b,
                100 + b,
                0,
                0,
                0,
                0,
                0,
                0,
            ]
        })
        .collect();
    let out = decode_bc5(&data, [w, h, d]);
    for z in 0..d {
        for y in 0..h {
            for x in 0..w {
                let b = (z * 2 + x / 4) as u8;
                let i = (((z * h + y) * w + x) * 2) as usize;
                assert_eq!([out[i], out[i + 1]], [10 + b, 100 + b], "texel {x} {y} {z}");
            }
        }
    }
}

#[test]
fn a_fire_turns_its_loop_downwind_and_its_mirror_keeps_the_wind() {
    for wind in [Vec2::X, Vec2::Y, Vec2::new(-0.6, 0.8)] {
        let p = Placement::of(0.37, BAKED_TALL, wind * 3.0, 8.0);
        // A point downwind of the foot is downwind in the bake, at its scale.
        let q = Vec3::new(wind.x, 0.5, wind.y) * 0.3;
        let b = p.to_bake(q);
        assert!(
            (b.x * p.scale - 0.3).abs() < 1.0e-4 && b.y.abs() < 1.0e-4,
            "{wind}: {b}"
        );
        // Up is stretched as well as scaled.
        assert!((stretched(b.z) * p.scale - 0.15).abs() < 1.0e-4);
        // Its mirror flips what is across the wind and nothing else.
        let across = Vec3::new(-wind.y, 0.0, wind.x) * 0.2;
        let flipped = Placement {
            mirror: -p.mirror,
            ..p
        };
        let (a, f) = (p.to_bake(q + across), flipped.to_bake(q + across));
        assert!((a.x - f.x).abs() < 1.0e-4 && (a.y + f.y).abs() < 1.0e-4 && a.y.abs() > 0.1);
    }
}

#[test]
fn neighbouring_fires_play_the_loop_apart() {
    let mut eighths = [0; 8];
    let mut mirrors = [0; 2];
    for i in 0..64 {
        let at = Vec3::new(i as f32 * 1.37, 0.0, (i / 8) as f32 * 2.11);
        let seed = (at.x * 0.731 + at.z * 0.377).rem_euclid(1.0);
        let p = Placement::of(seed, 1.2, Vec2::ZERO, 8.0);
        eighths[(p.offset / 8.0 * 8.0) as usize % 8] += 1;
        mirrors[usize::from(p.mirror < 0.0)] += 1;
        let own = 1.2 / BAKED_TALL;
        assert!(
            (p.scale / own - 1.0).abs() <= SCALE_SPREAD + 1.0e-5,
            "{}",
            p.scale
        );
    }
    assert!(
        eighths.iter().all(|&n| n >= 2),
        "offsets bunched: {eighths:?}"
    );
    assert!(mirrors.iter().all(|&n| n >= 16), "mirrors: {mirrors:?}");
}

#[test]
fn a_way_in_the_bake_comes_back_to_the_fires_frame() {
    for (turn, mirror) in [(0.0, 1.0), (0.7, -1.0), (2.9, 1.0), (-1.4, -1.0)] {
        let p = Placement {
            offset: 0.0,
            turn,
            mirror,
            scale: 1.0,
        };
        let q = Vec3::new(0.3, 0.8, -0.2);
        assert!(p.to_frame(p.to_bake(q)).distance(q) < 1e-5);
    }
}

#[test]
fn the_shader_stretches_the_loop_as_the_wisps_are() {
    let shader = std::fs::read_to_string(crate::data::assets().join("shaders/fire.wesl")).unwrap();
    assert!(shader.contains(&format!("const BAKED_STRETCH: f32 = {STRETCH:?};")));
    assert!(shader.contains(&format!("const BAKED_KNEE: f32 = {KNEE_M:?};")));
    assert!(shader.contains(&format!("const BAKED_FLAME_M: f32 = {FLAME_M:?};")));
}

#[test]
fn the_stretch_stands_the_flames_top_where_it_was_and_keeps_the_tongues_over_the_knee() {
    // The flame's top as tall as stretched all the way, a tongue over the knee as long as baked,
    // and the knee's height undone.
    assert!((stretched(FLAME_M) - FLAME_M * STRETCH).abs() < 1.0e-5);
    assert!((stretched(0.7) - stretched(0.5) - 0.2).abs() < 1.0e-5);
    for z in [-0.05, 0.1, KNEE_M, 0.6, 1.1] {
        assert!((unstretched(stretched(z)) - z).abs() < 1.0e-5, "{z}");
    }
}

#[test]
fn a_fire_plays_its_loop_on_the_worlds_clock_not_from_the_runs_start() {
    let sim = Sim::read();
    let fire = placed(1.3, 1.0);
    let seconds = sim.seconds();
    // A run started at `world` seconds on the world's clock, `now` seconds in, the clock held.
    let shown = |world: f64, now: f32| {
        let mut clock = LoopClock::default();
        clock.ahead(world, 0.0);
        sim.frames_at(now, fire.on_the_world(clock.ahead(world, now), seconds))
    };
    let hour = f64::from(DAY_SECONDS) / 24.0;
    // The motion film's dusk and night runs, 2.5 s in: each where the world's clock puts it,
    // so 3.6 h apart (270 s of play, 337.5 s of loop at its rate: 1.5 s on), not pose for pose.
    let (dusk, night) = (shown(18.4 * hour, 2.5), shown(22.0 * hour, 2.5));
    let on = (22.0 * hour * f64::from(fire.rate())).rem_euclid(f64::from(seconds)) as f32;
    assert_eq!(night.0, sim.frames_at(2.5 + on / fire.rate(), fire).0);
    assert_ne!(dusk.0, night.0, "dusk and night pose for pose");
    // Two machines at one moment, one 100 s into its run and one just started: one frame.
    let mut early = LoopClock::default();
    early.ahead(1000.0, 0.0);
    let a = sim.frames_at(
        100.0,
        fire.on_the_world(early.ahead(1100.0, 100.0), seconds),
    );
    let b = sim.frames_at(
        0.5,
        fire.on_the_world(LoopClock::default().ahead(1100.0, 0.5), seconds),
    );
    assert_eq!(a.0, b.0);
    // With the clock held the flame plays on.
    assert_ne!(shown(18.4 * hour, 2.5).0, shown(18.4 * hour, 3.0).0);
}
