use super::*;

/// A fragment of stone so big at its longest, falling or lying still.
fn piece(size: f32, still: bool) -> Fragment {
    let fell = [(
        IVec3::ZERO,
        terra_build::Cube::new(terra_build::Kind(2), terra_build::Fill::Solid),
    )];
    let mut piece = fragments::burst(&fell, &|_| false, &|_| Breaks::Chunks, 1.0, 1)[0];
    piece.size = Vec3::splat(size);
    piece.still = still;
    piece
}

#[test]
fn past_the_budget_the_small_pieces_fade_before_a_falling_chunk_however_new_they_are() {
    let mut shards = Shards::default();
    let root = Entity::PLACEHOLDER;
    // The chunks of a keep coming down, then many chips of lesser falls after them.
    for _ in 0..20 {
        shards.live.push(Live::new(piece(2.0, false), root));
    }
    for _ in 0..200 {
        shards.live.push(Live::new(piece(0.15, false), root));
    }
    for live in &mut shards.live[..20] {
        live.age_s = 2.0;
    }
    keep_within(&mut shards, 120);
    let fading = |big: bool| {
        shards
            .live
            .iter()
            .filter(|live| (live.fragment.size.x > 1.0) == big && live.fading())
            .count()
    };
    assert_eq!(fading(true), 0, "a chunk faded for the chips");
    assert_eq!(fading(false), 100);
    // A chunk lying still for long goes before a chip just thrown.
    let mut shards = Shards::default();
    shards.live.push(Live::new(piece(2.0, true), root));
    shards.live[0].age_s = 30.0;
    shards.live.push(Live::new(piece(0.15, false), root));
    keep_within(&mut shards, 1);
    assert!(shards.live[0].fading() && !shards.live[1].fading());
}

#[test]
fn a_chunk_landing_raises_a_billow_as_big_as_it_rolling_out_low_and_a_chip_none() {
    let up = Vec3::Y;
    assert!(landing(0.2, Vec3::X, up).is_none());
    let small = landing(1.0, Vec3::X * 3.0, up).expect("a chunk raises a billow");
    let big = landing(3.0, Vec3::X * 3.0, up).unwrap();
    for (size, billow) in [(1.0, &small), (3.0, &big)] {
        assert!(billow.wide > size * 1.5, "{} for {size} m", billow.wide);
        assert!(billow.life >= 4.0, "{}", billow.life);
        // Out along the ground faster than it rises, the way the chunk was going.
        assert!(
            billow.drift.y > 0.0 && billow.drift.x > 2.0 * billow.drift.y,
            "{}",
            billow.drift
        );
    }
    assert!(big.wide > small.wide && big.life >= small.life);
}

#[test]
fn chunks_landing_together_join_one_billow_and_one_far_off_or_later_raises_its_own() {
    let mut raised = dust::Raised::default();
    let here = Vec3::new(10.0, 0.0, 10.0);
    assert!(!raised.joins(here, 0.0), "nothing raised yet");
    raised.add(here, 8.0, 0.0);
    // A chunk landing two metres off a moment later is in the same cloud.
    assert!(raised.joins(here + Vec3::X * 2.0, 0.3));
    // One ten metres off, past half the billow's width, is a cloud of its own.
    assert!(!raised.joins(here + Vec3::X * 10.0, 0.3));
    // And seconds later, the first has drifted and thinned: a new one comes up.
    assert!(!raised.joins(here + Vec3::X * 2.0, 3.0));
}

