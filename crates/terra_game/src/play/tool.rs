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
//!   Drag left mouse    draw a room, a wall or a roof; let go and its plan is laid,
//!                      a click laying its own size; hold still on a plan to build it
//!   Left click         pick an item up and put it down, in move mode
//!   Scroll wheel       turn an item, a room, a wall, or which way stairs, a ramp or
//!                      a cut block rises                                 (or Z and C)
//!   R                  the next variant of the part
//!   Q                  take-apart mode, and back
//!   G                  move mode, and back
//!   X                  the grid on and off: items snap to it and turn in eighths
//!   Hold Alt           the opposite of the grid setting, while held
//!   Ctrl and wheel     the material walls, floors, roofs and blocks are laid in (D232);
//!                      Ctrl is the roll's key, which waits while a part is in hand
//!   Alt and wheel      the brush: blocks, changing material, carving and taking out
//!                      a box half a metre, 1, 2 or 4 m across (D226)
//!   Middle click       pick another of the item the view is on, or the material of
//!                      the cube it is on                                       (or V)
//!   1 to 9, 0          the first ten parts directly
//!
//! Those are the defaults. Every control but the wheel and the numbers can be set to
//! another key or button in the options menu, and the prompts name whatever it is set to.
//!
//! Whatever the tool would do is shown in the world before you commit: the wall a push
//! would move, the outline of a window, the footprint of a room, or a ghost of the item
//! where it would stand.

use super::catalogue::{Builds, Item, Kind, Part};
use super::controls::Action;
use super::intent::Intent;
use super::inventory::Belongings;
use super::props::{Blocked, Prop};
use super::tick::Sim;
use super::work::Hold;
use super::{IsLocalPlayer, Player};
use crate::build::act::Work as BaseWork;
use bevy::{
    light::{NotShadowCaster, NotShadowReceiver},
    prelude::*,
};
use std::sync::Arc;
use terra_build::ops::Op;
use terra_build::{CELL_M, cells};
use terra_items::ItemKind;

mod act;
mod aim;
mod prompt;
mod show;

use act::{act, animate};
use aim::{afford, aim};
pub use prompt::{legend, prompt, waiting};
use show::{highlight, preview_fit};

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
/// How far, in cells, the aim must move from where a room snapped before it moves on.
const ROOM_STICK_CELLS: f32 = 0.8;
/// How long after building or taking something apart it can be undone, in seconds.
const UNDO_S: f32 = crate::build::act::UNDO_S;
/// How far one notch raises or lowers an item off the grid; on it, a grid step.
const FREE_RAISE_M: f32 = 0.1;
/// How high an item can be raised above where it would stand.
const HIGHEST_RAISE_M: f32 = 3.0;
/// How far the ground can be above or below a room's level for the room to be aimed on
/// the level rather than where the view meets the ground.
const LEVEL_BUMPS_M: f32 = 0.75;
/// How long the button is held still on a plan, without drawing, before it is built.
const HOLD_TO_BUILD_S: f32 = 0.4;
/// The least a drawn room measures across, in cells, so a drag along a line still makes
/// a room that can be stood in: a metre inside its walls, which are a cube thick (D181).
const LEAST_ROOM_CELLS: i32 = 4;
/// How many storeys down and up from the base's floor a room can be placed (D143).
const STOREYS: std::ops::RangeInclusive<i32> = -2..=2;
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
    /// A wall tile, where a wall stands or could (D181); whether one may stand there;
    /// and whether the wall there has a window or door in it.
    Wall {
        tile: IVec3,
        fits: bool,
        open: bool,
    },
    /// Where a room would go: its lowest tile, its size in tiles, whether the ground is
    /// low enough for it to be cut into, whether it would add anything, whether the place
    /// is clear of trees and rocks, and an item set down where it would stand, if any.
    Ground {
        corner: IVec3,
        tiles: IVec2,
        fits: bool,
        adds: bool,
        clear: bool,
        item: Option<Entity>,
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
    /// A cube of the base, planned or built, to build on or take apart.
    Structure {
        cube: IVec3,
    },
    /// A door or window fitted into the base, by its lowest cube.
    Fitted {
        at: [i32; 3],
    },
    /// Where a flight, a post or a block would start: the cube against the face aimed at,
    /// or over the ground (D193).
    Place {
        at: IVec3,
    },
}

