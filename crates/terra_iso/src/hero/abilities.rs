//! What the hero can do, and how he is asked: each `Ability` with its binding, shown in the
//! play help, read each frame from the keys and mouse or from the script into his
//! `HeroIntent`. A scripted film asks by `--do NAME[:X,Z]@FROM[-TO]` (main.rs's Args: a press
//! at frame FROM, or held over FROM to TO, aimed at the ground point X,Z), beside `--press`
//! (WASD over frames) and `--click` (a light blow at a pixel); another system writes the
//! intent itself, in `HeroSystems::Intent` after `read`.

use super::Hero;
use crate::Args;
use crate::camera::MainCamera;
use bevy::prelude::*;

/// Something the hero does, by its binding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ability {
    /// The light chain: slash, backhand, finisher, each pressed in the last one's window.
    Light,
    /// The heavy blow, overhead.
    Heavy,
    /// The shield raised while held (a parry at its start).
    Block,
    /// The roll, along the way he is asked to go, else ahead; blows pass through its tuck.
    Roll,
    /// Moves at a walk while held, not a jog.
    Walk,
    /// Moves at a sprint while held.
    Sprint,
}

impl Ability {
    pub const ALL: [Ability; 6] = [
        Ability::Light,
        Ability::Heavy,
        Ability::Block,
        Ability::Roll,
        Ability::Walk,
        Ability::Sprint,
    ];

    /// Its name in `--do`.
    pub fn name(self) -> &'static str {
        match self {
            Ability::Light => "light",
            Ability::Heavy => "heavy",
            Ability::Block => "block",
            Ability::Roll => "roll",
            Ability::Walk => "walk",
            Ability::Sprint => "sprint",
        }
    }

    pub fn named(name: &str) -> Option<Ability> {
        Ability::ALL.into_iter().find(|a| a.name() == name)
    }

    /// Its binding, as the play help names it.
    pub fn keys(self) -> &'static str {
        match self {
            Ability::Light => "LMB",
            Ability::Heavy => "Q / Shift+LMB",
            Ability::Block => "RMB held",
            Ability::Roll => "Space",
            Ability::Walk => "Ctrl held",
            Ability::Sprint => "Shift held",
        }
    }

    /// Whether it lasts as long as it is held (else it is a press).
    pub fn held(self) -> bool {
        matches!(self, Ability::Block | Ability::Walk | Ability::Sprint)
    }
}

/// What the hero is asked to do this frame: the seam his input, a script and any other system
/// write and his moves read (moves.rs).
#[derive(Component, Default, Clone, Debug, PartialEq)]
pub struct HeroIntent {
    /// The ground way (x, z) he is asked to go, of length 1, or zero to stand.
    pub way: Vec2,
    pub walk: bool,
    pub sprint: bool,
    pub block: bool,
    /// The ground point (x, z) the shield is raised toward, if any (else the nearest foe).
    pub block_at: Option<Vec2>,
    /// A move pressed this frame, and the ground point (x, z) it is aimed at, if any (else he
    /// keeps his facing; a roll goes along `way`).
    pub press: Option<(Ability, Option<Vec2>)>,
}

impl HeroIntent {
    /// Asks for `ability` this frame, aimed at `at`: a press, or held.
    pub fn ask(&mut self, ability: Ability, at: Option<Vec2>) {
        match ability {
            Ability::Block => {
                self.block = true;
                self.block_at = at.or(self.block_at);
            }
            Ability::Walk => self.walk = true,
            Ability::Sprint => self.sprint = true,
            _ => self.press = Some((ability, at)),
        }
    }
}

/// The hero's systems in order: his intent read and written, then acted on.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HeroSystems {
    Intent,
    Act,
}

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, help)
        .add_systems(Update, read.in_set(HeroSystems::Intent));
}

