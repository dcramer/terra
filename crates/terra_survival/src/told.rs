//! What is said to a player (`Told`): what happened to them, or why something was not
//! done. Every feature says it the same way, naming the player; this machine shows what is
//! said to its own player as the notice above the bar (`Notice`, drawn by `bag`), and a
//! server will send each player theirs.

use bevy::prelude::*;
use terra_walker::IsLocalPlayer;

pub fn plugin(app: &mut App) {
    app.init_resource::<Notice>()
        .add_message::<Told>()
        .add_systems(PostUpdate, hear);
}

/// A line above the bar saying what happened, or why something was not done.
#[derive(Resource, Default)]
pub struct Notice {
    pub text: String,
    pub age: f32,
}

impl Notice {
    /// Shows a line, and logs it, so a scripted run can read what happened (D69).
    pub fn say(&mut self, text: impl Into<String>) {
        self.text = text.into();
        info!("Notice: {}", self.text);
        self.age = 0.0;
    }
}

/// Something said to a player: what happened to them, or why something was not done.
/// This machine shows what is said to its own player (`Notice`); a server will send each
/// player theirs.
#[derive(Message, Clone, Debug)]
pub struct Told {
    pub player: Entity,
    pub text: String,
}

impl Told {
    pub fn new(player: Entity, text: impl Into<String>) -> Self {
        Self {
            player,
            text: text.into(),
        }
    }
}

/// Shows what is said to this machine's player.
fn hear(
    mut told: MessageReader<Told>,
    ours: Option<Single<Entity, IsLocalPlayer>>,
    mut notice: ResMut<Notice>,
) {
    let ours = ours.map(|player| *player);
    for said in told.read() {
        if Some(said.player) == ours {
            notice.say(said.text.clone());
        }
    }
}