#[test]
fn a_big_fall_raises_a_surge_rolling_out_every_way_and_hanging_for_many_seconds() {
    assert!(
        dust::surge(40, 2.0, 0.5).is_empty(),
        "a small fall raised a surge"
    );
    // How far out a surge's dust reaches as it settles, and how long it hangs.
    let reach = |billows: &[(f32, Vec3, dust::Billow)]| {
        billows
            .iter()
            .map(|(_, from, b)| {
                (*from + b.drift * b.life * 0.5).with_y(0.0).length() + b.wide * 0.5
            })
            .fold(0.0f32, f32::max)
    };
    let lasts = |billows: &[(f32, Vec3, dust::Billow)]| {
        billows
            .iter()
            .map(|(after, _, b)| after + b.life)
            .fold(0.0f32, f32::max)
    };
    // A keep: 9 m across, 15 m tall.
    let keep = dust::surge(4888, 9.0, 1.7);
    let wall = dust::surge(400, 6.0, 1.2);
    assert!(keep.len() > wall.len(), "{} {}", keep.len(), wall.len());
    assert!(
        reach(&keep) > 3.0 * 9.0 * 0.5 && reach(&keep) > reach(&wall),
        "a keep's surge reaches {} m, a wall's {} m",
        reach(&keep),
        reach(&wall)
    );
    let hangs = lasts(&keep);
    assert!((10.0..=30.0).contains(&hangs), "it hangs {hangs} s");
    // Out every way round, as a ring, not one way.
    let mut quarters = BTreeSet::new();
    for (_, _, b) in &keep {
        if b.drift.with_y(0.0).length() > 1.0 {
            quarters.insert((b.drift.x > 0.0, b.drift.z > 0.0));
        }
    }
    assert_eq!(quarters.len(), 4, "it rolled out only {quarters:?}");
    // Its rings come up outside its foot, where the walls left standing round it do not
    // hide them: inside, a keep's surge rose within its own ground storey unseen.
    for (_, from, b) in &keep {
        if b.drift.with_y(0.0).length() > 1.0 {
            let out = from.with_y(0.0).length();
            assert!(
                out > 9.0 * 0.5,
                "a ring billow came up {out} m from the middle"
            );
        }
    }
    // And it rises over the heap.
    assert!(
        keep.iter()
            .any(|(_, from, b)| from.y + b.tall > 3.0 && b.drift.y > 0.4)
    );
}

#[test]
fn a_big_falls_dust_rises_over_its_heap_in_thin_plumes_taller_than_wide() {
    // One column as wide as the heap read as a cotton ball (castle film round 6): a
    // demolition's dust goes up in plumes a few metres across, several over the heap.
    let keep = dust::surge(4888, 9.0, 1.7);
    let plumes: Vec<_> = keep.iter().filter(|(_, _, b)| b.drift.y > 1.0).collect();
    assert!(plumes.len() >= 4, "{} plumes", plumes.len());
    for (_, from, b) in &plumes {
        assert!(
            b.tall > 1.3 * b.wide && b.wide < 7.0,
            "a plume {} m wide stacked {} m",
            b.wide,
            b.tall
        );
        assert!(
            from.with_y(0.0).length() < 9.0 * 0.5,
            "a plume off the heap"
        );
    }
    // Spread over the heap, not one stack.
    let apart = plumes
        .iter()
        .flat_map(|a| plumes.iter().map(move |b| (a.1 - b.1).with_y(0.0).length()))
        .fold(0.0f32, f32::max);
    assert!(apart > 2.0, "the plumes stand {apart} m apart at most");
}

#[test]
fn a_billow_is_most_of_its_width_within_two_seconds_however_long_it_hangs() {
    // Grown by the share of its life alone, a surge billow hanging 18 s was a fifth of its
    // width after 2 s, and the keep's surge never showed over its ground storey (round 6).
    for life in [1.6, 6.0, 18.0, 25.0] {
        let early = dust::grown(1.5, life);
        assert!(
            early >= 0.5,
            "{early} of its width 1.5 s into a {life} s billow"
        );
        assert!(dust::grown(0.0, life) < 0.05);
        assert!((dust::grown(life, life) - 1.0).abs() < 1e-4);
        let mut last = 0.0;
        for step in 0..=100 {
            let now = dust::grown(life * step as f32 / 100.0, life);
            assert!(now >= last, "it shrank at {step}%");
            last = now;
        }
    }
}

#[test]
fn a_fall_raises_more_cloud_the_more_of_it_lands() {
    let mut raised = dust::Raised::default();
    let here = Vec3::new(10.0, 0.0, 10.0);
    let mut clouds = 0;
    for chunk in 0..40 {
        let at = here + Vec3::X * (chunk % 3) as f32;
        if !raised.joins(at, 0.01 * chunk as f32) {
            raised.add(at, 8.0, 0.01 * chunk as f32);
            clouds += 1;
        }
    }
    // Many chunks landing together: fewer clouds than chunks, more than one.
    assert!(
        clouds > 40 / (dust::FEEDS as usize + 2) && clouds < 20,
        "{clouds}"
    );
}

