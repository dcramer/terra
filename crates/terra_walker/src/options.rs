//! The options menu (D80), opened with Esc: the player's settings on the left, and every
//! control on the right, each of which can be set to another key or mouse button.
//!
//!   Esc           open and close the menu; while a control waits for a key, give up
//!   Click a control, then press the key or mouse button it should be. Any key can be
//!   one, Backspace included. Right-click a control to clear it.
//!
//! What can be set, and how it is saved, is in `controls.rs`. Every change is saved as
//! it is made.

use crate::button::{button, left_click, pointer_on};
use crate::camera::grab_mouse;
use crate::controls::{Action, Binding, FIELD_OF_VIEW, LOOK_SPEED, Settings, VOLUME, capitalised};
use bevy::{
    input::mouse::{AccumulatedMouseScroll, MouseScrollUnit},
    picking::hover::Hovered,
    prelude::*,
    ui::Pressed,
    window::{CursorOptions, MonitorSelection, PresentMode, PrimaryWindow, WindowMode},
};
use terra_world::planet::MainCamera;
use terra_world::shell::Options;
use terra_world::{
    pacing::{FrameLimit, ScreenRate},
    quality::{Asked, Forced, Quality},
};

const PLAIN: Color = Color::srgba(0.07, 0.08, 0.1, 0.6);
const PICKED: Color = Color::srgba(0.9, 0.46, 0.17, 0.95);
const POINTED: Color = Color::srgba(0.35, 0.75, 0.85, 0.95);
const DIM: Color = Color::srgba(1.0, 1.0, 1.0, 0.7);
const ROW_HEIGHT: f32 = 27.0;
/// How many controls show at once; the rest are scrolled to.
const CONTROLS_SHOWN: usize = 20;

#[derive(Resource, Default)]
pub struct OptionsMenu {
    pub open: bool,
    /// The action, and which of its two bindings, waiting for a key or mouse button.
    listening: Option<(Action, usize)>,
    /// The last key pressed for a control is kept for something else.
    refused: bool,
    /// A press was taken for a control this frame, and is not also a click on the menu.
    taken: bool,
}

/// Whether changes are saved. A scripted run saves nothing.
#[derive(Resource)]
struct Saving(bool);

/// The settings that are not controls.
#[derive(Clone, Copy, PartialEq, Eq)]
enum General {
    LookSpeed,
    InvertLook,
    FieldOfView,
    FullScreen,
    Vsync,
    FrameLimit,
    Graphics,
    Volume(Part),
}

/// A part of the sound, with its own volume.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Part {
    Master,
    Music,
    Ambience,
    Effects,
}

impl Part {
    fn of(self, settings: &mut Settings) -> &mut f32 {
        match self {
            Part::Master => &mut settings.master_volume,
            Part::Music => &mut settings.music_volume,
            Part::Ambience => &mut settings.ambience_volume,
            Part::Effects => &mut settings.effects_volume,
        }
    }

    fn get(self, settings: &Settings) -> f32 {
        match self {
            Part::Master => settings.master_volume,
            Part::Music => settings.music_volume,
            Part::Ambience => settings.ambience_volume,
            Part::Effects => settings.effects_volume,
        }
    }
}

impl General {
    const ALL: [General; 11] = [
        General::LookSpeed,
        General::InvertLook,
        General::FieldOfView,
        General::FullScreen,
        General::Vsync,
        General::FrameLimit,
        General::Graphics,
        General::Volume(Part::Master),
        General::Volume(Part::Music),
        General::Volume(Part::Ambience),
        General::Volume(Part::Effects),
    ];

