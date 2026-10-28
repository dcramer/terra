//! How two parts sit together by the convention (D566), the one placing rule: the assembler
//! places a recipe's parts by it, and the part checker (tools/items/part.ts) measures every join a
//! part's kinds allow from what it says, through `cargo run -p terra_items --bin assemble -- joins`.
//!
//! A code-made handle meets a head at its top: a disc across the line, `proud_m` above an eye's
//! upper opening or at a socket's bottom. A trim on the line sits under its working part's entry
//! by a shift along +Y; a seated trim (a boss, a rim) is turned so its seat's direction and roll
//! lie on its board's point of its type, then shifted onto it. Other pairs do not touch, or meet
//! only at declared sizes (a code handle in an opening, a trim over the butt).

use glam::{Mat3, Quat, Vec3};
use serde::Serialize;

use super::{Kind, Library, Part, PartType};

/// The identity turn, x y z w.
pub const ONE: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "meet", rename_all = "snake_case")]
pub enum Meet {
    /// A code-made handle's top in the head's frame: a disc across the line at `y`, `d` across.
    HandleTop { head: String, y: f32, d: f32 },
    /// `part` in `onto`'s frame: turned by `turn` (a quaternion, x y z w), then shifted by `at`.
    Placed {
        part: String,
        onto: String,
        at: [f32; 3],
        turn: [f32; 4],
    },
}

/// One pairing a part's kinds allow, and how it sits.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Pairing {
    pub kind: String,
    pub partner: String,
    pub meet: Meet,
}

/// The kinds a part is made for: those it names, or every kind with a slot for its type.
pub fn kinds_of<'a>(lib: &'a Library, part: &Part) -> Vec<&'a Kind> {
    lib.kinds
        .values()
        .filter(|k| match &part.kinds {
            Some(named) => named.contains(&k.kind),
            None => k.slots.iter().any(|s| s.fits.contains(&part.kind)),
        })
        .collect()
}

/// Every pairing of `id` with another part in a slot of a kind it is made for, once each, with
/// how the two sit; pairs that do not touch are left out.
pub fn pairings(lib: &Library, id: &str) -> Result<Vec<Pairing>, String> {
    let part = lib.parts.get(id).ok_or_else(|| {
        format!(
            "part {id} is not in the library; the parts are {}",
            super::names(lib.parts.keys())
        )
    })?;
    let mut seen = Vec::new();
    let mut out = Vec::new();
    for kind in kinds_of(lib, part) {
        let mine = kind.slots.iter().position(|s| s.fits.contains(&part.kind));
        for (i, slot) in kind.slots.iter().enumerate() {
            if Some(i) == mine {
                continue;
            }
            for other in lib
                .parts
                .values()
                .filter(|p| slot.fits.contains(&p.kind) && p.part != part.part)
            {
                if seen.contains(&other.part) {
                    continue;
                }
                seen.push(other.part.clone());
                if let Some(meet) = meet(lib, part, other) {
                    out.push(Pairing {
                        kind: kind.kind.clone(),
                        partner: other.part.clone(),
                        meet,
                    });
                }
            }
        }
    }
    Ok(out)
}

/// How `a` and `b` sit together, if they touch by the convention.
pub fn meet(lib: &Library, a: &Part, b: &Part) -> Option<Meet> {
    let (ta, tb) = (lib.types.types.get(&a.kind)?, lib.types.types.get(&b.kind)?);
    match (ta.made_by.as_str(), tb.made_by.as_str()) {
        ("code", "code") => None,
        ("code", _) => handle_top(lib, b, tb, ta),
        (_, "code") => handle_top(lib, a, ta, tb),
        _ if ta.slot == "trim" => on_working(a, ta, b, tb),
        _ => on_working(b, tb, a, ta),
    }
}

fn handle_top(lib: &Library, head: &Part, ht: &PartType, handle: &PartType) -> Option<Meet> {
    if handle.slot != "handle" {
        return None;
    }
    let top = head.point(ht.handle_top.as_deref()?)?;
    let proud = if top.kind == "eye" {
        lib.types.rules.proud_m.value
    } else {
        0.0
    };
    Some(Meet::HandleTop {
        head: head.part.clone(),
        y: top.at[1] + proud,
        d: top.size.as_ref()?.across(),
    })
}

fn on_working(trim: &Part, tt: &PartType, work: &Part, tw: &PartType) -> Option<Meet> {
    if tt.slot != "trim" || tw.slot != "working" {
        return None;
    }
    let placed = |at: [f32; 3], turn: [f32; 4]| Meet::Placed {
        part: trim.part.clone(),
        onto: work.part.clone(),
        at,
        turn,
    };
    match tt.place.as_deref()? {
        "under_entry" => {
            let entry = work.point(tw.entry.as_deref()?)?;
            Some(placed([0.0, entry.at[1] - trim.cover_m?[1], 0.0], ONE))
        }
        "seat" => {
            let (seat, onto) = (trim.point("seat")?, work.point(&trim.kind)?);
            let turn = rigid(seat.dir, seat.up?, onto.dir, onto.up?);
            let at = Vec3::from(onto.at) - turn * Vec3::from(seat.at);
            Some(placed(at.into(), turn.normalize().to_array()))
        }
        _ => None,
    }
}

/// The turn taking a full point's direction and roll onto another's: B Aᵀ of their frames
/// (dir, up, dir x up).
pub fn rigid(dir_a: [f32; 3], up_a: [f32; 3], dir_b: [f32; 3], up_b: [f32; 3]) -> Quat {
    let frame = |d: [f32; 3], u: [f32; 3]| {
        let (d, u) = (Vec3::from(d), Vec3::from(u));
        Mat3::from_cols(d, u, d.cross(u))
    };
    Quat::from_mat3(&(frame(dir_b, up_b) * frame(dir_a, up_a).transpose()))
}
