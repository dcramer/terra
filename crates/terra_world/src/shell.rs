//! The parts every experiment gets for free: the window, overlays, inspectors, and the
//! scripted modes that let a run be checked without anyone watching.

use crate::debug::{FRAME_BUDGET_MS, STUTTER_MS};
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
    render::{RenderApp, renderer::RenderAdapterInfo, view::screenshot::save_to_disk},
    time::TimeUpdateStrategy,
    window::{CursorGrabMode, CursorOptions, PresentMode, PrimaryWindow, WindowLevel},
    winit::{UpdateMode, WinitSettings},
};
use bevy_inspector_egui::{bevy_egui::EguiPlugin, quick::WorldInspectorPlugin};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

mod picture;
pub use picture::{PICTURE_SIZE, Picture, RenderWaits, screenshot};

pub const USAGE: &str = "\
usage: terra_sandbox [options]

  --experiment <name>   switch on an experiment (repeatable)
  --list                list the experiments and exit
  --radius <metres>     planet radius (default 2000)
  --shot <file.png>     save a screenshot once the scene has settled, then exit
  --detail-sweep <from>-<to>@<frame>-<frame>
                        hold the clock and the wind still and sweep how far detail reaches,
                        every distance at which something changes its level of detail,
                        from one share of its own to another over those frames, so what
                        pops shows between frames of a still view (tools/sandbox/lodcheck.ts)
  --shots <prefix>@<frames>
                        save a screenshot on each of those frames, counted as --press
                        counts them, as <prefix>-<frame>.png, then exit: a list and
                        ranges, like out/walk@300-320,600, so what pops in or out shows
                        between one frame and the next; a range may step, 600-2400/2
  --clean               leave the frame-time readout off the screen, for pictures
  --mute                no sound
  --sound               sound under WSL too, where it is off by default
  --bench               measure frame times with vsync off, print them and how they hold
                        to the frame budget, then exit
  --bench-frames <n>    how many frames --bench times (default 600)
  --remote              let a script drive this run over Bevy's remote protocol, on
                        localhost:15702 (tools/sandbox/remote.ts)
  --remote-port <port>  listen on this port instead, so several runs can be driven apart
  --background          stay out of the way: open behind other windows without taking
                        focus, never take the mouse, ignore the real keyboard and
                        mouse, and stay silent; for runs a script drives
  --on-top              a scripted or background run floats above the other windows,
                        still without taking focus, so it is drawn when they would cover
                        it: macOS draws nothing of a hidden window, so its screenshots come
                        out black and its frame times are wrong
  --offscreen           a scripted or background run draws into a picture 1920x1080
                        instead of its window, which stays hidden: it works with the
                        screen asleep or locked, covers nothing, measures every machine at
                        the same size, and waits for the GPU each frame so a benchmark
                        times the GPU's work too
  --quality <preset>    draw at the low, medium or high quality preset, rather than the
                        one chosen for the GPU (an integrated GPU low, any other high)
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
  --press <key>@<frame> press a letter, digit, F1 to F12, esc, enter, backspace, space, ctrl, shift, equal, minus, pageup, pagedown or an arrow
                        (left, right, up, down) on a given frame, for scripted runs;
                        <key>@<from>-<to> holds it over a range of frames
  --give <item>:<count> start the play experiment carrying something extra, for
                        testing: a raw item such as stone:10, a tool such as
                        spear:1 or iron_axe:1, clothing to wear such as
                        leather_coat:1, or health, fed or stamina out of 100
                        (repeatable)
  --save <file>         where the play experiment keeps its saved game; scripted runs
                        save and load only when given this
  --calls <file.jsonl>  make remote calls on given frames of a scripted run, one a line,
                        {\"frame\": 120, \"method\": \"build.fill\", \"params\": {}}, counted
                        as --press counts them, through the methods --remote answers:
                        a film of building made the same every time (tools/sandbox/qa.ts)
  --frame-times <file.csv>
                        with --bench, write every timed frame's times, on the real clock
                        and the game's, the render passes' and the GPU's, one a line
  --trace <file.jsonl>  write the play experiment's whole state every frame, one line of
                        JSON each, to compare two runs (tools/sandbox/diverge.ts)
  --fresh               start the play experiment over, throwing away its saved game
  --no-lair             no lair of combat creatures (the spider nest) beside the place,
                        as every demo's and film's place (tools/sandbox/places.ts, D421)
  --free-build          building in the play experiment costs nothing, every cost met
                        as on the yard, though claims still hold: building with
                        pretend materials, as the QA films do (tools/sandbox/qa.ts)
  --level yard          play on the yard, a flat test level for building: no hills, sea,
                        trees, creatures or village; building is free and instant in
                        a claim that covers it; the sun stands at noon; nothing is saved
  --time <hour>         set the play experiment's clock to an hour of the day, 0 to 24,
                        such as 22 for night, for testing
  --date <month-day|day>
                        set the world's clock to a date, so its season can be seen: a
                        day of an Earth calendar, such as 10-15 for mid October, taken as
                        that share of the world's year after midwinter (21 December),
                        or a day of the clock itself, such as 0, when it starts, in late
                        August (docs/climate.md)
  --weather <name>      start on the first day with this weather, such as snowfall (D418)
  --site <place>        start at a place the land's rules make, found by its query:
                        lake, a mountain lake under the tree line with peaks a kilometre
                        over it, the start its shore, facing the water; tundra, level
                        ground past the trees over permafrost, facing the forest's edge;
                        town, level ground below the range for a market town, the forest
                        past its fields, the start beside it facing it (the town
                        experiment lays the town there); settlement, the same place
                        with only its plots cleared of the forest, for a settlement
                        founded there (D381); on a world with mountains
                        (--radius 30000) (docs/demos.md); <place>@<east km>,<north km>
                        pins it to the place its query finds there (D393), as the
                        demos' places are, so a change to the land elsewhere never
                        moves it
  --at <east,south>     start the play experiment's player so many metres east and south
                        of the start, for testing
  --face <east,south>   start the play experiment's player facing that spot, for testing
  --warp <east,south:east,south@frame>
                        on that frame, move the play experiment's player to the first
                        spot, facing the second, for testing (repeatable)
  --route <east,south>  walk the player to this spot, then the next given, steering as a
                        player would, for scripted walks such as benchmarks (repeatable)
  --walls plain         lay walls, floors and roofs plain rather than in logs, the
                        first kit (docs/building-kits.md)
  --kit-sheet           on the yard, lay one of each way log walls meet, built, going
                        up and as a blueprint, to look at (docs/building-kits.md)
  --sheet-rows <text>   only the kit sheet's rows whose names hold this, such as
                        hip or stone: L (repeatable)
  --village             on the yard, lay fifty buildings of every kind, roof and plan,
                        built, north of the start, to measure many in view
  --going-up            on the yard, lay a stone house's plan beside the start, hold the
                        camera on it and build it a step every tenth of a second, as
                        the hammer does, to measure building as it happens
  --timelapse <file>[@frame]
                        with the town experiment: lay the town's plan and build it up as
                        a time-lapse from that frame, its buildings a step at a time in the
                        hammer's order and the clock through the day, as the file in
                        assets/data/timelapse/ says (D381)
  --vantage <x,y,z,x,y,z[@frame]>
                        where a fixed camera stands and what it looks at, in metres
                        east, up and south of the start, for experiments that have
                        one; repeatable, each from its frame on, counted as --press
                        counts them, to picture several views in one run
  --vantage-ground      every --vantage's heights are above the ground under each
                        point, not above the start, as on rolling ground
  --cine <shot.json>[@frame]
                        film a shot (D357): the camera on the shot file's keyframed path,
                        lens, focus, aperture and exposure, rolling on that frame, counted
                        as --press counts them, and waiting on its first key before; a
                        route waits for it to roll; a run someone watches plays it over
                        and over (crates/terra_world/src/cine.rs, tools/sandbox/trailer.ts)
  --picture <width>x<height>
                        an offscreen run's picture this size rather than 1920x1080, such
                        as 2560x1440";

/// Engine messages that say what is normal, kept out of the log so what is left is worth
/// reading. Warnings and errors still come through. `RUST_LOG` replaces all of this.
const QUIET: &str = "\
bevy_render=warn,bevy_pbr=warn,bevy_winit=warn,bevy_egui=warn,bevy_diagnostic=warn,\
sctk_adwaita=off";

/// Frames to wait after loading before measuring or capturing by default, so the camera
/// and anything that eases in have settled.
const SETTLE_FRAMES: u32 = 120;
const BENCH_FRAMES: usize = 600;

/// A date to set the world's clock to (`--date`): the climate's, so tools without the engine read
/// dates the same way.
pub use terra_climate::Date;

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
    /// Screenshots on chosen frames of one run (`--shots`).
    pub shots: Option<Shots>,
    /// Detail swept over frames, the clock held still (`--detail-sweep`, D230).
    pub detail_sweep: Option<crate::detail::Sweep>,
    /// The yard's ground rolling gently rather than flat, as the lush forest stands on.
    pub rolling: bool,
    pub settle_frames: u32,
    pub bench: bool,
    /// Listen for a script driving the run (`remote`), and on which port.
    pub remote: bool,
    pub remote_port: u16,
    /// Advance the game by a fixed step every frame, as scripted runs do.
    pub steady: bool,
    /// Stay out of the way of the person at the machine, as scripted runs do.
    pub background: bool,
    /// An unattended run's window floats above the others, so it is drawn.
    pub on_top: bool,
    /// Draw into a picture instead of the window (`--offscreen`); unattended, no window opens.
    pub offscreen: bool,
    pub remote_token: String,
    /// No readout on screen.
    pub clean: bool,
    /// The quality preset asked for, rather than the GPU's (`--quality`).
    pub quality: Option<crate::quality::Quality>,
    /// Keep the loading screen up, to look at it.
    pub hold_loading: bool,
    /// No sound, and sound under WSL, where it is off unless asked for.
    pub mute: bool,
    pub sound: bool,
    /// A key, the frame it goes down, and the frame it comes back up.
    pub presses: Vec<(KeyCode, u32, u32)>,
    /// A fixed camera: where it stands and what it looks at, in an experiment's own space.
    pub vantage: Option<(Vec3, Vec3)>,
    /// Every fixed camera a scripted run was given, with the frame it moves there on,
    /// counted as `--press` counts them; one for frame 0 is where it starts.
    pub vantages: Vec<(u32, (Vec3, Vec3))>,
    /// The fixed cameras' heights are above the ground under each point rather than above
    /// the start, so a view at eye height stays at eye height on rolling ground.
    pub vantage_ground: bool,
    /// Things to start carrying, by name, for testing.
    pub gives: Vec<(String, u32)>,
    /// Where the play experiment keeps its saved game, if not the usual place.
    pub save: Option<PathBuf>,
    /// Where the play experiment writes its whole state every frame, to compare runs.
    pub trace: Option<PathBuf>,
    /// Remote calls a scripted run makes on given frames (`remote::Calls`).
    pub calls: Option<PathBuf>,
    /// Where a benchmark writes every timed frame's times.
    pub frame_times: Option<PathBuf>,
    /// Start over, throwing away any saved game.
    pub fresh: bool,
    /// No lair of combat creatures beside the place, as a demo's or film's (D421): play
    /// keeps it.
    pub no_lair: bool,
    /// Building costs nothing: every cost is met, as on the yard, but claims still hold.
    pub free_build: bool,
    /// Which level the play experiment is on: the world, or a test level.
    pub level: Level,
    /// What walls, floors and roofs are laid in: `logs`, the default, or `plain`
    /// (docs/building-kits.md).
    pub walls: Option<String>,
    /// On the yard: lay one of each way log walls meet, in each state (docs/building-kits.md).
    pub kit_sheet: bool,
    /// The kit sheet's rows to lay, those whose names hold any of these; all if none.
    pub sheet_rows: Vec<String>,
    /// On the yard: lay fifty buildings, every kind, roof and plan (docs/building-plan.md).
    pub village: bool,
    /// On the yard: build a stone house a step at a time, the camera on it (`--going-up`).
    pub going_up: bool,
    /// A settlement rising, its time-lapse's file and the frame it rolls on (`--timelapse`,
    /// D381).
    pub timelapse: Option<String>,
    /// On the yard: lay a town of so many buildings on a grid of streets, one base of a
    /// castle, or so many buildings in hamlets spread out to two kilometres, to measure
    /// building's drawing at scale (docs/development.md, measuring performance).
    pub town: Option<usize>,
    pub castle: bool,
    pub far: Option<usize>,
    /// A scripted run's fixed cameras glide from each to the next over the frames between,
    /// rather than jumping on its frame: a walk or a flight for a benchmark.
    pub glide: bool,
    /// The hour of the day the play experiment's clock is set to, if not its own.
    pub time: Option<f32>,
    /// The date the world's clock is set to, if not its own (`--date`).
    pub date: Option<Date>,
    /// A weather to start in (`--weather`): the first day from the date that has it (D418).
    pub weather: Option<String>,
    /// A place the land's rules make to start at, found by its query (`--site`): `lake`, a
    /// mountain lake under the tree line with peaks a kilometre over it, `tundra`, or `town`
    /// (docs/demos.md), pinned to a spot with `@<east km>,<north km>` (D393).
    pub site: Option<String>,
    /// Where the play experiment's player starts, east and south of the start in metres.
    pub start_at: Option<(f32, f32)>,
    /// What the play experiment's player starts facing, east and south of the start.
    pub start_facing: Option<(f32, f32)>,
    /// On a frame, put the play experiment's player at a spot, facing another.
    pub warps: Vec<Warp>,
    /// Spots for the player to walk to in turn, east and south of the start (`--route`).
    pub route: Vec<(f32, f32)>,
    /// How many frames `--bench` times.
    pub bench_frames: usize,
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
    /// A shot to film (`--cine`, `cine`).
    pub cine: Option<crate::cine::Shot>,
    /// How big an offscreen run's picture is (`--picture`).
    pub picture: UVec2,
}

