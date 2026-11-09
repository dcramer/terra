//! A grown tree's size without its meshes: how tall a variant stands and how far round its
//! foot, the numbers its drawn variant has ([`super::grow_one`]) worked out from its shape
//! alone. A world with nothing drawn asks these: where a tree stands and what it stops are by
//! its kind's variants' sizes (`crate::flora`), and a realm instance holding the woods'
//! solids (D640) must come to the same trees as the game that draws them.

use super::{Grown, TreeDef, tree};
use terra_gen::{Rng, Seed};

/// Variant `index` of `count` of a tree's kind, grown at its height in `size`: its dice as
/// its shape left them (its crown's cards are drawn from them next), its height and its shape.
pub(super) fn shape(
    def: &TreeDef,
    seed: Seed,
    (index, count): (u32, u32),
    size: (f32, f32),
) -> (Rng, f32, tree::Tree) {
    let mut dice = seed.index(u64::from(index)).rng();
    let height = size.0 + (size.1 - size.0) * (index as f32 + 0.5) / count as f32;
    let shape = tree::grow(&mut dice, def, height);
    (dice, height, shape)
}

/// How far round its foot a tree of a shape is: what a walker is stopped by is from this.
pub(super) fn spread(shape: &tree::Tree) -> f32 {
    shape.trunk_m * 1.3
}

/// How tall variant `index` of `count` of a kind stands and how far round its foot, in
/// metres, as [`super::grow_one`] makes it; none for a kind that is not a tree.
pub fn measure(def: &Grown, seed: Seed, of: (u32, u32), size: (f32, f32)) -> Option<(f32, f32)> {
    let Grown::Tree(def) = def else {
        return None;
    };
    let (_, height, shape) = shape(def, seed, of, size);
    Some((height, spread(&shape)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flora_kinds::Flora;
    use crate::grown::{Cards, grow_one};
    use bevy::prelude::*;

    /// Every tree of the plants' data measures as its drawn variant does, variant by variant:
    /// a world that draws nothing comes to the trees the game draws.
    #[test]
    fn a_tree_measures_as_its_drawn_variant_does() {
        let cards = Cards::read();
        let meshes = Assets::<Mesh>::default();
        let sink = crate::mesh_sink::MeshSink::new(&meshes);
        let mut trees = 0;
        for kind in Flora::read().kinds() {
            let Some(how @ Grown::Tree(_)) = &kind.grown else {
                continue;
            };
            let seed = Seed(7).derive("grown").derive(&kind.name);
            let size = kind.size.unwrap_or((1.0, 1.0));
            for one in 0..kind.variants {
                let of = (one, kind.variants);
                let drawn = grow_one(
                    how,
                    (&kind.name, seed),
                    of,
                    size,
                    &cards,
                    &Handle::default(),
                    &sink,
                );
                assert_eq!(
                    measure(how, seed, of, size),
                    Some((drawn.height, drawn.spread)),
                    "{} {one}",
                    kind.name
                );
                trees += 1;
            }
        }
        assert!(trees >= 10, "{trees} variants of trees measured");
    }
}
