//! The dial: a ring of the parts the builder can build, opened by holding the right mouse
//! button, or whatever the build menu is set to. Move the mouse toward a part and let go
//! to pick it. Under the aiming mark, a line says what is picked and what each button
//! would do.

use super::catalogue::PARTS;
use super::controls::{Action, Controls, Settings};
use super::flora::Growth;
use super::harvest::{self, Cutting};
use super::inventory::{Inventory, Tool};
use super::options::OptionsMenu;
use super::tool::{self, Builder};
use crate::shell::Options;
use bevy::{input::mouse::AccumulatedMouseMotion, prelude::*, window::CursorOptions};

const DIAL_SIZE: f32 = 650.0;
/// How far from the middle the parts' names sit.
const RING: f32 = 258.0;
const LABEL: Vec2 = Vec2::new(98.0, 36.0);
/// The mouse has to move this far from the middle before it points at anything.
const DEAD_ZONE: f32 = 28.0;
/// A press shorter than this that points at nothing is a tap, not a look at the dial.
const TAP_SECONDS: f32 = 0.22;
const PLAIN: Color = Color::srgba(0.07, 0.08, 0.1, 0.6);
const PICKED: Color = Color::srgba(0.9, 0.46, 0.17, 0.95);
const POINTED: Color = Color::srgba(0.35, 0.75, 0.85, 0.95);

#[derive(Resource, Default)]
pub struct Dial {
    pub open: bool,
    /// Where the mouse has moved to since the dial opened, from its middle.
    pointer: Vec2,
    /// The part the mouse points at.
    pointed: Option<usize>,
    /// How long the dial has been open, in seconds.
    held_for: f32,
    /// A number key picked a part while the dial's button was still down.
    pub closed_by_key: bool,
}

#[derive(Component)]
struct DialRoot;

#[derive(Component)]
struct DialLabel(usize);

#[derive(Component)]
struct DialMiddle;

#[derive(Component)]
struct Picked;

#[derive(Component)]
struct Prompt;

#[derive(Component)]
struct Legend;

/// The bar under the aiming mark that fills while something is being built, taken apart
/// or cut.
#[derive(Component)]
struct ProgressBar;

#[derive(Component)]
struct ProgressFill;

/// Everything on screen that is not the dial, hidden when taking pictures.
#[derive(Component)]
struct Hud;

pub fn plugin(app: &mut App) {
    app.init_resource::<Dial>()
        .add_systems(Startup, setup)
        .add_systems(Update, (draw, draw_progress).after(tool::Acting));
}

fn setup(mut commands: Commands) {
    // The aiming mark: a small ring in the middle of the view.
    commands.spawn((
        Hud,
        Node {
            position_type: PositionType::Absolute,
            left: percent(50),
            top: percent(50),
            width: px(8),
            height: px(8),
            margin: UiRect::all(px(-4)),
            border: UiRect::all(px(1.5)),
            border_radius: BorderRadius::MAX,
            ..default()
        },
        BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.85)),
    ));

    // Progress on whatever is being held down, just under the aiming mark.
    commands
        .spawn((
            ProgressBar,
            Node {
                position_type: PositionType::Absolute,
                left: percent(50),
                top: percent(50),
                width: px(132),
                height: px(7),
                margin: UiRect {
                    left: px(-66),
                    top: px(24),
                    ..default()
                },
                border_radius: BorderRadius::all(px(4)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.06, 0.08, 0.7)),
            Visibility::Hidden,
        ))
        .with_child((
            ProgressFill,
            Node {
                width: percent(0),
                height: percent(100),
                border_radius: BorderRadius::all(px(4)),
                ..default()
            },
            BackgroundColor(Color::WHITE),
        ));

    // The dial itself, hidden until its key is held.
    commands
        .spawn((
            DialRoot,
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            Visibility::Hidden,
        ))
        .with_children(|screen| {
            screen
                .spawn((
                    Node {
                        width: px(DIAL_SIZE),
                        height: px(DIAL_SIZE),
                        border_radius: BorderRadius::MAX,
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.05, 0.06, 0.08, 0.5)),
                ))
                .with_children(|disc| {
                    for (index, part) in PARTS.iter().enumerate() {
                        // Clockwise from the top.
                        let angle = index as f32 / PARTS.len() as f32 * std::f32::consts::TAU;
                        let middle = Vec2::splat(DIAL_SIZE * 0.5)
                            + Vec2::new(libm::sinf(angle), -libm::cosf(angle)) * RING;
                        disc.spawn((
                            DialLabel(index),
                            Node {
                                position_type: PositionType::Absolute,
                                left: px(middle.x - LABEL.x * 0.5),
                                top: px(middle.y - LABEL.y * 0.5),
                                width: px(LABEL.x),
                                height: px(LABEL.y),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                border_radius: BorderRadius::all(px(8)),
                                ..default()
                            },
                            BackgroundColor(PLAIN),
                        ))
                        .with_child((
                            Text::new(part.name),
                            TextFont::from_font_size(15.0),
                            TextColor(Color::WHITE),
                        ));
                    }
                    disc.spawn((
                        DialMiddle,
                        Text::new(""),
                        TextFont::from_font_size(17.0),
                        TextColor(Color::WHITE),
                        TextLayout::justify(Justify::Center),
                    ));
                });
        });

    // Along the bottom: what is picked, and what the buttons would do.
    commands
        .spawn((
            Hud,
            Node {
                position_type: PositionType::Absolute,
                bottom: px(92),
                width: percent(100),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(8),
                ..default()
            },
        ))
        .with_children(|column| {
            let pill = |colour: Color| {
                (
                    Node {
                        padding: UiRect::axes(px(16), px(7)),
                        border_radius: BorderRadius::all(px(7)),
                        ..default()
                    },
                    BackgroundColor(colour),
                )
            };
            column.spawn((
                Legend,
                Text::new(""),
                TextFont::from_font_size(12.5),
                TextColor(Color::srgba(1.0, 1.0, 1.0, 0.8)),
                TextShadow {
                    offset: Vec2::splat(1.0),
                    color: Color::srgba(0.0, 0.0, 0.0, 0.7),
                },
            ));
            column.spawn(pill(PLAIN)).with_child((
                Prompt,
                Text::new(""),
                TextFont::from_font_size(16.0),
                TextColor(Color::srgba(1.0, 1.0, 1.0, 0.95)),
            ));
            column.spawn(pill(PICKED)).with_child((
                Picked,
                Text::new(""),
                TextFont::from_font_size(15.0),
                TextColor(Color::WHITE),
            ));
        });
}

