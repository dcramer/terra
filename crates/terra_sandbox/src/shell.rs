//! The parts every experiment gets for free: the window, overlays, inspectors, and the
//! scripted modes that let a run be checked without anyone watching.

use bevy::{
    camera_controller::free_camera::FreeCameraPlugin,
    dev_tools::fps_overlay::FpsOverlayPlugin,
    input::{InputSystems, common_conditions::input_toggle_active},
    prelude::*,
    render::{
        renderer::RenderAdapterInfo,
        view::screenshot::{Screenshot, save_to_disk},
    },
    window::PresentMode,
};
use bevy_inspector_egui::{bevy_egui::EguiPlugin, quick::WorldInspectorPlugin};
use std::path::PathBuf;

pub const USAGE: &str = "\
usage: terra_sandbox [options]

  --experiment <name>   switch on an experiment (repeatable)
  --list                list the experiments and exit
  --radius <metres>     planet radius (default 2000)
  --shot <file.png>     save a screenshot once the scene has settled, then exit
  --bench               measure frame times with vsync off, print them, then exit
  --press <key>@<frame> press a letter key on a given frame, for scripted runs";

/// Frames to wait before measuring or capturing, so shaders have compiled and the scene
/// has finished loading.
const SETTLE_FRAMES: u32 = 120;
const BENCH_FRAMES: usize = 600;

#[derive(Resource, Clone, Debug)]
pub struct Options {
    pub experiments: Vec<String>,
    pub list: bool,
    pub radius_m: f32,
    pub shot: Option<PathBuf>,
    pub bench: bool,
    pub presses: Vec<(KeyCode, u32)>,
}

impl Options {
    pub fn from_args(args: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut options = Options {
            experiments: Vec::new(),
            list: false,
            radius_m: 2000.0,
            shot: None,
            bench: false,
            presses: Vec::new(),
        };
        let mut args = args.peekable();
        while let Some(arg) = args.next() {
            let mut value = |name: &str| args.next().ok_or(format!("{name} needs a value"));
            match arg.as_str() {
                "--experiment" => options.experiments.push(value("--experiment")?),
                "--list" => options.list = true,
                "--bench" => options.bench = true,
                "--shot" => options.shot = Some(PathBuf::from(value("--shot")?)),
                "--radius" => {
                    options.radius_m = value("--radius")?
                        .parse()
                        .map_err(|_| "--radius needs a number of metres".to_string())?;
                }
                "--press" => options.presses.push(parse_press(&value("--press")?)?),
                other => return Err(format!("Unknown option `{other}`")),
            }
        }
        Ok(options)
    }

    /// Whether this run drives itself and exits, rather than waiting for a person.
    pub fn scripted(&self) -> bool {
        self.shot.is_some() || self.bench
    }
}

fn parse_press(text: &str) -> Result<(KeyCode, u32), String> {
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
    let problem = || format!("--press wants a letter and a frame, like f@60, not `{text}`");
    let (key, frame) = text.split_once('@').ok_or_else(problem)?;
    let letter = key
        .chars()
        .next()
        .filter(|c| key.len() == 1 && c.is_ascii_alphabetic());
    let letter = letter.ok_or_else(problem)?.to_ascii_lowercase();
    let frame = frame.parse().map_err(|_| problem())?;
    Ok((LETTERS[(letter as u8 - b'a') as usize], frame))
}

pub struct ShellPlugin(pub Options);

impl Plugin for ShellPlugin {
    fn build(&self, app: &mut App) {
        let options = self.0.clone();
        let window = Window {
            title: "Terra sandbox".into(),
            resolution: (1600u32, 900u32).into(),
            // Vsync hides how long a frame really takes, so benchmarks turn it off.
            present_mode: if options.bench {
                PresentMode::AutoNoVsync
            } else {
                PresentMode::AutoVsync
            },
            ..default()
        };
        app.insert_resource(options)
            .insert_resource(ClearColor(Color::srgb(0.01, 0.01, 0.02)))
            .add_plugins(DefaultPlugins.set(WindowPlugin {
                primary_window: Some(window),
                ..default()
            }))
            .add_plugins((
                FreeCameraPlugin,
                FpsOverlayPlugin::default(),
                EguiPlugin::default(),
            ))
            // F1: every entity, resource and asset in the world, editable live.
            .add_plugins(
                WorldInspectorPlugin::new().run_if(input_toggle_active(false, KeyCode::F1)),
            )
            .add_systems(PreUpdate, scripted_presses.after(InputSystems))
            .add_systems(Update, scripted_run);
    }
}

/// Presses keys on the frames asked for, through the same input path a keyboard uses.
fn scripted_presses(
    options: Res<Options>,
    mut frame: Local<u32>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
) {
    *frame += 1;
    for &(key, on_frame) in &options.presses {
        if *frame == on_frame {
            keys.press(key);
        } else if *frame == on_frame + 1 {
            keys.release(key);
        }
    }
}

fn scripted_run(
    mut commands: Commands,
    options: Res<Options>,
    time: Res<Time<Real>>,
    adapter: Res<RenderAdapterInfo>,
    mut frame: Local<u32>,
    mut frame_ms: Local<Vec<f32>>,
    mut exit: MessageWriter<AppExit>,
) {
    if !options.scripted() {
        return;
    }
    *frame += 1;
    if *frame <= SETTLE_FRAMES {
        return;
    }
    if let Some(path) = &options.shot {
        if *frame == SETTLE_FRAMES + 1 {
            if let Some(folder) = path.parent() {
                std::fs::create_dir_all(folder).ok();
            }
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(path.clone()));
        }
        // Saving happens in the background; give it a moment before leaving.
        if *frame == SETTLE_FRAMES + 30 && !options.bench {
            println!("SANDBOX saved {}", path.display());
            exit.write(AppExit::Success);
        }
    }
    if options.bench {
        frame_ms.push(time.delta_secs() * 1000.0);
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
        assert_eq!(options.presses, [(KeyCode::KeyF, 60)]);
        assert!(options.scripted());
        assert!(!parse(&[]).unwrap().scripted());
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
