//! The loading screen (D135): what covers the window while the game gets ready, so the
//! first thing seen is the finished scene, not bare ground with figures and colours
//! arriving one by one.
//!
//! Every build starts on [`Screen::Loading`]. The screen stays up while anything is still
//! coming: a file a system asked to be waited for ([`Loading::wait_for`]), a model put in
//! the world that has not loaded or been placed yet, or a shader still compiling. Then it
//! waits a few quiet frames more, since what has just loaded often puts the next thing in
//! the world, and the game goes to [`Screen::Playing`] while the screen fades. The game
//! clock is held while loading, so nothing happens in a world nobody can see yet.
//!
//! Nothing here knows the game. Another screen, such as a title screen, enters
//! `Screen::Loading` to get the same screen for whatever it starts.

use crate::shell::Options;
use bevy::{
    asset::{LoadState, UntypedAssetId},
    prelude::*,
    render::{ExtractSchedule, MainWorld, RenderApp, render_resource::PipelineCache},
    text::LetterSpacing,
    world_serialization::{WorldAssetRoot, WorldInstance, WorldInstanceSpawner},
};
use std::time::Duration;

/// Frames in a row with nothing coming before the screen is taken away.
const QUIET_FRAMES: u32 = 10;
/// How long the screen takes to fade once the game starts.
const FADE_S: f32 = 0.4;
/// Something that never loads should not keep the player out for good.
const GIVE_UP: Duration = Duration::from_secs(60);

const BACKDROP: Color = Color::srgb(0.035, 0.04, 0.05);
const TITLE: Color = Color::srgb(0.88, 0.86, 0.80);
const QUIET: Color = Color::srgba(0.88, 0.86, 0.80, 0.55);
const TRACK: Color = Color::srgba(1.0, 1.0, 1.0, 0.08);
const FILL: Color = Color::srgb(0.85, 0.62, 0.32);

/// Which screen the game is on. Only loading and playing so far.
#[derive(States, Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum Screen {
    #[default]
    Loading,
    Playing,
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
        .add_systems(OnEnter(Screen::Loading), (cover, hold_clock))
        .add_systems(OnExit(Screen::Loading), (uncover, release_clock))
        .add_systems(
            Update,
            (
                watch.run_if(in_state(Screen::Loading)),
                fade.run_if(in_state(Screen::Playing)),
            ),
        );
    // Only the render world knows whether shaders are compiling; it says so as it extracts.
    app.sub_app_mut(RenderApp)
        .add_systems(ExtractSchedule, count_shaders);
}

fn count_shaders(mut main_world: ResMut<MainWorld>, pipelines: Res<PipelineCache>) {
    if let Some(mut shaders) = main_world.get_resource_mut::<Shaders>() {
        shaders.compiling = pipelines.waiting_pipelines().count();
        shaders.made = pipelines.pipelines().count() - shaders.compiling;
    }
}

fn hold_clock(mut time: ResMut<Time<Virtual>>) {
    time.pause();
}

fn release_clock(mut time: ResMut<Time<Virtual>>) {
    time.unpause();
}

fn cover(mut commands: Commands, mut loading: ResMut<Loading>, old: Query<Entity, With<Cover>>) {
    // Loading again while the last screen is still fading.
    for entity in &old {
        commands.entity(entity).despawn();
    }
    loading.asked = loading.files.len();
    loading.quiet = 0;
    loading.shown = 0.0;
    loading.up_for = Duration::ZERO;
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

/// What is still coming, and how much has come.
#[derive(Debug, Default, PartialEq)]
struct Count {
    files: (usize, usize),
    models: (usize, usize),
    shaders: (usize, usize),
}

impl Count {
    fn coming(&self) -> bool {
        self.files.0 + self.models.0 + self.shaders.0 > 0
    }

    /// How far along, from 0 to 1, counting each file, model and shader alike.
    fn fraction(&self) -> f32 {
        let coming = self.files.0 + self.models.0 + self.shaders.0;
        let all = coming + self.files.1 + self.models.1 + self.shaders.1;
        if all == 0 {
            1.0
        } else {
            (all - coming) as f32 / all as f32
        }
    }

    fn status(&self) -> &'static str {
        if self.files.0 + self.models.0 > 0 {
            "Loading"
        } else if self.shaders.0 > 0 {
            "Preparing shaders"
        } else {
            "Ready"
        }
    }
}

/// Watches what is still coming, moves the bar, and starts the game once nothing has come
/// for a few frames.
fn watch(
    mut loading: ResMut<Loading>,
    shaders: Res<Shaders>,
    assets: Res<AssetServer>,
    spawner: Res<WorldInstanceSpawner>,
    models: Query<(&WorldAssetRoot, Option<&WorldInstance>)>,
    real: Res<Time<Real>>,
    options: Res<Options>,
    mut bar: Single<&mut Node, With<Bar>>,
    mut status: Single<&mut Text, With<Status>>,
    mut next: ResMut<NextState<Screen>>,
) {
    loading.up_for += real.delta();
    // A file that failed is not coming; whatever asked for it says so.
    let arrived = |id: UntypedAssetId| {
        assets.is_loaded_with_dependencies(id)
            || matches!(assets.load_state(id), LoadState::Failed(_))
    };
    loading.files.retain(|&id| !arrived(id));
    let placing = models
        .iter()
        .filter(|(model, instance)| {
            !matches!(assets.load_state(&model.0), LoadState::Failed(_))
                && !instance.is_some_and(|instance| spawner.instance_is_ready(**instance))
        })
        .count();
    let count = Count {
        files: (loading.files.len(), loading.asked - loading.files.len()),
        models: (placing, models.iter().len() - placing),
        shaders: (shaders.compiling, shaders.made),
    };
    loading.quiet = if count.coming() { 0 } else { loading.quiet + 1 };
    loading.shown = loading.shown.max(count.fraction());
    bar.width = percent(100.0 * loading.shown);
    if status.0 != count.status() {
        status.0 = count.status().into();
    }
    // A screen held up to be looked at stays (--loading).
    if options.hold_loading {
        return;
    }
    if loading.quiet >= QUIET_FRAMES {
        info!("Loaded in {:.1} s.", loading.up_for.as_secs_f32());
        next.set(Screen::Playing);
    } else if loading.up_for > GIVE_UP {
        warn!(
            "Still loading after {} s, starting anyway: {count:?}",
            GIVE_UP.as_secs()
        );
        next.set(Screen::Playing);
    }
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
        let count = Count {
            files: (1, 1),
            models: (0, 2),
            shaders: (3, 3),
        };
        assert!(count.coming());
        assert_eq!(count.fraction(), 0.6);
        assert_eq!(count.status(), "Loading");
        let compiling = Count {
            shaders: (1, 9),
            ..default()
        };
        assert_eq!(compiling.status(), "Preparing shaders");
        assert_eq!(compiling.fraction(), 0.9);
        let nothing = Count::default();
        assert!(!nothing.coming());
        assert_eq!(nothing.fraction(), 1.0);
    }
}