#[test]
fn a_big_piece_lies_for_seconds_before_it_fades_and_a_small_one_briefly() {
    let fell = [(
        IVec3::new(0, 8, 0),
        terra_build::Cube::new(terra_build::Kind(2), terra_build::Fill::Solid),
    )];
    let breaks = |_| Breaks::Chunks;
    let made = fragments::burst(&fell, &|_| true, &breaks, 1.0, 100);
    let (big, small): (Vec<&Fragment>, Vec<&Fragment>) = made
        .iter()
        .partition(|piece| piece.size.max_element() >= BIG_M);
    assert!(!big.is_empty() && !small.is_empty());
    for piece in big {
        let rest = rest_for(piece);
        assert!((8.0..=10.0).contains(&rest), "{rest}");
    }
    for piece in small {
        assert!(rest_for(piece) < 4.0);
    }
}

#[test]
fn a_surges_dust_is_buff_tan_and_thick_at_its_core_not_a_grey_veil() {
    // A grey stone's surge is warmed to the tan of the earth and mortar it drives out.
    let tan = dust::surge_tint(Color::srgb(0.55, 0.55, 0.57)).to_srgba();
    assert!(
        tan.red > tan.green && tan.green > tan.blue && tan.red - tan.blue > 0.12,
        "{tan:?}"
    );
    // Each sprite is near opaque over its core and barely darker there than at its edge,
    // so overlapping billows hide what is behind them in the dust's own colour.
    let (image, _) = dust::dust_images();
    let side = image.width() as usize;
    let data = image.data.as_ref().expect("made on the CPU");
    let (mut alpha, mut shade, mut count) = (0.0, 0.0, 0.0);
    for y in 0..side {
        for x in 0..side {
            let (u, v) = (
                (x as f32 + 0.5) / side as f32 * 2.0 - 1.0,
                (y as f32 + 0.5) / side as f32 * 2.0 - 1.0,
            );
            if u * u + v * v > 0.3 * 0.3 {
                continue;
            }
            let texel = &data[(y * side + x) * 4..][..4];
            shade += f32::from(texel[0]) / 255.0;
            alpha += f32::from(texel[3]) / 255.0;
            count += 1.0;
        }
    }
    let (alpha, shade) = (alpha / count, shade / count);
    assert!(alpha > 0.8, "its core {alpha} opaque");
    assert!(shade > 0.8, "its core {shade} as light as its colour");
}

/// A dust sprite's cutout holds every texel of it that shows, and draws much less of the
/// square than the square did: the pixels a lit sprite pays for whether they show or not.
#[test]
fn a_dust_sprites_cutout_holds_all_it_shows_in_little_more_than_half_the_square() {
    use bevy::mesh::{Indices, VertexAttributeValues};
    let (image, _) = dust::dust_images();
    let mesh = dust::cutout(&image);
    let Some(VertexAttributeValues::Float32x2(uvs)) = mesh.attribute(Mesh::ATTRIBUTE_UV_0) else {
        panic!("uvs");
    };
    let Some(Indices::U32(indices)) = mesh.indices() else {
        panic!("indices");
    };
    let triangles: Vec<[Vec2; 3]> = indices
        .chunks(3)
        .map(|t| [t[0], t[1], t[2]].map(|i| Vec2::from(uvs[i as usize])))
        .collect();
    // Twice the signed area, positive on the inside of each of the fan's triangles.
    let cross = |a: Vec2, b: Vec2, p: Vec2| (b - a).perp_dot(p - a);
    let inside = |p: Vec2| {
        triangles.iter().any(|&[a, b, c]| {
            let (d0, d1, d2) = (cross(a, b, p), cross(b, c, p), cross(c, a, p));
            (d0 >= -1e-6 && d1 >= -1e-6 && d2 >= -1e-6) || (d0 <= 1e-6 && d1 <= 1e-6 && d2 <= 1e-6)
        })
    };
    let side = image.width() as usize;
    let data = image.data.as_ref().expect("made on the CPU");
    let (mut shown, mut covered, mut missed) = (0, 0, 0);
    for y in 0..side {
        for x in 0..side {
            let p = Vec2::new(
                (x as f32 + 0.5) / side as f32,
                (y as f32 + 0.5) / side as f32,
            );
            let holds = inside(p);
            covered += usize::from(holds);
            if data[(y * side + x) * 4 + 3] > 0 {
                shown += 1;
                missed += usize::from(!holds);
            }
        }
    }
    assert!(shown > side * side / 4, "the sprite shows {shown} texels");
    assert_eq!(missed, 0, "texels that show outside the cutout");
    let share = covered as f32 / (side * side) as f32;
    assert!(share < 0.6, "the cutout covers {share} of the square");
}

