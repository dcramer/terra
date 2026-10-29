//! What the play experiment plays, as chosen on its command line (D579): the title screen,
//! or a world without it. The game reads these (`terra_game::play::worlds`); the shell only
//! lets them through, as it does the frame recorder's. The rest of the choice is the shell's
//! own options: `--fresh` (a new world, or with `--save` that file started over), `--save`,
//! `--free-build` and `--level yard`.
//!
//!   --title           open on the title screen, whatever else is given
//!   --creative        the Creative world played last, or a new one
//!   --world=<name>    the world of that name, made if there is none
//!   --worlds=<dir>    keep the worlds there; a scripted run keeps none without it

use bevy::prelude::*;
use std::path::PathBuf;

const TITLE: &str = "--title";
const CREATIVE: &str = "--creative";
const WORLD: &str = "--world=";
const WORLDS: &str = "--worlds=";

/// The play experiment's choice from its command line.
#[derive(Resource, Clone, Default, Debug, PartialEq)]
pub struct Choice {
    pub title: bool,
    pub creative: bool,
    pub world: Option<String>,
    pub worlds: Option<PathBuf>,
}

/// Whether a flag is one of these, for the shell to let through.
pub fn takes(flag: &str) -> bool {
    flag == TITLE || flag == CREATIVE || flag.starts_with(WORLD) || flag.starts_with(WORLDS)
}

impl Choice {
    /// The choice among a command line's arguments; the rest are left to the shell.
    pub fn from_args(args: impl Iterator<Item = String>) -> Self {
        let mut choice = Self::default();
        for arg in args {
            if arg == TITLE {
                choice.title = true;
            } else if arg == CREATIVE {
                choice.creative = true;
            } else if let Some(name) = arg.strip_prefix(WORLD) {
                choice.world = Some(name.to_string());
            } else if let Some(dir) = arg.strip_prefix(WORLDS) {
                choice.worlds = Some(PathBuf::from(dir));
            }
        }
        choice
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_choice_is_read_from_among_the_other_arguments_and_only_its_own_are_taken() {
        let args = [
            "--experiment",
            "play",
            "--title",
            "--world=Hill fort",
            "--worlds=/tmp/w",
        ];
        let choice = Choice::from_args(args.iter().map(|s| s.to_string()));
        assert!(choice.title && !choice.creative);
        assert_eq!(choice.world.as_deref(), Some("Hill fort"));
        assert_eq!(choice.worlds, Some(PathBuf::from("/tmp/w")));
        assert!(args[2..].iter().all(|arg| takes(arg)));
        assert!(!takes("--experiment") && !takes("--worldly") && !takes("--fresh"));
    }
}
