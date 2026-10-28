//! The assembler (D565, D566): a recipe (a kind, its part ids, a material and a seed) in; each
//! part's place, the code-made handle's section, and the item's grip, second grip, working edge,
//! mass, balance and inertia out, refused when outside its kind's envelope. Pure functions of
//! the recipe and the data, so the game and the server get the same bytes (the pipeline note,
//! research/2026-10-28-assembled-items-pipeline.md, verdict 2).
//!
//! Parts are placed, never merged: one placement a part, naming the part whose one mesh every
//! item using it shares (verdict 1b). Parts on the line are placed by a shift along +Y alone and
//! never scaled (D566). Built so far for the line kinds (a haft through a head's eye or into its
//! socket, with trims under the head and over the butt); blades, boards and staves are refused
//! with a message until their placing is built.

mod data;
#[cfg(test)]
mod tests;

use std::f32::consts::PI;

pub use data::{Band, Kind, Library, Part, PartType, Point, Size};
use serde::Serialize;

/// What an item is made of: everything needed to make it again, byte for byte.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Recipe {
    pub kind: String,
    pub parts: Vec<String>,
    pub material: String,
    pub seed: u64,
}

/// One part's place in the item's frame: the grip at the origin, +Y up the handle (D546).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Placed {
    pub part: String,
    pub slot: String,
    pub at: [f32; 3],
    /// A quaternion, x y z w: identity for every part on the line.
    pub turn: [f32; 4],
}

/// The item's working edge: a span on its working part, and the side it faces.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Edge {
    pub from: [f32; 3],
    pub to: [f32; 3],
    pub side: [f32; 3],
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Assembly {
    pub recipe: Recipe,
    pub placed: Vec<Placed>,
    /// The code-made handle's diameter along the line, `[y, diameter]`, linear between, in the
    /// item's frame: each join's from its partner's point (D566), the grip's from the handle.
    pub section: Vec<[f32; 2]>,
    /// The handle's diameter at the grip, for the fingers (D562).
    pub grip_diameter_m: f32,
    pub second: Option<[f32; 3]>,
    pub edge: Option<Edge>,
    pub length_m: f32,
    pub mass_kg: f32,
    /// The centre of mass's distance from the grip along +Y.
    pub balance_m: f32,
    /// D459's I = m (b^2 + L^2/12).
    pub inertia_kgm2: f32,
}

impl Assembly {
    /// The canonical bytes: the same recipe and data give the same bytes, on game and server.
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }
}

const ONE: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

