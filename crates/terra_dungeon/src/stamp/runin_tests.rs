//! The run-in's output held while its code is made faster: a hash a theme of what it gives
//! (the doors, chests, mobs and torches, the tour, the camera's view, look and pace, and the
//! props laid after it) over `SEEDS` seeds, against the hashes the code gave before. A
//! refactor's guard, not a design's: where data or a design changes the run-in's output on
//! purpose, take the new hashes from this test's printout on the commit before it.

use super::tests::kind;
use super::*;
use crate::theme::assets;
use crate::{ModuleSet, generate};
use std::hash::{DefaultHasher, Hash, Hasher};
use terra_gen::Seed;

const SEEDS: u64 = 24;

/// Each theme's hash of the run-in's output over `SEEDS` seeds, from the code before it was
/// made faster.
const HASHES: &[(&str, u64)] = &[
    ("crypt", 3961196597882366179),
    ("flooded_mine", 300874927427389460),
    ("ruined_keep", 15727364600443360124),
];

fn of(stamp: &Stamp) -> u64 {
    let mut h = DefaultHasher::new();
    // Debug prints an f32 as its shortest exact form: equal text, equal bits.
    format!(
        "{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}",
        stamp.start,
        stamp.start_facing,
        stamp.route,
        stamp.doorways,
        stamp.view,
        stamp.look,
        stamp.pace,
        stamp.lights,
        stamp.mobs,
        stamp.chests,
        stamp.objects,
        stamp.props,
    )
    .hash(&mut h);
    h.finish()
}

#[test]
fn the_run_in_gives_what_it_gave() {
    let mut got = Vec::new();
    for theme in Theme::all(&assets()).unwrap() {
        let set = ModuleSet::new(&theme).unwrap();
        let seeds: Vec<u64> = (0..SEEDS).collect();
        let hashes = terra_gen::par::map(&seeds, |&s| {
            let dungeon = generate(&theme, &set, Seed(s)).ok()?;
            Some(of(&stamp(&dungeon, &theme, &kind)))
        });
        let mut h = DefaultHasher::new();
        hashes.hash(&mut h);
        println!("(\"{}\", {}),", theme.name, h.finish());
        got.push((theme.name.clone(), h.finish()));
    }
    let want: Vec<(String, u64)> = HASHES.iter().map(|&(n, h)| (n.to_string(), h)).collect();
    assert_eq!(got, want, "the run-in's output a theme");
}
