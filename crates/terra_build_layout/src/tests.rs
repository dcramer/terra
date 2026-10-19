//! The layout's checks of what the rules give it, moved up from the rules' own tests when the
//! layout became a crate over them (D407).

use terra_build::{Base, Sample, ground::cube_cm};

// ---- from crates/terra_build/src/ground.rs

#[test]
fn what_is_dressed_reads_the_same_ground_as_the_rules() {
    // A column with no sample takes its neighbours' ground (the rules' test), and the
    // layout's ticks of it are the same.
    let mut base = Base::default();
    base.sample_ground([
        Sample {
            column: [1, 0],
            height_cm: -100,
        },
        Sample {
            column: [-1, 0],
            height_cm: -50,
        },
        Sample {
            column: [1, -1],
            height_cm: 400,
        },
    ]);
    assert_eq!(base.ground().height_cm([0, 0]), -75);
    assert_eq!(
        crate::layout::roles::ground_ticks(&base, [0, 0]),
        -75 * crate::layout::TICKS / cube_cm()
    );
}

// ---- from crates/terra_build/src/ops/tests.rs (random operations)

#[test]
fn random_bases_laid_twice_are_laid_the_same() {
    // Layout is the same on every machine and every run: the bases random operations build
    // (the rules' property test's, every 300 steps), each laid twice by a log kit and a
    // masonry one, give the same pieces.
    use terra_build::kit::{Kit, Pattern};
    use terra_build::testing::random;
    let kits = [
        Kit::default(),
        Kit {
            id: random::STONE.0,
            pattern: Pattern::Masonry,
            ..Kit::default()
        },
    ];
    let mut laid = 0;
    for seed in 0..random::seeds() {
        for (i, base) in random::bases(seed, 1200, 300).iter().enumerate() {
            for kit in &kits {
                let pieces = crate::layout::lay(base, kit).pieces;
                assert_eq!(
                    pieces,
                    crate::layout::lay(base, kit).pieces,
                    "seed {seed} base {i}, {:?}",
                    kit.pattern
                );
                laid += usize::from(!pieces.is_empty());
            }
        }
    }
    assert!(laid > 0, "the random bases laid nothing");
}
