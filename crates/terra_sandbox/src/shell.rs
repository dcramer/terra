//! The parts every experiment gets for free: the window, overlays, inspectors, and the
//! scripted modes that let a run be checked without anyone watching.

use crate::{audio, debug};
use bevy::{
    audio::AudioPlugin,
    camera_controller::free_camera::FreeCameraPlugin,
    input::{
        InputSystems,
        common_conditions::input_toggle_active,
        mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll},
    },
    log::{DEFAULT_FILTER, LogPlugin},
    prelude::*,
    render::{
        renderer::RenderAdapterInfo,
        view::screenshot::{Screenshot, save_to_disk},
    },
    time::TimeUpdateStrategy,
    window::PresentMode,
    winit::{UpdateMode, WinitSettings},
};
use bevy_inspector_egui::{bevy_egui::EguiPlugin, quick::WorldInspectorPlugin};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

pub const USAGE: &str = "\
usage: terra_sandbox [options]

  --experiment <name>   switch on an experiment (repeatable)
  --list                list the experiments and exit
  --radius <metres>     planet radius (default 2000)
  --shot <file.png>     save a screenshot once the scene has settled, then exit
  --clean               leave the frame-time readout off the screen, for pictures
  --mute                no sound
  --sound               sound under WSL too, where it is off by default
  --bench               measure frame times with vsync off, print them, then exit
  --settle <frames>     frames to run before capturing or measuring (default 120)
  --press <key>@<frame> press a letter, digit, F1 to F12, esc, enter, backspace or an arrow
                        (left, right, up, down) on a given frame, for scripted runs;
                        <key>@<from>-<to> holds it over a range of frames
  --give <item>:<count> start the play experiment carrying something extra, for
                        testing: a raw item such as stone:10, a tool such as
                        spear:1 or iron_axe:1, clothing to wear such as
                        leather_coat:1, or health, fed or stamina out of 100
                        (repeatable)
  --save <file>         where the play experiment keeps its saved game; scripted runs
                        save and load only when given this
  --fresh               start the play experiment over, throwing away its saved game
  --at <east,south>     start the play experiment's player so many metres east and south
                        of the start, for testing
  --face <east,south>   start the play experiment's player facing that spot, for testing
  --warp <east,south:east,south@frame>
                        on that frame, move the play experiment's player to the first
                        spot, facing the second, for testing (repeatable)
  --vantage <x,y,z,x,y,z>
                        where a fixed camera stands and what it looks at, in metres
                        east, up and south of the base's corner, for experiments
                        that have one";

/// Engine messages that say what is normal, kept out of the log so what is left is worth
/// reading. Warnings and errors still come through. `RUST_LOG` replaces all of this.
const QUIET: &str = "\
bevy_render=warn,bevy_pbr=warn,bevy_winit=warn,bevy_egui=warn,bevy_diagnostic=warn,\
sctk_adwaita=off";

/// Frames to wait before measuring or capturing by default, so shaders have compiled and
/// the scene has finished loading.
const SETTLE_FRAMES: u32 = 120;
const BENCH_FRAMES: usize = 600;

#[derive(Resource, Clone, Debug)]
pub struct Options {
    pub experiments: Vec<String>,
    pub list: bool,
    pub radius_m: f32,
    pub shot: Option<PathBuf>,
    pub settle_frames: u32,
    pub bench: bool,
    /// No readout on screen.
    pub clean: bool,
    /// No sound, and sound under WSL, where it is off unless asked for.
    pub mute: bool,
    pub sound: bool,
    /// A key, the frame it goes down, and the frame it comes back up.
    pub presses: Vec<(KeyCode, u32, u32)>,
    /// A fixed camera: where it stands and what it looks at, in an experiment's own space.
    pub vantage: Option<(Vec3, Vec3)>,
    /// Things to start carrying, by name, for testing.
    pub gives: Vec<(String, u32)>,
    /// Where the play experiment keeps its saved game, if not the usual place.
    pub save: Option<PathBuf>,
    /// Start over, throwing away any saved game.
    pub fresh: bool,
    /// Where the play experiment's player starts, east and south of the start in metres.
    pub start_at: Option<(f32, f32)>,
    /// What the play experiment's player starts facing, east and south of the start.
    pub start_facing: Option<(f32, f32)>,
    /// On a frame, put the play experiment's player at a spot, facing another.
    pub warps: Vec<Warp>,
}

