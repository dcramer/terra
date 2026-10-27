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

/// How far in from a room's walls its open middle starts, in metres: past the wall props (0.8 m
/// out) and the clutter round and by them.
const MIDDLE_M: f32 = 2.0;

/// A theme's rooms of four cells or more: how many, how many with something the dressing laid
/// in their open middle, those pieces, the rooms' cells, and the most pieces a cell of a room.
fn middles(theme: &Theme) -> (u32, u32, usize, i32, f32) {
    let bare_theme = Theme {
        dressing: Vec::new(),
        ..theme.clone()
    };
    let set = ModuleSet::new(theme).unwrap();
    let (mut rooms, mut strewn, mut pieces, mut all, mut most) = (0, 0, 0, 0, 0.0f32);
    for s in 0..40 {
        let Ok(dungeon) = generate(theme, &set, Seed(s)) else {
            continue;
        };
        let full = stamp(&dungeon, theme, &kind);
        let bare = stamp(&dungeon, &bare_theme, &kind);
        let laid = &full.props[bare.props.len()..];
        let layout = &dungeon.layout;
        for (r, room) in dungeon.graph.rooms.iter().enumerate() {
            let rect = layout.rooms[r];
            let cells = rect.size.x * rect.size.y;
            if cells < 4 || !theme.dressing.iter().any(|d| d.roles.contains(&room.role)) {
                continue;
            }
            rooms += 1;
            all += cells;
            let lo = rect.min.as_vec3() * 4.0 + Vec3::splat(MIDDLE_M);
            let size = Vec3::new(rect.size.x as f32, 0.0, rect.size.y as f32) * 4.0;
            let hi = rect.min.as_vec3() * 4.0 + size - Vec3::splat(MIDDLE_M);
            let floor = floor_of(rect.min.y) as f32 * super::super::CUBE;
            let middle = (laid.iter())
                .filter(|p| {
                    (p.at.y - floor).abs() < 1.0
                        && p.at.x > lo.x
                        && p.at.x < hi.x
                        && p.at.z > lo.z
                        && p.at.z < hi.z
                })
                .count();
            strewn += (middle > 0) as u32;
            pieces += middle;
            most = most.max(middle as f32 / cells as f32);
        }
    }
    (rooms, strewn, pieces, all, most)
}

