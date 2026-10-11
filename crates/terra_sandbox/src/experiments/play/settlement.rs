//! A settlement (game design 001; D95): a few houses, a well and a stall, and the people
//! who live there. Their elder says what is wrong, that the spiders took the smith, and
//! where to look, by the sun and the glow rather than by a marker (D92). Others say what
//! they know. Bring the smith home to your claim and the elder says so.
//!
//! Everything in it is built from the same parts as the player's things (D108): each
//! house is a build put up by the same operations as a player's rooms, doors and windows,
//! and the well and stall are catalogue items, placed as a player places one. They are
//! the villagers', not the player's. The people go about their day where they stand.

use super::bag::Notice;
use super::catalogue;
use super::creatures::Lair;
use super::flora::ClearAround;
use super::interact::{Focus, Focusing, Interactable};
use super::people::{Arrive, Doing, Person};
use super::props::{self, Models, Theirs};
use super::scene::SUN_DIRECTION;
use crate::experiments::build::{self, Build, BuildLook, Built, Site};
use crate::planet::Planet;
use bevy::prelude::*;
use terra_build::{Base, BuildOp, CELL_M, Face, Panel, Side};

/// A house: where the middle of its door stands from the village's middle, in metres
/// east and south, and how wide and deep it is. Its door faces the well.
struct House {
    east: f32,
    south: f32,
    wide: f32,
    deep: f32,
    /// Whether it has a room upstairs as well.
    upstairs: bool,
}

const HOUSES: [House; 4] = [
    House {
        east: -6.0,
        south: -5.0,
        wide: 5.0,
        deep: 4.0,
        upstairs: false,
    },
    House {
        east: 6.5,
        south: -5.5,
        wide: 6.0,
        deep: 4.5,
        upstairs: true,
    },
    House {
        east: 0.0,
        south: -9.5,
        wide: 4.0,
        deep: 4.0,
        upstairs: false,
    },
    House {
        east: -8.5,
        south: 5.5,
        wide: 5.0,
        deep: 4.0,
        upstairs: false,
    },
];

/// An item from the catalogue the village has: what it is, where it stands from the
/// middle in metres east and south, and how many degrees it is turned.
const ITEMS: [(&str, f32, f32, f32); 2] = [
    ("../village/Well", 0.0, 0.0, 0.0),
    ("../village/MarketStand_1", 6.0, 4.0, -60.0),
];

/// The operations that put a house up, in its own space: the door in the wall along
/// z = 0, facing +Z, and the house behind it, its middle on x = 0.
fn house(house: &House) -> Vec<BuildOp> {
    let cells = |metres: f32| (metres / CELL_M).round() as i32;
    let (half, deep) = (cells(house.wide * 0.5), cells(house.deep));
    let mut ops = vec![BuildOp::AddRoom {
        min: [-half, 0, -deep],
        max: [half - 1, 0, -1],
    }];
    if house.upstairs {
        ops.push(BuildOp::AddRoom {
            min: [-half, 1, -deep],
            max: [half - 1, 1, -cells(1.5)],
        });
    }
    // A door a metre and a half wide, and a window in each end wall.
    for x in -1..2 {
        ops.push(BuildOp::SetPanel(
            Face::new(IVec3::new(x, 0, -1), Side::South),
            Panel::Door,
        ));
    }
    let middle = -deep / 2;
    for z in middle - 1..middle + 1 {
        ops.push(BuildOp::SetPanel(
            Face::new(IVec3::new(half - 1, 0, z), Side::East),
            Panel::WINDOW,
        ));
        ops.push(BuildOp::SetPanel(
            Face::new(IVec3::new(-half, 0, z), Side::West),
            Panel::WINDOW,
        ));
    }
    ops
}

#[derive(Resource)]
pub struct Village(pub Vec3);

pub fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (
            found.run_if(resource_added::<Lair>),
            talk.after(Focusing).run_if(resource_exists::<Village>),
        ),
    );
}

