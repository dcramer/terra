//! A settlement (game design 001; D95): a few houses, a well and a stall, and the people
//! who live there. Their elder says what is wrong, that the spiders took the smith, and
//! where to look, by the sun and the glow rather than by a marker (D92). Others say what
//! they know. Bring the smith home to your claim and the elder says so.
//!
//! The houses are solid; the people go about their day where they stand.

use super::bag::Notice;
use super::creatures::Lair;
use super::flora::Obstacles;
use super::interact::{Focus, Focusing, Interactable};
use super::people::{Arrive, Doing, Person};
use super::scene::SUN_DIRECTION;
use crate::experiments::build::Site;
use crate::planet::Planet;
use bevy::prelude::*;

/// One building or piece of the village: its model, where it stands from the middle,
/// how it is turned, how much bigger than its file it is drawn, and how far it keeps a
/// walker off, if it does.
struct Piece {
    file: &'static str,
    east: f32,
    south: f32,
    turn_degrees: f32,
    scale: f32,
    solid_m: Option<f32>,
}

const fn piece(
    file: &'static str,
    east: f32,
    south: f32,
    turn: f32,
    scale: f32,
    solid: f32,
) -> Piece {
    Piece {
        file,
        east,
        south,
        turn_degrees: turn,
        scale,
        solid_m: if solid > 0.0 { Some(solid) } else { None },
    }
}

/// The village, laid out around its well.
const PIECES: [Piece; 6] = [
    piece("models/village/House_1.glb", -7.0, -5.0, 30.0, 1.9, 3.0),
    piece("models/village/House_2.glb", 7.0, -6.0, -25.0, 1.9, 3.6),
    piece("models/village/House_3.glb", 0.0, -11.0, 0.0, 1.9, 2.4),
    piece("models/village/House_1.glb", -9.0, 6.0, 140.0, 1.9, 3.0),
    piece("models/village/Well.glb", 0.0, 0.0, 0.0, 1.6, 0.9),
    piece(
        "models/village/MarketStand_1.glb",
        6.0,
        4.0,
        -60.0,
        1.8,
        1.0,
    ),
];

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

/// Once the lair is chosen, sets the village down somewhere else, dry and level enough.
fn found(
    mut commands: Commands,
    site: Res<Site>,
    planet: Res<Planet>,
    lair: Res<Lair>,
    assets: Res<AssetServer>,
    mut obstacles: ResMut<Obstacles>,
    mut arrive: MessageWriter<Arrive>,
) {
    let on_ground = |east: f32, south: f32| {
        planet.drawn(
            site.frame
                .transform_point(Vec3::new(east, 0.0, south))
                .normalize(),
        )
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
    let up = middle.normalize();
    let east = up.cross(Vec3::Y).normalize_or(up.any_orthonormal_vector());
    let south = east.cross(up);
    for piece in &PIECES {
        let at = planet.drawn((middle + east * piece.east + south * piece.south).normalize());
        let up = at.normalize();
        let turn = Quat::from_axis_angle(up, piece.turn_degrees.to_radians());
        commands.spawn((
            Name::new(piece.file),
            WorldAssetRoot(assets.load(GltfAssetLabel::Scene(0).from_asset(piece.file))),
            Transform::from_translation(at)
                .looking_to(turn * south, up)
                .with_scale(Vec3::splat(piece.scale)),
        ));
        if let Some(solid) = piece.solid_m {
            obstacles.0.push((at, solid));
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
    info!("The village stands at {middle}.");
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
            "{}: \"Bren's working your smithy, I hear. You've done right by us.\"",
            person.name
        ),
    };
    notice.say(line);
}
