//! The launcher behind `cargo xtask play` and `make play` (D575, docs/launcher.md): what to
//! play (Continue, New game, Creative, the yard) and how (the build, the quality preset),
//! chosen in a menu in the terminal or by flags. The menu only fills in the flags, so a
//! script or agent chooses the same by flags (D69); it shows only when `play` is given
//! nothing in a terminal, or `--menu`. The last launch from the menu is kept in
//! `~/.config/terra/launch.json`, preselected the next time and played again by `--last`;
//! a run given flags never reads it, so a script's run does not depend on what someone
//! played last. Whatever `play` does (cook first, the release build by default, D581), the
//! launcher does: it ends in `windows::start`.

use crate::tracy::Mode;
use std::io::{BufRead, IsTerminal, Write};
use std::path::PathBuf;

/// What to play.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Choice {
    /// The world kept between runs.
    #[default]
    Continue,
    /// A new world, replacing the saved one (`--fresh`).
    New,
    /// The world kept, building free (`--free-build`, the stand-in for creative, D494).
    Creative,
    /// The flat test level for building, nothing saved (`--level yard`).
    Yard,
}

const CHOICES: [(Choice, &str, &str, &str); 4] = [
    (
        Choice::Continue,
        "continue",
        "Continue",
        "your world, kept between runs",
    ),
    (
        Choice::New,
        "new",
        "New game",
        "a new world; replaces your saved one",
    ),
    (
        Choice::Creative,
        "creative",
        "Creative",
        "your world, building free",
    ),
    (
        Choice::Yard,
        "yard",
        "The yard",
        "a flat level for building, nothing saved",
    ),
];

impl Choice {
    /// Its word on the command line and in `launch.json`.
    fn word(self) -> &'static str {
        CHOICES
            .iter()
            .find(|c| c.0 == self)
            .map_or("continue", |c| c.1)
    }

    fn from_word(word: &str) -> Option<Choice> {
        CHOICES.iter().find(|c| c.1 == word).map(|c| c.0)
    }

    fn title(self) -> &'static str {
        CHOICES.iter().find(|c| c.0 == self).map_or("", |c| c.2)
    }
}

/// The build played: optimised (the release profile, D581) or debug.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Build {
    #[default]
    Optimised,
    Debug,
}

const BUILDS: [(Build, &str); 2] = [(Build::Optimised, "optimised"), (Build::Debug, "debug")];

impl Build {
    /// Its cargo profile.
    pub(crate) fn profile(self) -> &'static str {
        match self {
            Build::Optimised => "release",
            Build::Debug => "dev",
        }
    }

    fn word(self) -> &'static str {
        BUILDS
            .iter()
            .find(|b| b.0 == self)
            .map_or("optimised", |b| b.1)
    }
}

/// The quality presets the game takes by `--quality` (`terra_world::quality`); none chosen
/// is `auto`, the GPU's own preset (D239).
const QUALITIES: [&str; 4] = ["low", "medium", "high", "ultra"];

/// What the session measures (docs/launcher.md, "Diagnosing performance"): nothing, a
/// session report written as the game exits (D576, `cargo xtask profile`), or that and
/// Tracy's profiler watching it live (D577, `cargo xtask profile play --tracy`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Profiling {
    #[default]
    Off,
    Report,
    Live,
}

const PROFILINGS: [(Profiling, &str); 3] = [
    (Profiling::Off, "off"),
    (Profiling::Report, "report"),
    (Profiling::Live, "live"),
];

impl Profiling {
    fn word(self) -> &'static str {
        PROFILINGS
            .iter()
            .find(|p| p.0 == self)
            .map_or("off", |p| p.1)
    }

    /// As the screen shows it.
    fn shown(self) -> &'static str {
        match self {
            Profiling::Live => "live (Tracy)",
            other => other.word(),
        }
    }

    fn from_word(word: &str) -> Option<Profiling> {
        PROFILINGS.iter().find(|p| p.1 == word).map(|p| p.0)
    }
}

/// One launch: what to play and how.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Launch {
    pub(crate) choice: Choice,
    pub(crate) build: Build,
    /// A preset of `QUALITIES`, or `None` for auto.
    pub(crate) quality: Option<&'static str>,
    pub(crate) profiling: Profiling,
}

impl Launch {
    /// The game's arguments for it, `extra` (given after the choices) last.
    pub(crate) fn game_args(&self, extra: &[String]) -> Vec<String> {
        let mut args: Vec<String> = vec!["--experiment".into(), "play".into()];
        args.extend(
            match self.choice {
                Choice::Continue => &[][..],
                Choice::New => &["--fresh"][..],
                Choice::Creative => &["--free-build"][..],
                Choice::Yard => &["--level", "yard"][..],
            }
            .iter()
            .map(|arg| arg.to_string()),
        );
        if let Some(quality) = self.quality {
            args.extend(["--quality".into(), quality.into()]);
        }
        args.extend(extra.iter().cloned());
        args
    }

