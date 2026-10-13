//! A settlement (game design 001; D95): a few houses, a well and a stall, and the people
//! who live there. Their elder says what is wrong, that the spiders took the smith, and
//! where to look, by the sun and the glow rather than by a marker (D92). Others say what
//! they know. Bring the smith home to your claim and the elder says so.
//!
//! Everything in it is built from the same parts as the player's things (D108): each
//! building is laid out from the village's seed by `terra_build::village`, a street of
//! houses of every kind of construction, a shed, a lookout and garden walls, and put up by
//! the same shortcuts as a player's rooms; the well and stall are catalogue items, placed
//! as a player places one. They are the villagers', not the player's. What grows keeps out
//! of every building and a margin round it, and off the street. The people go about their
//! day where they stand.

use super::Footprint;
use super::catalogue::Builds;
use super::creatures::Lair;
use super::daylight::{Clock, SkyPath};
use super::flora::ClearAround;
use super::interact::{Interactable, Interacted};
use super::people::{Arrive, Doing, Dyed, Person};
use super::props::{self, Models, Theirs};
use super::tick::Sim;
use super::told::Told;
use crate::build::{self, Build, BuildLook, Built, Site};
use crate::planet::Planet;
use crate::start::Start;
use bevy::prelude::*;
use terra_build::{Base, CELL_M};

/// An item from the catalogue the village has: what it is, where it stands from the
/// middle in metres east and south, and how many degrees it is turned.
const ITEMS: [(&str, f32, f32, f32); 3] = [
    ("../village/Well", 0.5, -1.5, 0.0),
    ("../village/MarketStand_1", 7.0, 3.5, -60.0),
    // Ilse's stool.
    ("Stool", -3.5, 2.5, 0.0),
];

/// One of the village's people: who, where they stand from the middle in metres east and
/// south, what they are doing, the template they are shaped as, what they wear, and what
/// they say when spoken to.
pub(super) struct Villager {
    name: &'static str,
    east: f32,
    south: f32,
    doing: Doing,
    pub(super) template: &'static str,
    pub(super) wears: &'static [Dyed],
    says: Option<&'static str>,
}

/// The elder by the well, and others about their day.
pub(super) const PEOPLE: [Villager; 4] = [
    Villager {
        name: "Maren",
        east: 1.6,
        south: 1.4,
        doing: Doing::Standing,
        template: "female",
        wears: &[Dyed(
            "leather_coat",
            Some(("Leather", Color::srgb(0.32, 0.42, 0.55))),
        )],
        says: None,
    },
    Villager {
        name: "Tobin",
        east: 5.0,
        south: 2.6,
        doing: Doing::Standing,
        template: "heavy",
        wears: &[
            Dyed(
                "leather_coat",
                Some(("Leather", Color::srgb(0.55, 0.42, 0.24))),
            ),
            Dyed("leather_cap", None),
        ],
        says: Some("Tobin: \"Ore rock has a rust to it. A stone pick will break it, slowly.\""),
    },
    Villager {
        name: "Ilse",
        east: -3.5,
        south: 2.5,
        doing: Doing::Sitting,
        template: "female",
        wears: &[Dyed(
            "leather_cap",
            Some(("Leather", Color::srgb(0.46, 0.5, 0.3))),
        )],
        says: Some("Ilse: \"The deer graze in the open. Come at them quietly, or they're gone.\""),
    },
    Villager {
        name: "Wyn",
        east: -2.0,
        south: -3.0,
        doing: Doing::Standing,
        template: "thin",
        wears: &[Dyed(
            "small_pack",
            Some(("Pack", Color::srgb(0.6, 0.3, 0.28))),
        )],
        says: Some("Wyn: \"Meat keeps you longer cooked. Any fire will do.\""),
    },
];

#[derive(Resource)]
pub struct Village(pub Vec3);

pub fn plugin(app: &mut App) {
    app.add_systems(Update, found.run_if(resource_added::<Lair>))
        .add_systems(
            FixedUpdate,
            talk.run_if(resource_exists::<Village>).in_set(Sim::World),
        );
}

