//! The pointer a script moves and clicks with (`sandbox.point`, `sandbox.click`), sent to
//! picking and the mouse's input as a person's would be.

use super::refused;
use bevy::{
    camera::RenderTarget,
    ecs::system::SystemParam,
    picking::pointer::{Location, PointerAction, PointerButton, PointerId, PointerInput},
    prelude::*,
    remote::{BrpResult, builtin_methods::parse_some},
    window::{PrimaryWindow, WindowRef},
};
use bevy_inspector_egui::bevy_egui::{
    EguiZoomFactor, PrimaryEguiContext, egui, input::EguiInputEvent,
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

/// What the pointer does, told to egui (the debug menu, the F1 inspector) as the window's own
/// would be: egui reads the window's events, which a script's pointer never makes, and a run
/// drawing into a picture has its context on no window at all.
#[derive(SystemParam)]
pub(super) struct Egui<'w, 's> {
    context: Query<'w, 's, (Entity, &'static EguiZoomFactor), With<PrimaryEguiContext>>,
    events: MessageWriter<'w, EguiInputEvent>,
}

impl Egui<'_, '_> {
    fn tell(&mut self, event: impl FnOnce(egui::Pos2) -> egui::Event, at: Vec2) {
        if let Ok((context, zoom)) = self.context.single() {
            let at = at / zoom.zoom_factor;
            let event = event(egui::pos2(at.x, at.y));
            self.events.write(EguiInputEvent { context, event });
        }
    }

    fn button(&mut self, button: MouseButton, pressed: bool, at: Vec2) {
        let button = match button {
            MouseButton::Right => egui::PointerButton::Secondary,
            _ => egui::PointerButton::Primary,
        };
        self.tell(
            |pos| egui::Event::PointerButton {
                pos,
                button,
                pressed,
                modifiers: egui::Modifiers::NONE,
            },
            at,
        );
    }
}

/// Takes a script's next pointer step, as the mouse would: what the pointer does goes to
/// picking, which hovers and presses what is under it, and to egui, and a button to the
/// mouse's input as well, which an unattended run lets go of every frame before this, as it
/// does keys.
pub(super) fn drive_pointer(
    mut pointing: ResMut<Pointing>,
    window: Query<Entity, With<PrimaryWindow>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut inputs: MessageWriter<PointerInput>,
    mut egui: Egui,
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
            egui.button(button, false, *at);
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
            egui.tell(egui::Event::PointerMoved, to);
            *at = to;
        }
        Some(PointerStep::Down(button, frames)) => {
            mouse.press(button);
            inputs.write(PointerInput::new(
                PointerId::Mouse,
                location(*at),
                PointerAction::Press(pointer(button)),
            ));
            egui.button(button, true, *at);
            pointing.held = Some((button, frames));
        }
        None => {}
    }
}