    fn name(self) -> &'static str {
        match self {
            General::LookSpeed => "Mouse look speed",
            General::InvertLook => "Invert mouse look",
            General::FieldOfView => "Field of view",
            General::FullScreen => "Full screen",
            General::Vsync => "Vsync",
            General::FrameLimit => "Frame-rate limit",
            General::Graphics => "Graphics",
            General::Volume(Part::Master) => "Master volume",
            General::Volume(Part::Music) => "Music",
            General::Volume(Part::Ambience) => "Ambience",
            General::Volume(Part::Effects) => "Effects",
        }
    }

    /// What it is set to; `screen` is the screen's refresh rate in millihertz, if known.
    fn value(self, settings: &Settings, screen: Option<u32>) -> String {
        let on_off = |on: bool| if on { "On" } else { "Off" }.to_string();
        match self {
            General::LookSpeed => format!("{:.1}", settings.look_speed),
            General::InvertLook => on_off(settings.invert_look),
            General::FieldOfView => format!("{}", settings.field_of_view),
            General::FullScreen => on_off(settings.full_screen),
            General::Vsync => on_off(settings.vsync),
            // With vsync on the screen paces frames, and no limit applies.
            General::FrameLimit if settings.vsync => "Vsync".to_string(),
            General::FrameLimit => match settings.frame_limit {
                FrameLimit::Off => "Off".to_string(),
                FrameLimit::Fps(fps) => format!("{fps} fps"),
                FrameLimit::Screen => match screen {
                    Some(millihertz) => {
                        format!("Screen {}", libm::roundf(millihertz as f32 / 1000.0))
                    }
                    None => "Screen".to_string(),
                },
            },
            General::Graphics => match settings.graphics {
                Some(quality) => capitalised(quality.name().to_string()),
                None => "Automatic".to_string(),
            },
            General::Volume(part) => format!("{}%", libm::roundf(part.get(settings) * 100.0)),
        }
    }

    /// Whether it has a range of values, stepped down and up, rather than on and off.
    fn ranged(self) -> bool {
        matches!(
            self,
            General::LookSpeed | General::FieldOfView | General::Volume(_)
        )
    }

    fn change(self, settings: &mut Settings, up: bool) {
        match self {
            General::LookSpeed => settings.look_speed = LOOK_SPEED.nudge(settings.look_speed, up),
            General::FieldOfView => {
                settings.field_of_view = FIELD_OF_VIEW.nudge(settings.field_of_view, up);
            }
            General::InvertLook => settings.invert_look = !settings.invert_look,
            General::FullScreen => settings.full_screen = !settings.full_screen,
            General::Vsync => settings.vsync = !settings.vsync,
            // The next limit, round again; a limit applies with vsync off, so choosing
            // one turns vsync off.
            General::FrameLimit => {
                let order = FrameLimit::CHOICES;
                let at = order
                    .iter()
                    .position(|limit| *limit == settings.frame_limit)
                    .unwrap_or(0);
                let step = if up { 1 } else { order.len() - 1 };
                settings.frame_limit = order[(at + step) % order.len()];
                settings.vsync = false;
            }
            // Automatic, then each preset from low to high, round again.
            General::Graphics => {
                let order: Vec<Option<Quality>> = std::iter::once(None)
                    .chain(Quality::ALL.into_iter().map(Some))
                    .collect();
                let at = order
                    .iter()
                    .position(|q| *q == settings.graphics)
                    .unwrap_or(0);
                let step = if up { 1 } else { order.len() - 1 };
                settings.graphics = order[(at + step) % order.len()];
            }
            General::Volume(part) => {
                let volume = part.of(settings);
                *volume = VOLUME.nudge(*volume, up);
            }
        }
    }
}

/// Something in the menu that can be clicked.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Control {
    /// Step a setting down or up; a setting that is on or off just switches.
    Change(General, bool),
    /// Wait for a key for one of an action's two bindings.
    Bind(Action, usize),
    Reset,
    Close,
}

/// Text that shows something that can change.
#[derive(Component, Clone, Copy)]
enum Shows {
    Value(General),
    Binding(Action, usize),
    Note,
}

#[derive(Component)]
struct Screen;

/// The list of controls, which scrolls.
#[derive(Component)]
struct ControlsList;

pub fn plugin(app: &mut App) {
    let scripted = app.world().resource::<Options>().scripted();
    // A scripted run uses the defaults, so it does the same thing on every machine.
    let settings = if scripted {
        Settings::default()
    } else {
        Settings::load()
    };
    app.insert_resource(settings)
        .insert_resource(Saving(!scripted))
        .init_resource::<OptionsMenu>()
        .add_systems(Startup, setup)
        // After the game has read the controls, so the click that closes the menu or
        // sets a control to a mouse button is not also a click in the game.
        .add_systems(
            Update,
            (listen, press, draw, apply, scroll_controls)
                .chain()
                .after(toggle)
                .in_set(MenuSystems),
        );
}

