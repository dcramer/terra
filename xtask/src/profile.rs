//! `cargo xtask profile` (D576, docs/launcher.md "The session report"): builds the game with
//! the frame recorder (`hitch-log`), runs it on the real GPU, walked by script or played by
//! a person, and prints the head of the session report it writes as it exits: the load's
//! longest steps, the frames, the slowest of them with what held them, the costliest systems.
//! The whole report, about a hundred lines, is kept in the checkout's `logs/` for an agent.
//! The launcher's `Profiling: report` plays through the same path. With `--tracy` (D577) the
//! build is a Tracy build: a walk is also captured by `tracy-capture` and its costliest zones
//! printed; a game played opens Tracy's profiler on it, live (the launcher's `Profiling: live`).

use crate::launcher::Build;
use crate::tracy::Mode;
use crate::{Task, output, root};
use std::path::Path;

/// The tasks this module runs (D410).
pub(crate) const TASKS: &[Task] = &[Task {
    name: "profile",
    takes: "[walk|play] [--seconds 60] [--build optimised|debug] [--tracy] [-- game args]",
    does: "what a session spends (D576): build with the frame recorder, walk play-walk's route by script for --seconds (walk, the default) or play until you quit (play) on the real GPU, and print the session report's head; the whole report lands in logs/. --tracy (D577): a Tracy build; a walk also keeps a capped Tracy capture in logs/ and prints its costliest zones, a game played opens Tracy's profiler live",
    group: "Play and check",
    run: profile,
}];

/// The report's file in the folder the game runs in.
const REPORT: &str = "profile-report.txt";
/// How many reports of a kind `logs/` keeps.
const KEPT: usize = 10;
/// How many lines of each of the report's sections the task prints.
const HEAD_LINES: usize = 4;
/// How many of the game's last lines of output a failure prints.
const OUTPUT_LINES: usize = 12;

