//! Digging (D39, D96): a pickaxe aimed at bare ground digs it. Hold the button, and after
//! a moment a scoop of ground comes out and the hole shows. What came out is measured by
//! material, and each material becomes items by the volume dug, as
//! `assets/data/ground.ron` says (D136), the rest of a part kept toward the next, so nothing
//! dug is lost. Materials it does not name, such as soil and sand, give nothing yet.
//!
//! A tree, a rock or a plant in the way is worked as before (`harvest`); digging is for
//! when the pickaxe meets the ground itself. Digging works anywhere for now; that the
//! world heals outside a claim (D94) comes later.

use super::controls::Action;
use super::data;
use super::harvest::Cutting;
use super::intent::Intent;
use super::inventory::{Belongings, Cut, Items};
use super::skills::{Did, ONCE, Skill, Skills};
use super::tick::Sim;
use super::work::Hold;
use super::{LocalPlayer, Player, Stance};
use crate::planet::Planet;
use bevy::prelude::*;
use serde::Deserialize;
use terra_core::MaterialId;
use terra_core::{Harvested, Operation};
use terra_items::ItemKind;
use terra_voxel::material;

/// How far from the player the pickaxe reaches the ground.
const REACH_M: f32 = 4.0;
/// How big a scoop one dig takes.
const SCOOP_M: f32 = 0.7;
/// How long a dig takes with a stone pickaxe at the start, in seconds.
const DIG_SECONDS: f32 = 0.9;
/// What digging one material gives, as `assets/data/ground.ron` has it.
#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct YieldDef {
    material: String,
    gives: String,
    per_item_m3: f32,
}

/// What digging one material gives: so many cubic metres make one of a kind of item.
struct Yield {
    material: MaterialId,
    name: String,
    gives: ItemKind,
    per_item_m3: f32,
}

/// What digging each material gives (`assets/data/ground.ron`), a registry (D136).
#[derive(Resource)]
pub struct Yields(Vec<Yield>);

impl Yields {
    /// The materials that give something, in order.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.0.iter().map(|each| each.name.as_str())
    }

    pub fn read(items: &Items) -> Self {
        const FILE: &str = "ground.ron";
        let defs: Vec<YieldDef> = data::read(FILE);
        Self(
            defs.into_iter()
                .map(|def| Yield {
                    material: material::named(&def.material).unwrap_or_else(|| {
                        panic!("{FILE}: there is no material called {}", def.material)
                    }),
                    gives: items.named(&def.gives, FILE),
                    name: def.material,
                    per_item_m3: def.per_item_m3,
                })
                .collect(),
        )
    }
}

/// Where the pickaxe meets the ground, and how far through digging there it is.
#[derive(Component, Default)]
pub struct Digging {
    pub target: Option<Vec3>,
    held: Option<(Vec3, Hold)>,
}

impl Digging {
    pub fn progress(&self) -> Option<f32> {
        self.held.map(|(_, hold)| hold.fraction())
    }
}

/// What a player has dug but not yet made an item, by what it is made of: cubic metres of
/// each material that gives anything, in the order `Yields` has them.
#[derive(Component, Default)]
struct Spoil(Vec<f32>);

pub fn plugin(app: &mut App) {
    app.register_required_components::<Player, Digging>()
        .register_required_components::<Player, Spoil>()
        .add_systems(
            FixedUpdate,
            dig.after(super::harvest::Harvesting).in_set(Sim::Act),
        );
}

/// The items a part of what was dug makes, when so much makes one, and what is left toward
/// the next.
fn items_from(volume_m3: f32, per_item_m3: f32) -> (u32, f32) {
    let items = (volume_m3 / per_item_m3 + 1e-4).floor();
    (items as u32, (volume_m3 - items * per_item_m3).max(0.0))
}

