//! The lush forest through the year (climate.md, D327): its leaves, bark and crowns are grown
//! plants' material, which reads the climate's maps (`terra_world::grown::FoliageClimate`)
//! each time they are painted. Which kind a leaf is, the card it is drawn from says, as each
//! kind's year in `assets/data/plant_years.ron` names its cards. And the sky the crowns hide
//! follows the broadleaves' leaves.

use bevy::prelude::*;
use terra_world::climate::{ClimateFields, ClimateMaps, PastCache};
use terra_world::daylight::Clock;
use terra_world::sky_occlusion::{self, SkyOccluders};

pub fn plugin(app: &mut App) {
    use terra_world::instance::InstanceApp;
    // The crowns are the world's forest's: another world's are found afresh.
    app.world_resource::<Crowns>().add_systems(
        Update,
        follow_leaves.run_if(terra_world::instance::in_world),
    );
}

/// How much of the sky a broadleaf's crown still hides bare, of what it hides in full leaf:
/// its limbs and twigs. A leafless oak wood lets through something like half to two thirds
/// of the light its summer canopy stops.
const BARE_CROWN: f32 = 0.35;
/// The crowns are worked out again when the broadleaves' leaves have come or gone by this
/// share since: a few times in an autumn and a spring, a few seconds on other threads each.
const RESHADE_BY: f32 = 0.1;

/// The crowns as the forest was planted, which of them are broadleaves', and how far in leaf
/// they were last shaded.
#[derive(Resource)]
struct Crowns {
    planted: Vec<sky_occlusion::Crown>,
    broadleaf: Vec<bool>,
    shaded_at: f32,
}

