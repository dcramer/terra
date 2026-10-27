use super::super::tests::kind;
use super::super::{Frame, LightKind, MID, ObjectRole, Prop, Theme, floor_of, stamp};
use super::OFF_DOOR_M;
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
