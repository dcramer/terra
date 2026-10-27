//! Render failures, loud (D555): a pipeline whose shader does not compile, or an error wgpu
//! reports, draws nothing, and the only sign was a line in the log while scripted runs and
//! films went on and passed. Every run now says `✗ render: …` once for each distinct failure,
//! in its log, and an unattended run (`--shot`, `--shots`, `--bench`, `--sheet`, a script's
//! `--background` or `--remote` run) that had one ends in failure, with a summary line; the
//! remote's `status` and `shot` name them.
//!
//! Two places tell of a failure, as Bevy 0.20 has them:
//! - the pipeline cache: a pipeline whose shader could not be processed or made into a module
//!   is left in `CachedPipelineState::Err` for good (a shader not yet loaded is retried, so it
//!   is no failure); it is read as the render world extracts, while pipelines are being made;
//! - wgpu's uncaptured errors and a lost device, which Bevy hands to the `RenderErrorHandler`
//!   (by default it quits the app). Ours records the error, then lets an unattended run go on
//!   past a validation error, so every failure of the run is listed and its pictures are made;
//!   a lost device or memory ends the run at once, and a person's play quits as Bevy's does.
//!
//! The exit is changed in `Last`, after whatever ended the run asked to: a successful exit of
//! an unattended run with failures becomes an error one.

use crate::shell::Options;
use bevy::{
    app::AppExit,
    ecs::message::{MessageCursor, Messages},
    prelude::*,
    render::{
        ExtractSchedule, MainWorld, RenderApp,
        error_handler::{ErrorType, RenderError, RenderErrorHandler, RenderErrorPolicy},
        render_resource::{CachedPipelineState, PipelineCache, PipelineDescriptor},
    },
    shader::{Shader, ShaderCacheError},
};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};

/// Whether any render failure was recorded in this process, for an exit made outside the
/// app's schedule (the scripted runs' closing timer, `shell::finish`).
static FAILED: AtomicBool = AtomicBool::new(false);

/// Whether this run has had a render failure.
pub fn failed() -> bool {
    FAILED.load(Ordering::Relaxed)
}

/// The most of a failure's text a line keeps; the whole is in the log line before it, which
/// Bevy writes.
const WIDTH: usize = 400;

/// The run's render failures, each line once, in the order first seen, with how many times
/// (pipelines, or reports from wgpu) it came.
#[derive(Resource, Default, Debug)]
pub struct RenderFailures {
    pub failures: Vec<(String, usize)>,
}

impl RenderFailures {
    /// Records a failure; true if its line is new, and so was said.
    pub fn record(&mut self, line: String) -> bool {
        FAILED.store(true, Ordering::Relaxed);
        if let Some((_, count)) = self.failures.iter_mut().find(|(seen, _)| *seen == line) {
            *count += 1;
            return false;
        }
        error!("{line}");
        self.failures.push((line, 1));
        true
    }

    /// Each failure's line, for the remote.
    pub fn lines(world: &World) -> Vec<String> {
        world
            .get_resource::<Self>()
            .map(|failures| {
                failures
                    .failures
                    .iter()
                    .map(|(line, _)| line.clone())
                    .collect()
            })
            .unwrap_or_default()
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<RenderFailures>()
        .insert_resource(RenderErrorHandler(handle))
        .add_systems(Last, fail_the_exit);
    if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
        render_app.add_systems(ExtractSchedule, scan_pipelines);
    }
}

/// A text of several lines on one, cut to [`WIDTH`]: its lines trimmed and joined, without
/// wgpu's headings and the excerpt's gutters, markers, notes and location (in a composed
/// module, whose line numbers are not the file's), the source line it points at kept as
/// `at …`. WESL's excerpts draw their gutter with `|`, naga's with `│`.
fn one_line(text: &str) -> String {
    let mut kept: Vec<String> = Vec::new();
    for line in text.lines().map(str::trim) {
        let gutter = line.find(['|', '│']);
        let numbered = gutter.is_some_and(|at| {
            let number = line[..at].trim();
            !number.is_empty() && number.chars().all(|c| c.is_ascii_digit())
        });
        if numbered {
            let code = line[gutter.unwrap_or(0)..]
                .trim_start_matches(['|', '│'])
                .trim();
            kept.push(format!("at `{code}`"));
        } else if !(line.is_empty()
            || line == "Caused by:"
            || line == "Validation Error"
            || line.starts_with(['|', '│', '=', '┌']))
        {
            kept.push(line.to_string());
        }
    }
    let mut line = kept.join(" · ");
    if line.chars().count() > WIDTH {
        line = line.chars().take(WIDTH).collect::<String>() + "…";
    }
    line
}

