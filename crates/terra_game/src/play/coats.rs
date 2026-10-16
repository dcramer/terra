//! Coats that turn with the seasons (creatures.md): a mountain hare or an arctic fox is
//! brown in summer and white in winter, moulting as the days shorten in autumn and lengthen in
//! spring, earlier and further where the land is colder, each animal on its own few days, as
//! the real ones do (their moult follows the day's length, its timing and how white they go
//! the climate: an Irish hare stays part brown all winter). So a heath in September has brown
//! hares, in October a mix of brown, patched and white, in January white ones.
//!
//! A kind's summer coat is a tint over its model's own pale colours (`coat` in
//! `creatures.ron`), laid on as a material of its own; animals of a kind share a few, the
//! coat taken in steps, so a hundred hares are a handful of materials (D127).

use super::creatures::{Creature, luck};
use super::daylight::{Clock, WorldClimate};
use super::species::{Bestiary, CoatDef};
use super::wildlife::Surroundings;
use bevy::{platform::collections::HashMap, prelude::*};

/// How many steps a coat is taken in, from summer's to winter's.
const STEPS: f32 = 6.0;

pub fn plugin(app: &mut App) {
    app.init_resource::<Tints>()
        .add_systems(Update, (choose, dress).chain());
}

/// An animal's coat, from its summer's (0) to its winter's (1), and its own tone.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Coat {
    pub white: f32,
    pub tone: f32,
}

/// Its meshes wear their coat.
#[derive(Component)]
struct Dressed;

/// The materials made for coats, by the model's own material and the tint laid on it.
#[derive(Resource, Default)]
struct Tints(HashMap<(AssetId<StandardMaterial>, [u16; 3]), Handle<StandardMaterial>>);

/// How white a coat is at a share of the year after midwinter (0 midwinter, 0.5 midsummer),
/// where the growing season's mean is so warm, for an animal whose moult runs so many shares
/// of a year early (or late, below nothing).
pub fn whiteness(phase: f32, warmth_c: f32, own: f32) -> f32 {
    // Colder land whitens earlier in autumn and browns later in spring.
    let early = ((5.0 - warmth_c) * 0.008).clamp(-0.03, 0.05) + own;
    // Most white in midwinter: wholly on cold land, part brown on mild.
    let most = (1.0 - (warmth_c - 4.0) * 0.12).clamp(0.5, 1.0);
    let ease = |from: f32, to: f32, at: f32| {
        let s = ((at - from) / (to - from)).clamp(0.0, 1.0);
        s * s * (3.0 - 2.0 * s)
    };
    // White from mid-October to the end of November, brown again from April into May.
    let autumn = ease(0.81 - early, 0.93 - early, phase);
    let spring = 1.0 - ease(0.27 + early, 0.39 + early, phase);
    most * if phase >= 0.6 { autumn } else { spring }
}

/// Gives each new animal of a kind with a seasonal coat its coat for the date, where it lives.
fn choose(
    mut commands: Commands,
    bestiary: Res<Bestiary>,
    clock: Res<Clock>,
    climate: Res<WorldClimate>,
    around: Surroundings,
    born: Query<(Entity, &Creature), Without<Coat>>,
) {
    let phase = climate.0.year.phase(clock.day);
    for (entity, creature) in &born {
        let Some(def) = bestiary.get(creature.species).coat else {
            continue;
        };
        // Each its own: a few days either way in its moult, and its tone.
        let luck = luck(creature.home());
        let own = (luck.x - 0.5) * 0.05;
        let white = if def.moults {
            whiteness(phase, around.warmth_c(creature.home()), own)
        } else {
            0.0
        };
        commands.entity(entity).insert(Coat {
            white,
            tone: (luck.y - 0.5) * 2.0 * def.strays,
        });
    }
}

/// The tint a coat lays over its model's colours.
pub fn tint(def: &CoatDef, coat: Coat) -> Vec3 {
    let white = (coat.white * STEPS).round() / STEPS;
    // Five tones a kind, from its darkest to its palest.
    let step = def.strays.max(1e-3) * 0.5;
    let tone = (coat.tone / step).round() * step;
    let summer = Vec3::from(def.summer) * (1.0 + tone);
    summer.lerp(Vec3::ONE, white).min(Vec3::ONE)
}