/// A dressed room's open middle is strewn, sparsely (barrow dress round 7: the clutter went
/// round the props, then the corners, then the walls, so the floor between stayed bare stone):
/// of the rooms of four cells or more, with the theme's scatter nearly all have something the
/// dressing laid more than `MIDDLE_M` in from every wall, and far more of it than without the
/// scatter, yet no room more than three pieces a cell there. That none stands in the eye's
/// path or a doorway is the first test's.
#[test]
fn a_rooms_open_middle_is_strewn() {
    for theme in Theme::all(&assets()).unwrap() {
        if !theme.dressing.iter().any(|d| !d.scatter.is_empty()) {
            continue;
        }
        let mut unstrewn = theme.clone();
        for d in &mut unstrewn.dressing {
            d.scatter.clear();
        }
        let (rooms, strewn, pieces, cells, most) = middles(&theme);
        let (_, before, had, _, _) = middles(&unstrewn);
        println!(
            "{}: of {rooms} rooms ({cells} cells), {strewn} strewn in their middle with {pieces} \
             pieces, at most {most:.2} a cell; without the scatter {before} with {had}",
            theme.name
        );
        assert!(
            strewn * 100 >= rooms * 95,
            "{}: {strewn} of {rooms} rooms strewn",
            theme.name
        );
        assert!(
            pieces >= had * 2,
            "{}: {pieces} pieces, {had} without",
            theme.name
        );
        assert!(most <= 3.0, "{}: {most:.2} pieces a cell", theme.name);
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
        let scattered: Vec<&(String, f32)> =
            theme.dressing.iter().flat_map(|d| &d.scatter).collect();
        for s in 0..40 {
            let Ok(dungeon) = generate(&theme, &set, Seed(s)) else {
                continue;
            };
            let full = stamp(&dungeon, &theme, &kind);
            let bare = stamp(&dungeon, &bare_theme, &kind);
            // The floor's scatter apart, strewn loose by design (`a_rooms_open_middle_is_strewn`).
            let laid: Vec<Vec3> = full.props[bare.props.len()..]
                .iter()
                .filter(|p| {
                    !scattered
                        .iter()
                        .any(|(m, s)| *m == p.model && *s == p.scale)
                })
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

/// Every room's feature (a tomb, the key's altar) has a fire or a lamp by it, within
/// `HERO_LIT_M` (barrow dress round 5): a torch does not count, since one hung over the
/// crypt's antechamber's sarcophagus left it a dark box (dress round 4, all three reviewers).
#[test]
fn every_feature_has_a_fire_or_lamp_by_it() {
    for theme in Theme::all(&assets()).unwrap() {
        if !theme.dressing.iter().any(|d| d.lamp.is_some()) {
            continue;
        }
        let set = ModuleSet::new(&theme).unwrap();
        let (mut lit, mut all) = (0, 0);
        for s in 0..40 {
            let Ok(dungeon) = generate(&theme, &set, Seed(s)) else {
                continue;
            };
            let full = stamp(&dungeon, &theme, &kind);
            let tombs = (full.pieces.iter()).filter(|p| p.model.contains("Sarcophagus"));
            let keys = (full.objects.iter()).filter(|o| o.role == ObjectRole::Key);
            for at in tombs.map(|p| p.at).chain(keys.map(|o| o.at)) {
                all += 1;
                let fire = (full.lights.iter())
                    .any(|l| l.kind != LightKind::Torch && l.at.distance(at) < HERO_LIT_M);
                lit += fire as u32;
            }
        }
        println!(
            "{}: {lit} of {all} features lit by a fire or lamp",
            theme.name
        );
        // A feature on a floor too crowded for its lamp stays as it was: 1 of 139 over these
        // seeds (2026-10-27), against 0 of 139 lit before features had lamps.
        assert!(
            lit * 100 >= all * 95,
            "{}: {lit} of {all} features lit",
            theme.name
        );
    }
}

/// Every room type's vignette is told (D539): its first piece and one beside it at least, in
/// 49 rooms of its type in 50 over 40 seeds. Laid all or none at a wall spot, the crypt's
/// dropped without a word in 1 to 5 rooms of a type over these seeds (the guard post's, 2.2 m
/// wide, in 3 of 40); drawn closer, cut short or on the open floor, in none (barrow dress 14,
/// 2026-10-27).
#[test]
fn every_room_types_vignette_is_told() {
    for theme in Theme::all(&assets()).unwrap() {
        // Its vignettes' models marked, to tell them from the same models laid as props.
        let mut marked = theme.clone();
        for d in &mut marked.dressing {
            for (m, _, _) in &mut d.vignette {
                *m = format!("vignette/{m}");
            }
        }
        let set = ModuleSet::new(&marked).unwrap();
        let mut told = vec![(0u32, 0u32); marked.dressing.len()];
        for s in 0..40 {
            let Ok(dungeon) = generate(&marked, &set, Seed(s)) else {
                continue;
            };
            let full = stamp(&dungeon, &marked, &kind);
            for (r, room) in dungeon.graph.rooms.iter().enumerate() {
                let rect = dungeon.layout.rooms[r];
                let lo = rect.min.as_vec3() * 4.0;
                let hi = lo + Vec3::new(rect.size.x as f32, 0.0, rect.size.y as f32) * 4.0;
                let floor = floor_of(rect.min.y) as f32 * super::super::CUBE;
                let inside = |p: Vec3| {
                    (p.y - floor).abs() < 1.0
                        && (lo.x..hi.x).contains(&p.x)
                        && (lo.z..hi.z).contains(&p.z)
                };
                let here = |m: &str| (full.props.iter()).any(|p| p.model == m && inside(p.at));
                for (k, d) in marked.dressing.iter().enumerate() {
                    if d.vignette.is_empty() || !d.roles.contains(&room.role) {
                        continue;
                    }
                    let laid = (d.vignette.iter()).filter(|(m, _, _)| here(m)).count();
                    let first = here(&d.vignette[0].0);
                    let ok = first && laid >= d.vignette.len().min(2);
                    told[k].1 += 1;
                    told[k].0 += ok as u32;
                    if !ok {
                        println!("{:?} seed {s} room {r}: {laid} pieces", d.roles);
                    }
                }
            }
        }
        for (d, &(laid, rooms)) in marked.dressing.iter().zip(&told) {
            if rooms == 0 {
                continue;
            }
            println!(
                "{}: the {:?} vignette told in {laid} of {rooms} rooms",
                theme.name, d.roles
            );
            assert!(
                laid * 50 >= rooms * 49,
                "{}: the {:?} vignette in {laid} of {rooms} rooms",
                theme.name,
                d.roles
            );
        }
    }
}

/// How near a vignette's first piece lies to its room's focus to read with it, in metres,
/// across the floor: past a hero's clearance (1.5 m) and a vignette's half width (up to
/// 1.35 m, the guard post's) and half a metre; a camp's within its fire's cell.
const FOCUS_M: f32 = 3.5;

/// A room's vignette lies by its focus, its hero or else its fire (barrow dress round 16):
/// laid at the first wall spot by its hash, the crypt's camp's bedroll lay a cell or more from
/// its fire and the guard post's fallen guard across the room from the bench, so no frame held
/// both. Of the rooms with a vignette and a focus over 40 seeds, 9 in 10 a type.
#[test]
fn a_vignette_lies_by_its_rooms_focus() {
    for theme in Theme::all(&assets()).unwrap() {
        let mut marked = theme.clone();
        for d in &mut marked.dressing {
            for (m, _, _) in &mut d.vignette {
                *m = format!("vignette/{m}");
            }
        }
        let set = ModuleSet::new(&marked).unwrap();
        let mut near = vec![(0u32, 0u32, 0.0f32); marked.dressing.len()];
        for s in 0..40 {
            let Ok(dungeon) = generate(&marked, &set, Seed(s)) else {
                continue;
            };
            let full = stamp(&dungeon, &marked, &kind);
            let heroic = heroes(&marked, &full);
            for (r, room) in dungeon.graph.rooms.iter().enumerate() {
                let rect = dungeon.layout.rooms[r];
                let lo = rect.min.as_vec3() * 4.0;
                let hi = lo + Vec3::new(rect.size.x as f32, 0.0, rect.size.y as f32) * 4.0;
                let floor = floor_of(rect.min.y) as f32 * super::super::CUBE;
                let inside = |p: &Vec3| {
                    (p.y - floor).abs() < 1.0
                        && (lo.x..hi.x).contains(&p.x)
                        && (lo.z..hi.z).contains(&p.z)
                };
                let fire = (full.lights.iter())
                    .filter(|l| l.kind == LightKind::Campfire)
                    .map(|l| l.at)
                    .find(inside);
                let Some(focus) = heroic.iter().copied().find(inside).or(fire) else {
                    continue;
                };
                let flat = |p: Vec3| Vec2::new(p.x, p.z);
                for (k, d) in marked.dressing.iter().enumerate() {
                    if d.vignette.is_empty() || !d.roles.contains(&room.role) {
                        continue;
                    }
                    let first =
                        (full.props.iter()).find(|p| p.model == d.vignette[0].0 && inside(&p.at));
                    let Some(first) = first else { continue };
                    let gap = flat(first.at).distance(flat(focus));
                    near[k].1 += 1;
                    near[k].0 += (gap < FOCUS_M) as u32;
                    near[k].2 = near[k].2.max(gap);
                }
            }
        }
        for (d, &(by, rooms, most)) in marked.dressing.iter().zip(&near) {
            if rooms > 0 {
                println!(
                    "{}: the {:?} vignette by its focus in {by} of {rooms}, at most {most:.1} m",
                    theme.name, d.roles
                );
            }
        }
        for (d, &(by, rooms, _)) in marked.dressing.iter().zip(&near) {
            assert!(
                by * 10 >= rooms * 9,
                "{}: the {:?} vignette by its focus in {by} of {rooms} rooms",
                theme.name,
                d.roles
            );
        }
    }
}