    /// As kept in `launch.json`: a new game is kept as Continue, so playing the last launch
    /// again plays the world it made rather than replacing it.
    fn remembered(&self) -> Launch {
        let choice = match self.choice {
            Choice::New => Choice::Continue,
            other => other,
        };
        Launch {
            choice,
            ..self.clone()
        }
    }

    fn to_json(&self) -> String {
        format!(
            "{{\n  \"choice\": \"{}\",\n  \"build\": \"{}\",\n  \"quality\": \"{}\",\n  \"profiling\": \"{}\"\n}}\n",
            self.choice.word(),
            self.build.word(),
            self.quality.unwrap_or("auto"),
            self.profiling.word()
        )
    }

    /// Reads what `to_json` wrote; a key missing or not understood is its default.
    fn from_json(text: &str) -> Launch {
        let value = |key: &str| -> Option<String> {
            let rest = &text[text.find(&format!("\"{key}\""))? + key.len() + 2..];
            let rest = rest.trim_start().strip_prefix(':')?.trim_start();
            let rest = rest.strip_prefix('"')?;
            Some(rest[..rest.find('"')?].to_string())
        };
        Launch {
            choice: value("choice")
                .and_then(|word| Choice::from_word(&word))
                .unwrap_or_default(),
            build: value("build")
                .and_then(|word| BUILDS.iter().find(|b| b.1 == word).map(|b| b.0))
                .unwrap_or_default(),
            quality: value("quality").and_then(|word| quality(&word)),
            profiling: value("profiling")
                .and_then(|word| Profiling::from_word(&word))
                .unwrap_or_default(),
        }
    }
}

fn quality(word: &str) -> Option<&'static str> {
    QUALITIES.iter().copied().find(|preset| *preset == word)
}

/// What `play` was given.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Request {
    choice: Option<Choice>,
    build: Option<Build>,
    /// `Some(None)` for `--quality auto`.
    quality: Option<Option<&'static str>>,
    profiling: Option<Profiling>,
    last: bool,
    menu: bool,
    /// Everything else, for the game: after `--`, or any argument not the launcher's, as
    /// `make play ARGS="--time 18:00"` passes them.
    game: Vec<String>,
}

impl Request {
    /// Reads `play`'s arguments: a choice first, then the launcher's flags, the rest the
    /// game's. An error says what was wrong and what is taken.
    pub(crate) fn parse(args: &[String]) -> Result<Request, String> {
        let mut request = Request::default();
        let mut args = args.iter().peekable();
        if let Some(choice) = args.peek().and_then(|word| Choice::from_word(word)) {
            request.choice = Some(choice);
            args.next();
        }
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--build" => {
                    let word = args.next().map_or("", String::as_str);
                    request.build = Some(match word {
                        "optimised" | "release" => Build::Optimised,
                        "debug" => Build::Debug,
                        _ => {
                            return Err(format!(
                                "--build is optimised or debug, not `{word}` (D575)"
                            ));
                        }
                    });
                }
                "--quality" => {
                    let word = args.next().map_or("", String::as_str);
                    request.quality = Some(match word {
                        "auto" => None,
                        word => Some(quality(word).ok_or(format!(
                            "--quality is auto, low, medium, high or ultra, not `{word}` (D239)"
                        ))?),
                    });
                }
                "--profiling" => {
                    let word = args.next().map_or("", String::as_str);
                    request.profiling = Some(Profiling::from_word(word).ok_or(format!(
                        "--profiling is off, report or live, not `{word}` (D576, D577)"
                    ))?);
                }
                "--last" => request.last = true,
                "--menu" => request.menu = true,
                "--" => request.game.extend(args.by_ref().cloned()),
                _ => request.game.push(arg.clone()),
            }
        }
        Ok(request)
    }

    fn is_empty(&self) -> bool {
        *self == Request::default()
    }
}

/// What `play` does with a request.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Plan {
    /// Show the menu, this launch preselected.
    Menu(Launch),
    /// Play this, with no menu.
    Play(Launch),
}

