//! What the builder says it would do, and its keys.

use super::*;
use crate::play::catalogue::{Builds, Kind};
use crate::play::controls::{Action, Settings, capitalised};
use crate::play::inventory::{self};
use crate::play::props::Blocked;
use crate::shell::Options;
use bevy::window::{CursorGrabMode, CursorOptions};

/// What each button would do right now, for the prompt under the aiming mark.
pub fn prompt(
    builder: &Builder,
    builds: &Builds,
    settings: &Settings,
    items: &inventory::Items,
) -> String {
    if builder.stowed {
        return format!(
            "{}  build menu",
            capitalised(settings.key(Action::BuildMenu))
        );
    }
    match builder.progress() {
        Some((_, true)) => return "Taking apart      Let go to stop".to_string(),
        Some((_, false)) => return "Building      Let go to stop".to_string(),
        None => {}
    }
    if let Some(reason) = builder.unclaimed {
        return reason.to_string();
    }
    if let Some((kind, more)) = builder.short {
        return format!(
            "Not enough to build this: {more} more {} needed ({})",
            items.name(kind),
            items.found(kind)
        );
    }
    let hold = settings.hold(Action::Use);
    let tap = capitalised(settings.key(Action::Use));
    let next = settings.key(Action::NextKind);
    let action = match (&builder.picked(builds).kind, builder.target) {
        (Kind::Reshape, Target::Wall(_)) if builder.pull => {
            format!("{hold}  pull the wall in      Scroll  push out instead")
        }
        (Kind::Reshape, Target::Wall(_)) => {
            format!("{hold}  push the wall out      Scroll  pull in instead")
        }
        (Kind::Window(_) | Kind::Door, Target::Wall(_) | Target::Edge { .. })
            if builder.run.is_empty() =>
        {
            "The wall is not wide enough here".to_string()
        }
        (Kind::Window(_), Target::Wall(_) | Target::Edge { .. }) => {
            format!("{hold}  cut this window      {next}  size")
        }
        (Kind::Door, Target::Wall(_) | Target::Edge { .. }) => format!("{hold}  cut a door"),
        (Kind::Reshape, _) => "Aim at an outside wall".to_string(),
        (Kind::Window(_) | Kind::Door, _) => "Aim at a wall, outside or interior".to_string(),
        (Kind::Wall(_), Target::Edge { fits: true, .. }) => {
            format!("{hold}  build this wall      Scroll  turn      {next}  length")
        }
        (Kind::Wall(_), Target::Edge { fits: false, .. }) => "Walls go indoors".to_string(),
        (Kind::Wall(_), _) => "Aim at a floor indoors".to_string(),
        (Kind::Room(_), Target::Ground { fits: false, .. }) => {
            "The ground is too high here: a room cuts into it no deeper than its roof".to_string()
        }
        (Kind::Room(_), Target::Ground { clear: false, .. }) => {
            "Trees or rocks are in the way: clear them first".to_string()
        }
        (Kind::Room(_), Target::Ground { item: Some(_), .. }) => {
            "The outlined item is in the way: move it or take it apart first".to_string()
        }
        (Kind::Room(_), Target::Ground { adds: false, .. }) => {
            "That is all indoors already".to_string()
        }
        (Kind::Room(_), Target::Ground { .. }) => format!(
            "{hold}  build this room      Scroll  turn      Shift Scroll  storey{}      {next}  size",
            storey(builder.storey)
        ),
        (Kind::Room(_), _) => "Aim at the ground, or across a room to extend it".to_string(),
        (Kind::Items(_) | Kind::Move, Target::Spot { blocked, .. }) => match blocked {
            None if builder.is_carrying() => format!(
                "{tap}  put it down      Scroll  turn      Shift Scroll  raise{}      {}  put it back",
                raised(builder.raise),
                capitalised(settings.key(Action::BuildMenu))
            ),
            None => format!(
                "{hold}  build      Scroll  turn      Shift Scroll  raise{}      {next}  next kind",
                raised(builder.raise)
            ),
            Some(Blocked::Structure) => "It would stick through a wall".to_string(),
            Some(Blocked::Item(_)) => "The outlined item is in the way".to_string(),
            Some(Blocked::Nature) => "A tree or a boulder is in the way".to_string(),
            Some(Blocked::Water) => "Not in the water".to_string(),
        },
        (Kind::Items(_), _) => "Aim at a floor or at the ground nearby".to_string(),
        (Kind::Move, Target::Prop(_)) => format!("{tap}  pick this up"),
        (Kind::Move, _) => format!(
            "Aim at an item to move it      {}  done",
            settings.key(Action::MoveItems)
        ),
        (Kind::Remove, Target::Prop(_)) => format!("{hold}  take this apart"),
        (Kind::Remove, Target::Edge { open: true, .. }) => format!("{hold}  close this opening"),
        (Kind::Remove, Target::Edge { .. }) => format!("{hold}  take this wall away"),
        (Kind::Remove, Target::Wall(_)) => format!("{hold}  close this opening"),
        (Kind::Remove, _) => format!(
            "Aim at an item, an interior wall, a window or a door      {}  done",
            settings.key(Action::TakeApart)
        ),
    };
    match &builder.price {
        Some((cost, times)) if *times > 0 => {
            format!(
                "{action}      Costs {}",
                inventory::describe(items, cost, *times)
            )
        }
        _ => action,
    }
}

/// How high an item is raised, if it is, to follow its key: " (0.5 m up)".
fn raised(raise: f32) -> String {
    if raise > 0.0 {
        format!(" ({raise:.1} m up)")
    } else {
        String::new()
    }
}

/// Which storey a room goes on, if not the base's floor: " (1 down)".
fn storey(storey: i32) -> String {
    match storey {
        0 => String::new(),
        up if up > 0 => format!(" ({up} up)"),
        down => format!(" ({} down)", -down),
    }
}

/// The keys worth knowing while a part is in hand.
pub fn legend(builder: &Builder, settings: &Settings) -> String {
    if builder.stowed {
        return String::new();
    }
    let key = |action| capitalised(settings.key(action));
    let (grid, held) = if builder.snap {
        ("grid off", "off the grid")
    } else {
        ("grid on", "on the grid")
    };
    format!(
        "{}  take apart     {}  move     {}  {grid}     {}  {held}     {}  copy",
        key(Action::TakeApart),
        key(Action::MoveItems),
        key(Action::Grid),
        key(Action::OffGrid),
        key(Action::Copy),
    )
}

/// Whether the tool is waiting for the player to take the mouse.
pub fn waiting(cursor: &CursorOptions, options: &Options) -> bool {
    cursor.grab_mode == CursorGrabMode::None && !options.scripted()
}