/// Puts a recipe's parts together by its kind's rules, or says why it cannot.
pub fn assemble(lib: &Library, recipe: &Recipe) -> Result<Assembly, String> {
    let kind = lib.kinds.get(&recipe.kind).ok_or_else(|| {
        format!(
            "kind {} is not in assets/data/kinds; the kinds are {}",
            recipe.kind,
            names(lib.kinds.keys())
        )
    })?;
    let filled = fill(lib, kind, recipe)?;
    let find = |slot: &str| {
        filled
            .iter()
            .find(|(s, _, _)| s.slot == slot)
            .map(|(_, p, t)| (*p, *t))
    };
    let (Some((handle, _)), Some((work, work_type))) = (find("handle"), find("working")) else {
        return Err(format!(
            "{}: a recipe needs its working part and its handle",
            recipe.kind
        ));
    };
    let (Some(entry), Some(top)) = (&work_type.entry, &work_type.handle_top) else {
        return Err(format!(
            "{}: {} ({}) is not placed on a haft's line; only eye and socket heads are built (D565 step 2), its kind's placing comes next",
            recipe.kind, work.part, work.kind
        ));
    };
    let len = need(handle, handle.length_m.as_ref(), "length_m")?;
    let grip = need(handle, handle.grip_from_end_m.as_ref(), "grip_from_end_m")?;
    let grip_d = need(handle, handle.grip_diameter_m.as_ref(), "grip_diameter_m")?;
    let density = need(handle, handle.density_kg_m3.as_ref(), "density_kg_m3")?;
    let entry = on_line(work, entry, lib.types.rules.join_m.value)?;
    let top = on_line(work, top, lib.types.rules.join_m.value)?;
    // The head sits so the haft's top stands proud of an eye's upper opening, or ends at a socket's bottom (D566).
    let proud = if top.kind == "eye" {
        lib.types.rules.proud_m.value
    } else {
        0.0
    };
    let head_y = len - proud - top.at[1];

    // Butt-up coordinates first (the haft from 0 to len), shifted to the grip at the end.
    let mut section = vec![
        [head_y + entry.at[1], size(work, entry)?],
        [head_y + top.at[1], size(work, top)?],
        [len, size(work, top)?],
        [grip, grip_d],
    ];
    let mut placed = vec![
        (handle.part.clone(), "handle", 0.0),
        (work.part.clone(), "working", head_y),
    ];
    let mut mass = vec![(mass_of(work)?, centre_of(work, head_y)?)];
    let mut lo = 0.0f32;
    let mut hi = len;
    hi = hi.max(head_y + bounds_of(work)?[1][1]);
    lo = lo.min(head_y + bounds_of(work)?[0][1]);
    for (slot, trim, trim_type) in filled.iter().filter(|(s, _, _)| s.slot == "trim") {
        let bore = on_line(trim, "bore", lib.types.rules.join_m.value)?;
        let cover = trim.cover_m.ok_or_else(|| {
            format!(
                "{}: a trim on the line gives cover_m; run node tools/items/part.ts {}",
                trim.part, trim.part
            )
        })?;
        let y = match trim_type.place.as_deref() {
            Some("under_entry") => head_y + entry.at[1] - cover[1],
            Some("over_butt") => 0.0,
            other => {
                return Err(format!(
                    "{}: trim {} places by {:?}, which a line item does not take",
                    recipe.kind, trim.part, other
                ));
            }
        };
        let d = size(trim, bore)?;
        section.push([y + cover[0].max(-y), d]);
        section.push([y + cover[1], d]);
        placed.push((trim.part.clone(), slot.slot.as_str(), y));
        mass.push((mass_of(trim)?, centre_of(trim, y)?));
        lo = lo.min(y + bounds_of(trim)?[0][1]);
        hi = hi.max(y + bounds_of(trim)?[1][1]);
    }
    section.sort_by(|a, b| a[0].total_cmp(&b[0]));
    section.dedup_by(|b, a| (b[0] - a[0]).abs() < 1e-6);
    if section[0][0] > 0.0 {
        section.insert(0, [0.0, section[0][1]]);
    }
    let (haft_m, haft_c) = haft_mass(&section, len, density);
    mass.push((haft_m, haft_c));

    let total: f32 = mass.iter().map(|(m, _)| m).sum();
    let centre = mass.iter().map(|(m, c)| m * c).sum::<f32>() / total;
    let length = hi - lo;
    let balance = centre - grip;
    let inertia = total * (balance * balance + length * length / 12.0);
    let head_mass = mass[0].0;
    envelope(kind, length, head_mass, balance, inertia)?;

    let edge = edge_of(work, head_y - grip);
    let second = handle
        .second_from_grip_m
        .as_ref()
        .map(|s| [0.0, s.value, 0.0]);
    Ok(Assembly {
        recipe: recipe.clone(),
        placed: placed
            .into_iter()
            .map(|(part, slot, y)| Placed {
                part,
                slot: slot.to_string(),
                at: [0.0, y - grip, 0.0],
                turn: ONE,
            })
            .collect(),
        section: section
            .into_iter()
            .filter(|s| s[0] <= len)
            .map(|[y, d]| [y - grip, d])
            .collect(),
        grip_diameter_m: grip_d,
        second,
        edge,
        length_m: length,
        mass_kg: total,
        balance_m: balance,
        inertia_kgm2: inertia,
    })
}

type Filled<'a> = Vec<(&'a data::Slot, &'a Part, &'a PartType)>;

/// Each recipe part into the first free slot of its kind that fits its type; every slot not
/// optional filled.
fn fill<'a>(lib: &'a Library, kind: &'a Kind, recipe: &Recipe) -> Result<Filled<'a>, String> {
    let mut out: Filled<'a> = Vec::new();
    for id in &recipe.parts {
        let part = lib.parts.get(id).ok_or_else(|| format!("part {id} is not in assets/data/parts; make it and run node tools/items/part.ts {id}"))?;
        let ty = lib
            .types
            .types
            .get(&part.kind)
            .ok_or_else(|| format!("part {id}: its type {} is not in types.json", part.kind))?;
        let slot = kind.slots.iter().find(|s| {
            s.fits.contains(&part.kind) && !out.iter().any(|(t, _, _)| std::ptr::eq(*t, *s))
        });
        let slot = slot.ok_or_else(|| {
            format!(
                "part {id} ({}) fits no free slot of the {} (its slots: {})",
                part.kind,
                kind.kind,
                names(
                    kind.slots
                        .iter()
                        .map(|s| format!("{} takes {}", s.name, s.fits.join("|")))
                )
            )
        })?;
        out.push((slot, part, ty));
    }
    if let Some(s) = kind
        .slots
        .iter()
        .find(|s| !s.optional && !out.iter().any(|(t, _, _)| std::ptr::eq(*t, *s)))
    {
        return Err(format!(
            "the {}'s {} slot is empty: a recipe gives it one of {}",
            kind.kind,
            s.name,
            s.fits.join(", ")
        ));
    }
    Ok(out)
}

