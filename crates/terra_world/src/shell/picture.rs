//! An offscreen run's picture: what its cameras draw into in place of a window, and each
//! frame waiting on the GPU as a window's would.

use super::Options;
use bevy::{
    camera::{CameraUpdateSystems, RenderTarget},
    prelude::*,
    render::{
        Render, RenderApp, RenderSystems,
        render_resource::{PollType, TextureFormat, TextureUsages},
        renderer::{RenderDevice, RenderQueue},
        view::screenshot::Screenshot,
    },
    ui::IsDefaultUiCamera,
    window::{PrimaryWindow, WindowRef},
};
use std::time::Instant;

/// Draws an offscreen, unattended run into its picture, each frame waiting for the last.
pub(super) fn plugin(app: &mut App) {
    app.add_systems(Startup, make_picture)
        .add_systems(PostUpdate, draw_into_picture.before(CameraUpdateSystems));
    let waits = RenderWaits::default();
    app.insert_resource(waits.clone());
    app.sub_app_mut(RenderApp)
        .init_resource::<InFlight>()
        .insert_resource(waits)
        .insert_resource(RenderBegan(Instant::now()))
        .add_systems(
            Render,
            (
                render_began.in_set(RenderSystems::ExtractCommands),
                wait_for_the_gpu.in_set(RenderSystems::Cleanup),
            ),
        );
}

/// How big an offscreen run's picture is: the resolution performance is judged at (D25).
pub const PICTURE_SIZE: UVec2 = UVec2::new(1920, 1080);

/// What an offscreen run draws into, in place of its window.
#[derive(Resource)]
pub struct Picture(pub Handle<Image>);

fn make_picture(mut commands: Commands, mut images: ResMut<Assets<Image>>, options: Res<Options>) {
    let mut image = Image::new_target_texture(
        options.picture.x,
        options.picture.y,
        TextureFormat::Rgba8Unorm,
        Some(TextureFormat::Rgba8UnormSrgb),
    );
    // So it can be read back for a screenshot.
    image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    // Drawn into, so nothing to upload: as made, its zeros waited their turn under the upload
    // cap (D420) behind the start's textures, so its cameras got no view until the sixth
    // frame, by when Bevy's upscaling blocked 1.1 s on its pipeline behind the load's others.
    image.data = None;
    commands.insert_resource(Picture(images.add(image)));
}

/// Points every camera that would draw into the window at the picture instead, including
/// cameras experiments spawn later.
///
/// Bevy draws the UI with the last camera into the primary window unless one is marked
/// [`IsDefaultUiCamera`]; with none left drawing into it the screen's UI (bars, panels,
/// notices) would be missing from the picture, so the last camera drawing into it is marked.
fn draw_into_picture(
    mut commands: Commands,
    picture: Res<Picture>,
    primary: Query<(), With<PrimaryWindow>>,
    mut cameras: Query<(Entity, &Camera, &mut RenderTarget, Has<IsDefaultUiCamera>)>,
) {
    let mut marked = false;
    let mut last = None;
    for (entity, camera, mut target, default_ui) in &mut cameras {
        marked |= default_ui;
        let into_window = match &*target {
            RenderTarget::Window(WindowRef::Primary) => true,
            RenderTarget::Window(WindowRef::Entity(window)) => primary.contains(*window),
            _ => false,
        };
        if into_window {
            *target = RenderTarget::Image(picture.0.clone().into());
        }
        let drawn = matches!(&*target, RenderTarget::Image(image) if image.handle == picture.0);
        if drawn && last.is_none_or(|(order, at)| (camera.order, entity) > (order, at)) {
            last = Some((camera.order, entity));
        }
    }
    if let (false, Some((_, entity))) = (marked, last) {
        commands.entity(entity).insert(IsDefaultUiCamera);
    }
}

/// Waiting for the frame before this one to be drawn.
#[derive(Resource, Default)]
struct InFlight(Option<Box<dyn FnOnce(&RenderDevice) + Send + Sync>>);

/// For each frame of an offscreen run, how long the render world worked on it before it
/// waited for the GPU, and how long it then waited, in ms: whether frames are held up by the
/// CPU's side of rendering (preparing, queuing and recording what is drawn, which the render
/// passes' own CPU times leave out) or by the GPU. Shared by both worlds.
#[derive(Resource, Clone, Default)]
pub struct RenderWaits(pub std::sync::Arc<std::sync::Mutex<Vec<[f32; 2]>>>);

/// When the render world began its frame.
#[derive(Resource)]
struct RenderBegan(Instant);

fn render_began(mut began: ResMut<RenderBegan>) {
    began.0 = Instant::now();
}

/// Nothing is shown, so nothing holds the CPU back while the GPU catches up, as a window
/// does by letting only a frame or two be in flight; without this a benchmark would time
/// only the CPU. Like a window, each frame waits for the one before it to be drawn, so
/// the CPU works on one frame while the GPU draws the last.
fn wait_for_the_gpu(
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    mut in_flight: ResMut<InFlight>,
    (began, waits): (Res<RenderBegan>, Res<RenderWaits>),
) {
    let busy = began.0.elapsed();
    // Done once everything this frame submitted is.
    let drawn = queue.submit(std::iter::empty());
    let wait = move |device: &RenderDevice| {
        let until = PollType::Wait {
            submission_index: Some(drawn),
            timeout: None,
        };
        device.poll(until).ok();
    };
    let waiting = Instant::now();
    if let Some(before) = in_flight.0.replace(Box::new(wait)) {
        before(&device);
    }
    if let Ok(mut waits) = waits.0.lock() {
        waits.push([
            busy.as_secs_f32() * 1000.0,
            waiting.elapsed().as_secs_f32() * 1000.0,
        ]);
    }
}

/// A screenshot of what a run shows: its window, or its picture when it is offscreen.
pub fn screenshot(picture: Option<&Picture>) -> Screenshot {
    picture.map_or_else(Screenshot::primary_window, |picture| {
        Screenshot::image(picture.0.clone())
    })
}
