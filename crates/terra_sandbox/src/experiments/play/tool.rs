//! The builder tool: what it is aimed at, what it would do there, and doing it. Its parts
//! are in `tool/`: `aim` (what it is on, and what that would cost), `act` (doing it),
//! `show` (outlines and ghosts before it is done) and `prompt` (what it says).
//!
//! The controls follow the habit of Subnautica and games like it, as far as we could
//! confirm it (research/subnautica-2-building.md): the right mouse button is the menu,
//! the left one builds, the wheel turns what you are holding, and taking apart and moving
//! are modes with their own keys.
//!
//!   Hold right mouse   the dial of parts; move the mouse to one and let go (or hold B)
//!   Tap right mouse    put the part away, or put back what you picked up
//!   Hold left click    build, or take apart: it takes a moment, and letting go
//!                      early leaves nothing done                         (or E)
//!   Left click         pick an item up and put it down, in move mode
//!   Scroll wheel       turn an item, a room or a wall; switch a wall between
//!                      pushing out and pulling in                        (or Z and C)
//!   R                  the next variant of the part
//!   Q                  take-apart mode, and back
//!   F                  move mode, and back
//!   X                  the grid on and off: items snap to it and turn in eighths
//!   Hold Alt           the opposite of the grid setting, while held
//!   Middle click       pick another of the item the view is on                 (or V)
//!   1 to 9, 0          the first ten parts directly
//!
//! Those are the defaults. Every control but the wheel and the numbers can be set to
//! another key or button in the options menu, and the prompts name whatever it is set to.
//!
//! Whatever the tool would do is shown in the world before you commit: the wall a push
//! would move, the outline of a window, the footprint of a room, or a ghost of the item
//! where it would stand.

use super::catalogue::{self, Item, Kind, PARTS, Part};
use super::controls::{Action, Controls};
use super::dial::Dial;
use super::inventory::{Belongings, Cost, InventoryScreen, Raw, Tool};
use super::props::{Blocked, Prop};
use super::work::Hold;
use bevy::{
    input::mouse::AccumulatedMouseScroll,
    light::{NotShadowCaster, NotShadowReceiver},
    prelude::*,
};
use terra_build::{BuildOp, CELL_M, Face, cells};

mod act;
mod aim;
mod prompt;
mod show;

use act::{act, animate};
use aim::{afford, aim};
pub use prompt::{legend, prompt, waiting};
use show::highlight;

/// How far from the player walls and rooms can be worked on, and items placed.
const STRUCTURE_REACH_M: f32 = 18.0;
const ITEM_REACH_M: f32 = 9.0;
/// How far a player can step from where they began a job before it is abandoned.
const WANDER_M: f32 = 2.5;
/// Where an item is offered when the view is not on anything within reach.
const ITEM_NEAR_M: f32 = 3.0;
/// The grid items snap to, in metres. It divides a tile evenly, so it lines up with walls.
const GRID_M: f32 = 0.5;
/// How far one notch of the wheel turns an item, on the grid and off it, in degrees.
const GRID_TURN: f32 = 45.0;
const FREE_TURN: f32 = 15.0;
const BUILD_COLOUR: Color = Color::srgb(0.35, 0.9, 1.0);
const OPENING_COLOUR: Color = Color::srgb(1.0, 0.78, 0.25);
const REFUSED_COLOUR: Color = Color::srgb(1.0, 0.35, 0.3);

/// The systems that use the tool, for ordering against the player's movement.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct Acting;

/// What the tool is pointed at.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Target {
    Nothing,
    /// A tile of the outside wall.
    Wall(Face),
    /// The edge between two tiles, where an interior wall stands or could; whether one
    /// may stand there; and whether the wall there has a window or door in it.
    Edge {
        face: Face,
        fits: bool,
        open: bool,
    },
    /// Where a room would go: its lowest tile, its size in tiles, whether the ground is
    /// low enough for it to be cut into, whether it would add anything, and whether the
    /// place is clear of trees and rocks.
    Ground {
        corner: IVec3,
        tiles: IVec2,
        fits: bool,
        adds: bool,
        clear: bool,
    },
    /// Somewhere an item could stand, in the base's own space, how far above the floor
    /// or ground, and what is in the way if anything.
    Spot {
        at: Vec3,
        lift: f32,
        indoors: bool,
        blocked: Option<Blocked>,
    },
    /// An item already placed.
    Prop(Entity),
}

