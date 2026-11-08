//! The run's shader gate (D632): a shader that does not compile fails the run, its exit an
//! error naming the shader and what Bevy said of it (the import it could not resolve, the
//! compiler's error). Bevy only warns once and draws nothing of the material: tell.wesl imported
//! `bevy_pbr::forward_io` (0.20's `bevy_pbr::render::forward_io`) and the foes' tells vanished
//! from the route unnoticed for a job (iso-forest-demo j29, j30).
//!
//! Two halves, every shader alike: a log layer (`layer`, LogPlugin's `custom_layer`) keeps what
//! the shader cache and the pipeline cache warn of, and the render world counts each pipeline's
//! frames spent failing (not compiled and not compiling); one failing `STUCK` frames on end fails
//! the run with its shaders' paths and those words. An import still loading is retried by Bevy
//! and compiles within a few frames, so it never reaches `STUCK`.

use bevy::diagnostic::FrameCount;
use bevy::log::{BoxedLayer, tracing, tracing_subscriber};
use bevy::prelude::*;
use bevy::render::render_resource::{CachedPipelineState, PipelineCache, PipelineDescriptor};
use bevy::render::{Render, RenderApp, RenderSystems};
use std::sync::{Arc, Mutex, PoisonError};

/// Frames a pipeline fails on end before the run fails: a shader still loading resolves in a
/// few (a still compiles each pipeline in the frame that asks for it, main.rs).
const STUCK: u32 = 120;
/// What the log keeps at most, so a fault repeated every frame cannot grow it.
const KEPT: usize = 32;

/// What the two halves found, shared by the log layer, the render world and the gate.
#[derive(Resource, Clone, Default)]
struct Faults(Arc<Mutex<Said>>);

#[derive(Default)]
struct Said {
    /// The shader and pipeline caches' warnings and errors, as logged.
    log: Vec<String>,
    /// Each stuck pipeline: its shaders' paths and its last error.
    stuck: Vec<String>,
}

impl Faults {
    fn said(&self) -> std::sync::MutexGuard<'_, Said> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// LogPlugin's `custom_layer`: hears the shader caches' faults for the gate.
pub fn layer(app: &mut App) -> Option<BoxedLayer> {
    let faults = Faults::default();
    app.insert_resource(faults.clone());
    Some(Box::new(Listen(faults)))
}

pub fn plugin(app: &mut App) {
    let faults = app
        .world()
        .get_resource::<Faults>()
        .cloned()
        .unwrap_or_default();
    app.insert_resource(faults.clone()).add_systems(Last, gate);
    if let Some(render) = app.get_sub_app_mut(RenderApp) {
        render
            .insert_resource(faults)
            .add_systems(Render, count.in_set(RenderSystems::Cleanup));
    }
}

/// Whether a log line is the shader cache's or the pipeline cache's fault.
fn heard(target: &str, level: tracing::Level) -> bool {
    level <= tracing::Level::WARN
        && (target.starts_with("bevy_shader")
            || target.starts_with("bevy_render::render_resource::pipeline_cache"))
}

struct Listen(Faults);

impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for Listen {
    fn on_event(&self, event: &tracing::Event<'_>, _: tracing_subscriber::layer::Context<'_, S>) {
        let meta = event.metadata();
        if !heard(meta.target(), *meta.level()) {
            return;
        }
        let mut text = Message::default();
        event.record(&mut text);
        let mut said = self.0.said();
        if said.log.len() < KEPT && !said.log.contains(&text.0) {
            said.log.push(text.0);
        }
    }
}

#[derive(Default)]
struct Message(String);

impl tracing::field::Visit for Message {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.0 = format!("{value:?}");
        }
    }
}

/// A pipeline's state this frame, as the gate counts it.
#[derive(Clone, Copy, PartialEq, Debug)]
enum State {
    Compiled,
    /// Being compiled in the background: never counted, a cold driver cache takes seconds.
    Compiling,
    /// Queued again after an error, or failed.
    Failing,
}

/// Counts each pipeline's failing frames on end (`run`, by index), a compiled one back to 0;
/// the indices that reach `STUCK` this frame.
fn tick(run: &mut Vec<u32>, states: impl Iterator<Item = State>) -> Vec<usize> {
    let mut stuck = Vec::new();
    for (i, state) in states.enumerate() {
        if run.len() <= i {
            run.resize(i + 1, 0);
        }
        match state {
            State::Compiled => run[i] = 0,
            State::Compiling => {}
            State::Failing => {
                run[i] += 1;
                if run[i] == STUCK {
                    stuck.push(i);
                }
            }
        }
    }
    stuck
}

fn state(s: &CachedPipelineState) -> State {
    match s {
        CachedPipelineState::Ok(_) => State::Compiled,
        CachedPipelineState::Creating(_) => State::Compiling,
        CachedPipelineState::Queued | CachedPipelineState::Err(_) => State::Failing,
    }
}

