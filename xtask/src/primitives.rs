//! One definition of each shared primitive (D396): a mixer or other small piece of maths that
//! many places need is written once, in the lowest crate every user can depend on, and called
//! from there. Written out again by hand, copies drift apart, and nobody can tell which places
//! must agree. Each primitive here is known by its constants; the check fails if one appears in
//! a Rust file anywhere but the primitive's home. A new primitive is a row of [`PRIMITIVES`].

use crate::{output, root};
use std::fmt::Write as _;

/// A primitive: what it is, the file it lives in, what to call instead, and what gives it
/// away: its constants, as hex digits without `0x` or separators (so this file is not a
/// copy), or, for one with no telling constant, the names of its functions defined again.
pub(crate) struct Primitive {
    pub(crate) what: &'static str,
    pub(crate) home: &'static str,
    pub(crate) call: &'static str,
    pub(crate) constants: &'static [&'static str],
    pub(crate) functions: &'static [&'static str],
}

pub(crate) const PRIMITIVES: &[Primitive] = &[
    Primitive {
        what: "the 32-bit hash mixers (lowbias32 and the prospector's)",
        home: "crates/terra_gen/src/hash.rs",
        call: "terra_gen::hash::{lowbias32, lowbias32_round, prospector32, prospector32_round}",
        constants: &["7feb352d", "846ca68b", "2c1b3c6d", "297a2d39"],
        functions: &[],
    },
    Primitive {
        what: "SplitMix64's finaliser",
        home: "crates/terra_gen/src/hash.rs",
        call: "terra_gen::hash::splitmix64",
        constants: &["bf58476d1ce4e5b9", "94d049bb133111eb"],
        functions: &[],
    },
    Primitive {
        what: "the smoothstep easing",
        home: "crates/terra_gen/src/ease.rs",
        call: "terra_gen::smoothstep",
        constants: &[],
        functions: &["smoothstep"],
    },
];

/// The folders whose Rust is searched: the workspace and the tasks. Spikes are thrown away.
const SEARCHED: [&str; 2] = ["crates", "xtask"];

