//! The spar on screen (D405): the two fighters' health as bars at the top, the controls at
//! the bottom left (for a while from the start, and whenever the sword is drawn), and a
//! white flash over everything when a blow is parried. The controls are logged at the start
//! too, so a scripted run says them.

use super::contact::Glare;
use super::partner::Partner;
use super::{Drawn, Fighter, HEALTH};
use bevy::{picking::Pickable, prelude::*};
use terra_walker::{IsLocalPlayer, Player};

/// What the controls say.
const CONTROLS: &str = "T  draw or sheathe sword and shield\n\
Q  lock on to the sparring partner\n\
WASD  strafe, facing the view\n\
Space  jump        Ctrl  roll (untouchable mid-roll)\n\
Left click  light chain (press again in its swing)\n\
Shift + left click  heavy blow (breaks a guard)\n\
Hold right click  block; raise it just before a blow to parry";
/// How long the controls show from the start with the sword sheathed, seconds.
const HINT_S: f32 = 20.0;
/// How fast the parry's white flash fades, a share a second.
const GLARE_FADE: f32 = 7.0;

#[derive(Component)]
struct Bars;

/// A health bar's fill: the player's, or the partner's.
#[derive(Component)]
struct Fill(bool);

/// A bar's column, the player's or the foe's, and the foe's name over its bar.
#[derive(Component)]
struct Column(bool);

#[derive(Component)]
struct FoeName;

#[derive(Component)]
struct ControlsPanel;

#[derive(Component)]
struct Whiteout;

/// The creatures a drawn player may be fighting.
#[derive(bevy::ecs::system::SystemParam)]
struct Foes<'w, 's> {
    bestiary: Res<'w, crate::species::Bestiary>,
    creatures:
        Query<'w, 's, (&'static Transform, &'static crate::creatures::Creature), Without<Player>>,
}

impl Foes<'_, '_> {
    /// The nearest living hostile creature within a fight's reach of a place: its name and
    /// the share of its health it has left.
    fn at_hand(&self, feet: Vec3) -> Option<(String, f32)> {
        self.creatures
            .iter()
            .filter(|(at, creature)| {
                creature.health > 0.0
                    && self.bestiary.hostile(creature.species)
                    && at.translation.distance(feet) < super::FOE_AT_HAND_M
            })
            .min_by(|a, b| {
                let away = |at: &Transform| at.translation.distance_squared(feet);
                away(a.0).total_cmp(&away(b.0))
            })
            .map(|(_, creature)| {
                let about = self.bestiary.get(creature.species);
                (about.name.clone(), creature.health / about.health.max(1.0))
            })
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Glare>()
        .add_systems(Startup, spawn)
        .add_systems(Update, show);
}

fn bar(parent: &mut ChildSpawnerCommands, label: &str, player: bool) {
    parent
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                margin: UiRect::horizontal(px(18)),
                ..default()
            },
            Column(player),
        ))
        .with_children(|column| {
            let mut name = column.spawn((
                Text::new(label),
                TextFont::from_font_size(14.0),
                TextColor(Color::WHITE),
                TextShadow::default(),
            ));
            if !player {
                name.insert(FoeName);
            }
            column
                .spawn((
                    Node {
                        width: px(220),
                        height: px(10),
                        margin: UiRect::top(px(4)),
                        border: UiRect::all(px(1)),
                        ..default()
                    },
                    BorderColor::all(Color::srgba(0.0, 0.0, 0.0, 0.8)),
                    BackgroundColor(Color::srgba(0.1, 0.05, 0.05, 0.7)),
                ))
                .with_children(|frame| {
                    frame.spawn((
                        Node {
                            width: percent(100),
                            height: percent(100),
                            ..default()
                        },
                        BackgroundColor(if player {
                            Color::srgb(0.72, 0.16, 0.12)
                        } else {
                            Color::srgb(0.78, 0.62, 0.22)
                        }),
                        Fill(player),
                    ));
                });
        });
}

