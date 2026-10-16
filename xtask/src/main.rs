//! Project tasks, and the one place every tool is started from. Run as `cargo xtask <task>`;
//! `cargo xtask tools` lists them all, from `TASKS` below. A new tool gets a line there.

mod bench;
mod build;
mod check;
mod doctor;
mod load;
mod scenes;
mod seed;
mod stage;
mod windows;
mod worktrees;

use bench::bench;
use build::{artifacts, contract, fuzz, kits, rules, sections, texels};
use check::check;
use doctor::doctor;
use windows::{play, win, yard};

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

/// A task: its name, what it takes, what it does, the group it is listed under, and how
/// it runs, given the arguments after its name.
struct Task {
    name: &'static str,
    takes: &'static str,
    does: &'static str,
    group: &'static str,
    run: fn(&[String]) -> bool,
}

/// Every task, in the order `tools` lists them. The TypeScript and Blender tools are started
/// here too, so nobody has to remember where each one lives; each has its own README.
const TASKS: &[Task] = &[
    Task {
        name: "play",
        takes: "[--fresh] [game args]",
        does: "play the tech demo on the real GPU, wherever you are; --fresh starts a new game",
        group: "Play and check",
        run: play,
    },
    Task {
        name: "yard",
        takes: "[game args]",
        does: "play the yard, a flat test level for building: nothing on it, building free and instant, nothing saved",
        group: "Play and check",
        run: yard,
    },
    Task {
        name: "loop",
        takes: "",
        does: "play the demo's loop by script and check every step happened (D69)",
        group: "Play and check",
        run: |_| run("node", &["tools/sandbox/loop.ts"]),
    },
    Task {
        name: "cabin",
        takes: "[--here] [--until STEP]",
        does: "build a cabin from plan to door by script, fit a door and shutters, open and shut them, walk in, and check every step by state (D69, D139, D201)",
        group: "Play and check",
        run: |args| {
            let mut all = vec!["tools/sandbox/cabin.ts"];
            all.extend(args.iter().map(String::as_str));
            run("node", &all)
        },
    },
    Task {
        name: "demo",
        takes: "[--skip-cabin] [--hour 15] [--out shots/demo]",
        does: "the construction demo's pictures: the cabin built by script, then the village and the cabin from fixed views (tools/sandbox)",
        group: "Play and check",
        run: |args| {
            let mut all = vec!["tools/sandbox/demo.ts"];
            all.extend(args.iter().map(String::as_str));
            run("node", &all)
        },
    },
    Task {
        name: "vantage",
        takes: "[--place lake] [--time 13,17] [--date 10-15]",
        does: "the vantage sheet: the lush forest from fixed views, side by side, to judge how the world looks (tools/sandbox)",
        group: "Play and check",
        run: |args| {
            let mut all = vec!["tools/sandbox/vantage.ts"];
            all.extend(args.iter().map(String::as_str));
            run("node", &all)
        },
    },
    Task {
        name: "looks",
        takes: "[--place lush|lake|tundra|town|all] [--time 13] [--from DIR]",
        does: "the looks check (D356): each place's views shot and held to the art style's rules, each number against its reference, pass or fail, and the view furthest out (docs/art-style.md#places)",
        group: "Play and check",
        run: |args| node("tools/sandbox/looks.ts", args),
    },
    Task {
        name: "foliage",
        takes: "[--card <name>] [--draw <dir>]",
        does: "the foliage check (D368): every leaf, needle and frond card the plants wear measured from its atlas and normal map, leaves a card, texels a leaf, relief, tilt, detail, colour, held to what reads as leaves; --draw shows the leaves it counted (docs/art-style.md#places)",
        group: "Play and check",
        run: |args| node("tools/textures/foliage.ts", args),
    },
    Task {
        name: "collide",
        takes: "[--count 20]",
        does: "walk the player into the lush forest's rocks, boulders, stumps, logs and trees, and check against their meshes that each stops them where it is drawn, or is stepped over if low (tools/sandbox)",
        group: "Play and check",
        run: |args| {
            let mut all = vec!["tools/sandbox/collide.ts"];
            all.extend(args.iter().map(String::as_str));
            run("node", &all)
        },
    },
    Task {
        name: "lodcheck",
        takes: "[--views floor,path,clearing,village,town] [--time 17] [--limit 0.5]",
        does: "find what pops as detail changes: still views while every switch distance is swept; fails past the limit (tools/sandbox)",
        group: "Play and check",
        run: |args| {
            let mut all = vec!["tools/sandbox/lodcheck.ts"];
            all.extend(args.iter().map(String::as_str));
            run("node", &all)
        },
    },
    Task {
        name: "pops",
        takes: "[--frames 1200-1320] [-- sandbox args]",
        does: "find what pops in or out, or flickers, in a stretch of a scripted walk, frame by frame (tools/sandbox)",
        group: "Play and check",
        run: |args| {
            let mut all = vec!["tools/sandbox/pops.ts"];
            all.extend(args.iter().map(String::as_str));
            run("node", &all)
        },
    },
    Task {
        name: "film",
        takes: "[--place lake] [--frames 300-2700] [--time 15] [--date 10-15] [--out shots/film.mp4] [-- sandbox args]",
        does: "a video of a scripted run, by default the forest walk, at the game's own speed (tools/sandbox)",
        group: "Look at things",
        run: |args| {
            let mut all = vec!["tools/sandbox/film.ts"];
            all.extend(args.iter().map(String::as_str));
            run("node", &all)
        },
    },
    Task {
        name: "trailer",
        takes: "[--only lake,forest] [--again] [--stills] [--no-render] [--size 1920x1080]",
        does: "the trailer: a run through each place filmed with a cinematic look, cut with crossfades and the game's own sound, in ~/.cache/terra-review/trailer/ (docs/trailer.md)",
        group: "Look at things",
        run: |args| {
            let mut all = vec!["tools/sandbox/trailer.ts"];
            all.extend(args.iter().map(String::as_str));
            run("node", &all)
        },
    },
    Task {
        name: "qa",
        takes: "clips|<scenario> [--set <kit>|all] [--part <stage>] [--no-trace] [--runs 3] | --list | --index",
        does: "QA clips of building and destruction in a set, the same every time, each with a real-time frame trace against the budget, in shots/qa/ (tools/sandbox)",
        group: "Look at things",
        run: |args| {
            let mut all = vec!["tools/sandbox/qa.ts"];
            all.extend(args.iter().map(String::as_str));
            run("node", &all)
        },
    },
    Task {
        name: "bench",
        takes: "[scene…] [--runs N] [--record] [--debug] [--copy NAME]",
        does: "time the benchmark scenes: release build, offscreen at 1920x1080, against the last recorded; with every scene, also lodcheck",
        group: "Play and check",
        run: bench,
    },
    Task {
        name: "check",
        takes: "[--quick] [--verbose]",
        does: "what every change must pass, run by git before each push: formatting, lints, every test, the kit, foliage, artifact, docs and dependency checks; --quick, run before each commit, leaves out the slow sweeps, the artifacts ratchet and the Blender scripts",
        group: "Play and check",
        run: check,
    },
    Task {
        name: "kits",
        takes: "",
        does: "check every building kit file: the materials it needs, sizes and the style guide's budgets",
        group: "Play and check",
        run: |_| kits(),
    },
    Task {
        name: "contract",
        takes: "",
        does: "check every model the game places against the model contract: size, anchor, axes, sockets and, for doors and shutters, fit in their hole",
        group: "Play and check",
        run: |_| contract(),
    },
    Task {
        name: "rules",
        takes: "[--pure] [--here] [--debug]",
        does: "audit building: its rules by test and live, the village's drawing, edits' and frames' timings (research/building-audit.md)",
        group: "Play and check",
        run: rules,
    },
    Task {
        name: "sections",
        takes: "[building]",
        does: "time laying and merging the village's sections, as the game's workers do, against the 2 ms bar, and the block kinds' near triangles and MiB against their budget (D284)",
        group: "Play and check",
        run: sections,
    },
    Task {
        name: "artifacts",
        takes: "[building] [--all] [--quick] [--ratchet|--record] | --frames [--quality high,low] [--checks still,sun,glide,going-up,undamaged,damaged]",
        does: "find what would draw as junk in building's meshes: z-fighting, sky through walls, gaps under roofs, pieces poking out; --frames, what only frames show, on the GPU: shimmer, acne, flashes during an edit, TAA trails, light probes popping",
        group: "Play and check",
        run: artifacts,
    },
    Task {
        name: "fuzz",
        takes: "[--seeds N] [--from S] [--ratchet|--record]",
        does: "build small bases at random from seeds, as a player builds, and find junk in their meshes as artifacts does; each base with a fault shrunk to its fewest elements and printed as a test to paste. The full check holds seeds 0 to 200 to a baseline (--ratchet)",
        group: "Play and check",
        run: fuzz,
    },
    Task {
        name: "texels",
        takes: "[building] [--worst N]",
        does: "find where building's textures are stretched or smeared, from the meshes: each kit's share of area drawn past 1.5:1 and 2.5:1, and where the worst is",
        group: "Play and check",
        run: texels,
    },
    Task {
        name: "look",
        takes: "<material>… | --all",
        does: "the material bench (D386): a material shot from fixed vantages at noon, in shade and in the evening beside its reference photo and an 18% grey card, its texel density, sharpness, grey-card balance, value range and stretch against its pattern's targets; fails on a regression; shots/look/ (tools/kits)",
        group: "Play and check",
        run: |args| node("tools/kits/look.ts", args),
    },
    Task {
        name: "town",
        takes: "[--seed N | --seeds A-B] [--ground flat|hillside|rolling|all] [--out shots/town.svg]",
        does: "a market town's plan from its seed, drawn as an SVG: square, streets, buildings, yards, the things its day is done at and its path graph; CPU only (terra_build::town)",
        group: "Look at things",
        run: |args| cargo_example("terra_build", "town", args, false),
    },
    Task {
        name: "life",
        takes: "[--seed N | --seeds A-B] [--ground flat|hillside|rolling|all] [--from HOUR] [--hours N]",
        does: "the life check (D326): a market town's people through a game day on the CPU, failing on anyone stuck, in a wall, inside another, sliding their feet or late for their day; counts by hour and a map in shots/life-*.svg (terra_life)",
        group: "Play and check",
        run: |args| cargo_example("terra_life", "life", args, true),
    },
    Task {
        name: "fields",
        takes: "[--radius 30000] [--at E,N] [--size 8000] [--cell 32] [--date 06-20] [--site lake|tundra|town[@E,N]] [--out shots/fields]",
        does: "the field maps: a region of the world's land on a date as maps of its rules (height, slope, facing, the growing season and the tree line, snow, shelter) and their numbers, on the CPU, no GPU (D326)",
        group: "Look at things",
        run: |args| {
            let mut cargo = vec![
                "run",
                "-q",
                "--release",
                "-p",
                "terra_land",
                "--bin",
                "fields",
                "--",
            ];
            cargo.extend(args.iter().map(String::as_str));
            run("cargo", &cargo)
        },
    },
    Task {
        name: "sandbox",
        takes: "[args]",
        does: "the sandbox, the test environment; --list for its experiments (crates/terra_sandbox)",
        group: "Look at things",
        run: |args| cargo_run("terra_sandbox", &[], args),
    },
    Task {
        name: "workbench",
        takes: "[args]",
        does: "models in the game's renderer: --model, --prop Forge|all, --hold, --sheet",
        group: "Look at things",
        run: |args| cargo_run("terra_sandbox", &["--experiment", "workbench"], args),
    },
    Task {
        name: "remote",
        takes: "<command> [args]",
        does: "drive a running sandbox: start, warp, give, look, shot… (tools/sandbox)",
        group: "Look at things",
        run: |args| node("tools/sandbox/remote.ts", args),
    },
    Task {
        name: "models",
        takes: "[--port 8765]",
        does: "the model gallery in a browser: every model, its size, problems and a page each (tools/models)",
        group: "Look at things",
        run: |args| node("tools/models/serve.ts", args),
    },
    Task {
        name: "props",
        takes: "<step> [prop…]",
        does: "make props: concept, model, refine, all; fronts to see every model from the front (tools/props)",
        group: "Make art",
        run: |args| node("tools/props/props.ts", args),
    },
    Task {
        name: "textures",
        takes: "<step> [args]",
        does: "make materials: fetch from Poly Haven, material from an image, stack for the ground (tools/textures)",
        group: "Make art",
        run: |args| node("tools/textures/textures.ts", args),
    },
    Task {
        name: "pack",
        takes: "<name> --from <source> [--pattern block|frame|coursed|roof]",
        does: "make a construction material in one command: its pack from a CC0 scan, an image or a reference model, checked and credited, and its file (tools/kits, D290)",
        group: "Make art",
        run: |args| node("tools/kits/pack.ts", args),
    },
    Task {
        name: "tripo",
        takes: "<command> [args]",
        does: "Tripo on our Studio subscription: image, model, rig, animate, balance… (tools/tripo)",
        group: "Make art",
        run: |args| node("tools/tripo/tripo.ts", args),
    },
    Task {
        name: "bodies",
        takes: "",
        does: "rebuild every character asset the game loads from art/ (tools/characters)",
        group: "Make art",
        run: |_| run("tools/characters/build_bodies.sh", &[]),
    },
    Task {
        name: "blender",
        takes: "<script.py> [args]",
        does: "run a script in Blender, headless (tools/blender)",
        group: "Make art",
        run: |args| run_owned("tools/blender/run.sh", args),
    },
    Task {
        name: "icons",
        takes: "[args]",
        does: "bake every item's icon from its look into assets/icons (D129)",
        group: "Make art",
        run: icons,
    },
    Task {
        name: "paint-icons",
        takes: "<item…> | --unpainted",
        does: "paint item icons in the art style (tools/icons)",
        group: "Make art",
        run: |args| node("tools/icons/paint.ts", args),
    },
    Task {
        name: "doctor",
        takes: "",
        does: "what this machine is missing",
        group: "This machine",
        run: |_| doctor(),
    },
    Task {
        name: "win",
        takes: "-p <package> [--release] [--copy <name>] [-- args]",
        does: "under WSL, build for Windows and run it there, on the real GPU",
        group: "This machine",
        run: win,
    },
    Task {
        name: "seed",
        takes: "[--refresh]",
        does: "keep the shared seed build current with origin/main and link its compiled dependencies into this worktree, so its first build is quick; fast-forwards the main checkout when it is clean (D183)",
        group: "This machine",
        run: seed::seed,
    },
    Task {
        name: "worktrees",
        takes: "[--prune | --done]",
        does: "what each worktree holds; --prune clears away the ones agents are done with (a hook does it hourly); --done, once your work is pushed, deletes this worktree's build",
        group: "This machine",
        run: worktrees::worktrees,
    },
    Task {
        name: "decision",
        takes: "[D28 D198 …]",
        does: "the next free decision number, here and on origin/main after a fetch; given numbers, those decisions in full with their log lines (docs/decisions/, D389)",
        group: "This machine",
        run: |args| {
            let mut all = vec![if args.is_empty() { "--next" } else { "--show" }.to_string()];
            all.extend(args.iter().cloned());
            node("tools/docs/docs.ts", &all)
        },
    },
    Task {
        name: "tools",
        takes: "",
        does: "this list",
        group: "This machine",
        run: |_| {
            println!("{}", usage());
            true
        },
    },
];
/// Every task, by group, for `tools` and for a task that is not known.
fn usage() -> String {
    let mut text = String::from("usage: cargo xtask <task> [args]\n");
    let width = TASKS
        .iter()
        .map(|task| task.name.len() + task.takes.len() + 1)
        .max()
        .unwrap_or(0)
        .min(34);
    let mut group = "";
    for task in TASKS {
        if task.group != group {
            group = task.group;
            text.push_str(&format!("\n{group}\n"));
        }
        let call = format!("{} {}", task.name, task.takes);
        let call = call.trim_end();
        if call.len() > width {
            text.push_str(&format!("  {call}\n  {:width$}  {}\n", "", task.does));
        } else {
            text.push_str(&format!("  {call:width$}  {}\n", task.does));
        }
    }
    text
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let name = args.next().unwrap_or_default();
    let rest: Vec<String> = args.collect();
    use_hooks();
    let ok = match TASKS
        .iter()
        .find(|task| task.name == name || (name == "help" && task.name == "tools"))
    {
        Some(task) => (task.run)(&rest),
        None => {
            eprintln!("{}", usage());
            false
        }
    };
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Points git at the repository's hooks, so `cargo xtask check --quick` runs before every commit
/// and `cargo xtask check` before every push (D37).
/// Any task does it, so a new clone has it as soon as anyone runs one; worktrees share it.
fn use_hooks() {
    if output("git", &["config", "--get", "core.hooksPath"]).as_deref() != Some(".githooks") {
        let set = output("git", &["config", "core.hooksPath", ".githooks"]).is_some();
        if set {
            eprintln!(
                "git now runs the quick check before each commit and the full one before each push (.githooks)"
            );
        }
    }
}

/// A crate's example program, built quietly and run from the root.
fn cargo_example(package: &str, example: &str, args: &[String], release: bool) -> bool {
    let mut all = vec!["run", "-q", "-p", package, "--example", example];
    if release {
        all.insert(2, "--release");
    }
    all.push("--");
    all.extend(args.iter().map(String::as_str));
    run("cargo", &all)
}

/// A TypeScript tool, run by Node from the root.
fn node(script: &str, args: &[String]) -> bool {
    let mut all = vec![script.to_string()];
    all.extend(args.iter().cloned());
    run_owned("node", &all)
}

/// A package's program, with some arguments first.
fn cargo_run(package: &str, first: &[&str], args: &[String]) -> bool {
    let mut all = vec!["run", "-p", package, "--"];
    all.extend(first);
    all.extend(args.iter().map(String::as_str));
    run("cargo", &all)
}

fn run_owned(program: &str, args: &[String]) -> bool {
    run(
        program,
        &args.iter().map(String::as_str).collect::<Vec<_>>(),
    )
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

fn run(program: &str, args: &[&str]) -> bool {
    println!("\n> {program} {}", args.join(" "));
    Command::new(program)
        .args(args)
        .current_dir(root())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn output(program: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(program)
        .args(args)
        .current_dir(root())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Renders every item's icon and writes them into assets/icons. It opens a window
/// for the few seconds it takes; under WSL the CPU renderer is enough.
fn icons(args: &[String]) -> bool {
    let mut all = vec!["run", "-p", "terra_sandbox", "--", "--experiment", "icons"];
    all.extend(args.iter().map(String::as_str));
    run("cargo", &all)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_task_has_its_own_name() {
        // The first task of a name is the one that runs, so a second would never run.
        for task in TASKS {
            assert_eq!(
                TASKS.iter().filter(|other| other.name == task.name).count(),
                1,
                "{} twice",
                task.name
            );
        }
    }
}
