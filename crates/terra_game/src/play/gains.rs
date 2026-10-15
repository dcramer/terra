//! What you just got: a short list at the right of the screen, "+3 Wood", for whatever
//! comes into what you carry, whether cut, dug, butchered, made at a station or picked
//! back up. More of the same while it is still showing adds to its line rather than
//! starting another. Each line fades a few seconds after it last grew.
//!
//! It reads [`Gained`], which `Belongings` writes, so it knows nothing of where the
//! items came from.

use super::IsLocalPlayer;
use super::inventory::{Gained, Items};
use bevy::prelude::*;
use terra_items::ItemKind;

/// How long a line stays after it last grew, in seconds, and how much of that it fades.
const SHOWN_SECONDS: f32 = 3.5;
const FADE_SECONDS: f32 = 0.8;
/// The most lines shown at once; the oldest goes first.
const LINES: usize = 6;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Line {
    kind: ItemKind,
    count: u32,
    /// Seconds since it last grew.
    age: f32,
}

/// The lines showing, newest last.
#[derive(Resource, Default, Debug)]
struct Feed(Vec<Line>);

impl Feed {
    /// Adds so many of a kind: to its line if one is showing, which moves to the bottom
    /// as the newest, or as a new line.
    fn add(&mut self, kind: ItemKind, count: u32) {
        let line = match self.0.iter().position(|line| line.kind == kind) {
            Some(at) => {
                let mut line = self.0.remove(at);
                line.count += count;
                line
            }
            None => Line {
                kind,
                count,
                age: 0.0,
            },
        };
        self.0.push(Line { age: 0.0, ..line });
        if self.0.len() > LINES {
            self.0.remove(0);
        }
    }

    fn age(&mut self, seconds: f32) {
        for line in &mut self.0 {
            line.age += seconds;
        }
        self.0.retain(|line| line.age < SHOWN_SECONDS);
    }
}

/// How visible a line of an age is, from 1 down to 0 as it goes.
fn opacity(age: f32) -> f32 {
    ((SHOWN_SECONDS - age) / FADE_SECONDS).clamp(0.0, 1.0)
}

#[derive(Component)]
struct LineText(usize);

pub fn plugin(app: &mut App) {
    app.add_message::<Gained>()
        .init_resource::<Feed>()
        .add_systems(Startup, setup)
        .add_systems(Update, (gather, draw).chain());
}

fn setup(mut commands: Commands) {
    commands
        .spawn((
            Name::new("Gains"),
            Node {
                position_type: PositionType::Absolute,
                right: px(28),
                bottom: px(150),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexEnd,
                row_gap: px(4),
                ..default()
            },
        ))
        .with_children(|feed| {
            for index in 0..LINES {
                feed.spawn((
                    LineText(index),
                    Text::new(""),
                    TextFont::from_font_size(15.0),
                    TextColor(Color::WHITE),
                    Node {
                        padding: UiRect::axes(px(10), px(4)),
                        border_radius: BorderRadius::all(px(6)),
                        ..default()
                    },
                    BackgroundColor(Color::NONE),
                    Visibility::Hidden,
                ));
            }
        });
}

fn gather(
    time: Res<Time>,
    items: Res<Items>,
    mut gained: MessageReader<Gained>,
    ours: Option<Single<Entity, IsLocalPlayer>>,
    mut feed: ResMut<Feed>,
) {
    feed.age(time.delta_secs());
    // What came to this machine's player, which is what its screen shows.
    let ours = ours.map(|player| *player);
    for &Gained {
        player,
        kind,
        count,
    } in gained.read()
    {
        if Some(player) != ours {
            continue;
        }
        // Logged, so a scripted run can read what came in (D69).
        info!("Gained {count} {}.", items.name(kind));
        feed.add(kind, count);
    }
}

fn draw(
    feed: Res<Feed>,
    items: Res<Items>,
    options: Res<crate::shell::Options>,
    mut lines: Query<(
        &LineText,
        &mut Text,
        &mut TextColor,
        &mut BackgroundColor,
        &mut Visibility,
    )>,
) {
    for (LineText(index), mut text, mut colour, mut backing, mut visibility) in &mut lines {
        let line = feed.0.get(*index).filter(|_| options.shows_interface());
        visibility.set_if_neq(if line.is_some() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
        let Some(line) = line else { continue };
        let shown = format!("+{} {}", line.count, items.name(line.kind));
        if text.0 != shown {
            text.0 = shown;
        }
        let alpha = opacity(line.age);
        colour.set_if_neq(TextColor(Color::srgba(1.0, 0.95, 0.8, alpha)));
        backing.set_if_neq(BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.45 * alpha)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind(n: u16) -> ItemKind {
        ItemKind(n)
    }

    #[test]
    fn more_of_the_same_adds_to_its_line_and_brings_it_down() {
        let mut feed = Feed::default();
        feed.add(kind(1), 2);
        feed.add(kind(2), 1);
        feed.age(1.0);
        feed.add(kind(1), 3);
        let shown: Vec<(ItemKind, u32)> = feed.0.iter().map(|l| (l.kind, l.count)).collect();
        assert_eq!(shown, vec![(kind(2), 1), (kind(1), 5)]);
        assert_eq!(feed.0[1].age, 0.0);
    }

    #[test]
    fn lines_go_after_a_while_and_the_oldest_first_when_full() {
        let mut feed = Feed::default();
        for n in 0..LINES as u16 + 2 {
            feed.add(kind(n), 1);
        }
        assert_eq!(feed.0.len(), LINES);
        assert_eq!(feed.0[0].kind, kind(2));
        feed.age(SHOWN_SECONDS + 0.1);
        assert!(feed.0.is_empty());
        assert_eq!(opacity(0.0), 1.0);
        assert_eq!(opacity(SHOWN_SECONDS), 0.0);
    }
}
