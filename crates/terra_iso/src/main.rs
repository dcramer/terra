//! A spike: what a realistic 2.5D party game in the manner of Baldur's Gate II would look
//! like on our engine and assets. See README.md.

// Bevy systems take their inputs as arguments, so long argument lists and nested query
// types are normal here, as in terra_sandbox.
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

mod camera;
mod camp;
mod command;
mod demo;
mod foliage;
mod forest;
mod hero;
mod kit;
mod marsh;
mod mipmaps;
mod motes;
mod nav;
mod party;
mod play;
mod rest;
mod rim;
mod ruin;
mod scene;
mod smoke;
mod terrain;
mod ui;
mod view;
mod water;
mod wildlife;
mod wood;

use bevy::diagnostic::FrameCount;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::window::{ExitCondition, WindowResolution};
use std::path::PathBuf;

/// `--shot <png>` draws offscreen and saves a still after `--frames`, on the film's fixed step
/// (`FILM_FPS`), so the same code gives the same still; `--demo` sends the
/// party across the square at `--demo-at`; `--hour day|dusk|night`; `--zoom <metres>`;
/// `--focus x,z`; `--size WxH`.
#[derive(Resource, Clone)]
pub struct Args {
    pub shot: Option<PathBuf>,
    pub frames: u32,
    pub demo: bool,
    pub demo_at: u32,
    pub hour: scene::Hour,
    pub zoom: f32,
    pub focus: Vec2,
    pub size: (u32, u32),
    /// `--biome NAME`: the biome file `biomes/NAME.ron` (kit/, D634), whose layout says
    /// which scene runs; `--scene ruin|forest|marsh` is the same, `--scene glade` (or none)
    /// the party's glade. Its light preset sets the hour unless `--hour` says.
    pub biome: Option<String>,
    /// The biome's layout, once its file is read (`None`: the glade).
    pub layout: Option<kit::biome::Layout>,
    /// `--film N` with `--shot dir/f.jpg`: N frames from `--frames` on, at a fixed 30 per
    /// second of game time, saved as dir/f-0000.jpg on; clips play rather than hold.
    pub film: u32,
    /// `--asset NAME` with `--shot dir/NAME.png`: the asset view (view.rs), one asset alone;
    /// with `--film N`, N frames of its walk and run as the game plays them, dir/NAME-f0000.jpg on.
    pub asset: Option<String>,
    /// `--play`: the hero is the player (play.rs), the default when a window opens on a
    /// biome; a `--shot`/`--film` run plays only with it, and `--press wd@120-160` (keys
    /// held over those frames) and `--click X,Y@170` (a left click at those pixels on that
    /// frame) script it.
    pub play: bool,
    pub press: Vec<(String, u32, u32)>,
    pub clicks: Vec<(Vec2, u32)>,
    /// `--do NAME[:X,Z]@FROM[-TO]`: one of the hero's abilities (hero/abilities.rs, `Ability`:
    /// light, heavy, block, roll, walk, sprint) pressed at FROM, or held to TO, aimed at the
    /// ground point X,Z. `--route NAME` reads a scripted fight's flags from
    /// `routes/NAME.txt` (`routes`).
    pub abilities: Vec<(String, Option<Vec2>, u32, u32)>,
}

impl Args {
    /// A still: every clip and effect held at its set moment (not while playing).
    pub fn still(&self) -> bool {
        self.shot.is_some() && self.film == 0 && !self.play
    }
}

/// A film's frame rate in game time, whatever the machine draws.
pub const FILM_FPS: f64 = 30.0;

/// Where a film without `--shot` saves its frames.
const FILM_DIR: &str = "target/film";

impl Args {
    /// A film always runs offscreen and ends: given no `--shot`, its frames go to
    /// `FILM_DIR/f-NNNN.jpg`; without one it opened a window and never exited (a worker job
    /// held its slot for two hours, iso-forest-demo j1).
    fn film_saves(&mut self) {
        if self.film > 0 && self.shot.is_none() && self.asset.is_none() {
            self.shot = Some(PathBuf::from(FILM_DIR).join("f.jpg"));
        }
    }

    /// The frame a run with no window exits on: its still's or last film frame's, and 30
    /// more for the saves to land. `None` for a window, which runs until it is closed.
    pub fn last_frame(&self) -> Option<u32> {
        self.shot.as_ref().map(|_| self.frames + self.film + 30)
    }
}

