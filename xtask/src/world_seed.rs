//! A world is made from its seed alone (D596): every random stream in the game's code starts
//! from the world's seed, derived by name or place (`terra_gen::Seed::derive`, `at`, `index`),
//! never from a seed written into the code or from the operating system's randomness. Then
//! the same seed makes the same world on every machine, and two seeds make two worlds.
//!
//! Two rules over the crates' own code (tests, benches, examples and programs are free to pick
//! a seed, as they are about one world):
//!
//! - **No seed written in**: `Seed(1)`, `Seed(0x5EED)`. A stream is the world's seed derived
//!   for its purpose. What is heard and not part of the world (a sound's dice) is listed in
//!   [`ALLOWED`] with why.
//! - **No system randomness**: `thread_rng`, `rand::random` and the like, anywhere. The clock
//!   is the shared crates' rule (`check`); a Bevy crate times itself with it.
//!
//! The determinism tests (`terra_land::fingerprint`) check what this cannot see: the same seed
//! gives the same world bit for bit, and each part of it changes with the seed.

use crate::{output, root};
use std::fmt::Write as _;

/// What reads the operating system's randomness, so a run cannot be repeated.
const SYSTEM_RANDOMNESS: [&str; 8] = [
    "thread_rng",
    "rand::random",
    "rand::rng()",
    "RandomState::new",
    "getrandom",
    "OsRng",
    "from_entropy",
    "fastrand::",
];

/// Seeds written in that are not the world's, each with why it may be: the file, and why.
const ALLOWED: &[(&str, &str)] = &[
    (
        "crates/terra_game/src/play/music.rs",
        "the music's dice: what is heard, not the world",
    ),
    (
        "crates/terra_game/src/play/noises.rs",
        "the calls' dice: what is heard, not the world",
    ),
    (
        "crates/terra_game/src/play/soundscape.rs",
        "the bells' dice: what is heard, not the world",
    ),
    (
        "crates/terra_world/src/audio/bank.rs",
        "the sound bank's dice: what is heard, not the world",
    ),
    (
        "crates/terra_world/src/rises.rs",
        "when a fish rises: a moment's effect, not the world",
    ),
    (
        "crates/terra_creatures/src/creatures.rs",
        "a creature's dice are keyed by its home, which the world's seed chose (wildlife meetings); deriving them from the world's seed changes every duel's rolls, the combat scorecard's to take",
    ),
    (
        "crates/terra_creatures/src/wildlife/mod.rs",
        "the climate's growing season at sea level, kept once for every world, which the climate's noise does not move",
    ),
    (
        "crates/terra_sandbox/src/experiments/lush.rs",
        "the lush forest, a demo scene of one forest, its look baselines and the trailer's, until its rules are the world's flora",
    ),
];

/// Whether a file is the crates' own code, not a test's, a bench's, an example's or a program's.
fn checked(path: &str) -> bool {
    path.starts_with("crates/")
        && path.ends_with(".rs")
        && !path.ends_with("tests.rs")
        && !path.ends_with("main.rs")
        && !path.contains("_bench")
        && !["/tests/", "/examples/", "/benches/", "/bin/", "/testing/"]
            .iter()
            .any(|part| path.contains(part))
}

const TESTS_ONLY: &str = "#[cfg(test)]";

/// The files of the modules a source declares for its tests alone (`#[cfg(test)] mod name;`),
/// and the folders of their own modules, which are tests' too (a folder ends in `/`).
fn test_modules(path: &str, text: &str) -> Vec<String> {
    let base = match path.rsplit_once('/') {
        Some((dir, "lib.rs" | "main.rs" | "mod.rs")) => dir.to_string(),
        Some((dir, file)) => format!("{dir}/{}", file.trim_end_matches(".rs")),
        None => return Vec::new(),
    };
    let mut found = Vec::new();
    let mut after = false;
    for line in text.lines() {
        let mut code = line.trim();
        if let Some(rest) = code.strip_prefix(TESTS_ONLY) {
            (after, code) = (true, rest.trim());
        }
        if !after || code.is_empty() {
            continue;
        }
        // `#[path = "x.rs"]`, beside the file declaring it.
        let dir = path.rsplit_once('/').map_or("", |(dir, _)| dir);
        if let Some(file) = code
            .strip_prefix("#[path = \"")
            .and_then(|r| r.split('"').next())
        {
            found.push(format!("{dir}/{file}"));
        }
        let declared = ["mod ", "pub mod ", "pub(crate) mod ", "pub(super) mod "]
            .iter()
            .find_map(|start| code.strip_prefix(start)?.strip_suffix(';'));
        if let Some(name) = declared {
            found.push(format!("{base}/{name}.rs"));
            found.push(format!("{base}/{name}/"));
        }
        after = code.starts_with("#[");
    }
    found
}

/// The lines of a source, numbered from 1, comments and what is only for tests left out: the
/// item after a `#[cfg(test)]`, a line to its `;` or its braces' end.
fn code_lines(text: &str) -> Vec<(usize, &str)> {
    let mut kept = Vec::new();
    let (mut testing, mut depth) = (false, 0i32);
    for (number, line) in text.lines().enumerate() {
        let mut code = line.split("//").next().unwrap_or("");
        if let Some(rest) = code.trim_start().strip_prefix(TESTS_ONLY) {
            testing = true;
            code = rest;
            if code.trim().is_empty() {
                continue;
            }
        }
        if testing {
            depth += code.matches('{').count() as i32 - code.matches('}').count() as i32;
            let ended = depth <= 0 && (code.contains('}') || code.trim_end().ends_with(';'));
            if ended {
                (testing, depth) = (false, 0);
            }
            continue;
        }
        kept.push((number + 1, code));
    }
    kept
}

