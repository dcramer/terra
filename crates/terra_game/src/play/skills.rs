//! Skills by doing (D88): doing a thing gives you its skill, and doing it more improves
//! it. Each step makes that work quicker. Dying costs some progress toward the next step
//! of each skill, never a step already reached (D101).
//!
//! Other systems say what was done by sending [`Did`]; only this one turns it into
//! progress. The list shows beside the backpack while the inventory is open (D93).

use super::Player;
use super::bag::Notice;
use super::death::WentDown;
use super::inventory::InventoryScreen;
use bevy::prelude::*;

/// Something you can get better at.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Skill {
    Chopping,
    Mining,
    Foraging,
    Building,
    Crafting,
    Fighting,
    /// The bow, a skill of its own (D88).
    Archery,
}

impl Skill {
    pub const ALL: [Skill; 7] = [
        Skill::Chopping,
        Skill::Mining,
        Skill::Foraging,
        Skill::Building,
        Skill::Crafting,
        Skill::Fighting,
        Skill::Archery,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Skill::Chopping => "Chopping",
            Skill::Mining => "Mining",
            Skill::Foraging => "Foraging",
            Skill::Building => "Building",
            Skill::Crafting => "Crafting",
            Skill::Fighting => "Fighting",
            Skill::Archery => "Archery",
        }
    }
}

/// How much doing something once teaches, before it is scaled by how big a thing it was.
pub const ONCE: f32 = 1.0;
/// The highest step a skill reaches.
const TOP: u32 = 20;
/// Each step makes the work this much quicker.
const QUICKER_PER_STEP: f32 = 0.05;
/// What dying costs: this share of the progress toward each skill's next step.
const DEATH_COST: f32 = 0.5;

/// What you know and how well: progress in each skill, where a whole number is a step.
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct Skills {
    progress: [Option<f32>; Skill::ALL.len()],
}

/// What changed when something was done.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Learned {
    Nothing,
    /// Done for the first time: the skill is yours now.
    New,
    /// A new step reached.
    Step(u32),
}

/// How much progress a step takes: more for each step up.
fn cost_of(step: u32) -> f32 {
    6.0 + 4.0 * step as f32
}

impl Skills {
    /// The step reached, if the skill is known at all.
    pub fn step(&self, skill: Skill) -> Option<u32> {
        self.progress[skill as usize].map(|progress| step_of(progress).0)
    }

    /// How far toward the next step, from 0 to 1, if the skill is known.
    pub fn toward_next(&self, skill: Skill) -> Option<f32> {
        self.progress[skill as usize].map(|progress| step_of(progress).1)
    }

    /// How much quicker the work is for the skill: 1 untried, more with each step.
    pub fn speed(&self, skill: Skill) -> f32 {
        1.0 + QUICKER_PER_STEP * self.step(skill).unwrap_or(0) as f32
    }

    /// Does a thing once, `amount` times over.
    pub fn did(&mut self, skill: Skill, amount: f32) -> Learned {
        let slot = &mut self.progress[skill as usize];
        let Some(progress) = slot else {
            *slot = Some(0.0);
            return Learned::New;
        };
        let before = step_of(*progress).0;
        if before >= TOP {
            return Learned::Nothing;
        }
        *progress += amount / cost_of(before);
        let after = step_of(*progress).0.min(TOP);
        if after > before {
            Learned::Step(after)
        } else {
            Learned::Nothing
        }
    }

    /// Every known skill and its progress, for saving.
    pub fn known(&self) -> Vec<(Skill, f32)> {
        Skill::ALL
            .into_iter()
            .filter_map(|skill| Some((skill, self.progress[skill as usize]?)))
            .collect()
    }

    /// Sets a skill's progress, as when a saved world is loaded.
    pub fn restore(&mut self, skill: Skill, progress: f32) {
        self.progress[skill as usize] = Some(progress.max(0.0));
    }

    /// Loses some of the way toward each next step, never a step already reached.
    pub fn lose_some(&mut self) {
        for progress in self.progress.iter_mut().flatten() {
            let (step, toward) = step_of(*progress);
            *progress = step as f32 + toward * (1.0 - DEATH_COST);
        }
    }
}

/// The step a progress reaches, and how far it is toward the next.
fn step_of(progress: f32) -> (u32, f32) {
    let step = progress.floor().max(0.0);
    (step as u32, progress - step)
}

/// Something done that a skill grows from, so much of it.
#[derive(Message, Clone, Copy)]
pub struct Did(pub Skill, pub f32);

