//! What a settlement's days are made of, as data (`assets/data/life.ron`, D324): the things
//! of a town and the places round each where something is done (smart objects, as The Sims'
//! objects advertise what they are for and The Witcher 3's people pool spots), the
//! activities and the clip each plays, and the roles, each a day of windows. A new activity
//! is an entry here, not code.

use serde::Deserialize;
use std::collections::BTreeMap;

/// Where the plan is kept, under `assets/`.
pub const FILE: &str = "data/life.ron";

/// A settlement's days.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Plan {
    /// How many live in each home, at least and at most.
    pub household: (u32, u32),
    /// What can be done round each kind of thing, by its name (`terra_build::town::ThingKind`).
    pub things: BTreeMap<String, Vec<SpotPlan>>,
    pub activities: BTreeMap<String, Activity>,
    /// The roles, those taken first first: each takes its count of the town's people.
    pub roles: Vec<Role>,
    /// The kinds of animal the town keeps, by name.
    #[serde(default)]
    pub animals: BTreeMap<String, crate::animals::Kind>,
}

/// A place by a thing where something is done.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct SpotPlan {
    /// Where, in the thing's own metres: across its front, and out of it.
    pub at: (f32, f32),
    /// Which way someone there faces, in the thing's own terms; toward the thing's middle
    /// if not said.
    #[serde(default)]
    pub face: Option<(f32, f32)>,
    /// Measured from where someone coming to the thing arrives rather than its middle: a
    /// pen's gate, which is wherever its yard puts it.
    #[serde(default)]
    pub from_stand: bool,
    /// Any number may be there at once: a door going in, a gate going out.
    #[serde(default)]
    pub shared: bool,
    /// The activities done there.
    pub does: Vec<String>,
}

/// Something done.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct Activity {
    /// The body's clip while it is done.
    #[serde(default)]
    pub clip: Option<String>,
    /// How many game minutes it is done at one place before another is picked.
    #[serde(default)]
    pub stay: (f32, f32),
    /// Only at the person's own things: their stall, their forge, their home's yard.
    #[serde(default)]
    pub own: bool,
    /// Done indoors at home: gone in through the door.
    #[serde(default)]
    pub inside: bool,
    /// Done out of the town: gone out through a gate.
    #[serde(default)]
    pub away: bool,
    /// Got to at a run.
    #[serde(default)]
    pub run: bool,
    /// Tried before the window's others whenever one of its places is open and free: work
    /// that comes and goes, as a building site's while its building goes up (D381).
    #[serde(default)]
    pub first: bool,
}

/// How many of the town take a role.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub enum Count {
    /// One for each thing of a kind, who owns it.
    Per(String),
    Fixed(u32),
    /// A share of everyone.
    Share(f32),
    /// Everyone left.
    Rest,
}

/// A role: who, how many, and their day.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Role {
    pub name: String,
    pub count: Count,
    #[serde(default)]
    pub child: bool,
    /// From each hour, until the next window's, one of these activities at a time.
    pub day: Vec<Window>,
}

/// A part of a day: from an hour, what is done.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Window {
    pub from: f32,
    pub does: Vec<String>,
}

impl Plan {
    /// The plan from its file's text, checked: every activity a spot or a window names is
    /// one there is, every window in order through the day.
    pub fn parse(text: &str) -> Result<Plan, String> {
        let plan: Plan = ron::from_str(text).map_err(|why| format!("{FILE}: {why}"))?;
        for (thing, spots) in &plan.things {
            for spot in spots {
                for activity in &spot.does {
                    if !plan.activities.contains_key(activity) {
                        return Err(format!(
                            "{FILE}: a {thing}'s spot does {activity}, which is no activity"
                        ));
                    }
                }
            }
        }
        for role in &plan.roles {
            if role.day.is_empty() {
                return Err(format!("{FILE}: {} has no day", role.name));
            }
            for pair in role.day.windows(2) {
                if pair[1].from <= pair[0].from {
                    return Err(format!(
                        "{FILE}: {}'s day is out of order at {}",
                        role.name, pair[1].from
                    ));
                }
            }
            for window in &role.day {
                if !(0.0..24.0).contains(&window.from) {
                    return Err(format!(
                        "{FILE}: {}'s day has an hour {}",
                        role.name, window.from
                    ));
                }
                for activity in &window.does {
                    if !plan.activities.contains_key(activity) {
                        return Err(format!(
                            "{FILE}: {} does {activity}, which is no activity",
                            role.name
                        ));
                    }
                }
            }
        }
        Ok(plan)
    }
}
