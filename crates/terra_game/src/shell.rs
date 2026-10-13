//! The parts every experiment gets for free: the window, overlays, inspectors, and the
//! scripted modes that let a run be checked without anyone watching.

use crate::{
    audio, debug,
    loading::{self, Screen},
    record,
};
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
    window::{CursorGrabMode, CursorOptions, PresentMode, WindowLevel},
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
  --remote              let a script drive this run over Bevy's remote protocol, on
                        localhost:15702 (tools/sandbox/remote.ts)
  --remote-port <port>  listen on this port instead, so several runs can be driven apart
  --background          stay out of the way: open behind other windows without taking
                        focus, never take the mouse, ignore the real keyboard and
                        mouse, and stay silent; for runs a script drives
  --steady              advance the game a sixtieth of a second every frame, however
                        fast frames are drawn, as scripted runs do
  --remote-token <text> what this run answers sandbox.whoami with, so a script knows it is
                        talking to the run it started
  --settle <frames>     frames to run before capturing or measuring, counted from when
                        loading is done (default 120)
  --loading             keep the loading screen up, to look at it
  --model <path>        in the workbench: a model to load, under assets/; repeatable,
                        to stand several side by side
  --prop <name>         in the workbench: an item of the build catalogue, such as Forge,
                        drawn as placed in play and stood beside the models, its size
                        printed; repeatable, or all for every item
  --anim <clip>         in the workbench: a clip to play; repeatable
  --hold <item>         in the workbench: an item's held look in the right hand, named as
                        in --give, such as stone_axe
  --wear <item>         in the workbench: an item's look, on every model, named as in
                        --give, such as leather_coat (repeatable)
  --template <name>[@<height>]
                        in the workbench: the template the next model's body is shaped
                        as, such as female or heavy, and its height against the base's,
                        such as male@1.15 (repeatable, in the models' order)
  --crowd <n>           in the crowd experiment: how many characters (100 by default)
  --linear-skinning     skin with Bevy's own linear blend instead of dual quaternions,
                        to compare the two (crates/terra_game/src/skinning.rs)
  --joints              in the workbench: print where each model's joints and sockets
                        stand once shaped, to compare with the export's
  --attach <file>[@<socket>]
                        in the workbench: any look, on a socket such as socket.back, or
                        without one as its file says (repeatable)
  --focus <name>        in the workbench: look closely at a part, such as socket.hand.R
  --sheet <prefix>      in the workbench: capture each clip, save one picture of it per
                        clip as <prefix>-<clip>.png, then exit
  --renders <folder>    in the icon baker: also write every item's render here, for
                        painting (tools/icons)
  --keep <item>         in the icon baker: keep an item's painted icon though its look's
                        file changed (moved, or given morphs), recording the look as it is
                        now; only after checking the icon still matches it (repeatable)
  --sides <degrees,…>   in the workbench sheet: directions to look from, round from the
                        front (default 30,120,210,300)
  --moments <fraction,…>
                        in the workbench sheet: how far through each clip to look
                        (default 0.4)
  --press <key>@<frame> press a letter, digit, F1 to F12, esc, enter, backspace, equal, minus, pageup, pagedown or an arrow
                        (left, right, up, down) on a given frame, for scripted runs;
                        <key>@<from>-<to> holds it over a range of frames
  --give <item>:<count> start the play experiment carrying something extra, for
                        testing: a raw item such as stone:10, a tool such as
                        spear:1 or iron_axe:1, clothing to wear such as
                        leather_coat:1, or health, fed or stamina out of 100
                        (repeatable)
  --save <file>         where the play experiment keeps its saved game; scripted runs
                        save and load only when given this
  --trace <file.jsonl>  write the play experiment's whole state every frame, one line of
                        JSON each, to compare two runs (tools/sandbox/diverge.ts)
  --fresh               start the play experiment over, throwing away its saved game
  --level yard          play on the yard, a flat test level for building: no hills, sea,
                        trees, creatures or village; building is free and instant in
                        a claim that covers it; the sun stands at noon; nothing is saved
  --time <hour>         set the play experiment's clock to an hour of the day, 0 to 24,
                        such as 22 for night, for testing
  --at <east,south>     start the play experiment's player so many metres east and south
                        of the start, for testing
  --face <east,south>   start the play experiment's player facing that spot, for testing
  --warp <east,south:east,south@frame>
                        on that frame, move the play experiment's player to the first
                        spot, facing the second, for testing (repeatable)
  --walls plain         lay walls, floors and roofs plain rather than in logs, the
                        first kit (docs/kits.md)
  --kit-sheet           on the yard, lay one of each way log walls meet, built, going
                        up and as a blueprint, to look at (docs/kits.md)
  --vantage <x,y,z,x,y,z>
                        where a fixed camera stands and what it looks at, in metres
                        east, up and south of the base's corner, for experiments
                        that have one";

/// Engine messages that say what is normal, kept out of the log so what is left is worth
/// reading. Warnings and errors still come through. `RUST_LOG` replaces all of this.
const QUIET: &str = "\
bevy_render=warn,bevy_pbr=warn,bevy_winit=warn,bevy_egui=warn,bevy_diagnostic=warn,\
sctk_adwaita=off";

/// Frames to wait after loading before measuring or capturing by default, so the camera
/// and anything that eases in have settled.
const SETTLE_FRAMES: u32 = 120;
const BENCH_FRAMES: usize = 600;

/// Which level the play experiment is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Level {
    /// The world, as a player has it.
    #[default]
    World,
    /// A flat test level for working on building: smooth ground and nothing on it,
    /// building free and instant in a claim that covers it, the sun at noon, never saved.
    Yard,
}