/// A jump in a scripted run: on `frame`, stand `at` a spot facing `facing`, each in metres
/// east and south of the start.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Warp {
    pub frame: u32,
    pub at: (f32, f32),
    pub facing: (f32, f32),
}

/// A sequence of screenshots from one scripted run: the frames, counted as `--press`
/// counts them, each saved as `<prefix>-<frame>.png`.
#[derive(Clone, Debug, PartialEq)]
pub struct Shots {
    pub prefix: PathBuf,
    pub frames: Vec<u32>,
}

impl Shots {
    fn parse(text: &str) -> Result<Self, String> {
        let problem = || {
            format!("--shots wants a prefix and frames, like out/walk@300-320,600, not `{text}`")
        };
        let (prefix, list) = text.rsplit_once('@').ok_or_else(problem)?;
        let mut frames = Vec::new();
        for part in list.split(',') {
            let number = |n: &str| n.trim().parse::<u32>().map_err(|_| problem());
            // A range may take every so many frames: 600-2400/2.
            let (range, step) = part.split_once('/').unwrap_or((part, "1"));
            let step = number(step)?.max(1) as usize;
            match range.split_once('-') {
                Some((from, to)) => frames.extend((number(from)?..=number(to)?).step_by(step)),
                None => frames.push(number(range)?),
            }
        }
        frames.sort_unstable();
        frames.dedup();
        if prefix.is_empty() || frames.is_empty() || frames.len() > 20_000 {
            return Err(problem());
        }
        Ok(Self {
            prefix: PathBuf::from(prefix),
            frames,
        })
    }

