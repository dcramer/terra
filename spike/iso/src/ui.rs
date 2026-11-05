//! The screen's furniture, after Baldur's Gate II: the party's portraits down the right,
//! drawn live from their faces; a log of what was said at the bottom; the selection box;
//! and a red frame round the screen while the game is paused.

use crate::camera::{MainCamera, Offscreen};
use crate::command::Drag;
use crate::party::{Member, Selected};
use bevy::prelude::*;
use bevy::ui::UiTargetCamera;

pub const PORTRAIT_COLUMN: f32 = 118.0;

pub fn plugin(app: &mut App) {
    app.init_resource::<Log>()
        .add_systems(PostStartup, spawn)
        .add_systems(Update, (portraits, log, frame, drag_box, click_portrait));
}

#[derive(Resource, Default)]
pub struct Log {
    lines: Vec<(String, Color)>,
    dirty: bool,
}

impl Log {
    pub fn say(&mut self, who: &str, what: &str) {
        self.lines
            .push((format!("{who}: {what}"), Color::srgb(0.95, 0.85, 0.55)));
        self.dirty = true;
    }
    pub fn note(&mut self, what: String) {
        self.lines.push((what, Color::srgb(0.75, 0.75, 0.8)));
        self.dirty = true;
    }
    pub fn count(&self) -> usize {
        self.lines.len()
    }
}

#[derive(Component)]
struct Portrait(usize);
#[derive(Component)]
struct LogText;
#[derive(Component)]
struct PauseFrame;
#[derive(Component)]
struct DragBox;

const PANEL: Color = Color::srgba(0.07, 0.06, 0.05, 0.92);
const BRASS: Color = Color::srgb(0.55, 0.43, 0.24);

fn font(size: f32) -> TextFont {
    TextFont {
        font_size: FontSize::Px(size),
        ..default()
    }
}

fn spawn(
    mut commands: Commands,
    members: Query<&Member>,
    camera: Query<Entity, With<MainCamera>>,
    offscreen: Option<Res<Offscreen>>,
    mut log: ResMut<Log>,
) {
    let target = |e: &mut EntityCommands| {
        if offscreen.is_some()
            && let Ok(cam) = camera.single()
        {
            e.insert(UiTargetCamera(cam));
        }
    };
    let mut members: Vec<&Member> = members.iter().collect();
    members.sort_by_key(|m| m.slot);
    let mut column = commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            right: px(0),
            top: px(0),
            bottom: px(0),
            width: px(PORTRAIT_COLUMN),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            padding: UiRect::all(px(8)),
            row_gap: px(8),
            border: UiRect::left(px(2)),
            ..default()
        },
        BackgroundColor(PANEL),
        BorderColor::all(BRASS),
    ));
    target(&mut column);
    column.with_children(|col| {
        for m in &members {
            col.spawn((
                Node {
                    width: px(100),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    border: UiRect::all(px(2)),
                    ..default()
                },
                BorderColor::all(BRASS),
                Portrait(m.slot),
            ))
            .with_children(|p| {
                p.spawn((
                    ImageNode::new(m.portrait.clone()),
                    Node {
                        width: px(96),
                        height: px(120),
                        ..default()
                    },
                ));
                // Hit points: everyone is well, today.
                p.spawn((
                    Node {
                        width: px(96),
                        height: px(5),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.15, 0.6, 0.2)),
                ));
                p.spawn((
                    Text::new(m.name),
                    font(14.0),
                    TextColor(Color::srgb(0.92, 0.86, 0.7)),
                ));
                p.spawn((
                    Text::new(m.class),
                    font(11.0),
                    TextColor(Color::srgb(0.6, 0.58, 0.52)),
                ));
            });
        }
    });

    let mut bottom = commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            right: px(PORTRAIT_COLUMN),
            bottom: px(0),
            height: px(112),
            padding: UiRect::axes(px(14), px(8)),
            border: UiRect::top(px(2)),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::FlexEnd,
            ..default()
        },
        BackgroundColor(PANEL),
        BorderColor::all(BRASS),
    ));
    target(&mut bottom);
    bottom.with_children(|b| {
        b.spawn((Text::new(""), font(14.0), LogText, Node { flex_grow: 1.0, ..default() }));
        b.spawn((
            Text::new("LMB choose / send  |  drag box  |  RMB let go  |  1-4, = choose  |  Space pause  |  T hour  |  WASD / edge / MMB pan  |  wheel zoom"),
            font(11.0),
            TextColor(Color::srgb(0.5, 0.48, 0.44)),
        ));
    });

    let mut pause = commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            right: px(PORTRAIT_COLUMN),
            top: px(0),
            bottom: px(112),
            border: UiRect::all(px(4)),
            justify_content: JustifyContent::Center,
            ..default()
        },
        BorderColor::all(Color::srgb(0.75, 0.12, 0.1)),
        Visibility::Hidden,
        PauseFrame,
    ));
    target(&mut pause);
    pause.with_children(|p| {
        p.spawn((
            Text::new("Paused"),
            font(22.0),
            TextColor(Color::srgb(0.95, 0.3, 0.25)),
            Node {
                margin: UiRect::top(px(10)),
                ..default()
            },
        ));
    });

    let mut drag = commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            border: UiRect::all(px(1)),
            ..default()
        },
        BorderColor::all(Color::srgb(0.3, 1.0, 0.35)),
        Visibility::Hidden,
        DragBox,
    ));
    target(&mut drag);
    log.note("You arrive in the village of Ashford as the day wanes.".into());
}

