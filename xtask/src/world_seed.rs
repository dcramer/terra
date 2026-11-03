//! A world is made from its seed alone (D596): every random stream in the game's code starts
//! from the world's seed, derived by name or place (`terra_gen::Seed::derive`, `at`, `index`),
//! never from a seed written into the code or from the operating system's randomness. Then
//! the same seed makes the same world on every machine, and two seeds make two worlds.
//!
//! Three rules over the crates' own code (tests, benches, examples and programs are free to pick
//! a seed, as they are about one world):
//!
//! - **No seed written in**: `Seed(1)`, `Seed(0x5EED)`. A stream is the world's seed derived
//!   for its purpose. What is heard and not part of the world (a sound's dice) is listed in
//!   [`ALLOWED`] with why.
//! - **No system randomness**: `thread_rng`, `rand::random` and the like, anywhere. The clock
//!   is the shared crates' rule (`check`); a Bevy crate times itself with it.
//! - **No hash keyed by place alone**: a `terra_gen::hash` call whose arguments name no seed
//!   (`hash::unit3(0, x, y, z)`) draws the same at the same place in every world. World
//!   content takes the world's seed; a look's grain, a moment's effect or a thing's own
//!   detail keyed in its own frame is listed in [`KEYED_BY_PLACE`] with why.
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
];

/// terra_gen's hashes (`terra_gen::hash`): what draws a number from words (`of`, `unit3`,
/// `mix3`) and the mixers a hash or stream is made of.
const HASHES: [&str; 8] = [
    "of",
    "unit3",
    "mix3",
    "splitmix64",
    "lowbias32",
    "lowbias32_round",
    "prospector32",
    "prospector32_round",
];

/// Words in a hash's arguments that show it takes the world's seed: a seed derived from it
/// (`seed`, `building_seed`, `settings.seed`), or the state of a stream begun from one and
/// stepped on (`state`, `self.0`). Case counts: a `SEED` constant is a seed written in.
const SEEDED: [&str; 3] = ["seed", "state", "self."];

