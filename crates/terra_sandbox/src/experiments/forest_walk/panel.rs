//! The walk's help panel: the keys, and the date, hour and weather now, in the top right
//! corner; hidden and shown by its key.

use super::{Walk, moment};
use bevy::prelude::*;
use terra_world::climate::SnowCover;
use terra_world::daylight::{Clock, WorldClimate};

#[derive(Component)]
pub(super) struct Panel;

#[derive(Component)]
pub(super) struct PanelText;

/// The keys, as the panel lists them.
pub const KEYS: &str = "F6  summer / winter\n\
F7  dawn / noon / dusk / night\n\
J L  an hour back / on\n\
F8  next weather\n\
P   feedback: picture and note\n\
O   hide this";

pub(super) fn spawn(mut commands: Commands) {
    let panel = commands
        .spawn((
            Panel,
            Node {
                position_type: PositionType::Absolute,
                right: px(12),
                top: px(12),
                padding: UiRect::axes(px(10), px(6)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
            GlobalZIndex(i32::MAX - 2),
        ))
        .id();
    commands.spawn((
        PanelText,
        Text::new(""),
        TextFont::from_font_size(15.0),
        TextColor(Color::srgb(0.92, 0.92, 0.88)),
        ChildOf(panel),
    ));
}

pub(super) fn show(
    walk: Res<Walk>,
    climate: Res<WorldClimate>,
    clock: Res<Clock>,
    snow: Res<SnowCover>,
    mut panel: Single<&mut Node, With<Panel>>,
    mut text: Single<&mut Text, With<PanelText>>,
) {
    let display = if walk.panel {
        Display::Flex
    } else {
        Display::None
    };
    if panel.display != display {
        panel.display = display;
    }
    if !walk.panel {
        return;
    }
    let shown = text_of(&walk, &moment(&climate, &clock, &walk.weather), snow.0);
    if text.0 != shown {
        text.0 = shown;
    }
}

/// What the panel says.
pub fn text_of(walk: &Walk, moment: &str, snow: f32) -> String {
    let notice = walk
        .notice
        .as_ref()
        .map_or(String::new(), |(text, _)| format!("\n\n{text}"));
    format!(
        "Forest walk\n{moment}\nsnow lying {:.0}%\n\n{KEYS}{notice}",
        snow * 100.0
    )
}
