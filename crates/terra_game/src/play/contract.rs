//! The model contract (docs/art-style.md, D303): what every model the game places must be,
//! so the game places, fits, swings and holds it from its entry's size, its anchor and its
//! sockets alone, with no offset, turn or scale fixed up for one model in code or data.
//!
//! - **Real size.** Its catalogue entry states how wide, high and deep it is, in metres,
//!   along its X, Y and Z; its file is that size, within [`tolerance`].
//! - **Axes.** glTF's: +Y up, its front facing +Z.
//! - **Anchor.** Its origin is where its kind says ([`Anchor`]).
//! - **Sockets.** The points code attaches to are empties in its file, `socket.<name>`,
//!   and each kind has the ones code needs ([`Entry::sockets`]).
//! - **Fit.** A door's or shutters' leaf is made for the leaf the first hole it fits takes, within
//!   an eighth of it, and is stretched to that leaf (`terra_build::fitting`): a door's inside
//!   the frame it comes with, shutters over the hole and a little past its edges; it stays
//!   within the wall's faces (doors) or on its outer half (shutters).
//!
//! [`check`] checks one model; `cargo xtask contract` checks them all and prints each, and
//! the test here holds `cargo xtask check` to it.

use super::catalogue::{Builds, Rest};
use super::props::{Shape, read_shape, socket};
use bevy::prelude::*;
use terra_build::fitting::Hang;
use terra_build::{CELL_M, Prop, PropKind};

/// Where a model's origin is, by its kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anchor {
    /// Something stood on a floor or the ground: the middle of its base.
    Base,
    /// A door's or a shutter's leaf: the foot of its hinge, on the edge it turns on, in
    /// the middle of its thickness, the leaf running along +X from it.
    Hinge,
}

/// A model the game places, as its catalogue entry gives it.
#[derive(Clone, Debug)]
pub struct Entry {
    pub name: String,
    /// Its file, under `assets/models`.
    pub file: String,
    /// How wide, high and deep it is, in metres.
    pub dims: Vec3,
    pub anchor: Anchor,
    /// The sockets code needs it to have.
    pub sockets: Vec<&'static str>,
    /// For a door or shutters, how it hangs and the first hole it fits, wide by high in
    /// cubes, which it is made for.
    pub hole: Option<(Hang, (i32, i32))>,
}

/// The bag a player drops when they go down, which is not built so is not in the
/// catalogue: its file and size (`death`).
pub const BAG: (&str, Vec3) = ("props/Sack.glb", Vec3::new(0.39, 0.35, 0.7));

/// Every model the game places: the catalogue's items, doors and shutters, and the bag.
pub fn entries(builds: &Builds) -> Vec<Entry> {
    let mut entries: Vec<Entry> = builds
        .items()
        .map(|item| Entry {
            name: item.name.clone(),
            file: super::props::item_file(item),
            dims: item.dims,
            anchor: Anchor::Base,
            sockets: if item.rest == Some(Rest::Sit) {
                vec!["seat"]
            } else {
                Vec::new()
            },
            hole: None,
        })
        .collect();
    entries.extend(builds.fittings().iter().map(|fitting| Entry {
        name: fitting.name.clone(),
        file: format!("openings/{}.glb", fitting.model),
        dims: fitting.dims,
        anchor: Anchor::Hinge,
        sockets: vec!["hinge", "handle"],
        hole: fitting.fits.first().map(|&hole| (fitting.hang, hole)),
    }));
    entries.push(Entry {
        name: "Dropped bag".to_string(),
        file: BAG.0.to_string(),
        dims: BAG.1,
        anchor: Anchor::Base,
        sockets: Vec::new(),
        hole: None,
    });
    entries
}

/// How far a door's or shutters' leaf may be made from the opening it fills, as a share of
/// it: it is stretched to fill it.
const STRETCH: f32 = 0.125;

/// How far a model's size, anchor or socket may be from what its entry says, in metres,
/// along a side `dim` long: two percent, and never under a centimetre.
pub fn tolerance(dim: f32) -> f32 {
    (dim * 0.02).max(0.01)
}

