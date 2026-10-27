use super::super::tests::kind;
use super::super::{Frame, LightKind, MID, ObjectRole, Prop, Stamp, Theme, floor_of, stamp};
use super::{HERO_LIT_M, OFF_DOOR_M};
use crate::place::Side;
use crate::theme::{Dress, assets};
use crate::{ModuleSet, generate};
use glam::{Vec2, Vec3};
use terra_gen::Seed;

fn has(list: &[(String, f32)], p: &Prop) -> bool {
    list.iter().any(|(m, _)| *m == p.model)
}

/// Every room of a dressed role gets its type's set (D539): a hero prop of its type, or a
/// feature standing as one (an object, the rest's fire, the goal's bowls, a tomb), and wall
/// props and clutter of its set; the props a theme's dressing laid are those of its stamp not
/// in the stamp undressed, and none stands in the eye's path or a doorway, a hero's ends
/// included.
#[test]
fn rooms_are_dressed_as_their_type_round_a_hero_off_the_way() {
    for theme in Theme::all(&assets()).unwrap() {
        if theme.dressing.is_empty() {
            continue;
        }
        let bare_theme = Theme {
            dressing: Vec::new(),
            ..theme.clone()
        };
        let set = ModuleSet::new(&theme).unwrap();
        let (mut rooms, mut heroes, mut sets, mut props) = (0, 0, 0, 0);
        let (mut nearest, mut ends, mut door) = (f32::MAX, f32::MAX, f32::MAX);
        for s in 0..40 {
            let Ok(dungeon) = generate(&theme, &set, Seed(s)) else {
                continue;
            };
            let full = stamp(&dungeon, &theme, &kind);
            let bare = stamp(&dungeon, &bare_theme, &kind);
            assert_eq!(full.route, bare.route, "{} seed {s}: one tour", theme.name);
            let laid = &full.props[bare.props.len()..];
            props += laid.len();
            let flat = |v: Vec3| Vec2::new(v.x, v.z);
            // The eye's path, not the floor's route the dressing kept off.
            let eye = |p: Vec3| {
                (full.view.iter())
                    .filter(|v| (v.y - p.y).abs() < 3.0)
                    .map(|v| flat(*v).distance(flat(p)))
                    .fold(f32::MAX, f32::min)
            };
            let layout = &dungeon.layout;
            let doors: Vec<Vec3> = (layout.cells.iter())
                .flat_map(|cell| {
                    (0..4)
                        .filter(|&d| matches!(cell.sides[d], Side::Door(_) | Side::Way))
                        .map(|d| Frame::new(cell.at, d).point(MID, 0.0, 0.0))
                })
                .collect();
            let heroic: Vec<&(String, f32)> = theme.dressing.iter().flat_map(|d| &d.hero).collect();
            for p in laid {
                nearest = nearest.min(eye(p.at));
                let level = doors.iter().filter(|d| (d.y - p.at.y).abs() < 1.0);
                door = level.map(|d| d.distance(p.at)).fold(door, f32::min);
                if heroic.iter().any(|(m, _)| *m == p.model) {
                    let along = if p.facing % 2 == 0 { Vec3::X } else { Vec3::Z };
                    for end in [p.at - along, p.at + along] {
                        ends = ends.min(eye(end));
                    }
                }
            }
            for (r, room) in dungeon.graph.rooms.iter().enumerate() {
                let dresses: Vec<&Dress> = (theme.dressing.iter())
                    .filter(|d| d.roles.contains(&room.role))
                    .collect();
                if dresses.is_empty() {
                    continue;
                }
                rooms += 1;
                let rect = layout.rooms[r];
                let lo = rect.min.as_vec3() * 4.0;
                let hi = lo + Vec3::new(rect.size.x as f32, 0.0, rect.size.y as f32) * 4.0;
                let floor = floor_of(rect.min.y) as f32 * super::super::CUBE;
                let inside = |p: Vec3| {
                    (p.y - floor).abs() < 1.0
                        && p.x > lo.x
                        && p.x < hi.x
                        && p.z > lo.z
                        && p.z < hi.z
                };
                let here: Vec<&Prop> = laid.iter().filter(|p| inside(p.at)).collect();
                // What stands raised (on a plinth, a fire's bowl, a tomb's kit piece) by its
                // place over the floor.
                let over = |p: Vec3| (p.y - floor) > -0.5 && (p.y - floor) < 3.0;
                let within = |p: Vec3| over(p) && inside(p.with_y(floor));
                let feature = (bare.objects.iter())
                    .any(|o| o.role != ObjectRole::Door && within(o.at))
                    || (bare.lights.iter()).any(|l| l.kind != LightKind::Torch && within(l.at))
                    || (bare.props.iter().map(|p| (&p.model, p.at)))
                        .chain(bare.pieces.iter().map(|p| (&p.model, p.at)))
                        .any(|(m, at)| m.contains("Sarcophagus") && within(at));
                let hero = here.iter().any(|p| dresses.iter().any(|d| has(&d.hero, p)));
                heroes += (hero || feature) as u32;
                let full_set = dresses.iter().all(|d| {
                    (d.props.is_empty() || here.iter().any(|p| has(&d.props, p)))
                        && (d.clutter.is_empty() || here.iter().any(|p| has(&d.clutter, p)))
                });
                sets += full_set as u32;
            }
        }
        println!(
            "{}: {props} props; {heroes} of {rooms} rooms with a hero, {sets} with their set; \
             the nearest {nearest:.2} m from the eye's path, a hero's end {ends:.2}, a doorway {door:.2}",
            theme.name
        );
        assert_eq!(heroes, rooms, "{}: a hero a room", theme.name);
        // A room all niches, rubble and racks has no bare wall for its wall props: 3 in 100
        // over these seeds (2026-10-27).
        assert!(
            sets * 100 >= rooms * 95,
            "{}: rooms with their set",
            theme.name
        );
        assert!(
            nearest > 0.9,
            "{}: a prop {nearest:.2} m from the eye",
            theme.name
        );
        assert!(
            ends > 0.6,
            "{}: a hero's end {ends:.2} m from the eye",
            theme.name
        );
        assert!(
            door > OFF_DOOR_M,
            "{}: a prop {door:.2} m from a doorway",
            theme.name
        );
    }
}