/// Every hex literal on a line, as lower-case digits without separators or leading zeros.
fn hex_literals(line: &str) -> Vec<String> {
    let mut found = Vec::new();
    let bytes = line.as_bytes();
    let mut i = 0;
    while i + 1 < bytes.len() {
        let starts = bytes[i] == b'0'
            && matches!(bytes[i + 1], b'x' | b'X')
            && (i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_'));
        if !starts {
            i += 1;
            continue;
        }
        let mut j = i + 2;
        let mut digits = String::new();
        while j < bytes.len() && (bytes[j].is_ascii_hexdigit() || bytes[j] == b'_') {
            if bytes[j] != b'_' {
                digits.push(bytes[j].to_ascii_lowercase() as char);
            }
            j += 1;
        }
        let digits = digits.trim_start_matches('0');
        if !digits.is_empty() {
            found.push(digits.to_string());
        }
        i = j;
    }
    found
}

/// The function a line defines, if it does: `fn name(`, after any `pub`, `pub(…)` or `const`.
fn defined_function(line: &str) -> Option<&str> {
    let mut rest = line.trim_start();
    loop {
        if let Some(after) = rest.strip_prefix("pub(") {
            rest = after.split_once(')')?.1.trim_start();
        } else if let Some(after) = rest
            .strip_prefix("pub ")
            .or_else(|| rest.strip_prefix("const "))
        {
            rest = after.trim_start();
        } else {
            break;
        }
    }
    let name = rest.strip_prefix("fn ")?;
    let end = name.find(['(', '<'])?;
    Some(name[..end].trim())
}

/// Where a source writes out a primitive's constant, or defines one of its functions, though
/// it is not its home: line (from 1) and what to say of it.
fn copies(path: &str, text: &str, primitives: &[Primitive]) -> Vec<String> {
    let mut found = Vec::new();
    for (number, line) in text.lines().enumerate() {
        if let Some(name) = defined_function(line) {
            for primitive in primitives {
                if primitive.home != path && primitive.functions.contains(&name) {
                    found.push(format!(
                        "{path}:{}: fn {name} is {}: call {} ({})",
                        number + 1,
                        primitive.what,
                        primitive.call,
                        primitive.home
                    ));
                }
            }
        }
        for literal in hex_literals(line) {
            for primitive in primitives {
                if primitive.home != path && primitive.constants.contains(&literal.as_str()) {
                    found.push(format!(
                        "{path}:{}: 0x{literal} is {}: call {} ({})",
                        number + 1,
                        primitive.what,
                        primitive.call,
                        primitive.home
                    ));
                }
            }
        }
    }
    found
}

/// The check's stage: every copy of a primitive found outside its home, with what to call.
pub(crate) fn one_definition(out: &mut String) -> bool {
    let mut args = vec!["ls-files", "--"];
    args.extend(SEARCHED);
    let Some(listed) = output("git", &args) else {
        let _ = writeln!(out, "git cannot list the Rust sources");
        return false;
    };
    let mut found = Vec::new();
    for path in listed.lines().filter(|path| path.ends_with(".rs")) {
        let Ok(text) = std::fs::read_to_string(root().join(path)) else {
            continue;
        };
        found.extend(copies(path, &text, PRIMITIVES));
    }
    for primitive in PRIMITIVES {
        if !root().join(primitive.home).exists() {
            found.push(format!(
                "{} has no home: {} is gone",
                primitive.what, primitive.home
            ));
        }
    }
    if found.is_empty() {
        return true;
    }
    let _ = writeln!(
        out,
        "a shared primitive written out again; call the one in its home instead, with the same input, so what it gives stays the same:"
    );
    for line in found {
        let _ = writeln!(out, "  {line}");
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literals_are_found_however_they_are_written() {
        // Written in pieces, so this file holds no copy of its own.
        let line = format!(
            "let a = 0x{}_{}; b = 0x0000_{}_u32; c = x0x1; d=0xff;",
            "7FEB", "352D", "7feb352d"
        );
        assert_eq!(hex_literals(&line), vec!["7feb352d", "7feb352d", "ff"]);
    }

    #[test]
    fn a_planted_copy_fails_and_its_home_and_lookalikes_pass() {
        let mixer = PRIMITIVES[0].constants[0];
        let copy = format!(
            "fn mine(mut x: u32) -> u32 {{\n    x ^= x >> 16;\n    x.wrapping_mul(0x{}_{})\n}}",
            &mixer[..4],
            &mixer[4..]
        );
        let found = copies("crates/terra_game/src/planted.rs", &copy, PRIMITIVES);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(
            found[0].starts_with("crates/terra_game/src/planted.rs:3: "),
            "{found:?}"
        );
        assert!(found[0].contains(PRIMITIVES[0].call), "{found:?}");
        assert!(copies(PRIMITIVES[0].home, &copy, PRIMITIVES).is_empty());
        // The golden ratio scatters inputs everywhere; it is not a mixer.
        let scatter = "let h = i.wrapping_mul(0x9e37_79b9) ^ j.wrapping_mul(0x85eb_ca6b);";
        assert!(copies("crates/x.rs", scatter, PRIMITIVES).is_empty());
    }

    #[test]
    fn a_function_defined_again_fails_and_calls_and_lookalikes_pass() {
        let easing = PRIMITIVES
            .iter()
            .find(|primitive| primitive.functions.contains(&"smoothstep"))
            .expect("smoothstep is a primitive");
        let planted =
            "use x;\n\npub(super) fn smoothstep(a: f32, b: f32, x: f32) -> f32 {\n    0.0\n}\n";
        let found = copies("crates/terra_game/src/planted.rs", planted, PRIMITIVES);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(
            found[0].starts_with("crates/terra_game/src/planted.rs:3: fn smoothstep "),
            "{found:?}"
        );
        assert!(found[0].contains(easing.call), "{found:?}");
        assert!(copies(easing.home, planted, PRIMITIVES).is_empty());
        let fine = "    let k = smoothstep(0.0, 1.0, x);\nfn smoothstep_twice(x: f32) -> f32 {\n// fn smoothstep(\n";
        assert!(copies("crates/x.rs", fine, PRIMITIVES).is_empty());
    }

    #[test]
    fn a_definition_is_read_past_its_visibility() {
        assert_eq!(defined_function("fn a(x: f32)"), Some("a"));
        assert_eq!(
            defined_function("    pub(crate) const fn b<T>()"),
            Some("b")
        );
        assert_eq!(defined_function("pub fn c ()"), Some("c"));
        assert_eq!(defined_function("let d = fn_pointer(1);"), None);
    }

    /// Each home holds every constant it is known by and defines every function, so one
    /// mistyped here would be caught: a primitive that matched nothing would guard nothing.
    #[test]
    fn every_home_holds_its_constants_and_functions() {
        for primitive in PRIMITIVES {
            let text = std::fs::read_to_string(root().join(primitive.home)).expect(primitive.home);
            let held: Vec<String> = text.lines().flat_map(hex_literals).collect();
            for constant in primitive.constants {
                assert!(
                    held.iter().any(|literal| literal == constant),
                    "{} lacks {constant}",
                    primitive.home
                );
            }
            for function in primitive.functions {
                assert!(
                    text.lines()
                        .any(|line| defined_function(line) == Some(function)),
                    "{} does not define {function}",
                    primitive.home
                );
            }
        }
    }

    #[test]
    fn the_repository_has_one_definition_of_each() {
        let mut out = String::new();
        assert!(one_definition(&mut out), "{out}");
    }
}
