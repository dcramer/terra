//! What the assembler reads: the part types (`assets/data/parts/types.json`), the kinds
//! (`assets/data/kinds/<kind>.json`) and the parts (`assets/data/parts/<part>.json`), the same
//! files the TypeScript checks hold to their schemas (`tools/items`). Only the fields the
//! assembler uses are read; serde ignores the rest, which the checks own.

use std::collections::BTreeMap;

use serde::Deserialize;

/// A number with its source; the assembler reads only the value.
#[derive(Clone, Debug, Deserialize)]
pub struct Sourced<T> {
    pub value: T,
}

/// A kind's number or band, or "open" (not yet known, so not checked).
#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum Band {
    Known(Sourced<[f32; 2]>),
    Open(Sourced<String>),
}

impl Band {
    pub fn known(&self) -> Option<[f32; 2]> {
        match self {
            Band::Known(b) => Some(b.value),
            Band::Open(_) => None,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Envelope {
    pub inertia_kgm2: Band,
    pub length_m: Band,
    pub head_mass_kg: Band,
    pub balance_m: Band,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Slot {
    pub slot: String,
    pub name: String,
    pub fits: Vec<String>,
    #[serde(default)]
    pub optional: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Kind {
    pub kind: String,
    pub slots: Vec<Slot>,
    pub envelope: Envelope,
}

/// A part type's template: who makes it, its slot and how it is placed (D566).
#[derive(Clone, Debug, Deserialize)]
pub struct PartType {
    pub made_by: String,
    pub slot: String,
    /// A working part's point the handle enters by (`eye.lower`, `socket.mouth`, `shoulder`).
    pub entry: Option<String>,
    /// A working part's point the handle's top stops at (`eye.upper`, `socket.bottom`).
    pub handle_top: Option<String>,
    /// A trim's placing: `under_entry`, `over_butt` or `seat`.
    pub place: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Rules {
    pub proud_m: Sourced<f32>,
    pub join_m: Sourced<f32>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Types {
    pub rules: Rules,
    pub types: BTreeMap<String, PartType>,
}

/// A connection point: a diameter, or `[width, thickness]` for a flat tang.
#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum Size {
    Round(f32),
    Flat([f32; 2]),
}

impl Size {
    /// Across, for a round join; a tang's width.
    pub fn across(&self) -> f32 {
        match self {
            Size::Round(d) => *d,
            Size::Flat([w, _]) => *w,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Point {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub at: [f32; 3],
    pub dir: [f32; 3],
    pub size: Option<Size>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Part {
    pub part: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub mass_kg: Option<Sourced<f32>>,
    pub centre_m: Option<[f32; 3]>,
    pub bounds_m: Option<[[f32; 3]; 2]>,
    pub cover_m: Option<[f32; 2]>,
    #[serde(default)]
    pub points: Vec<Point>,
    pub length_m: Option<Sourced<f32>>,
    pub grip_from_end_m: Option<Sourced<f32>>,
    pub second_from_grip_m: Option<Sourced<f32>>,
    pub grip_diameter_m: Option<Sourced<f32>>,
    pub density_kg_m3: Option<Sourced<f32>>,
}

impl Part {
    pub fn point(&self, name: &str) -> Option<&Point> {
        self.points.iter().find(|p| p.name == name)
    }
}

/// Everything an assembly may use, read once: the types, the kinds and the parts by id.
#[derive(Clone, Debug)]
pub struct Library {
    pub types: Types,
    pub kinds: BTreeMap<String, Kind>,
    pub parts: BTreeMap<String, Part>,
}

impl Library {
    /// From the files' text: `types.json`, every kind's and every part's.
    pub fn from_json(types: &str, kinds: &[&str], parts: &[&str]) -> Result<Library, String> {
        let types: Types = serde_json::from_str(types).map_err(|e| {
            format!("assets/data/parts/types.json: {e}; run node tools/items/kind.ts")
        })?;
        let mut lib = Library {
            types,
            kinds: BTreeMap::new(),
            parts: BTreeMap::new(),
        };
        for text in kinds {
            let k: Kind = serde_json::from_str(text)
                .map_err(|e| format!("a kind: {e}; run node tools/items/kind.ts"))?;
            lib.kinds.insert(k.kind.clone(), k);
        }
        for text in parts {
            let p: Part = serde_json::from_str(text)
                .map_err(|e| format!("a part: {e}; run node tools/items/part.ts <part>"))?;
            lib.parts.insert(p.part.clone(), p);
        }
        Ok(lib)
    }
}