/// The heroes of a theme's stamp, by their dressing's (model, scale): a treasury's hero chest
/// is larger than its wall props' and its vignette's.
fn heroes(theme: &Theme, stamp: &Stamp) -> Vec<Vec3> {
    let heroic: Vec<&(String, f32)> = theme.dressing.iter().flat_map(|d| &d.hero).collect();
    (stamp.props.iter())
        .filter(|p| heroic.iter().any(|(m, s)| *m == p.model && *s == p.scale))
        .map(|p| p.at)
        .collect()
}

/// Every room's hero has a light by it (barrow dress round 3): a torch on its wall or its own
/// lamp, within `HERO_LIT_M`. Without the lamps the crypt's robbed tombs and passages were lit
/// only from across the room, near black in the dressed rooms' sheet.
#[test]
fn every_hero_has_a_light_by_it() {
    for theme in Theme::all(&assets()).unwrap() {
        if !theme.dressing.iter().any(|d| d.lamp.is_some()) {
            continue;
        }
        let unlamped = Theme {
            dressing: (theme.dressing.iter().cloned())
                .map(|d| Dress { lamp: None, ..d })
                .collect(),
            ..theme.clone()
        };
        let set = ModuleSet::new(&theme).unwrap();
        let lit = |s: &Stamp| {
            let all = heroes(&theme, s);
            let by = |h: &&Vec3| s.lights.iter().any(|l| l.at.distance(**h) < HERO_LIT_M);
            (all.iter().filter(by).count(), all.len())
        };
        let (mut with, mut without, mut lamps) = ((0, 0), (0, 0), 0);
        for s in 0..40 {
            let Ok(dungeon) = generate(&theme, &set, Seed(s)) else {
                continue;
            };
            let full = stamp(&dungeon, &theme, &kind);
            let bare = stamp(&dungeon, &unlamped, &kind);
            let (a, b) = (lit(&full), lit(&bare));
            (with.0, with.1) = (with.0 + a.0, with.1 + a.1);
            (without.0, without.1) = (without.0 + b.0, without.1 + b.1);
            lamps += full
                .lights
                .iter()
                .filter(|l| l.kind == LightKind::Lamp)
                .count();
        }
        println!(
            "{}: {} of {} heroes lit with their lamps ({lamps} lamps), {} of {} without",
            theme.name, with.0, with.1, without.0, without.1
        );
        // A hero on a floor too crowded for its lamp stays as it was: 2 of 346 over these
        // seeds (2026-10-27), against 200 unlit without the lamps.
        assert!(
            with.0 * 100 >= with.1 * 99,
            "{}: {} of {} heroes lit",
            theme.name,
            with.0,
            with.1
        );
    }
}

