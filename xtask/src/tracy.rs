//! Tracy (D577, docs/launcher.md "Live: Tracy"): the profiler that shows every span on a
//! timeline, live, and keeps a capture to open later. The game's `tracy` feature (Bevy's
//! `trace_tracy`) builds its client in; this module fetches Tracy's own tools, at the version
//! that client speaks, into a folder of their own (never installed system-wide), and runs
//! them: the profiler beside a game someone plays, or a capture capped in length beside a
//! scripted walk, summarised a zone a line by `tracy-csvexport` for an agent.
//!
//! Tracy is BSD 3-clause (https://github.com/wolfpld/tracy); its client in the game, through
//! rust_tracy_client (MIT or Apache 2.0), is only in profiling builds, never one handed out.

use crate::windows::on_wsl;
use crate::{output, root};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

/// The Tracy the pinned Bevy's client speaks (`tracy-client-sys` 0.30 holds Tracy 0.14.1's
/// client). Client and profiler must match exactly: Tracy's NEWS promises nothing between
/// versions. A Bevy update that moves the client moves this, and the doctor says so (`matches`).
pub(crate) const VERSION: &str = "0.14.1";

/// The release's archives and the SHA-256 GitHub publishes for each (the release's asset
/// digests, https://api.github.com/repos/wolfpld/tracy/releases/tags/v0.14.1).
const ARCHIVES: &[(&str, &str)] = &[
    (
        "windows",
        "f7499d74914aa3ba94a2c1ce72f36477d7b61d9d0f7c9790e05274c258c97fb5",
    ),
    (
        "linux",
        "4f57574337b206cac86758081ab21c37cf9c83fc12170d2e339e1f4ee94ff590",
    ),
    (
        "macos",
        "971e2d8a742a57dc081cb2e36cc6200d565be7555c39a2f6439f9d65e2b416cb",
    ),
];

/// The game's port for Tracy (its default). The client listens on 127.0.0.1 alone
/// (`only-localhost`, terra_world's Cargo.toml).
const PORT: &str = "8086";

/// How long a capture may run past the walk, for the build's start and the load: its cap.
pub(crate) const LOAD_BOUND_S: u32 = 120;

/// What `profile` does with Tracy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    Off,
    /// The profiler opened beside a game someone plays.
    Live,
    /// A capture of at most these seconds from the game's start, summarised.
    Capture(u32),
}

/// Where the tools are kept: under WSL on the Windows side (`%LOCALAPPDATA%\terra\tools`),
/// where Windows programs run from, as the developer allowed (2026-10-29); elsewhere in the
/// cache (`~/.cache/terra/tools`).
pub(crate) fn folder() -> Option<PathBuf> {
    let tools = if on_wsl() {
        crate::windows::windows_folder("LOCALAPPDATA")?
            .join("terra")
            .join("tools")
    } else {
        crate::session::cache_folder().join("tools")
    };
    Some(tools.join(format!("tracy-{VERSION}")))
}

/// A tool's program in `folder`: `tracy-profiler`, `tracy-capture`, `tracy-csvexport`.
fn program(folder: &Path, name: &str) -> PathBuf {
    if on_wsl() {
        folder.join(format!("{name}.exe"))
    } else {
        folder.join(name)
    }
}

/// The release's archive for this machine.
fn archive() -> &'static str {
    if on_wsl() {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    }
}

