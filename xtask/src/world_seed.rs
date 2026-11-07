//! A world is made from its seed alone (D596): every random stream in the game's code starts
//! from the world's seed, derived by name or place (`terra_gen::Seed::derive`, `at`, `index`),
//! never from a seed written into the code or from the operating system's randomness. Then
//! the same seed makes the same world on every machine, and two seeds make two worlds.
//!
//! Three rules over the crates' own code (tests, benches, examples and programs are free to pick
//! a seed, as they are about one world):
//!
//! - **No seed written in**: `Seed(1)`, `Seed(0x5EED)`, or a number given a seed's name
//!   (`const SEED: u32 = 2`, `let seed = 7`, `seed: 3` in a struct). A stream is the world's
//!   seed derived for its purpose. What is heard and not part of the world (a sound's dice),
//!   a look's grain baked once, and the seed of seed 1's pinned demos are listed in
//!   [`ALLOWED`] with why.
//! - **No system randomness**: `thread_rng`, `rand::random` and the like, anywhere but for a
//!   secret, listed in [`SECRETS`] with why. The clock
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

/// Where the system's randomness is the point: a secret, which must not be repeatable and
/// never reaches the world. The file, and why. Nothing else may read it.
const SECRETS: &[(&str, &str)] = &[(
    "crates/terra_walker/src/net/session.rs",
    "a hosted server's challenge secret (D592): a hello overheard on one run must not pass on another, and the world's seed is public",
)];