/// How far the street runs east and west of the green's middle, and how wide it is, in
/// metres: kept clear of what grows, and trodden bare.
const STREET_M: (f32, f32) = (42.0, 5.0);

/// Once the lair is chosen, sets the village down somewhere else, dry and level enough,
/// and builds it from its seed (`terra_build::village`, D108): each building put up by the
/// same shortcuts as the player's, stood on the ground where it goes, its door to the
/// street.
#[allow(clippy::too_many_arguments)]
fn found(
    mut commands: Commands,
    site: Res<Site>,
    start: Res<Start>,
    planet: Res<Planet>,
    lair: Res<Lair>,
    look: Res<BuildLook>,
    models: Res<Models>,
    builds: Res<Builds>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut arrive: MessageWriter<Arrive>,
    mut clear: MessageWriter<ClearAround>,
) {
    let on_ground = |east: f32, south: f32| planet.ground_at(start.at(east, south));
    let dry = |point: Vec3| point.length() > planet.sea_radius_m() + 1.0;
    let Some(middle) = [(-70.0, 55.0), (75.0, 60.0), (-60.0, -75.0), (60.0, -80.0)]
        .into_iter()
        .map(|(east, south)| on_ground(east, south))
        .filter(|&point| dry(point) && point.distance(lair.0) > 60.0)
        .max_by(|a, b| a.distance(lair.0).total_cmp(&b.distance(lair.0)))
    else {
        warn!("No dry ground for the village.");
        return;
    };
    commands.insert_resource(Village(middle));
    let up = middle.normalize();
    let east = up.cross(Vec3::Y).normalize_or(up.any_orthonormal_vector());
    let south = east.cross(up);
    let at = |e: f32, s: f32| planet.ground_at(middle + east * e + south * s);
    // The street, its plants cleared and trodden bare.
    let (long, wide) = STREET_M;
    let mut step = -long;
    while step <= long {
        clear.write(ClearAround(at(step, 0.0), wide + 1.5));
        step += 4.0;
    }
    let street = Transform::from_matrix(Mat4::from_cols(
        east.extend(0.0),
        up.extend(0.0),
        south.extend(0.0),
        middle.extend(1.0),
    ));
    let cells = |metres: f32| (metres / CELL_M).round() as i32;
    commands.spawn((
        Name::new("Village street"),
        Footprint {
            frame: street,
            cell_m: CELL_M,
            ground: (-cells(long)..cells(long))
                .flat_map(|x| (-cells(wide)..cells(wide)).map(move |z| IVec2::new(x, z)))
                .collect(),
            ..default()
        },
    ));

    let to_site = site.frame.compute_affine().inverse();
    let kinds = look.kits.as_deref().and_then(build::village_kinds);
    let Some(kinds) = kinds else {
        warn!("The village needs a log, a stone and a hewn timber kit.");
        return;
    };
    let seed = planet.seed().derive("village").0 as u32;
    for building in terra_build::village::village(seed, kinds) {
        let [e, s] = building.at;
        let door = at(e, s);
        let up = door.normalize();
        let east = up.cross(Vec3::Y).normalize_or(up.any_orthonormal_vector());
        let south = east.cross(up);
        // Its own +Z turned to look the way its door does, its X along the street.
        let rotation = Quat::from_mat3(&Mat3::from_cols(east, up, south))
            * Quat::from_rotation_y(building.facing);
        let mut shape = Base::default();
        for &op in &building.ops {
            shape.apply(op);
        }
        let frame = build::lifted(
            Transform::from_translation(door).with_rotation(rotation),
            &shape,
            &planet,
        );
        let label = format!("Settlement: {}", building.name);
        let mut built = Build::new(&mut commands, &look, frame, &label);
        built.replace_with(&building.ops, &mut commands, &planet, &mut meshes);
        commands.spawn((Name::new(label), Built(built)));
        let local = to_site.transform_point3(door);
        info!(
            "Settlement: {}: its door {:.1} m east and {:.1} m south of the start, looking {:.0} degrees from south.",
            building.name,
            local.x,
            local.z,
            building.facing.to_degrees()
        );
    }

    // The well and the stall, placed as a player places an item, but the villagers'.
    for (file, item_east, item_south, turn) in ITEMS {
        let Some(item) = builds.item_from(file) else {
            error!("The village wants {file}, which is not in the catalogue.");
            continue;
        };
        let at = to_site.transform_point3(middle + east * item_east + south * item_south);
        if let Some(entity) = props::place(
            &mut commands,
            &models,
            &site,
            &planet,
            &item,
            at.with_y(0.0),
            0.0,
            turn,
            false,
        ) {
            commands.entity(entity).insert(Theirs);
        }
    }

    // The people: the elder by the well, and others about their day.
    for villager in PEOPLE {
        let at = middle + east * villager.east + south * villager.south;
        arrive.write(Arrive {
            name: villager.name,
            at,
            facing: (middle - at).reject_from(up),
            doing: villager.doing,
            trade: None,
            template: villager.template,
            wears: villager.wears.to_vec(),
            interact: Some(Interactable::Talk),
            says: villager.says,
        });
    }
    let local = site
        .frame
        .compute_affine()
        .inverse()
        .transform_point3(middle);
    info!(
        "The village stands {:.0} m east and {:.0} m south of the start.",
        local.x, local.z
    );
}