fn spawn(mut commands: Commands) {
    info!("Melee: controls: {}", CONTROLS.replace('\n', "; "));
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: px(16),
                width: percent(100),
                justify_content: JustifyContent::Center,
                ..default()
            },
            Visibility::Hidden,
            Pickable::IGNORE,
            Bars,
        ))
        .with_children(|row| {
            bar(row, "You", true);
            bar(row, "Sparring partner", false);
        });
    commands.spawn((
        Text::new(CONTROLS),
        TextFont::from_font_size(13.0),
        TextColor(Color::srgba(1.0, 1.0, 1.0, 0.85)),
        TextShadow::default(),
        Node {
            position_type: PositionType::Absolute,
            left: px(16),
            bottom: px(110),
            ..default()
        },
        Pickable::IGNORE,
        ControlsPanel,
    ));
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            ..default()
        },
        BackgroundColor(Color::NONE),
        GlobalZIndex(50),
        Pickable::IGNORE,
        Whiteout,
    ));
}

#[allow(clippy::type_complexity)]
fn show(
    time: Res<Time>,
    mut glare: ResMut<Glare>,
    ours: Option<Single<(Option<&Fighter>, Has<Drawn>, &Transform), (With<Player>, IsLocalPlayer)>>,
    partners: Query<&Fighter, (With<Partner>, Without<Player>)>,
    foes: Foes,
    (mut columns, mut names): (
        Query<(&Column, &mut Node), Without<Fill>>,
        Query<&mut Text, With<FoeName>>,
    ),
    mut bars: Query<&mut Visibility, (With<Bars>, Without<ControlsPanel>)>,
    mut panel: Query<&mut Visibility, (With<ControlsPanel>, Without<Bars>)>,
    mut fills: Query<(&Fill, &mut Node)>,
    mut whiteout: Query<&mut BackgroundColor, With<Whiteout>>,
    filming: Option<Res<terra_world::cine::Cine>>,
    options: Option<Res<terra_world::shell::Options>>,
) {
    let (fighter, drawn, feet) = ours.map_or((None, false, None), |ours| {
        let (fighter, drawn, at) = ours.into_inner();
        (
            fighter.map(|fighter| fighter.health),
            drawn,
            Some(at.translation),
        )
    });
    let shown = |on: bool| {
        if on {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        }
    };
    // The bars and controls are for someone playing, never in a film's picture or a fixed
    // camera's shot, which show the scene alone (`shows_interface`).
    let playing = filming.is_none() && options.is_none_or(|options| options.shows_interface());
    for mut visible in &mut bars {
        visible.set_if_neq(shown(playing && drawn));
    }
    let hint = playing && (drawn || time.elapsed_secs() < HINT_S);
    for mut visible in &mut panel {
        visible.set_if_neq(shown(hint));
    }
    // Who the other bar is: the sparring partner, or with none the foe at hand, by its name
    // and its own health; the player's bar here is a spar's, and in a fight with a creature
    // their health is the game's own bar.
    let sparring = partners
        .iter()
        .next()
        .map(|fighter| fighter.health / HEALTH);
    let foe = match sparring {
        Some(share) => Some(("Sparring partner".to_string(), share)),
        None => feet.and_then(|feet| foes.at_hand(feet)),
    };
    for (column, mut node) in &mut columns {
        let on = if column.0 {
            sparring.is_some()
        } else {
            foe.is_some()
        };
        let display = if on { Display::Flex } else { Display::None };
        if node.display != display {
            node.display = display;
        }
    }
    if let Some((name, _)) = &foe {
        for mut text in &mut names {
            if text.0 != *name {
                text.0.clone_from(name);
            }
        }
    }
    for (fill, mut node) in &mut fills {
        let share = if fill.0 {
            fighter.map(|health| health / HEALTH)
        } else {
            foe.as_ref().map(|(_, share)| *share)
        };
        node.width = percent(100.0 * share.unwrap_or(1.0).clamp(0.0, 1.0));
    }
    glare.0 = (glare.0 - time.delta_secs() * GLARE_FADE).max(0.0);
    for mut color in &mut whiteout {
        color.0 = Color::srgba(1.0, 0.97, 0.9, 0.55 * glare.0);
    }
}