/// A jump in a scripted run: on `frame`, stand `at` a spot facing `facing`, each in metres
/// east and south of the start.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Warp {
    pub frame: u32,
    pub at: (f32, f32),
    pub facing: (f32, f32),
}

/// Which frame of a scripted run this is, counted as `--press` counts them.
#[derive(Resource, Default)]
pub struct ScriptFrame(pub u32);

impl Options {
    pub fn from_args(args: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut options = Options {
            experiments: Vec::new(),
            list: false,
            radius_m: 2000.0,
            shot: None,
            settle_frames: SETTLE_FRAMES,
            bench: false,
            clean: false,
            mute: false,
            sound: false,
            presses: Vec::new(),
            vantage: None,
            gives: Vec::new(),
            save: None,
            fresh: false,
            start_at: None,
            start_facing: None,
            warps: Vec::new(),
        };
        let mut args = args.peekable();
        while let Some(arg) = args.next() {
            let mut value = |name: &str| args.next().ok_or(format!("{name} needs a value"));
            match arg.as_str() {
                "--experiment" => options.experiments.push(value("--experiment")?),
                "--list" => options.list = true,
                "--bench" => options.bench = true,
                "--clean" => options.clean = true,
                "--mute" => options.mute = true,
                "--sound" => options.sound = true,
                "--fresh" => options.fresh = true,
                "--at" => options.start_at = Some(east_south("--at", &value("--at")?)?),
                "--face" => options.start_facing = Some(east_south("--face", &value("--face")?)?),
                "--warp" => {
                    let text = value("--warp")?;
                    let problem = || {
                        format!(
                            "--warp wants where to stand and what to face, then a frame, like 66,-62:66,-64@900, not `{text}`"
                        )
                    };
                    let (places, frame) = text.split_once('@').ok_or_else(problem)?;
                    let (at, facing) = places.split_once(':').ok_or_else(problem)?;
                    options.warps.push(Warp {
                        frame: frame.trim().parse().map_err(|_| problem())?,
                        at: east_south("--warp", at)?,
                        facing: east_south("--warp", facing)?,
                    });
                }
                "--save" => options.save = Some(PathBuf::from(value("--save")?)),
                "--shot" => options.shot = Some(PathBuf::from(value("--shot")?)),
                "--radius" => {
                    options.radius_m = value("--radius")?
                        .parse()
                        .map_err(|_| "--radius needs a number of metres".to_string())?;
                }
                "--settle" => {
                    options.settle_frames = value("--settle")?
                        .parse()
                        .map_err(|_| "--settle needs a number of frames".to_string())?;
                }
                "--press" => options.presses.push(parse_press(&value("--press")?)?),
                "--give" => {
                    let text = value("--give")?;
                    let (name, count) = text
                        .split_once(':')
                        .and_then(|(name, count)| Some((name.to_string(), count.parse().ok()?)))
                        .ok_or(format!(
                            "--give wants an item and a count, like stone:10, not `{text}`"
                        ))?;
                    options.gives.push((name, count));
                }
                "--vantage" => {
                    let numbers: Vec<f32> = value("--vantage")?
                        .split(',')
                        .filter_map(|number| number.trim().parse().ok())
                        .collect();
                    let [a, b, c, d, e, f] = numbers[..] else {
                        return Err("--vantage needs six numbers, like 4,3,20,6,2,0".to_string());
                    };
                    options.vantage = Some((Vec3::new(a, b, c), Vec3::new(d, e, f)));
                }
                other => return Err(format!("Unknown option `{other}`")),
            }
        }
        Ok(options)
    }

    /// Whether this run drives itself and exits, rather than waiting for a person.
    pub fn scripted(&self) -> bool {
        self.shot.is_some() || self.bench
    }

