//! What the assembler reads: the part types (`assets/data/parts/types.json`), the materials'
//! densities (`assets/data/parts/materials.json`), the kinds (`assets/data/kinds/<kind>.json`)
//! and the parts (`assets/data/parts/<part>.json`), the same files the TypeScript checks hold to
//! their schemas (`tools/items`). Only the fields the assembler uses are read; serde ignores the
//! rest, which the checks own.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

/// A number with its source; the assembler reads only the value.
#[derive(Clone, Debug, Deserialize)]
pub struct Sourced<T> {
    pub value: T,
}

/// A number that must carry its source to be read at all (a material's density).
#[derive(Clone, Debug, Deserialize)]
pub struct Cited {
    pub value: f32,
    pub source: String,
}

/// A material a recipe names: what its working part is made of (assets/data/parts/materials.json).
#[derive(Clone, Debug, Deserialize)]
pub struct Material {
    pub density_kg_m3: Cited,
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
    /// A full point's roll, square to `dir` (a seat or a nock, D566).
    pub up: Option<[f32; 3]>,
    pub size: Option<Size>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Part {
    pub part: String,
    #[serde(rename = "type")]
    pub kind: String,
    /// The kinds it is made for, when its type fits several.
    pub kinds: Option<Vec<String>>,
    /// Its model under the asset root, when a model makes it.
    pub model: Option<String>,
    /// A code-made part's look under the asset root: a model whose first material (one, named by
    /// its role) the part is drawn in, on its own UVs (tools/items/haft.py).
    pub skin: Option<String>,
    /// A trim's mass: its own material, whatever the item's.
    pub mass_kg: Option<Sourced<f32>>,
    /// A working part's volume: its mass is this times its recipe's material's density.
    pub volume_m3: Option<Sourced<f32>>,
    pub centre_m: Option<[f32; 3]>,
    pub bounds_m: Option<[[f32; 3]; 2]>,
    pub cover_m: Option<[f32; 2]>,
    #[serde(default)]
    pub points: Vec<Point>,
    pub length_m: Option<Sourced<f32>>,
    pub grip_from_end_m: Option<Sourced<f32>>,
    pub second_from_grip_m: Option<Sourced<f32>>,
    pub grip_diameter_m: Option<Sourced<f32>>,
    /// A code-made haft's own shape: `[up from the butt, diameter]` points (m) off its joins and
    /// grip, such as the swell at the butt the hand closes against and the throat it tapers to
    /// under the head; the handle runs straight between them and the joins.
    pub profile_m: Option<Sourced<Vec<[f32; 2]>>>,
    pub density_kg_m3: Option<Sourced<f32>>,
}

impl Part {
    pub fn point(&self, name: &str) -> Option<&Point> {
        self.points.iter().find(|p| p.name == name)
    }
}

#[derive(Clone, Debug, Deserialize)]
struct Materials {
    materials: BTreeMap<String, Material>,
}

/// Everything an assembly may use, read once: the types, the materials, the kinds and the parts
/// by id.
#[derive(Clone, Debug)]
pub struct Library {
    pub types: Types,
    pub materials: BTreeMap<String, Material>,
    pub kinds: BTreeMap<String, Kind>,
    pub parts: BTreeMap<String, Part>,
}

/// The data files beside the part types that are not parts.
const NOT_PARTS: [&str; 3] = ["types.json", "materials.json", "part.schema.json"];

impl Library {
    /// From the files' text: `types.json`, `materials.json`, every kind's and every part's.
    pub fn from_json(
        types: &str,
        materials: &str,
        kinds: &[&str],
        parts: &[&str],
    ) -> Result<Library, String> {
        let types: Types = serde_json::from_str(types).map_err(|e| {
            format!("assets/data/parts/types.json: {e}; run node tools/items/kind.ts")
        })?;
        let materials: Materials = serde_json::from_str(materials).map_err(|e| {
            format!("assets/data/parts/materials.json: {e}; each material gives density_kg_m3 as {{\"value\", \"source\"}}")
        })?;
        let mut lib = Library {
            types,
            materials: materials.materials,
            kinds: BTreeMap::new(),
            parts: BTreeMap::new(),
        };
        for text in kinds {
            let k: Kind = serde_json::from_str(text)
                .map_err(|e| format!("a kind: {e}; run node tools/items/kind.ts"))?;
            lib.kinds.insert(k.kind.clone(), k);
        }
        lib.add_parts(parts.iter().copied())?;
        Ok(lib)
    }

    /// Reads the library from the asset root's `data/parts` and `data/kinds`.
    pub fn read(assets: &Path) -> Result<Library, String> {
        let text =
            |p: &Path| std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()));
        let jsons = |dir: &Path, skip: &[&str]| -> Result<Vec<String>, String> {
            let mut files: Vec<_> = std::fs::read_dir(dir)
                .map_err(|e| format!("{}: {e}", dir.display()))?
                .filter_map(|f| f.ok().map(|f| f.path()))
                .filter(|p| {
                    p.extension().is_some_and(|e| e == "json")
                        && !p.file_name().is_some_and(|n| skip.iter().any(|s| n == *s))
                })
                .collect();
            files.sort();
            files.iter().map(|p| text(p)).collect()
        };
        let (parts, kinds) = (assets.join("data/parts"), assets.join("data/kinds"));
        let kind_texts = jsons(&kinds, &["kind.schema.json"])?;
        let part_texts = jsons(&parts, &NOT_PARTS)?;
        Library::from_json(
            &text(&parts.join("types.json"))?,
            &text(&parts.join("materials.json"))?,
            &kind_texts.iter().map(String::as_str).collect::<Vec<_>>(),
            &part_texts.iter().map(String::as_str).collect::<Vec<_>>(),
        )
    }

    /// Puts these parts in, each replacing any of its id.
    pub fn add_parts<'a>(&mut self, parts: impl Iterator<Item = &'a str>) -> Result<(), String> {
        for text in parts {
            let p: Part = serde_json::from_str(text)
                .map_err(|e| format!("a part: {e}; run node tools/items/part.ts <part>"))?;
            self.parts.insert(p.part.clone(), p);
        }
        Ok(())
    }
}