fn setup(mut commands: Commands) {
    let title = TextFont::from_font_size(22.0);
    let heading = TextFont::from_font_size(15.0);
    let normal = TextFont::from_font_size(14.0);
    let small = TextFont::from_font_size(12.0);
    let white = TextColor(Color::WHITE);

    let row = || Node {
        height: px(ROW_HEIGHT),
        // Rows keep their height in the scrolling list rather than squeeze to fit it.
        flex_shrink: 0.0,
        align_items: AlignItems::Center,
        justify_content: JustifyContent::SpaceBetween,
        column_gap: px(8),
        ..default()
    };
    let button = |control: Control, width: f32| {
        (
            control,
            button(),
            Node {
                width: px(width),
                height: px(ROW_HEIGHT - 3.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border_radius: BorderRadius::all(px(6)),
                ..default()
            },
            BackgroundColor(PLAIN),
        )
    };

    commands
        .spawn((
            Screen,
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.45)),
            // Over everything else in the game, under the frame-time readout.
            GlobalZIndex(1000),
            Visibility::Hidden,
        ))
        .with_children(|screen| {
            screen
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: px(12),
                        padding: UiRect::all(px(24)),
                        border_radius: BorderRadius::all(px(12)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.05, 0.06, 0.08, 0.94)),
                ))
                .with_children(|panel| {
                    panel.spawn((Text::new("Options"), title, white));
                    panel
                        .spawn(Node {
                            column_gap: px(40),
                            ..default()
                        })
                        .with_children(|columns| {
                            // The settings that are not controls.
                            columns
                                .spawn(Node {
                                    width: px(330),
                                    flex_direction: FlexDirection::Column,
                                    ..default()
                                })
                                .with_children(|column| {
                                    column.spawn((Text::new("General"), heading.clone(), white));
                                    for general in General::ALL {
                                        // Sound has a heading of its own, master first.
                                        if general == General::Volume(Part::Master) {
                                            column.spawn(Node {
                                                height: px(10),
                                                ..default()
                                            });
                                            column.spawn((
                                                Text::new("Sound"),
                                                heading.clone(),
                                                white,
                                            ));
                                        }
                                        column.spawn(row()).with_children(|line| {
                                            line.spawn((
                                                Text::new(general.name()),
                                                TextLayout::no_wrap(),
                                                normal.clone(),
                                                white,
                                            ));
                                            if general.ranged() {
                                                line.spawn(Node {
                                                    align_items: AlignItems::Center,
                                                    column_gap: px(6),
                                                    ..default()
                                                })
                                                .with_children(|stepper| {
                                                    stepper
                                                        .spawn(button(
                                                            Control::Change(general, false),
                                                            28.0,
                                                        ))
                                                        .with_child((
                                                            Text::new("-"),
                                                            normal.clone(),
                                                            white,
                                                        ));
                                                    stepper.spawn((
                                                        Shows::Value(general),
                                                        Node {
                                                            width: px(44),
                                                            justify_content: JustifyContent::Center,
                                                            ..default()
                                                        },
                                                        Text::new(""),
                                                        TextLayout::justify(Justify::Center),
                                                        normal.clone(),
                                                        white,
                                                    ));
                                                    stepper
                                                        .spawn(button(
                                                            Control::Change(general, true),
                                                            28.0,
                                                        ))
                                                        .with_child((
                                                            Text::new("+"),
                                                            normal.clone(),
                                                            white,
                                                        ));
                                                });
                                            } else {
                                                line.spawn(button(
                                                    Control::Change(general, true),
                                                    106.0,
                                                ))
                                                .with_child((
                                                    Shows::Value(general),
                                                    Text::new(""),
                                                    normal.clone(),
                                                    white,
                                                ));
                                            }
                                        });
                                    }
                                    column.spawn(Node {
                                        height: px(14),
                                        ..default()
                                    });
                                    column.spawn(button(Control::Reset, 330.0)).with_child((
                                        Text::new("Put everything back as it was"),
                                        normal.clone(),
                                        white,
                                    ));
                                    column.spawn(Node {
                                        height: px(14),
                                        ..default()
                                    });
                                    // Wrapped text needs a width of its own, or it is sized as if
                                    // every word were on a line by itself.
                                    column.spawn((
                                        Node {
                                            width: px(330),
                                            ..default()
                                        },
                                        Text::new(
                                            "Kept as they are: Esc for this menu, 1 to 8 for \
                                             the tool bar, 1 to 0 for the parts of the build \
                                             menu, the mouse wheel to zoom or turn, and F1 to \
                                             F3 for developer tools.",
                                        ),
                                        small.clone(),
                                        TextColor(DIM),
                                    ));
                                });

                            // Every control.
                            columns
                                .spawn(Node {
                                    width: px(470),
                                    flex_direction: FlexDirection::Column,
                                    ..default()
                                })
                                .with_children(|column| {
                                    column.spawn((
                                        Text::new("Controls  (scroll for more)"),
                                        heading.clone(),
                                        white,
                                    ));
                                    // More controls than fit: the list scrolls with the wheel,
                                    // so the menu stays inside the window.
                                    let mut list = column.spawn((
                                        ControlsList,
                                        ScrollPosition::default(),
                                        Node {
                                            flex_direction: FlexDirection::Column,
                                            max_height: px(CONTROLS_SHOWN as f32 * ROW_HEIGHT),
                                            overflow: Overflow::scroll_y(),
                                            ..default()
                                        },
                                    ));
                                    list.with_children(|column| {
                                        for action in Action::ALL {
                                            column.spawn(row()).with_children(|line| {
                                                line.spawn((
                                                    Text::new(action.name()),
                                                    TextLayout::no_wrap(),
                                                    normal.clone(),
                                                    white,
                                                ));
                                                line.spawn(Node {
                                                    column_gap: px(8),
                                                    ..default()
                                                })
                                                .with_children(|pair| {
                                                    for slot in 0..2 {
                                                        pair.spawn(button(
                                                            Control::Bind(action, slot),
                                                            118.0,
                                                        ))
                                                        .with_child((
                                                            Shows::Binding(action, slot),
                                                            Text::new(""),
                                                            normal.clone(),
                                                            white,
                                                        ));
                                                    }
                                                });
                                            });
                                        }
                                    });
                                });
                        });
                    panel
                        .spawn(Node {
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::SpaceBetween,
                            ..default()
                        })
                        .with_children(|bottom| {
                            bottom.spawn((Shows::Note, Text::new(""), normal.clone(), white));
                            bottom.spawn(button(Control::Close, 150.0)).with_child((
                                Text::new("Back to the game"),
                                normal.clone(),
                                white,
                            ));
                        });
                });
        });
}

