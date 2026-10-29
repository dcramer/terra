//! The loading screen (D135): what covers the window while the game gets ready, so the
//! first thing seen is the finished scene, not bare ground with figures and colours
//! arriving one by one.
//!
//! Every build starts on [`Screen::Loading`]. The screen stays up while anything is still
//! coming, then waits a few quiet frames more, since what has just loaded often puts the
//! next thing in the world, and the game goes to [`Screen::Playing`] while the screen fades.
//! The game clock is held while loading, so nothing happens in a world nobody can see yet.
//!
//! What is coming is gathered each frame into [`Progress`] by systems in the [`Track`] set.
//! This module tracks what any game has, without knowing it:
//!
//! - files a system asked to be waited for ([`Loading::wait_for`]), such as a model it
//!   reads before putting anything in the world, or its sounds;
//! - models put in the world that have not loaded or been placed yet;
//! - pictures on screen ([`ImageNode`]) still loading;
//! - anything in view the renderer cannot draw yet, whatever its material, because its
//!   mesh, its material or a texture it uses has not been prepared;
//! - shaders still compiling.
//!
//! Work only a feature knows about, such as ground still being meshed, it reports itself,
//! with a system in [`Track`] that calls [`Progress::report`].
//!
//! Another screen, such as a title screen, enters `Screen::Loading` to get the same screen
//! for whatever it starts.

use crate::shell::Options;
use bevy::{
    asset::{LoadState, UntypedAssetId},
    pbr::{PreparedMaterial, RenderMaterialInstances, RenderMeshInstances},
    prelude::*,
    render::{
        ExtractSchedule, MainWorld, RenderApp, erased_render_asset::ErasedRenderAssets,
        mesh::RenderMesh, render_asset::RenderAssets, render_resource::PipelineCache,
    },
    text::LetterSpacing,
    world_serialization::{WorldAssetRoot, WorldInstance, WorldInstanceSpawner},
};
use std::time::Duration;

pub mod hold;
pub use hold::Hold;

/// Frames in a row with nothing coming before the screen is taken away.
const QUIET_FRAMES: u32 = 10;
/// How long the screen takes to fade once the game starts.
const FADE_S: f32 = 0.4;
/// Something that never loads should not keep the player out for good.
const GIVE_UP: Duration = Duration::from_secs(60);
/// A scripted run's pictures and films must not start with anything missing, as pines
/// drawn as crowns over no trunks on a busy machine, so it waits far longer, saying what it
/// is waiting for as it goes (D352).
const GIVE_UP_SCRIPTED: Duration = Duration::from_secs(600);
/// How often a run still loading past the player's cap says what it waits for.
const SAY_EVERY: Duration = Duration::from_secs(15);

/// How long a run waits for everything before it starts anyway.
fn give_up(options: &Options) -> Duration {
    if options.scripted() {
        GIVE_UP_SCRIPTED
    } else {
        GIVE_UP
    }
}

const BACKDROP: Color = Color::srgb(0.035, 0.04, 0.05);
const TITLE: Color = Color::srgb(0.88, 0.86, 0.80);
const QUIET: Color = Color::srgba(0.88, 0.86, 0.80, 0.55);
const TRACK: Color = Color::srgba(1.0, 1.0, 1.0, 0.08);
const FILL: Color = Color::srgb(0.85, 0.62, 0.32);

/// Which screen the game is on.
#[derive(States, Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum Screen {
    #[default]
    Loading,
    /// The game's title screen (D579), over the world it loaded, the clock held.
    Title,
    Playing,
}

/// The screen the game goes to once loaded: playing, unless a title screen asked for itself.
#[derive(Resource, Clone, Copy, Debug)]
pub struct Then(pub Screen);

impl Default for Then {
    fn default() -> Self {
        Self(Screen::Playing)
    }
}