/// The point of the grid nearest `raw`, in cells, keeping to `held` until the aim has moved
/// well off it, so it never flickers between two.
fn stick(held: Option<IVec2>, raw: Vec2) -> IVec2 {
    match held {
        Some(held) if (raw - held.as_vec2()).abs().max_element() < ROOM_STICK_CELLS => held,
        _ => raw.round().as_ivec2(),
    }
}

/// What something costs, and how many times over.
type Price = (Vec<(ItemKind, u32)>, u32);

/// An item picked up in move mode, and where it came from in case it is put back.
struct Carried {
    item: Arc<Item>,
    at: Vec3,
    lift: f32,
    turn: f32,
    indoors: bool,
}

/// The button is down on a wall or room: drawing it if the aim has moved, building what
/// is laid if it is held still for long enough.
#[derive(Clone, Copy, Debug)]
struct Drag {
    from: IVec2,
    to: IVec2,
    level: i32,
    /// Whether the aim has moved off where it began: then it is drawing, even if it comes
    /// back.
    moved: bool,
    /// How long it has been held without drawing, in seconds.
    still_s: f32,
}

/// The room between two corners on the grid, as its lowest corner and its size in cells,
/// at least `LEAST_ROOM_CELLS` across in the direction it was drawn.
fn drawn_room(from: IVec2, to: IVec2) -> (IVec2, IVec2) {
    let reach = |from: i32, to: i32| {
        let toward = if to < from { -1 } else { 1 };
        let to = if (to - from).abs() < LEAST_ROOM_CELLS {
            from + toward * LEAST_ROOM_CELLS
        } else {
            to
        };
        (from.min(to), (to - from).abs())
    };
    let (x, east) = reach(from.x, to.x);
    let (z, south) = reach(from.y, to.y);
    (IVec2::new(x, z), IVec2::new(east, south))
}

/// The wall tiles of a wall drawn from one tile toward another, along the grid's four
/// directions only (D146). Both ends included, so it is at least a tile long.
fn drawn_wall(from: IVec2, to: IVec2, level: i32) -> Vec<IVec3> {
    crate::build::plans::wall_tiles(from, to, level)
}

/// What the hammer does to the base (D139, D144): lays a plan, for nothing, at a click;
/// builds what goes in next, takes apart what came last, fits a door or shutters, or mends
/// what is damaged (D305),
/// each held for a moment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Doing {
    Lay,
    Build,
    TakeApart,
    Fit,
    Repair,
}

/// What the hammer would do to the base now, worked out as it is aimed, and whether the
/// base would allow it: the words of a refusal are what the prompt shows.
#[derive(Clone, Debug)]
pub struct Planned {
    pub doing: Doing,
    pub work: BaseWork,
}

/// What is being built or taken apart while the button is held. Everything needed to
/// finish it is taken when the button goes down, so looking away does not change it.
enum Job {
    /// Work on the base, done when the hold is through; still held, the next goes on.
    Base {
        doing: Doing,
        what: String,
        ops: Vec<Op>,
        /// The part being built, so held on, the next of it goes in.
        part: Option<terra_build::PartId>,
        /// The cube being taken apart from, so held on, the next comes out.
        cube: Option<IVec3>,
    },
    Place {
        item: Arc<Item>,
        at: Vec3,
        lift: f32,
        turn: f32,
        indoors: bool,
    },
    TakeAway(Entity),
}

/// The last thing a player built or took apart, which they can undo for a while.
struct Done {
    what: Undoable,
    /// When it was done, in seconds of the game's clock.
    at_s: f32,
}