/// How far apart two props stand at most to be of one group, in metres.
const GROUP_LINK_M: f32 = 1.7;

/// A room's dressing stands in groups, as set dressers lay it (barrow dress round 3): of the
/// props a theme's dressing laid, linked within `GROUP_LINK_M`, they stand in fewer, larger
/// groups, most of three or more and few alone, where taken by a hash along every wall foot
/// they stood a metre or more apart round the room.
#[test]
fn dressing_stands_in_groups() {
    for theme in Theme::all(&assets()).unwrap() {
        if theme.dressing.is_empty() {
            continue;
        }
        let bare_theme = Theme {
            dressing: Vec::new(),
            ..theme.clone()
        };
        let set = ModuleSet::new(&theme).unwrap();
        let (mut props, mut grouped, mut alone, mut groups) = (0, 0, 0, 0);
        for s in 0..40 {
            let Ok(dungeon) = generate(&theme, &set, Seed(s)) else {
                continue;
            };
            let full = stamp(&dungeon, &theme, &kind);
            let bare = stamp(&dungeon, &bare_theme, &kind);
            let laid: Vec<Vec3> = full.props[bare.props.len()..]
                .iter()
                .map(|p| p.at)
                .collect();
            // Single-link groups, on one level.
            let mut group: Vec<usize> = (0..laid.len()).collect();
            fn root(group: &mut [usize], mut i: usize) -> usize {
                while group[i] != i {
                    group[i] = group[group[i]];
                    i = group[i];
                }
                i
            }
            for i in 0..laid.len() {
                for j in i + 1..laid.len() {
                    let (a, b) = (laid[i], laid[j]);
                    if (a.y - b.y).abs() < 1.0 && a.distance(b) < GROUP_LINK_M {
                        let (ra, rb) = (root(&mut group, i), root(&mut group, j));
                        group[ra] = rb;
                    }
                }
            }
            let mut size = vec![0; laid.len()];
            for i in 0..laid.len() {
                size[root(&mut group, i)] += 1;
            }
            for i in 0..laid.len() {
                let n = size[root(&mut group, i)];
                grouped += (n >= 3) as usize;
                alone += (n == 1) as usize;
            }
            groups += size.iter().filter(|&&n| n > 0).count();
            props += laid.len();
        }
        println!(
            "{}: {props} props in {groups} groups, {grouped} in groups of 3 or more, {alone} alone",
            theme.name
        );
        // Taken along the wall feet by a hash (d575b05c): 7667 props in 2042 groups, 87% in
        // groups of 3 or more, 6.5% alone; in groups (2026-10-27), 7866 in 1455, 93%, 2.2%.
        assert!(
            props * 10 >= groups * 45,
            "{}: {props} props in {groups} groups",
            theme.name
        );
        assert!(
            grouped * 100 >= props * 90,
            "{}: {grouped} of {props} grouped",
            theme.name
        );
        assert!(
            alone * 100 <= props * 4,
            "{}: {alone} of {props} alone",
            theme.name
        );
    }
}
