//! Meetings a film asks for (D384): a run's shot file may list them (`meetings`), each a kind,
//! what it is doing, when and where it is seen, so a film shows the animal it is made for
//! rather than whatever its walk happens to meet. Each is put out unseen, out of the picture
//! or behind the trees, timed by its pace to be at its spot when asked, and goes in from there
//! on its own feet, round the trunks as every creature does; the film's measure says whether
//! it was seen (`play::sighted`, the trailer's "meeting" line).

use super::*;
use crate::play::species::{Moves, SpeciesDef};
use serde::Deserialize;
use terra_world::cine::Cine;

/// What a meeting asked for is doing as it comes into sight.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Behaviour {
    /// Walking calmly across the way ahead, from one side into the trees on the other.
    Crossing,
    /// Walking in to its spot and about its day there, grazing or nosing about.
    Grazing,
    /// Running off, startled, across the picture and away.
    Fleeing,
}

/// One meeting a shot asks for, as its file has it.
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct Meeting {
    /// The kind, by its name in creatures.ron or a word of it no other kind has ("stag").
    pub kind: String,
    pub behaviour: Behaviour,
    /// When it is at its spot, seconds of the shot's clock (from when the run sets off; the
    /// trailer's film starts at its `lead_in`, 1.5 s).
    pub at: f32,
    /// Its spot then: so far ahead of the camera, metres, and so many degrees off its view
    /// axis, to the right (to the left below nothing).
    pub ahead: f32,
    #[serde(default)]
    pub bearing: f32,
    /// How many of it (one by default).
    #[serde(default)]
    pub group: Option<u32>,
}

/// Marks an animal put out for the meeting a shot asked for, by its place in the list.
#[derive(Component, Clone, Copy, Debug)]
pub struct Asked(pub usize);

/// The shot's meetings, their kinds, and which are staged or given up.
#[derive(Resource, Default)]
pub(super) struct Asking {
    read: bool,
    meetings: Vec<(Meeting, Species)>,
    done: Vec<bool>,
    /// Why the last try at each failed.
    why: Vec<String>,
    /// The player's feet at the last look, and how fast they go.
    last: Option<Vec3>,
    speed: f32,
    since_s: f32,
}

/// A meeting is staged from so long before it is asked to be seen, at the first look a start
/// out of sight fits, and given up if none has by so long before.
const LEAD_S: (f32, f32) = (1.5, 10.0);
/// A crossing walks on so long past its spot, a fleeing one runs on so long.
const ON_S: f32 = 6.0;
/// Ways tried from a spot to start from, degrees from straight out to the side it is on
/// (away from the view axis) toward the way the camera looks; below nothing, toward the
/// camera, so it comes from beside the picture's edge.
const CROSSING_FROM: [f32; 5] = [0.0, -30.0, 30.0, -55.0, 55.0];
const FLEEING_FROM: [f32; 4] = [-40.0, -20.0, -60.0, 0.0];
const GRAZING_FROM: [f32; 5] = [0.0, 40.0, -30.0, 80.0, -60.0];
/// How far the spot is moved, ahead and round, metres and degrees, when the asked one is in
/// water, a trunk or hidden from the camera: the nearest first.
const NUDGE_AHEAD_M: [f32; 7] = [0.0, 3.0, -3.0, 6.0, -6.0, 10.0, -10.0];
const NUDGE_ROUND_DEG: [f32; 5] = [0.0, 4.0, -4.0, 8.0, -8.0];

fn nudges() -> Vec<(f32, f32)> {
    let mut all: Vec<(f32, f32)> = NUDGE_AHEAD_M
        .iter()
        .flat_map(|&m| NUDGE_ROUND_DEG.iter().map(move |&d| (m, d)))
        .collect();
    all.sort_by(|a, b| {
        (a.0.abs() / 3.0 + a.1.abs() / 4.0).total_cmp(&(b.0.abs() / 3.0 + b.1.abs() / 4.0))
    });
    all
}
/// The steepest ground a walker is put on or sent across, rise over run.
const STEEPEST: f32 = 0.8;