#[derive(Component)]
struct Panel;

#[derive(Component)]
struct List;

pub fn plugin(app: &mut App) {
    app.add_message::<Did>()
        .add_systems(Startup, setup)
        .add_systems(Update, (learn, forget, draw));
}

fn learn(
    mut did: MessageReader<Did>,
    mut skills: Single<&mut Skills, With<Player>>,
    mut notice: ResMut<Notice>,
) {
    for &Did(skill, amount) in did.read() {
        match skills.did(skill, amount) {
            Learned::Nothing => {}
            Learned::New => notice.say(format!("You are learning {}", skill.name().to_lowercase())),
            Learned::Step(step) => {
                info!("{} reached step {step}.", skill.name());
                notice.say(format!(
                    "Your {} improves ({step})",
                    skill.name().to_lowercase()
                ));
            }
        }
    }
}

fn forget(mut went_down: MessageReader<WentDown>, mut skills: Single<&mut Skills, With<Player>>) {
    for _ in went_down.read() {
        skills.lose_some();
    }
}

fn setup(mut commands: Commands) {
    commands
        .spawn((
            Panel,
            Node {
                position_type: PositionType::Absolute,
                right: px(24),
                top: percent(30),
                width: px(270),
                flex_direction: FlexDirection::Column,
                row_gap: px(6),
                padding: UiRect::all(px(18)),
                border_radius: BorderRadius::all(px(12)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.06, 0.08, 0.9)),
            Visibility::Hidden,
        ))
        .with_children(|panel| {
            panel.spawn((
                Text::new("Skills"),
                TextFont::from_font_size(20.0),
                TextColor(Color::WHITE),
            ));
            panel.spawn((
                List,
                Text::new(""),
                TextFont::from_font_size(13.0),
                TextColor(Color::WHITE),
            ));
        });
}

fn draw(
    screen: Res<InventoryScreen>,
    skills: Single<&Skills, With<Player>>,
    mut panel: Single<&mut Visibility, With<Panel>>,
    mut list: Single<&mut Text, With<List>>,
) {
    let wanted = if screen.open {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    if **panel != wanted {
        **panel = wanted;
    }
    if !screen.open {
        return;
    }
    let lines: Vec<String> = Skill::ALL
        .into_iter()
        .filter_map(|skill| {
            let step = skills.step(skill)?;
            let toward = skills.toward_next(skill).unwrap_or(0.0);
            let filled = (toward * 10.0).round() as usize;
            Some(format!(
                "{:<10} {:>2}  [{}{}]",
                skill.name(),
                step,
                "=".repeat(filled),
                "-".repeat(10 - filled)
            ))
        })
        .collect();
    let text = if lines.is_empty() {
        "Nothing yet. You learn by doing.".to_string()
    } else {
        lines.join("\n")
    };
    if list.0 != text {
        list.0 = text;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doing_a_thing_gives_its_skill_and_more_improves_it() {
        let mut skills = Skills::default();
        assert_eq!(skills.step(Skill::Chopping), None);
        assert_eq!(skills.did(Skill::Chopping, ONCE), Learned::New);
        assert_eq!(skills.step(Skill::Chopping), Some(0));
        let mut steps = Vec::new();
        for _ in 0..40 {
            if let Learned::Step(step) = skills.did(Skill::Chopping, ONCE) {
                steps.push(step);
            }
        }
        assert_eq!(steps, [1, 2, 3], "each step takes longer");
        assert!(skills.speed(Skill::Chopping) > skills.speed(Skill::Mining));
    }

    #[test]
    fn dying_costs_progress_but_never_a_step() {
        let mut skills = Skills::default();
        skills.did(Skill::Mining, ONCE);
        for _ in 0..8 {
            skills.did(Skill::Mining, ONCE);
        }
        let step = skills.step(Skill::Mining).unwrap();
        let toward = skills.toward_next(Skill::Mining).unwrap();
        assert!(step >= 1 && toward > 0.0);
        skills.lose_some();
        assert_eq!(skills.step(Skill::Mining), Some(step));
        assert!(skills.toward_next(Skill::Mining).unwrap() < toward);
        assert_eq!(
            skills.step(Skill::Fighting),
            None,
            "nothing unknown is learned by dying"
        );
    }

    #[test]
    fn skills_stop_at_the_top() {
        let mut skills = Skills::default();
        for _ in 0..100_000 {
            skills.did(Skill::Crafting, ONCE * 10.0);
        }
        assert_eq!(skills.step(Skill::Crafting), Some(TOP));
    }
}