/// Hashes keyed by a place or a number alone that may be, as none makes one world unlike
/// another: the file or folder (ending `/`), the function (or "" for all of them), and why.
/// Each is a look's grain (a texture, a shader's mirror), a moment's effect, a thing's own
/// detail keyed in its own frame (a build's cells, whose plan the world's seed laid), or not
/// a draw at all. World content (where things are, layouts, land, flora, creatures, weather)
/// takes the world's seed instead.
const KEYED_BY_PLACE: &[(&str, &str, &str)] = &[
    (
        "crates/terra_build_layout/",
        "",
        "a build laid from its own cells (a Base's grid): its boards, stones, stairs and trims follow what is built, as a blueprint's, the same in every world; what is built and where is the world's",
    ),
    (
        "crates/terra_build_mesh/",
        "",
        "a build's meshes, chips and rubble from its own cells, as terra_build_layout",
    ),
    (
        "crates/terra_build/src/fire.rs",
        "",
        "how fire takes a build's cubes: a moment's effect",
    ),
    (
        "crates/terra_build/src/loss.rs",
        "",
        "what falls when a build is broken: a moment's effect",
    ),
    (
        "crates/terra_building/src/loss/",
        "",
        "the dust and debris of a build's fall and their textures: a moment's effect",
    ),
    (
        "crates/terra_bases/src/arrays/",
        "",
        "the crack masks drawn on damaged builds: a texture's grain",
    ),
    (
        "crates/terra_dungeon/src/stamp/",
        "",
        "a laid dungeon's cubes and furnishings from its own cells; its rooms and dressing are laid from its seed (terra_dungeon::generate)",
    ),
    (
        "crates/terra_world/src/fire/",
        "",
        "flames, embers and their light: a moment's effect",
    ),
    (
        "crates/terra_world/src/sky/",
        "",
        "the clouds' noise texture: a look's grain; the weather that moves them is the world's",
    ),
    (
        "crates/terra_world/src/scene.rs",
        "",
        "the haze's density texture: a look's grain",
    ),
    (
        "crates/terra_world/src/waves.rs",
        "",
        "the sea's wave train: a look's grain",
    ),
    (
        "crates/terra_voxel/src/planet/index.rs",
        "",
        "a map's key, not a draw",
    ),
    (
        "crates/terra_creatures/src/creatures.rs",
        "own_number",
        "a creature's number from its home, which its dice take with the world's seed (own_dice) and which spreads its far thinking over the ticks (turn)",
    ),
    (
        "crates/terra_creatures/src/melee/burst.rs",
        "",
        "a blow's burst: a moment's effect",
    ),
    (
        "crates/terra_town/src/animals.rs",
        "unit",
        "a fold over its callers' words, each led by the town's seed",
    ),
    (
        "crates/terra_folk/src/town/windows.rs",
        "",
        "which windows are lit at night and how their glass looks: the evening's look",
    ),
    (
        "crates/terra_sandbox/src/experiments/lush/thicket/",
        "",
        "a tree's twigs keyed by the limb they grow on, whose shape the forest's seed grew",
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

/// The name of the function the end of `code` is in: the last `fn name` before it.
fn enclosing_fn(code: &str) -> &str {
    let ident = |c: char| c.is_alphanumeric() || c == '_';
    let mut end = code.len();
    while let Some(found) = code[..end].rfind("fn ") {
        if !code[..found].ends_with(ident) {
            let name = &code[found + 3..];
            return &name[..name.find(|c: char| !ident(c)).unwrap_or(name.len())];
        }
        end = found;
    }
    ""
}

/// The calls of terra_gen's hashes in a file's code, tests and comments left out, by
/// `hash::name(` or by a name the file imports (`use terra_gen::hash::{of, unit}`): each its
/// line, the hash, the function it is in and its arguments.
fn hash_calls(text: &str) -> Vec<(usize, &'static str, String, String)> {
    let lines = code_lines(text);
    let (mut code, mut starts) = (String::new(), Vec::new());
    for (number, line) in &lines {
        starts.push((code.len(), *number));
        code.push_str(line);
        code.push('\n');
    }
    let imported: Vec<&str> = lines
        .iter()
        .filter_map(|(_, line)| line.trim().strip_prefix("use terra_gen::hash::"))
        .flat_map(|names| {
            let names = names.trim_end_matches(';');
            names.trim_matches(['{', '}']).split(',').map(str::trim)
        })
        .collect();
    let ident = |c: char| c.is_alphanumeric() || c == '_' || c == ':' || c == '.';
    let mut found = Vec::new();
    for hash in HASHES {
        let called = format!("{hash}(");
        for (at, _) in code.match_indices(&called) {
            let before = &code[..at];
            let by_name = imported.contains(&hash) && !before.ends_with(ident);
            if !before.ends_with("hash::") && !by_name {
                continue;
            }
            let open = at + called.len();
            let mut depth = 1;
            let close = code[open..]
                .char_indices()
                .find_map(|(i, c)| {
                    match c {
                        '(' => depth += 1,
                        ')' => depth -= 1,
                        _ => {}
                    }
                    (depth == 0).then_some(open + i)
                })
                .unwrap_or(code.len());
            let line = starts
                .iter()
                .rev()
                .find(|(start, _)| *start <= at)
                .map_or(0, |(_, number)| *number);
            let within = enclosing_fn(before).to_string();
            found.push((line, hash, within, code[open..close].to_string()));
        }
    }
    found.sort_by_key(|call| call.0);
    found
}

/// A file's hashes keyed by a place or a number alone, not by the world's seed, as lines to
/// print; those listed in [`KEYED_BY_PLACE`] are marked used instead.
fn unseeded(path: &str, text: &str, used: &mut [bool]) -> Vec<String> {
    if path.starts_with("crates/terra_gen/") {
        return Vec::new();
    }
    let mut found = Vec::new();
    for (line, hash, within, arguments) in hash_calls(text) {
        if SEEDED.iter().any(|word| arguments.contains(word)) {
            continue;
        }
        let listed = KEYED_BY_PLACE.iter().position(|(place, function, _)| {
            let here = match place.ends_with('/') {
                true => path.starts_with(place),
                false => path == *place,
            };
            here && (function.is_empty() || *function == within)
        });
        match listed {
            Some(entry) => used[entry] = true,
            None => found.push(format!(
                "{path}:{line}: {hash}(…) in {within} is keyed by a place or a number alone, the same in every world: take the world's seed into it (the system's own, from planet.seed().derive(\"what it is for\")); a look's grain, a moment's effect or a thing's own detail in its own frame may be listed in xtask/src/world_seed.rs's KEYED_BY_PLACE with why"
            )),
        }
    }
    found
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
    let mut used = vec![false; KEYED_BY_PLACE.len()];
    for (path, text) in sources
        .iter()
        .filter(|(path, _)| checked(path) && !a_test(path))
    {
        found.extend(breaches(path, text));
        found.extend(unseeded(path, text, &mut used));
    }
    for (file, _) in ALLOWED {
        if !root().join(file).exists() {
            found.push(format!(
                "{file} is allowed a seed of its own but is gone: take it out of ALLOWED"
            ));
        }
    }
    for ((place, function, _), used) in KEYED_BY_PLACE.iter().zip(&used) {
        if !used {
            found.push(format!(
                "{place} {function} is listed in KEYED_BY_PLACE, but no hash there is keyed by place alone now: take it out"
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
    fn a_hash_keyed_by_place_alone_is_found_and_one_taking_a_seed_is_not() {
        let text = "use terra_gen::hash::{of, splitmix64};\nfn lay(at: IVec3, seed: u32) {\n    let a = terra_gen::hash::unit3(0, at.x as u32, 1, 2);\n    let b = hash::of(&[seed, at.x as u32]);\n    let c = hash::of(&[\n        at.x as u32,\n        7,\n    ]);\n    let d = size_of(&[1]) + of(&[seed]) + std::mem::size_of::<u8>();\n}\nfn step(&mut self) -> u64 { self.0 = splitmix64(self.0); splitmix64(7) }\n#[cfg(test)]\nfn t() { hash::of(&[1]); }\n";
        let mut used = vec![false; KEYED_BY_PLACE.len()];
        let found = unseeded("crates/terra_x/src/lay.rs", text, &mut used);
        assert_eq!(found.len(), 3, "{found:#?}");
        assert!(found[0].starts_with("crates/terra_x/src/lay.rs:3: unit3(…) in lay "));
        assert!(found[1].starts_with("crates/terra_x/src/lay.rs:5: of(…) in lay "));
        assert!(found[2].starts_with("crates/terra_x/src/lay.rs:11: splitmix64(…) in step "));
        assert!(!used.contains(&true));
        // Listed, for a file or a folder and for all its functions or one.
        let waves = "crates/terra_world/src/waves.rs";
        assert!(unseeded(waves, text, &mut used).is_empty());
        let entry = KEYED_BY_PLACE
            .iter()
            .position(|(place, ..)| *place == waves);
        assert!(used[entry.unwrap()]);
        let creatures = "crates/terra_creatures/src/creatures.rs";
        assert_eq!(unseeded(creatures, text, &mut used).len(), 3);
        let own = text.replace("fn lay", "fn own_number");
        assert_eq!(unseeded(creatures, &own, &mut used).len(), 1);
        // terra_gen's own primitives are what the rule is about.
        assert!(unseeded("crates/terra_gen/src/seed.rs", text, &mut used).is_empty());
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