#[test]
fn what_fire_burns_through_raises_no_dust_where_it_stood_and_what_breaks_does() {
    // A burning keep's timber, burnt through cube by cube, threw a puff of dust from each,
    // dozens of dark discs over the fire (castle film round 13): it goes up in the fire's
    // smoke; what harm breaks still puffs where it stood.
    let cube = Cube::new(Kind(2), terra_build::Fill::Solid);
    let fell: Vec<(IVec3, Cube)> = (0..6).map(|x| (IVec3::new(x, 9, 0), cube)).collect();
    let destroyed = fell.iter().take(4).map(|(at, _)| at.to_array()).collect();
    let mut fallen = Fallen {
        fell,
        destroyed,
        ..Fallen::default()
    };
    assert_eq!(
        broken(&fallen).len(),
        4,
        "a puff where each destroyed cube stood"
    );
    fallen.burnt = true;
    assert!(broken(&fallen).is_empty(), "burnt through, no dust");
}

#[test]
fn a_billow_comes_up_as_one_cloud_never_as_scattered_discs() {
    // Laid out at once, a surge's sprites came up a third of a metre wide and metres apart,
    // a column's strung ten metres up, and read as rows of dark round discs over the keep's
    // breach until they grew (castle film r16, 24-26.4 s). Each sprite by the dense middle
    // of it, a quarter of its width round its centre: through its first half second a
    // billow's sprites touch into one cloud, and none stands alone to 0.75 s.
    let up = Vec3::Y;
    let mut billows: Vec<dust::Billow> = dust::surge(4888, 9.0, 1.7)
        .into_iter()
        .map(|(_, _, b)| b)
        .collect();
    billows.extend(dust::surge(400, 6.0, 1.2).into_iter().map(|(_, _, b)| b));
    billows.extend(dust::landing(3.0, Vec3::X, up));
    assert!(billows.iter().any(|b| b.tall > 0.0), "no plume to try");
    for (n, billow) in billows.iter().enumerate() {
        let sprites = dust::layout(billow, up, 0x51ed ^ n as u32);
        for age in [0.03, 0.1, 0.25, 0.5, 0.75] {
            let grow = dust::grown(age, billow.life);
            let at: Vec<(Vec3, f32)> = sprites
                .iter()
                .map(|s| {
                    (
                        s.lay * grow + s.drift * age,
                        0.25 * dust::sprite_wide(s.wide, grow),
                    )
                })
                .collect();
            let touch = |i: usize, j: usize| at[i].0.distance(at[j].0) <= at[i].1 + at[j].1;
            // Joined, as a flood from the first sprite over those touching.
            let mut joined = vec![false; at.len()];
            let mut open = vec![0];
            joined[0] = true;
            while let Some(i) = open.pop() {
                for (j, joins) in joined.iter_mut().enumerate() {
                    if !*joins && touch(i, j) {
                        *joins = true;
                        open.push(j);
                    }
                }
            }
            let apart = joined.iter().filter(|j| !**j).count();
            let alone = (0..at.len())
                .filter(|&i| (0..at.len()).all(|j| j == i || !touch(i, j)))
                .count();
            let what = format!(
                "billow {n} ({} m wide, {} m tall) at {age} s, {} sprites",
                billow.wide,
                billow.tall,
                at.len()
            );
            assert!(age > 0.5 || apart == 0, "{what}: {apart} apart");
            assert_eq!(alone, 0, "{what}: {alone} alone");
        }
    }
}