fn args() -> (Args, Option<(kit::biome::Biome, kit::light::Light)>) {
    let (mut a, hour) = parse(std::env::args().skip(1));
    let loaded = biome(&mut a);
    a.hour = hour.unwrap_or(a.hour);
    a.play = loaded.is_some() && (a.play || a.shot.is_none());
    a.film_saves();
    if let Some(dir) = a.shot.as_ref().and_then(|s| s.parent())
        && dir == std::path::Path::new(FILM_DIR)
    {
        std::fs::create_dir_all(dir).expect("making the film's folder");
        eprintln!("--film without --shot: frames go to {FILM_DIR}/f-NNNN.jpg");
    }
    (a, loaded)
}

/// The flags with each `--route NAME` replaced by its file's: `routes/NAME.txt` beside this
/// crate (a path ending `.txt` is read as it is), the flags `cargo iso` takes, whitespace
/// apart, `#` to the line's end a note. So a scripted fight is a file in the repository, one
/// command to play or film, not a copied line (D249); flags after it override its own.
fn routes(flags: impl Iterator<Item = String>) -> Vec<String> {
    let mut out = Vec::new();
    let mut it = flags;
    while let Some(flag) = it.next() {
        if flag != "--route" {
            out.push(flag);
            continue;
        }
        let name = it.next().expect("--route NAME (routes/NAME.txt)");
        let path = if name.ends_with(".txt") {
            PathBuf::from(&name)
        } else {
            PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/routes"))
                .join(format!("{name}.txt"))
        };
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            eprintln!(
                "--route {name}: {}: {e}. A route is crates/terra_iso/routes/<name>.txt, the \
                 flags cargo iso takes (forest.txt is one): check the name or write the file",
                path.display()
            );
            std::process::exit(2)
        });
        out.extend(route_flags(&text));
    }
    out
}

/// A route file's flags: its words, with `#` notes left out.
fn route_flags(text: &str) -> impl Iterator<Item = String> + '_ {
    text.lines()
        .map(|l| l.split_once('#').map_or(l, |(f, _)| f))
        .flat_map(str::split_whitespace)
        .map(String::from)
}

/// The flags as given, and the `--hour` if one was.
fn parse(flags: impl Iterator<Item = String>) -> (Args, Option<scene::Hour>) {
    let mut a = Args {
        shot: None,
        frames: 300,
        demo: false,
        demo_at: 120,
        hour: scene::Hour::Dusk,
        zoom: 14.0,
        focus: Vec2::new(-1.0, 1.0),
        size: (1920, 1080),
        biome: None,
        layout: None,
        film: 0,
        asset: None,
        play: false,
        press: Vec::new(),
        clicks: Vec::new(),
        abilities: Vec::new(),
    };
    let mut hour = None;
    let mut it = routes(flags).into_iter();
    while let Some(flag) = it.next() {
        let mut val = || it.next().expect("a value after the flag");
        match flag.as_str() {
            "--shot" => a.shot = Some(val().into()),
            "--frames" => a.frames = val().parse().unwrap(),
            "--film" => a.film = val().parse().unwrap(),
            "--asset" => a.asset = Some(val()),
            "--demo" => a.demo = true,
            "--biome" => a.biome = Some(val()),
            "--scene" => {
                let name = val();
                a.biome = (name != "glade").then_some(name);
            }
            "--play" => a.play = true,
            "--press" => {
                let v = val();
                let (keys, span) = v.split_once('@').expect("--press KEYS@FROM-TO");
                let (from, to) = span.split_once('-').expect("--press KEYS@FROM-TO");
                a.press
                    .push((keys.into(), from.parse().unwrap(), to.parse().unwrap()));
            }
            "--click" => {
                let v = val();
                let (xy, at) = v.split_once('@').expect("--click X,Y@FRAME");
                let p: Vec<f32> = xy.split(',').map(|s| s.parse().unwrap()).collect();
                a.clicks.push((Vec2::new(p[0], p[1]), at.parse().unwrap()));
            }
            "--do" => {
                let v = val();
                let (what, span) = v.split_once('@').expect("--do NAME[:X,Z]@FROM[-TO]");
                let (name, at) = match what.split_once(':') {
                    Some((n, xz)) => {
                        let p: Vec<f32> = xz.split(',').map(|s| s.parse().unwrap()).collect();
                        (n, Some(Vec2::new(p[0], p[1])))
                    }
                    None => (what, None),
                };
                if hero::Ability::named(name).is_none() {
                    let all: Vec<_> = hero::Ability::ALL.iter().map(|a| a.name()).collect();
                    panic!(
                        "--do {name}: no such ability; the hero's are {all:?} (hero/abilities.rs)"
                    );
                }
                let (from, to) = match span.split_once('-') {
                    Some((f, t)) => (f.parse().unwrap(), t.parse().unwrap()),
                    None => (span.parse().unwrap(), span.parse::<u32>().unwrap() + 1),
                };
                a.abilities.push((name.into(), at, from, to));
            }
            "--demo-at" => a.demo_at = val().parse().unwrap(),
            "--zoom" => a.zoom = val().parse().unwrap(),
            "--hour" => {
                hour = Some(match val().as_str() {
                    "day" => scene::Hour::Day,
                    "night" => scene::Hour::Night,
                    _ => scene::Hour::Dusk,
                })
            }
            "--focus" => {
                let v: Vec<f32> = val().split(',').map(|s| s.parse().unwrap()).collect();
                a.focus = Vec2::new(v[0], v[1]);
            }
            "--size" => {
                let v: Vec<u32> = val().split('x').map(|s| s.parse().unwrap()).collect();
                a.size = (v[0], v[1]);
            }
            other => panic!("unknown flag {other}: see the Args doc in main.rs"),
        }
    }
    (a, hour)
}