/// Fetches Tracy's tools from its release on GitHub unless they are there, checked against
/// the release's SHA-256; gives their folder.
pub(crate) fn fetch() -> Option<PathBuf> {
    let Some(folder) = folder() else {
        eprintln!(
            "✗ tracy: could not find where to keep Tracy's tools (%LOCALAPPDATA% under WSL); is Windows interop on? (D577)"
        );
        return None;
    };
    if program(&folder, "tracy-capture").exists() {
        return Some(folder);
    }
    let system = archive();
    let name = format!("{system}-{VERSION}.zip");
    let url = format!("https://github.com/wolfpld/tracy/releases/download/v{VERSION}/{name}");
    let want = ARCHIVES
        .iter()
        .find(|(archive, _)| *archive == system)
        .map_or("", |(_, sum)| sum);
    println!(
        "tracy: fetching Tracy {VERSION}'s tools ({url}) into {}",
        folder.display()
    );
    let zip = folder.with_extension("zip");
    let fetched = std::fs::create_dir_all(&folder).is_ok()
        && Command::new("curl")
            .args(["-fsSL", "-o"])
            .arg(&zip)
            .arg(&url)
            .status()
            .is_ok_and(|s| s.success());
    let sum = fetched
        .then(|| output("sha256sum", &[&zip.to_string_lossy()]))
        .flatten()
        .and_then(|said| said.split_whitespace().next().map(str::to_string))
        .unwrap_or_default();
    let unpacked = fetched
        && sum == want
        && Command::new("unzip")
            .args(["-q", "-o"])
            .arg(&zip)
            .arg("-d")
            .arg(&folder)
            .status()
            .is_ok_and(|s| s.success());
    let _ = std::fs::remove_file(&zip);
    if fetched && sum != want {
        let _ = std::fs::remove_dir_all(&folder);
        eprintln!(
            "✗ tracy: {name}'s SHA-256 is {sum}, not the release's {want}; nothing was kept. Fetch again; if it repeats, the release changed: check https://github.com/wolfpld/tracy/releases (D577)."
        );
        return None;
    }
    if !unpacked || !program(&folder, "tracy-capture").exists() {
        eprintln!(
            "✗ tracy: fetching {url} into {} failed; check the network, curl and unzip (D577).",
            folder.display()
        );
        return None;
    }
    Some(folder)
}

/// The `tracy-client-sys` the lock file holds and the Tracy its bundled client is, read from
/// its source in Cargo's registry (`tracy/common/TracyVersion.hpp`).
fn client() -> Result<(String, String), String> {
    let lock = std::fs::read_to_string(root().join("Cargo.lock")).unwrap_or_default();
    let version = lock
        .split("[[package]]")
        .find(|package| package.contains("name = \"tracy-client-sys\""))
        .and_then(|package| {
            package
                .lines()
                .find_map(|line| line.strip_prefix("version = "))
        })
        .map(|version| version.trim_matches('"').to_string())
        .ok_or("no tracy-client-sys in Cargo.lock: terra_world's `tracy` feature (Bevy's `trace_tracy`) is gone or Bevy changed; see docs/launcher.md (D577)")?;
    let registry = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cargo")))
        .unwrap_or_default()
        .join("registry")
        .join("src");
    let header = std::fs::read_dir(&registry)
        .into_iter()
        .flatten()
        .flatten()
        .map(|index| {
            index
                .path()
                .join(format!("tracy-client-sys-{version}"))
                .join("tracy/common/TracyVersion.hpp")
        })
        .find_map(|header| std::fs::read_to_string(header).ok())
        .ok_or(format!(
            "tracy-client-sys {version}'s source is not in {}: `cargo fetch` brings it",
            registry.display()
        ))?;
    let tracy = header_version(&header).ok_or(format!(
        "tracy-client-sys {version}'s TracyVersion.hpp names no version this reads; read it and set VERSION in xtask/src/tracy.rs (D577)"
    ))?;
    Ok((version, tracy))
}

/// `Major.Minor.Patch` from Tracy's `TracyVersion.hpp`.
fn header_version(header: &str) -> Option<String> {
    let part = |name: &str| {
        header.lines().find_map(|line| {
            let (_, value) = line.split_once(&format!("int {name} = "))?;
            value.trim().trim_end_matches(';').parse::<u32>().ok()
        })
    };
    Some(format!(
        "{}.{}.{}",
        part("Major")?,
        part("Minor")?,
        part("Patch")?
    ))
}

/// The version the fetched tools report (`tracy-csvexport -V`), if they are fetched.
fn tools_version(folder: &Path) -> Option<String> {
    let out = Command::new(program(folder, "tracy-csvexport"))
        .arg("-V")
        .current_dir(folder)
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let said = String::from_utf8_lossy(&out.stdout);
    said.split_whitespace().nth(1).map(str::to_string)
}