/// The menu's own clicks and drawing, after the game has read the controls, so the click
/// that closes the menu or sets a control to a mouse button is not also a click in the game.
/// Whatever reads clicks in the game runs before this set.
#[derive(SystemSet, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct MenuSystems;

/// Esc: gives up waiting for a key, or opens or closes this menu. Opening it gives the
/// pointer back; closing it with Esc takes it again. Runs before anything else in the game
/// reads the mouse. A screen above the walker that Esc should close, such as the inventory,
/// closes itself first and takes the press (`ButtonInput::clear_just_pressed`).
pub fn toggle(
    keys: Res<ButtonInput<KeyCode>>,
    mut menu: ResMut<OptionsMenu>,
    window: Single<(&Window, &mut CursorOptions)>,
) {
    if !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    if menu.listening.take().is_some() {
        menu.refused = false;
        return;
    }
    menu.open = !menu.open;
    let (window, mut cursor) = window.into_inner();
    grab_mouse(&mut cursor, !menu.open && window.focused);
}

/// While a control waits, the next key or mouse button pressed becomes it.
fn listen(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    saving: Res<Saving>,
    mut menu: ResMut<OptionsMenu>,
    mut settings: ResMut<Settings>,
) {
    let Some((action, slot)) = menu.listening else {
        return;
    };
    let pressed = keys
        .get_just_pressed()
        .next()
        .map(|&key| Binding::Key(key))
        .or_else(|| {
            mouse
                .get_just_pressed()
                .next()
                .map(|&button| Binding::Mouse(button))
        });
    // Esc is for giving up, which `toggle` sees to.
    let Some(binding) = pressed.filter(|&b| b != Binding::Key(KeyCode::Escape)) else {
        return;
    };
    menu.taken = true;
    if binding.reserved() {
        menu.refused = true;
        return;
    }
    settings.bind(action, slot, Some(binding));
    menu.listening = None;
    menu.refused = false;
    if saving.0 {
        settings.save();
    }
}

/// Clicks on the menu. A right-click on a control clears it.
fn press(
    mouse: Res<ButtonInput<MouseButton>>,
    clicked: Query<&Control, Added<Pressed>>,
    pointed: Query<(&Hovered, &Control)>,
    saving: Res<Saving>,
    mut menu: ResMut<OptionsMenu>,
    mut settings: ResMut<Settings>,
) {
    if std::mem::take(&mut menu.taken) || !menu.open {
        return;
    }
    if mouse.just_pressed(MouseButton::Right) {
        for (hovered, &control) in &pointed {
            if let (true, Control::Bind(action, slot)) = (hovered.get(), control) {
                settings.bind(action, slot, None);
                if saving.0 {
                    settings.save();
                }
            }
        }
    }
    if !left_click(&mouse) {
        return;
    }
    for &control in &clicked {
        match control {
            Control::Change(general, up) => general.change(&mut settings, up),
            Control::Bind(action, slot) => {
                menu.listening = Some((action, slot));
                menu.refused = false;
                continue;
            }
            Control::Reset => *settings = Settings::default(),
            // The pointer stays free: the next click takes control, as at the start, so
            // this click does nothing in the game.
            Control::Close => {
                menu.open = false;
                menu.listening = None;
                continue;
            }
        }
        if saving.0 {
            settings.save();
        }
    }
}