    /// Where the screenshot of a frame goes.
    pub fn path(&self, frame: u32) -> PathBuf {
        let name = self
            .prefix
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        self.prefix.with_file_name(format!("{name}-{frame:05}.png"))
    }
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
            shots: None,
            detail_sweep: None,
            rolling: false,
            settle_frames: SETTLE_FRAMES,
            bench: false,
            remote: false,
            remote_port: 15702,
            steady: false,
            background: false,
            on_top: false,
            offscreen: false,
            remote_token: String::new(),
            clean: false,
            hold_loading: false,
            mute: false,
            sound: false,
            presses: Vec::new(),
            vantage: None,
            vantages: Vec::new(),
            vantage_ground: false,
            gives: Vec::new(),
            save: None,
            trace: None,
            calls: None,
            frame_times: None,
            fresh: false,
            no_lair: false,
            free_build: false,
            level: Level::World,
            walls: None,
            kit_sheet: false,
            sheet_rows: Vec::new(),
            village: false,
            going_up: false,
            town: None,
            timelapse: None,
            castle: false,
            far: None,
            glide: false,
            start_at: None,
            time: None,
            date: None,
            weather: None,
            site: None,
            start_facing: None,
            warps: Vec::new(),
            route: Vec::new(),
            bench_frames: BENCH_FRAMES,
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
            quality: None,
            cine: None,
            picture: PICTURE_SIZE,
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
                "--on-top" => options.on_top = true,
                "--offscreen" => options.offscreen = true,
                "--clean" => options.clean = true,
                "--quality" => {
                    let name = value("--quality")?;
                    options.quality = Some(
                        crate::quality::Quality::from_name(&name)
                            .ok_or(format!("--quality is low, medium or high, not `{name}`"))?,
                    );
                }
                "--loading" => options.hold_loading = true,
                "--mute" => options.mute = true,
                "--sound" => options.sound = true,
                "--fresh" => options.fresh = true,
                "--no-lair" => options.no_lair = true,
                "--free-build" => options.free_build = true,
                "--level" => {
                    options.level = match value("--level")?.as_str() {
                        "world" => Level::World,
                        "yard" => Level::Yard,
                        other => return Err(format!("--level is world or yard, not `{other}`")),
                    }
                }
                "--time" => {
                    let text = value("--time")?;
                    let hour = text.parse().ok().filter(|h: &f32| (0.0..=24.0).contains(h));
                    let wrong = format!("--time wants an hour from 0 to 24, not `{text}`");
                    options.time = Some(hour.ok_or(wrong)?);
                }
                "--date" => options.date = Some(Date::parse(&value("--date")?)?),
                "--weather" => options.weather = Some(value("--weather")?),
                "--site" => options.site = Some(value("--site")?),
                "--at" => options.start_at = Some(east_south("--at", &value("--at")?)?),
                "--face" => options.start_facing = Some(east_south("--face", &value("--face")?)?),
                "--route" => options
                    .route
                    .push(east_south("--route", &value("--route")?)?),
                "--bench-frames" => {
                    options.bench_frames = value("--bench-frames")?
                        .parse()
                        .ok()
                        .filter(|&frames| frames >= 100)
                        .ok_or_else(|| "--bench-frames needs a number, 100 or more".to_string())?;
                }
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
                "--calls" => options.calls = Some(PathBuf::from(value("--calls")?)),
                "--frame-times" => {
                    options.frame_times = Some(PathBuf::from(value("--frame-times")?))
                }
                "--shot" => options.shot = Some(PathBuf::from(value("--shot")?)),
                "--shots" => options.shots = Some(Shots::parse(&value("--shots")?)?),
                "--detail-sweep" => {
                    options.detail_sweep =
                        Some(crate::detail::Sweep::parse(&value("--detail-sweep")?)?)
                }
                "--walls" => {
                    options.walls = match value("--walls")?.as_str() {
                        kind @ ("logs" | "plain") => Some(kind.to_string()),
                        other => return Err(format!("--walls is logs or plain, not `{other}`")),
                    }
                }
                "--kit-sheet" => options.kit_sheet = true,
                "--sheet-rows" => options.sheet_rows.push(value("--sheet-rows")?),
                "--village" => options.village = true,
                "--vantage-ground" => options.vantage_ground = true,
                "--going-up" => options.going_up = true,
                "--timelapse" => options.timelapse = Some(value("--timelapse")?),
                "--town" => {
                    options.town = Some(
                        value("--town")?
                            .parse()
                            .map_err(|_| "--town wants a number of buildings".to_string())?,
                    )
                }
                "--castle" => options.castle = true,
                "--far" => {
                    options.far = Some(
                        value("--far")?
                            .parse()
                            .map_err(|_| "--far wants a number of buildings".to_string())?,
                    )
                }
                "--glide" => options.glide = true,
                "--cine" => options.cine = Some(crate::cine::Shot::read(&value("--cine")?)?),
                "--picture" => {
                    let text = value("--picture")?;
                    options.picture = text
                        .split_once('x')
                        .and_then(|(w, h)| Some(UVec2::new(w.parse().ok()?, h.parse().ok()?)))
                        .filter(|size| size.x >= 64 && size.y >= 64)
                        .ok_or(format!(
                            "--picture wants a width and height, like 2560x1440, not `{text}`"
                        ))?;
                }
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
                    let text = value("--vantage")?;
                    let (place, frame) = text.split_once('@').unwrap_or((&text, "0"));
                    let numbers: Vec<f32> = place
                        .split(',')
                        .filter_map(|number| number.trim().parse().ok())
                        .collect();
                    let (&[a, b, c, d, e, f], Ok(frame)) =
                        (numbers.as_slice(), frame.trim().parse::<u32>())
                    else {
                        return Err("--vantage needs six numbers, and may say from which frame, like 4,3,20,6,2,0 or 4,3,20,6,2,0@300".to_string());
                    };
                    let view = (Vec3::new(a, b, c), Vec3::new(d, e, f));
                    if frame == 0 {
                        options.vantage = Some(view);
                    }
                    options.vantages.push((frame, view));
                }
                other => return Err(format!("Unknown option `{other}`")),
            }
        }
        Ok(options)
    }

    /// Whether this run drives itself and exits, rather than waiting for a person.
    pub fn scripted(&self) -> bool {
        self.shot.is_some() || self.shots.is_some() || self.bench || self.sheet.is_some()
    }

    /// Whether the game draws its interface: not for pictures, from a fixed camera or in a
    /// shot (`--cine`), which show the scene alone.
    pub fn shows_interface(&self) -> bool {
        self.vantage.is_none() && self.cine.is_none()
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
pub const KEYS: &str = "a letter, digit, F1 to F12, esc, enter, backspace, space, ctrl, shift, equal, minus, pageup, pagedown or an arrow (left, right, up, down)";

/// A key a script can press, by name: one of [`KEYS`].
pub fn parse_key(key: &str) -> Option<KeyCode> {
    use KeyCode as K;
    #[rustfmt::skip]
    const LETTERS: [KeyCode; 26] = [
        K::KeyA, K::KeyB, K::KeyC, K::KeyD, K::KeyE, K::KeyF, K::KeyG, K::KeyH, K::KeyI,
        K::KeyJ, K::KeyK, K::KeyL, K::KeyM, K::KeyN, K::KeyO, K::KeyP, K::KeyQ, K::KeyR,
        K::KeyS, K::KeyT, K::KeyU, K::KeyV, K::KeyW, K::KeyX, K::KeyY, K::KeyZ,
    ];
    const DIGITS: [KeyCode; 10] = [
        K::Digit0,
        K::Digit1,
        K::Digit2,
        K::Digit3,
        K::Digit4,
        K::Digit5,
        K::Digit6,
        K::Digit7,
        K::Digit8,
        K::Digit9,
    ];
    const FUNCTIONS: [KeyCode; 12] = [
        K::F1,
        K::F2,
        K::F3,
        K::F4,
        K::F5,
        K::F6,
        K::F7,
        K::F8,
        K::F9,
        K::F10,
        K::F11,
        K::F12,
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
        None if key.eq_ignore_ascii_case("space") => KeyCode::Space,
        None if key.eq_ignore_ascii_case("ctrl") => KeyCode::ControlLeft,
        None if key.eq_ignore_ascii_case("shift") => KeyCode::ShiftLeft,
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
        if self.0.scripted() {
            stay_awake();
        }
        if self.0.unattended() {
            crate::gpu_turn::take(self.0.bench);
        }
        let mut options = self.0.clone();
        // A shot with a path holds the camera (`cine`), as a fixed camera does.
        if options
            .cine
            .as_ref()
            .is_some_and(|shot| shot.holds_camera())
        {
            options
                .vantage
                .get_or_insert((Vec3::new(0.0, 10.0, 10.0), Vec3::ZERO));
            options.vantage_ground = false;
        }
        let readout = !options.clean;
        let window = Window {
            title: "Terra sandbox".into(),
            // The application id the desktop knows the window by: an unattended run's its own, so
            // a desktop can put such windows out of the way (a Hyprland rule sends `terra-unattended`
            // to a hidden workspace) while a window someone plays in stays where they are.
            name: Some(
                if options.unattended() {
                    "terra-unattended"
                } else {
                    "terra"
                }
                .into(),
            ),
            // An unattended run needs nobody's attention, and should not take the keyboard
            // away from whatever is in front, nor sit on top of it.
            focused: !options.unattended(),
            window_level: match (options.unattended(), options.on_top) {
                (true, true) => WindowLevel::AlwaysOnTop,
                (true, false) => WindowLevel::AlwaysOnBottom,
                (false, _) => WindowLevel::Normal,
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
        // Pictures read on the CPU only have a source of their own (`data::CPU_ONLY`), which
        // must be there before the asset server is made.
        let assets_folder = crate::data::assets().to_string_lossy().into_owned();
        app.register_asset_source(
            crate::data::CPU_ONLY,
            bevy::asset::io::AssetSourceBuilder::platform_default(&assets_folder, None),
        );
        crate::cooked::register(app);
        let sound = options.sound_on();
        // Bevy's glTF loader behind a gate, so models loading together never nest on an IO
        // thread's stack (gltf_gate.rs); offscreen and unattended, no window (headless.rs).
        let windowless = options.offscreen && options.unattended();
        let engine = crate::gltf_gate::replace(crate::headless::engine(app, windowless))
            .set(WindowPlugin {
                primary_window: Some(window),
                ..default()
            })
            .set(AssetPlugin {
                file_path: crate::data::assets().to_string_lossy().into_owned(),
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
                crate::mipmaps::plugin,
                audio::plugin(sound),
                FreeCameraPlugin,
                EguiPlugin::default(),
                // On by default, screenshots included, so it is there when it is needed.
                debug::plugin(readout),
                crate::pacing::plugin,
                crate::gpu_memory::plugin,
            ))
            // F1: every entity, resource and asset in the world, editable live.
            .add_plugins(
                WorldInspectorPlugin::new().run_if(input_toggle_active(false, KeyCode::F1)),
            )
            .add_systems(
                PreUpdate,
                (scripted_presses, move_the_vantage)
                    .chain()
                    .after(InputSystems),
            )
            .add_systems(Update, scripted_run)
            .add_systems(PostUpdate, keep_the_mouse_free);
        if self.0.remote || self.0.calls.is_some() {
            crate::remote::plugin(app);
        }
        // Meshes share slabs of GPU memory, one set a vertex layout; a full slab is copied
        // into a bigger one, after which every mesh in it is prepared and batched again
        // (D283). Slabs are pages of at most 64 MiB, as engines keep fixed-size pages:
        // a slab starts at 4 MiB, doubles to 64 and, full there, another is begun beside it,
        // so a growth copies and batches again at most 32 MiB of meshes, a layout holds at
        // most one part-empty page, and a small layout no 64 MiB one (D420).
        // Uploads are spread over frames, 128 MiB a frame at most (Godot's staging pool),
        // rather than every texture and mesh of a place staged on its first frame.
        app.insert_resource(bevy::render::render_asset::RenderAssetBytesPerFrame::new(
            128 * 1024 * 1024,
        ));
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            use bevy::render::{
                mesh::allocator::MeshAllocatorSettings, slab_allocator::SlabAllocatorSettings,
            };
            render_app.insert_resource(MeshAllocatorSettings {
                slab_allocator_settings: SlabAllocatorSettings {
                    min_slab_size: 4 * 1024 * 1024,
                    max_slab_size: 64 * 1024 * 1024,
                    large_threshold: 64 * 1024 * 1024,
                    growth_factor: 2.0,
                },
                ..default()
            });
        }
        if self.0.offscreen && self.0.unattended() {
            picture::plugin(app);
        }
    }
}

/// In an unattended run, the keyboard and mouse are whatever the script says and nothing
/// else. The window is a real one, so without this, anything typed while it has focus
/// would steer the run.
pub(crate) fn scripted_presses(
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

/// A scripted run given several fixed cameras moves to each on its frame, so one run can
/// picture several views.
fn move_the_vantage(frame: Res<ScriptFrame>, mut options: ResMut<Options>) {
    if !frame.is_changed() {
        return;
    }
    if options.glide {
        if let Some(view) = glided(&options.vantages, frame.0) {
            options.vantage = Some(view);
        }
        return;
    }
    let next = options
        .vantages
        .iter()
        .find(|(at, _)| *at == frame.0 && *at > 0)
        .map(|(_, view)| *view);
    if let Some(view) = next {
        options.vantage = Some(view);
    }
}

/// Where a camera gliding through fixed cameras is on a frame: on the line between the
/// last it has passed and the next, or at the first before it and the last after.
fn glided(vantages: &[(u32, (Vec3, Vec3))], frame: u32) -> Option<(Vec3, Vec3)> {
    let mut keys: Vec<&(u32, (Vec3, Vec3))> = vantages.iter().collect();
    keys.sort_by_key(|(at, _)| *at);
    let after = keys.iter().position(|(at, _)| *at > frame);
    match after {
        None => keys.last().map(|(_, view)| *view),
        Some(0) => keys.first().map(|(_, view)| *view),
        Some(next) => {
            let (a, (from_a, to_a)) = *keys[next - 1];
            let (b, (from_b, to_b)) = *keys[next];
            let t = (frame - a) as f32 / (b - a) as f32;
            Some((from_a.lerp(from_b, t), to_a.lerp(to_b, t)))
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
    picture: Option<Res<Picture>>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut frame: Local<u32>,
    mut frame_ms: Local<Vec<(u32, f32)>>,
    mut part_ms: Local<[Vec<f32>; 3]>,
    parts: Option<Res<crate::debug::FrameParts>>,
    mut last_frame: Local<Option<Instant>>,
    (entities, meshes, images, mut grew, bare): (
        &bevy::ecs::entity::Entities,
        Res<Assets<Mesh>>,
        Res<Assets<Image>>,
        Local<Option<[usize; 3]>>,
        Option<Res<crate::cover::BareGround>>,
    ),
    (store, mut pass_ms, mut pass_triangles, mut timed): (
        Res<bevy::diagnostic::DiagnosticsStore>,
        Local<std::collections::BTreeMap<String, Vec<f32>>>,
        Local<std::collections::BTreeMap<String, Vec<f32>>>,
        Local<(Option<Instant>, usize)>,
    ),
    (drawn, mut drawn_counts): (Query<(&ViewVisibility, &Mesh3d)>, Local<Vec<[usize; 2]>>),
    mut exit: MessageWriter<AppExit>,
    (mut times, render_waits, quality): (
        Local<Vec<String>>,
        Option<Res<RenderWaits>>,
        Option<Res<crate::quality::Quality>>,
    ),
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
    if let Some(shots) = &options.shots {
        take_shots(
            shots,
            *frame,
            &mut commands,
            picture.as_deref(),
            bare.as_deref(),
            &options,
            &mut exit,
        );
    }
    let settle = options.settle_frames;
    if *frame <= settle {
        return;
    }
    if let Some(path) = &options.shot {
        if *frame == settle + 1 {
            if let Some(folder) = path.parent() {
                std::fs::create_dir_all(folder).ok();
            }
            // The run ends when the file is there, so one left from an earlier run must go.
            std::fs::remove_file(path).ok();
            commands
                .spawn(screenshot(picture.as_deref()))
                .observe(save_to_disk(path.clone()));
        }
        // Saving happens in the background, and how many frames that takes depends on
        // how fast frames are going by. Leave once the file is there, or give up.
        if *frame > settle + 1 && !options.bench {
            if path.exists() {
                println!("SANDBOX saved {}", path.display());
                finish(&mut exit, AppExit::Success);
            } else if *frame > settle + 3000 {
                println!("SANDBOX could not save {}", path.display());
                finish(&mut exit, AppExit::error());
            }
        }
    }
    if options.bench {
        // What the world holds, at the first timed frame and the last: what a long run
        // keeps adding and never lets go of shows here.
        let holds = [entities.len() as usize, meshes.len(), images.len()];
        grew.get_or_insert(holds);
        frame_ms.extend(real_ms.map(|ms| (*frame, ms)));
        // Every frame, one a line, to read a stretch's hitches beside what happened then.
        if options.frame_times.is_some()
            && let Some(ms) = real_ms
        {
            let part = |ms: Option<f32>| ms.map_or(String::new(), |ms| format!("{ms:.2}"));
            times.push(format!(
                "{},{ms:.2},{},{},{}",
                *frame,
                part(parts.as_ref().map(|p| p.game_ms)),
                part(parts.as_ref().and_then(|p| p.render_ms)),
                part(parts.as_ref().and_then(|p| p.gpu_ms)),
            ));
        }
        if let Some(parts) = &parts {
            part_ms[0].push(parts.game_ms);
            part_ms[1].extend(parts.render_ms);
            part_ms[2].extend(parts.gpu_ms);
        }
        // Each render pass's time on the GPU, to say which passes the frame goes on: for
        // each frame the GPU has timed, what each pass took, and nothing for a pass that did
        // not run, so one that ran once is not counted in every frame.
        let then = crate::debug::passes_timed(&store).filter(|&then| timed.0 != Some(then));
        if let Some(then) = then {
            timed.0 = Some(then);
            timed.1 += 1;
        }
        for diagnostic in store.iter() {
            let path = diagnostic.path().as_str();
            if let (Some(pass), Some(then)) = (
                path.strip_prefix("render/")
                    .and_then(|p| p.strip_suffix("/elapsed_gpu")),
                then,
            ) {
                let ms = crate::debug::timed_then(diagnostic, then).unwrap_or(0.0);
                let earlier = timed.1 - 1;
                let times = pass_ms
                    .entry(pass.to_string())
                    .or_insert_with(|| vec![0.0; earlier]);
                times.push(ms as f32);
            }
            // The triangles each pass sent to be drawn, where the GPU counts them.
            if let (Some(pass), Some(count)) = (
                path.strip_prefix("render/")
                    .and_then(|p| p.strip_suffix("/clipper_invocations")),
                diagnostic.value(),
            ) {
                pass_triangles
                    .entry(pass.to_string())
                    .or_default()
                    .push(count as f32);
            }
        }
        // How many meshes are drawn, and how many distinct ones (each an indirect draw),
        // now and then: in any view, the sun's included, after culling by distance.
        if frame.is_multiple_of(30) {
            let mut distinct = std::collections::HashSet::new();
            let mut instances = 0;
            for (visibility, mesh) in &drawn {
                if visibility.get() {
                    instances += 1;
                    distinct.insert(mesh.id());
                }
            }
            drawn_counts.push([instances, distinct.len()]);
        }
        let frames = options.bench_frames;
        if frame_ms.len() == frames {
            if let Some(path) = &options.frame_times {
                let text = format!("frame,ms,game_ms,render_ms,gpu_ms\n{}\n", times.join("\n"));
                match std::fs::write(path, text) {
                    Ok(()) => println!("SANDBOX frame times in {}", path.display()),
                    Err(error) => println!("SANDBOX could not write {}: {error}", path.display()),
                }
            }
            let in_order: Vec<f32> = frame_ms.iter().map(|(_, ms)| *ms).collect();
            // Drift is the game's own work, where leaks and a throttled machine show,
            // apart from what the view happens to hold; frames when that is not measured.
            let sustained = Sustained::of(
                &in_order,
                (part_ms[0].len() == frames).then_some(part_ms[0].as_slice()),
            );
            // The run in tenths, each its median, to see whether it climbs or jumps, and
            // whether the game's own work does.
            let by_tenths = |values: &[f32]| -> String {
                values
                    .chunks(frames.div_ceil(10))
                    .map(|part| {
                        let mut part = part.to_vec();
                        part.sort_by(f32::total_cmp);
                        format!("{:.1}", part[part.len() / 2])
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            };
            let tenths = by_tenths(&in_order);
            let game_tenths = (!part_ms[0].is_empty()).then(|| by_tenths(&part_ms[0]));
            frame_ms.sort_by(|a, b| a.1.total_cmp(&b.1));
            let size = match (&picture, window.single()) {
                (Some(_), _) => format!("{}x{} offscreen", options.picture.x, options.picture.y),
                (None, Ok(window)) => {
                    format!("{}x{}", window.physical_width(), window.physical_height())
                }
                (None, Err(_)) => "no window".to_string(),
            };
            println!(
                "SANDBOX frames on {} at {size}: median {:.2} ms, 99th percentile {:.2} ms, worst {:.2} ms",
                adapter.name,
                frame_ms[frames / 2].1,
                frame_ms[frames * 99 / 100].1,
                frame_ms[frames - 1].1
            );
            // What the run is judged by: the integrated GPU at the low preset by D239's
            // median, everything else by D25's budget (`cargo xtask bench`).
            println!(
                "SANDBOX drawn at the {} preset on a {:?} GPU",
                quality.map_or("high", |quality| quality.name()),
                adapter.device_type
            );
            println!(
                "SANDBOX against the {FRAME_BUDGET_MS:.1} ms budget: {:.2}% of frames over, {} hitches over {:.1} ms, the game's work in the last third {:+.1}% on the first",
                sustained.over_pct, sustained.hitches, STUTTER_MS, sustained.drift_pct
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
            // The render world's own side of the timed frames: a frame waiting little for
            // the GPU is held up by the CPU, one waiting long by the GPU.
            if let Some(waits) = render_waits.as_ref().and_then(|w| w.0.lock().ok()) {
                let timed = waits.len().saturating_sub(frame_ms.len());
                let (mut busy, mut waited): (Vec<f32>, Vec<f32>) =
                    waits[timed..].iter().map(|[b, w]| (*b, *w)).unzip();
                println!(
                    "SANDBOX render world, median: busy {} before waiting {} for the GPU",
                    median(&mut busy),
                    median(&mut waited)
                );
            }
            // The costliest passes on the GPU, by their medians.
            let mut passes: Vec<(f32, &str)> = pass_ms
                .iter_mut()
                .map(|(pass, ms)| {
                    ms.sort_by(f32::total_cmp);
                    (ms[ms.len() / 2], pass.as_str())
                })
                .collect();
            passes.sort_by(|a, b| b.0.total_cmp(&a.0));
            if !passes.is_empty() {
                let costliest: Vec<String> = passes
                    .iter()
                    .take(20)
                    .map(|(ms, pass)| format!("{pass} {ms:.2}"))
                    .collect();
                println!(
                    "SANDBOX GPU passes, medians in ms: {}",
                    costliest.join(", ")
                );
            }
            // What is drawn: triangles a frame by pass, meshes, and memory.
            let mut triangles: Vec<(f32, &str)> = pass_triangles
                .iter_mut()
                .map(|(pass, counts)| {
                    counts.sort_by(f32::total_cmp);
                    (counts[counts.len() / 2], pass.as_str())
                })
                .filter(|(count, _)| *count > 0.0)
                .collect();
            triangles.sort_by(|a, b| b.0.total_cmp(&a.0));
            if !triangles.is_empty() {
                let all: f32 = triangles.iter().map(|(count, _)| count).sum();
                let costliest: Vec<String> = triangles
                    .iter()
                    .take(8)
                    .map(|(count, pass)| format!("{pass} {:.2}", count / 1e6))
                    .collect();
                println!(
                    "SANDBOX triangles a frame, medians in millions: {:.2} in all; {}",
                    all / 1e6,
                    costliest.join(", ")
                );
            }
            let middle = |pick: usize| {
                let mut values: Vec<usize> = drawn_counts.iter().map(|c| c[pick]).collect();
                values.sort_unstable();
                values.get(values.len() / 2).copied().unwrap_or(0)
            };
            let value = |path: &str| {
                store
                    .iter()
                    .find(|d| d.path().as_str() == path)
                    .and_then(|d| d.value())
                    .unwrap_or(0.0)
            };
            println!(
                "SANDBOX drawn, medians: {} meshes, {} distinct; memory: process {:.0} MiB, meshes on the GPU {:.0} MiB in {:.0}",
                middle(0),
                middle(1),
                value("process/mem_usage") * 1024.0,
                value("mesh_allocator_slabs_size") / (1024.0 * 1024.0),
                value("mesh_allocator_allocations"),
            );
            // When the slowest frames came, counted as --press counts, to find what they
            // have in common.
            let slowest: Vec<String> = frame_ms
                .iter()
                .rev()
                .take(8)
                .map(|(frame, ms)| format!("{frame} ({ms:.1} ms)"))
                .collect();
            println!("SANDBOX slowest frames: {}", slowest.join(", "));
            println!("SANDBOX by tenths of the run, medians in ms: {tenths}");
            if let Some(game) = game_tenths {
                println!("SANDBOX the game's own work by tenths, medians in ms: {game}");
            }
            let [entities_then, meshes_then, images_then] = grew.unwrap_or(holds);
            println!(
                "SANDBOX the world over the timed frames: entities {entities_then} to {}, meshes {meshes_then} to {}, images {images_then} to {}",
                holds[0], holds[1], holds[2]
            );
            finish(&mut exit, AppExit::Success);
        }
    }
}

/// Takes the sequence's screenshot on each of its frames, and ends a run that is only for
/// them once every file is there. Files left from an earlier run go first, since the run
/// ends when the files are there.
fn take_shots(
    shots: &Shots,
    frame: u32,
    commands: &mut Commands,
    picture: Option<&Picture>,
    bare: Option<&crate::cover::BareGround>,
    options: &Options,
    exit: &mut MessageWriter<AppExit>,
) {
    if frame == 1 {
        if let Some(folder) = shots.path(0).parent() {
            std::fs::create_dir_all(folder).ok();
        }
        for &at in &shots.frames {
            std::fs::remove_file(shots.path(at)).ok();
        }
    }
    if shots.frames.binary_search(&frame).is_ok() {
        // How much of the ground near the camera nothing grows on, with each picture.
        if let Some(bare) = bare {
            println!(
                "SANDBOX frame {frame}: bare ground within {} m {:.1}%, paths {:.1}%{}",
                crate::cover::MEASURED_M,
                bare.bare * 100.0,
                bare.paths * 100.0,
                if bare.complete { "" } else { ", still growing" }
            );
        }
        commands
            .spawn(screenshot(picture))
            .observe(save_to_disk(shots.path(frame)));
    }
    let last = *shots.frames.last().expect("a sequence has frames");
    if frame <= last || options.bench || options.shot.is_some() {
        return;
    }
    if shots.frames.iter().all(|&at| shots.path(at).exists()) {
        println!(
            "SANDBOX saved {} screenshots as {}-*.png",
            shots.frames.len(),
            shots.prefix.display()
        );
        finish(exit, AppExit::Success);
    } else if frame > last + 3000 {
        println!(
            "SANDBOX could not save every screenshot of {}",
            shots.prefix.display()
        );
        finish(exit, AppExit::error());
    }
}

/// How a run of frames, in the order they were drawn, holds to the frame budget: the share
/// over it, the hitches (frames over twice it), and the drift, how much slower the last
/// third of the game's own work ran than the first, which leaks, heat and a throttled
/// machine show as. It is the game's work, not the frame, so a walk past more or fewer
/// trees is not taken for one.
#[derive(Debug, PartialEq)]
pub struct Sustained {
    pub over_pct: f32,
    pub hitches: usize,
    pub drift_pct: f32,
}

impl Sustained {
    pub fn of(frame_ms: &[f32], game_ms: Option<&[f32]>) -> Self {
        let over = frame_ms.iter().filter(|&&ms| ms > FRAME_BUDGET_MS).count();
        let hitches = frame_ms.iter().filter(|&&ms| ms > STUTTER_MS).count();
        let median = |part: &[f32]| {
            let mut part = part.to_vec();
            part.sort_by(f32::total_cmp);
            part.get(part.len() / 2).copied().unwrap_or(0.0)
        };
        let work = game_ms.unwrap_or(frame_ms);
        let third = work.len() / 3;
        let (first, last) = (median(&work[..third]), median(&work[work.len() - third..]));
        Self {
            over_pct: 100.0 * over as f32 / frame_ms.len().max(1) as f32,
            hitches,
            drift_pct: if first > 0.0 {
                100.0 * (last / first - 1.0)
            } else {
                0.0
            },
        }
    }
}

/// Keeps macOS from throttling a scripted run. A program none of whose windows can be seen,
/// as a scripted run's offscreen window cannot, is put into App Nap after some 40 s: its
/// threads run slower and its timers late, and a benchmark's frames came out three times as
/// long from then on. A run the person started says it is latency-critical work they asked
/// for, as benchmarks and games do, which keeps it out of App Nap until it ends.
#[cfg(target_os = "macos")]
fn stay_awake() {
    use objc2_foundation::{NSActivityOptions, NSProcessInfo, NSString};
    let activity = NSProcessInfo::processInfo().beginActivityWithOptions_reason(
        NSActivityOptions::UserInitiated | NSActivityOptions::LatencyCritical,
        &NSString::from_str("a scripted run of the game, timed or captured"),
    );
    // The activity lasts as long as the program; ending it is ending the run.
    std::mem::forget(activity);
}

#[cfg(not(target_os = "macos"))]
fn stay_awake() {}

/// How long a finished scripted run may take to close before it is ended.
const CLOSING_S: u64 = 5;

/// Ends a scripted run whose work is done. Closing normally is given a few seconds, for
/// whatever happens as the game closes; then the process is ended, because Bevy's
/// shutdown sometimes never finishes, waiting on its render thread (seen on macOS).
fn finish(exit: &mut MessageWriter<AppExit>, how: AppExit) {
    let code = if how.is_success() { 0 } else { 1 };
    exit.write(how);
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(CLOSING_S));
        println!("SANDBOX took over {CLOSING_S} s to close, so it was ended");
        std::process::exit(code);
    });
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

    #[test]
    fn a_run_holds_to_the_budget_by_its_share_over_its_hitches_and_its_drift() {
        // Three thirds: steady at 8 ms, then two frames over budget and one hitch, then 10 ms.
        let mut frames = vec![8.0; 30];
        frames.extend([8.0, 20.0, 40.0, 8.0, 8.0, 8.0, 8.0, 8.0, 8.0, 8.0]);
        frames.extend(vec![10.0; 30]);
        let sustained = Sustained::of(&frames, None);
        assert_eq!(sustained.hitches, 1);
        assert!((sustained.over_pct - 200.0 / 70.0).abs() < 1e-3);
        assert!((sustained.drift_pct - 25.0).abs() < 1e-3);
        // With the game's work measured, the drift is its.
        let game = vec![2.0; 70];
        assert_eq!(Sustained::of(&frames, Some(&game)).drift_pct, 0.0);
        assert_eq!(Sustained::of(&[], None).hitches, 0);
    }

    #[test]
    fn a_route_is_its_spots_in_order_and_a_long_bench_its_frames() {
        let options = parse(&[
            "--route",
            "-58,-39",
            "--route",
            "10,-80",
            "--bench-frames",
            "3600",
        ])
        .unwrap();
        assert_eq!(options.route, [(-58.0, -39.0), (10.0, -80.0)]);
        assert_eq!(options.bench_frames, 3600);
        assert!(parse(&["--bench-frames", "5"]).is_err());
    }

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
        let sequence = parse(&["--shots", "out/walk@300-302,600,301"]).unwrap();
        let shots = sequence.shots.unwrap();
        assert_eq!(shots.frames, [300, 301, 302, 600]);
        let every_other = parse(&["--shots", "out/film@10-16/2"])
            .unwrap()
            .shots
            .unwrap();
        assert_eq!(every_other.frames, [10, 12, 14, 16]);
        assert_eq!(shots.path(301), PathBuf::from("out/walk-00301.png"));
        assert!(
            sequence.shot.is_none() && parse(&["--shots", "out/walk@300-302"]).unwrap().scripted()
        );
        assert!(
            parse(&["--shots", "out/walk"])
                .unwrap_err()
                .contains("--shots")
        );
        assert!(
            parse(&["--shots", "out/walk@x"])
                .unwrap_err()
                .contains("--shots")
        );
        let fixed = parse(&["--vantage", "4,3,20,6,2,0"]).unwrap();
        assert_eq!(
            fixed.vantage,
            Some((Vec3::new(4.0, 3.0, 20.0), Vec3::new(6.0, 2.0, 0.0)))
        );
        assert!(parse(&["--vantage", "1,2"]).unwrap_err().contains("six"));
        let moving = parse(&["--vantage", "1,2,3,4,5,6", "--vantage", "7,8,9,1,2,3@300"]).unwrap();
        assert_eq!(
            moving.vantage,
            Some((Vec3::new(1.0, 2.0, 3.0), Vec3::new(4.0, 5.0, 6.0)))
        );
        assert_eq!(moving.vantages.len(), 2);
        assert_eq!(moving.vantages[1].0, 300);
        // Gliding, halfway between them halfway through, held at either end.
        let at = |frame| glided(&moving.vantages, frame).unwrap().0;
        assert_eq!(at(150), Vec3::new(4.0, 5.0, 6.0));
        assert_eq!(at(400), Vec3::new(7.0, 8.0, 9.0));
        assert!(parse(&["--glide", "--town", "300"]).unwrap().glide);
        assert!(
            parse(&["--vantage", "1,2,3,4,5,6@x"])
                .unwrap_err()
                .contains("six")
        );
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
        // A key let go before it is pressed, or on the same frame.
        assert!(parse(&["--press", "w@90-40"]).is_err());
        assert!(parse(&["--press", "w@40-40"]).is_err());
    }

    #[test]
    fn reads_the_play_options_and_refuses_what_is_out_of_range() {
        let play = parse(&[
            "--time", "22", "--give", "stone:10", "--give", "spear:1", "--level", "yard",
            "--walls", "plain",
        ])
        .unwrap();
        assert_eq!(play.time, Some(22.0));
        assert_eq!(
            play.gives,
            [("stone".to_string(), 10), ("spear".to_string(), 1)]
        );
        assert!(play.yard());
        assert_eq!(play.walls.as_deref(), Some("plain"));
        assert!(!parse(&["--level", "world"]).unwrap().yard());
        // The hour runs from 0 to 24, both ends included.
        assert_eq!(parse(&["--time", "0"]).unwrap().time, Some(0.0));
        assert_eq!(parse(&["--time", "24"]).unwrap().time, Some(24.0));
        for hour in ["24.5", "-1", "night"] {
            assert!(parse(&["--time", hour]).unwrap_err().contains("0 to 24"));
        }
        for give in ["stone", "stone:lots", "stone:-1"] {
            assert!(parse(&["--give", give]).unwrap_err().contains("stone:10"));
        }
        assert!(
            parse(&["--level", "moon"])
                .unwrap_err()
                .contains("world or yard")
        );
        assert!(
            parse(&["--walls", "stone"])
                .unwrap_err()
                .contains("logs or plain")
        );
    }

    #[test]
    fn a_date_is_a_day_of_the_clock_or_of_a_calendar() {
        let year = terra_climate::Year {
            days: 365.0,
            tilt_deg: 23.44,
            start: 0.6768,
        };
        assert_eq!(parse(&["--date", "40"]).unwrap().date, Some(Date::Day(40)));
        assert_eq!(Date::Day(-3).day(&year), -3);
        // Midwinter's day starts the year, its middle half a day on.
        assert_eq!(
            parse(&["--date", "12-21"]).unwrap().date,
            Some(Date::OfYear(0.5 / 365.0))
        );
        let date = |text: &str| parse(&["--date", text]).unwrap().date.unwrap();
        // Counted another way: the clock starts on the 25th of August, the 247th day after
        // midwinter (10 in December, 31 in January and so on), 0.6768 of the way round.
        assert_eq!(date("8-25").day(&year), 0);
        assert_eq!(date("10-15").day(&year), 51);
        assert_eq!(date("1-15").day(&year), 143);
        // And a date a year on from the start comes before the year turns again.
        for text in ["8-24", "3-1", "12-21", "12-20"] {
            let day = date(text).day(&year);
            assert!((0..365).contains(&day), "{text}: {day}");
            let Date::OfYear(share) = date(text) else {
                unreachable!()
            };
            let gap = (year.phase(day) as f64 - share).rem_euclid(1.0);
            assert!(gap.min(1.0 - gap) < 1.5 / 365.0, "{text}: {day}");
        }
        for wrong in ["13-1", "2-30", "oct", "10-", "0-5"] {
            assert!(parse(&["--date", wrong]).unwrap_err().contains("10-15"));
        }
    }
}