/// The biome `--biome` names and its light, setting the run's layout and hour (the light's);
/// a file that does not read stops the run with what to fix (kit/mod.rs).
fn biome(args: &mut Args) -> Option<(kit::biome::Biome, kit::light::Light)> {
    let name = args.biome.as_deref()?;
    let read = kit::biome::Biome::load(name).and_then(|b| Ok((b.light()?, b)));
    let (light, biome) = read.unwrap_or_else(|e| {
        eprintln!("--biome {name}: {e}");
        std::process::exit(2)
    });
    args.hour = light.hour.scene();
    args.layout = Some(biome.layout);
    Some((biome, light))
}

/// Its exit is the app's: a run's gate (demo/tally.rs) fails it with an error code.
fn main() -> AppExit {
    let (args, loaded) = args();
    let window = if args.shot.is_some() {
        None
    } else {
        Some(Window {
            title: "Terra · isometric".into(),
            resolution: WindowResolution::new(1600, 900),
            ..default()
        })
    };
    let mut app = App::new();
    app
        // The experiment's own models (the knight), beside the repository's assets.
        .register_asset_source(
            "iso",
            bevy::asset::io::AssetSourceBuilder::platform_default("crates/terra_iso/assets", None),
        )
        .insert_resource(args.clone())
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: window,
                    exit_condition: if args.shot.is_some() {
                        ExitCondition::DontExit
                    } else {
                        ExitCondition::OnPrimaryClosed
                    },
                    ..default()
                })
                // A still or film compiles each pipeline in the frame that asks for it, so
                // what frame N shows never waits on the driver: compiled in the background,
                // a mesh is skipped until its pipeline is ready, and on a cold shader cache
                // (the first shot after a build) neon's paving was still black at frame 300
                // (uc-infra j3: one of three noise shots, the rest of the frame alike).
                .set(bevy::render::RenderPlugin {
                    synchronous_pipeline_compilation: args.shot.is_some(),
                    ..default()
                })
                // terra_walker brings the workspace's file_watcher feature (the hero, D635);
                // the experiment never watched its files.
                .set(AssetPlugin {
                    watch_for_changes_override: Some(false),
                    ..default()
                }),
        )
        .add_systems(Last, shoot);
    // A still runs on the film's fixed step too, so the same code gives the same still: in
    // real time the fires' flicker moved a still's p95 by 0.0025 between shots (iso-kit j1),
    // past the loop's "unchanged ties" bar (D634).
    if args.shot.is_some() {
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_secs_f64(1.0 / FILM_FPS),
        ));
    }
    if args.asset.is_some() {
        return app.add_plugins((mipmaps::plugin, view::plugin)).run();
    }
    use kit::biome::Layout;
    if let Some((biome, light)) = loaded {
        app.insert_resource(biome).insert_resource(light);
        // A biome's reflection probe, if its file names one (kit/reflect.rs).
        app.add_plugins(kit::reflect::plugin);
    }
    if args.play {
        app.add_plugins((play::plugin, demo::plugin));
    }
    if args.layout == Some(Layout::Ruin) {
        return app
            .add_plugins((mipmaps::plugin, rim::plugin, ruin::plugin))
            .run();
    }
    if args.layout == Some(Layout::Street) {
        // The rim for the street's people (a prop's `rim`).
        return app
            .add_plugins((mipmaps::plugin, rim::plugin, kit::street::plugin))
            .run();
    }
    if args.layout == Some(Layout::Marsh) {
        // The glade's ground material, then the marsh's own land over it.
        return app
            .add_plugins((mipmaps::plugin, terrain::plugin, rim::plugin, marsh::plugin))
            .run();
    }
    if args.layout == Some(Layout::Forest) {
        // The glade's land, wood, pond and beasts without its party, camera and orders.
        return app
            .add_plugins((
                mipmaps::plugin,
                terrain::plugin,
                scene::plugin,
                forest::plugin,
                foliage::plugin,
                party::animate,
                rim::plugin,
                water::plugin,
                wildlife::plugin,
                // The camp fire's smoke is the ruin's flipbook fire's (wood::smoulder), not the
                // glade's column (smoke.rs).
                motes::plugin,
                wood::plugin,
            ))
            .add_systems(PostStartup, |args: Res<Args>, mut commands: Commands| {
                commands.insert_resource(args.hour);
            })
            .run();
    }
    app.add_plugins((
        mipmaps::plugin,
        terrain::plugin,
        scene::plugin,
        forest::plugin,
        foliage::plugin,
        party::plugin,
        camera::plugin,
        command::plugin,
        ui::plugin,
        rim::plugin,
        water::plugin,
        wildlife::plugin,
        (motes::plugin, smoke::plugin),
        camp::plugin,
        rest::plugin,
    ))
    .add_systems(
        PostStartup,
        |mut gizmos: ResMut<GizmoConfigStore>, args: Res<Args>, mut commands: Commands| {
            let (config, _) = gizmos.config_mut::<DefaultGizmoConfigGroup>();
            config.line.width = 1.8;
            config.depth_bias = -1.0;
            commands.insert_resource(args.hour);
        },
    )
    .run()
}

