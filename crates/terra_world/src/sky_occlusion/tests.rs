use super::*;

fn wood(crowns: Vec<Crown>) -> SkyOccluders {
    SkyOccluders {
        middle: Vec3::ZERO,
        up: Vec3::Y,
        east: Vec3::X,
        half_m: 40.0,
        below_m: 5.0,
        above_m: 35.0,
        crowns,
        shadows: Vec::new(),
    }
}

fn cell_at(seen: &Seen, o: &SkyOccluders, p: Vec3) -> usize {
    let size = Vec3::new(o.half_m * 2.0, o.above_m + o.below_m, o.half_m * 2.0);
    let rel = p + Vec3::new(o.half_m, o.below_m, o.half_m);
    let c = (rel / (size / seen.cells.as_vec3())).as_uvec3();
    ((c.z * seen.cells.y + c.y) * seen.cells.x + c.x) as usize
}

#[test]
fn the_volume_is_in_the_world_only_once_it_is_lit_and_the_old_one_until_then() {
    // D485: a fragment inside a volume takes its diffuse light from the volume alone, so
    // a volume gathered before its light is written blacks out all it holds. Frame by
    // frame, as the game runs it (the work on other threads, polled): no unlit volume
    // is ever there to gather; and when the crowns change, the volume lit for the old
    // ones stays until the new one is lit, so the wood is never without its shade.
    AsyncComputeTaskPool::get_or_init(bevy::tasks::TaskPool::new);
    let mut app = App::new();
    app.insert_resource(Options::from_args(std::iter::empty()).unwrap())
        .init_resource::<Assets<Image>>()
        .insert_resource(crate::skylight::clear_sky(
            Vec3::new(0.3, 0.5, 0.8),
            Quat::IDENTITY,
        ))
        .insert_resource(wood(Vec::new()))
        .add_systems(Update, (bake, receive, light).chain());
    let mut frames = 0;
    let mut until = |app: &mut App, done: &dyn Fn(&[Handle<Image>]) -> bool, held: usize| {
        loop {
            app.update();
            let volumes = lit_volumes(app);
            assert!(volumes.len() >= held && volumes.len() <= 1, "{volumes:?}");
            if done(&volumes) {
                return volumes[0].clone();
            }
            frames += 1;
            assert!(frames < 20_000, "the volume was never lit");
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    };
    let first = until(&mut app, &|volumes| !volumes.is_empty(), 0);
    app.world_mut()
        .resource_mut::<SkyOccluders>()
        .crowns
        .clear();
    let second = until(&mut app, &|volumes| volumes[0] != first, 1);
    assert_ne!(first, second);
    assert_eq!(
        app.world_mut().query::<&Name>().iter(app.world()).count(),
        1
    );
}

#[test]
fn open_ground_sees_the_whole_sky() {
    let o = wood(Vec::new());
    let seen = seen(&o);
    let cell = cell_at(&seen, &o, Vec3::new(3.0, 1.0, -7.0));
    assert!(
        seen.sides[cell].iter().all(|s| (s - 1.0).abs() < 1e-4),
        "{:?}",
        seen.sides[cell]
    );
}

#[test]
fn under_a_closed_canopy_little_sky_is_seen_from_below_and_more_from_the_side() {
    // Crowns every ten metres over the middle of the square, each hiding nine tenths of
    // the sky straight up through it, their edges meeting.
    let mut crowns = Vec::new();
    for i in -2..=2 {
        for j in -2..=2 {
            crowns.push(Crown {
                middle: Vec3::new(i as f32 * 10.0, 15.0, j as f32 * 10.0),
                half: Vec3::new(7.0, 5.0, 7.0),
                closes: 0.9,
            });
        }
    }
    let o = wood(crowns);
    let seen = seen(&o);
    let under = cell_at(&seen, &o, Vec3::new(0.0, 1.0, 0.0));
    let up = seen.sides[under][2];
    assert!(up < 0.4, "looking up under it sees {up} of the sky");
    let side = seen.sides[under][0];
    assert!(
        side > up,
        "a side sees the low sky under the crown's edge: {side} against {up}"
    );
    let above = cell_at(&seen, &o, Vec3::new(0.0, 30.0, 0.0));
    assert!(
        seen.sides[above][2] > 0.99,
        "above the crown the sky is open"
    );
    let away = cell_at(&seen, &o, Vec3::new(35.0, 1.0, 35.0));
    assert!(seen.sides[away][2] > seen.sides[under][2] + 0.3);
}

#[test]
fn in_a_tall_stand_the_low_sky_is_hidden_by_the_stems() {
    // Crowns 22 to 30 m up, one every 4 m over the whole square, as a tall pine stand's.
    let mut crowns = Vec::new();
    for i in -10..10 {
        for j in -10..10 {
            crowns.push(Crown {
                middle: Vec3::new(i as f32 * 4.0 + 2.0, 26.0, j as f32 * 4.0 + 2.0),
                half: Vec3::new(2.5, 4.0, 2.5),
                closes: 0.6,
            });
        }
    }
    let o = wood(crowns);
    let seen = seen(&o);
    let under = cell_at(&seen, &o, Vec3::new(1.0, 1.5, -1.0));
    // Stems half a metre across, one a 16 m² cell, hide 0.03 of a level look a metre;
    // the low looks run 40 m or more to the square's edge under the crowns, so a side sees
    // a third of its sky at most (exp(-1.2) along the lowest), against all of it without.
    for side in [0, 1, 4, 5] {
        let s = seen.sides[under][side];
        assert!(s < 0.4, "side {side} sees {s} of its sky");
    }
    let above = cell_at(&seen, &o, Vec3::new(1.0, 33.5, -1.0));
    assert!(
        seen.sides[above][2] > 0.99,
        "above the crowns the sky is open"
    );
}

#[test]
fn the_sky_overhead_is_closed_under_a_crown_and_open_above_it_and_outside() {
    // A canopy over the middle of the square, as in the test above.
    let mut crowns = Vec::new();
    for i in -2..=2 {
        for j in -2..=2 {
            crowns.push(Crown {
                middle: Vec3::new(i as f32 * 10.0, 15.0, j as f32 * 10.0),
                half: Vec3::new(7.0, 5.0, 7.0),
                closes: 0.9,
            });
        }
    }
    let o = wood(crowns);
    let seen = seen(&o);
    let under = open_at(&o, &seen, Vec3::new(0.5, 1.7, -0.5));
    let above = open_at(&o, &seen, Vec3::new(0.5, 30.0, -0.5));
    let beside = open_at(&o, &seen, Vec3::new(36.0, 1.7, 36.0));
    assert!(under < 0.5, "under the crown {under}");
    assert!(above > 0.99, "above it {above}");
    assert!(beside > under + 0.3, "beside it {beside}");
    assert_eq!(open_at(&o, &seen, Vec3::new(90.0, 1.7, 0.0)), 1.0);
    // Between cells it changes smoothly, not in steps.
    let a = open_at(&o, &seen, Vec3::new(10.0, 1.7, 0.0));
    let b = open_at(&o, &seen, Vec3::new(10.5, 1.7, 0.0));
    assert!((a - b).abs() < 0.1, "{a} then {b}");
}

fn unpack(v: u32) -> Vec3 {
    let e = (v >> 27) as i32 - 15 - 9;
    let s = libm::exp2f(e as f32);
    Vec3::new(
        (v & 511) as f32 * s,
        (v >> 9 & 511) as f32 * s,
        (v >> 18 & 511) as f32 * s,
    )
}

/// The light the volume's texture gives a surface of world normal `n` in a cell, read as
/// Bevy's shader reads it (`irradiance_volume.wesl`, `irradiance_volume_light`): the
/// normal unturned by the volume's own turn; for each axis the slab of that axis, a third
/// of the depth, and in it the upper half of the height (`neg_offset` of 0.5) where the
/// normal's part along the axis is negative; the three blended by the parts' squares.
fn as_bevy_reads(data: &[u8], cells: UVec3, cell: UVec3, n: Vec3) -> Vec3 {
    let mut light = Vec3::ZERO;
    for (axis, part) in n.to_array().into_iter().enumerate() {
        let t = cell.y + if part < 0.0 { cells.y } else { 0 };
        let p = cell.z + axis as u32 * cells.z;
        let at = (((p * cells.y * 2 + t) * cells.x + cell.x) * 4) as usize;
        let texel = u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        light += unpack(texel) * part * part;
    }
    light
}

#[test]
fn under_an_open_sky_the_volume_lights_what_faces_the_sun_and_the_sky_most() {
    // D490: a shaded side facing the sun's half of the sky takes more light than one
    // facing away, and (the sky being brighter than the ground's bounce) what faces up
    // more than what faces down. The painting is turned into the world as the
    // environment map's is, and the volume as a forest's is, east of north.
    let cells = UVec3::new(2, 2, 2);
    let seen = Seen {
        cells,
        sides: vec![[1.0; 6]; 8],
        up: vec![1.0; 8],
    };
    let luma = |c: Vec3| c.dot(Vec3::new(0.2126, 0.7152, 0.0722));
    let frame = Quat::from_rotation_y(0.7);
    for sun in [
        Vec3::new(-0.95, 0.23, -0.2),
        Vec3::new(0.3, 0.5, 0.8),
        Vec3::new(0.0, 0.9, 0.4),
    ] {
        let open = crate::skylight::clear_sky(sun, frame);
        let data = texels(&seen, &world_sides(&open));
        let flat = Vec3::new(sun.x, 0.0, sun.z).normalize();
        let read = |n: Vec3| as_bevy_reads(&data, cells, UVec3::ONE, n);
        let (toward, away) = (read(flat), read(-flat));
        assert!(
            luma(toward) > 1.1 * luma(away),
            "sun {sun}: toward {toward}, away {away}"
        );
        let (up, down) = (read(Vec3::Y), read(-Vec3::Y));
        assert!(luma(up) > luma(down), "sun {sun}: up {up}, down {down}");
    }
}

#[test]
fn under_a_closed_canopy_the_floor_takes_a_few_percent_of_the_open_skys_light() {
    // D286: forests are measured to let a few percent to a tenth of the open sky's diffuse
    // light reach their floor; what faces up sees the crowns' shaded undersides, so it
    // takes less than what faces the crowns' sunlit flanks, and is not black.
    let cells = UVec3::new(2, 2, 2);
    let luma = |c: Vec3| c.dot(Vec3::new(0.2126, 0.7152, 0.0722));
    let open = crate::skylight::clear_sky(Vec3::new(0.3, 0.8, 0.5).normalize(), Quat::IDENTITY);
    let sides = world_sides(&open);
    let up_with = |share: f32, up: f32| {
        let seen = Seen {
            cells,
            sides: vec![[share, share, share, 1.0, share, share]; 8],
            up: vec![up; 8],
        };
        let data = texels(&seen, &sides);
        let read = |n: Vec3| as_bevy_reads(&data, cells, UVec3::ONE, n);
        (luma(read(Vec3::Y)), luma(read(Vec3::X)))
    };
    let (open_up, _) = up_with(1.0, 1.0);
    let (closed_up, closed_side) = up_with(0.0, 0.0);
    let ratio = closed_up / open_up;
    assert!(
        ratio > 0.01 && ratio < 0.1,
        "floor under a closed canopy {ratio} of the open's"
    );
    assert!(
        closed_side > closed_up,
        "side {closed_side}, up {closed_up}"
    );
}

#[test]
fn colours_pack_as_the_gpu_reads_them() {
    for c in [
        Vec3::new(0.5, 0.25, 0.125),
        Vec3::new(3.0, 0.01, 1.7),
        Vec3::new(0.0009, 0.0004, 0.0),
    ] {
        let back = unpack(rgb9e5(c));
        assert!(
            (back - c).abs().max_element() <= c.max_element() / 256.0,
            "{c} came back {back}"
        );
    }
    assert_eq!(rgb9e5(Vec3::ZERO), 0);
}