/// An item picked up in move mode, and where it came from in case it is put back.
struct Carried {
    item: &'static Item,
    at: Vec3,
    lift: f32,
    turn: f32,
    indoors: bool,
}

/// What is being built or taken apart while the button is held. Everything needed to
/// finish it is taken when the button goes down, so looking away does not change it.
enum Job {
    /// Changes to the base, with what they cost or give back.
    Structure {
        ops: Vec<BuildOp>,
        pay: Option<(Cost, u32)>,
        refund: Option<(Cost, u32)>,
    },
    Place {
        item: &'static Item,
        at: Vec3,
        lift: f32,
        turn: f32,
        indoors: bool,
    },
    TakeAway(Entity),
}

struct Work {
    job: Job,
    hold: Hold,
    /// Whether it takes something away, which is shown differently.
    removing: bool,
    /// Where the player stood when it began. Wandering off abandons it.
    began_at: Vec3,
}

#[derive(Resource)]
pub struct Builder {
    /// Which part is picked, as its place in the catalogue.
    pub part: usize,
    /// Nothing is in hand: the tool does nothing until a part is picked.
    pub stowed: bool,
    /// The variant last used of each part.
    variants: Vec<usize>,
    /// How far the next item is turned, in degrees.
    turn: f32,
    /// Whether the next room is turned a quarter circle.
    room_turned: bool,
    /// Whether the next interior wall runs north and south instead of east and west.
    wall_turned: bool,
    /// The wall cells the thing aimed at would change. Empty when it would not fit.
    run: Vec<Face>,
    /// Whether items snap to the grid and turn in eighths.
    pub snap: bool,
    /// The same, right now: holding Alt does the opposite of the setting.
    snapping: bool,
    /// The item the view is on, whatever is in hand.
    viewed: Option<Entity>,
    /// How many notches the wheel has turned this frame, away from the player counting up.
    wheel: i32,
    /// The part to go back to on leaving take-apart or move mode.
    before_mode: Option<usize>,
    carrying: Option<Carried>,
    /// What is being built or taken apart right now.
    work: Option<Work>,
    /// Set when something is finished: the button has to come up before the next.
    must_release: bool,
    /// Whether reshaping pulls a wall in instead of pushing it out.
    pull: bool,
    /// An item shrunk because it is being taken apart, to be put right if that stops.
    shrinking: Option<Entity>,
    /// Set by a tap of the right mouse button: put away, or put back.
    pub cancel: bool,
    /// What building the thing aimed at would cost, and how many times over.
    price: Option<(Cost, u32)>,
    /// What there is not enough of to pay that, and how many more are needed.
    short: Option<(Raw, u32)>,
    /// Why the claim forbids building here, if it does (D94).
    unclaimed: Option<&'static str>,
    pub target: Target,
    /// The rectangles the highlight was last drawn for, and in which look.
    shown: (Vec<[Vec3; 4]>, usize),
    highlight: Entity,
    ghost: Entity,
    /// The film that fills up over a wall or room as it is built.
    filling: Entity,
    /// The item itself, growing inside its ghost as it is built.
    rising: Entity,
    looks: [Handle<StandardMaterial>; 3],
    ghost_looks: [Handle<StandardMaterial>; 3],
}