/// Puts each animal's meshes in its coat, once its model is in.
fn dress(
    mut commands: Commands,
    bestiary: Res<Bestiary>,
    mut tints: ResMut<Tints>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    animals: Query<(Entity, &Creature, &Coat), Without<Dressed>>,
    children: Query<&Children>,
    mut meshes: Query<&mut MeshMaterial3d<StandardMaterial>>,
) {
    for (entity, creature, &coat) in &animals {
        let Some(def) = bestiary.get(creature.species).coat else {
            continue;
        };
        let tint = tint(&def, coat);
        let key = (tint * 1000.0).round().as_uvec3();
        let key = [key.x as u16, key.y as u16, key.z as u16];
        let mut found = false;
        for child in children.iter_descendants(entity) {
            let Ok(mut material) = meshes.get_mut(child) else {
                continue;
            };
            found = true;
            let own = material.0.id();
            let worn = tints
                .0
                .entry((own, key))
                .or_insert_with(|| {
                    let mut made = materials.get(own).cloned().unwrap_or_default();
                    let base = made.base_color.to_linear();
                    made.base_color = Color::linear_rgba(
                        base.red * tint.x,
                        base.green * tint.y,
                        base.blue * tint.z,
                        base.alpha,
                    );
                    materials.add(made)
                })
                .clone();
            material.0 = worn;
        }
        if found {
            commands.entity(entity).insert(Dressed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A share of the year after midwinter, the 21st of December, for a day of a month.
    fn on(month: u32, day: u32) -> f32 {
        let days = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
        let of_year: u32 = days[..month as usize - 1].iter().sum::<u32>() + day - 1;
        ((of_year as f32 + 365.0 - 354.0) % 365.0) / 365.0
    }

    #[test]
    fn a_hare_on_cold_land_is_brown_in_summer_white_in_winter_and_patched_between() {
        let cold = 3.0;
        assert_eq!(whiteness(on(7, 15), cold, 0.0), 0.0, "July");
        assert!(
            whiteness(on(9, 12), cold, 0.0) < 0.05,
            "still brown in September"
        );
        let october = whiteness(on(10, 25), cold, 0.0);
        assert!(
            october > 0.2 && october < 0.8,
            "moulting in late October: {october}"
        );
        assert!(whiteness(on(1, 15), cold, 0.0) > 0.99, "white in January");
        let april = whiteness(on(4, 20), cold, 0.0);
        assert!(
            april > 0.1 && april < 0.9,
            "moulting back in April: {april}"
        );
        assert_eq!(whiteness(on(6, 10), cold, 0.0), 0.0, "June");
    }

    #[test]
    fn milder_land_whitens_later_and_less_and_each_animal_on_its_own_days() {
        let (cold, mild) = (2.0, 7.0);
        let late_october = on(10, 28);
        assert!(whiteness(late_october, mild, 0.0) < whiteness(late_october, cold, 0.0));
        assert!(
            whiteness(on(1, 15), mild, 0.0) < 0.9,
            "part brown all winter"
        );
        assert!(whiteness(late_october, cold, 0.02) > whiteness(late_october, cold, -0.02));
    }

    #[test]
    fn a_summer_coat_tints_the_model_and_a_winter_one_leaves_it_pale() {
        let def = CoatDef {
            summer: (0.6, 0.45, 0.3),
            strays: 0.1,
            moults: true,
        };
        let summer = tint(
            &def,
            Coat {
                white: 0.0,
                tone: 0.0,
            },
        );
        assert!((summer - Vec3::new(0.6, 0.45, 0.3)).length() < 1e-4);
        assert_eq!(
            tint(
                &def,
                Coat {
                    white: 1.0,
                    tone: 0.1
                }
            ),
            Vec3::ONE
        );
        let darker = tint(
            &def,
            Coat {
                white: 0.0,
                tone: -0.1,
            },
        );
        assert!(darker.x < summer.x);
    }
}