/// What the loading screen is waiting for, beyond what it finds in the world for itself.
#[derive(Resource, Default)]
pub struct Loading {
    /// Files asked to be waited for, not yet loaded.
    files: Vec<UntypedAssetId>,
    /// How many files have been asked for since the screen went up, for the bar.
    asked: usize,
    /// Frames in a row with nothing coming.
    quiet: u32,
    /// How far the bar has come, which never goes back.
    shown: f32,
    /// How long the screen has been up, in real time.
    up_for: Duration,
}

impl Loading {
    /// Keeps the loading screen up until a file has loaded, with everything it needs.
    /// For files a system loads before putting anything in the world, such as a model whose
    /// animations it reads first; a model put in the world is waited for without asking.
    pub fn wait_for(&mut self, file: impl Into<UntypedAssetId>) {
        self.files.push(file.into());
        self.asked += 1;
    }
}

/// How many shaders are compiling, as the render world last said. Anything that must not
/// be seen half drawn, such as an icon being baked, waits for none.
#[derive(Resource, Default)]
pub struct Shaders {
    pub compiling: usize,
    /// How many have been made, for the bar.
    pub made: usize,
}

impl Shaders {
    pub fn idle(&self) -> bool {
        self.compiling == 0
    }
}

/// How many things in view the render world could not yet draw, and could, as it last said
/// while loading.
#[derive(Resource, Default)]
pub(crate) struct Drawables {
    waiting: usize,
    ready: usize,
    /// Some of those waiting, with what they wait for, to say which.
    missing: Vec<(Entity, &'static str)>,
}

/// How many of those waiting the render world names.
const NAMED: usize = 64;

/// The systems that report to [`Progress`]. They run only while loading, before the screen
/// looks at what they said.
#[derive(SystemSet, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Track;

/// What is still coming and what has come, gathered afresh each frame while loading.
#[derive(Resource, Default, Debug)]
pub struct Progress(Vec<Part>);

#[derive(Debug, PartialEq)]
struct Part {
    what: &'static str,
    coming: usize,
    done: usize,
}

impl Progress {
    /// Says that so many of something are still coming and so many have come. `what` is
    /// shown on the screen while it holds the most back, such as "Shaping the ground".
    /// Reports of the same thing add up.
    pub fn report(&mut self, what: &'static str, coming: usize, done: usize) {
        match self.0.iter_mut().find(|part| part.what == what) {
            Some(part) => {
                part.coming += coming;
                part.done += done;
            }
            None => self.0.push(Part { what, coming, done }),
        }
    }

    fn coming(&self) -> usize {
        self.0.iter().map(|part| part.coming).sum()
    }

    /// How far along, from 0 to 1, counting everything alike.
    fn fraction(&self) -> f32 {
        let coming = self.coming();
        let all = coming + self.0.iter().map(|part| part.done).sum::<usize>();
        if all == 0 {
            1.0
        } else {
            (all - coming) as f32 / all as f32
        }
    }

