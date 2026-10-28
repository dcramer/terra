//! The assembler as a command (D565, D566): the one placing rule, for the tools that measure what
//! it places. A binary of terra_items rather than an xtask, so it builds from serde alone in
//! seconds and xtask does not rebuild whenever the assembler changes.
//!
//!   assemble kinds <type> [<kind>...]             the kinds taking a part of <type> and those it is
//!                                                 made for, <kind>... the ones it names
//!   assemble joins <part>                         how <part> sits with each partner its kinds allow
//!   assemble item <kind> <part>... [--material M] [--seed N]   the item's canonical bytes
//!
//! `--parts -` reads a JSON array of parts from stdin in place of assets/data/parts' (the part
//! checker sends the parts it checks against, made-up ones in its tests). JSON on stdout; an
//! error on stderr says what to fix. The item's model: `cargo run -p terra_bake --bin item`.

use std::path::Path;
use std::process::ExitCode;

use terra_items::assembly::{Command, Kind, assemble, kinds_for, kinds_taking, pairings};

fn main() -> ExitCode {
    match run() {
        Ok(out) => {
            println!("{out}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("✗ assemble: {e}");
            ExitCode::FAILURE
        }
    }
}

const USAGE: &str = "use: assemble kinds <type> [<kind>...] | assemble joins <part> | assemble item <kind> <part>... [--material M] [--seed N]; add --parts - to read the parts from stdin";

fn run() -> Result<String, String> {
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let cmd = Command::parse(std::env::args().skip(1), &assets)?;
    match cmd.words.first().map(String::as_str) {
        Some("kinds") => {
            let [_, kind, named @ ..] = cmd.words.as_slice() else {
                return Err(USAGE.into());
            };
            let names = |ks: Vec<&Kind>| ks.into_iter().map(|k| k.kind.clone()).collect::<Vec<_>>();
            let made_for = kinds_for(&cmd.lib, kind, (!named.is_empty()).then_some(named));
            let out = serde_json::json!({
                "taking": names(kinds_taking(&cmd.lib, kind)),
                "made_for": names(made_for),
            });
            Ok(out.to_string())
        }
        Some("joins") => {
            let [_, part] = cmd.words.as_slice() else {
                return Err(USAGE.into());
            };
            serde_json::to_string(&pairings(&cmd.lib, part)?).map_err(|e| e.to_string())
        }
        Some("item") => String::from_utf8(assemble(&cmd.lib, &cmd.recipe(1)?)?.to_bytes())
            .map_err(|e| e.to_string()),
        _ => Err(USAGE.into()),
    }
}