    /// Whether anything is heard. A scripted run is silent, so it neither plays sounds
    /// over whatever the person at the machine is doing nor needs a sound device. WSL
    /// usually has no sound device, and its sound library fills the log saying so, so
    /// sound is off there unless asked for.
    pub fn sound_on(&self) -> bool {
        !self.scripted() && !self.mute && (self.sound || !on_wsl())
    }
}

/// Metres east and south of the start, written like 70,-55.
fn east_south(name: &str, text: &str) -> Result<(f32, f32), String> {
    text.split_once(',')
        .and_then(|(east, south)| Some((east.trim().parse().ok()?, south.trim().parse().ok()?)))
        .ok_or(format!(
            "{name} wants metres east and south, like 70,-55, not `{text}`"
        ))
}

fn parse_press(text: &str) -> Result<(KeyCode, u32, u32), String> {
    const LETTERS: [KeyCode; 26] = [
        KeyCode::KeyA,
        KeyCode::KeyB,
        KeyCode::KeyC,
        KeyCode::KeyD,
        KeyCode::KeyE,
        KeyCode::KeyF,
        KeyCode::KeyG,
        KeyCode::KeyH,
        KeyCode::KeyI,
        KeyCode::KeyJ,
        KeyCode::KeyK,
        KeyCode::KeyL,
        KeyCode::KeyM,
        KeyCode::KeyN,
        KeyCode::KeyO,
        KeyCode::KeyP,
        KeyCode::KeyQ,
        KeyCode::KeyR,
        KeyCode::KeyS,
        KeyCode::KeyT,
        KeyCode::KeyU,
        KeyCode::KeyV,
        KeyCode::KeyW,
        KeyCode::KeyX,
        KeyCode::KeyY,
        KeyCode::KeyZ,
    ];
    let problem = || {
        format!(
            "--press wants a letter, digit, F1 to F12, esc, enter, backspace or an arrow (left, right, up, down) and a frame, like f@60, F3@10 or w@40-90, not `{text}`"
        )
    };
    let (key, frames) = text.split_once('@').ok_or_else(problem)?;
    const DIGITS: [KeyCode; 10] = [
        KeyCode::Digit0,
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
        KeyCode::Digit7,
        KeyCode::Digit8,
        KeyCode::Digit9,
    ];
    const FUNCTIONS: [KeyCode; 12] = [
        KeyCode::F1,
        KeyCode::F2,
        KeyCode::F3,
        KeyCode::F4,
        KeyCode::F5,
        KeyCode::F6,
        KeyCode::F7,
        KeyCode::F8,
        KeyCode::F9,
        KeyCode::F10,
        KeyCode::F11,
        KeyCode::F12,
    ];
    let function = key
        .strip_prefix(['F', 'f'])
        .and_then(|number| number.parse::<usize>().ok())
        .and_then(|number| FUNCTIONS.get(number.checked_sub(1)?));
    let code = match function {
        Some(&code) => code,
        None if key.eq_ignore_ascii_case("esc") => KeyCode::Escape,
        None if key.eq_ignore_ascii_case("enter") => KeyCode::Enter,
        None if key.eq_ignore_ascii_case("backspace") => KeyCode::Backspace,
        None if key.eq_ignore_ascii_case("left") => KeyCode::ArrowLeft,
        None if key.eq_ignore_ascii_case("right") => KeyCode::ArrowRight,
        None if key.eq_ignore_ascii_case("up") => KeyCode::ArrowUp,
        None if key.eq_ignore_ascii_case("down") => KeyCode::ArrowDown,
        None => {
            let symbol = key
                .chars()
                .next()
                .filter(|c| key.len() == 1 && c.is_ascii_alphanumeric());
            let symbol = symbol.ok_or_else(problem)?.to_ascii_lowercase();
            match symbol.to_digit(10) {
                Some(digit) => DIGITS[digit as usize],
                None => LETTERS[(symbol as u8 - b'a') as usize],
            }
        }
    };
    let frame = |text: &str| text.parse::<u32>().map_err(|_| problem());
    let (down, up) = match frames.split_once('-') {
        Some((from, to)) => (frame(from)?, frame(to)?),
        None => (frame(frames)?, frame(frames)? + 1),
    };
    if up <= down {
        return Err(problem());
    }
    Ok((code, down, up))
}

