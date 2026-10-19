//! The repository's kits as the tests over building read them, in every building crate
//! (D407: a crate's tests see only the crates below it, so what they share is here, not in a
//! test module): not the game's, which loads them through its asset server.

pub mod random;

use crate::kit::{Kit, MATERIALS, Texture};
use crate::pieces::Material;
use crate::{Base, BuildOp, Panel};
use std::collections::BTreeMap;

/// The repository's `assets/` folder.
pub fn assets() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

/// A shipped kit by its file's name, without its modelled pieces.
pub fn shipped(name: &str) -> Kit {
    let read = |path: &str| {
        std::fs::read_to_string(assets().join(path)).map_err(|why| format!("{path}: {why}"))
    };
    let mut kit = Kit::read(&format!("{MATERIALS}/{name}"), &read).unwrap();
    kit.dress.made = None;
    kit
}

/// A kit of no materials, as the layout's and drawing's tests lay.
pub fn kit() -> Kit {
    Kit::default()
}

/// A cabin four metres by three outside.
pub fn cabin() -> Base {
    let mut base = Base::default();
    base.apply(BuildOp::AddRoom {
        min: [0, 0, 0],
        max: [7, 0, 5],
        kind: kit().kind(),
    });
    base
}

/// Walls `panel` across each tile, in `kit()`'s kind.
pub fn wall(base: &mut Base, tiles: impl IntoIterator<Item = [i32; 3]>, panel: Panel) {
    for tile in tiles {
        base.apply(BuildOp::SetWall {
            tile,
            panel: Some(panel),
            kind: kit().kind(),
        });
    }
}

/// A kit with a texture for floors, so it lays them.
pub fn textured() -> Kit {
    let texture = |path: &str| Texture {
        path: path.into(),
        metres: [1.0, 1.0],
        depth: 0.0,
        reflectance: crate::kit::matte(),
        strips: None,
        cells: 1,
        worn: None,
    };
    Kit {
        textures: BTreeMap::from([(Material::Floor, texture("floor"))]),
        ..kit()
    }
}

/// A prop put straight into a base, unchecked by the operations, for a test of how it hangs
/// whatever stands round it (the layout's `fitting` tests).
pub fn put_prop(base: &mut Base, prop: crate::Prop) {
    base.props.insert(prop.min, prop);
}