impl Options {
    /// Whether this is the yard, the flat test level.
    pub fn yard(&self) -> bool {
        self.level == Level::Yard
    }
}

/// A run condition: the play experiment is in the world, not on a test level.
pub fn in_the_world(options: Res<Options>) -> bool {
    !options.yard()
}

#[derive(Resource, Clone, Debug)]
pub struct Options {
    pub experiments: Vec<String>,
    pub list: bool,
    pub radius_m: f32,
    pub shot: Option<PathBuf>,
    pub settle_frames: u32,
    pub bench: bool,
    /// Listen for a script driving the run (`remote`), and on which port.
    pub remote: bool,
    pub remote_port: u16,
    /// Advance the game by a fixed step every frame, as scripted runs do.
    pub steady: bool,
    /// Stay out of the way of the person at the machine, as scripted runs do.
    pub background: bool,
    pub remote_token: String,
    /// No readout on screen.
    pub clean: bool,
    /// Keep the loading screen up, to look at it.
    pub hold_loading: bool,
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
    /// Where the play experiment writes its whole state every frame, to compare runs.
    pub trace: Option<PathBuf>,
    /// Start over, throwing away any saved game.
    pub fresh: bool,
    /// Which level the play experiment is on: the world, or a test level.
    pub level: Level,
    /// What walls, floors and roofs are laid in: `logs`, the default, or `plain`
    /// (docs/kits.md).
    pub walls: Option<String>,
    /// On the yard: lay one of each way log walls meet, in each state (docs/kits.md).
    pub kit_sheet: bool,
    /// The hour of the day the play experiment's clock is set to, if not its own.
    pub time: Option<f32>,
    /// Where the play experiment's player starts, east and south of the start in metres.
    pub start_at: Option<(f32, f32)>,
    /// What the play experiment's player starts facing, east and south of the start.
    pub start_facing: Option<(f32, f32)>,
    /// On a frame, put the play experiment's player at a spot, facing another.
    pub warps: Vec<Warp>,
    /// In the workbench: the models to load, their clips to play, what they hold and
    /// wear, what to look at closely, and where to save a sheet, from which sides, at
    /// which moments.
    pub models: Vec<String>,
    /// In the workbench: items of the builder's catalogue to stand beside the models, by
    /// name, or `all`.
    pub props: Vec<String>,
    pub hold: Option<String>,
    pub attaches: Vec<String>,
    pub wears: Vec<String>,
    /// In the workbench: the template each model's body is shaped as, in the order of the
    /// models (`appearance`); with `joints`, where its joints and sockets stand once shaped.
    pub templates: Vec<String>,
    /// Skin with Bevy's linear blend rather than dual quaternions (`skinning`), to compare.
    pub linear_skinning: bool,
    /// In the crowd experiment: how many characters.
    pub crowd: Option<usize>,
    pub joints: bool,
    pub focus: Option<String>,
    pub sides: Vec<f32>,
    pub moments: Vec<f32>,
    pub anims: Vec<String>,
    pub sheet: Option<PathBuf>,
    pub renders: Option<PathBuf>,
    /// In the icon baker: items whose painted icons still match their looks, though the
    /// looks' files changed, so they are recorded as made from the looks as they are now.
    pub keeps: Vec<String>,
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
            remote: false,
            remote_port: 15702,
            steady: false,
            background: false,
            remote_token: String::new(),
            clean: false,
            hold_loading: false,
            mute: false,
            sound: false,
            presses: Vec::new(),
            vantage: None,
            gives: Vec::new(),
            save: None,
            trace: None,
            fresh: false,
            level: Level::World,
            walls: None,
            kit_sheet: false,
            start_at: None,
            time: None,
            start_facing: None,
            warps: Vec::new(),
            models: Vec::new(),
            props: Vec::new(),
            hold: None,
            attaches: Vec::new(),
            wears: Vec::new(),
            templates: Vec::new(),
            linear_skinning: false,
            crowd: None,
            joints: false,
            focus: None,
            sides: Vec::new(),
            moments: Vec::new(),
            anims: Vec::new(),
            sheet: None,
            renders: None,
            keeps: Vec::new(),
        };
        let mut args = args.peekable();
        while let Some(arg) = args.next() {
            let mut value = |name: &str| args.next().ok_or(format!("{name} needs a value"));
            match arg.as_str() {
                "--experiment" => options.experiments.push(value("--experiment")?),
                "--list" => options.list = true,
                "--bench" => options.bench = true,
                "--remote" => options.remote = true,
                "--remote-token" => options.remote_token = value("--remote-token")?,
                "--remote-port" => {
                    options.remote = true;
                    options.remote_port = value("--remote-port")?
                        .parse()
                        .map_err(|_| "--remote-port needs a port number".to_string())?;
                }
                "--steady" => options.steady = true,
                "--background" => options.background = true,
                "--clean" => options.clean = true,
                "--loading" => options.hold_loading = true,
                "--mute" => options.mute = true,
                "--sound" => options.sound = true,
                "--fresh" => options.fresh = true,
                "--level" => {
                    options.level = match value("--level")?.as_str() {
                        "world" => Level::World,
                        "yard" => Level::Yard,
                        other => return Err(format!("--level is world or yard, not `{other}`")),
                    }
                }
                "--time" => {
                    let text = value("--time")?;
                    let hour = text
                        .parse::<f32>()
                        .ok()
                        .filter(|hour| (0.0..=24.0).contains(hour))
                        .ok_or_else(|| {
                            format!("--time wants an hour from 0 to 24, like 22, not `{text}`")
                        })?;
                    options.time = Some(hour);
                }
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
                "--trace" => options.trace = Some(PathBuf::from(value("--trace")?)),
                "--shot" => options.shot = Some(PathBuf::from(value("--shot")?)),
                "--walls" => {
                    options.walls = match value("--walls")?.as_str() {
                        kind @ ("logs" | "plain") => Some(kind.to_string()),
                        other => return Err(format!("--walls is logs or plain, not `{other}`")),
                    }
                }
                "--kit-sheet" => options.kit_sheet = true,
                "--model" => options.models.push(value("--model")?),
                "--prop" => options.props.push(value("--prop")?),
                "--hold" => options.hold = Some(value("--hold")?),
                "--attach" => options.attaches.push(value("--attach")?),
                "--wear" => options.wears.push(value("--wear")?),
                "--template" => options.templates.push(value("--template")?),
                "--joints" => options.joints = true,
                "--linear-skinning" => options.linear_skinning = true,
                "--crowd" => {
                    options.crowd = Some(value("--crowd")?.parse().map_err(|_| {
                        "--crowd wants how many characters, such as 500".to_string()
                    })?)
                }
                "--focus" => options.focus = Some(value("--focus")?),
                "--sides" => options.sides = numbers("--sides", &value("--sides")?)?,
                "--moments" => options.moments = numbers("--moments", &value("--moments")?)?,
                "--anim" => options.anims.push(value("--anim")?),
                "--sheet" => options.sheet = Some(PathBuf::from(value("--sheet")?)),
                "--renders" => options.renders = Some(PathBuf::from(value("--renders")?)),
                "--keep" => options.keeps.push(value("--keep")?),
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
        self.shot.is_some() || self.bench || self.sheet.is_some()
    }

    /// Whether this run belongs to a script rather than a person, so it keeps out of the
    /// way: a scripted run, or one a script drives (`--background`).
    pub fn unattended(&self) -> bool {
        self.scripted() || self.background
    }

    /// Whether anything is heard. An unattended run is silent, so it neither plays sounds
    /// over whatever the person at the machine is doing nor needs a sound device. WSL
    /// usually has no sound device, and its sound library fills the log saying so, so
    /// sound is off there unless asked for.
    pub fn sound_on(&self) -> bool {
        !self.unattended() && !self.mute && (self.sound || !on_wsl())
    }
}