/// The ground point (x, z) under a window pixel, on the plane at `y`.
fn ground_at(
    camera: &Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    px: Vec2,
    y: f32,
) -> Option<Vec2> {
    let (cam, tr) = camera.single().ok()?;
    let ray = cam.viewport_to_world(tr, px).ok()?;
    let d = ray.intersect_plane(Vec3::Y * y, InfinitePlane3d::new(Vec3::Y))?;
    Some(ray.get_point(d).xz())
}

/// The keys, mouse and script into his intent, afresh each frame.
fn read(
    args: Res<Args>,
    frame: Res<bevy::diagnostic::FrameCount>,
    (keys, mouse, windows): (
        Res<ButtonInput<KeyCode>>,
        Res<ButtonInput<MouseButton>>,
        Query<&Window>,
    ),
    camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut hero: Query<(&Transform, &mut HeroIntent), With<Hero>>,
) {
    let Ok((tr, mut intent)) = hero.single_mut() else {
        return;
    };
    let key = |k: KeyCode| keys.pressed(k);
    let input = Vec2::new(
        (key(KeyCode::KeyD) as i32 - key(KeyCode::KeyA) as i32) as f32,
        (key(KeyCode::KeyW) as i32 - key(KeyCode::KeyS) as i32) as f32,
    );
    let shift = key(KeyCode::ShiftLeft) || key(KeyCode::ShiftRight);
    *intent = HeroIntent {
        way: crate::play::screen_move(input, crate::ruin::camera::YAW),
        walk: [
            KeyCode::ControlLeft,
            KeyCode::ControlRight,
            KeyCode::AltLeft,
        ]
        .into_iter()
        .any(key),
        sprint: shift,
        block: mouse.pressed(MouseButton::Right),
        block_at: None,
        press: None,
    };
    let cursor = windows.single().ok().and_then(|w| w.cursor_position());
    let aim = cursor.and_then(|px| ground_at(&camera, px, tr.translation.y));
    if mouse.just_pressed(MouseButton::Left) {
        let blow = if shift {
            Ability::Heavy
        } else {
            Ability::Light
        };
        intent.ask(blow, aim);
    } else if keys.just_pressed(KeyCode::KeyQ) {
        intent.ask(Ability::Heavy, aim);
    } else if keys.just_pressed(KeyCode::Space) {
        intent.ask(Ability::Roll, None);
    }
    // The script: a click is a light blow at that pixel's ground point (as play's own).
    if let Some((px, _)) = args.clicks.iter().find(|c| c.1 == frame.0) {
        let at = ground_at(&camera, *px, tr.translation.y);
        intent.ask(Ability::Light, at);
    }
    for (name, at, from, to) in &args.abilities {
        let Some(ability) = Ability::named(name) else {
            continue;
        };
        let now = if ability.held() {
            (*from..*to).contains(&frame.0)
        } else {
            frame.0 == *from
        };
        if now {
            intent.ask(ability, *at);
        }
    }
}

/// The play help, in a window only: each ability's binding.
fn help(mut commands: Commands, args: Res<Args>) {
    if args.shot.is_some() {
        return;
    }
    let mut text = String::from("WASD move");
    for a in Ability::ALL {
        text.push_str(&format!("  |  {} {}", a.keys(), a.name()));
    }
    commands.spawn((
        Text::new(text),
        TextFont::from_font_size(13.0),
        TextColor(Color::srgb(0.85, 0.82, 0.75)),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(12.0),
            bottom: Val::Px(10.0),
            ..default()
        },
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every ability goes by its own name and binding, and asking for one shows in the intent
    /// where the moves read it: a held one as held, a press as this frame's press.
    #[test]
    fn each_ability_is_named_bound_and_asked() {
        for a in Ability::ALL {
            assert_eq!(Ability::named(a.name()), Some(a));
            assert!(!a.keys().is_empty());
            let mut intent = HeroIntent::default();
            intent.ask(a, Some(Vec2::X));
            let held = intent.block || intent.walk || intent.sprint;
            assert_eq!(held, a.held(), "{a:?}");
            assert_eq!(intent.press.is_some(), !a.held(), "{a:?}");
        }
        assert_eq!(Ability::named("fireball"), None);
    }
}