/// Opens the dial while the build menu's button is held, follows the mouse round it, and
/// picks the part pointed at when the button is let go. A tap that points at nothing is
/// a cancel instead. Runs before the player's view turns, which it holds still while the
/// dial is open.
pub fn steer(
    time: Res<Time<Real>>,
    keys: Res<ButtonInput<KeyCode>>,
    controls: Controls,
    motion: Res<AccumulatedMouseMotion>,
    inventory: Res<Inventory>,
    menu: Res<OptionsMenu>,
    mut dial: ResMut<Dial>,
    mut builder: ResMut<Builder>,
) {
    // The dial belongs to the builder, and is put away with it.
    if inventory.open || menu.open || inventory.holding() != Some(Tool::Builder) {
        dial.open = false;
        return;
    }
    let held = controls.held(Action::BuildMenu);
    // After a number has picked a part, the button has to be let go before it opens again.
    dial.closed_by_key &= held;
    if !dial.open {
        if dial.closed_by_key {
            return;
        }
        if held && !builder.is_carrying() {
            *dial = Dial {
                open: true,
                ..default()
            };
        } else if controls.went_down(Action::BuildMenu) {
            // Nothing to choose while carrying something: the button only puts it back.
            builder.cancel = true;
        }
        return;
    }
    dial.held_for += time.delta_secs();
    dial.pointer = (dial.pointer + motion.delta).clamp_length_max(RING);
    dial.pointed = (dial.pointer.length() > DEAD_ZONE).then(|| {
        // Clockwise from the top, as the names are laid out. Down the screen is +y.
        let angle = libm::atan2f(dial.pointer.x, -dial.pointer.y);
        let turn = angle.rem_euclid(std::f32::consts::TAU) / std::f32::consts::TAU;
        (turn * PARTS.len() as f32).round() as usize % PARTS.len()
    });
    // A number picks a part outright: the first ten, clockwise from the top.
    const NUMBERS: [KeyCode; 10] = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
        KeyCode::Digit7,
        KeyCode::Digit8,
        KeyCode::Digit9,
        KeyCode::Digit0,
    ];
    if let Some(part) = NUMBERS.iter().position(|&key| keys.just_pressed(key)) {
        builder.pick(part);
        dial.open = false;
        dial.closed_by_key = true;
        return;
    }
    if !held {
        match dial.pointed {
            Some(part) => builder.pick(part),
            None if dial.held_for < TAP_SECONDS => builder.cancel = true,
            None => {}
        }
        dial.open = false;
    }
}