/// What is wrong with a model against its entry; nothing if it keeps the contract.
pub fn check(entry: &Entry) -> Vec<String> {
    match read_shape(&entry.file) {
        Ok(shape) => check_shape(entry, &shape),
        Err(problem) => vec![problem],
    }
}

fn check_shape(entry: &Entry, shape: &Shape) -> Vec<String> {
    let mut wrong = Vec::new();
    let tol = Vec3::new(
        tolerance(entry.dims.x),
        tolerance(entry.dims.y),
        tolerance(entry.dims.z),
    );
    let size = shape.high - shape.low;
    if (size - entry.dims).abs().cmpgt(tol).any() {
        wrong.push(format!(
            "is {} wide, high and deep, not {}",
            metres(size),
            metres(entry.dims)
        ));
    }
    // Where its origin should be, as a point of its box from its low corner: 0 its low
    // side, 0.5 its middle.
    let (wanted, said) = match entry.anchor {
        Anchor::Base => (Vec3::new(0.5, 0.0, 0.5), "the middle of its base"),
        Anchor::Hinge => (
            Vec3::new(0.0, 0.0, 0.5),
            "its hinge edge's foot, mid-thickness",
        ),
    };
    let anchor = shape.low + size * wanted;
    if anchor.abs().cmpgt(tol).any() {
        wrong.push(format!("has its origin {} from {said}", metres(-anchor)));
    }
    for name in &entry.sockets {
        match socket(&shape.sockets, name) {
            None => wrong.push(format!("has no socket.{name}")),
            Some(at) if *name == "hinge" && Vec2::new(at.x, at.z).length() > tol.x => {
                wrong.push(format!("has its hinge {} off its origin", metres(at)))
            }
            Some(_) => {}
        }
    }
    for (name, at) in &shape.sockets {
        if at.cmplt(shape.low - tol).any() || at.cmpgt(shape.high + tol).any() {
            wrong.push(format!("has socket.{name} at {} outside it", metres(*at)));
        }
    }
    if let Some((hang, hole)) = entry.hole {
        wrong.extend(check_fit(entry, hang, hole));
    }
    wrong
}

/// A door or shutters hung in the hole it is made for, along a wall running east a whole cube
/// thick, the thickest any kit lays: each leaf as wide and high as the model, and its
/// thickness within the wall's faces, or for shutters out from the face of its frame.
fn check_fit(entry: &Entry, hang: Hang, (wide, high): (i32, i32)) -> Vec<String> {
    let opens = IVec3::Z;
    let prop = Prop {
        kind: PropKind(0),
        min: [0, 0, 0],
        max: [wide - 1, high - 1, 0],
        open: false,
        opens: opens.to_array(),
        hang,
    };
    let middle = CELL_M * 0.5;
    let mut wrong = Vec::new();
    for leaf in prop.leaves(middle) {
        let made = Vec2::new(entry.dims.x, entry.dims.y);
        // Made for the opening it fills, near enough that stretching it to fill the
        // opening keeps its look.
        let needed = Vec2::new(leaf.width, leaf.height);
        if ((needed - made).abs() / needed)
            .cmpgt(Vec2::splat(STRETCH))
            .any()
        {
            wrong.push(format!(
                "is made {:.3} by {:.3} m, but its hole of {wide} by {high} cubes opens {:.3} by {:.3} m for a leaf",
                made.x, made.y, needed.x, needed.y
            ));
        }
        // Its origin goes in the middle of the leaf's thickness on its hinge, and the
        // model is as thick as it is made.
        let pose = leaf.pose(0.0);
        let centre = (pose.hinge + pose.through * (leaf.thick * 0.5)).z - middle;
        let (front, back) = (centre + entry.dims.z * 0.5, centre - entry.dims.z * 0.5);
        let (low, top) = match hang {
            Hang::Door => (-middle, middle),
            Hang::Shutters => (
                middle + terra_build::fitting::FRAME_PROUD_M,
                middle + terra_build::fitting::FRAME_PROUD_M + entry.dims.z,
            ),
        };
        if back < low - 1e-3 || front > top + 1e-3 {
            wrong.push(format!(
                "stands {back:.3} to {front:.3} m out from its wall's middle, past {low:.3} to {top:.3}",
            ));
        }
    }
    wrong
}

