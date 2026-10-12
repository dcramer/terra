//! The wheel of kinds: a ring of the variants of whatever the hammer has picked, such as
//! room sizes or window shapes, opened by holding R. Move the mouse toward one and let
//! go to pick it; a tap of R steps to the next instead. What to build is chosen in the
//! build menu (`build_menu`). Under the aiming mark, a line says what is picked and what
//! each button would do.

use super::IsLocalPlayer;
use super::build_menu::BuildMenu;
use super::controls::{Action, Controls, Settings};
use super::flora::Growth;
use super::harvest::{self, Cutting};
use super::interact::Focus;
use super::inventory::{Belongings, InventoryScreen};
use super::options::OptionsMenu;
use super::tool::{self, Builder};
use crate::shell::Options;
use bevy::{input::mouse::AccumulatedMouseMotion, prelude::*, window::CursorOptions};

const DIAL_SIZE: f32 = 650.0;
/// The most kinds a part has.
const MOST_KINDS: usize = 8;
/// How far from the middle the kinds' names sit.
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
}

#[derive(Component)]
struct DialRoot;

#[derive(Component)]
struct DialLabel(usize);

#[derive(Component)]
struct DialLabelText(usize);

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
                    // As many places as the most kinds any part has; each is set round
                    // the ring, and named, for the part picked when the wheel opens.
                    for index in 0..MOST_KINDS {
                        disc.spawn((
                            DialLabel(index),
                            Node {
                                position_type: PositionType::Absolute,
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
                            DialLabelText(index),
                            Text::new(""),
                            TextFont::from_font_size(14.0),
                            TextColor(Color::WHITE),
                            TextLayout::justify(Justify::Center),
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

/// Opens the wheel while R is held with a part picked, follows the mouse round it, and
/// picks the kind pointed at when R is let go. A tap steps to the next kind instead, or,
/// with sprint held, the one before. Runs before the player's view turns, which it holds
/// still while the wheel is open.
#[allow(clippy::too_many_arguments)]
pub fn steer(
    time: Res<Time<Real>>,
    controls: Controls,
    motion: Res<AccumulatedMouseMotion>,
    inventory: Res<InventoryScreen>,
    mut belongings: Belongings,
    menu: Res<OptionsMenu>,
    build_menu: Res<BuildMenu>,
    mut dial: ResMut<Dial>,
    mut builder: Single<&mut Builder, IsLocalPlayer>,
) {
    let Some(belongings) = belongings.mine() else {
        return;
    };
    // The wheel belongs to the hammer, with a part in it.
    let usable = !inventory.open
        && !menu.open
        && !build_menu.open
        && belongings.tool_in_hand().is_some_and(|tool| tool.builds)
        && !builder.stowed
        && !builder.is_carrying()
        && !builder.is_working();
    if !usable {
        dial.open = false;
        return;
    }
    let held = controls.held(Action::NextKind);
    if !dial.open {
        if controls.went_down(Action::NextKind) {
            *dial = Dial {
                open: true,
                ..default()
            };
        }
        return;
    }
    let count = builder.picked().variants().min(MOST_KINDS);
    dial.held_for += time.delta_secs();
    dial.pointer = (dial.pointer + motion.delta).clamp_length_max(RING);
    dial.pointed = (count > 1 && dial.pointer.length() > DEAD_ZONE).then(|| {
        // Clockwise from the top, as the names are laid out. Down the screen is +y.
        let angle = libm::atan2f(dial.pointer.x, -dial.pointer.y);
        let turn = angle.rem_euclid(std::f32::consts::TAU) / std::f32::consts::TAU;
        (turn * count as f32).round() as usize % count
    });
    if held {
        return;
    }
    let part = builder.part;
    match dial.pointed {
        Some(kind) => builder.choose_variant(part, kind),
        None if dial.held_for < TAP_SECONDS && count > 1 => {
            let back = controls.held(Action::Sprint);
            let next = (builder.variant_of(part) + if back { count - 1 } else { 1 }) % count;
            builder.choose_variant(part, next);
        }
        None => {}
    }
    dial.open = false;
}

fn draw(
    dial: Res<Dial>,
    settings: Res<Settings>,
    builder: Single<&Builder, IsLocalPlayer>,
    inventory: Res<InventoryScreen>,
    focus: Res<Focus>,
    mut belongings: Belongings,
    cutting: Single<&Cutting, IsLocalPlayer>,
    digging: Single<&super::dig::Digging, IsLocalPlayer>,
    growth: Query<&Growth>,
    options: Res<Options>,
    cursor: Single<&CursorOptions>,
    mut root: Single<&mut Visibility, (With<DialRoot>, Without<Hud>)>,
    mut hud: Query<&mut Visibility, With<Hud>>,
    mut labels: Query<(&DialLabel, &mut Node, &mut BackgroundColor)>,
    mut texts: ParamSet<(
        Single<&mut Text, With<DialMiddle>>,
        Single<&mut Text, With<Picked>>,
        Single<&mut Text, With<Prompt>>,
        Single<&mut Text, With<Legend>>,
        Query<(&DialLabelText, &mut Text)>,
    )>,
) {
    let Some(belongings) = belongings.mine() else {
        return;
    };
    let set = |text: &mut Text, to: String| {
        if text.0 != to {
            text.0 = to;
        }
    };
    // A tap should not flash the wheel up, nor should a part with one kind.
    let count = builder.picked().variants().min(MOST_KINDS);
    let wanted = if dial.open
        && count > 1
        && (dial.held_for > TAP_SECONDS * 0.5 || dial.pointed.is_some())
    {
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
    // The picked part's kinds, round the ring clockwise from the top.
    for (label, mut node, mut background) in &mut labels {
        let shown = label.0 < count;
        let display = if shown { Display::Flex } else { Display::None };
        if node.display != display {
            node.display = display;
        }
        if shown {
            let angle = label.0 as f32 / count as f32 * std::f32::consts::TAU;
            let middle = Vec2::splat(DIAL_SIZE * 0.5)
                + Vec2::new(libm::sinf(angle), -libm::cosf(angle)) * RING;
            node.left = px(middle.x - LABEL.x * 0.5);
            node.top = px(middle.y - LABEL.y * 0.5);
        }
        let colour = if dial.pointed == Some(label.0) {
            POINTED
        } else if builder.variant() == label.0 {
            PICKED
        } else {
            PLAIN
        };
        if background.0 != colour {
            background.0 = colour;
        }
    }
    let part = builder.picked();
    for (label, mut text) in &mut texts.p4() {
        let name = part
            .variant_name(label.0.min(count.saturating_sub(1)))
            .unwrap_or("");
        set(&mut text, name.to_string());
    }
    // In the middle: the part, and the kind pointed at.
    set(
        &mut texts.p0(),
        match dial.pointed.and_then(|kind| part.variant_name(kind)) {
            Some(kind) => format!("{}\n\n{kind}", part.name),
            None => part.name.to_string(),
        },
    );

    let holding = belongings.holding();
    let items = belongings.items();
    let hammer = belongings.tool_in_hand().is_some_and(|tool| tool.builds);
    let part = builder.picked();
    set(
        &mut texts.p1(),
        match (holding, part.variant_name(builder.variant())) {
            (None, _) => match belongings.broken_in_hand() {
                Some(tool) => format!("{}  (broken)", items.name(tool)),
                None => "Empty hand".to_string(),
            },
            (Some(tool), _) if !hammer => items.name(tool).to_string(),
            _ if builder.stowed => "Hammer:  nothing picked".to_string(),
            (_, Some(variant)) if part.variants() > 1 => format!(
                "{}:  {}  ({} of {})",
                part.name,
                variant,
                builder.variant() + 1,
                part.variants()
            ),
            (_, Some(variant)) => format!("{}:  {}", part.name, variant),
            (_, None) => part.name.to_string(),
        } + if builder.snap || !hammer {
            ""
        } else {
            "     grid off"
        },
    );
    set(
        &mut texts.p2(),
        if tool::waiting(&cursor, &options) && !inventory.open {
            "Click to take control".to_string()
        } else if inventory.at.is_some() {
            format!("{}  leave the station", settings.key(Action::Interact))
        } else if inventory.open {
            format!("{}  close the inventory", settings.key(Action::Inventory))
        } else if dial.open && builder.picked().variants() > 1 {
            "Move the mouse to a kind and let go".to_string()
        } else {
            let doing = match holding {
                Some(_) if hammer => tool::prompt(&builder, &settings, items),
                // A pickaxe on bare ground digs it.
                Some(_) if cutting.target.is_none() && digging.target.is_some() => {
                    match digging.progress() {
                        Some(_) => "Digging      Let go to stop".to_string(),
                        None => format!(
                            "{}  dig      Stone and ore come out of the ground",
                            settings.hold(Action::Use)
                        ),
                    }
                }
                Some(tool) => match items.tool(tool) {
                    Some(does) => {
                        harvest::prompt(items.name(tool), does, &cutting, &growth, &settings)
                    }
                    None => String::new(),
                },
                None if let Some(kind) = belongings.broken_in_hand() => {
                    match super::wear::mended_at(items, kind) {
                        Some(station) => format!(
                            "Broken: mend it at a {} in your claim",
                            station.name().to_lowercase()
                        ),
                        None => "Broken".to_string(),
                    }
                }
                None => format!(
                    "1 to 8  pick a tool      {}  inventory",
                    settings.key(Action::Inventory)
                ),
            };
            match focus.what {
                Some(what) => format!(
                    "{}  {}      {doing}",
                    settings.key(Action::Interact),
                    what.verb()
                ),
                None => doing,
            }
        },
    );
    set(
        &mut texts.p3(),
        if dial.open || inventory.open || !hammer {
            String::new()
        } else {
            tool::legend(&builder, &settings)
        },
    );
}

/// Fills the bar under the aiming mark while a job is held down: building, taking apart,
/// cutting or digging.
fn draw_progress(
    builder: Single<&Builder, IsLocalPlayer>,
    cutting: Single<&Cutting, IsLocalPlayer>,
    digging: Single<&super::dig::Digging, IsLocalPlayer>,
    mut bar: Single<&mut Visibility, With<ProgressBar>>,
    mut fill: Single<(&mut Node, &mut BackgroundColor), With<ProgressFill>>,
) {
    let progress = builder
        .progress()
        .or(cutting.progress().map(|done| (done, false)))
        .or(digging.progress().map(|done| (done, false)));
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