impl Builder {
    pub fn picked(&self) -> &'static Part {
        &PARTS[self.part]
    }

    pub fn variant(&self) -> usize {
        self.variants[self.part]
    }

    /// Which kind of a part is chosen.
    pub fn variant_of(&self, part: usize) -> usize {
        self.variants[part]
    }

    pub fn choose_variant(&mut self, part: usize, variant: usize) {
        if variant < PARTS[part].variants() {
            self.variants[part] = variant;
        }
    }

    /// Whether something is being built or taken apart right now.
    pub fn is_working(&self) -> bool {
        self.work.is_some()
    }

    pub fn pick(&mut self, part: usize) {
        self.part = part.min(PARTS.len() - 1);
        self.stowed = false;
        self.before_mode = None;
    }

    pub fn is_carrying(&self) -> bool {
        self.carrying.is_some()
    }

    /// How far through building or taking apart the builder is, from 0 to 1, and whether
    /// it is taking apart.
    pub fn progress(&self) -> Option<(f32, bool)> {
        self.work
            .as_ref()
            .map(|work| (work.hold.fraction(), work.removing))
    }

    /// Switches to a mode, or back from it to whatever was picked before.
    fn toggle(&mut self, mode: usize) {
        if self.part == mode && !self.stowed {
            match self.before_mode.take() {
                Some(part) => self.part = part,
                None => self.stowed = true,
            }
        } else {
            self.before_mode = (!self.stowed && self.before_mode.is_none()).then_some(self.part);
            self.part = mode;
            self.stowed = false;
        }
    }

    /// The item that would be placed: the one being carried, or the picked part's.
    fn item(&self) -> Option<&'static Item> {
        match (&self.carrying, &self.picked().kind) {
            (Some(carried), _) => Some(carried.item),
            (None, Kind::Items(items)) => Some(&items[self.variant()]),
            _ => None,
        }
    }

    /// The footprint of the room that would be built, in cells.
    fn room(&self) -> Option<IVec2> {
        match self.picked().kind {
            Kind::Room(shapes) => {
                let [east, south] = shapes[self.variant()].metres.map(cells);
                Some(if self.room_turned {
                    IVec2::new(south, east)
                } else {
                    IVec2::new(east, south)
                })
            }
            _ => None,
        }
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, setup).add_systems(
        Update,
        (choose, aim, afford, act, animate, highlight)
            .chain()
            .in_set(Acting),
    );
}

fn setup(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut gizmos: ResMut<GizmoConfigStore>,
) {
    let (lines, _) = gizmos.config_mut::<DefaultGizmoConfigGroup>();
    lines.line.width = 2.5;
    // Drawn a little toward the eye, so a line lying on a floor is not lost in it.
    lines.depth_bias = -0.002;

    // A glowing film, brighter than anything lit, so it reads in sun and in shade.
    let mut film = |colour: Color, strength: f32| {
        let colour = colour.to_linear();
        materials.add(StandardMaterial {
            base_color: Color::LinearRgba(LinearRgba::new(
                colour.red * 2.2,
                colour.green * 2.2,
                colour.blue * 2.2,
                strength,
            )),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            cull_mode: None,
            double_sided: true,
            ..default()
        })
    };
    let colours = [BUILD_COLOUR, OPENING_COLOUR, REFUSED_COLOUR];
    let looks = colours.map(|colour| film(colour, 0.16));
    // A ghost is a whole object, not a patch on a wall, and needs to be seen as one.
    let ghost_looks = colours.map(|colour| film(colour, 0.4));
    let unseen = || {
        (
            MeshMaterial3d(looks[0].clone()),
            Transform::default(),
            Visibility::Hidden,
            NotShadowCaster,
            NotShadowReceiver,
        )
    };
    let highlight = commands
        .spawn((Name::new("Builder highlight"), unseen()))
        .id();
    let ghost = commands.spawn((Name::new("Builder ghost"), unseen())).id();
    let filling = commands
        .spawn((Name::new("Builder progress"), unseen()))
        .id();
    let rising = commands
        .spawn((
            Name::new("Item being built"),
            Transform::default(),
            Visibility::Hidden,
        ))
        .id();
    commands.insert_resource(Builder {
        part: 0,
        stowed: true,
        variants: vec![0; PARTS.len()],
        turn: 0.0,
        room_turned: false,
        wall_turned: false,
        run: Vec::new(),
        snap: true,
        snapping: true,
        viewed: None,
        wheel: 0,
        before_mode: None,
        carrying: None,
        work: None,
        must_release: false,
        pull: false,
        shrinking: None,
        cancel: false,
        price: None,
        short: None,
        unclaimed: None,
        target: Target::Nothing,
        shown: (Vec::new(), 0),
        highlight,
        ghost,
        filling,
        rising,
        looks,
        ghost_looks,
    });
}