/// Decides from the request, whether stdin and stdout are a terminal, and the remembered
/// launch: the menu for nothing given in a terminal or for `--menu`; otherwise play, from
/// the defaults (or the remembered launch for `--last`) with the flags given over it. With
/// no terminal (an agent, a script) and nothing given, the defaults play, never the menu.
pub(crate) fn plan(request: &Request, terminal: bool, remembered: Option<Launch>) -> Plan {
    let menu = request.menu || (terminal && request.is_empty());
    let mut launch = if menu || request.last {
        remembered.unwrap_or_default()
    } else {
        Launch::default()
    };
    if let Some(choice) = request.choice {
        launch.choice = choice;
    }
    if let Some(build) = request.build {
        launch.build = build;
    }
    if let Some(quality) = request.quality {
        launch.quality = quality;
    }
    if let Some(profiling) = request.profiling {
        launch.profiling = profiling;
    }
    if menu {
        Plan::Menu(launch)
    } else {
        Plan::Play(launch)
    }
}

/// `launch.json`'s path: in the user's config folder, one per machine (D575).
fn remembered_path() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .unwrap_or_default()
        .join("terra")
        .join("launch.json")
}

/// `cargo xtask play`: the launcher, then the game.
pub(crate) fn play(args: &[String]) -> bool {
    let request = match Request::parse(args) {
        Ok(request) => request,
        Err(why) => {
            eprintln!(
                "✗ play: {why}. `cargo xtask play [continue|new|creative|yard] [--build optimised|debug] [--quality auto|low|medium|high|ultra] [--profiling off|report|live] [--last] [--menu] [-- game args]`."
            );
            return false;
        }
    };
    let path = remembered_path();
    let remembered = std::fs::read_to_string(&path)
        .ok()
        .map(|text| Launch::from_json(&text));
    let terminal = std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
    let last = remembered.as_ref().map(|launch| launch.choice);
    let launch = match plan(&request, terminal, remembered) {
        Plan::Play(launch) => launch,
        Plan::Menu(preselected) => {
            let stdin = std::io::stdin();
            let chosen = menu(
                preselected,
                last,
                &header(),
                &mut stdin.lock(),
                &mut std::io::stdout(),
            );
            let Some(launch) = chosen else {
                return true;
            };
            let kept = path
                .parent()
                .map_or(Ok(()), std::fs::create_dir_all)
                .and_then(|()| std::fs::write(&path, launch.remembered().to_json()));
            if let Err(why) = kept {
                eprintln!(
                    "play: could not keep this launch in {} ({why}); the next opens on Continue.",
                    path.display()
                );
            }
            launch
        }
    };
    let game = launch.game_args(&request.game);
    match launch.profiling {
        Profiling::Off => crate::windows::start(launch.build, game),
        Profiling::Report => crate::profile::report(launch.build, &game, "play", Mode::Off),
        Profiling::Live => crate::profile::report(launch.build, &game, "play", Mode::Live),
    }
}

/// The menu's first line: the game, and the branch and commit it is built from.
fn header() -> String {
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .args(args)
            .current_dir(crate::root())
            .output()
            .ok()
            .filter(|out| out.status.success())
            .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
            .unwrap_or_default()
    };
    format!(
        "Terra   {} {}",
        git(&["rev-parse", "--abbrev-ref", "HEAD"]),
        git(&["rev-parse", "--short", "HEAD"])
    )
}

/// The menu (docs/launcher.md, "What it shows"), a line read at a time: Enter plays the
/// preselected launch, a number plays that choice (New game asks first), `o` changes the
/// build and quality, `q` or the end of input quits with `None`. `last`, the remembered
/// launch's choice, is marked.
pub(crate) fn menu(
    mut launch: Launch,
    last: Option<Choice>,
    header: &str,
    input: &mut impl BufRead,
    out: &mut impl Write,
) -> Option<Launch> {
    let mut read = |out: &mut dyn Write, prompt: &str| -> Option<String> {
        let _ = write!(out, "{prompt}");
        let _ = out.flush();
        let mut line = String::new();
        match input.read_line(&mut line) {
            Ok(0) | Err(_) => None,
            Ok(_) => Some(line.trim().to_lowercase()),
        }
    };
    loop {
        let _ = writeln!(out, "{}", screen(&launch, last, header));
        let answer = read(out, "> ")?;
        let choice = match answer.as_str() {
            "" => launch.choice,
            "q" => return None,
            "o" => {
                options(&mut launch, out, &mut read)?;
                continue;
            }
            number => match number
                .parse::<usize>()
                .ok()
                .and_then(|n| CHOICES.get(n.wrapping_sub(1)))
            {
                Some(choice) => choice.0,
                None => {
                    let _ = writeln!(out, "`{number}` is not one of these: 1-4, o, q, or Enter.");
                    continue;
                }
            },
        };
        if choice == Choice::New {
            let sure = read(
                out,
                "New game deletes your saved world for good. Start a new one? (y/N) ",
            )?;
            if sure != "y" && sure != "yes" {
                let _ = writeln!(out, "Your world is kept.");
                continue;
            }
        }
        return Some(Launch { choice, ..launch });
    }
}