/// A meeting placed: its spot, where it starts, the way it goes from there and how long
/// until it reaches its spot, seconds.
#[derive(Clone, Copy, Debug)]
pub(super) struct Staged {
    pub spot: Vec3,
    pub start: Vec3,
    pub way: Vec3,
    pub lead_s: f32,
}

/// The kind a meeting names: by its name, or a word of one no other kind's has.
pub(super) fn kind_named(bestiary: &Bestiary, kind: &str) -> Result<Species, String> {
    let wanted = kind.to_lowercase();
    if let Some(species) = bestiary
        .all()
        .find(|&each| bestiary.name(each).to_lowercase() == wanted)
    {
        return Ok(species);
    }
    let with: Vec<Species> = bestiary
        .all()
        .filter(|&each| {
            bestiary
                .name(each)
                .to_lowercase()
                .split(' ')
                .any(|word| word == wanted)
        })
        .collect();
    match with.as_slice() {
        [one] => Ok(*one),
        [] => Err(format!("no kind is called `{kind}`")),
        many => Err(format!(
            "`{kind}` is any of {}: name one",
            many.iter()
                .map(|&each| bestiary.name(each))
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

/// The meetings a shot's file lists, with their kinds.
pub(super) fn read(
    more: &serde_json::Map<String, serde_json::Value>,
    bestiary: &Bestiary,
) -> Result<Vec<(Meeting, Species)>, String> {
    let Some(list) = more.get("meetings") else {
        return Ok(Vec::new());
    };
    let meetings: Vec<Meeting> =
        serde_json::from_value(list.clone()).map_err(|e| format!("meetings: {e}"))?;
    meetings
        .into_iter()
        .map(|meeting| {
            let species = kind_named(bestiary, &meeting.kind)?;
            if matches!(bestiary.get(species).moves, Moves::Swims(_)) {
                return Err(format!("meetings: {} swims", meeting.kind));
            }
            Ok((meeting, species))
        })
        .collect()
}

/// Where a player now at a point is once they have gone so far on along a route, and the
/// way they go there: on from the nearest point of its nearest leg; at its end they stand.
pub(super) fn on_route(route: &[Vec3], here: Vec3, metres: f32) -> Option<(Vec3, Vec3)> {
    if route.len() < 2 {
        return None;
    }
    let nearest = |a: Vec3, b: Vec3| {
        let s = ((here - a).dot(b - a) / (b - a).length_squared().max(1e-6)).clamp(0.0, 1.0);
        a.lerp(b, s)
    };
    let (mut leg, mut at) = (0..route.len() - 1)
        .map(|leg| (leg, nearest(route[leg], route[leg + 1])))
        .min_by(|a, b| a.1.distance(here).total_cmp(&b.1.distance(here)))?;
    let mut left = metres.max(0.0);
    loop {
        let end = route[leg + 1];
        let way = (end - route[leg]).normalize_or_zero();
        let to_end = at.distance(end);
        if left <= to_end || leg + 2 >= route.len() {
            return Some((at + way * left.min(to_end), way));
        }
        left -= to_end;
        (at, leg) = (end, leg + 1);
    }
}

/// How fast one of a kind goes into sight for a meeting, metres a second.
fn pace_mps(about: &SpeciesDef, behaviour: Behaviour) -> f32 {
    let pace = match behaviour {
        Behaviour::Crossing | Behaviour::Grazing => creatures::Pace::Calm,
        Behaviour::Fleeing => creatures::Pace::Startled,
    };
    match (about.moves, behaviour) {
        (Moves::Flies(mps), _) => pace.flies_mps(mps),
        (_, Behaviour::Grazing) => creatures::keeps_to(about.walk_mps, about),
        _ => pace.runs_mps(about),
    }
}

/// Where a meeting is put out, `lead_s` before it is asked to be at its spot, by a player
/// at `feet` going `ahead` there then: its spot so far ahead of the camera and off its axis
/// as asked, moved a little if that is in water, a trunk or hidden from the camera; and a
/// start out of sight now, from which its pace takes it there in that time, on dry ground
/// not too steep all the way. Why not, if nowhere fits.
pub(super) fn stage(
    around: &Surroundings,
    meeting: &Meeting,
    about: &SpeciesDef,
    (feet, ahead): (Vec3, Vec3),
    lead_s: f32,
) -> Result<Staged, String> {
    let up = feet.normalize_or(Vec3::Y);
    let ahead = ahead.reject_from(up).normalize_or_zero();
    if ahead == Vec3::ZERO {
        return Err("the camera's way is unknown".into());
    }
    let right = ahead.cross(up);
    let eye = eye_at(around, feet, ahead);
    let flies = matches!(about.moves, Moves::Flies(_));
    let lift = up * 0.4;
    let radius = about.body.radius;
    let dry = |p: Vec3| flies || around.depth_m(p) < -0.1;
    // Clear of every trunk and solid, across the ground.
    let apart = |p: Vec3, foot: Vec3| (p - foot).reject_from(up).length();
    let clear = |p: Vec3| {
        around
            .trunks(p, radius + 2.0)
            .all(|trunk| apart(p, trunk.foot) > trunk.radius_m + radius + 0.3)
            && around
                .obstacles
                .near(p, radius + 2.0)
                .all(|(foot, round)| apart(p, foot) > round + radius + 0.3)
    };
    // Why each spot tried did not fit: in water, in a trunk, hidden from the camera.
    let mut not = [0; 3];
    let spot = nudges().into_iter().find_map(|(more_m, more_deg)| {
        let off = (meeting.bearing + more_deg).to_radians();
        let dir = ahead * libm::cosf(off) + right * libm::sinf(off);
        let spot = around.ground(eye + dir * (meeting.ahead + more_m).max(4.0));
        let fault = if !dry(spot) {
            0
        } else if !clear(spot) {
            1
        } else if !shows_from(around, eye, spot + lift) {
            2
        } else {
            return Some(spot);
        };
        not[fault] += 1;
        None
    });
    let Some(spot) = spot else {
        return Err(format!(
            "of its spots {} in water, {} in a trunk, {} hidden from the camera",
            not[0], not[1], not[2]
        ));
    };
    // Out to the side it is on, away from the view axis.
    let side = (spot - eye).reject_from(up).dot(right);
    let outward = if side >= 0.0 { right } else { -right };
    let mps = pace_mps(about, meeting.behaviour);
    let from = match meeting.behaviour {
        Behaviour::Crossing => &CROSSING_FROM[..],
        Behaviour::Fleeing => &FLEEING_FROM[..],
        Behaviour::Grazing => &GRAZING_FROM[..],
    };
    let mut not = [0; 3];
    for &degrees in from {
        let (sin, cos) = (
            libm::sinf(degrees.to_radians()),
            libm::cosf(degrees.to_radians()),
        );
        let dir = outward * cos + ahead * sin;
        let start = around.ground(spot + dir * mps * lead_s);
        let fault = if !dry(start) || !clear(start) {
            0
        } else if around.seen_now(start + lift, about.seen_m()) {
            1
        } else if !flies && !walkable(around, start, spot) {
            2
        } else {
            3
        };
        if fault < 3 {
            not[fault] += 1;
            continue;
        }
        let way = (spot - start).reject_from(up).normalize_or_zero();
        return Ok(Staged {
            spot,
            start,
            way,
            lead_s,
        });
    }
    Err(format!(
        "of its starts {} in water or a trunk, {} in sight, {} with no dry, gentle way in",
        not[0], not[1], not[2]
    ))
}

/// Whether a walker can go straight from one point to another: dry ground, never steeper than
/// `STEEPEST`, every 2 m of the way.
fn walkable(around: &Surroundings, from: Vec3, to: Vec3) -> bool {
    let steps = (from.distance(to) / 2.0).ceil().max(1.0) as usize;
    let mut last = around.ground(from);
    (1..=steps).all(|i| {
        let at = around.ground(from.lerp(to, i as f32 / steps as f32));
        let rise = (at.length() - last.length()).abs();
        let run = at.distance(last).max(0.1);
        last = at;
        around.depth_m(at) < -0.1 && rise / run <= STEEPEST
    })
}

/// Stages each meeting the shot asks for once its time comes: where its start is out of
/// sight, put out there and sent on, as asked; told in the log either way.
#[allow(clippy::too_many_arguments)]
pub(super) fn ask(
    mut commands: Commands,
    time: Res<Time>,
    cine: Option<Res<Cine>>,
    mut asking: ResMut<Asking>,
    mut cells: ResMut<Cells>,
    (kinds, bestiary): (Option<Res<Kinds>>, Res<Bestiary>),
    around: Surroundings,
    options: Res<crate::shell::Options>,
    viewers: Query<&Transform, With<Viewpoint>>,
) {
    let Some(cine) = cine else { return };
    if !asking.read {
        asking.read = true;
        asking.meetings =
            read(&cine.shot.more, &bestiary).unwrap_or_else(|why| panic!("--cine: {why}"));
        asking.done = vec![false; asking.meetings.len()];
        asking.why = vec![String::new(); asking.meetings.len()];
    }
    let (Some(t), Some(kinds)) = (cine.t, kinds.filter(|kinds| kinds.ready())) else {
        return;
    };
    if asking.done.iter().all(|&done| done) {
        return;
    }
    asking.since_s += time.delta_secs();
    if asking.since_s < LOOK_EVERY_S {
        return;
    }
    let dt = std::mem::take(&mut asking.since_s);
    let Some(feet) = viewers.iter().next().map(|at| at.translation) else {
        return;
    };
    if let Some(last) = asking.last {
        asking.speed = (feet - last).reject_from(feet.normalize()).length() / dt;
    }
    asking.last = Some(feet);
    let facing = viewers
        .iter()
        .next()
        .map_or(Vec3::ZERO, |at| at.forward().as_vec3());
    let route: Vec<Vec3> = options
        .route
        .iter()
        .map(|&(east, south)| around.start.at(east, south))
        .collect();
    for index in 0..asking.meetings.len() {
        let (meeting, species) = asking.meetings[index].clone();
        let lead = meeting.at - t;
        if asking.done[index] || lead > LEAD_S.1 {
            continue;
        }
        let name = bestiary.name(species).to_string();
        if lead < LEAD_S.0 {
            asking.done[index] = true;
            info!(
                "Wildlife: meeting {index} ({name}) not staged: {}.",
                asking.why[index]
            );
            continue;
        }
        let about = bestiary.get(species);
        // Where the player will be when it is asked to be seen, on along their route.
        let then = on_route(&route, feet, asking.speed * lead).unwrap_or((feet, facing));
        let staged = match stage(&around, &meeting, about, then, lead) {
            Ok(staged) => staged,
            Err(why) => {
                asking.why[index] = why;
                continue;
            }
        };
        asking.done[index] = true;
        let put = put_out(
            &mut commands,
            (&kinds, &bestiary),
            &around,
            species,
            &meeting,
            staged,
        );
        for &animal in &put {
            commands.entity(animal).insert(Asked(index));
        }
        let local = around.start.local(staged.start);
        info!(
            "Wildlife: meeting {index} ({name}, {:?}) staged at {t:.1} s: {} from {:.0},{:.0}, \
             {:.0} m from its spot, there at {:.1} s.",
            meeting.behaviour,
            put.len(),
            local.x,
            local.z,
            staged.start.distance(staged.spot),
            meeting.at
        );
        cells.met.extend(put);
    }
}

/// Puts out a meeting's animals at its start and sends them on as it asked.
fn put_out(
    commands: &mut Commands,
    (kinds, bestiary): (&Kinds, &Bestiary),
    around: &Surroundings,
    species: Species,
    meeting: &Meeting,
    staged: Staged,
) -> Vec<Entity> {
    let up = staged.start.normalize_or(Vec3::Y);
    let across = staged.way.cross(up);
    let mut put = Vec::new();
    for member in 0..meeting.group.unwrap_or(1).max(1) {
        // The others a little to either side and behind the first.
        let off = member as f32;
        let side = if member % 2 == 0 { 1.0 } else { -1.0 };
        let from = around
            .ground(staged.start + across * side * off.min(1.0) * 1.4 - staged.way * off * 1.2);
        let Some(animal) =
            creatures::spawn(commands, (kinds, bestiary), &around.planet, species, from)
        else {
            continue;
        };
        let (behaviour, way, lead) = (meeting.behaviour, staged.way, staged.lead_s);
        let to = staged.spot + (from - staged.start);
        let flies = matches!(bestiary.get(species).moves, Moves::Flies(_));
        commands
            .entity(animal)
            .entry::<creatures::Creature>()
            .and_modify(move |mut animal| match behaviour {
                _ if flies => animal.fly(from, to + way * 15.0),
                Behaviour::Grazing => animal.walk_to(to),
                Behaviour::Crossing => animal.dash(way, lead + ON_S, creatures::Pace::Calm),
                Behaviour::Fleeing => animal.dash(way, lead + ON_S, creatures::Pace::Startled),
            });
        put.push(animal);
    }
    put
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_player_on_a_route_is_found_on_along_it_and_stands_at_its_end() {
        let route = [Vec3::ZERO, Vec3::X * 10.0, Vec3::new(10.0, 0.0, 10.0)];
        let (at, way) = on_route(&route, Vec3::new(4.0, 0.0, 1.0), 3.0).unwrap();
        assert!(
            at.distance(Vec3::X * 7.0) < 1e-4 && way == Vec3::X,
            "{at} {way}"
        );
        // Round the corner.
        let (at, way) = on_route(&route, Vec3::new(4.0, 0.0, 1.0), 10.0).unwrap();
        assert!(
            at.distance(Vec3::new(10.0, 0.0, 4.0)) < 1e-4 && way == Vec3::Z,
            "{at}"
        );
        // Past its end.
        let (at, _) = on_route(&route, Vec3::new(4.0, 0.0, 1.0), 100.0).unwrap();
        assert!(at.distance(Vec3::new(10.0, 0.0, 10.0)) < 1e-4, "{at}");
    }

    #[test]
    fn a_kind_is_named_by_its_name_or_a_word_only_it_has() {
        let bestiary = Bestiary::read(&Default::default());
        let stag = bestiary.named("Red deer stag").unwrap();
        assert_eq!(kind_named(&bestiary, "stag"), Ok(stag));
        assert_eq!(kind_named(&bestiary, "red deer STAG"), Ok(stag));
        assert!(
            kind_named(&bestiary, "fox")
                .unwrap_err()
                .contains("Red fox")
        );
        assert!(kind_named(&bestiary, "unicorn").is_err());
        let more = serde_json::json!({ "meetings": [
            { "kind": "stag", "behaviour": "crossing", "at": 12, "ahead": 30, "bearing": 20 },
            { "kind": "Red fox", "behaviour": "fleeing", "at": 8, "ahead": 25, "group": 2 },
        ]});
        let read = read(more.as_object().unwrap(), &bestiary).unwrap();
        assert_eq!(read.len(), 2);
        assert_eq!(
            (read[0].1, read[0].0.behaviour),
            (stag, Behaviour::Crossing)
        );
        assert_eq!(read[1].0.group, Some(2));
        let wrong = serde_json::json!({ "meetings": [{ "kind": "stag", "behaviour": "dancing", "at": 1, "ahead": 9 }] });
        assert!(super::read(wrong.as_object().unwrap(), &bestiary).is_err());
    }

    #[test]
    fn an_asked_meeting_starts_out_of_sight_and_reaches_its_spot_in_view_on_time() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        let planet = Planet::smooth(terra_core::PlanetSpec {
            seed: Seed(1),
            radius_m: 2000.0,
        });
        world.insert_resource(Start::choose(&planet, true));
        let feet = planet.ground_at(Vec3::Y * 2000.0);
        let up = feet.normalize();
        let ahead = Vec3::X;
        let ground = |at: Vec3| planet.ground_at(at);
        let right = ahead.cross(up);
        // The camera now, 8 s of a jog back from where the player will be.
        let now = feet - ahead * 4.3 * 8.0;
        let eye = now + up * EYE_UP_M - ahead * EYE_BEHIND_M;
        world.spawn((
            MainCamera,
            GlobalTransform::from(Transform::from_translation(eye).looking_to(ahead, up)),
            Projection::Perspective(PerspectiveProjection::default()),
        ));
        // A trunk just where the stag is asked to be, and a row of them beside the way that
        // hides what is behind them from the camera now.
        let then_eye = feet + up * EYE_UP_M - ahead * EYE_BEHIND_M;
        let off = 20f32.to_radians();
        let asked_at =
            ground(then_eye + (ahead * libm::cosf(off) + right * libm::sinf(off)) * 30.0);
        let row: Vec<Vec3> = (0..30)
            .map(|i| ground(feet + right * 16.0 + ahead * (i as f32 * 1.5 - 5.0)))
            .collect();
        world.insert_resource(planet);
        world.init_resource::<Trunks>();
        world.init_resource::<Obstacles>();
        world.init_resource::<Warmth>();
        world.init_resource::<Perches>();
        world.init_resource::<Thickets>();
        world.init_resource::<Clock>();
        world.resource_mut::<Obstacles>().add(asked_at, 0.5);
        for at in row {
            world.resource_mut::<Obstacles>().add(at, 0.6);
        }
        let bestiary = Bestiary::read(&Default::default());
        let stag = bestiary.named("Red deer stag").unwrap();
        let meeting = Meeting {
            kind: "stag".into(),
            behaviour: Behaviour::Crossing,
            at: 12.0,
            ahead: 30.0,
            bearing: 20.0,
            group: None,
        };
        let mps = pace_mps(bestiary.get(stag), Behaviour::Crossing);
        let (staged, checks) = world
            .run_system_once(move |around: Surroundings| {
                let about = bestiary.get(stag);
                let staged = stage(&around, &meeting, about, (feet, ahead), 8.0).unwrap();
                let in_trunk = around
                    .trunks(staged.spot, 3.0)
                    .any(|trunk| trunk.foot.distance(staged.spot) < trunk.radius_m + 0.3);
                (
                    staged,
                    [
                        // Unseen where it is put out, now.
                        around.seen_now(staged.start + up * 0.4, f32::INFINITY),
                        // Seen at its spot from where the camera will be.
                        shows_from(&around, then_eye, staged.spot + up * 0.4),
                        in_trunk,
                        walkable(&around, staged.start, staged.spot),
                    ],
                )
            })
            .unwrap();
        assert_eq!(
            checks,
            [false, true, false, true],
            "unseen at its start; seen at its spot; not in a trunk; walkable in"
        );
        // Moved off the trunk it was asked onto, by a few metres at most.
        let moved = staged.spot.distance(asked_at);
        assert!((0.5..9.0).contains(&moved), "{moved}");
        // Its pace takes it to its spot in the time asked, heading there.
        let takes = staged.start.distance(staged.spot) / mps;
        assert!((takes - 8.0).abs() < 0.3, "{takes}");
        assert!(staged.way.dot((staged.spot - staged.start).normalize()) > 0.99);
    }
}
