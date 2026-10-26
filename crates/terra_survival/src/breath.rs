//! Breath on screen (D523, D74): a small bar over the vitals, shown only while the head is
//! under water or the breath is coming back, and faded out once it is full again. The breath
//! is the mover's (`mover::Body::breath`, its kind's `breath_s`), read, never kept here.

use bevy::{picking::Pickable, prelude::*};
use terra_walker::movement::Movers;
use terra_walker::{IsLocalPlayer, Player};

/// How fast the bar comes in under water and goes once full, a share of fully shown a second.
const FADE_IN: f32 = 8.0;
const FADE_OUT: f32 = 2.0;
/// Its colour: the air's, pale, so it is told from health's red, stamina's gold, food's
/// green and mana's blue (`vitals`).
const AIR: (f32, f32, f32) = (0.78, 0.9, 0.95);
/// Where it sits: over the vitals' three bars (`vitals`, 24 px in, 24 up, 9 px each, 5
/// apart), and shorter than them, as it is small and only now and then there.
const LEFT_PX: f32 = 24.0;
const BOTTOM_PX: f32 = 24.0 + 3.0 * 9.0 + 2.0 * 5.0 + 9.0;
const WIDTH_PX: f32 = 130.0;

/// The bar as shown: how far it is faded in (0 hidden, 1 shown) and the share of breath left.
#[derive(Component, Clone, Copy, PartialEq, Debug, Default)]
pub struct Gauge {
    pub shown: f32,
    pub left: f32,
}

impl Gauge {
    /// The bar after a frame of `dt` s, by the body's breath left, s, of its kind's `full`,
    /// and whether its mouth is under water: wanted while under or while the breath is not
    /// yet full, faded in fast and out slowly.
    pub fn step(&mut self, breath: f32, full: f32, under: bool, dt: f32) {
        self.left = if full > 0.0 {
            (breath / full).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let wanted = under || breath < full;
        self.shown = if wanted {
            (self.shown + FADE_IN * dt).min(1.0)
        } else {
            (self.shown - FADE_OUT * dt).max(0.0)
        };
    }
}

/// The bar's track, which carries the gauge.
#[derive(Component)]
struct Track;

/// The bar's fill.
#[derive(Component)]
struct Fill;

/// The bar's name beside it.
#[derive(Component)]
struct Label;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, setup).add_systems(Update, show);
}

fn setup(mut commands: Commands) {
    commands
        .spawn((
            Track,
            Gauge::default(),
            Node {
                position_type: PositionType::Absolute,
                left: px(LEFT_PX),
                bottom: px(BOTTOM_PX),
                width: px(WIDTH_PX),
                height: px(7),
                border_radius: BorderRadius::all(px(3)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Visibility::Hidden,
            Pickable::IGNORE,
        ))
        .with_child((
            Fill,
            Node {
                width: percent(100),
                height: percent(100),
                border_radius: BorderRadius::all(px(3)),
                ..default()
            },
            BackgroundColor(Color::NONE),
        ))
        .with_child((
            Label,
            Node {
                position_type: PositionType::Absolute,
                left: px(WIDTH_PX + 8.0),
                top: px(-5),
                ..default()
            },
            Text::new("Breath"),
            TextFont::from_font_size(12.0),
            TextColor(Color::NONE),
        ));
}

fn show(
    time: Res<Time>,
    movers: Option<Res<Movers>>,
    player: Option<Single<Entity, (With<Player>, IsLocalPlayer)>>,
    options: Res<terra_world::shell::Options>,
    mut track: Single<(&mut Gauge, &mut BackgroundColor, &mut Visibility), With<Track>>,
    mut fill: Single<(&mut Node, &mut BackgroundColor), (With<Fill>, Without<Track>)>,
    mut label: Single<&mut TextColor, With<Label>>,
) {
    let body = movers
        .as_ref()
        .zip(player)
        .and_then(|(movers, player)| movers.body(*player));
    let (gauge, track_colour, visibility) = &mut *track;
    match body {
        Some(body) => gauge.step(
            body.breath,
            body.kind.breath_s,
            body.under,
            time.delta_secs(),
        ),
        None => **gauge = Gauge::default(),
    }
    let wanted = if options.shows_interface() && gauge.shown > 0.0 {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    visibility.set_if_neq(wanted);
    if wanted == Visibility::Hidden {
        return;
    }
    let a = gauge.shown;
    track_colour.0 = Color::srgba(0.0, 0.0, 0.0, 0.45 * a);
    let (node, fill_colour) = &mut *fill;
    let width = percent(100.0 * gauge.left);
    if node.width != width {
        node.width = width;
    }
    fill_colour.0 = Color::srgba(AIR.0, AIR.1, AIR.2, a);
    label.0 = Color::srgba(1.0, 1.0, 1.0, 0.85 * a);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A man's breath (movers.ron): 30 s under water, 4 s back from none.
    const FULL: f32 = 30.0;
    const BACK_S: f32 = 4.0;
    const DT: f32 = 1.0 / 60.0;

    /// Frames of a body's breath, as the mover keeps it: used a second a second under water,
    /// refilled at `FULL / BACK_S` a second at the surface, never past full.
    fn run(gauge: &mut Gauge, breath: &mut f32, under: bool, seconds: f32) {
        for _ in 0..(seconds / DT).round() as usize {
            *breath = if under {
                (*breath - DT).max(0.0)
            } else {
                (*breath + FULL / BACK_S * DT).min(FULL)
            };
            gauge.step(*breath, FULL, under, DT);
        }
    }

    #[test]
    fn the_bar_shows_only_under_water_and_while_breath_comes_back() {
        let mut gauge = Gauge::default();
        let mut breath = FULL;
        // Full, above water: never shown.
        run(&mut gauge, &mut breath, false, 2.0);
        assert_eq!(gauge.shown, 0.0);
        assert_eq!(gauge.left, 1.0);
        // Under water: shown at once, though the breath is still all but full.
        run(&mut gauge, &mut breath, true, 0.2);
        assert_eq!(gauge.shown, 1.0);
        assert!(gauge.left < 1.0 && gauge.left > 0.99, "{}", gauge.left);
        // Twenty seconds under: a third left.
        run(&mut gauge, &mut breath, true, 19.8);
        assert_eq!(gauge.shown, 1.0);
        assert!((gauge.left - 1.0 / 3.0).abs() < 0.01, "{}", gauge.left);
        // Surfaced: still shown while it refills, the bar climbing.
        run(&mut gauge, &mut breath, false, 1.0);
        assert_eq!(gauge.shown, 1.0);
        assert!(
            (gauge.left - (1.0 / 3.0 + 1.0 / BACK_S)).abs() < 0.01,
            "{}",
            gauge.left
        );
        // Full again (the rest, five twelfths, in 5/3 s): fading, then gone.
        run(&mut gauge, &mut breath, false, 5.0 / 3.0 + 0.15);
        assert_eq!(gauge.left, 1.0);
        assert!(gauge.shown > 0.0 && gauge.shown < 1.0, "{}", gauge.shown);
        run(&mut gauge, &mut breath, false, 1.0);
        assert_eq!(gauge.shown, 0.0);
    }

    #[test]
    fn out_of_breath_the_bar_is_empty_and_stays_shown() {
        let mut gauge = Gauge::default();
        let mut breath = FULL;
        run(&mut gauge, &mut breath, true, FULL + 5.0);
        assert_eq!((gauge.shown, gauge.left), (1.0, 0.0));
    }
}
