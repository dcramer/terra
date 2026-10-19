//! A settlement (game design 001; D95): a few houses, a well and a stall, and the people
//! who live there. Their elder says what is wrong, that the spiders took the smith, and
//! where to look, by the sun and the glow rather than by a marker (D92). Others say what
//! they know. Bring the smith home to your claim and the elder says so.
//!
//! Everything in it is built from the same parts as the player's things (D108): each
//! building is laid out from the village's seed by `terra_town::village`, a street of
//! houses of every kind of construction, a shed, a lookout and garden walls, and put up by
//! the same shortcuts as a player's rooms; the well and stall are catalogue items, placed
//! as a player places one. They are the villagers', not the player's. What grows keeps out
//! of every building and a margin round it, and off the street. The people go about their
//! day where they stand.

use super::Footprint;
use super::catalogue::Builds;
use super::claim::Village;
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
use terra_build_layout::fitting::Hang;

/// An item from the catalogue the village has: what it is, where it stands from the
/// middle in metres east and south, and how many degrees it is turned.
const ITEMS: [(&str, f32, f32, f32); 3] = [
    ("Well", 0.5, -1.5, 0.0),
    ("MarketStall", 7.0, 3.5, -60.0),
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
            Dyed("trousers", None),
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
            "trousers",
            Some(("Cloth", Color::srgb(0.46, 0.5, 0.3))),
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
/// and builds it from its seed (`terra_town::village`, D108): each building put up by the
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
    let kinds = look.kits.as_deref().and_then(build::village_kinds);
    let Some(kinds) = kinds else {
        warn!("The village needs a log, a stone and a hewn timber kit.");
        return;
    };
    let seed = planet.seed().derive("village").0 as u32;
    let buildings = terra_town::village::village(seed, &kinds);
    let Some((middle, east)) = site_for(&planet, &start, lair.0, &buildings) else {
        warn!("No dry ground for the village.");
        return;
    };
    let up = middle.normalize();
    let south = east.cross(up);
    let at = |e: f32, s: f32| planet.ground_at(middle + east * e + south * s);
    let reach = reaches(&buildings).iter().map(|&[e, s]| at(e, s)).collect();
    commands.insert_resource(Village(middle, reach));
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
    for building in buildings {
        let [e, s] = building.at;
        let door = at(e, s);
        let up = door.normalize();
        let east = east
            .reject_from(up)
            .normalize_or(up.any_orthonormal_vector());
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
        if let Err(why) = built.put_up(&building.ops, &mut commands, &planet, &mut meshes) {
            warn!(
                "Settlement: {}: not put up by the operations: {why}",
                building.name
            );
        }
        // Its doors and shutters, of one kind each a house, by the village's seed: the
        // doors mostly shut, the shutters mostly open.
        let pick = |hang, size: (i32, i32), salt: u32| {
            let kinds: Vec<_> = builds
                .fittings()
                .iter()
                .filter(|fitting| fitting.hang == hang && fitting.fits(size.0, size.1))
                .collect();
            let roll = |extra: u32| {
                terra_gen::hash::lowbias32_round(
                    seed ^ building.name.len() as u32
                        ^ (building.at[0].to_bits())
                        ^ extra.wrapping_mul(0x9e37_79b9),
                )
            };
            let fitting =
                kinds.get(roll(u32::from(hang == Hang::Door)) as usize % kinds.len().max(1))?;
            let open = match hang {
                Hang::Door => roll(salt + 7) % 3 == 0,
                Hang::Shutters => roll(salt + 11) % 4 != 0,
            };
            Some((fitting.kind, open))
        };
        let fitted = build::props::fit_all(&mut built, |hang, size, index| {
            pick(hang, size, index as u32)
        });
        info!(
            "Settlement: {}: {fitted} doors and shutters fitted.",
            building.name
        );
        // Its own name the base's, which stands where it does; this one holds the build.
        commands.spawn((Name::new(format!("{label}, as built")), Built(built)));
        let local = start.local(door);
        info!(
            "Settlement: {}: its door {:.1} m east and {:.1} m south of the start, looking {:.0} degrees from south.",
            building.name,
            local.x,
            local.z,
            building.facing.to_degrees()
        );
    }

    // Where to stand to see it, as the kit sheet gives its rows' (D69): an eye and what it
    // looks at, in metres east, up and south of the start, from places in the village's own
    // east and south and heights over its ground there.
    let vantage = |name: &str, eye: [f32; 3], look: [f32; 3]| {
        let point = |[e, s, up]: [f32; 3]| {
            let ground = at(e, s);
            start.local(ground + ground.normalize() * up)
        };
        let (eye, look) = (point(eye), point(look));
        info!(
            "Settlement vantage: {name}: --vantage {:.2},{:.2},{:.2},{:.2},{:.2},{:.2}",
            eye.x, eye.y, eye.z, look.x, look.y, look.z
        );
    };
    vantage("street", [-34.0, 0.5, 1.7], [6.0, -0.5, 2.2]);
    vantage("green", [-7.0, 4.0, 1.7], [0.0, -11.0, 3.0]);
    vantage("street east", [33.0, -1.0, 1.8], [0.0, 1.0, 2.5]);
    vantage("rise", [-44.0, 1.0, 12.0], [2.0, 0.0, 0.0]);
    vantage("above", [22.0, -22.0, 22.0], [0.0, 1.0, 0.0]);

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
    let local = start.local(middle);
    info!(
        "The village stands {:.0} m east and {:.0} m south of the start.",
        local.x, local.z
    );
}

