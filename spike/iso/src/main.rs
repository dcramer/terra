//! A spike: what a realistic 2.5D party game in the manner of Baldur's Gate II would look
//! like on our engine and assets. See README.md.

mod camera;
mod camp;
mod command;
mod foliage;
mod forest;
mod kit;
mod mipmaps;
mod motes;
mod nav;
mod marsh;
mod party;
mod pool;
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
}

/// A film's frame rate in game time, whatever the machine draws.
pub const FILM_FPS: f64 = 30.0;

fn args() -> (Args, Option<(kit::biome::Biome, kit::light::Light)>) {
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
    };
    let mut hour = None;
    let mut it = std::env::args().skip(1);
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
    let loaded = biome(&mut a);
    a.hour = hour.unwrap_or(a.hour);
    (a, loaded)
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

fn main() {
    let (args, loaded) = args();
    let window = if args.shot.is_some() {
        None
    } else {
        Some(Window {
            title: "Terra · isometric spike".into(),
            resolution: WindowResolution::new(1600, 900),
            ..default()
        })
    };
    let mut app = App::new();
    app
        // The spike's own models (the knight), beside the repository's assets.
        .register_asset_source(
            "spike",
            bevy::asset::io::AssetSourceBuilder::platform_default("spike/iso/assets", None),
        )
        .insert_resource(args.clone())
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: window,
            exit_condition: if args.shot.is_some() {
                ExitCondition::DontExit
            } else {
                ExitCondition::OnPrimaryClosed
            },
            ..default()
        }))
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
        app.add_plugins((mipmaps::plugin, view::plugin)).run();
        return;
    }
    use kit::biome::Layout;
    if let Some((biome, light)) = loaded {
        app.insert_resource(biome).insert_resource(light);
    }
    if args.layout == Some(Layout::Ruin) {
        app.add_plugins((mipmaps::plugin, rim::plugin, ruin::plugin))
            .run();
        return;
    }
    if args.layout == Some(Layout::Marsh) {
        // The glade's ground material, then the marsh's own land over it.
        app.add_plugins((mipmaps::plugin, terrain::plugin, rim::plugin, marsh::plugin))
            .run();
        return;
    }
    if args.layout == Some(Layout::Forest) {
        // The glade's land, wood, pond and beasts without its party, camera and orders.
        app.add_plugins((
            mipmaps::plugin,
            terrain::plugin,
            scene::plugin,
            forest::plugin,
            foliage::plugin,
            party::animate,
            rim::plugin,
            water::plugin,
            wildlife::plugin,
            (motes::plugin, smoke::plugin),
            wood::plugin,
        ))
        .add_systems(PostStartup, |args: Res<Args>, mut commands: Commands| {
            commands.insert_resource(args.hour);
        })
        .run();
        return;
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
    .run();
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
    if frame.0 == args.frames + args.film + 30 {
        exit.write(AppExit::Success);
    }
}