enum Undoable {
    /// An item built where it stands.
    Placed(Entity),
    /// An item taken apart: what it was and where it stood.
    TakenAway(Carried),
}

/// The next step of work on the base, chosen once the last is in: building on with a part,
/// or taking apart from a cube, from where the player stood, after so many seconds.
struct Following {
    doing: Doing,
    part: Option<terra_build::PartId>,
    cube: Option<IVec3>,
    began_at: Vec3,
    waited_s: f32,
}

struct Work {
    job: Job,
    hold: Hold,
    /// Whether it takes something away, which is shown differently.
    removing: bool,
    /// Where the player stood when it began. Wandering off abandons it.
    began_at: Vec3,
}

/// A player's builder: what is picked, turned and aimed at, and the work in hand.
#[derive(Component)]
pub struct Builder {
    /// Which part is picked, as its place in the catalogue.
    pub part: usize,
    /// Nothing is in hand: the tool does nothing until a part is picked.
    pub stowed: bool,
    /// Whether the builder is the tool in hand, with the hands free to use it.
    in_hand: bool,
    /// The variant last used of each part, by its place in the catalogue; one not here
    /// is the first.
    variants: Vec<usize>,
    /// How far the next item is turned, in degrees.
    turn: f32,
    /// How far above where it would stand the next item is raised, in metres.
    raise: f32,
    /// How many storeys above the base's floor the next room is, below it if negative
    /// (D143).
    storey: i32,
    /// The corner the room in hand last snapped to, east and south in cells, which it
    /// keeps until the aim has moved well off it, so it never flickers between two.
    held_corner: Option<IVec2>,
    /// The point of the grid a wall or room would be drawn from, and its floor, if one is
    /// picked.
    point: Option<(IVec2, i32)>,
    /// A wall or room being drawn, or pressed on.
    drag: Option<Drag>,
    /// Whether the next room is turned a quarter circle.
    room_turned: bool,
    /// Whether the next interior wall runs north and south instead of east and west.
    wall_turned: bool,
    /// How many quarter turns clockwise from north the next flight, stair or slope rises.
    facing: i32,
    /// The wall cells the thing aimed at would change. Empty when it would not fit.
    run: Vec<IVec3>,
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
    /// Still held after a step went off to be worked out (`Building::act_later`): what
    /// comes next once it is in, and how long it has been waited for.
    following: Option<Following>,
    /// The last thing built or taken apart, to undo.
    done: Option<Done>,
    /// Set when something is finished: the button has to come up before the next.
    must_release: bool,
    /// An item shrunk because it is being taken apart, to be put right if that stops.
    shrinking: Option<Entity>,
    /// Set by a tap of the right mouse button: put away, or put back.
    pub cancel: bool,
    /// What building the thing aimed at would cost, and how many times over.
    price: Option<Price>,
    /// What there is not enough of to pay that, and how many more are needed.
    short: Option<(ItemKind, u32)>,
    /// Why the claim forbids building here, if it does (D94).
    unclaimed: Option<&'static str>,
    pub target: Target,
    /// The cube of the base the view is on, within reach, openings included.
    aimed: Option<IVec3>,
    /// What the hammer would do to the base now.
    pub planned: Option<Planned>,
    /// The material, a kind of construction, every plan is laid in (D232): picked on Ctrl
    /// and the wheel, or from a cube with the middle button; none picked, the first kit.
    pub material: Option<terra_build::Kind>,
    /// How many cubes across the brush is that blocks are laid, changed and carved with,
    /// and cubes taken out (D226): 1, 2, 4 or 8, half a metre to four.
    pub brush: i32,
    /// The columns a roof aimed into a room would cover, walls and all: the room's shape,
    /// not the rectangle round it (D241). Empty when a roof is drawn.
    pub footprint: std::collections::BTreeSet<[i32; 2]>,
}

