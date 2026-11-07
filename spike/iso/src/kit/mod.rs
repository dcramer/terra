//! The shared biome kit (D634): a biome is a data file, `spike/iso/biomes/<name>.ron`
//! (biome.rs, its schema), and the kit draws it. A light is a preset beside it,
//! `biomes/light/<name>.ron` (light.rs). A new kind of thing (a layout, a scatter shape, a
//! weather) is kit code with a test; a biome that needs code shows the kit is missing
//! something. How to add one: .agents/skills/iso-art/references/kit.md.

pub mod biome;
pub mod effects;
pub mod ground;
pub mod laid;
pub mod light;
pub mod scatter;
pub mod street;
pub mod water;

use std::path::PathBuf;

/// The biomes' folder, wherever the spike runs from (the repository's root on the worker,
/// spike/iso under `cargo test`).
pub fn dir() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/biomes"))
}

/// A RON file read into `T`, or the error a person fixes it by (D556): the file, the line
/// and column, what ron found, and what to do, naming the schema's file.
pub fn read<T: serde::de::DeserializeOwned>(
    path: &std::path::Path,
    schema: &str,
) -> Result<T, String> {
    let text = std::fs::read_to_string(path).map_err(|e| {
        format!(
            "{}: {e}. A biome is spike/iso/biomes/<name>.ron and a light \
             spike/iso/biomes/light/<name>.ron (D634): check the name, or write the file \
             (references/kit.md in the iso-art skill says how)",
            path.display()
        )
    })?;
    parse(&text, &path.display().to_string(), schema)
}

/// `read` on text already in hand; `name` is what the error calls it. An optional field
/// takes its value bare (`lux: 30.0`) or as `Some(30.0)`.
pub fn parse<T: serde::de::DeserializeOwned>(
    text: &str,
    name: &str,
    schema: &str,
) -> Result<T, String> {
    let options =
        ron::Options::default().with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME);
    options.from_str(text).map_err(|e| {
        use ron::error::Error as E;
        let fix = match &e.code {
            E::NoSuchStructField { .. } => format!(
                "the schema has no such field: rename it to one of those, or, if the kit \
                 should draw something new, add the field to spike/iso/src/kit/{schema} \
                 with a test (D634), never a field the kit ignores"
            ),
            E::MissingStructField { .. } => format!(
                "add the field; its doc comment in spike/iso/src/kit/{schema} says what it \
                 holds, and spike/iso/biomes/ruin.ron is a working file"
            ),
            E::NoSuchEnumVariant { .. } => format!(
                "use one of those names; a new one is kit code in spike/iso/src/kit/{schema}"
            ),
            _ => format!(
                "check the RON syntax there against spike/iso/biomes/ruin.ron and the schema \
                 in spike/iso/src/kit/{schema}"
            ),
        };
        format!(
            "{name}:{}:{}: {}. Fix: {fix}",
            e.span.start.line, e.span.start.col, e.code
        )
    })
}
