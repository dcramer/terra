//! An assembled item's model (D565): a recipe put together by the assembler
//! (terra_items::assembly) and written as a glb the held-look checks read as a held look.
//!
//!   cargo run -p terra_bake --bin item -- <kind> <part>... --out FILE.glb [--material M] [--seed N]
//!
//! `--parts -` reads the parts as a JSON array on stdin in place of assets/data/parts', and
//! `--models DIR` reads their models' paths under DIR instead of the asset root (the tests'
//! made-up parts). The same recipe and models write the same bytes.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use terra_items::assembly::{Command, assemble};

fn main() -> ExitCode {
    match run() {
        Ok(line) => {
            println!("✓ item: {line}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("✗ item: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<String, String> {
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let cmd = Command::parse(std::env::args().skip(1), &assets)?;
    let out = cmd
        .flags
        .get("out")
        .ok_or("--out FILE.glb names where the item's model goes")?;
    let models = cmd
        .flags
        .get("models")
        .map_or(assets.clone(), PathBuf::from);
    let item = assemble(&cmd.lib, &cmd.recipe(0)?)?;
    let bytes = terra_bake::item::write(&cmd.lib, &item, |path| {
        std::fs::read(models.join(path)).map_err(|e| {
            format!(
                "{}: {e}; make the part's model (the models skill) or fix its /model",
                models.join(path).display()
            )
        })
    })?;
    std::fs::write(out, &bytes).map_err(|e| format!("{out}: {e}"))?;
    Ok(format!(
        "{out}, {} parts, {:.3} kg, balance {:.3} m, {} bytes",
        item.placed.len(),
        item.mass_kg,
        item.balance_m,
        bytes.len()
    ))
}
