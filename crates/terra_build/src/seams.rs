//! The line between building's rules and what lays and draws them (D407): the rules (cubes,
//! ops, plans, kits, costs, loss, the store) never name the layout, the drawing, the
//! settlements or the fixtures, so the rules can become a crate of their own that a server
//! compiles without them. Every module of the crate is on one side; a new one is put on its
//! side here. Tests and test-only items may name anything.

use std::path::{Path, PathBuf};

/// The rules: what a base is and how it changes, with no layout or art.
const RULES: &[&str] = &[
    "construct",
    "cost",
    "cubes",
    "fire",
    "ground",
    "junctions",
    "kit",
    "layers",
    "lib",
    "look",
    "loss",
    "ops",
    "order",
    "parts",
    "pieces",
    "plans",
    "put_up",
    "ray",
    "roofer",
    "rooms",
    "save",
    "shortcuts",
    "support",
];

/// What stands over the rules: the layout (where pieces go, which a server needs), the
/// drawing, the settlements and the fixtures.
const ABOVE: &[&str] = &[
    // The layout.
    "fitting",
    "layout",
    "walk", //
    // The drawing.
    "cells",
    "checker",
    "draw",
    "dress",
    "fragments",
    "hull",
    "inspect",
    "mesh",
    "modules",
    "packs",
    "relief",
    "roofing",
    "sections",
    "surface",
    "timber", //
    // The settlements (the town is terra_town).
    "village", //
    // The fixtures.
    "roof_walls",
    "scenes",
];

/// This test's own module.
const OWN: &str = "seams";

fn src() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

/// A module's files: `name.rs` and everything under `name/`, but its tests.
fn files_of(name: &str) -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            if path.is_dir() {
                if stem != "tests" {
                    walk(&path, out);
                }
            } else if path.extension().is_some_and(|e| e == "rs") && stem != "tests" {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    let file = src().join(format!("{name}.rs"));
    if file.is_file() {
        out.push(file);
    }
    walk(&src().join(name), &mut out);
    out.sort();
    out
}

