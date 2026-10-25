//! Building play on a base (D407): what is added over the base on screen (`build::plugin`)
//! wherever a base is built on, by a player or the build keys alone: what building costs
//! and how each kind goes in ([`BuildRules`], from the kits the base read), the claim's
//! offcuts, the base's fire, and the kit sheet and villages laid as scenes; and, where a
//! player builds ([`player_plugin`]), acting on the base and all that follows from it.

use crate::act::{BuildRules, Offcuts};
use crate::{act, fire, going_up, loss, props, remote, sheet, village};
use bevy::prelude::*;
use terra_bases::memory;
use terra_bases::{BuildLook, footing_kind, kit_names, plans};
use terra_build::kit::Pattern;

/// Marks that building play is in place over the base, so asking for it twice is safe.
#[derive(Resource)]
struct Added;

/// The base on screen and building play over it, without a player: the build keys, the kit
/// sheet and villages laid as scenes.
pub fn plugin(app: &mut App) {
    terra_bases::plugin(app);
    if app.world().contains_resource::<Added>() {
        return;
    }
    // Acting on the base (`act::Building`, which the fire burns through) carries and tells
    // by survival's belongings and messages (D198); a run of building play without
    // survival's plugins (the sandbox's `build`) makes them here, and where survival is
    // added this leaves its own be.
    terra_survival::inventory::Belongings::init(app);
    app.add_message::<terra_survival::told::Told>();
    app.insert_resource(Added)
        .init_resource::<BuildRules>()
        .init_resource::<Offcuts>()
        .init_resource::<act::Applying>()
        .add_systems(Startup, read_rules.after(terra_bases::setup))
        .add_systems(Update, (sheet::lay, village::lay, village::lay_scale));
    fire::plugin(app);
}

/// The player's base, built by plans (docs/building-plan.md, step 4): acting on it, its
/// doors and shutters and every village house's, and drawing it again as it changes. Only
/// where a player builds, so a run of the build keys alone needs nothing of the player.
pub fn player_plugin(app: &mut App) {
    act::plugin(app);
    props::plugin(app);
    loss::plugin(app);
    remote::plugin(app);
    crate::contacts::plugin(app);
    memory::remote(app);
    going_up::plugin(app);
}

/// What building costs and how each kind goes in, from the kits the base read as it was set
/// up (the catalogue's prices are put in at `PostStartup`, `tool`'s `price`).
fn read_rules(look: Res<BuildLook>, mut rules: ResMut<BuildRules>) {
    let Some(kits) = look.kits.as_deref() else {
        return;
    };
    *rules = BuildRules {
        rules: kits.rules(),
        blocks: kits
            .0
            .iter()
            .map(|kit| kit.kind())
            .filter(|&kind| kits.one_at_a_time(kind))
            .collect(),
        footed: kits
            .0
            .iter()
            .filter(|kit| kit.pattern == Pattern::Masonry && kit.framing.is_none())
            .map(|kit| kit.kind())
            .collect(),
        names: kit_names(),
        budgets: Vec::new(),
        footing: footing_kind(kits),
        families: kits
            .0
            .iter()
            .map(|kit| (kit.kind(), plans::family(kit)))
            .collect(),
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_core::{PlanetSpec, Seed};
    use terra_world::planet::Planet;
    use terra_world::start::Start;

    /// The rules building play reads are the kits the base read as it was set up: every kit
    /// is named in them, and its name finds it again.
    #[test]
    fn building_play_reads_its_rules_from_the_kits_the_base_read() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .init_asset::<Image>();
        let planet = Planet::new(PlanetSpec {
            seed: Seed(1),
            radius_m: 2000.0,
        });
        app.insert_resource(terra_world::shell::Options::from_args(std::iter::empty()).unwrap())
            .insert_resource(Start(Transform::from_translation(planet.start())))
            .insert_resource(planet)
            .insert_resource(terra_bases::DrivenByPlayer);
        plugin(&mut app);
        app.world_mut().run_schedule(Startup);
        let kits = app.world().resource::<BuildLook>().kits.clone();
        let kits = kits.expect("the base reads the kits");
        let rules = app.world().resource::<BuildRules>();
        assert!(!kits.0.is_empty());
        for kit in &kits.0 {
            let name = rules.name(kit.kind());
            assert_ne!(name, "plain", "kit {:?} is named in the rules", kit.kind());
            assert_eq!(rules.kind(name), Some(kit.kind()), "{name} finds its kit");
        }
    }
}