    /// What came and what is still coming, for the log: `Loading 40 of 40, ...`.
    fn summary(&self) -> String {
        self.0
            .iter()
            .map(|part| {
                let all = part.coming + part.done;
                format!("{} {} of {all}", part.what, part.done)
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// What holds the most back.
    fn status(&self) -> &'static str {
        self.0
            .iter()
            .filter(|part| part.coming > 0)
            .max_by_key(|part| part.coming)
            .map_or("Ready", |part| part.what)
    }
}

/// The loading screen's root.
#[derive(Component)]
struct Cover;

/// A part of the screen that fades, with its colour when not faded.
#[derive(Component)]
struct Fades(Color);

#[derive(Component)]
struct Bar;

#[derive(Component)]
struct Status;

/// The screen fading away, with how far it has gone, from 0 to 1.
#[derive(Component)]
struct Fading(f32);

pub fn plugin(app: &mut App) {
    app.init_state::<Screen>()
        .init_resource::<Loading>()
        .init_resource::<Shaders>()
        .init_resource::<Drawables>()
        .init_resource::<Progress>()
        .init_resource::<Hold>()
        .configure_sets(
            Update,
            Track.run_if(in_state(Screen::Loading)).before(watch),
        )
        .add_systems(OnEnter(Screen::Loading), (cover, hold_clock))
        .add_systems(OnExit(Screen::Loading), (uncover, release_clock))
        .add_systems(
            Update,
            (
                (files, models, pictures, drawables, shaders).in_set(Track),
                watch.run_if(in_state(Screen::Loading)),
                fade.run_if(not(in_state(Screen::Loading))),
            ),
        )
        .add_systems(Last, hold::cut_history);
    // Only the render world knows whether shaders are compiling and what it can draw; it
    // says so as it extracts.
    app.sub_app_mut(RenderApp)
        .add_systems(ExtractSchedule, (count_shaders, count_drawables));
}

/// Whether the render world says what it cannot draw yet and which shaders compile: while
/// loading, and all through a scripted run, whose hold waits on them as on the files and
/// models it put in the world ([`hold::watch`]). Not a bench's, which is never held.
fn counted(main_world: &World) -> bool {
    main_world
        .get_resource::<State<Screen>>()
        .is_some_and(|screen| *screen.get() == Screen::Loading)
        || main_world
            .get_resource::<Options>()
            .is_some_and(|options| options.scripted() && !options.bench)
}

/// Counted only while it is read ([`counted`]).
fn count_shaders(mut main_world: ResMut<MainWorld>, pipelines: Res<PipelineCache>) {
    if !counted(&main_world) {
        return;
    }
    if let Some(mut shaders) = main_world.get_resource_mut::<Shaders>() {
        shaders.compiling = pipelines.waiting_pipelines().count();
        shaders.made = pipelines.pipelines().count() - shaders.compiling;
    }
}

/// Counts what is in view and whether it can be drawn: its mesh and its material, with the
/// textures the material uses, prepared on the GPU. A material whose texture is still
/// loading is not drawn at all, so a figure in a skin still loading is missing, not plain.
/// Counted only while it is read ([`counted`]), since it looks at everything in view.
fn count_drawables(
    mut main_world: ResMut<MainWorld>,
    instances: Option<Res<RenderMaterialInstances>>,
    materials: Option<Res<ErasedRenderAssets<PreparedMaterial>>>,
    placed: Option<Res<RenderMeshInstances>>,
    meshes: Option<Res<RenderAssets<RenderMesh>>>,
) {
    let (true, Some(instances), Some(materials), Some(placed), Some(meshes)) =
        (counted(&main_world), instances, materials, placed, meshes)
    else {
        return;
    };
    let mut missing = Vec::new();
    let waiting = instances
        .instances
        .iter()
        .filter_map(|(entity, instance)| {
            let why = if materials.get(instance.asset_id).is_none() {
                "its material"
            } else if placed
                .mesh_asset_id(*entity)
                .is_some_and(|mesh| meshes.get(mesh).is_none())
            {
                "its mesh"
            } else {
                return None;
            };
            if missing.len() < NAMED {
                missing.push((entity.id(), why));
            }
            Some(())
        })
        .count();
    if let Some(mut drawables) = main_world.get_resource_mut::<Drawables>() {
        drawables.waiting = waiting;
        drawables.ready = instances.instances.len() - waiting;
        drawables.missing = missing;
    }
}

/// Files asked to be waited for. A file that failed is not coming; whatever asked for it
/// says so.
fn files(mut loading: ResMut<Loading>, assets: Res<AssetServer>, mut progress: ResMut<Progress>) {
    let coming = asked_coming(&mut loading, &assets);
    progress.report("Loading", coming, loading.asked - coming);
}

/// How many files asked to be waited for are still coming, those that came let go.
fn asked_coming(loading: &mut Loading, assets: &AssetServer) -> usize {
    loading.files.retain(|&id| {
        !assets.is_loaded_with_dependencies(id)
            && !matches!(assets.load_state(id), LoadState::Failed(_))
    });
    loading.files.len()
}

/// Models put in the world, until they have loaded and been placed.
fn models(
    assets: Res<AssetServer>,
    spawner: Res<WorldInstanceSpawner>,
    models: Query<(&WorldAssetRoot, Option<&WorldInstance>)>,
    mut progress: ResMut<Progress>,
) {
    let placing = placing(&assets, &spawner, &models);
    progress.report("Loading", placing, models.iter().len() - placing);
}

/// How many models put in the world have not loaded or been placed yet.
fn placing(
    assets: &AssetServer,
    spawner: &WorldInstanceSpawner,
    models: &Query<(&WorldAssetRoot, Option<&WorldInstance>)>,
) -> usize {
    models
        .iter()
        .filter(|(model, instance)| {
            !matches!(assets.load_state(&model.0), LoadState::Failed(_))
                && !instance.is_some_and(|instance| spawner.instance_is_ready(**instance))
        })
        .count()
}

/// Pictures on screen, such as the icons in the bag, until they have loaded. One made by
/// the game rather than loaded is there at once.
fn pictures(assets: Res<AssetServer>, pictures: Query<&ImageNode>, mut progress: ResMut<Progress>) {
    let coming = pictures
        .iter()
        .filter(|picture| matches!(assets.load_state(&picture.image), LoadState::Loading))
        .count();
    progress.report("Loading", coming, pictures.iter().len() - coming);
}

fn drawables(drawables: Res<Drawables>, mut progress: ResMut<Progress>) {
    progress.report("Preparing the scene", drawables.waiting, drawables.ready);
}

fn shaders(shaders: Res<Shaders>, mut progress: ResMut<Progress>) {
    progress.report("Preparing shaders", shaders.compiling, shaders.made);
}

fn hold_clock(mut time: ResMut<Time<Virtual>>) {
    time.pause();
}

fn release_clock(mut time: ResMut<Time<Virtual>>) {
    time.unpause();
}

fn cover(
    mut commands: Commands,
    mut loading: ResMut<Loading>,
    mut drawables: ResMut<Drawables>,
    old: Query<Entity, With<Cover>>,
) {
    // Loading again while the last screen is still fading.
    for entity in &old {
        commands.entity(entity).despawn();
    }
    loading.asked = loading.files.len();
    loading.quiet = 0;
    loading.shown = 0.0;
    loading.up_for = Duration::ZERO;
    // What the render world said when last loading is no longer so.
    *drawables = Drawables::default();
    commands
        .spawn((
            Name::new("Loading screen"),
            Cover,
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: px(22),
                ..default()
            },
            BackgroundColor(BACKDROP),
            Fades(BACKDROP),
            // Over everything in the game, the options menu included, but under the
            // frame-time readout.
            GlobalZIndex(i32::MAX - 2),
        ))
        .with_children(|screen| {
            screen.spawn((
                Text::new("TERRA"),
                TextFont::from_font_size(48.0),
                LetterSpacing::Px(16.0),
                // The spacing follows the last letter too; this keeps the word centred.
                Node {
                    margin: UiRect::left(px(16)),
                    ..default()
                },
                TextColor(TITLE),
                Fades(TITLE),
            ));
            screen
                .spawn((
                    Node {
                        width: px(320),
                        height: px(3),
                        border_radius: BorderRadius::all(px(2)),
                        ..default()
                    },
                    BackgroundColor(TRACK),
                    Fades(TRACK),
                ))
                .with_children(|track| {
                    track.spawn((
                        Bar,
                        Node {
                            width: percent(0),
                            height: percent(100),
                            border_radius: BorderRadius::all(px(2)),
                            ..default()
                        },
                        BackgroundColor(FILL),
                        Fades(FILL),
                    ));
                });
            screen.spawn((
                Status,
                Text::new("Loading"),
                TextFont::from_font_size(14.0),
                TextColor(QUIET),
                Fades(QUIET),
            ));
        });
}

/// Watches what is still coming, moves the bar, and starts the game once nothing has come
/// for a few frames.
fn watch(
    mut loading: ResMut<Loading>,
    mut progress: ResMut<Progress>,
    real: Res<Time<Real>>,
    options: Res<Options>,
    mut bar: Single<&mut Node, With<Bar>>,
    mut status: Single<&mut Text, With<Status>>,
    (mut next, then): (ResMut<NextState<Screen>>, Option<Res<Then>>),
    (drawables, names): (Res<Drawables>, Query<(Option<&Name>, Option<&ChildOf>)>),
    recorder: Option<Res<crate::recorder::Recorder>>,
) {
    let then = then.map_or(Screen::Playing, |then| then.0);
    // What was said this frame; next frame is said afresh.
    let progress = std::mem::take(&mut *progress);
    loading.up_for += real.delta();
    loading.quiet = if progress.coming() > 0 {
        0
    } else {
        loading.quiet + 1
    };
    loading.shown = loading.shown.max(progress.fraction());
    bar.width = percent(100.0 * loading.shown);
    if status.0 != progress.status() {
        status.0 = progress.status().into();
    }
    // A screen held up to be looked at stays (--loading).
    if options.hold_loading {
        return;
    }
    if loading.quiet >= QUIET_FRAMES {
        // By the wall clock from the app's start, as a player waits: the frames' deltas
        // summed (`up_for`) left out the start-up schedules and a first frame of 282 s.
        // Its longest systems, summed over its frames and threads, say why it took that
        // long without a trace (the frame recorder, D576).
        let seconds = real.startup().elapsed().as_secs_f32();
        if let Some(recorder) = &recorder {
            recorder.loaded(seconds);
        }
        let longest = recorder
            .map(|recorder| recorder.longest_so_far())
            .filter(|longest| !longest.is_empty())
            .map_or(String::new(), |longest| format!(" Longest: {longest}."));
        info!(
            "Loaded in {seconds:.1} s ({:.1} s of it on the loading screen's frames): {}.{longest}",
            loading.up_for.as_secs_f32(),
            progress.summary()
        );
        next.set(then);
        return;
    }
    let cap = give_up(&options);
    let waited = loading.up_for;
    if waited > cap {
        warn!(
            "Still loading after {} s, starting anyway: {}; waiting for {}.",
            cap.as_secs(),
            progress.summary(),
            waiting_for(&drawables, &names)
        );
        next.set(then);
    } else if waited > GIVE_UP
        && (waited - real.delta()).as_secs() / SAY_EVERY.as_secs()
            != waited.as_secs() / SAY_EVERY.as_secs()
    {
        // A scripted run past the player's cap: what it is still waiting for, loudly.
        warn!(
            "Still loading after {} s, waiting up to {} s: {}; waiting for {}.",
            waited.as_secs(),
            cap.as_secs(),
            progress.summary(),
            waiting_for(&drawables, &names)
        );
    }
}

/// What in view the renderer cannot draw yet, by name, with what each waits for and how
/// many of each: `Pine bark (its material) x12, Chunk (its mesh) x3`.
fn waiting_for(drawables: &Drawables, names: &Query<(Option<&Name>, Option<&ChildOf>)>) -> String {
    if drawables.waiting == 0 {
        return "nothing in view".into();
    }
    // An entity's own name, or the nearest named one it hangs from.
    let name = |mut entity: Entity| {
        let mut part = false;
        for _ in 0..8 {
            match names.get(entity) {
                Ok((Some(name), _)) if part => return format!("part of {}", name.as_str()),
                Ok((Some(name), _)) => return name.as_str().to_string(),
                Ok((None, Some(parent))) => {
                    entity = parent.parent();
                    part = true;
                }
                _ => break,
            }
        }
        format!("unnamed {entity}")
    };
    let mut counts: Vec<(String, usize)> = Vec::new();
    for &(entity, why) in &drawables.missing {
        let key = format!("{} ({why})", name(entity));
        match counts.iter_mut().find(|(k, _)| *k == key) {
            Some((_, n)) => *n += 1,
            None => counts.push((key, 1)),
        }
    }
    counts.sort_by_key(|count| std::cmp::Reverse(count.1));
    let named: Vec<String> = counts.iter().map(|(k, n)| format!("{k} x{n}")).collect();
    let more = drawables.waiting.saturating_sub(drawables.missing.len());
    format!(
        "{} things in view: {}{}",
        drawables.waiting,
        named.join(", "),
        if more > 0 {
            format!(" and {more} more")
        } else {
            String::new()
        }
    )
}

/// Fades the screen away as the game starts; a scripted run takes it away at once, so its
/// pictures never have it in them.
fn uncover(mut commands: Commands, options: Res<Options>, cover: Query<Entity, With<Cover>>) {
    for entity in &cover {
        if options.scripted() {
            commands.entity(entity).despawn();
        } else {
            commands.entity(entity).insert(Fading(0.0));
        }
    }
}

fn fade(
    mut commands: Commands,
    real: Res<Time<Real>>,
    mut cover: Query<(Entity, &mut Fading)>,
    children: Query<&Children>,
    mut parts: Query<(&Fades, Option<&mut BackgroundColor>, Option<&mut TextColor>)>,
) {
    for (root, mut fading) in &mut cover {
        fading.0 += real.delta_secs() / FADE_S;
        if fading.0 >= 1.0 {
            commands.entity(root).despawn();
            continue;
        }
        let left = 1.0 - fading.0;
        for part in std::iter::once(root).chain(children.iter_descendants(root)) {
            let Ok((full, background, text)) = parts.get_mut(part) else {
                continue;
            };
            let colour = full.0.with_alpha(full.0.alpha() * left);
            // Text has a background too, left clear; its colour is the text's.
            match (text, background) {
                (Some(mut text), _) => text.0 = colour,
                (None, Some(mut background)) => background.0 = colour,
                (None, None) => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_everything_alike() {
        let mut progress = Progress::default();
        progress.report("Loading", 1, 1);
        progress.report("Loading", 0, 2);
        progress.report("Preparing shaders", 3, 3);
        assert_eq!(progress.coming(), 4);
        assert_eq!(progress.fraction(), 0.6);
        assert_eq!(progress.status(), "Preparing shaders");
        let mut compiling = Progress::default();
        compiling.report("Loading", 0, 4);
        compiling.report("Preparing shaders", 1, 5);
        assert_eq!(compiling.status(), "Preparing shaders");
        assert_eq!(compiling.fraction(), 0.9);
        let nothing = Progress::default();
        assert_eq!(nothing.coming(), 0);
        assert_eq!(nothing.fraction(), 1.0);
        assert_eq!(nothing.status(), "Ready");
    }

    #[test]
    fn reports_of_one_thing_add_up() {
        let mut progress = Progress::default();
        progress.report("Loading", 2, 1);
        progress.report("Preparing shaders", 4, 0);
        progress.report("Loading", 3, 0);
        // Five loading in all hold back more than four shaders.
        assert_eq!(progress.status(), "Loading");
        assert_eq!(
            progress.summary(),
            "Loading 1 of 6, Preparing shaders 0 of 4"
        );
    }

    /// The loading screen as `cover` puts it up, with no frame yet watched.
    fn screen() -> World {
        screen_for(&[])
    }

    /// The screen for a run given these options.
    fn screen_for(args: &[&str]) -> World {
        let mut world = World::new();
        world.insert_resource(Loading::default());
        world.insert_resource(Progress::default());
        world.insert_resource(Drawables::default());
        let mut real = Time::<Real>::default();
        real.update_with_duration(Duration::ZERO);
        world.insert_resource(real);
        world.insert_resource(Options::from_args(args.iter().map(|a| a.to_string())).unwrap());
        world.insert_resource(NextState::<Screen>::default());
        world.spawn((Bar, Node::default()));
        world.spawn((Status, Text::new("Loading")));
        world
    }

    /// One frame of so many seconds, with so much said to be coming and done. Whether the
    /// game has been told to start, and how far the bar is.
    fn frame(world: &mut World, coming: usize, done: usize, seconds: f32) -> (bool, Val) {
        world
            .resource_mut::<Time<Real>>()
            .update_with_duration(Duration::from_secs_f32(seconds));
        world
            .resource_mut::<Progress>()
            .report("Loading", coming, done);
        world.run_system_cached(watch).unwrap();
        let started = matches!(
            world.resource::<NextState<Screen>>(),
            NextState::Pending(Screen::Playing)
        );
        let bar = world
            .query_filtered::<&Node, With<Bar>>()
            .single(world)
            .unwrap()
            .width;
        (started, bar)
    }

    #[test]
    fn the_game_starts_once_nothing_has_come_for_some_frames_in_a_row() {
        let mut world = screen();
        assert!(!frame(&mut world, 2, 0, 0.1).0);
        for _ in 1..QUIET_FRAMES {
            assert!(!frame(&mut world, 0, 2, 0.1).0);
        }
        // Something more comes just before: the quiet starts again.
        assert!(!frame(&mut world, 1, 2, 0.1).0);
        for _ in 1..QUIET_FRAMES {
            assert!(!frame(&mut world, 0, 3, 0.1).0);
        }
        assert!(frame(&mut world, 0, 3, 0.1).0);
    }

    #[test]
    fn the_bar_never_goes_back() {
        let mut world = screen();
        assert_eq!(frame(&mut world, 1, 3, 0.1).1, percent(75.0));
        // More was found to be coming: the bar waits rather than falling back.
        assert_eq!(frame(&mut world, 3, 1, 0.1).1, percent(75.0));
        assert_eq!(frame(&mut world, 0, 4, 0.1).1, percent(100.0));
    }

    #[test]
    fn the_game_starts_anyway_when_something_never_loads() {
        let mut world = screen();
        let half = GIVE_UP.as_secs_f32() / 2.0;
        assert!(!frame(&mut world, 1, 0, half).0);
        assert!(!frame(&mut world, 1, 0, half).0);
        assert!(frame(&mut world, 1, 0, 0.1).0);
    }

    /// A scripted run's pictures wait for everything far longer than a player would, so a
    /// busy machine does not film pines with no trunks; it still starts in the end.
    #[test]
    fn a_scripted_run_waits_far_longer_for_what_never_loads() {
        let mut world = screen_for(&["--shot", "out.png"]);
        let step = GIVE_UP.as_secs_f32();
        assert!(!frame(&mut world, 1, 0, step).0);
        assert!(
            !frame(&mut world, 1, 0, step).0,
            "gave up at the player's cap"
        );
        let rest = GIVE_UP_SCRIPTED.as_secs_f32() - 2.0 * step;
        assert!(!frame(&mut world, 1, 0, rest - 1.0).0);
        assert!(frame(&mut world, 1, 0, 2.0).0, "never gave up");
    }

    /// What cannot be drawn yet is named, by the nearest name it hangs from, and counted.
    #[test]
    fn what_is_waiting_is_named() {
        let mut world = World::new();
        let tree = world.spawn(Name::new("Pine")).id();
        let bark = world.spawn(ChildOf(tree)).id();
        let bark2 = world.spawn(ChildOf(tree)).id();
        let chunk = world.spawn(Name::new("Chunk")).id();
        let drawables = Drawables {
            waiting: 4,
            ready: 10,
            missing: vec![
                (bark, "its material"),
                (bark2, "its material"),
                (chunk, "its mesh"),
            ],
        };
        let said = world
            .run_system_cached_with(
                |In(d): In<Drawables>, names: Query<(Option<&Name>, Option<&ChildOf>)>| {
                    waiting_for(&d, &names)
                },
                drawables,
            )
            .unwrap();
        assert_eq!(
            said,
            "4 things in view: part of Pine (its material) x2, Chunk (its mesh) x1 and 1 more"
        );
    }
}
