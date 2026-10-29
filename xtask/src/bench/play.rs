//! The play scenes (D578, docs/launcher.md "The play benchmark"): a scene named `play-…` is
//! timed from the program's start rather than after a settle, so the load and the first
//! minute's streaming, where a player sees freezes, are measured. Its load is the `Loaded in`
//! line's wall clock to the first playable frame; every frame of the walk after it is timed,
//! and a freeze (a frame of 100 ms or more, `terra_world::recorder::FREEZE_MS`) fails it as a
//! hitch fails a settled scene. It is run warm, its textures cooked (D188).

use super::Timing;
use crate::scenes::Scene;

/// A frame this long is a freeze, as the game's recorder and the sandbox count one.
pub(super) const FREEZE_MS: f32 = 100.0;

/// Whether a scene is a play scene, timed from the program's start.
pub(super) fn from_start(scene: &Scene) -> bool {
    scene.name.starts_with("play-")
}

/// Reads `Loaded in 15.0 s (8.2 s of it on the loading screen's frames): …`, a log line
/// (`terra_world::loading`) wherever it sits in the line, as the load's seconds.
pub(super) fn parse_load(line: &str) -> Option<f32> {
    let at = line.find("Loaded in ")? + "Loaded in ".len();
    line[at..].split_once(" s")?.0.parse().ok()
}

/// Whether the textures the game reads are cooked: a play scene timed with none read every
/// picture as made, mipmapped on the CPU (round 1's 5.1 s of the load), so its numbers mean
/// nothing. Says why when they are not.
pub(super) fn cooked_present() -> bool {
    let folder = crate::windows::cooked_folder();
    let any = folder
        .as_ref()
        .and_then(|folder| std::fs::read_dir(folder).ok())
        .is_some_and(|mut files| files.next().is_some());
    if !any {
        let named = folder.map_or("the cook cache".to_string(), |f| f.display().to_string());
        eprintln!(
            "✗ bench: a play scene is timed warm, with its textures cooked (D188, D578), and {named} \
             is empty, so the game would mipmap every picture on the CPU. Run `cargo xtask cook` \
             (under WSL, `cargo xtask win` mirrors it to Windows) and bench again."
        );
    }
    any
}

/// The last recorded load of a scene on the same GPU, size and build, and its commit.
fn last_load(csv: &str, timing: &Timing, build: &str, scene: &str) -> Option<(f32, String)> {
    csv.lines()
        .skip(1)
        .filter_map(|line| {
            let cells: Vec<&str> = line.split(',').collect();
            let [_, commit, gpu, size, built, named, ..] = cells[..] else {
                return None;
            };
            let load = cells.get(13)?.parse().ok()?;
            (gpu == timing.gpu && size == timing.size && built == build && named == scene)
                .then(|| (load, commit.to_string()))
        })
        .last()
}

/// A play scene's second line in the table: its load against the last recorded, SLOWER past
/// `slower_by` as a median is, and its freezes.
pub(super) fn said(csv: &str, timing: &Timing, build: &str, scene: &str, slower_by: f32) -> String {
    let load = match (timing.load_s, last_load(csv, timing, build, scene)) {
        (None, _) => "the load not said".to_string(),
        (Some(load), None) => format!("loaded in {load:.1} s, nothing recorded"),
        (Some(load), Some((before, then))) => {
            let change = load / before - 1.0;
            let note = if change > slower_by { "  SLOWER" } else { "" };
            format!(
                "loaded in {load:.1} s, {:+.0}% on {then}{note}",
                change * 100.0
            )
        }
    };
    format!(
        "{load}; {} freezes of {FREEZE_MS:.0} ms or more",
        timing.freezes
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_load_from_its_log_line() {
        let line = "02:26:10.230 f1013 INFO terra_world::loading: Loaded in 25.3 s (20.3 s of it \
                    on the loading screen's frames): Preparing shaders 141 of 141";
        assert_eq!(parse_load(line), Some(25.3));
        assert_eq!(parse_load("Loaded in a while"), None);
        assert_eq!(parse_load("Builds streamed in: 17 ms"), None);
    }

    #[test]
    fn only_a_play_scene_is_timed_from_the_start() {
        let named = |name| Scene {
            name,
            about: "",
            args: &[],
            frames: "600",
        };
        assert!(from_start(&named("play-walk")));
        assert!(!from_start(&named("world")));
    }

    #[test]
    fn the_load_is_compared_with_the_last_recorded_on_the_same_gpu_and_build() {
        let csv = "header\n\
            2026-10-29,aaa,RTX,1920x1080 offscreen,release,play-walk,10,12,40,3,0,1,0,15.0,0\n\
            2026-10-29,bbb,RTX,1920x1080 offscreen,debug,play-walk,10,12,40,3,0,1,0,30.0,0\n\
            2026-10-29,ccc,RTX,1920x1080 offscreen,release,world,10,12,40,3,0,1,0\n";
        let mut timing = super::super::bench_tests::timing(10.0, 12.0, 40.0);
        (timing.gpu, timing.load_s, timing.freezes) = ("RTX".into(), Some(18.0), 2);
        let line = said(csv, &timing, "release", "play-walk", 0.1);
        assert_eq!(
            line,
            "loaded in 18.0 s, +20% on aaa  SLOWER; 2 freezes of 100 ms or more"
        );
        timing.load_s = Some(15.5);
        assert!(!said(csv, &timing, "release", "play-walk", 0.1).contains("SLOWER"));
        assert!(said(csv, &timing, "release", "world", 0.1).contains("nothing recorded"));
    }
}
