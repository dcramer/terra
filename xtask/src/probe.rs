//! `cargo xtask probe` (D609): what a change costs on the GPU, its variants measured A B A B
//! in one load of the optimised sandbox on settled frames only, the clock held; the game's
//! `--probe` does the measuring (crates/terra_world/src/debug/probe.rs). A lever is a
//! tunable (D547) or a hide, never an option of its own (D249). A bench (D185) still
//! decides whether a change lands; the probe says where its time goes.

use crate::bench::sandbox_program;
use crate::scenes::SCENES;
use crate::windows::on_wsl;
use crate::{Task, root};
use std::io::{BufRead, BufReader, Write as _};
use std::process::{Command, Stdio};

/// The task this module runs (D410).
pub(crate) const TASKS: &[Task] = &[Task {
    name: "probe",
    takes: "\"<name>: <lever>, …\"… [--scene <bench scene>] [-- sandbox args]",
    does: "what a change costs on the GPU (D609): the variants A B A B in one load, the clock held, counting only frames with every pipeline built and everything drawn; a lever is tune.name=value or hide=<material part>; prints each phase with the GPU's clocks, a table a round, and fails on a lever matching nothing. probe.frames and probe.rounds are tunables (--tune)",
    group: "Play and check",
    run: probe,
}];

/// Frames the bench would time, past any probe's end: the probe ends the run itself.
const ENDLESS: &str = "1000000";

fn probe(args: &[String]) -> bool {
    let usage = "`cargo xtask probe \"old: fog.light_span=0\" \"new: fog.light_span=1\" [--scene lake-stand] [-- --experiment play --vantage …]`, a variant an argument (D609)";
    let (ours, extra) = match args.iter().position(|a| a == "--") {
        Some(at) => (&args[..at], &args[at + 1..]),
        None => (args, &[][..]),
    };
    let mut variants: Vec<&str> = Vec::new();
    let mut scene = None;
    let mut words = ours.iter();
    while let Some(word) = words.next() {
        match word.as_str() {
            "--scene" => match words
                .next()
                .and_then(|n| SCENES.iter().find(|s| s.name == n))
            {
                Some(found) => scene = Some(found),
                None => {
                    let names: Vec<&str> = SCENES.iter().map(|s| s.name).collect();
                    eprintln!(
                        "--scene takes one of the bench's scenes: {}",
                        names.join(", ")
                    );
                    return false;
                }
            },
            text if !text.starts_with("--") => variants.push(text),
            other => {
                eprintln!("probe does not take `{other}`: {usage}");
                return false;
            }
        }
    }
    if variants.is_empty() {
        eprintln!("probe needs its variants: {usage}");
        return false;
    }
    if scene.is_none() && extra.is_empty() {
        eprintln!(
            "probe needs a view to measure, a bench scene (--scene) or the sandbox's arguments \
             after `--`; a still view (--vantage) measures one thing, a walked one many: {usage}"
        );
        return false;
    }
    let Some((program, folder)) = sandbox_program(true, "probe") else {
        return false;
    };
    let mut command = Command::new(program);
    if let Some(scene) = scene {
        command.args(scene.args);
    }
    command
        .args(extra)
        // Held from the first frame, so the hour is the scene's (D609).
        .args(["--tune", "daylight.pace=0"])
        .args(variants.iter().flat_map(|v| ["--probe", v]))
        .args([
            "--offscreen",
            "--bench",
            "--settle",
            "0",
            "--bench-frames",
            ENDLESS,
        ])
        .current_dir(&folder);
    if !on_wsl() {
        command.env("BEVY_ASSET_ROOT", &folder);
    }
    run(command)
}

/// Runs the probe, printing its lines and keeping the rest in a log it names.
fn run(mut command: Command) -> bool {
    let logs = root().join("target").join("probe");
    std::fs::create_dir_all(&logs).ok();
    let log_path = logs.join("probe.log");
    let Ok(mut log) = std::fs::File::create(&log_path) else {
        eprintln!("could not write {}", log_path.display());
        return false;
    };
    let mut child = match command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            eprintln!("could not start the sandbox: {error}");
            return false;
        }
    };
    let stderr = child.stderr.take();
    let errors = std::thread::spawn(move || {
        stderr.map_or(Vec::new(), |e| {
            BufReader::new(e)
                .lines()
                .map_while(Result::ok)
                .collect::<Vec<String>>()
        })
    });
    let mut failed = false;
    if let Some(stdout) = child.stdout.take() {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            writeln!(log, "{line}").ok();
            if line.starts_with("PROBE") || line.starts_with("SANDBOX waiting for the GPU") {
                failed |= line.starts_with("PROBE ✗");
                println!("{line}");
            }
        }
    }
    let status = child.wait().ok();
    for line in errors.join().unwrap_or_default() {
        writeln!(log, "{line}").ok();
    }
    let passed = status.is_some_and(|s| s.success()) && !failed;
    if !passed {
        eprintln!(
            "✗ the probe failed (its reason follows `PROBE ✗` above, or the sandbox's last lines \
             are in {})",
            log_path.display()
        );
    } else {
        println!("probe's whole output: {}", log_path.display());
    }
    passed
}