fn shoot(
    args: Res<Args>,
    frame: Res<FrameCount>,
    offscreen: Option<Res<camera::Offscreen>>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let (Some(path), Some(image)) = (&args.shot, offscreen) else {
        return;
    };
    if args.asset.is_some() {
        return; // the asset view shoots its own (view.rs)
    }
    let n = frame.0.wrapping_sub(args.frames);
    if args.film > 0 && n < args.film {
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("f");
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("jpg");
        let file = path.with_file_name(format!("{stem}-{n:04}.{ext}"));
        commands
            .spawn(Screenshot::image(image.0.clone()))
            .observe(save_to_disk(file));
    } else if args.film == 0 && frame.0 == args.frames {
        commands
            .spawn(Screenshot::image(image.0.clone()))
            .observe(save_to_disk(path.clone()));
    }
    if Some(frame.0) == args.last_frame() {
        exit.write(AppExit::Success);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flags(s: &str) -> Args {
        let (mut a, _) = parse(s.split(' ').map(String::from));
        a.film_saves();
        a
    }

    /// A film without `--shot` runs offscreen (no window: `shot` set) and exits after its
    /// frames, as one with `--shot` does; a still exits after its frame, a window never.
    /// Its fault, the film left without a shot as before, has no last frame: it ran forever.
    #[test]
    fn a_film_ends_after_its_frames() {
        let film = flags("--biome forest --play --film 12 --frames 40");
        assert!(film.shot.is_some(), "a film opens no window");
        assert_eq!(film.last_frame(), Some(40 + 12 + 30));
        let shot = flags("--film 12 --frames 40 --shot target/job/x/f.jpg");
        assert_eq!(shot.last_frame(), film.last_frame());
        assert_eq!(shot.shot, Some(PathBuf::from("target/job/x/f.jpg")));
        assert_eq!(flags("--shot s.png").last_frame(), Some(300 + 30));
        assert_eq!(flags("--biome forest").last_frame(), None);
        let (unsaved, _) = parse("--film 12".split(' ').map(String::from));
        assert_eq!(
            unsaved.last_frame(),
            None,
            "the fault: a film left as before"
        );
    }

    /// `--route forest` reads routes/forest.txt: its notes left out, its flags parsed as if
    /// given (the biome, the film, the presses and the aimed roll), and a flag after it wins.
    #[test]
    fn a_route_file_is_its_flags() {
        let text = "--biome forest # the place\n# a note --do heavy@9\n--press w@60-95\n\
                    --do roll:11.5,19.8@1024 --frames 60";
        let words: Vec<_> = route_flags(text).collect();
        assert_eq!(
            words,
            [
                "--biome",
                "forest",
                "--press",
                "w@60-95",
                "--do",
                "roll:11.5,19.8@1024",
                "--frames",
                "60"
            ]
        );
        let (forest, _) = parse(
            "--route forest --frames 1230 --film 0"
                .split(' ')
                .map(String::from),
        );
        assert_eq!(forest.biome.as_deref(), Some("forest"));
        assert!(forest.play);
        assert_eq!(
            (forest.frames, forest.film),
            (1230, 0),
            "flags after it win"
        );
        assert!(forest.press.contains(&("w".into(), 60, 95)));
        let roll = forest.abilities.iter().find(|a| a.0 == "roll");
        assert_eq!(
            roll.map(|r| (r.1, r.2)),
            Some((Some(Vec2::new(11.5, 19.8)), 1024))
        );
        assert!(forest.abilities.len() > 30, "the route's every --do");
    }
}
