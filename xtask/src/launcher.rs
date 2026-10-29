//! The launcher behind `cargo xtask play` and `make play` (D575, docs/launcher.md): the
//! build's choices (the build, profiling), in a menu in the terminal or by flags, and then
//! the game, which opens on its own title screen for what to play (D579). A word chooses
//! what to play instead and goes straight in, for scripts and the `make` shortcuts:
//! `continue`, `new` (a new world), `creative` (the Creative world played last, or a new
//! one) or `yard`. The menu only fills in the flags, so a script or agent chooses the same
//! by flags (D69); it shows only when `play` is given nothing in a terminal, or `--menu`.
//! The last options from the menu are kept in `~/.config/terra/launch.json`, preselected the
//! next time and played again by `--last`; a run given flags never reads it, so a script's
//! run does not depend on what someone played last. Whatever `play` does (cook first, the
//! release build by default, D581), the launcher does: it ends in `windows::start`.

use crate::tracy::Mode;
use std::io::{BufRead, IsTerminal, Write};
use std::path::PathBuf;

/// What to play, chosen on the command line rather than on the title screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Choice {
    /// The world played last (made if there is none).
    Continue,
    /// A new world, beside the others (`--fresh`).
    New,
    /// The Creative world played last, or a new one (`--creative`, D494).
    Creative,
    /// The flat test level for building, nothing saved (`--level yard`).
    Yard,
}

/// Each choice's word and the game's arguments for it.
const CHOICES: [(Choice, &str, &[&str]); 4] = [
    (Choice::Continue, "continue", &[]),
    (Choice::New, "new", &["--fresh"]),
    (Choice::Creative, "creative", &["--creative"]),
    (Choice::Yard, "yard", &["--level", "yard"]),
];

/// The game's argument for its title screen, when nothing is chosen in a terminal.
const TITLE: &str = "--title";

impl Choice {
    fn from_word(word: &str) -> Option<Choice> {
        CHOICES.iter().find(|c| c.1 == word).map(|c| c.0)
    }

    fn game_args(self) -> &'static [&'static str] {
        CHOICES.iter().find(|c| c.0 == self).map_or(&[], |c| c.2)
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

/// One launch: what to play, if chosen here, and how.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Launch {
    /// None: the game's title screen chooses.
    pub(crate) choice: Option<Choice>,
    pub(crate) build: Build,
    pub(crate) profiling: Profiling,
}

impl Launch {
    /// The game's arguments for it, `extra` (given after the choices) last.
    pub(crate) fn game_args(&self, extra: &[String]) -> Vec<String> {
        let mut args: Vec<String> = vec!["--experiment".into(), "play".into()];
        let chosen = self.choice.map_or(&[TITLE][..], Choice::game_args);
        args.extend(chosen.iter().map(|arg| arg.to_string()));
        args.extend(extra.iter().cloned());
        args
    }

    /// As kept in `launch.json`: the build's choices; what to play is the title screen's.
    fn to_json(&self) -> String {
        format!(
            "{{\n  \"build\": \"{}\",\n  \"profiling\": \"{}\"\n}}\n",
            self.build.word(),
            self.profiling.word()
        )
    }

    /// Reads what `to_json` wrote; a key missing or not understood is its default, and one
    /// an older launcher kept (`choice`, `quality`) is passed over.
    fn from_json(text: &str) -> Launch {
        let value = |key: &str| -> Option<String> {
            let rest = &text[text.find(&format!("\"{key}\""))? + key.len() + 2..];
            let rest = rest.trim_start().strip_prefix(':')?.trim_start();
            let rest = rest.strip_prefix('"')?;
            Some(rest[..rest.find('"')?].to_string())
        };
        Launch {
            choice: None,
            build: value("build")
                .and_then(|word| BUILDS.iter().find(|b| b.1 == word).map(|b| b.0))
                .unwrap_or_default(),
            profiling: value("profiling")
                .and_then(|word| Profiling::from_word(&word))
                .unwrap_or_default(),
        }
    }
}

/// What `play` was given.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Request {
    choice: Option<Choice>,
    build: Option<Build>,
    profiling: Option<Profiling>,
    last: bool,
    menu: bool,
    /// Everything else, for the game: after `--`, or any argument not the launcher's, as
    /// `make play ARGS="--time 18:00"` passes them (`--quality` among them, the game's).
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
    /// Show the menu, these options preselected.
    Menu(Launch),
    /// Play this, with no menu.
    Play(Launch),
}

