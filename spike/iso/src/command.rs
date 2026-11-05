//! Commanding the party as the Infinity Engine does: click someone to choose them, drag a
//! box round several, click the ground to send the chosen there in formation, right click
//! to let go; green circles under the chosen, a marker where they were sent, and the game
//! pausable at any moment with the space bar.

use crate::Args;
use crate::camera::MainCamera;
use crate::nav::Nav;
use crate::party::{Actor, Folk, Member, Selected};
use crate::scene::Hour;
use crate::ui::{Log, PORTRAIT_COLUMN};
use bevy::diagnostic::FrameCount;
use crate::terrain::Land;
use bevy::prelude::*;

pub fn plugin(app: &mut App) {
    app.init_resource::<Drag>()
        .init_resource::<Hovered>()
        .init_resource::<Markers>()
        .add_systems(Update, (hover, click, keys, script).chain())
        // After the transforms are propagated, so a ring is drawn where its walker is this
        // frame, not where it stood the frame before.
        .add_systems(
            PostUpdate,
            circles.after(bevy::transform::TransformSystems::Propagate),
        );
}

#[derive(Resource, Default)]
pub struct Drag {
    pub start: Option<Vec2>,
    pub now: Vec2,
}

#[derive(Resource, Default)]
pub struct Hovered(pub Option<Entity>);

#[derive(Resource, Default)]
struct Markers(Vec<(Vec2, f32)>);

fn cursor(windows: &Query<&Window>) -> Option<Vec2> {
    windows.single().ok()?.cursor_position()
}

/// Who is under the cursor: the nearest to the camera whose body the cursor is on.
fn hover(
    windows: Query<&Window>,
    camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    actors: Query<(Entity, &Actor, &GlobalTransform), Or<(With<Member>, With<Folk>)>>,
    mut hovered: ResMut<Hovered>,
) {
    hovered.0 = None;
    let (Some(c), Ok((cam, cam_tr))) = (cursor(&windows), camera.single()) else {
        return;
    };
    let mut best: Option<(f32, Entity)> = None;
    for (e, actor, tr) in &actors {
        let feet = tr.translation();
        let (Ok(f), Ok(h)) = (
            cam.world_to_viewport(cam_tr, feet),
            cam.world_to_viewport(cam_tr, feet + Vec3::Y * actor.height),
        ) else {
            continue;
        };
        let Ok(side) = cam.world_to_viewport(cam_tr, feet + cam_tr.right() * actor.radius * 1.4)
        else {
            continue;
        };
        let half = (side.x - f.x).abs();
        if (c.x - f.x).abs() < half && c.y < f.y + half * 0.5 && c.y > h.y {
            if best.is_none_or(|(y, _)| f.y > y) {
                best = Some((f.y, e));
            }
        }
    }
    hovered.0 = best.map(|(_, e)| e);
}

fn ground(land: &Land, cam: &Camera, cam_tr: &GlobalTransform, at: Vec2) -> Option<Vec2> {
    let ray = cam.viewport_to_world(cam_tr, at).ok()?;
    Some(land.hit(ray)?.xz())
}

/// A ring laid on the land, so it follows the slope rather than cutting into it.
fn ring(gizmos: &mut Gizmos, land: &Land, at: Vec2, r: f32, color: LinearRgba) {
    let points = (0..=40).map(|i| {
        let p = at + Vec2::from_angle(i as f32 / 40.0 * std::f32::consts::TAU) * r;
        Vec3::new(p.x, land.height(p) + 0.05, p.y)
    });
    gizmos.linestrip(points, color);
}

#[allow(clippy::too_many_arguments)]
fn click(
    windows: Query<&Window>,
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut drag: ResMut<Drag>,
    hovered: Res<Hovered>,
    mut commands: Commands,
    members: Query<(Entity, &Member, &GlobalTransform, Has<Selected>)>,
    folk: Query<&Folk>,
    mut actors: Query<&mut Actor>,
    nav: Res<Nav>,
    land: Res<Land>,
    mut markers: ResMut<Markers>,
    mut log: ResMut<Log>,
) {
    let (Some(c), Ok((cam, cam_tr)), Ok(window)) =
        (cursor(&windows), camera.single(), windows.single())
    else {
        return;
    };
    drag.now = c;
    let over_ui = c.x > window.width() - PORTRAIT_COLUMN;
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    if buttons.just_pressed(MouseButton::Left) && !over_ui {
        drag.start = Some(c);
    }
    if buttons.just_pressed(MouseButton::Right) {
        for (e, ..) in &members {
            commands.entity(e).remove::<Selected>();
        }
    }
    if !buttons.just_released(MouseButton::Left) {
        return;
    }
    let Some(start) = drag.start.take() else {
        return;
    };
    if start.distance(c) > 6.0 {
        // A box: everyone whose feet are inside it.
        let (lo, hi) = (start.min(c), start.max(c));
        for (e, m, tr, sel) in &members {
            let inside = cam
                .world_to_viewport(cam_tr, tr.translation())
                .is_ok_and(|p| p.cmpge(lo).all() && p.cmple(hi).all());
            if inside {
                if !sel {
                    log.say(m.name, m.barks[0]);
                }
                commands.entity(e).insert(Selected);
            } else if !shift {
                commands.entity(e).remove::<Selected>();
            }
        }
        return;
    }
    if let Some(who) = hovered.0 {
        if let Ok((_, m, _, sel)) = members.get(who) {
            if !shift {
                for (e, ..) in &members {
                    commands.entity(e).remove::<Selected>();
                }
            }
            if shift && sel {
                commands.entity(who).remove::<Selected>();
            } else {
                commands.entity(who).insert(Selected);
                let n = log.count();
                log.say(m.name, m.barks[(m.slot + n) % 3]);
            }
            return;
        }
        if let Ok(f) = folk.get(who) {
            log.note(format!("{}. They pay you no mind.", f.name));
            return;
        }
    }
    let Some(goal) = ground(&land, cam, cam_tr, c) else {
        return;
    };
    let chosen: Vec<(Entity, Vec2)> = members
        .iter()
        .filter(|m| m.3)
        .map(|m| (m.0, m.2.translation().xz()))
        .collect();
    order(&chosen, goal, &nav, &mut actors, &mut markers);
}

