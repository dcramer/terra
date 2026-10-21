//! What stands beside a break, as it is drawn.

use super::*;

#[test]
fn a_wall_beside_a_break_stands_cracked_with_its_stones_in_place() {
    // A stone wall a cube thick, one cube of it broken out.
    let kits = shipped();
    let stone = kind(&kits, Pattern::Masonry, Profile::Round);
    let mut base = Base::default();
    base.apply(BuildOp::Fill {
        min: [0, 0, 0],
        max: [7, 3, 0],
        cube: Some(Cube::new(stone, Fill::Solid)),
    });
    let rules = kits.rules();
    base.act(
        &terra_build::ops::Op::Damage {
            cubes: vec![[3, 1, 0]],
            amount: u16::MAX,
        },
        &terra_build::ops::Context::free(&rules),
    )
    .unwrap();
    assert!(base.broken(IVec3::new(3, 1, 0)), "no break");
    let worn = crate::sections::laying::worn_within(&base, &kits, IVec3::ZERO, IVec3::new(7, 3, 0));
    // Its neighbours broke with it: cracked, a quarter worn, but short of the half at which
    // a stone wall's stones fall out of their beds and show its dark core, which drew every
    // cube beside the castle's breaks as a dark window in a pale frame, a grid at film scale
    // (castle film round 5).
    let beside = worn.get(&[2, 1, 0]).copied().unwrap_or(0);
    assert!(
        (1..2).contains(&beside),
        "worn {beside} quarters beside the break"
    );
    // And a cube away from it stands whole.
    assert_eq!(worn.get(&[6, 3, 0]), None);
}