/// The files a pipeline's stages are, by their asset paths, each once.
fn shaders(descriptor: &PipelineDescriptor) -> String {
    let handles: Vec<&Handle<Shader>> = match descriptor {
        PipelineDescriptor::RenderPipelineDescriptor(d) => std::iter::once(&d.vertex.shader)
            .chain(d.fragment.as_ref().map(|f| &f.shader))
            .collect(),
        PipelineDescriptor::ComputePipelineDescriptor(d) => vec![&d.shader],
        PipelineDescriptor::MeshPipelineDescriptor(d) => d
            .task
            .as_ref()
            .map(|t| &t.shader)
            .into_iter()
            .chain(std::iter::once(&d.mesh.shader))
            .chain(d.fragment.as_ref().map(|f| &f.shader))
            .collect(),
    };
    let mut names: Vec<String> = Vec::new();
    for handle in handles {
        let name = handle.path().map_or_else(
            || "a shader without a path".to_string(),
            ToString::to_string,
        );
        if !names.contains(&name) {
            names.push(name);
        }
    }
    names.join(", ")
}

fn label(descriptor: &PipelineDescriptor) -> &str {
    match descriptor {
        PipelineDescriptor::RenderPipelineDescriptor(d) => d.label.as_deref(),
        PipelineDescriptor::ComputePipelineDescriptor(d) => d.label.as_deref(),
        PipelineDescriptor::MeshPipelineDescriptor(d) => d.label.as_deref(),
    }
    .unwrap_or("unlabelled")
}

/// The line for a pipeline that failed for good, or none for one that has not (or not yet).
pub fn pipeline_failure(
    descriptor: &PipelineDescriptor,
    state: &CachedPipelineState,
) -> Option<String> {
    let CachedPipelineState::Err(error) = state else {
        return None;
    };
    let (what, text) = match error {
        ShaderCacheError::ProcessShaderError(text) => ("its shader did not compile", text),
        ShaderCacheError::CreateShaderModule(text) => ("wgpu refused its shader", text),
        ShaderCacheError::ShaderNotLoaded(_) | ShaderCacheError::ShaderImportNotYetAvailable => {
            return None;
        }
    };
    Some(format!(
        "✗ render: pipeline {} ({}): {what}: {}",
        label(descriptor),
        shaders(descriptor),
        one_line(text)
    ))
}

/// The line for an error wgpu reported.
pub fn wgpu_failure(error: &RenderError) -> String {
    let description = one_line(&error.description);
    let description = if description.is_empty() {
        "(no description)".to_string()
    } else {
        description
    };
    format!("✗ render: wgpu {:?} error: {description}", error.ty)
}

/// Reads the pipelines that failed for good since last asked, as the render world extracts.
/// A pipeline stays where it is in the cache and an errored one is not made again, so each is
/// read once; the cache is read only while pipelines are being made.
fn scan_pipelines(
    mut main_world: ResMut<MainWorld>,
    cache: Res<PipelineCache>,
    mut seen: Local<HashSet<usize>>,
    mut last: Local<(usize, usize)>,
) {
    let now = (cache.pipelines().count(), cache.waiting_pipelines().count());
    if now == *last && now.1 == 0 {
        return;
    }
    *last = now;
    let found: Vec<String> = cache
        .pipelines()
        .enumerate()
        .filter_map(|(id, pipeline)| {
            if seen.contains(&id) {
                return None;
            }
            let line = pipeline_failure(&pipeline.descriptor, &pipeline.state)?;
            seen.insert(id);
            Some(line)
        })
        .collect();
    if found.is_empty() {
        return;
    }
    let mut failures = main_world.get_resource_or_init::<RenderFailures>();
    for line in found {
        failures.record(line);
    }
}

/// Whether this run is a script's rather than a person's play.
fn unattended(world: &World) -> bool {
    world
        .get_resource::<Options>()
        .is_none_or(|options| options.unattended() || options.remote)
}