fn profile(args: &[String]) -> bool {
    let usage = "`cargo xtask profile [walk|play] [--seconds 60] [--build optimised|debug] [--tracy] [-- game args]`";
    let (ours, extra) = match args.iter().position(|a| a == "--") {
        Some(at) => (&args[..at], &args[at + 1..]),
        None => (args, &[][..]),
    };
    let mut walk = true;
    let mut seconds = 60u32;
    let mut build = Build::Optimised;
    let mut tracy = false;
    let mut ours = ours.iter();
    while let Some(arg) = ours.next() {
        match arg.as_str() {
            "--tracy" => tracy = true,
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
    let tracy = match (tracy, walk) {
        (false, _) => Mode::Off,
        (true, true) => Mode::Capture(seconds + crate::tracy::LOAD_BOUND_S),
        (true, false) => Mode::Live,
    };
    report(build, &game, if walk { "walk" } else { "play" }, tracy).0
}

/// The game's arguments for a scripted walk: `play-walk`'s new game and route (D578),
/// offscreen, timed from the start, a sixtieth of a second of play a frame, ended by the
/// recorder `seconds` by the clock after the load, however fast frames are drawn; the
/// bench's frames, at a thousand a second, only bound it.
pub(crate) fn walk_args(seconds: u32, extra: &[String]) -> Vec<String> {
    let scene = crate::scenes::SCENES
        .iter()
        .find(|scene| scene.name == "play-walk")
        .map_or(&[][..], |scene| scene.args);
    let mut game: Vec<String> = scene.iter().map(|arg| arg.to_string()).collect();
    game.extend(extra.iter().cloned());
    game.extend(["--offscreen", "--bench", "--settle", "0", "--bench-frames"].map(String::from));
    game.push((seconds * 1000).to_string());
    game.push(format!("--profile-seconds={seconds}"));
    game
}

/// Runs the game with the session report on, played when `kind` is `play` (its output in the
/// terminal and its log brought home) and otherwise walked quietly, then keeps the report in
/// `logs/` and prints its head and path. With Tracy (`tracy`), a Tracy build, its profiler
/// opened first or a capture taken beside it (`crate::tracy`). Gives whether all went well,
/// and the game's exit code if it failed (the launcher starts it again on some, D579).
pub(crate) fn report(
    build: Build,
    game: &[String],
    kind: &str,
    tracy: Mode,
) -> (bool, Option<i32>) {
    let played = kind == "play";
    let mut args = game.to_vec();
    args.push(format!("--profile-report={REPORT}"));
    let tools = match tracy {
        Mode::Off => None,
        Mode::Live | Mode::Capture(_) => match crate::tracy::fetch() {
            Some(folder) => Some(folder),
            None => return (false, None),
        },
    };
    let mut capture = None;
    if let Some(folder) = &tools {
        match tracy {
            Mode::Live => crate::tracy::open_profiler(folder),
            Mode::Capture(seconds) => {
                capture = crate::tracy::Capture::start(folder, seconds, kind);
                if capture.is_none() {
                    return (false, None);
                }
            }
            Mode::Off => {}
        }
    }
    let game = crate::windows::Game {
        package: "terra_sandbox",
        profile: build.profile(),
        features: Some(if tools.is_some() {
            "tracy"
        } else {
            "hitch-log"
        }),
        copy: Some(if tools.is_some() { "tracy" } else { "profile" }),
        args: &args,
        output: (!played).then_some("profile-output.log"),
        played,
    };
    let Some((ended, folder)) = game.run() else {
        if let Some(capture) = capture {
            capture.abandon();
        }
        return (false, None);
    };
    let ok = ended.ok();
    let captured = capture.is_none_or(crate::tracy::Capture::finish);
    let Ok(text) = std::fs::read_to_string(folder.join(REPORT)) else {
        let output = folder.join("profile-output.log");
        let said = if played {
            None
        } else {
            Some(std::fs::read_to_string(&output).unwrap_or_default())
        };
        eprintln!("{}", no_report(ended, said.as_deref(), &output));
        return (false, ended.code());
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
    (ok && captured, ended.code())
}

/// What to say when the game wrote no report (D556): how it ended, and its output's last
/// lines (`said`, `None` when it went to the terminal above), or that it said nothing, which
/// is a program that never started (a DLL missing, as libstdc++ was in perf round 10).
fn no_report(ended: crate::windows::Ended, said: Option<&str>, output: &Path) -> String {
    let mut text = format!(
        "✗ profile: the game {} without writing its session report.",
        ended.words()
    );
    let Some(said) = said else {
        text.push_str(" Its output is above.");
        return text;
    };
    let lines: Vec<&str> = said
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    if lines.is_empty() {
        text.push_str(&format!(
            " It printed nothing ({}), so it likely never started: a DLL it needs is missing, or it was stopped at once. Open the program in {} by hand to see Windows' own error, and run `cargo xtask doctor`.",
            output.display(),
            output.parent().unwrap_or(output).display()
        ));
        return text;
    }
    let from = lines.len().saturating_sub(OUTPUT_LINES);
    let last: Vec<String> = lines[from..]
        .iter()
        .map(|line| format!("  {line}"))
        .collect();
    text.push_str(&format!(
        " Its last lines, from {}:\n{}",
        output.display(),
        last.join("\n")
    ));
    text
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
    fn a_missing_report_names_how_the_game_ended_and_its_last_lines() {
        use crate::windows::Ended;
        let output = Path::new("/tmp/run/profile-output.log");
        let said: String = (1..=20).map(|n| format!("line {n}\n\n")).collect();
        let text = no_report(Ended::Failed(Some(101)), Some(&said), output);
        assert!(text.contains("code 101, a Rust panic"), "{text}");
        assert!(
            text.contains("  line 20") && text.contains("  line 9\n"),
            "{text}"
        );
        assert!(!text.contains("line 8\n"), "{text}");
        let silent = no_report(Ended::Failed(Some(53)), Some(" \n"), output);
        assert!(
            silent.contains("code 53") && silent.contains("never started"),
            "{silent}"
        );
        assert!(no_report(Ended::Ok, None, output).ends_with("above."));
    }

    #[test]
    fn the_head_keeps_each_sections_first_lines() {
        let text = "Terra session report: a GPU\nArguments: x\n\nLoad: 1 s\n a\n b\n c\n d\n\nPlaying: 2 frames\n x\n";
        assert_eq!(
            head(text),
            "Terra session report: a GPU\nLoad: 1 s\n a\n b\n c\nPlaying: 2 frames\n x"
        );
    }

    #[test]
    fn a_walk_is_play_walks_route_ended_by_the_clock_after_its_seconds() {
        let game = walk_args(30, &["--quality".into(), "low".into()]);
        let joined = game.join(" ");
        assert!(
            joined.starts_with("--experiment play --time 17 --route"),
            "{joined}"
        );
        assert!(
            joined.ends_with(
                "--quality low --offscreen --bench --settle 0 --bench-frames 30000 --profile-seconds=30"
            ),
            "{joined}"
        );
    }
}
