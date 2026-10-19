//! The pointer a script moves and clicks with (`sandbox.point`, `sandbox.click`), sent to
//! picking and the mouse's input as a person's would be.

use super::refused;
use bevy::{
    camera::RenderTarget,
    picking::pointer::{Location, PointerAction, PointerButton, PointerId, PointerInput},
    prelude::*,
    remote::{BrpResult, builtin_methods::parse_some},
    window::{PrimaryWindow, WindowRef},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::VecDeque;

/// What a script is doing with the pointer: the steps still to take, one a frame, and the
/// mouse button it holds down, with the frames left until it lets go.
#[derive(Resource, Default)]
pub(super) struct Pointing {
    steps: VecDeque<PointerStep>,
    held: Option<(MouseButton, u32)>,
}

enum PointerStep {
    Move(Vec2),
    Down(MouseButton, u32),
}

#[derive(Deserialize)]
struct Point {
    x: f32,
    y: f32,
    button: Option<String>,
    frames: Option<u32>,
}

pub(super) fn point(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let Point { x, y, .. } = parse_some(params)?;
    let mut pointing = world.resource_mut::<Pointing>();
    pointing.steps.push_back(PointerStep::Move(Vec2::new(x, y)));
    Ok(json!({ "x": x, "y": y }))
}

pub(super) fn click(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let Point {
        x,
        y,
        button,
        frames,
    } = parse_some(params)?;
    let button = match button.as_deref().unwrap_or("left") {
        "left" => MouseButton::Left,
        "right" => MouseButton::Right,
        other => return Err(refused(format!("wanted left or right, not `{other}`"))),
    };
    let frames = frames.unwrap_or(1).max(1);
    let mut pointing = world.resource_mut::<Pointing>();
    pointing.steps.push_back(PointerStep::Move(Vec2::new(x, y)));
    pointing.steps.push_back(PointerStep::Down(button, frames));
    Ok(json!({ "x": x, "y": y, "button": format!("{button:?}"), "frames": frames }))
}

/// Takes a script's next pointer step, as the mouse would: what the pointer does goes to
/// picking, which hovers and presses what is under it, and a button to the mouse's input
/// as well, which an unattended run lets go of every frame before this, as it does keys.
pub(super) fn drive_pointer(
    mut pointing: ResMut<Pointing>,
    window: Query<Entity, With<PrimaryWindow>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut inputs: MessageWriter<PointerInput>,
    mut at: Local<Vec2>,
) {
    let Some(target) = RenderTarget::Window(WindowRef::Primary).normalize(window.single().ok())
    else {
        return;
    };
    let pointer = |button: MouseButton| match button {
        MouseButton::Right => PointerButton::Secondary,
        _ => PointerButton::Primary,
    };
    let location = |position: Vec2| Location {
        target: target.clone(),
        position,
    };
    if let Some((button, left)) = pointing.held {
        if left > 1 {
            mouse.press(button);
            mouse.clear_just_pressed(button);
            pointing.held = Some((button, left - 1));
        } else {
            mouse.release(button);
            inputs.write(PointerInput::new(
                PointerId::Mouse,
                location(*at),
                PointerAction::Release(pointer(button)),
            ));
            pointing.held = None;
        }
        return;
    }
    match pointing.steps.pop_front() {
        Some(PointerStep::Move(to)) => {
            inputs.write(PointerInput::new(
                PointerId::Mouse,
                location(to),
                PointerAction::Move { delta: to - *at },
            ));
            *at = to;
        }
        Some(PointerStep::Down(button, frames)) => {
            mouse.press(button);
            inputs.write(PointerInput::new(
                PointerId::Mouse,
                location(*at),
                PointerAction::Press(pointer(button)),
            ));
            pointing.held = Some((button, frames));
        }
        None => {}
    }
}