fn metres(v: Vec3) -> String {
    format!("{:.3} x {:.3} x {:.3} m", v.x, v.y, v.z)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::play::crafting::Stations;
    use crate::play::inventory::Items;

    /// The checker, as `cargo xtask contract` runs it: every model the game places keeps the
    /// contract.
    #[test]
    fn every_model_keeps_the_contract() {
        let builds = Builds::read(&Items::default(), &Stations::read());
        let mut wrong = Vec::new();
        for entry in entries(&builds) {
            let problems = check(&entry);
            println!(
                "{:<24} {:<34} {}",
                entry.name,
                entry.file,
                if problems.is_empty() {
                    "keeps the contract".to_string()
                } else {
                    problems.join("; ")
                }
            );
            wrong.extend(
                problems
                    .into_iter()
                    .map(|problem| format!("{} ({}) {problem}", entry.name, entry.file)),
            );
        }
        assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
    }

    fn entry(anchor: Anchor, dims: Vec3) -> Entry {
        Entry {
            name: "Test".into(),
            file: String::new(),
            dims,
            anchor,
            sockets: Vec::new(),
            hole: None,
        }
    }

    fn shape(low: Vec3, high: Vec3, sockets: Vec<(String, Vec3)>) -> Shape {
        Shape::from_box(low, high, sockets)
    }

    #[test]
    fn a_floor_prop_stands_on_the_middle_of_its_base_at_its_size() {
        let chest = entry(Anchor::Base, Vec3::new(1.0, 0.6, 0.6));
        let right = shape(Vec3::new(-0.5, 0.0, -0.3), Vec3::new(0.5, 0.6, 0.3), vec![]);
        assert!(check_shape(&chest, &right).is_empty());
        // Tripo's origin in its middle, and a model left at another size.
        let centred = shape(
            Vec3::new(-0.5, -0.3, -0.3),
            Vec3::new(0.5, 0.3, 0.3),
            vec![],
        );
        assert_eq!(check_shape(&chest, &centred).len(), 1);
        let small = shape(Vec3::new(-0.4, 0.0, -0.3), Vec3::new(0.4, 0.6, 0.3), vec![]);
        assert_eq!(check_shape(&chest, &small).len(), 1);
    }

    #[test]
    fn a_leaf_hangs_from_its_hinge_with_its_sockets() {
        let mut door = entry(Anchor::Hinge, Vec3::new(0.9, 1.9, 0.08));
        door.sockets = vec!["hinge", "handle"];
        let low = Vec3::new(0.0, 0.0, -0.04);
        let high = Vec3::new(0.9, 1.9, 0.04);
        let sockets = vec![
            ("hinge".to_string(), Vec3::ZERO),
            ("handle".to_string(), Vec3::new(0.8, 1.0, 0.04)),
        ];
        assert!(check_shape(&door, &shape(low, high, sockets.clone())).is_empty());
        // Hinged on its other edge, as a model made with its hinges on the right is.
        let other = shape(low - Vec3::X * 0.9, high - Vec3::X * 0.9, sockets);
        assert!(!check_shape(&door, &other).is_empty());
        // Without its handle.
        let bare = shape(low, high, vec![("hinge".to_string(), Vec3::ZERO)]);
        assert_eq!(check_shape(&door, &bare), vec!["has no socket.handle"]);
    }

    #[test]
    fn a_door_made_for_its_hole_fits_it_and_one_too_thick_does_not() {
        let hole = (2, 4);
        let leaf = Prop {
            kind: PropKind(0),
            min: [0, 0, 0],
            max: [1, 3, 0],
            open: false,
            opens: [0, 0, 1],
            hang: Hang::Door,
        }
        .leaves(CELL_M * 0.5)[0];
        let made = |thick: f32| entry(Anchor::Hinge, Vec3::new(leaf.width, leaf.height, thick));
        assert!(check_fit(&made(0.08), Hang::Door, hole).is_empty());
        assert_eq!(check_fit(&made(0.6), Hang::Door, hole).len(), 1);
        let wide = entry(Anchor::Hinge, Vec3::new(1.2, leaf.height, 0.08));
        assert_eq!(check_fit(&wide, Hang::Door, hole).len(), 1);
    }
}