fn envelope(kind: &Kind, length: f32, head: f32, balance: f32, inertia: f32) -> Result<(), String> {
    let e = &kind.envelope;
    for (what, v, band) in [
        ("length_m", length, &e.length_m),
        ("head_mass_kg", head, &e.head_mass_kg),
        ("balance_m", balance, &e.balance_m),
        ("inertia_kgm2", inertia, &e.inertia_kgm2),
    ] {
        if let Some([lo, hi]) = band.known()
            && (v < lo || v > hi)
        {
            return Err(format!(
                "the {}'s {what} {v:.4} is outside its envelope [{lo}, {hi}] (assets/data/kinds/{}.json, D459's tempo clamp): choose other parts, or it is another kind with its own clips",
                kind.kind, kind.kind
            ));
        }
    }
    Ok(())
}

/// The frustums between the section's joins, of the handle's wood: its mass and centre along +Y.
fn haft_mass(section: &[[f32; 2]], len: f32, density: f32) -> (f32, f32) {
    let (mut m, mut moment) = (0.0, 0.0);
    for w in section.windows(2) {
        let ([y0, d0], [y1, d1]) = (w[0], w[1]);
        let (y1, d1) = if y1 > len {
            (len, d0 + (d1 - d0) * (len - y0) / (y1 - y0))
        } else {
            (y1, d1)
        };
        if y1 <= y0 {
            continue;
        }
        let (r0, r1, h) = (d0 / 2.0, d1 / 2.0, y1 - y0);
        let v = PI * h * (r0 * r0 + r0 * r1 + r1 * r1) / 3.0;
        // A frustum's centroid from its r0 end: h (r0^2 + 2 r0 r1 + 3 r1^2) / (4 (r0^2 + r0 r1 + r1^2)).
        let c = y0
            + h * (r0 * r0 + 2.0 * r0 * r1 + 3.0 * r1 * r1) / (4.0 * (r0 * r0 + r0 * r1 + r1 * r1));
        m += v * density;
        moment += v * density * c;
    }
    (m, if m > 0.0 { moment / m } else { 0.0 })
}

fn edge_of(work: &Part, y: f32) -> Option<Edge> {
    let at = |p: &Point| [p.at[0], p.at[1] + y, p.at[2]];
    match (
        work.point("edge.lower"),
        work.point("edge.upper"),
        work.point("tip").or(work.point("face")),
    ) {
        (Some(a), Some(b), _) => Some(Edge {
            from: at(a),
            to: at(b),
            side: a.dir,
        }),
        (_, _, Some(t)) => Some(Edge {
            from: at(t),
            to: at(t),
            side: t.dir,
        }),
        _ => None,
    }
}

fn on_line<'a>(part: &'a Part, name: &str, join: f32) -> Result<&'a Point, String> {
    let p = part.point(name).ok_or_else(|| {
        format!(
            "part {} has no {name} point; run node tools/items/part.ts {}",
            part.part, part.part
        )
    })?;
    if p.at[0].abs() > join || p.at[2].abs() > join || (p.dir[1] - 1.0).abs() > 1e-3 {
        return Err(format!(
            "part {}: {name} at {:?} facing {:?} is off the line: a join along a handle is at x = z = 0 facing +Y (D566)",
            part.part, p.at, p.dir
        ));
    }
    Ok(p)
}

fn size(part: &Part, p: &Point) -> Result<f32, String> {
    p.size
        .as_ref()
        .map(Size::across)
        .ok_or_else(|| format!("part {}: {} has no size", part.part, p.name))
}

fn need(part: &Part, v: Option<&data::Sourced<f32>>, what: &str) -> Result<f32, String> {
    v.map(|s| s.value).ok_or_else(|| {
        format!(
            "part {}: no {what}; a {} is made from it (types.json)",
            part.part, part.kind
        )
    })
}

fn mass_of(part: &Part) -> Result<f32, String> {
    need(part, part.mass_kg.as_ref(), "mass_kg")
}

fn centre_of(part: &Part, y: f32) -> Result<f32, String> {
    part.centre_m.map(|c| c[1] + y).ok_or_else(|| {
        format!(
            "part {}: no centre_m; the part checker writes it",
            part.part
        )
    })
}

fn bounds_of(part: &Part) -> Result<[[f32; 3]; 2], String> {
    part.bounds_m.ok_or_else(|| {
        format!(
            "part {}: no bounds_m; the part checker writes it",
            part.part
        )
    })
}

fn names<T: std::fmt::Display>(it: impl Iterator<Item = T>) -> String {
    it.map(|t| t.to_string()).collect::<Vec<_>>().join(", ")
}