/// Whether a line makes a seed of a number written in: `Seed(7)`, `terra_gen::Seed(0x5EED`.
fn written_seed(line: &str) -> bool {
    line.match_indices("Seed(").any(|(at, _)| {
        let before = line[..at].chars().next_back();
        let named = before.is_some_and(|c| c.is_alphanumeric() || c == '_');
        let after = line[at + "Seed(".len()..].trim_start();
        !named && after.starts_with(|c: char| c.is_ascii_digit())
    })
}

/// What in one file breaks the rules, as lines to print.
fn breaches(path: &str, text: &str) -> Vec<String> {
    let mut found = Vec::new();
    for (number, line) in code_lines(text) {
        if let Some(what) = SYSTEM_RANDOMNESS.iter().find(|what| line.contains(*what)) {
            found.push(format!(
                "{path}:{number}: {what} reads the system's randomness: derive a seed from the world's instead (terra_gen::Seed::derive)"
            ));
        }
        if written_seed(line) && !ALLOWED.iter().any(|(file, _)| *file == path) {
            found.push(format!(
                "{path}:{number}: a seed written in: derive it from the world's seed (planet.seed().derive(\"what it is for\"), WorldSpec::seed); a sound's dice may be listed in xtask/src/world_seed.rs's ALLOWED"
            ));
        }
    }
    found
}

/// The check's stage: every stream not started from the world's seed.
pub(crate) fn seeded_from_the_world(out: &mut String) -> bool {
    let Some(listed) = output("git", &["ls-files", "--", "crates"]) else {
        let _ = writeln!(out, "git cannot list the Rust sources");
        return false;
    };
    let sources: Vec<(&str, String)> = listed
        .lines()
        .filter(|path| path.ends_with(".rs"))
        .filter_map(|path| Some((path, std::fs::read_to_string(root().join(path)).ok()?)))
        .collect();
    let tests: Vec<String> = sources
        .iter()
        .flat_map(|(path, text)| test_modules(path, text))
        .collect();
    let a_test = |path: &str| {
        tests.iter().any(|test| match test.ends_with('/') {
            true => path.starts_with(test.as_str()),
            false => path == test,
        })
    };
    let mut found = Vec::new();
    for (path, text) in sources
        .iter()
        .filter(|(path, _)| checked(path) && !a_test(path))
    {
        found.extend(breaches(path, text));
    }
    for (file, _) in ALLOWED {
        if !root().join(file).exists() {
            found.push(format!(
                "{file} is allowed a seed of its own but is gone: take it out of ALLOWED"
            ));
        }
    }
    if found.is_empty() {
        return true;
    }
    let _ = writeln!(
        out,
        "a world is made from its seed alone (D596); these start a stream from something else:"
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
    fn a_seed_written_in_is_found_and_one_derived_or_named_is_not() {
        let path = "crates/terra_world/src/planted.rs";
        let planted = "let a = Seed(7).rng();\nlet b = terra_gen::Seed( 0x5EED + i);\nlet c = Seed(world.seed).derive(\"x\");\nlet d = PlanetSeed(3); // Seed(4)\n";
        let found = breaches(path, planted);
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(found[0].starts_with("crates/terra_world/src/planted.rs:1: "));
        assert!(found[1].starts_with("crates/terra_world/src/planted.rs:2: "));
        // Its tests may pick one; what follows them is checked again.
        let tested = "#[cfg(test)]\nmod tests;\n#[cfg(test)]\nimpl A {\n    fn t() {\n        Seed(1);\n    }\n}\nfn f() { Seed(2); }\n#[cfg(test)]\nmod tests { fn t() { Seed(3); } }\n";
        let found = breaches(path, tested);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].starts_with("crates/terra_world/src/planted.rs:9: "));
        assert!(!checked("crates/terra_world/src/flora/tests.rs"));
        assert!(!checked("crates/terra_land/src/bin/fields.rs"));
        assert!(checked("crates/terra_world/src/flora.rs"));
    }

    #[test]
    fn a_module_declared_for_tests_alone_is_a_tests_file() {
        let lib = "mod a;\n#[cfg(test)]\nmod movement_mover;\n#[cfg(test)] mod staged;\nmod b;\n";
        assert_eq!(
            test_modules("crates/w/src/lib.rs", lib),
            [
                "crates/w/src/movement_mover.rs",
                "crates/w/src/movement_mover/",
                "crates/w/src/staged.rs",
                "crates/w/src/staged/"
            ]
        );
        let flora = "#[cfg(test)]\n#[allow(dead_code)]\nmod tests;\nmod under;\n";
        assert_eq!(
            test_modules("crates/w/src/flora.rs", flora),
            ["crates/w/src/flora/tests.rs", "crates/w/src/flora/tests/"]
        );
        let moved = "#[cfg(test)]\n#[path = \"movement_mover.rs\"]\nmod mover_tests;\n";
        assert_eq!(
            test_modules("crates/w/src/movement.rs", moved)[0],
            "crates/w/src/movement_mover.rs"
        );
    }

    #[test]
    fn the_systems_randomness_is_found() {
        let found = breaches("crates/x/src/lib.rs", "let r = rand::thread_rng();\n");
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].contains("thread_rng"));
    }

    #[test]
    fn every_stream_in_the_repository_starts_from_the_world_seed() {
        let mut out = String::new();
        assert!(seeded_from_the_world(&mut out), "{out}");
    }
}