fn draw(
    dial: Res<Dial>,
    settings: Res<Settings>,
    builder: Res<Builder>,
    inventory: Res<Inventory>,
    cutting: Res<Cutting>,
    growth: Query<&Growth>,
    options: Res<Options>,
    cursor: Single<&CursorOptions>,
    mut root: Single<&mut Visibility, (With<DialRoot>, Without<Hud>)>,
    mut hud: Query<&mut Visibility, With<Hud>>,
    mut labels: Query<(&DialLabel, &mut BackgroundColor)>,
    mut texts: ParamSet<(
        Single<&mut Text, With<DialMiddle>>,
        Single<&mut Text, With<Picked>>,
        Single<&mut Text, With<Prompt>>,
        Single<&mut Text, With<Legend>>,
    )>,
) {
    let set = |text: &mut Text, to: String| {
        if text.0 != to {
            text.0 = to;
        }
    };
    // A tap should not flash the dial up.
    let wanted = if dial.open && (dial.held_for > TAP_SECONDS * 0.5 || dial.pointed.is_some()) {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    if **root != wanted {
        **root = wanted;
    }
    // A fixed camera is for taking pictures, which should show the scene alone.
    let wanted = if options.vantage.is_some() {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };
    for mut hud in &mut hud {
        if *hud != wanted {
            *hud = wanted;
        }
    }
    for (label, mut background) in &mut labels {
        let colour = if dial.pointed == Some(label.0) {
            POINTED
        } else if builder.part == label.0 && !builder.stowed {
            PICKED
        } else {
            PLAIN
        };
        if background.0 != colour {
            background.0 = colour;
        }
    }
    // In the middle: the part pointed at, and the kinds of it there are.
    let shown = &PARTS[dial.pointed.unwrap_or(builder.part)];
    let kinds: Vec<&str> = (0..shown.variants())
        .filter_map(|variant| shown.variant_name(variant))
        .collect();
    set(
        &mut texts.p0(),
        if kinds.len() > 1 {
            format!("{}\n\n{}", shown.name, kinds.join("\n"))
        } else {
            shown.name.to_string()
        },
    );

    let holding = inventory.holding();
    let part = builder.picked();
    set(
        &mut texts.p1(),
        match (holding, part.variant_name(builder.variant())) {
            (None, _) => "Empty hand".to_string(),
            (Some(Tool::Cutter), _) => "Cutter".to_string(),
            _ if builder.stowed => "Builder:  nothing picked".to_string(),
            (_, Some(variant)) if part.variants() > 1 => format!(
                "{}:  {}  ({} of {})",
                part.name,
                variant,
                builder.variant() + 1,
                part.variants()
            ),
            (_, Some(variant)) => format!("{}:  {}", part.name, variant),
            (_, None) => part.name.to_string(),
        } + if builder.snap || holding != Some(Tool::Builder) {
            ""
        } else {
            "     grid off"
        },
    );
    set(
        &mut texts.p2(),
        if tool::waiting(&cursor, &options) && !inventory.open {
            "Click to take control".to_string()
        } else if inventory.open {
            format!("{}  close the inventory", settings.key(Action::Inventory))
        } else if dial.open {
            "Move the mouse to a part and let go, or press its number".to_string()
        } else {
            match holding {
                Some(Tool::Builder) => tool::prompt(&builder, &settings),
                Some(Tool::Cutter) => harvest::prompt(&cutting, &growth, &settings),
                None => format!(
                    "1 to 6  pick a tool      {}  inventory",
                    settings.key(Action::Inventory)
                ),
            }
        },
    );
    set(
        &mut texts.p3(),
        if dial.open || inventory.open || holding != Some(Tool::Builder) {
            String::new()
        } else {
            tool::legend(&builder, &settings)
        },
    );
}

/// Fills the bar under the aiming mark while a job is held down: building, taking apart
/// or cutting.
fn draw_progress(
    builder: Res<Builder>,
    cutting: Res<Cutting>,
    mut bar: Single<&mut Visibility, With<ProgressBar>>,
    mut fill: Single<(&mut Node, &mut BackgroundColor), With<ProgressFill>>,
) {
    let progress = builder
        .progress()
        .or(cutting.progress().map(|done| (done, false)));
    let wanted = if progress.is_some() {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    if **bar != wanted {
        **bar = wanted;
    }
    if let Some((done, removing)) = progress {
        fill.0.width = percent(done * 100.0);
        let colour = if removing {
            Color::srgb(1.0, 0.45, 0.4)
        } else {
            Color::WHITE
        };
        if fill.1.0 != colour {
            fill.1.0 = colour;
        }
    }
}