/// For `doctor`: whether the client the game builds, this module's `VERSION` and the tools
/// fetched are one Tracy; `Err` says what to do.
pub(crate) fn matches() -> Result<String, String> {
    let (client, speaks) = client()?;
    if speaks != VERSION {
        return Err(format!(
            "the game's client (tracy-client-sys {client}) is Tracy {speaks}, but xtask fetches {VERSION}: set VERSION and ARCHIVES' checksums in xtask/src/tracy.rs to Tracy {speaks}'s release, once the developer allows that download (D577)"
        ));
    }
    let Some(folder) = folder().filter(|folder| program(folder, "tracy-capture").exists()) else {
        return Ok(format!(
            "the game's client (tracy-client-sys {client}) is Tracy {VERSION}; its tools are fetched by the first `cargo xtask profile --tracy`"
        ));
    };
    match tools_version(&folder) {
        Some(tools) if tools == VERSION => Ok(format!(
            "Tracy {VERSION}'s tools in {} match the game's client (tracy-client-sys {client})",
            folder.display()
        )),
        tools => Err(format!(
            "the tools in {} say Tracy {}, the game's client is {VERSION}: delete that folder and run `cargo xtask profile --tracy` to fetch {VERSION} (D577)",
            folder.display(),
            tools
                .as_deref()
                .unwrap_or("nothing (tracy-csvexport -V did not run)")
        )),
    }
}

/// A path as the tools take it: under WSL, Windows' own.
fn native(path: &Path) -> String {
    if on_wsl() {
        output("wslpath", &["-w", &path.to_string_lossy()])
            .unwrap_or_else(|| path.display().to_string())
    } else {
        path.display().to_string()
    }
}

/// Opens Tracy's profiler on the game's port; it connects as soon as the game starts and shows
/// every span as it runs. Started apart, so it stays open when the game and this task end.
pub(crate) fn open_profiler(folder: &Path) {
    let profiler = program(folder, "tracy-profiler");
    let started = if on_wsl() {
        Command::new("cmd.exe")
            .args(["/c", "start", "\"\""])
            .arg(native(&profiler))
            .args(["-a", "127.0.0.1", "-p", PORT])
            .current_dir(folder)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    } else {
        Command::new(&profiler)
            .args(["-a", "127.0.0.1", "-p", PORT])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .is_ok()
    };
    if started {
        println!(
            "tracy: Tracy's profiler is open and connects once the game starts (a Tracy build's first launch builds Bevy again, minutes)."
        );
    } else {
        eprintln!(
            "✗ tracy: could not start {}; the game plays without it. Start it by hand with `-a 127.0.0.1` (D577).",
            profiler.display()
        );
    }
}

/// A capture being taken: `tracy-capture`, started before the game, waits for it and records
/// from its start (the client keeps everything until something connects).
pub(crate) struct Capture {
    child: Child,
    folder: PathBuf,
    file: PathBuf,
    log: PathBuf,
}

impl Capture {
    /// Starts `tracy-capture`, capped at `seconds` from the game's start.
    pub(crate) fn start(folder: &Path, seconds: u32, kind: &str) -> Option<Capture> {
        let stamp = output("date", &["+%Y%m%d-%H%M%S"]).unwrap_or_default();
        let captures = folder.join("captures");
        let file = captures.join(format!("{stamp}-tracy-{kind}.tracy"));
        let log = root().join("logs").join("tracy-capture.log");
        let started = std::fs::create_dir_all(&captures)
            .and_then(|()| std::fs::create_dir_all(root().join("logs")))
            .and_then(|()| std::fs::File::create(&log))
            .and_then(|out| {
                Command::new(program(folder, "tracy-capture"))
                    .args([
                        "-a",
                        "127.0.0.1",
                        "-p",
                        PORT,
                        "-f",
                        "-s",
                        &seconds.to_string(),
                    ])
                    .arg("-o")
                    .arg(native(&file))
                    .current_dir(folder)
                    .stdout(out.try_clone()?)
                    .stderr(out)
                    .spawn()
            });
        match started {
            Ok(child) => Some(Capture {
                child,
                folder: folder.to_path_buf(),
                file,
                log,
            }),
            Err(why) => {
                eprintln!("✗ tracy: could not start tracy-capture ({why}) (D577).");
                None
            }
        }
    }

    /// Stops it unfinished, when the game never ran.
    pub(crate) fn abandon(mut self) {
        self.stop();
    }