impl Default for Builder {
    fn default() -> Self {
        Self {
            // The first part, a block, in hand (D253).
            part: 0,
            stowed: false,
            in_hand: false,
            variants: Vec::new(),
            turn: 0.0,
            raise: 0.0,
            storey: 0,
            held_corner: None,
            point: None,
            drag: None,
            room_turned: false,
            wall_turned: false,
            facing: 0,
            run: Vec::new(),
            snap: true,
            snapping: true,
            viewed: None,
            wheel: 0,
            before_mode: None,
            carrying: None,
            work: None,
            following: None,
            done: None,
            must_release: false,
            shrinking: None,
            cancel: false,
            price: None,
            short: None,
            unclaimed: None,
            target: Target::Nothing,
            aimed: None,
            planned: None,
            material: None,
            footprint: Default::default(),
            brush: 1,
        }
    }
}

/// How this machine draws its player's builder: the highlight, the ghost of what would be
/// built, and the work going on.
#[derive(Resource)]
pub struct BuilderLook {
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
    /// The cube of the base the view is on, within reach, if any.
    pub fn aimed(&self) -> Option<IVec3> {
        self.aimed
    }

    pub fn picked<'a>(&self, builds: &'a Builds) -> &'a Part {
        builds.part(self.part)
    }

    pub fn variant(&self) -> usize {
        self.variant_of(self.part)
    }

    /// Which kind of a part is chosen.
    pub fn variant_of(&self, part: usize) -> usize {
        self.variants.get(part).copied().unwrap_or(0)
    }

    pub fn choose_variant(&mut self, part: usize, variant: usize, builds: &Builds) {
        if variant < builds.part(part).variants() {
            if self.variants.len() <= part {
                self.variants.resize(part + 1, 0);
            }
            self.variants[part] = variant;
        }
    }

    /// Whether something is being built or taken apart right now.
    pub fn is_working(&self) -> bool {
        self.work.is_some()
    }

    pub fn pick(&mut self, part: usize, builds: &Builds) {
        self.part = part.min(builds.parts().len() - 1);
        self.stowed = false;
        self.before_mode = None;
    }

    pub fn is_carrying(&self) -> bool {
        self.carrying.is_some()
    }

    /// Whether a wall or room is drawn, or being drawn, which a tap of the right button
    /// lets go of.
    pub fn is_drawing(&self) -> bool {
        self.drag.is_some()
    }

    /// What is drawn and should be shown: what is being dragged out.
    fn drawn(&self) -> Option<(IVec2, IVec2, i32)> {
        self.drag
            .filter(|drag| drag.moved)
            .map(|drag| (drag.from, drag.to, drag.level))
    }

    /// Whether the picked part is drawn by dragging: a room or a wall.
    fn draws(&self, builds: &Builds) -> bool {
        matches!(
            self.picked(builds).kind,
            Kind::Room(_) | Kind::Roof(_) | Kind::Wall(_) | Kind::Floor
        )
    }

    /// Which way the next flight, stair or slope rises.
    pub fn facing(&self) -> terra_build::Facing {
        terra_build::Facing::North.turned(self.facing)
    }

    /// Whether the wheel turns which way the part in hand rises.
    fn faces(&self, builds: &Builds) -> bool {
        match &self.picked(builds).kind {
            Kind::Flight(_) => true,
            Kind::Block(shapes) => shapes[self.variant()].1.turns(),
            _ => false,
        }
    }

    /// Whether the mouse wheel turns what is in hand, rather than moving the camera: while
    /// a part is out that the wheel turns or switches, or that is laid in a material, which
    /// Ctrl and the wheel change, or with a brush, which Alt and the wheel size.
    pub fn wants_wheel(&self, builds: &Builds) -> bool {
        !self.stowed
            && (self.item(builds).is_some()
                || self.room(builds).is_some()
                || self.faces(builds)
                || self.lays_material(builds)
                || self.brushes(builds)
                || matches!(self.picked(builds).kind, Kind::Wall(_)))
    }

    /// Whether Ctrl is the material key now, not the roll: a part is in hand (D232).
    pub fn takes_material(&self) -> bool {
        self.in_hand && !self.stowed
    }

    /// Whether the part in hand is laid, or changes cubes, in the material picked.
    pub fn lays_material(&self, builds: &Builds) -> bool {
        matches!(
            self.picked(builds).kind,
            Kind::Room(_)
                | Kind::Roof(_)
                | Kind::Wall(_)
                | Kind::Floor
                | Kind::Flight(_)
                | Kind::Post
                | Kind::Block(_)
                | Kind::Replace
        )
    }

    /// Whether the part in hand works on a box the brush's size (D226): blocks, changing
    /// material, carving and taking out cubes.
    pub fn brushes(&self, builds: &Builds) -> bool {
        matches!(
            self.picked(builds).kind,
            Kind::Block(_) | Kind::Replace | Kind::Carve | Kind::Remove
        )
    }

    /// The brush's size in metres.
    pub fn brush_m(&self) -> f32 {
        self.brush as f32 * CELL_M
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
    fn item<'a>(&'a self, builds: &'a Builds) -> Option<&'a Arc<Item>> {
        match (&self.carrying, &self.picked(builds).kind) {
            (Some(carried), _) => Some(&carried.item),
            (None, Kind::Items(items)) => Some(&items[self.variant()]),
            _ => None,
        }
    }

    /// The footprint of the room or roof that would be built, in cells: a roof's, where it
    /// is not over a room, three metres square.
    fn room(&self, builds: &Builds) -> Option<IVec2> {
        match &self.picked(builds).kind {
            Kind::Roof(_) => Some(IVec2::splat(cells(3.0))),
            Kind::Floor => Some(IVec2::splat(cells(2.0))),
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
    app.register_required_components::<Player, Builder>()
        .add_systems(Startup, setup)
        .add_systems(PostStartup, price)
        .add_systems(
            FixedUpdate,
            (choose, aim, afford, act).chain().in_set(Sim::Act),
        )
        .add_systems(
            Update,
            (animate, highlight, preview_fit).chain().in_set(Acting),
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
    commands.insert_resource(BuilderLook {
        shown: (Vec::new(), 0),
        highlight,
        ghost,
        filling,
        rising,
        looks,
        ghost_looks,
    });
}

/// Tells the base what the catalogue says: what fitting a door or shutters costs, and each
/// claim's budget of planned cubes (D193).
fn price(
    builds: Res<Builds>,
    items: Res<crate::play::inventory::Items>,
    mut rules: ResMut<crate::build::act::BuildRules>,
    mut looks: ResMut<crate::build::props::FittingLooks>,
) {
    // What each door and shutters is drawn with.
    looks.0 = builds
        .fittings()
        .iter()
        .map(|fitting| {
            (
                fitting.kind,
                crate::build::props::FittingLook {
                    path: format!("models/openings/{}.glb", fitting.model),
                    dims: fitting.dims,
                    // Its hinge socket, which the model contract puts at its origin.
                    hinge: crate::play::models::read_sockets(&format!(
                        "openings/{}.glb",
                        fitting.model
                    ))
                    .ok()
                    .and_then(|sockets| crate::play::props::socket(&sockets, "hinge"))
                    .unwrap_or(Vec3::ZERO),
                    frame: fitting.frame.clone(),
                },
            )
        })
        .collect();
    let bill = |cost: &crate::play::inventory::Cost| {
        let mut bill = terra_build::Bill::default();
        for &(kind, count) in cost {
            bill.add(&terra_build::Bill::of(
                items.name(kind),
                terra_build::Amount::whole(u64::from(count)),
            ));
        }
        bill
    };
    // A leaf of each door and shutters (a pair costs two, terra_build::fitting).
    for fitting in builds.fittings() {
        rules.rules.props.insert(fitting.kind, bill(&fitting.cost));
    }
    rules.budgets = builds.claim.planned.clone();
}

/// Reads the keys and the wheel: switching modes, cycling variants, and turning what is
/// in hand. The builder only answers while it is the tool in hand.
fn choose(
    builds: Res<Builds>,
    rules: Res<crate::build::act::BuildRules>,
    look: Res<crate::build::BuildLook>,
    mut belongings: Belongings,
    mut players: Query<(Entity, &Intent, &mut Builder), With<Player>>,
) {
    for (player, intent, mut builder) in &mut players {
        let Some(belongings) = belongings.of(player) else {
            continue;
        };
        builder.wheel = 0;
        let in_hand = !intent.busy && belongings.tool_in_hand().is_some_and(|tool| tool.builds);
        builder.in_hand = in_hand;
        if !in_hand {
            // Put back anything being carried, and put the part away.
            builder.cancel = builder.is_carrying();
            builder.stowed = true;
            builder.before_mode = None;
            builder.work = None;
            builder.drag = None;
            continue;
        }
        // Nothing about the job changes while it is being done.
        if intent.choosing || builder.work.is_some() {
            continue;
        }
        if !builder.is_carrying() {
            if intent.pressed(Action::TakeApart) {
                builder.toggle(builds.part_of(|kind| matches!(kind, Kind::Remove)));
            }
            if intent.pressed(Action::MoveItems) {
                builder.toggle(builds.part_of(|kind| matches!(kind, Kind::Move)));
            }
        }
        builder.snapping = builder.snap != intent.held(Action::OffGrid);
        if intent.pressed(Action::Grid) {
            builder.snap = !builder.snap;
            if builder.snap {
                builder.turn = (builder.turn / GRID_TURN).round() * GRID_TURN % 360.0;
            }
        }
        if builder.stowed {
            continue;
        }
        // Ctrl and the wheel change the material plans are laid in (D232), through every
        // kit there is, as Enshrouded's do: the roll's key, as Shift and the wheel are
        // the sprint's, the roll waiting while a part is in hand. Alt and the wheel change
        // the brush's size (D226).
        if intent.held(Action::Dodge) && intent.wheel != 0 {
            let kinds: Vec<terra_build::Kind> = rules.names.keys().copied().collect();
            if !kinds.is_empty() {
                let now = builder.material.unwrap_or(look.laying);
                let at = kinds.iter().position(|&kind| kind == now).unwrap_or(0) as i32;
                let next = (at + intent.wheel).rem_euclid(kinds.len() as i32) as usize;
                builder.material = Some(kinds[next]);
            }
            continue;
        }
        if intent.held(Action::OffGrid) && intent.wheel != 0 && builder.brushes(&builds) {
            let sizes = [1, 2, 4, 8];
            let at = sizes.iter().position(|&n| n == builder.brush).unwrap_or(0) as i32;
            builder.brush = sizes[(at + intent.wheel).clamp(0, 3) as usize];
            continue;
        }
        // Shift and the wheel, or Page Up and Page Down, raise and lower what is placed: an
        // item by a grid step or a tenth of a metre, a wall or room by a storey (D144).
        let lifting = intent.held(Action::Sprint);
        let height = if lifting { intent.wheel } else { 0 } + intent.pressed(Action::Raise) as i32
            - intent.pressed(Action::Lower) as i32;
        if height != 0 {
            if let Some(item) = builder.item(&builds) {
                if grounded(item) {
                    builder.raise = 0.0;
                } else {
                    let step = if builder.snapping {
                        GRID_M
                    } else {
                        FREE_RAISE_M
                    };
                    builder.raise =
                        (builder.raise + step * height as f32).clamp(0.0, HIGHEST_RAISE_M);
                }
            } else if builder.draws(&builds) {
                builder.storey = (builder.storey + height).clamp(*STOREYS.start(), *STOREYS.end());
            }
        }
        // Kinds are chosen on the wheel, or stepped through with a tap of R (`dial`).
        // One notch at a time, however the wheel reports it. Turning left and right (Z and C)
        // stand in for the wheel.
        builder.wheel = if lifting { 0 } else { intent.wheel }
            + intent.pressed(Action::TurnRight) as i32
            - intent.pressed(Action::TurnLeft) as i32;
        if builder.wheel != 0 {
            if builder.item(&builds).is_some() {
                let step = if builder.snapping {
                    GRID_TURN
                } else {
                    FREE_TURN
                };
                builder.turn = (builder.turn + step * builder.wheel as f32).rem_euclid(360.0);
            } else if builder.faces(&builds) {
                builder.facing = (builder.facing + builder.wheel).rem_euclid(4);
            } else if builder.room(&builds).is_some() {
                builder.room_turned = !builder.room_turned;
            } else if matches!(builder.picked(&builds).kind, Kind::Wall(_)) {
                builder.wall_turned = !builder.wall_turned;
            }
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
        Target::Wall { tile: at, .. } => Some(tile(at.to_array())),
        Target::Ground { corner, tiles, .. } => Some(Vec3::new(
            (corner.x as f32 + tiles.x as f32 * 0.5) * CELL_M,
            corner.y as f32 * terra_build::STOREY_M,
            (corner.z as f32 + tiles.y as f32 * 0.5) * CELL_M,
        )),
        Target::Spot { at, .. } => Some(at.with_y(0.0)),
        Target::Prop(entity) => props.get(entity).ok().map(|prop| prop.at.with_y(0.0)),
        Target::Structure { cube } | Target::Place { at: cube } => Some(tile(cube.to_array())),
        Target::Fitted { at } => Some(tile(at)),
    }
}

/// Whether an item must stand on the ground or a floor, and cannot be raised off it: a
/// station, or a claim stone.
pub fn grounded(item: &Item) -> bool {
    item.station.is_some() || item.claims
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_room_is_the_rectangle_between_its_corners_whichever_way_it_is_drawn() {
        let room = (IVec2::new(-2, 1), IVec2::new(6, 4));
        assert_eq!(drawn_room(IVec2::new(-2, 1), IVec2::new(4, 5)), room);
        assert_eq!(drawn_room(IVec2::new(4, 5), IVec2::new(-2, 1)), room);
        assert_eq!(drawn_room(IVec2::new(4, 1), IVec2::new(-2, 5)), room);
    }

    #[test]
    fn a_room_drawn_along_a_line_is_wide_enough_to_stand_in() {
        let (corner, size) = drawn_room(IVec2::new(0, 0), IVec2::new(6, 0));
        assert_eq!(
            (corner, size),
            (IVec2::new(0, 0), IVec2::new(6, LEAST_ROOM_CELLS))
        );
        let (corner, size) = drawn_room(IVec2::new(0, 0), IVec2::new(-1, -6));
        assert_eq!(corner, IVec2::new(-LEAST_ROOM_CELLS, -6));
        assert_eq!(size, IVec2::new(LEAST_ROOM_CELLS, 6));
    }

    #[test]
    fn a_wall_runs_along_the_grid_the_way_it_was_drawn_furthest() {
        // East, from tile to tile.
        let east = drawn_wall(IVec2::new(1, 2), IVec2::new(4, 3), 0);
        assert_eq!(
            east,
            [
                IVec3::new(1, 0, 2),
                IVec3::new(2, 0, 2),
                IVec3::new(3, 0, 2),
                IVec3::new(4, 0, 2)
            ]
        );
        // North, backwards, on the storey above.
        let north = drawn_wall(IVec2::new(1, 2), IVec2::new(0, 0), 1);
        assert_eq!(
            north,
            [
                IVec3::new(1, 1, 2),
                IVec3::new(1, 1, 1),
                IVec3::new(1, 1, 0)
            ]
        );
    }

    #[test]
    fn a_wall_drawn_back_to_where_it_began_is_a_tile_long() {
        assert_eq!(drawn_wall(IVec2::new(3, 3), IVec2::new(3, 3), 0).len(), 1);
    }
}