/// The sky occlusion volume (D270) follows the broadleaves' leaves: a bare oak's crown hides
/// far less of the sky than one in leaf, so under bare oaks the shade is the winter sky's,
/// not summer's green (D327).
fn follow_leaves(
    mut commands: Commands,
    occluders: Option<ResMut<SkyOccluders>>,
    crowns: Option<ResMut<Crowns>>,
    trees: Query<(&Name, &GlobalTransform)>,
    maps: Option<Res<ClimateMaps>>,
    fields: Option<Res<ClimateFields>>,
    clock: Res<Clock>,
    mut cache: Local<Option<PastCache>>,
) {
    let (Some(mut occluders), Some(fields)) = (occluders, fields) else {
        return;
    };
    let Some(mut crowns) = crowns else {
        // Which tree each crown is: the one whose foot is nearest under its middle.
        let feet: Vec<(Vec3, bool)> = trees
            .iter()
            .filter(|(name, _)| matches!(name.as_str(), "Oak" | "Pine" | "Spruce"))
            .map(|(name, at)| (at.translation(), name.as_str() == "Oak"))
            .collect();
        if feet.is_empty() {
            return;
        }
        let up = occluders.up;
        let broadleaf = occluders
            .crowns
            .iter()
            .map(|crown| {
                feet.iter()
                    .min_by(|a, b| {
                        let across = |foot: Vec3| (crown.middle - foot).reject_from(up).length();
                        across(a.0).total_cmp(&across(b.0))
                    })
                    .is_some_and(|(_, oak)| *oak)
            })
            .collect();
        commands.insert_resource(Crowns {
            planted: occluders.crowns.clone(),
            broadleaf,
            shaded_at: 1.0,
        });
        return;
    };
    // Each time the maps are painted, and once when the crowns are first known: the floor,
    // and the crowns with it, come some frames after the world is entered (`lay_floor`), by
    // when the maps' first painting may be past and the next an hour or a walk away.
    if !(crowns.is_added() || maps.is_some_and(|maps| maps.is_changed())) {
        return;
    }
    let Some(oak) = fields.years.iter().find(|year| year.kind == "oak") else {
        return;
    };
    let cache = cache.get_or_insert_with(|| PastCache::new(4));
    let time = clock.time();
    let past = fields.past_at(cache, occluders.middle, time);
    let hours = fields
        .fields
        .daylight(fields.frame.latitude(occluders.middle), time.day())
        .day_hours;
    let on = oak.leaves(&past, hours, 0.0).on;
    // As planted, in full leaf; again only once the leaves have come or gone far enough.
    if (on - crowns.shaded_at).abs() < RESHADE_BY {
        return;
    }
    crowns.shaded_at = on;
    let thin = BARE_CROWN + (1.0 - BARE_CROWN) * on;
    occluders.crowns = crowns
        .planted
        .iter()
        .zip(&crowns.broadleaf)
        .map(|(crown, broadleaf)| sky_occlusion::Crown {
            closes: crown.closes * if *broadleaf { thin } else { 1.0 },
            ..*crown
        })
        .collect();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The day on which the oaks at the start are least in leaf, and how far, found from the
    /// climate itself.
    fn a_bare_day(fields: &ClimateFields) -> (Clock, f32) {
        let oak = fields
            .years
            .iter()
            .find(|year| year.kind == "oak")
            .expect("plant_years.ron has an oak");
        let mut cache = PastCache::new(4);
        let (clock, on) = (0..2000)
            .step_by(7)
            .map(|day| {
                let clock = Clock { day, hour: 12.0 };
                let time = clock.time();
                let past = fields.past_at(&mut cache, Vec3::Y, time);
                let hours = fields
                    .fields
                    .daylight(fields.frame.latitude(Vec3::Y), time.day())
                    .day_hours;
                (clock, oak.leaves(&past, hours, 0.0).on)
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .expect("some day");
        // Far enough out of leaf that the crowns are shaded again (`RESHADE_BY`).
        assert!(on < 0.7, "the oaks are at least {on} in leaf all year");
        (clock, on)
    }

    /// The fault planted: the crowns arrive frames after the maps' first painting, as they
    /// do when the floor is decided behind the loading screen. The bare oak's crown must
    /// still let the winter sky through, without waiting for the maps' next painting.
    #[test]
    fn crowns_that_come_after_the_maps_are_painted_are_thinned_for_bare_oaks() {
        let years = terra_world::data::read(terra_world::climate::YEARS_FILE);
        let fields = ClimateFields::for_tests_of(years, 2000.0, terra_gen::Seed(1));
        let (clock, on) = a_bare_day(&fields);
        let mut app = App::new();
        app.insert_resource(fields)
            .insert_resource(clock)
            .insert_resource(ClimateMaps::default())
            .add_systems(Update, follow_leaves);
        let foot = Vec3::Y;
        app.world_mut()
            .spawn((Name::new("Oak"), GlobalTransform::from_translation(foot)));
        app.world_mut().spawn((
            Name::new("Pine"),
            GlobalTransform::from_translation(foot + Vec3::X * 20.0),
        ));
        // The maps are painted and seen, the forest's floor still being decided.
        app.update();
        app.update();
        let crown = |at: Vec3| sky_occlusion::Crown {
            middle: at + Vec3::Y * 8.0,
            half: Vec3::splat(4.0),
            closes: 0.8,
        };
        app.insert_resource(SkyOccluders {
            middle: foot,
            up: Vec3::Y,
            east: Vec3::X,
            half_m: 40.0,
            below_m: 5.0,
            above_m: 35.0,
            crowns: vec![crown(foot), crown(foot + Vec3::X * 20.0)],
            shadows: Vec::new(),
        });
        app.update();
        app.update();
        let crowns = &app.world().resource::<SkyOccluders>().crowns;
        let thin = BARE_CROWN + (1.0 - BARE_CROWN) * on;
        assert!(
            (crowns[0].closes - 0.8 * thin).abs() < 1e-4,
            "the bare oak's crown closes {} of the sky, not {} ({on} in leaf)",
            crowns[0].closes,
            0.8 * thin
        );
        assert_eq!(crowns[1].closes, 0.8, "the pine keeps its needles");
    }
}