pub struct ShellPlugin(pub Options);

impl Plugin for ShellPlugin {
    fn build(&self, app: &mut App) {
        let options = self.0.clone();
        let readout = !options.clean;
        let window = Window {
            title: "Terra sandbox".into(),
            // A scripted run needs nobody's attention, and should not take the keyboard
            // away from whatever is in front.
            focused: !options.scripted(),
            resolution: (1600u32, 900u32).into(),
            // Vsync hides how long a frame really takes, so benchmarks turn it off.
            present_mode: if options.bench {
                PresentMode::AutoNoVsync
            } else {
                PresentMode::AutoVsync
            },
            ..default()
        };
        if options.scripted() {
            // Its window does not take focus, and a window out of focus is normally only
            // drawn sixty times a second. A scripted run is drawn as fast as it will go.
            app.insert_resource(WinitSettings {
                focused_mode: UpdateMode::Continuous,
                unfocused_mode: UpdateMode::Continuous,
            });
            // A scripted run advances the game by a sixtieth of a second every frame,
            // however fast the machine draws, so the same script does the same thing on
            // a software renderer and on a fast GPU.
            app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
                1.0 / 60.0,
            )));
        }
        let sound = options.sound_on();
        let engine = DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(window),
                ..default()
            })
            .set(LogPlugin {
                filter: format!("{DEFAULT_FILTER},{QUIET}"),
                ..default()
            });
        app.insert_resource(options)
            .init_resource::<ScriptFrame>()
            .insert_resource(ClearColor(Color::srgb(0.01, 0.01, 0.02)))
            .add_plugins(if sound {
                engine
            } else {
                engine.disable::<AudioPlugin>()
            })
            .add_plugins((
                audio::plugin(sound),
                FreeCameraPlugin,
                EguiPlugin::default(),
                // On by default, screenshots included, so it is there when it is needed.
                debug::plugin(readout),
            ))
            // F1: every entity, resource and asset in the world, editable live.
            .add_plugins(
                WorldInspectorPlugin::new().run_if(input_toggle_active(false, KeyCode::F1)),
            )
            .add_systems(PreUpdate, scripted_presses.after(InputSystems))
            .add_systems(Update, scripted_run);
    }
}

/// In a scripted run, the keyboard and mouse are whatever the script says and nothing
/// else. The window is a real one, so without this, anything typed while it has focus
/// would steer the run.
fn scripted_presses(
    options: Res<Options>,
    mut frame: ResMut<ScriptFrame>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut motion: ResMut<AccumulatedMouseMotion>,
    mut scroll: ResMut<AccumulatedMouseScroll>,
) {
    if !options.scripted() {
        return;
    }
    frame.0 += 1;
    keys.reset_all();
    mouse.reset_all();
    motion.delta = Vec2::ZERO;
    scroll.delta = Vec2::ZERO;
    for &(key, down, up) in &options.presses {
        if (down..up).contains(&frame.0) {
            keys.press(key);
            // Held since an earlier frame, so not newly pressed on this one.
            if frame.0 != down {
                keys.clear_just_pressed(key);
            }
        }
    }
}

fn scripted_run(
    mut commands: Commands,
    options: Res<Options>,
    adapter: Res<RenderAdapterInfo>,
    mut frame: Local<u32>,
    mut frame_ms: Local<Vec<f32>>,
    mut last_frame: Local<Option<Instant>>,
    mut exit: MessageWriter<AppExit>,
) {
    if !options.scripted() {
        return;
    }
    // Scripted runs use a made-up game clock, so frames are timed against the real one.
    let now = Instant::now();
    let real_ms = last_frame
        .replace(now)
        .map(|last| (now - last).as_secs_f32() * 1000.0);
    *frame += 1;
    let settle = options.settle_frames;
    if *frame <= settle {
        return;
    }
    if let Some(path) = &options.shot {
        if *frame == settle + 1 {
            if let Some(folder) = path.parent() {
                std::fs::create_dir_all(folder).ok();
            }
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(path.clone()));
        }
        // Saving happens in the background, and how many frames that takes depends on
        // how fast frames are going by. Leave once the file is there, or give up.
        if *frame > settle + 1 && !options.bench {
            if path.exists() {
                println!("SANDBOX saved {}", path.display());
                exit.write(AppExit::Success);
            } else if *frame > settle + 3000 {
                println!("SANDBOX could not save {}", path.display());
                exit.write(AppExit::error());
            }
        }
    }
    if options.bench {
        frame_ms.extend(real_ms);
        if frame_ms.len() == BENCH_FRAMES {
            frame_ms.sort_by(|a, b| a.total_cmp(b));
            println!(
                "SANDBOX frames on {}: median {:.2} ms, 99th percentile {:.2} ms, worst {:.2} ms",
                adapter.name,
                frame_ms[BENCH_FRAMES / 2],
                frame_ms[BENCH_FRAMES * 99 / 100],
                frame_ms[BENCH_FRAMES - 1]
            );
            exit.write(AppExit::Success);
        }
    }
}

