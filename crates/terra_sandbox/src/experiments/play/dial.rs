//! The dial: a ring of the parts the builder can build, opened by holding a key. Move
//! the mouse toward a part and let go to pick it. Under the aiming mark, a line says
//! what is picked and what each button would do.

use super::View;
use super::catalogue::PARTS;
use super::tool::{self, Builder};
use crate::shell::Options;
use bevy::{input::mouse::AccumulatedMouseMotion, prelude::*, window::CursorOptions};

const DIAL_SIZE: f32 = 600.0;
/// How far from the middle the parts' names sit.
const RING: f32 = 236.0;
const LABEL: Vec2 = Vec2::new(98.0, 36.0);
/// The mouse has to move this far from the middle before it points at anything.
const DEAD_ZONE: f32 = 28.0;
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

/// Everything on screen that is not the dial, hidden when taking pictures.
#[derive(Component)]
struct Hud;

pub fn plugin(app: &mut App) {
    app.init_resource::<Dial>()
        .add_systems(Startup, setup)
        .add_systems(Update, draw.after(tool::Acting));
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
                bottom: px(28),
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

/// Opens the dial while its key is held, follows the mouse round it, and picks the part
/// pointed at when the key is let go. Runs before the player's view turns, which it
/// holds still while the dial is open.
pub fn steer(
    keys: Res<ButtonInput<KeyCode>>,
    motion: Res<AccumulatedMouseMotion>,
    mut dial: ResMut<Dial>,
    mut builder: ResMut<Builder>,
) {
    if keys.just_pressed(KeyCode::KeyQ) {
        *dial = Dial {
            open: true,
            ..default()
        };
    }
    if !dial.open {
        return;
    }
    dial.pointer = (dial.pointer + motion.delta).clamp_length_max(RING);
    dial.pointed = (dial.pointer.length() > DEAD_ZONE).then(|| {
        // Clockwise from the top, as the names are laid out. Down the screen is +y.
        let angle = libm::atan2f(dial.pointer.x, -dial.pointer.y);
        let turn = angle.rem_euclid(std::f32::consts::TAU) / std::f32::consts::TAU;
        (turn * PARTS.len() as f32).round() as usize % PARTS.len()
    });
    if !keys.pressed(KeyCode::KeyQ) {
        if let Some(part) = dial.pointed {
            builder.pick(part);
        }
        dial.open = false;
    }
}

fn draw(
    dial: Res<Dial>,
    builder: Res<Builder>,
    options: Res<Options>,
    view: Res<View>,
    cursor: Single<&CursorOptions>,
    mut root: Single<&mut Visibility, (With<DialRoot>, Without<Hud>)>,
    mut hud: Query<&mut Visibility, With<Hud>>,
    mut labels: Query<(&DialLabel, &mut BackgroundColor)>,
    mut middle: Single<&mut Text, (With<DialMiddle>, Without<Picked>, Without<Prompt>)>,
    mut picked: Single<&mut Text, (With<Picked>, Without<DialMiddle>, Without<Prompt>)>,
    mut prompt: Single<&mut Text, (With<Prompt>, Without<DialMiddle>, Without<Picked>)>,
) {
    let set = |text: &mut Text, to: String| {
        if text.0 != to {
            text.0 = to;
        }
    };
    let wanted = if dial.open {
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
        } else if builder.part == label.0 {
            PICKED
        } else {
            PLAIN
        };
        if background.0 != colour {
            background.0 = colour;
        }
    }
    let shown = &PARTS[dial.pointed.unwrap_or(builder.part)];
    set(
        &mut middle,
        match shown.variants() {
            1 => shown.name.to_string(),
            kinds => format!("{}\n{kinds} kinds", shown.name),
        },
    );

    let part = builder.picked();
    set(
        &mut picked,
        match part.variant_name(builder.variant()) {
            Some(variant) if part.variants() > 1 => format!(
                "{}:  {}  ({} of {})",
                part.name,
                variant,
                builder.variant() + 1,
                part.variants()
            ),
            Some(variant) => format!("{}:  {}", part.name, variant),
            None => part.name.to_string(),
        },
    );
    set(
        &mut prompt,
        if tool::waiting(&cursor, &options, &view) {
            "Click to take control".to_string()
        } else if dial.open {
            "Move the mouse to a part and let go".to_string()
        } else {
            format!("{}      Hold Q  parts", tool::prompt(&builder))
        },
    );
}