type Ask<'a> = dyn FnMut(&mut dyn Write, &str) -> Option<String> + 'a;

/// The options (`o`): the build and the quality, each kept by Enter or chosen by a word or
/// its first letters.
fn options(launch: &mut Launch, out: &mut dyn Write, read: &mut Ask<'_>) -> Option<()> {
    let pick = |answer: &str, words: &[&'static str]| -> Option<&'static str> {
        let mut found = words
            .iter()
            .copied()
            .filter(|word| word.starts_with(answer));
        let first = found.next()?;
        found.next().is_none().then_some(first)
    };
    let builds = BUILDS.map(|b| b.1);
    let answer = read(
        out,
        &format!("Build: optimised or debug [{}] ", launch.build.word()),
    )?;
    if !answer.is_empty() {
        match pick(&answer, &builds) {
            Some(word) => {
                launch.build = BUILDS
                    .iter()
                    .find(|b| b.1 == word)
                    .map_or(Build::Optimised, |b| b.0);
                if let Some(note) = unbuilt(launch.build) {
                    let _ = writeln!(out, "{note}");
                }
            }
            None => {
                let _ = writeln!(
                    out,
                    "`{answer}` is not optimised or debug; the build is kept."
                );
            }
        }
    }
    let words = ["auto", "low", "medium", "high", "ultra"];
    let answer = read(
        out,
        &format!(
            "Quality: auto, low, medium, high or ultra [{}] ",
            launch.quality.unwrap_or("auto")
        ),
    )?;
    if !answer.is_empty() {
        match pick(&answer, &words) {
            Some(word) => launch.quality = quality(word),
            None => {
                let _ = writeln!(out, "`{answer}` is not a preset; the quality is kept.");
            }
        }
    }
    let words = PROFILINGS.map(|p| p.1);
    let answer = read(
        out,
        &format!(
            "Profiling: off, report (what the session spent its time on, written when you quit) or live (Tracy: watch every system as it runs) [{}] ",
            launch.profiling.word()
        ),
    )?;
    if !answer.is_empty() {
        match pick(&answer, &words).and_then(Profiling::from_word) {
            Some(profiling) => {
                launch.profiling = profiling;
                let said = match profiling {
                    Profiling::Off => None,
                    Profiling::Report => Some(
                        "A report build times every system, so its frames run about a sixth slower; its first launch builds it, a few minutes.",
                    ),
                    Profiling::Live => Some(
                        "Tracy's profiler opens beside the game and shows every system as it runs, and the report is written when you quit. Its first launch fetches Tracy and builds the game again, a few minutes.",
                    ),
                };
                if let Some(said) = said {
                    let _ = writeln!(out, "{said}");
                }
            }
            None => {
                let _ = writeln!(
                    out,
                    "`{answer}` is not off, report or live; profiling is kept."
                );
            }
        }
    }
    Some(())
}

/// The menu's screen.
fn screen(launch: &Launch, last: Option<Choice>, header: &str) -> String {
    let mut lines = vec![String::new(), header.to_string(), String::new()];
    for (number, (choice, _, title, what)) in CHOICES.iter().enumerate() {
        let mark = if Some(*choice) == last {
            "  <- last time"
        } else {
            ""
        };
        lines.push(
            format!("  {}  {title:<12}  {what:<42}{mark}", number + 1)
                .trim_end()
                .to_string(),
        );
    }
    lines.push(String::new());
    lines.push(format!(
        "  Build: {}   Quality: {}   Profiling: {}      o  change these",
        launch.build.word(),
        launch.quality.unwrap_or("auto (the GPU's preset)"),
        launch.profiling.shown()
    ));
    lines.push(String::new());
    lines.push(format!(
        "Enter plays {}.  1-4 choose, o options, q quit.",
        launch.choice.title()
    ));
    lines.join("\n")
}

/// Says when a build has not been made on this machine, so its first launch builds it.
fn unbuilt(build: Build) -> Option<String> {
    let folder = match build {
        Build::Debug => "debug",
        Build::Optimised => "release",
    };
    let target = crate::root().join("target");
    let program = if crate::windows::on_wsl() {
        target
            .join(crate::windows::WINDOWS_TARGET)
            .join(folder)
            .join("terra_sandbox.exe")
    } else {
        target.join(folder).join("terra_sandbox")
    };
    (!program.exists()).then(|| {
        format!(
            "The {} build is not made yet on this machine; its first launch builds it, a few minutes.",
            build.word()
        )
    })
}

#[cfg(test)]
mod tests;