fn draw(
    menu: Res<OptionsMenu>,
    settings: Res<Settings>,
    mut screen: Single<&mut Visibility, With<Screen>>,
    mut buttons: Query<(&Hovered, Has<Pressed>, &Control, &mut BackgroundColor)>,
    mut texts: Query<(&Shows, &mut Text)>,
    rate: Res<ScreenRate>,
) {
    let wanted = if menu.open {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    if **screen != wanted {
        **screen = wanted;
    }
    if !menu.open {
        return;
    }
    for (hovered, pressed, control, mut background) in &mut buttons {
        let waiting = matches!(control, Control::Bind(action, slot) if menu.listening == Some((*action, *slot)));
        let colour = if waiting {
            PICKED
        } else if pointer_on(hovered, pressed) {
            POINTED
        } else {
            PLAIN
        };
        if background.0 != colour {
            background.0 = colour;
        }
    }
    for (shows, mut text) in &mut texts {
        let wanted = match *shows {
            Shows::Value(general) => general.value(&settings, rate.0),
            Shows::Binding(action, slot) if menu.listening == Some((action, slot)) => {
                "Press a key".to_string()
            }
            Shows::Binding(action, slot) => settings.bindings(action)[slot]
                .map_or("-".to_string(), |binding| capitalised(binding.label())),
            Shows::Note => match menu.listening {
                Some(_) if menu.refused => {
                    "That key is kept for something else. Try another.      Esc  give up"
                        .to_string()
                }
                Some((action, _)) => format!(
                    "Press a key or mouse button for {}.      Esc  give up",
                    action.name().to_lowercase()
                ),
                None => "Click a control to change it, right-click to clear it.      Esc  back to the game"
                    .to_string(),
            },
        };
        if text.0 != wanted {
            text.0 = wanted;
        }
    }
}

/// Puts the settings into effect whenever they change, and at the start.
fn apply(
    settings: Res<Settings>,
    options: Res<Options>,
    mut window: Single<&mut Window, With<PrimaryWindow>>,
    mut projection: Single<&mut Projection, With<MainCamera>>,
    (mut asked, forced): (ResMut<Asked>, Res<Forced>),
    mut frame_limit: ResMut<FrameLimit>,
) {
    if !settings.is_changed() {
        return;
    }
    if let Projection::Perspective(perspective) = &mut **projection {
        perspective.fov = settings.field_of_view.to_radians();
    }
    // The preset `--quality` asked for stands; a scripted run otherwise draws at the
    // GPU's own, so its pictures and timings do not depend on the machine's settings.
    if !forced.0 && !options.scripted() && asked.0 != settings.graphics {
        asked.0 = settings.graphics;
    }
    // A scripted run keeps the window it was given, so its pictures and timings compare.
    if options.scripted() {
        return;
    }
    let mode = if settings.full_screen {
        WindowMode::BorderlessFullscreen(MonitorSelection::Current)
    } else {
        WindowMode::Windowed
    };
    if window.mode != mode {
        window.mode = mode;
    }
    let present_mode = if settings.vsync {
        PresentMode::AutoVsync
    } else {
        PresentMode::AutoNoVsync
    };
    if window.present_mode != present_mode {
        window.present_mode = present_mode;
    }
    // A limit applies with vsync off; with it on, the screen paces frames.
    let limit = if settings.vsync {
        FrameLimit::Off
    } else {
        settings.frame_limit
    };
    if *frame_limit != limit {
        *frame_limit = limit;
    }
}

/// The wheel scrolls the list of controls while the menu is open.
fn scroll_controls(
    menu: Res<OptionsMenu>,
    wheel: Res<AccumulatedMouseScroll>,
    list: Single<(&mut ScrollPosition, &ComputedNode), With<ControlsList>>,
) {
    if !menu.open || wheel.delta.y == 0.0 {
        return;
    }
    let step = match wheel.unit {
        MouseScrollUnit::Line => wheel.delta.y * ROW_HEIGHT,
        MouseScrollUnit::Pixel => wheel.delta.y,
    };
    let (mut position, computed) = list.into_inner();
    let most = ((computed.content_size().y - computed.size().y) * computed.inverse_scale_factor())
        .max(0.0);
    position.y = (position.y - step).clamp(0.0, most);
}
