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
