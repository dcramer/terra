//! `cargo xtask profile` (D576, docs/launcher.md "The session report"): builds the game with
//! the frame recorder (`hitch-log`), runs it on the real GPU, walked by script or played by
//! a person, and prints the head of the session report it writes as it exits: the load's
//! longest steps, the frames, the slowest of them with what held them, the costliest systems.
//! The whole report, about a hundred lines, is kept in the checkout's `logs/` for an agent.
//! The launcher's `Profiling: report` plays through the same path.

use crate::launcher::Build;
use crate::{Task, output, root};

/// The tasks this module runs (D410).
pub(crate) const TASKS: &[Task] = &[Task {
    name: "profile",
    takes: "[walk|play] [--seconds 60] [--build optimised|debug] [-- game args]",
    does: "what a session spends (D576): build with the frame recorder, walk play-walk's route by script for --seconds (walk, the default) or play until you quit (play) on the real GPU, and print the session report's head; the whole report lands in logs/",
    group: "Play and check",
    run: profile,
}];

/// The report's file in the folder the game runs in.
const REPORT: &str = "profile-report.txt";
/// How many reports of a kind `logs/` keeps.
const KEPT: usize = 10;
/// How many lines of each of the report's sections the task prints.
const HEAD_LINES: usize = 4;

fn profile(args: &[String]) -> bool {
    let usage =
        "`cargo xtask profile [walk|play] [--seconds 60] [--build optimised|debug] [-- game args]`";
    let (ours, extra) = match args.iter().position(|a| a == "--") {
        Some(at) => (&args[..at], &args[at + 1..]),
        None => (args, &[][..]),
    };
    let mut walk = true;
    let mut seconds = 60u32;
    let mut build = Build::Optimised;
    let mut ours = ours.iter();
    while let Some(arg) = ours.next() {
        match arg.as_str() {
            "walk" => walk = true,
            "play" => walk = false,
            "--seconds" => match ours.next().and_then(|s| s.parse().ok()) {
                Some(given) if given > 0 => seconds = given,
                _ => {
                    eprintln!("✗ profile: --seconds takes a whole number of seconds. {usage}");
                    return false;
                }
            },
            "--build" => match ours.next().map(String::as_str) {
                Some("optimised" | "release") => build = Build::Optimised,
                Some("debug") => build = Build::Debug,
                other => {
                    eprintln!(
                        "✗ profile: --build is optimised or debug, not `{}`. {usage}",
                        other.unwrap_or("")
                    );
                    return false;
                }
            },
            other => {
                eprintln!(
                    "✗ profile: `{other}` is not one of profile's; the game's arguments go after `--`. {usage}"
                );
                return false;
            }
        }
    }
    let game = if walk {
        walk_args(seconds, extra)
    } else {
        let mut game: Vec<String> = vec!["--experiment".into(), "play".into()];
        game.extend(extra.iter().cloned());
        game
    };
    report(build, &game, if walk { "walk" } else { "play" })
}

/// The game's arguments for a scripted walk of `seconds`: `play-walk`'s new game and route
/// (D578), offscreen, timed from the start, a sixtieth of a second a frame.
pub(crate) fn walk_args(seconds: u32, extra: &[String]) -> Vec<String> {
    let scene = crate::scenes::SCENES
        .iter()
        .find(|scene| scene.name == "play-walk")
        .map_or(&[][..], |scene| scene.args);
    let mut game: Vec<String> = scene.iter().map(|arg| arg.to_string()).collect();
    game.extend(extra.iter().cloned());
    game.extend(["--offscreen", "--bench", "--settle", "0", "--bench-frames"].map(String::from));
    game.push((seconds * 60).to_string());
    game
}

/// Runs the game with the session report on, played when `kind` is `play` (its output in the
/// terminal and its log brought home) and otherwise walked quietly, then keeps the report in
/// `logs/` and prints its head and path.
pub(crate) fn report(build: Build, game: &[String], kind: &str) -> bool {
    let played = kind == "play";
    let mut args = game.to_vec();
    args.push(format!("--profile-report={REPORT}"));
    let game = crate::windows::Game {
        package: "terra_sandbox",
        profile: build.profile(),
        features: Some("hitch-log"),
        copy: Some("profile"),
        args: &args,
        output: (!played).then_some("profile-output.log"),
        played,
    };
    let Some((ok, folder)) = game.run() else {
        return false;
    };
    let Ok(text) = std::fs::read_to_string(folder.join(REPORT)) else {
        eprintln!(
            "✗ profile: the game wrote no report at {}; its output is {}.",
            folder.join(REPORT).display(),
            if played {
                "above".to_string()
            } else {
                folder.join("profile-output.log").display().to_string()
            }
        );
        return false;
    };
    let _ = std::fs::remove_file(folder.join(REPORT));
    let stamp = output("date", &["+%Y%m%d-%H%M%S"]).unwrap_or_default();
    let logs = root().join("logs");
    let name = format!("{stamp}-profile-{kind}.txt");
    let kept =
        std::fs::create_dir_all(&logs).and_then(|()| std::fs::write(logs.join(&name), &text));
    println!("{}", head(&text));
    match kept {
        Ok(()) => {
            let names: Vec<String> = std::fs::read_dir(&logs)
                .map(|entries| {
                    (entries.flatten())
                        .filter_map(|entry| entry.file_name().into_string().ok())
                        .collect()
                })
                .unwrap_or_default();
            for old in crate::session::older(&names, &name, KEPT) {
                let _ = std::fs::remove_file(logs.join(old));
            }
            println!("Report: {}", logs.join(&name).display());
        }
        Err(why) => eprintln!("Could not keep the report in {} ({why}).", logs.display()),
    }
    if !ok && !played {
        eprintln!(
            "✗ profile: the game ended with an error; its output is {}.",
            folder.join("profile-output.log").display()
        );
    }
    ok
}

/// The report's head: its first line, and the first lines of each section after.
pub(crate) fn head(text: &str) -> String {
    let mut lines = Vec::new();
    for (at, section) in text.split("\n\n").enumerate() {
        let keep = if at == 0 { 1 } else { HEAD_LINES };
        lines.extend(section.lines().take(keep));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_head_keeps_each_sections_first_lines() {
        let text = "Terra session report: a GPU\nArguments: x\n\nLoad: 1 s\n a\n b\n c\n d\n\nPlaying: 2 frames\n x\n";
        assert_eq!(
            head(text),
            "Terra session report: a GPU\nLoad: 1 s\n a\n b\n c\nPlaying: 2 frames\n x"
        );
    }

    #[test]
    fn a_walk_is_play_walks_route_timed_from_the_start_for_its_seconds() {
        let game = walk_args(30, &["--quality".into(), "low".into()]);
        let joined = game.join(" ");
        assert!(
            joined.starts_with("--experiment play --time 17 --route"),
            "{joined}"
        );
        assert!(
            joined.ends_with("--quality low --offscreen --bench --settle 0 --bench-frames 1800"),
            "{joined}"
        );
    }
}