/// Numbers written like 30,120,210.
fn numbers(name: &str, text: &str) -> Result<Vec<f32>, String> {
    text.split(',')
        .map(|number| number.trim().parse().ok())
        .collect::<Option<Vec<f32>>>()
        .filter(|numbers| !numbers.is_empty())
        .ok_or(format!("{name} wants numbers like 30,120, not `{text}`"))
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
    let problem =
        || format!("--press wants {KEYS} and a frame, like f@60, F3@10 or w@40-90, not `{text}`");
    let (key, frames) = text.split_once('@').ok_or_else(problem)?;
    let code = parse_key(key).ok_or_else(problem)?;
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

/// The keys a script can press, in words.
pub const KEYS: &str = "a letter, digit, F1 to F12, esc, enter, backspace, equal, minus, pageup, pagedown or an arrow (left, right, up, down)";

/// A key a script can press, by name: one of [`KEYS`].
pub fn parse_key(key: &str) -> Option<KeyCode> {
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
    Some(match function {
        Some(&code) => code,
        None if key.eq_ignore_ascii_case("esc") => KeyCode::Escape,
        None if key.eq_ignore_ascii_case("enter") => KeyCode::Enter,
        None if key.eq_ignore_ascii_case("backspace") => KeyCode::Backspace,
        None if key.eq_ignore_ascii_case("left") => KeyCode::ArrowLeft,
        None if key.eq_ignore_ascii_case("right") => KeyCode::ArrowRight,
        None if key.eq_ignore_ascii_case("up") => KeyCode::ArrowUp,
        None if key.eq_ignore_ascii_case("down") => KeyCode::ArrowDown,
        None if key.eq_ignore_ascii_case("equal") => KeyCode::Equal,
        None if key.eq_ignore_ascii_case("minus") => KeyCode::Minus,
        None if key.eq_ignore_ascii_case("pageup") => KeyCode::PageUp,
        None if key.eq_ignore_ascii_case("pagedown") => KeyCode::PageDown,
        None => {
            let symbol = key
                .chars()
                .next()
                .filter(|c| key.len() == 1 && c.is_ascii_alphanumeric());
            let symbol = symbol?.to_ascii_lowercase();
            match symbol.to_digit(10) {
                Some(digit) => DIGITS[digit as usize],
                None => LETTERS[(symbol as u8 - b'a') as usize],
            }
        }
    })
}

pub struct ShellPlugin(pub Options);

impl Plugin for ShellPlugin {
    fn build(&self, app: &mut App) {
        let options = self.0.clone();
        let readout = !options.clean;
        let window = Window {
            title: "Terra sandbox".into(),
            // An unattended run needs nobody's attention, and should not take the keyboard
            // away from whatever is in front, nor sit on top of it.
            focused: !options.unattended(),
            window_level: if options.unattended() {
                WindowLevel::AlwaysOnBottom
            } else {
                WindowLevel::Normal
            },
            resolution: (1600u32, 900u32).into(),
            // Vsync hides how long a frame really takes, so benchmarks turn it off.
            present_mode: if options.bench {
                PresentMode::AutoNoVsync
            } else {
                PresentMode::AutoVsync
            },
            ..default()
        };
        if options.unattended() {
            // Its window does not take focus, and a window out of focus is normally only
            // drawn sixty times a second. An unattended run is drawn as fast as it will go.
            app.insert_resource(WinitSettings {
                focused_mode: UpdateMode::Continuous,
                unfocused_mode: UpdateMode::Continuous,
            });
        }
        if options.scripted() || options.steady {
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
                // What the log keeps is kept in memory too, for a script to ask about.
                custom_layer: record::layer,
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
                loading::plugin,
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
            .add_systems(Update, scripted_run)
            .add_systems(PostUpdate, keep_the_mouse_free);
        if self.0.remote {
            crate::remote::plugin(app);
        }
    }
}

/// In an unattended run, the keyboard and mouse are whatever the script says and nothing
/// else. The window is a real one, so without this, anything typed while it has focus
/// would steer the run.
fn scripted_presses(
    options: Res<Options>,
    screen: Res<State<Screen>>,
    mut frame: ResMut<ScriptFrame>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut motion: ResMut<AccumulatedMouseMotion>,
    mut scroll: ResMut<AccumulatedMouseScroll>,
) {
    if !options.unattended() {
        return;
    }
    keys.reset_all();
    mouse.reset_all();
    motion.delta = Vec2::ZERO;
    scroll.delta = Vec2::ZERO;
    // A run driven live presses its keys over the remote protocol, after this.
    if !options.scripted() {
        return;
    }
    // A script's frames are counted from when loading is done, however long that took.
    if !counting(&options, &screen) {
        return;
    }
    frame.0 += 1;
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

/// An unattended run never takes the mouse, whatever an experiment asks for: in play a
/// held mouse is fenced in and put back in the middle of the window every frame, which
/// would take the pointer from the person at the machine. Runs after experiments ask, so
/// the window is only ever told to let go.
fn keep_the_mouse_free(options: Res<Options>, mut cursor: Single<&mut CursorOptions>) {
    if options.unattended() && cursor.grab_mode != CursorGrabMode::None {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    }
}

fn scripted_run(
    mut commands: Commands,
    options: Res<Options>,
    screen: Res<State<Screen>>,
    adapter: Res<RenderAdapterInfo>,
    mut frame: Local<u32>,
    mut frame_ms: Local<Vec<f32>>,
    mut part_ms: Local<[Vec<f32>; 3]>,
    parts: Option<Res<crate::debug::FrameParts>>,
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
    if !counting(&options, &screen) {
        return;
    }
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
        if let Some(parts) = &parts {
            part_ms[0].push(parts.game_ms);
            part_ms[1].extend(parts.render_ms);
            part_ms[2].extend(parts.gpu_ms);
        }
        if frame_ms.len() == BENCH_FRAMES {
            frame_ms.sort_by(|a, b| a.total_cmp(b));
            println!(
                "SANDBOX frames on {}: median {:.2} ms, 99th percentile {:.2} ms, worst {:.2} ms",
                adapter.name,
                frame_ms[BENCH_FRAMES / 2],
                frame_ms[BENCH_FRAMES * 99 / 100],
                frame_ms[BENCH_FRAMES - 1]
            );
            // Where the time goes: the medians of the game's systems, the render passes on
            // the CPU, and the GPU, so a slow frame says which to work on.
            let median = |v: &mut Vec<f32>| {
                v.sort_by(|a, b| a.total_cmp(b));
                v.get(v.len() / 2)
                    .map_or("-".to_string(), |m| format!("{m:.2} ms"))
            };
            println!(
                "SANDBOX frame parts, median: game {}, render CPU {}, GPU {}",
                median(&mut part_ms[0]),
                median(&mut part_ms[1]),
                median(&mut part_ms[2])
            );
            exit.write(AppExit::Success);
        }
    }
}

/// Whether a scripted run's frames are being counted: once loading is done, or while the
/// loading screen is held up to be looked at.
pub fn counting(options: &Options, screen: &State<Screen>) -> bool {
    *screen.get() == Screen::Playing || options.hold_loading
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
    fn reads_the_workbench_options() {
        let options = parse(&[
            "--model",
            "a.glb",
            "--model",
            "b.glb",
            "--attach",
            "pack.glb@socket.back",
            "--sides",
            "30, 120",
            "--moments",
            "0.25,0.75",
            "--remote",
            "--background",
        ])
        .unwrap();
        assert_eq!(options.models, ["a.glb", "b.glb"]);
        assert_eq!(options.attaches, ["pack.glb@socket.back"]);
        assert_eq!(options.sides, [30.0, 120.0]);
        assert_eq!(options.moments, [0.25, 0.75]);
        assert!(options.remote);
        assert!(options.unattended() && !options.scripted() && !options.sound_on());
        assert!(parse(&["--sides", "front"]).is_err());
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