/// Decides from the request, whether stdin and stdout are a terminal, and the remembered
/// launch: the menu for nothing given in a terminal or for `--menu`; otherwise play, from
/// the defaults (or the remembered options for `--last`) with the flags given over them.
/// With no word, a terminal opens the game on its title screen and no terminal (an agent,
/// a script) plays Continue, never the menu nor the title screen.
pub(crate) fn plan(request: &Request, terminal: bool, remembered: Option<Launch>) -> Plan {
    let menu = request.menu || (terminal && request.is_empty());
    let mut launch = if menu || request.last {
        remembered.unwrap_or_default()
    } else {
        Launch::default()
    };
    launch.choice = request
        .choice
        .or((!terminal && !menu).then_some(Choice::Continue));
    if let Some(build) = request.build {
        launch.build = build;
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
                "✗ play: {why}. `cargo xtask play [continue|new|creative|yard] [--build optimised|debug] [--profiling off|report|live] [--last] [--menu] [-- game args]`."
            );
            return false;
        }
    };
    let path = remembered_path();
    let remembered = std::fs::read_to_string(&path)
        .ok()
        .map(|text| Launch::from_json(&text));
    let terminal = std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
    let launch = match plan(&request, terminal, remembered) {
        Plan::Play(launch) => launch,
        Plan::Menu(preselected) => {
            let stdin = std::io::stdin();
            let chosen = menu(
                preselected,
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
                .and_then(|()| std::fs::write(&path, launch.to_json()));
            if let Err(why) = kept {
                eprintln!(
                    "play: could not keep these options in {} ({why}); the next launch opens on the defaults.",
                    path.display()
                );
            }
            launch
        }
    };
    let mut launch = launch;
    loop {
        let game = launch.game_args(&request.game);
        let (ok, code) = match launch.profiling {
            Profiling::Off => crate::windows::start(launch.build, game),
            Profiling::Report => crate::profile::report(launch.build, &game, "play", Mode::Off),
            Profiling::Live => crate::profile::report(launch.build, &game, "play", Mode::Live),
        };
        match again(code) {
            Some(choice) => launch.choice = choice,
            None => return ok,
        }
    }
}

/// The game's exit codes asking to be started again, as the title screen's choices need a
/// fresh process (`terra_game::play::title`, D579): the yard is another level, made as the
/// game starts, and a world left for the title screen cannot be emptied in place. The game
/// is started again here, with the same build and profiling, so a session stays in one
/// terminal and each of its games is reported.
const AGAIN_ON_TITLE: i32 = 75;
const AGAIN_ON_YARD: i32 = 76;

/// What to play next, if the game asked to be started again: `Some(None)` the title screen.
fn again(code: Option<i32>) -> Option<Option<Choice>> {
    match code {
        Some(AGAIN_ON_TITLE) => Some(None),
        Some(AGAIN_ON_YARD) => Some(Some(Choice::Yard)),
        _ => None,
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

/// The menu (docs/launcher.md, "What it shows"), a line read at a time: Enter plays with
/// these options, on the game's title screen; `o` changes the build and profiling; `q` or
/// the end of input quits with `None`.
pub(crate) fn menu(
    mut launch: Launch,
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
        let _ = writeln!(out, "{}", screen(&launch, header));
        match read(out, "> ")?.as_str() {
            "" => return Some(launch),
            "q" => return None,
            "o" => options(&mut launch, out, &mut read)?,
            other => {
                let _ = writeln!(out, "`{other}` is not one of these: Enter, o or q.");
            }
        }
    }
}

type Ask<'a> = dyn FnMut(&mut dyn Write, &str) -> Option<String> + 'a;

/// The options (`o`): the build and profiling, each kept by Enter or chosen by a word or
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
fn screen(launch: &Launch, header: &str) -> String {
    [
        String::new(),
        header.to_string(),
        String::new(),
        format!(
            "  Build: {}   Profiling: {}      o  change these",
            launch.build.word(),
            launch.profiling.shown()
        ),
        String::new(),
        "Enter plays: the game opens on its title screen (worlds, settings).  o options, q quit."
            .to_string(),
    ]
    .join("\n")
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
