//! The words the assembler's commands share (`assemble` here, `item` in terra_bake): the
//! library read from the assets, `--parts -` replacing its parts from stdin, a recipe's
//! `--material` and `--seed`, and any other `--flag value` left for the command.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

use super::{Library, Recipe};

pub struct Command {
    pub lib: Library,
    /// The words that are not flags, in order.
    pub words: Vec<String>,
    pub material: String,
    pub seed: u64,
    /// Every other `--flag value`.
    pub flags: BTreeMap<String, String>,
}

impl Command {
    /// From the command's arguments, the library read from `assets`.
    pub fn parse(args: impl IntoIterator<Item = String>, assets: &Path) -> Result<Command, String> {
        let mut cmd = Command {
            lib: Library::read(assets)?,
            words: Vec::new(),
            material: "iron".into(),
            seed: 0,
            flags: BTreeMap::new(),
        };
        let mut it = args.into_iter();
        while let Some(a) = it.next() {
            let Some(flag) = a.strip_prefix("--") else {
                cmd.words.push(a);
                continue;
            };
            let value = it.next().ok_or_else(|| format!("--{flag} takes a value"))?;
            match flag {
                "parts" if value == "-" => {
                    let mut text = String::new();
                    std::io::stdin()
                        .read_to_string(&mut text)
                        .map_err(|e| e.to_string())?;
                    let parts: Vec<serde_json::Value> = serde_json::from_str(&text)
                        .map_err(|e| format!("stdin is not a JSON array of parts: {e}"))?;
                    let texts: Vec<String> = parts.iter().map(|p| p.to_string()).collect();
                    cmd.lib.parts.clear();
                    cmd.lib.add_parts(texts.iter().map(String::as_str))?;
                }
                "parts" => return Err("--parts takes -, the parts as a JSON array on stdin".into()),
                "material" => cmd.material = value,
                "seed" => {
                    cmd.seed = value
                        .parse()
                        .map_err(|_| format!("--seed {value} is not a whole number"))?
                }
                _ => {
                    cmd.flags.insert(flag.to_string(), value);
                }
            }
        }
        Ok(cmd)
    }

    /// The recipe of `<kind> <part>...`, the words after the first `skip`.
    pub fn recipe(&self, skip: usize) -> Result<Recipe, String> {
        let [kind, parts @ ..] = self.words.get(skip..).unwrap_or_default() else {
            return Err("name the kind, then its parts: <kind> <part>...".into());
        };
        Ok(Recipe {
            kind: kind.clone(),
            parts: parts.to_vec(),
            material: self.material.clone(),
            seed: self.seed,
        })
    }
}