/// A pipeline's shaders, by path (an embedded one `embedded://iso/...`).
fn shaders(d: &PipelineDescriptor) -> Vec<String> {
    let handles = match d {
        PipelineDescriptor::RenderPipelineDescriptor(d) => {
            let mut h = vec![&d.vertex.shader];
            h.extend(d.fragment.as_ref().map(|f| &f.shader));
            h
        }
        PipelineDescriptor::ComputePipelineDescriptor(d) => vec![&d.shader],
        PipelineDescriptor::MeshPipelineDescriptor(d) => {
            let mut h = vec![&d.mesh.shader];
            h.extend(d.task.as_ref().map(|t| &t.shader));
            h.extend(d.fragment.as_ref().map(|f| &f.shader));
            h
        }
    };
    handles
        .into_iter()
        .map(|h| {
            h.path()
                .map_or_else(|| format!("{:?}", h.id()), |p| p.to_string())
        })
        .collect()
}

/// The render world's half: each pipeline's failing frames, a stuck one told to the gate.
fn count(cache: Res<PipelineCache>, faults: Res<Faults>, mut run: Local<Vec<u32>>) {
    let stuck = tick(&mut run, cache.pipelines().map(|p| state(&p.state)));
    if stuck.is_empty() {
        return;
    }
    let pipelines: Vec<_> = cache.pipelines().collect();
    let lines = stuck.into_iter().map(|i| {
        let p = pipelines[i];
        let why = match &p.state {
            CachedPipelineState::Err(e) => e.to_string(),
            _ => "queued again after an error".into(),
        };
        format!("{} ({why})", shaders(&p.descriptor).join(" + "))
    });
    let mut said = faults.said();
    for line in lines {
        // A material's pipelines (one a view, a key) fail alike: one line for them.
        if !said.stuck.contains(&line) {
            said.stuck.push(line);
        }
    }
}

/// The gate's verdict once a pipeline is stuck: its shaders, what Bevy said and the fix.
fn verdict(said: &Said) -> String {
    let log = if said.log.is_empty() {
        "nothing logged".into()
    } else {
        said.log.join(" | ").replace('\n', " ")
    };
    format!(
        "shaders: {} kind(s) of pipeline never compiled in {STUCK} frames, so what they draw is not \
         drawn: {}. Bevy said: {log}. Fix the shader named (an import's path is its \
         `#define_import_path` under ~/.cargo/registry/src/*/bevy_pbr-0.20.0/src/); do not \
         drop the gate (shader_gate.rs, D632)",
        said.stuck.len(),
        said.stuck.join("; ").replace('\n', " ")
    )
}

/// Fails the run at the first stuck pipeline; on a still's or film's last frame, says it passed.
fn gate(
    faults: Res<Faults>,
    args: Res<crate::Args>,
    frame: Res<FrameCount>,
    mut exit: MessageWriter<AppExit>,
    mut failed: Local<bool>,
) {
    if *failed {
        return;
    }
    let said = faults.said();
    if !said.stuck.is_empty() {
        *failed = true;
        error!("✗ {}", verdict(&said));
        exit.write(AppExit::from_code(1));
    } else if args.last_frame().is_some_and(|l| frame.0 + 1 == l) {
        info!("✓ shaders: every pipeline compiled");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing_subscriber::layer::SubscriberExt;

    /// The planted fault (D632): the shader cache's warning for j29's import is heard and
    /// named in the verdict; another crate's warning is not.
    #[test]
    fn a_shader_that_does_not_compile_is_named_with_its_import() {
        let faults = Faults::default();
        let subscriber = tracing_subscriber::registry().with(Listen(faults.clone()));
        tracing::subscriber::with_default(subscriber, || {
            tracing::warn!(
                target: "bevy_shader::shader_cache",
                "Shader `embedded://iso/demo/tell.wesl` has an unresolved import:\nerror: module \
                 not found: `bevy_pbr::forward_io`"
            );
            tracing::warn!(target: "bevy_render::mesh::allocator", "Mesh contains no vertices.");
        });
        let mut said = faults.said();
        assert_eq!(said.log.len(), 1, "{:?}", said.log);
        said.stuck
            .push("embedded://iso/demo/tell.wesl (queued again after an error)".into());
        let v = verdict(&said);
        assert!(v.contains("embedded://iso/demo/tell.wesl"), "{v}");
        assert!(v.contains("bevy_pbr::forward_io"), "{v}");
    }

    /// A pipeline failing frame after frame is stuck once, at `STUCK`; one that compiles in the
    /// end, or compiles slowly in the background, never is.
    #[test]
    fn only_a_pipeline_that_keeps_failing_is_stuck() {
        use State::*;
        let mut run = Vec::new();
        let mut stuck = Vec::new();
        for f in 0..3 * STUCK {
            // 0 cycles as an unresolved import does (compiling, failed, queued); 1 fails for
            // a while, then compiles; 2 compiles for ever.
            let zero = [Compiling, Failing, Failing][f as usize % 3];
            let one = if f < STUCK - 1 { Failing } else { Compiled };
            for i in tick(&mut run, [zero, one, Compiling].into_iter()) {
                stuck.push((f, i));
            }
        }
        assert_eq!(stuck.len(), 1, "{stuck:?}");
        assert_eq!(stuck[0].1, 0);
    }
}
