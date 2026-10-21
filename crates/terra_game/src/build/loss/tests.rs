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
            .any(|(_, from, b)| from.y > 3.0 && b.drift.y > 0.4)
    );
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