/// Reads the keys and the wheel: switching modes, cycling variants, and turning what is
/// in hand. The builder only answers while it is the tool in hand.
fn choose(
    controls: Controls,
    scroll: Res<AccumulatedMouseScroll>,
    dial: Res<Dial>,
    screen: Res<InventoryScreen>,
    belongings: Belongings,
    mut builder: ResMut<Builder>,
) {
    builder.wheel = 0;
    if screen.open || belongings.holding() != Some(Tool::Hammer) {
        // Put back anything being carried, and put the part away.
        builder.cancel = builder.is_carrying();
        builder.stowed = true;
        builder.before_mode = None;
        builder.work = None;
        return;
    }
    // Nothing about the job changes while it is being done.
    if dial.open || builder.work.is_some() {
        return;
    }
    if !builder.is_carrying() {
        if controls.went_down(Action::TakeApart) {
            builder.toggle(catalogue::part_of(|kind| matches!(kind, Kind::Remove)));
        }
        if controls.went_down(Action::MoveItems) {
            builder.toggle(catalogue::part_of(|kind| matches!(kind, Kind::Move)));
        }
    }
    builder.snapping = builder.snap != controls.held(Action::OffGrid);
    if controls.went_down(Action::Grid) {
        builder.snap = !builder.snap;
        if builder.snap {
            builder.turn = (builder.turn / GRID_TURN).round() * GRID_TURN % 360.0;
        }
    }
    if builder.stowed {
        return;
    }
    // Kinds are chosen on the wheel, or stepped through with a tap of R (`dial`).
    // One notch at a time, however the wheel reports it. Turning left and right (Z and C)
    // stand in for the wheel.
    let notch = |amount: f32| (amount > 0.0) as i32 - (amount < 0.0) as i32;
    builder.wheel = notch(scroll.delta.y) + controls.went_down(Action::TurnRight) as i32
        - controls.went_down(Action::TurnLeft) as i32;
    if builder.wheel != 0 {
        if builder.item().is_some() {
            let step = if builder.snapping {
                GRID_TURN
            } else {
                FREE_TURN
            };
            builder.turn = (builder.turn + step * builder.wheel as f32).rem_euclid(360.0);
        } else if builder.room().is_some() {
            builder.room_turned = !builder.room_turned;
        } else if matches!(builder.picked().kind, Kind::Wall(_)) {
            builder.wall_turned = !builder.wall_turned;
        } else if matches!(builder.picked().kind, Kind::Reshape) {
            builder.pull = !builder.pull;
        }
    }
}

/// Works out what the middle of the view is pointed at, within the player's reach.
/// Where on the base's grid a target is, in the base's own space, for checking it against
/// the claim.
fn target_point(target: &Target, props: &Query<&Prop>) -> Option<Vec3> {
    let tile = |tile: [i32; 3]| {
        Vec3::new(
            (tile[0] as f32 + 0.5) * CELL_M,
            0.0,
            (tile[2] as f32 + 0.5) * CELL_M,
        )
    };
    match *target {
        Target::Nothing => None,
        Target::Wall(face) | Target::Edge { face, .. } => Some(tile(face.tile)),
        Target::Ground { corner, tiles, .. } => Some(Vec3::new(
            (corner.x as f32 + tiles.x as f32 * 0.5) * CELL_M,
            0.0,
            (corner.z as f32 + tiles.y as f32 * 0.5) * CELL_M,
        )),
        Target::Spot { at, .. } => Some(at.with_y(0.0)),
        Target::Prop(entity) => props.get(entity).ok().map(|prop| prop.at.with_y(0.0)),
    }
}