fn portraits(
    members: Query<(&Member, Has<Selected>)>,
    mut frames: Query<(&Portrait, &mut BorderColor)>,
) {
    for (p, mut border) in &mut frames {
        let sel = members.iter().any(|(m, s)| m.slot == p.0 && s);
        border.set_all(if sel {
            Color::srgb(0.3, 1.0, 0.35)
        } else {
            BRASS
        });
    }
}

fn log(mut log: ResMut<Log>, text: Query<Entity, With<LogText>>, mut commands: Commands) {
    if !log.dirty {
        return;
    }
    log.dirty = false;
    let Ok(e) = text.single() else {
        return;
    };
    let start = log.lines.len().saturating_sub(4);
    commands.entity(e).despawn_children().insert(Text::new(""));
    commands.entity(e).with_children(|t| {
        for (line, color) in &log.lines[start..] {
            t.spawn((
                TextSpan::new(format!("{line}\n")),
                font(14.0),
                TextColor(*color),
            ));
        }
    });
}

fn frame(time: Res<Time<Virtual>>, mut frame: Query<&mut Visibility, With<PauseFrame>>) {
    for mut v in &mut frame {
        *v = if time.is_paused() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

fn drag_box(drag: Res<Drag>, mut boxes: Query<(&mut Node, &mut Visibility), With<DragBox>>) {
    for (mut node, mut v) in &mut boxes {
        match drag.start {
            Some(s) if s.distance(drag.now) > 6.0 => {
                let (lo, hi) = (s.min(drag.now), s.max(drag.now));
                node.left = px(lo.x);
                node.top = px(lo.y);
                node.width = px(hi.x - lo.x);
                node.height = px(hi.y - lo.y);
                *v = Visibility::Inherited;
            }
            _ => *v = Visibility::Hidden,
        }
    }
}

/// A click on a portrait chooses that member, as in the game.
fn click_portrait(
    buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    frames: Query<(&Portrait, &ComputedNode, &UiGlobalTransform)>,
    members: Query<(Entity, &Member)>,
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    mut log: ResMut<Log>,
) {
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    let Some(c) = windows
        .single()
        .ok()
        .and_then(|w| w.cursor_position().map(|c| c * w.scale_factor()))
    else {
        return;
    };
    for (p, node, tr) in &frames {
        if !node.contains_point(*tr, c) {
            continue;
        }
        for (e, m) in &members {
            if m.slot == p.0 {
                commands.entity(e).insert(Selected);
                log.say(m.name, m.barks[1]);
            } else if !keys.pressed(KeyCode::ShiftLeft) {
                commands.entity(e).remove::<Selected>();
            }
        }
    }
}