/// Seeds written in that are not the world's, each with why it may be: the file, and why.
const ALLOWED: &[(&str, &str)] = &[
    (
        "crates/terra_net/src/transport/loopback.rs",
        "a link's dice: which packets a test's network loses, not the world",
    ),
    (
        "crates/terra_net/src/transport/conditioned.rs",
        "a link's dice, as the loopback's: which packets a soak's link loses",
    ),
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
        "crates/terra_world/src/ground_lattice.rs",
        "the ground shader's noise lattice: a look's grain, as the hashed noise it stands for is the same in every world",
    ),
    (
        "crates/terra_world/src/rises.rs",
        "when a fish rises: a moment's effect, not the world",
    ),
    (
        "crates/terra_world/src/audio/synth.rs",
        "a synth's length measured from one voice: what is heard, not the world",
    ),
    (
        "crates/terra_sandbox/src/bed.rs",
        "a sound bed's layers' dice: what is heard, not the world",
    ),
    (
        "crates/terra_folk/src/town_sound.rs",
        "the town's murmur's dice: what is heard, not the world",
    ),
    (
        "crates/terra_world/src/sky/noise3d.rs",
        "the clouds' noise texture, baked once: a look's grain, the same sky texture in every world (the weather is the world's)",
    ),
    (
        "crates/terra_world/src/fire/volume.rs",
        "the flames' gas noise texture: a look's grain and a moment's effect",
    ),
    (
        "crates/terra_build_layout/src/layout/roofs/frame.rs",
        "a batten's own grain in its build's frame, as KEYED_BY_PLACE's terra_build_layout: the same in every world",
    ),
    (
        "crates/terra_voxel/src/patterned.rs",
        "ICE_WEDGES' placeholder seed: a world's ground gives its own (terra_land, seed.derive(\"ice wedges\"))",
    ),
    (
        "crates/terra_items/src/assembly/command.rs",
        "the assembly tool's default --seed, a command line's, not a world's",
    ),
    (
        "crates/terra_world/src/planet/entering.rs",
        "FIRST_SEED, the seed of a run given none and of a world kept from before seeds (D596)",
    ),
    (
        "crates/terra_land/src/site.rs",
        "PINNED_SEED, the world the demos' places are pinned on, and PINNED_TOWN_SEED, its demo town's (D393, D596)",
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
        "crates/terra_net/src/identity.rs",
        "challenge",
        "a connection's challenge from the server's own secret (D592): who may join, not the world; a world's seed is public and would let a hello be forged",
    ),
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
        "crates/terra_world/src/ground_lattice.rs",
        "value",
        "the ground shader's noise lattice: a look's grain, as the hashed noise it stands for is the same in every world",
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
    (
        "crates/terra_iso/src/party.rs",
        "idle_phase",
        "where along its idle an actor starts, from where it stands: a moment's effect, so a still is the same run to run",
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

/// Rust's integer types, which a seed written in as a number is declared as.
const INTEGERS: [&str; 12] = [
    "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize",
];

/// Whether a line gives a name for a seed a number written in: `const SEED: u32 = 2;`,
/// `let town_seed = 0x5EED;`, `Wanted { seed: 7, .. }`. A name holding `seed` in any case,
/// then `=`, or `:` with an integer type and `=`, or `:` alone as a field is given, then a
/// digit (`wanted.seed = 2` too). A parameter (`seed: u32`), a comparison (`seed == 1`) or a
/// value taken from another name (`seed: wanted.seed`) is not one.
fn named_seed_number(line: &str) -> bool {
    let ident = |c: char| c.is_alphanumeric() || c == '_';
    // An integer: a digit first, and no `.` after its digits (`0.3` is a shader's float).
    let number = |rest: &str| {
        let rest = rest.trim_start();
        let digits = rest.find(|c: char| !c.is_ascii_alphanumeric() && c != '_');
        rest.starts_with(|c: char| c.is_ascii_digit())
            && !digits.is_some_and(|end| rest[end..].starts_with('.'))
    };
    let assigned = |rest: &str| {
        let rest = rest.trim_start();
        rest.starts_with('=')
            && !rest.starts_with("==")
            && !rest.starts_with("=>")
            && number(&rest[1..])
    };
    let lower = line.to_ascii_lowercase();
    lower.match_indices("seed").any(|(at, _)| {
        // The name ends in its word `seed` (`seed`, `TOWN_SEED`): `SEEDS` or `SEED_DIGITS`
        // count something.
        let end = at + "seed".len();
        let word = line[..at].ends_with('_') || !line[..at].ends_with(ident);
        if !word || line[end..].starts_with(ident) {
            return false;
        }
        let rest = line[end..].trim_start();
        match rest.strip_prefix(':') {
            Some(typed) if !typed.starts_with(':') => {
                let typed = typed.trim_start();
                match INTEGERS
                    .iter()
                    .find(|int| typed.starts_with(**int) && !typed[int.len()..].starts_with(ident))
                {
                    Some(int) => assigned(&typed[int.len()..]),
                    None => number(typed),
                }
            }
            _ => assigned(rest),
        }
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
        let secret = SECRETS.iter().any(|(file, _)| *file == path);
        if let Some(what) = SYSTEM_RANDOMNESS
            .iter()
            .find(|what| line.contains(*what))
            .filter(|_| !secret)
        {
            found.push(format!(
                "{path}:{number}: {what} reads the system's randomness: derive a seed from the world's instead (terra_gen::Seed::derive)"
            ));
        }
        let written = written_seed(line) || named_seed_number(line);
        if written && !ALLOWED.iter().any(|(file, _)| *file == path) {
            found.push(format!(
                "{path}:{number}: a seed written in: derive it from the world's seed (planet.seed().derive(\"what it is for\"), WorldSpec::seed); a sound's dice or a look's grain may be listed in xtask/src/world_seed.rs's ALLOWED with why"
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
    fn a_seed_named_and_written_in_as_a_number_is_found() {
        let path = "crates/terra_folk/src/town.rs";
        let written = "pub const SEED: u32 = 2;\nconst TOWN_SEED: u64 = 0x5EED;\nlet seed = 7;\nlet mut far_seed: usize = 3;\nWanted { seed: 9, near };\nwanted.seed = 4;\n";
        let found = breaches(path, written);
        assert_eq!(found.len(), 6, "{found:#?}");
        let taken = "fn f(seed: u32, s: Seed) -> u32 {\n    let seed = planet.seed().derive(\"town\").0 as u32;\n    let w = Wanted { seed: SEED, near };\n    if seed == 1 { return 3; }\n    match seed { 1 => 2, _ => 3 }\n    let x: u32 = 2;\n    info!(\"seed: {seed} = 1\");\n}\nstruct A { seed: u32, seeds: Vec<u32> }\n";
        let found = breaches(path, taken);
        assert!(found.is_empty(), "{found:#?}");
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