/// Which way a place lies from another, as someone there would say it: by the sun, or at
/// night by the moon, wherever it is in the sky just now.
fn by_the_sky(from: Vec3, to: Vec3, toward_sky: Vec3, by_night: bool) -> String {
    let up = from.normalize();
    let way = (to - from).reject_from(up).normalize_or_zero();
    let light = toward_sky.reject_from(up).normalize_or_zero();
    let body = if by_night { "moon" } else { "sun" };
    let toward = way.dot(light);
    let side = way.dot(up.cross(light));
    if toward > 0.6 {
        format!("toward the {body}")
    } else if toward < -0.6 {
        format!("with the {body} at your back")
    } else if side > 0.0 {
        format!("with the {body} on your right")
    } else {
        format!("with the {body} on your left")
    }
}

/// F at someone in the village: they say what they know.
fn talk(
    clock: Res<Clock>,
    sky: Option<Res<SkyPath>>,
    mut interacted: MessageReader<Interacted>,
    village: Res<Village>,
    lair: Res<Lair>,
    people: Query<&Person>,
    mut told: MessageWriter<Told>,
) {
    for used in interacted.read() {
        let Some(person) = used
            .on(Interactable::Talk)
            .and_then(|entity| people.get(entity).ok())
        else {
            continue;
        };
        if let Some(line) = person.says {
            told.write(Told::new(used.player, line));
            continue;
        }
        // The elder speaks of the smith.
        let smith = people
            .iter()
            .find(|person| person.trade == Some("smith"))
            .map(|smith| smith.doing);
        let line = match smith {
            Some(Doing::Bound) | None => format!(
                "{}: \"Spiders took Bren, our smith. They nest where the light comes out of the ground, {}. You'll see the glow.\"",
                person.name,
                match &sky {
                    Some(sky) => {
                        let sun = sky.sun(clock.hour);
                        let by_night = sky.height(clock.hour) < 0.0;
                        by_the_sky(
                            village.0,
                            lair.0,
                            if by_night { -sun } else { sun },
                            by_night,
                        )
                    }
                    None => "past the trees".to_string(),
                }
            ),
            Some(Doing::Following(_)) => format!(
                "{}: \"You found Bren! Take them somewhere safe; they'll work a forge for you.\"",
                person.name
            ),
            Some(_) => format!(
                "{}: \"Bren's working your forge, I hear. You've done right by us.\"",
                person.name
            ),
        };
        told.write(Told::new(used.player, line));
    }
}
