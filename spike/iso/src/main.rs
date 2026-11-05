//! A spike: what a realistic 2.5D party game in the manner of Baldur's Gate II would look
//! like on our engine and assets. See README.md.

mod camera;
mod command;
mod mipmaps;
mod nav;
mod party;
mod scene;
mod terrain;
mod ui;

use bevy::diagnostic::FrameCount;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::window::{ExitCondition, WindowResolution};
use std::path::PathBuf;

/// `--shot <png>` draws offscreen and saves a still after `--frames`; `--demo` sends the
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
}

fn args() -> Args {
    let mut a = Args {
        shot: None,
        frames: 300,
        demo: false,
        demo_at: 120,
        hour: scene::Hour::Dusk,
        zoom: 15.0,
        focus: Vec2::new(-1.0, 1.0),
        size: (1920, 1080),
    };
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        let mut val = || it.next().expect("a value after the flag");
        match flag.as_str() {
            "--shot" => a.shot = Some(val().into()),
            "--frames" => a.frames = val().parse().unwrap(),
            "--demo" => a.demo = true,
            "--demo-at" => a.demo_at = val().parse().unwrap(),
            "--zoom" => a.zoom = val().parse().unwrap(),
            "--hour" => {
                a.hour = match val().as_str() {
                    "day" => scene::Hour::Day,
                    "night" => scene::Hour::Night,
                    _ => scene::Hour::Dusk,
                }
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
    a
}

fn main() {
    let args = args();
    let window = if args.shot.is_some() {
        None
    } else {
        Some(Window {
            title: "Terra · isometric spike".into(),
            resolution: WindowResolution::new(1600, 900),
            ..default()
        })
    };
    App::new()
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
        .add_plugins((
            mipmaps::plugin,
            terrain::plugin,
            scene::plugin,
            party::plugin,
            camera::plugin,
            command::plugin,
            ui::plugin,
        ))
        .add_systems(
            PostStartup,
            |mut gizmos: ResMut<GizmoConfigStore>, args: Res<Args>, mut commands: Commands| {
                gizmos.config_mut::<DefaultGizmoConfigGroup>().0.line.width = 2.5;
                commands.insert_resource(args.hour);
            },
        )
        .add_systems(Last, shoot)
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
    if frame.0 == args.frames {
        commands
            .spawn(Screenshot::image(image.0.clone()))
            .observe(save_to_disk(path.clone()));
    }
    if frame.0 == args.frames + 8 {
        exit.write(AppExit::Success);
    }
}