/// Once the lair is chosen, sets the village down somewhere else, dry and level enough,
/// and builds it.
#[allow(clippy::too_many_arguments)]
fn found(
    mut commands: Commands,
    site: Res<Site>,
    planet: Res<Planet>,
    lair: Res<Lair>,
    look: Res<BuildLook>,
    models: Res<Models>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut arrive: MessageWriter<Arrive>,
    mut clear: MessageWriter<ClearAround>,
) {
    let on_ground = |east: f32, south: f32| {
        planet.ground_at(site.frame.transform_point(Vec3::new(east, 0.0, south)))
    };
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
    clear.write(ClearAround(middle, 20.0));
    let up = middle.normalize();
    let east = up.cross(Vec3::Y).normalize_or(up.any_orthonormal_vector());
    let south = east.cross(up);

    let to_site = site.frame.compute_affine().inverse();
    // The houses, each its own build, standing upright where it is, its door to the well.
    for (index, plan) in HOUSES.iter().enumerate() {
        let door = planet.ground_at(middle + east * plan.east + south * plan.south);
        let up = door.normalize();
        let away = (door - middle).reject_from(up).normalize_or(south);
        let ops = house(plan);
        let mut shape = Base::default();
        for &op in &ops {
            shape.apply(op);
        }
        let frame = build::lifted(
            Transform::from_translation(door).looking_to(away, up),
            &shape,
            &planet,
        );
        let mut built = Build::new(&mut commands, &look, frame, &format!("House {index}"));
        built.replace_with(&ops, &mut commands, &planet, &mut meshes);
        commands.spawn((Name::new(format!("House {index}")), Built(built)));
        let local = to_site.transform_point3(door);
        info!(
            "House {index}'s door is {:.1} m east and {:.1} m south of the start.",
            local.x, local.z
        );
    }

    // The well and the stall, placed as a player places an item, but the villagers'.
    for (file, item_east, item_south, turn) in ITEMS {
        let Some(item) = catalogue::item_from(file) else {
            error!("The village wants {file}, which is not in the catalogue.");
            continue;
        };
        let at = to_site.transform_point3(middle + east * item_east + south * item_south);
        if let Some(entity) = props::place(
            &mut commands,
            &models,
            &site,
            &planet,
            item,
            at.with_y(0.0),
            0.0,
            turn,
            false,
        ) {
            commands.entity(entity).insert(Theirs);
        }
    }

    // The people: the elder by the well, and others about their day.
    let place = |east_m: f32, south_m: f32| middle + east * east_m + south * south_m;
    let people = [
        (
            "Maren",
            place(1.6, 1.4),
            Doing::Standing,
            Color::srgb(0.32, 0.42, 0.55),
            Some(Interactable::Talk),
            None,
        ),
        (
            "Tobin",
            place(5.0, 2.6),
            Doing::Standing,
            Color::srgb(0.55, 0.42, 0.24),
            Some(Interactable::Talk),
            Some("Tobin: \"Ore rock has a rust to it. A stone pick will break it, slowly.\""),
        ),
        (
            "Ilse",
            place(-3.5, 2.5),
            Doing::Sitting,
            Color::srgb(0.46, 0.5, 0.3),
            Some(Interactable::Talk),
            Some("Ilse: \"The deer graze in the open. Come at them quietly, or they're gone.\""),
        ),
        (
            "Wyn",
            place(-2.0, -3.0),
            Doing::Standing,
            Color::srgb(0.6, 0.3, 0.28),
            Some(Interactable::Talk),
            Some("Wyn: \"Meat keeps you longer cooked. Any fire will do.\""),
        ),
    ];
    for (name, at, doing, colour, interact, says) in people {
        arrive.write(Arrive {
            name,
            at,
            facing: (middle - at).reject_from(up),
            doing,
            smith: false,
            colour,
            interact,
            says,
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

/// Which way a place lies from another, as someone there would say it: by the sun.
fn by_the_sun(from: Vec3, to: Vec3) -> &'static str {
    let up = from.normalize();
    let way = (to - from).reject_from(up).normalize_or_zero();
    let sun = SUN_DIRECTION.reject_from(up).normalize_or_zero();
    let toward = way.dot(sun);
    let side = way.dot(up.cross(sun));
    if toward > 0.6 {
        "toward the low sun"
    } else if toward < -0.6 {
        "with the sun at your back"
    } else if side > 0.0 {
        "with the sun on your right"
    } else {
        "with the sun on your left"
    }
}

/// F at someone in the village: they say what they know.
fn talk(
    focus: Res<Focus>,
    village: Res<Village>,
    lair: Res<Lair>,
    people: Query<&Person>,
    mut notice: ResMut<Notice>,
) {
    if !focus.used(Interactable::Talk) {
        return;
    }
    let Some(person) = focus.entity.and_then(|entity| people.get(entity).ok()) else {
        return;
    };
    if let Some(line) = person.says {
        notice.say(line);
        return;
    }
    // The elder speaks of the smith.
    let smith = people
        .iter()
        .find(|person| person.smith)
        .map(|smith| smith.doing);
    let line = match smith {
        Some(Doing::Bound) | None => format!(
            "{}: \"Spiders took Bren, our smith. They nest where the light comes out of the ground, {}. You'll see the glow.\"",
            person.name,
            by_the_sun(village.0, lair.0)
        ),
        Some(Doing::Following) => format!(
            "{}: \"You found Bren! Take them somewhere safe; they'll work a forge for you.\"",
            person.name
        ),
        Some(_) => format!(
            "{}: \"Bren's working your forge, I hear. You've done right by us.\"",
            person.name
        ),
    };
    notice.say(line);
}
