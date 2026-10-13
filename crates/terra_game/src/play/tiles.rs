//! The look and keys the build menu and the station screens share (D67, D109): a panel of
//! tabs and a grid of tiles, each a name with a line beneath, moved over with the arrows.
//! Each screen owns what its tiles are; this is only how they look and move.

use super::button::button;
use bevy::{ecs::hierarchy::ChildSpawnerCommands, prelude::*};

pub const TILE: Vec2 = Vec2::new(170.0, 66.0);
pub const GAP: f32 = 8.0;
pub const PLAIN: Color = Color::srgba(0.12, 0.13, 0.16, 0.95);
pub const CURSOR: Color = Color::srgba(0.35, 0.75, 0.85, 0.95);
pub const PICKED: Color = Color::srgba(0.9, 0.46, 0.17, 0.95);
pub const BACKDROP: Color = Color::srgba(0.05, 0.06, 0.08, 0.92);
pub const FAINT: TextColor = TextColor(Color::srgba(1.0, 1.0, 1.0, 0.7));

/// The panel the tabs and tiles sit in.
pub fn panel() -> (Node, BackgroundColor) {
    (
        Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(10),
            padding: UiRect::all(px(20)),
            border_radius: BorderRadius::all(px(12)),
            ..default()
        },
        BackgroundColor(BACKDROP),
    )
}

/// A grid for tiles, so many across.
pub fn grid(columns: usize, rows: usize) -> Node {
    Node {
        display: Display::Grid,
        grid_template_columns: RepeatedGridTrack::px(columns as u16, TILE.x),
        column_gap: px(GAP),
        row_gap: px(GAP),
        min_height: px(TILE.y * rows as f32 + GAP * (rows as f32 - 1.0)),
        ..default()
    }
}

/// A tab along the top, numbered from one.
pub fn tab(parent: &mut ChildSpawnerCommands, marker: impl Bundle, index: usize, name: &str) {
    parent
        .spawn((
            marker,
            button(),
            Node {
                padding: UiRect::axes(px(12), px(6)),
                border_radius: BorderRadius::all(px(6)),
                ..default()
            },
            BackgroundColor(PLAIN),
        ))
        .with_child((
            Text::new(format!("{}  {name}", index + 1)),
            TextFont::from_font_size(13.0),
            TextColor(Color::WHITE),
        ));
}

/// A tile: its name, and a line of text beneath it such as what it costs.
pub fn tile(parent: &mut ChildSpawnerCommands, marker: impl Bundle, name: &str, line: impl Bundle) {
    parent
        .spawn((
            marker,
            button(),
            Node {
                width: px(TILE.x),
                height: px(TILE.y),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::all(px(8)),
                border: UiRect::all(px(2)),
                border_radius: BorderRadius::all(px(8)),
                ..default()
            },
            BackgroundColor(PLAIN),
            BorderColor::all(Color::NONE),
        ))
        .with_children(|tile| {
            tile.spawn((
                Text::new(name),
                TextFont::from_font_size(15.0),
                TextColor(Color::WHITE),
            ));
            tile.spawn((line, TextFont::from_font_size(11.0), FAINT));
        });
}

/// Where the arrows move a cursor in a grid so many across, kept within `count`.
pub fn step(keys: &ButtonInput<KeyCode>, cursor: usize, count: usize, columns: usize) -> usize {
    let by = |key, by: isize| if keys.just_pressed(key) { by } else { 0 };
    let moved = by(KeyCode::ArrowRight, 1)
        + by(KeyCode::ArrowLeft, -1)
        + by(KeyCode::ArrowDown, columns as isize)
        + by(KeyCode::ArrowUp, -(columns as isize));
    if count == 0 {
        return 0;
    }
    (cursor as isize + moved).clamp(0, count as isize - 1) as usize
}

/// Outlines a tile when it is pointed at or the keys are on it.
pub fn outline(border: &mut BorderColor, on: bool) {
    let wanted = BorderColor::all(if on { CURSOR } else { Color::NONE });
    if *border != wanted {
        *border = wanted;
    }
}

/// Sets a background only when it changes, so the UI is not redrawn for nothing.
pub fn fill(background: &mut BackgroundColor, colour: Color) {
    if background.0 != colour {
        background.0 = colour;
    }
}

/// Shows or hides a screen, only when that changes.
pub fn show(visibility: &mut Visibility, shown: bool) {
    let wanted = if shown {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    if *visibility != wanted {
        *visibility = wanted;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_arrows_move_within_the_grid() {
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::ArrowDown);
        assert_eq!(step(&keys, 1, 10, 4), 5);
        assert_eq!(step(&keys, 8, 10, 4), 9, "stops at the last tile");
        keys.reset_all();
        keys.press(KeyCode::ArrowLeft);
        assert_eq!(step(&keys, 0, 10, 4), 0, "stops at the first");
        assert_eq!(
            step(&keys, 0, 0, 4),
            0,
            "an empty grid has its cursor at nought"
        );
    }
}