fn dig(
    time: Res<Time>,
    mut players: Query<
        (
            Entity,
            &Transform,
            &Intent,
            &Skills,
            &Cutting,
            &mut Stance,
            &mut Digging,
            &mut Spoil,
            Has<LocalPlayer>,
        ),
        With<Player>,
    >,
    mut planet: ResMut<Planet>,
    yields: Res<Yields>,
    mut belongings: Belongings,
    mut did: MessageWriter<Did>,
    mut gizmos: Gizmos,
) {
    for (player, player_at, intent, skills, cutting, mut stance, mut digging, mut spoil, ours) in
        &mut players
    {
        let Some(mut belongings) = belongings.of(player) else {
            continue;
        };
        let feet = player_at.translation;
        digging.target = None;
        let pickaxe = belongings
            .tool_in_hand()
            .filter(|tool| tool.works(Cut::Rock) && intent.free())
            .cloned();
        // Something to cut in the way is cut, not dug round.
        let (Some(pickaxe), None) = (pickaxe, cutting.target) else {
            digging.held = None;
            continue;
        };
        let Some(aim) = intent.aim else {
            digging.held = None;
            continue;
        };
        let Some(hit) = planet
            .raycast(aim.origin, *aim.direction, 30.0)
            .filter(|hit| hit.point.distance(feet) < REACH_M)
        else {
            digging.held = None;
            continue;
        };
        digging.target = Some(hit.point);

        // A ring where it would dig, filling as it digs, for this machine's player to see.
        let normal = planet.normal(hit.point);
        let across = normal.any_orthonormal_vector();
        let fraction = digging.progress().unwrap_or(0.0);
        let around = |turn: f32| {
            hit.point
                + normal * 0.05
                + Quat::from_axis_angle(normal, turn * std::f32::consts::TAU) * across * SCOOP_M
        };
        let amber = Color::srgb(1.0, 0.78, 0.25);
        if ours {
            gizmos.linestrip(
                (0..=32).map(|step| around(step as f32 / 32.0)),
                amber.with_alpha(0.45),
            );
        }
        if ours && fraction > 0.0 {
            let steps = (fraction * 32.0).ceil() as usize;
            gizmos.linestrip(
                (0..=steps).map(|step| around(step as f32 / steps as f32 * fraction)),
                Color::WHITE,
            );
        }

        if !intent.held(Action::Use) {
            digging.held = None;
            continue;
        }
        // Moving the aim to another place starts again.
        if digging
            .held
            .is_some_and(|(at, _)| at.distance(hit.point) > SCOOP_M)
        {
            digging.held = None;
        }
        let speed = pickaxe.cuts(Cut::Rock).unwrap_or(1.0) * skills.speed(Skill::Mining);
        let (at, mut hold) = *digging
            .held
            .get_or_insert((hit.point, Hold::new(DIG_SECONDS / speed.max(0.01))));
        let finished = hold.advance(time.delta_secs());
        digging.held = Some((at, hold));
        stance.since_acting = 0.0;
        if !finished {
            continue;
        }
        digging.held = None;

        // The scoop is centred a little into the ground, so it bites.
        let taken = planet.apply(&Operation::Dig {
            centre: at - normal * (SCOOP_M * 0.3),
            radius: SCOOP_M,
        });
        // What it gives is shown as it comes in (`gains`).
        spoil.0.resize(yields.0.len(), 0.0);
        let mut dug = Vec::new();
        for (each, kept) in yields.0.iter().zip(spoil.0.iter_mut()) {
            let amount = taken.amount(Harvested::Material(each.material));
            dug.push(format!("{amount:.2} m³ of {}", each.name.to_lowercase()));
            *kept += amount;
            let (count, rest) = items_from(*kept, each.per_item_m3);
            if count == 0 {
                continue;
            }
            // What does not fit in the backpack drops at the player's feet (`pile`).
            belongings.give(each.gives, count);
            *kept = rest;
        }
        did.write(Did(player, Skill::Mining, ONCE));
        belongings.wear_in_hand(1);
        info!(
            "Dug at {:.1} m: {}, the rest giving nothing.",
            at.length(),
            dug.join(", ")
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::play::inventory::Items;

    #[test]
    fn what_is_dug_makes_items_and_keeps_the_rest() {
        const PER: f32 = 0.4;
        assert_eq!(items_from(PER * 0.5, PER).0, 0);
        let (count, rest) = items_from(PER * 2.25, PER);
        assert_eq!(count, 2);
        assert!((rest - PER * 0.25).abs() < 1e-5);
        assert_eq!(items_from(PER * 2.0, PER).0, 2);
    }

    /// Reading checks that each material and what it gives are ones there are.
    #[test]
    fn digging_stone_gives_something() {
        let yields = Yields::read(&Items::default());
        assert!(yields.0.iter().any(|each| each.material == material::STONE));
    }

    #[test]
    fn a_pickaxe_digs_and_an_axe_does_not() {
        let items = Items::default();
        let works = |name| items.tool(items.kind(name)).unwrap().works(Cut::Rock);
        assert!(works("Stone pickaxe"));
        assert!(!works("Stone axe"));
    }
}
