//! Who this machine's player is in a hosted world (D592): the name the others see, the
//! identity a server knows their character by, and the address last joined, kept beside the
//! worlds (`player.ron`) so they are typed once. The identity is a secret made once from
//! the operating system's randomness and never sent: a server is shown only that its holder
//! is here (`terra_net::identity`). A run keeping no worlds keeps no profile either: it has
//! a new identity each time, as a script's player has no character to come back to.

use super::menu::PLAYER_BYTES;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use terra_net::identity::Identity;

const FILE: &str = "player.ron";

#[derive(Resource, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Profile {
    pub name: String,
    secret: [u8; 32],
    #[serde(default)]
    pub address: String,
    /// Where it is kept, if anywhere.
    #[serde(skip)]
    path: Option<PathBuf>,
}

impl Profile {
    /// The profile kept in the worlds' folder, made and kept now if there is none or it
    /// cannot be read; with no folder, one for this run alone.
    pub fn load(root: Option<&Path>) -> Profile {
        let path = root.map(|root| root.join(FILE));
        let kept = path
            .as_ref()
            .and_then(|path| std::fs::read_to_string(path).ok())
            .and_then(|text| ron::from_str::<Profile>(&text).ok());
        if let Some(mut profile) = kept {
            profile.path = path;
            profile.name.truncate(
                (0..=PLAYER_BYTES.min(profile.name.len()))
                    .rev()
                    .find(|&at| profile.name.is_char_boundary(at))
                    .unwrap_or(0),
            );
            return profile;
        }
        let mut secret = [0; 32];
        // With no randomness to be had the game cannot make an identity nobody else has.
        getrandom::fill(&mut secret).expect("the operating system gives random bytes");
        let profile = Profile {
            name: "Player".into(),
            secret,
            address: String::new(),
            path,
        };
        profile.save();
        profile
    }

    pub fn save(&self) {
        let Some(path) = &self.path else { return };
        let written = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
            .map_err(|problem| problem.to_string())
            .and_then(|text| std::fs::write(path, text).map_err(|problem| problem.to_string()));
        if let Err(problem) = written {
            warn!(
                "Could not keep the player's profile at {}: {problem}. Without it this player is a stranger to every server next time.",
                path.display()
            );
        }
    }

    pub fn identity(&self) -> Identity {
        Identity::from_secret(self.secret)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_profile_is_made_once_and_is_the_same_player_when_read_again() {
        let dir = std::env::temp_dir().join(format!("terra-profile-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut first = Profile::load(Some(&dir));
        assert_eq!(first.name, "Player");
        first.name = "Ada".into();
        first.address = "203.0.113.7".into();
        first.save();
        let again = Profile::load(Some(&dir));
        assert_eq!(again, first);
        assert_eq!(again.identity().public(), first.identity().public());
        // Another machine's is another player; one kept nowhere is new each time.
        let other = Profile::load(None);
        assert_ne!(other.identity().public(), first.identity().public());
        assert_ne!(
            Profile::load(None).identity().public(),
            other.identity().public()
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