/// Where the village goes, and which way its street runs east: of places 75 to 115 m from
/// the start round it, and a street turned every way there, the one whose buildings and
/// street stand on the most level ground, all of it dry and well away from the lair, so
/// it sits on the land rather than on tall footings down a hillside. The same on every
/// machine: the planet and the village's plan decide it.
fn site_for(
    planet: &Planet,
    start: &Start,
    lair: Vec3,
    buildings: &[terra_town::village::Building],
) -> Option<(Vec3, Vec3)> {
    let dry = |point: Vec3| point.length() > planet.sea_radius_m() + 1.5;
    let samples = reaches(buildings);
    let mut best: Option<(f32, Vec3, Vec3)> = None;
    for radius in [75.0_f32, 95.0, 115.0] {
        for step in 0..16 {
            let angle = step as f32 / 16.0 * std::f32::consts::TAU;
            let middle =
                planet.ground_at(start.at(radius * libm::cosf(angle), radius * libm::sinf(angle)));
            if !dry(middle) || middle.distance(lair) < 80.0 {
                continue;
            }
            let up = middle.normalize();
            let north = up.cross(Vec3::Y).normalize_or(up.any_orthonormal_vector());
            for turn in 0..8 {
                let east =
                    Quat::from_axis_angle(up, turn as f32 / 8.0 * std::f32::consts::PI) * north;
                let south = east.cross(up);
                let mut heights = Vec::with_capacity(samples.len());
                let mut wet = false;
                for &[e, s] in &samples {
                    let ground = planet.ground_at(middle + east * e + south * s);
                    wet |= !dry(ground);
                    heights.push(ground.length());
                }
                if wet {
                    continue;
                }
                let (low, high) = heights
                    .iter()
                    .fold((f32::MAX, f32::MIN), |(l, h), &x| (l.min(x), h.max(x)));
                let score = high - low;
                if best.as_ref().is_none_or(|(least, _, _)| score < *least) {
                    best = Some((score, middle, east));
                }
            }
        }
    }
    best.map(|(_, middle, east)| (middle, east))
}

/// Where the village reaches, in its own metres east and south: along the street, and at
/// each building's door, a few metres behind it and its corners.
fn reaches(buildings: &[terra_town::village::Building]) -> Vec<[f32; 2]> {
    let mut samples: Vec<[f32; 2]> = (-5..=5).map(|i| [i as f32 * 8.0, 0.0]).collect();
    for building in buildings {
        let [e, s] = building.at;
        let (sin, cos) = (libm::sinf(building.facing), libm::cosf(building.facing));
        // Its door, its back and its corners, as far as a house reaches.
        for (back, aside) in [
            (0.0, 0.0),
            (5.0, 0.0),
            (0.0, 4.5),
            (0.0, -4.5),
            (6.0, 4.5),
            (6.0, -4.5),
        ] {
            samples.push([e - sin * back + cos * aside, s - cos * back - sin * aside]);
        }
    }
    samples
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
            .map(|smith| smith.doing());
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