/// Whether this is Linux under Windows.
fn on_wsl() -> bool {
    std::fs::read_to_string("/proc/version").is_ok_and(|v| v.to_lowercase().contains("microsoft"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Options, String> {
        Options::from_args(args.iter().map(|a| a.to_string()))
    }

    #[test]
    fn reads_the_options() {
        let options = parse(&[
            "--experiment",
            "scatter",
            "--radius",
            "500",
            "--shot",
            "out/a.png",
            "--press",
            "F@60",
        ])
        .unwrap();
        assert_eq!(options.experiments, ["scatter"]);
        assert_eq!(options.radius_m, 500.0);
        assert_eq!(options.shot, Some(PathBuf::from("out/a.png")));
        assert_eq!(options.presses, [(KeyCode::KeyF, 60, 61)]);
        let digit = parse(&["--press", "3@10"]).unwrap();
        assert_eq!(digit.presses, [(KeyCode::Digit3, 10, 11)]);
        let held = parse(&["--press", "w@40-90"]).unwrap();
        assert_eq!(held.presses, [(KeyCode::KeyW, 40, 90)]);
        let function = parse(&["--press", "F3@10"]).unwrap();
        assert_eq!(function.presses, [(KeyCode::F3, 10, 11)]);
        let escape = parse(&["--press", "esc@10"]).unwrap();
        assert_eq!(escape.presses, [(KeyCode::Escape, 10, 11)]);
        assert!(parse(&["--press", "F13@10"]).is_err());
        assert!(options.scripted());
        assert!(!parse(&[]).unwrap().scripted());
        let fixed = parse(&["--vantage", "4,3,20,6,2,0"]).unwrap();
        assert_eq!(
            fixed.vantage,
            Some((Vec3::new(4.0, 3.0, 20.0), Vec3::new(6.0, 2.0, 0.0)))
        );
        assert!(parse(&["--vantage", "1,2"]).unwrap_err().contains("six"));
        let placed = parse(&["--at", "-40,30", "--face", "-45, 31"]).unwrap();
        assert_eq!(placed.start_at, Some((-40.0, 30.0)));
        assert_eq!(placed.start_facing, Some((-45.0, 31.0)));
        assert!(
            parse(&["--face", "north"])
                .unwrap_err()
                .contains("east and south")
        );
        let warped = parse(&["--warp", "66,-62:66,-64.4@900"]).unwrap();
        assert_eq!(
            warped.warps,
            [Warp {
                frame: 900,
                at: (66.0, -62.0),
                facing: (66.0, -64.4)
            }]
        );
        assert!(
            parse(&["--warp", "66,-62@900"])
                .unwrap_err()
                .contains("like")
        );
    }

    #[test]
    fn explains_bad_options() {
        assert!(parse(&["--nope"]).unwrap_err().contains("Unknown option"));
        assert!(parse(&["--radius"]).unwrap_err().contains("needs a value"));
        assert!(parse(&["--radius", "big"]).unwrap_err().contains("number"));
        assert!(
            parse(&["--press", "ff@3"])
                .unwrap_err()
                .contains("like f@60")
        );
        assert!(parse(&["--press", "f"]).unwrap_err().contains("like f@60"));
    }
}
