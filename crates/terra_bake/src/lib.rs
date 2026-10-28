//! What the bake's commands share: the glTF binary read and written (`glb`, the texture bake's
//! and the item's), and an assembled item written as a model (`item`, `cargo run -p terra_bake
//! --bin item`).

pub mod glb;
pub mod item;