    fn stop(&mut self) {
        if on_wsl() {
            let _ = output("taskkill.exe", &["/IM", "tracy-capture.exe", "/F"]);
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }

    /// Waits for the capture to be written, moves it to `logs/`, summarises its zones with
    /// `tracy-csvexport` into a `.csv` beside it and prints the costliest systems. The
    /// game has ended, so the capture has all it will get: one not saved in `SAVE_S` (the
    /// game never connected) is stopped.
    pub(crate) fn finish(mut self) -> bool {
        let ended = std::time::Instant::now();
        let done = loop {
            match self.child.try_wait() {
                Ok(Some(status)) => break status.success(),
                Ok(None) if ended.elapsed().as_secs() < SAVE_S => {
                    std::thread::sleep(std::time::Duration::from_millis(250));
                }
                _ => {
                    self.stop();
                    break false;
                }
            }
        };
        if !done || !self.file.exists() {
            eprintln!(
                "✗ tracy: no capture; tracy-capture's output is in {} (D577).",
                self.log.display()
            );
            return false;
        }
        let logs = root().join("logs");
        let kept = logs.join(self.file.file_name().unwrap_or_default());
        let moved = std::fs::copy(&self.file, &kept).and_then(|_| std::fs::remove_file(&self.file));
        if let Err(why) = moved {
            eprintln!(
                "✗ tracy: could not move the capture to {} ({why}).",
                kept.display()
            );
            return false;
        }
        let names: Vec<String> = std::fs::read_dir(&logs)
            .map(|entries| {
                (entries.flatten())
                    .filter_map(|entry| entry.file_name().into_string().ok())
                    .collect()
            })
            .unwrap_or_default();
        let name = kept
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        for old in crate::session::older(&names, &name, KEPT) {
            let _ = std::fs::remove_file(logs.join(&old));
            let _ = std::fs::remove_file(logs.join(old).with_extension("csv"));
        }
        let csv = kept.with_extension("csv");
        let zones = Command::new(program(&self.folder, "tracy-csvexport"))
            .args(["-s", SEPARATOR])
            .arg(native(&kept))
            .current_dir(&self.folder)
            .stderr(Stdio::null())
            .output()
            .ok()
            .filter(|out| out.status.success())
            .map(|out| String::from_utf8_lossy(&out.stdout).into_owned());
        let size = kept.metadata().map_or(0, |m| m.len()) as f64 / 1e6;
        let Some(zones) = zones else {
            eprintln!(
                "✗ tracy: tracy-csvexport could not read {} ({size:.0} MB); open it in Tracy's profiler (D577).",
                kept.display()
            );
            return false;
        };
        let _ = std::fs::write(&csv, &zones);
        println!("\n{}", costliest(&zones, SHOWN));
        println!(
            "Capture: {} ({size:.0} MB; open it in {}), every zone: {}",
            kept.display(),
            native(&program(&self.folder, "tracy-profiler")),
            csv.display()
        );
        true
    }
}

/// How long a capture may take to save once the game has ended.
const SAVE_S: u64 = 180;
/// How many captures `logs/` keeps: each is tens to hundreds of megabytes.
const KEPT: usize = 3;
/// How many zones the summary names in each list.
const SHOWN: usize = 12;
/// `tracy-csvexport`'s separator: Rust's names hold commas (`f<A, B>`) and semicolons (`[u8; 4]`).
const SEPARATOR: &str = "|";

/// A system of the CSV: its name, time in total, runs, longest run (milliseconds).
struct Zone {
    name: String,
    total_ms: f64,
    runs: u64,
    longest_ms: f64,
}

/// The costliest systems of `tracy-csvexport`'s CSV (name, src_file, src_line, total_ns,
/// total_perc, counts, mean_ns, min_ns, max_ns, std_ns), by total and by longest run, a line
/// each, named as the session report names them. Systems only, by their whole time, as the
/// report counts them: Tracy's own time would charge a schedule for the time its systems run
/// on other threads, and take from a system the render work it opens spans for.
pub(crate) fn costliest(csv: &str, top: usize) -> String {
    let mut lines = csv.lines();
    let header: Vec<&str> = lines.next().unwrap_or_default().split(SEPARATOR).collect();
    let column = |name: &str| header.iter().position(|h| *h == name);
    let (Some(name), Some(total), Some(count), Some(max)) = (
        column("name"),
        column("total_ns"),
        column("counts"),
        column("max_ns"),
    ) else {
        return "tracy-csvexport's columns were not the ones known; read the CSV.".into();
    };
    let mut zones: Vec<Zone> = lines
        .filter_map(|line| {
            let fields: Vec<&str> = line.split(SEPARATOR).collect();
            Some(Zone {
                name: system(fields.get(name)?)?,
                total_ms: fields.get(total)?.parse::<f64>().ok()? / 1e6,
                runs: fields.get(count)?.parse().ok()?,
                longest_ms: fields.get(max)?.parse::<f64>().ok()? / 1e6,
            })
        })
        .collect();
    let mut out = vec![format!(
        "Tracy: systems by time in total, the load included; one that runs a schedule (run_render_schedule) counts the systems it runs, and every zone is in the CSV:\n{:>11} {:>9} {:>11}  system",
        "total", "runs", "longest"
    )];
    let line = |zone: &Zone| {
        format!(
            "{:>8.0} ms {:>9} {:>8.2} ms  {}",
            zone.total_ms, zone.runs, zone.longest_ms, zone.name
        )
    };
    zones.sort_by(|a, b| b.total_ms.total_cmp(&a.total_ms));
    out.extend(zones.iter().take(top).map(line));
    out.push("Tracy: systems by their longest single run:".into());
    zones.sort_by(|a, b| b.longest_ms.total_cmp(&a.longest_ms));
    out.extend(zones.iter().take(top).map(line));
    out.join("\n")
}

/// A system's name as the session report shows it, if the zone is one of Bevy's system spans:
/// `system{name="a::b"}` as `a::b`, `system_commands{…}` with ` (commands)`.
fn system(zone: &str) -> Option<String> {
    let (span, rest) = zone.split_once('{')?;
    let name = rest
        .strip_suffix('}')?
        .strip_prefix("name=")?
        .trim_matches('"');
    match span {
        "system" => Some(name.to_string()),
        "system_commands" => Some(format!("{name} (commands)")),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_costliest_systems_come_first_by_total_and_by_longest_run() {
        let csv = "name|src_file|src_line|total_ns|total_perc|counts|mean_ns|min_ns|max_ns|std_ns\n\
            system{name=\"quick\"}|a.rs|1|1000000|1|10|100000|1|900000|1\n\
            system{name=\"x::f<A, B>\"}|b.rs|2|50000000|50|5|10000000|1|30000000|1\n\
            system_commands{name=\"spiky\"}|b.rs|2|20000000|20|5|4000000|1|40000000|1\n\
            schedule{name=Update}|c.rs|3|90000000|90|5|4000000|1|90000000|1\n\
            render thread|d.rs|4|90000000|90|1|90000000|1|90000000|1\n";
        let said = costliest(csv, 5);
        let lines: Vec<&str> = said.lines().collect();
        assert!(
            lines[2].contains("50 ms") && lines[2].ends_with("x::f<A, B>"),
            "{said}"
        );
        assert!(lines[2].contains("30.00 ms"), "{said}");
        assert!(lines[3].ends_with("spiky (commands)"), "{said}");
        assert!(lines[4].ends_with("quick"), "{said}");
        // Schedules and threads are not systems.
        assert_eq!(
            lines[5], "Tracy: systems by their longest single run:",
            "{said}"
        );
        assert!(lines[6].ends_with("spiky (commands)"), "{said}");
        assert_eq!(lines.len(), 9, "{said}");
    }

    #[test]
    fn the_clients_version_is_read_from_its_header() {
        let header = "namespace tracy\n{\nnamespace Version\n{\nenum { Major = 0 };\n\
            constexpr int Major = 0;\nconstexpr int Minor = 14;\nconstexpr int Patch = 1;\n}\n}\n";
        assert_eq!(header_version(header).as_deref(), Some("0.14.1"));
    }

    #[test]
    fn the_tools_fetched_are_the_tracy_the_games_client_is() {
        // Cargo.lock holds the client through terra_world's `tracy` feature; its source is in
        // the registry once anything has fetched it.
        match client() {
            Ok((client, speaks)) => assert_eq!(speaks, VERSION, "tracy-client-sys {client}"),
            Err(why) => assert!(why.contains("cargo fetch"), "{why}"),
        }
    }
}