/// Code with its comments, strings and character literals blanked (newlines kept, so lines
/// still count), and every `#[cfg(test)]` item blanked with them.
fn code_of(text: &str) -> Vec<u8> {
    let b = text.as_bytes();
    let mut out = b.to_vec();
    let blank = |out: &mut Vec<u8>, from: usize, to: usize| {
        for c in &mut out[from..to.min(b.len())] {
            if *c != b'\n' {
                *c = b' ';
            }
        }
    };
    let mut i = 0;
    while i < b.len() {
        let rest = &b[i..];
        if rest.starts_with(b"//") {
            let end = rest
                .iter()
                .position(|&c| c == b'\n')
                .map_or(b.len(), |n| i + n);
            blank(&mut out, i, end);
            i = end;
        } else if rest.starts_with(b"/*") {
            let (mut depth, mut j) = (0usize, i);
            while j < b.len() {
                if b[j..].starts_with(b"/*") {
                    depth += 1;
                    j += 2;
                } else if b[j..].starts_with(b"*/") {
                    depth -= 1;
                    j += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    j += 1;
                }
            }
            blank(&mut out, i, j);
            i = j;
        } else if rest.starts_with(b"r#") || rest.starts_with(b"r\"") {
            let hashes = rest[1..].iter().take_while(|&&c| c == b'#').count();
            let close: Vec<u8> = std::iter::once(b'"')
                .chain(std::iter::repeat_n(b'#', hashes))
                .collect();
            let open = i + 1 + hashes + 1;
            let end = b[open..]
                .windows(close.len())
                .position(|w| w == close.as_slice())
                .map_or(b.len(), |n| open + n + close.len());
            blank(&mut out, i, end);
            i = end;
        } else if b[i] == b'"' {
            let mut j = i + 1;
            while j < b.len() && b[j] != b'"' {
                j += if b[j] == b'\\' { 2 } else { 1 };
            }
            blank(&mut out, i, j + 1);
            i = j + 1;
        } else if b[i] == b'\'' && (rest.get(1) == Some(&b'\\') || rest.get(2) == Some(&b'\'')) {
            let mut j = i + 1;
            while j < b.len() && b[j] != b'\'' {
                j += if b[j] == b'\\' { 2 } else { 1 };
            }
            blank(&mut out, i, j + 1);
            i = j + 1;
        } else {
            i += 1;
        }
    }
    // A test-only item: to the end of its statement, or its block.
    let attribute = b"#[cfg(test)]";
    let mut from = 0;
    while let Some(at) = out[from..]
        .windows(attribute.len())
        .position(|w| w == attribute)
        .map(|n| from + n)
    {
        let mut j = at + attribute.len();
        while j < out.len() && out[j] != b';' && out[j] != b'{' {
            j += 1;
        }
        if j < out.len() && out[j] == b'{' {
            let mut depth = 0usize;
            while j < out.len() {
                match out[j] {
                    b'{' => depth += 1,
                    b'}' => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
                j += 1;
            }
        }
        let end = (j + 1).min(out.len());
        for c in &mut out[at..end] {
            if *c != b'\n' {
                *c = b' ';
            }
        }
        from = end;
    }
    out
}

fn ident(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

/// Each module of `ABOVE` the code names, and the line it does on: as `crate::name`, in a
/// `crate::{..}` group, or as a path starting `name::`.
fn named(code: &[u8]) -> Vec<(usize, &'static str)> {
    let line = |at: usize| code[..at].iter().filter(|&&c| c == b'\n').count() + 1;
    let mut found = Vec::new();
    for at in 0..code.len() {
        if at > 0 && (ident(code[at - 1]) || code[at - 1] == b':') {
            continue;
        }
        let rest = &code[at..];
        for &name in ABOVE {
            let n = name.len();
            if rest.starts_with(name.as_bytes()) && rest[n..].starts_with(b"::") {
                found.push((line(at), name));
            }
        }
        if !rest.starts_with(b"crate::") {
            continue;
        }
        let after = &rest[7..];
        let inner: &[u8] = if after.first() == Some(&b'{') {
            let end = after.iter().position(|&c| c == b';').unwrap_or(after.len());
            &after[1..end]
        } else {
            after
        };
        // The names at the head of the group's paths (or the one path).
        let mut depth = 0usize;
        let mut head = true;
        for (k, &c) in inner.iter().enumerate() {
            match c {
                b'{' => depth += 1,
                b'}' if depth == 0 => break,
                b'}' => depth -= 1,
                b',' if depth == 0 => head = true,
                c if head && ident(c) => {
                    head = false;
                    let word: Vec<u8> = inner[k..]
                        .iter()
                        .copied()
                        .take_while(|&c| ident(c))
                        .collect();
                    if let Some(&name) = ABOVE.iter().find(|&&n| n.as_bytes() == word.as_slice()) {
                        found.push((line(at), name));
                    }
                }
                c if !c.is_ascii_whitespace() => head = false,
                _ => {}
            }
            if inner.as_ptr() == after.as_ptr() && !head && !ident(c) {
                break;
            }
        }
    }
    found.sort();
    found.dedup();
    found
}

/// The crate root re-exports what the crate offers from every side, and declares its
/// modules: its `pub use` and `mod` lines are its face, not a rule naming a module.
fn without_the_roots_face(code: Vec<u8>) -> Vec<u8> {
    let text = String::from_utf8(code).expect("utf-8");
    let mut out = String::with_capacity(text.len());
    let mut skipping = false;
    for line in text.split_inclusive('\n') {
        let t = line.trim_start();
        if skipping
            || t.starts_with("pub use ")
            || t.starts_with("pub mod ")
            || t.starts_with("mod ")
        {
            skipping = !line.trim_end().ends_with(';');
            out.extend(line.chars().map(|c| if c == '\n' { c } else { ' ' }));
        } else {
            out.push_str(line);
        }
    }
    out.into_bytes()
}

#[test]
fn every_module_is_on_one_side() {
    let mut unplaced = Vec::new();
    for entry in std::fs::read_dir(src()).expect("src").flatten() {
        let path = entry.path();
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        if path.extension().is_some_and(|e| e != "rs") {
            continue;
        }
        if stem != OWN && !RULES.contains(&stem.as_str()) && !ABOVE.contains(&stem.as_str()) {
            unplaced.push(stem);
        }
    }
    unplaced.sort();
    unplaced.dedup();
    assert!(
        unplaced.is_empty(),
        "put each module on its side in src/seams.rs (D407), RULES or ABOVE: {unplaced:?}"
    );
}

#[test]
fn the_rules_never_name_the_layout_or_the_drawing() {
    let mut crossings = Vec::new();
    for &module in RULES {
        for file in files_of(module) {
            let text = std::fs::read_to_string(&file).expect("read");
            let mut code = code_of(&text);
            if module == "lib" {
                code = without_the_roots_face(code);
            }
            let shown = file
                .strip_prefix(src())
                .unwrap_or(&file)
                .display()
                .to_string();
            for (line, name) in named(&code) {
                crossings.push(format!("src/{shown}:{line} names `{name}`"));
            }
        }
    }
    assert!(
        crossings.is_empty(),
        "a rules module names one above it (D407): move the type it needs down into the rules \
         (a plain-data type the drawing also reads goes in `pieces`), or the code up beside \
         what it names:\n  {}",
        crossings.join("\n  ")
    );
}

/// The check sees each way of naming a module, and none of what only looks like it.
#[test]
fn it_sees_a_module_named_every_way() {
    let code = code_of(
        "use crate::layout::Piece;\nuse crate::{Base, mesh::Surface};\nfn f() { timber::draw(); }\n\
         // crate::dress in a comment\nconst S: &str = \"crate::draw\";\nfn g() { a::layout::b(); }\n\
         #[cfg(test)]\nmod tests { use crate::inspect; }\n",
    );
    assert_eq!(
        named(&code),
        vec![(1, "layout"), (2, "mesh"), (3, "timber")]
    );
}