/// Sends the chosen to `goal` in a wedge facing the way they go: the first at its point.
fn order(
    chosen: &[(Entity, Vec2)],
    goal: Vec2,
    nav: &Nav,
    actors: &mut Query<&mut Actor>,
    markers: &mut Markers,
) {
    if chosen.is_empty() {
        return;
    }
    let centre = chosen.iter().map(|c| c.1).sum::<Vec2>() / chosen.len() as f32;
    let ahead = (goal - centre).try_normalize().unwrap_or(Vec2::Y);
    let side = ahead.perp();
    let wedge = [
        Vec2::ZERO,
        Vec2::new(-1.1, -1.1),
        Vec2::new(1.1, -1.1),
        Vec2::new(0.0, -2.2),
        Vec2::new(-2.2, -2.2),
        Vec2::new(2.2, -2.2),
    ];
    for (i, (e, at)) in chosen.iter().enumerate() {
        let slot = wedge[i % wedge.len()];
        let spot = goal + side * slot.x + ahead * slot.y;
        let Some(spot) = nav.nearest_open(spot) else {
            continue;
        };
        if let (Ok(mut actor), Some(path)) = (actors.get_mut(*e), nav.path(*at, spot)) {
            let length: f32 = path
                .iter()
                .scan(*at, |here, p| {
                    let d = here.distance(*p);
                    *here = *p;
                    Some(d)
                })
                .sum();
            actor.running = length > 8.0;
            actor.path = path.into();
        }
    }
    markers.0.push((goal, 0.0));
}

fn keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut time: ResMut<Time<Virtual>>,
    mut hour: ResMut<Hour>,
    mut commands: Commands,
    members: Query<(Entity, &Member)>,
    mut log: ResMut<Log>,
) {
    if keys.just_pressed(KeyCode::Space) {
        if time.is_paused() {
            time.unpause();
        } else {
            time.pause();
            log.note("Paused.".into());
        }
    }
    if keys.just_pressed(KeyCode::KeyT) {
        *hour = hour.next();
        log.note(
            format!("It is now {:?}.", *hour)
                .to_lowercase()
                .replace("it is now", "It is now"),
        );
    }
    let all = keys.just_pressed(KeyCode::Equal) || keys.just_pressed(KeyCode::Backquote);
    for (i, key) in [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
    ]
    .into_iter()
    .enumerate()
    {
        if keys.just_pressed(key) || all {
            for (e, m) in &members {
                if m.slot == i || all {
                    commands.entity(e).insert(Selected);
                    if !all {
                        log.say(m.name, m.barks[0]);
                    }
                } else if !keys.pressed(KeyCode::ShiftLeft) {
                    commands.entity(e).remove::<Selected>();
                }
            }
        }
    }
}

/// A scripted run (`--demo`): the whole party chosen and sent across the square.
fn script(
    args: Res<Args>,
    frame: Res<FrameCount>,
    mut commands: Commands,
    members: Query<(Entity, &GlobalTransform), With<Member>>,
    nav: Res<Nav>,
    mut actors: Query<&mut Actor>,
    mut markers: ResMut<Markers>,
) {
    if !args.demo || frame.0 != args.demo_at {
        return;
    }
    let chosen: Vec<(Entity, Vec2)> = members
        .iter()
        .map(|(e, tr)| (e, tr.translation().xz()))
        .collect();
    for (e, _) in &chosen {
        commands.entity(*e).insert(Selected);
    }
    order(
        &chosen,
        Vec2::new(-4.5, -1.5),
        &nav,
        &mut actors,
        &mut markers,
    );
}

/// The circles at their feet, BG-style: green for the chosen, a fainter ring for whoever
/// the cursor is on (blue for folk), and where the party was sent.
fn circles(
    mut gizmos: Gizmos,
    time: Res<Time<Real>>,
    hovered: Res<Hovered>,
    members: Query<(Entity, &Actor, &GlobalTransform, Has<Selected>), With<Member>>,
    folk: Query<(Entity, &Actor, &GlobalTransform), With<Folk>>,
    mut markers: ResMut<Markers>,
    land: Res<Land>,
) {
    // Quiet enough not to be the loudest thing on screen.
    let green = LinearRgba::rgb(0.16, 1.2, 0.22);
    for (e, actor, tr, sel) in &members {
        let r = actor.radius * 1.5;
        if sel {
            ring(&mut gizmos, &land, tr.translation().xz(), r, green);
        } else if hovered.0 == Some(e) {
            ring(&mut gizmos, &land, tr.translation().xz(), r, green * 0.35);
        }
    }
    for (e, actor, tr) in &folk {
        if hovered.0 == Some(e) {
            ring(
                &mut gizmos,
                &land,
                tr.translation().xz(),
                actor.radius * 1.5,
                LinearRgba::rgb(0.3, 0.8, 3.0),
            );
        }
    }
    let dt = time.delta_secs();
    markers.0.retain_mut(|(at, age)| {
        *age += dt;
        let r = 0.6 * (1.0 - *age / 0.8).max(0.0) + 0.1;
        ring(&mut gizmos, &land, *at, r, green * (1.0 - *age / 0.8).max(0.0));
        *age < 0.8
    });
}