/// Ours in place of Bevy's `RenderErrorHandler`, which quits on any error.
fn handle(error: &RenderError, main_world: &mut World, _: &mut World) -> RenderErrorPolicy {
    main_world
        .get_resource_or_init::<RenderFailures>()
        .record(wgpu_failure(error));
    match error.ty {
        ErrorType::Validation | ErrorType::Internal if unattended(main_world) => {
            RenderErrorPolicy::Ignore
        }
        _ => {
            main_world.write_message(AppExit::error());
            RenderErrorPolicy::StopRendering
        }
    }
}

/// When the run is ending: the summary of its render failures, and an unattended run that had
/// one fails, however it was asked to end.
fn fail_the_exit(
    mut exits: ResMut<Messages<AppExit>>,
    mut read: Local<MessageCursor<AppExit>>,
    failures: Res<RenderFailures>,
    options: Option<Res<Options>>,
    mut said: Local<bool>,
) {
    let ending: Vec<bool> = read.read(&exits).map(AppExit::is_success).collect();
    if *said || failures.failures.is_empty() || ending.is_empty() {
        return;
    }
    *said = true;
    let unattended = options.is_none_or(|options| options.unattended() || options.remote);
    let fails = if unattended && ending.iter().all(|success| *success) {
        exits.write(AppExit::error());
        ", so the run fails"
    } else {
        ""
    };
    println!(
        "SANDBOX ✗ {} render failure{}{fails}:",
        failures.failures.len(),
        if failures.failures.len() == 1 {
            ""
        } else {
            "s"
        }
    );
    for (line, count) in &failures.failures {
        let times = if *count > 1 {
            format!(" (×{count})")
        } else {
            String::new()
        };
        println!("  {line}{times}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::render::render_resource::{FragmentState, RenderPipelineDescriptor, VertexState};
    use bevy::render::renderer::RenderDevice;

    /// A pipeline whose shader has a type error is made on wgpu's stub device, as the game's
    /// pipeline cache makes it, and is read as a failure naming the pipeline and the error;
    /// one that compiles is not.
    #[test]
    fn a_pipeline_whose_shader_does_not_compile_is_a_failure() {
        let (device, _queue) = wgpu::Device::noop(&wgpu::DeviceDescriptor::default());
        let mut cache = PipelineCache::new(RenderDevice::from(device), true);
        let good: Handle<Shader> =
            bevy::asset::uuid_handle!("5a4b9d1e-0c1f-4a6e-9a51-6f0d2e3c7b10");
        let bad: Handle<Shader> = bevy::asset::uuid_handle!("5a4b9d1e-0c1f-4a6e-9a51-6f0d2e3c7b11");
        let source = |body: &str| {
            format!(
                "@vertex fn vertex() -> @builtin(position) vec4<f32> {{ {body} }}\n\
                 @fragment fn fragment() -> @location(0) vec4<f32> {{ return vec4<f32>(1.0); }}"
            )
        };
        cache.set_shader(
            good.id(),
            Shader::from_wgsl(source("return vec4<f32>(0.0);"), "good.wgsl"),
        );
        cache.set_shader(
            bad.id(),
            Shader::from_wgsl(source("return 1u;"), "bad.wgsl"),
        );
        let pipeline = |label: &'static str, shader: &Handle<Shader>| RenderPipelineDescriptor {
            label: Some(label.into()),
            vertex: VertexState {
                shader: shader.clone(),
                ..default()
            },
            fragment: Some(FragmentState {
                shader: shader.clone(),
                targets: vec![Some(wgpu::TextureFormat::Rgba8Unorm.into())],
                ..default()
            }),
            ..default()
        };
        let good_id = cache.queue_render_pipeline(pipeline("drawn", &good));
        let bad_id = cache.queue_render_pipeline(pipeline("broken", &bad));
        for _ in 0..4 {
            cache.process_queue();
        }
        let lines: Vec<String> = cache
            .pipelines()
            .filter_map(|p| pipeline_failure(&p.descriptor, &p.state))
            .collect();
        assert!(
            matches!(
                cache.get_render_pipeline_state(good_id),
                CachedPipelineState::Ok(_)
            ),
            "the good pipeline was not made: {:?}",
            cache.get_render_pipeline_state(good_id)
        );
        assert!(
            matches!(
                cache.get_render_pipeline_state(bad_id),
                CachedPipelineState::Err(_)
            ),
            "the broken pipeline did not fail: {:?}",
            cache.get_render_pipeline_state(bad_id)
        );
        assert_eq!(lines.len(), 1, "{lines:#?}");
        assert!(
            lines[0].starts_with("✗ render: pipeline broken ("),
            "{}",
            lines[0]
        );
        assert!(!lines[0].contains('\n'), "{}", lines[0]);
    }

    /// A failure seen again is counted, not said again.
    #[test]
    fn a_failure_is_said_once_and_counted() {
        let mut failures = RenderFailures::default();
        assert!(failures.record("✗ render: one".into()));
        assert!(!failures.record("✗ render: one".into()));
        assert!(failures.record("✗ render: two".into()));
        assert_eq!(
            failures.failures,
            vec![
                ("✗ render: one".to_string(), 2),
                ("✗ render: two".to_string(), 1)
            ]
        );
        assert!(failed());
    }

    /// wgpu's errors of several lines read on one, without its headings.
    #[test]
    fn a_wgpu_error_reads_on_one_line() {
        let error = RenderError {
            ty: ErrorType::Validation,
            description:
                "Validation Error\n\nCaused by:\n  In Device::create_buffer, label = 'x'\n    \
                          Usage flags BufferUsages(MAP_READ | VERTEX) are not allowed\n"
                    .into(),
            source: None,
        };
        assert_eq!(
            wgpu_failure(&error),
            "✗ render: wgpu Validation error: In Device::create_buffer, label = 'x' · \
             Usage flags BufferUsages(MAP_READ | VERTEX) are not allowed"
        );
    }

    /// A validation error is recorded and an unattended run goes on past it; a lost device
    /// ends the run.
    #[test]
    fn a_wgpu_error_is_recorded_and_a_lost_device_ends_the_run() {
        let mut main_world = World::new();
        main_world.init_resource::<Messages<AppExit>>();
        let mut render_world = World::new();
        let error = |ty| RenderError {
            ty,
            description: format!("{ty:?} happened"),
            source: None,
        };
        let policy = handle(
            &error(ErrorType::Validation),
            &mut main_world,
            &mut render_world,
        );
        assert!(matches!(policy, RenderErrorPolicy::Ignore));
        assert_eq!(main_world.resource::<Messages<AppExit>>().len(), 0);
        let policy = handle(
            &error(ErrorType::DeviceLost),
            &mut main_world,
            &mut render_world,
        );
        assert!(matches!(policy, RenderErrorPolicy::StopRendering));
        assert_eq!(main_world.resource::<Messages<AppExit>>().len(), 1);
        assert_eq!(
            RenderFailures::lines(&main_world),
            vec![
                "✗ render: wgpu Validation error: Validation happened",
                "✗ render: wgpu DeviceLost error: DeviceLost happened"
            ]
        );
    }

    /// A shader error, as WESL and naga print them (copied from a run with a shader broken on
    /// purpose), reads as its message and the line of code it points at.
    #[test]
    fn a_shader_error_reads_as_its_message_and_line() {
        let wesl = "error: cannot find declaration of `missing`\n    |\n\
                    148 |     let x = missing(1.0);\n    |             ^^^^^^^ cannot find it\n    |\n\
                    \x20   = note: in declaration of `fragment` in unknown module\n";
        assert_eq!(
            one_line(wesl),
            "error: cannot find declaration of `missing` · at `let x = missing(1.0);`"
        );
        let naga = "Validation Error\n\nCaused by:\n  In Device::create_shader_module\n    \n\
                    Shader '' parsing error: the type of `x` is expected to be `f32`\n    \
                    ┌─ wgsl:774:9\n    │\n774 │     let x: f32 = vec3<f32>(1.0);\n    │         ^ here\n\n";
        assert_eq!(
            one_line(naga),
            "In Device::create_shader_module · Shader '' parsing error: the type of `x` is \
             expected to be `f32` · at `let x: f32 = vec3<f32>(1.0);`"
        );
    }

    /// An unattended run that had a failure fails, however it was asked to end; one without
    /// ends as it was asked.
    #[test]
    fn an_unattended_run_with_a_failure_fails() {
        let run = |failure: bool| {
            let mut app = App::new();
            app.add_plugins(MinimalPlugins).add_plugins(plugin);
            if failure {
                app.world_mut()
                    .resource_mut::<RenderFailures>()
                    .record("✗ render: one".into());
            }
            app.world_mut().write_message(AppExit::Success);
            app.update();
            app.should_exit()
        };
        assert_eq!(run(false), Some(AppExit::Success));
        assert!(run(true).is_some_and(|exit| exit.is_error()));
    }
}
