//! The builder tool: what it is aimed at, what it would do there, and doing it.
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

use super::bag::Notice;
use super::catalogue::{self, Item, Kind, PARTS, Part};
use super::claim::Claim;
use super::controls::{Action, Controls, Settings, capitalised};
use super::dial::Dial;
use super::flora::Obstacles;
use super::inventory::{self, Belongings, Cost, InventoryScreen, Raw, Tool};
use super::props::{self, Blocked, Models, Prop, Standing};
use super::skills::{Did, ONCE, Skill};
use super::work::Hold;
use super::{Player, View};
use crate::experiments::build::Site;
use crate::planet::Planet;
use crate::shell::Options;
use bevy::{
    asset::RenderAssetUsages,
    input::mouse::AccumulatedMouseScroll,
    light::{NotShadowCaster, NotShadowReceiver},
    prelude::*,
    render::render_resource::PrimitiveTopology,
    window::{CursorGrabMode, CursorOptions},
};
use terra_build::{
    Base, BuildOp, CELL_M, DOOR_HEIGHT_M, DOOR_WIDTH_M, Face, Panel, STOREY_M, Side, cells,
};
use terra_core::Ground;

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
    /// low enough for its floor to clear it, and whether it would add anything.
    Ground {
        corner: IVec3,
        tiles: IVec2,
        fits: bool,
        adds: bool,
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
fn aim(
    planet: Res<Planet>,
    site: Res<Site>,
    options: Res<Options>,
    dial: Res<Dial>,
    view: Res<View>,
    models: Res<Models>,
    obstacles: Res<Obstacles>,
    camera: Single<&GlobalTransform, With<Camera3d>>,
    player: Single<&Transform, With<Player>>,
    props: Query<(Entity, &Prop, &Transform, &Visibility)>,
    mut builder: ResMut<Builder>,
) {
    // What is being worked on stays the target until the work is done or dropped.
    if builder.work.is_some() {
        return;
    }
    builder.viewed = None;
    // A fixed camera is for taking pictures, and an open dial is for choosing.
    if options.vantage.is_some() || dial.open {
        builder.target = Target::Nothing;
        return;
    }
    let to_base = site.frame.compute_affine().inverse();
    let origin = camera.translation();
    let direction = *camera.forward();
    let local_origin = to_base.transform_point3(origin);
    let local_direction = to_base.transform_vector3(direction).normalize();
    let within = |point: Vec3, reach: f32| point.distance(player.translation) <= reach;
    // The first of the base's walls, floors and roofs the view meets, interior walls
    // included; the ground; and the first item.
    let hull = site.base.raycast(local_origin, local_direction, 200.0);
    let ground = planet.ground.raycast(origin, direction, 200.0);
    let met = |distance: f32| local_origin + local_direction * distance;
    let outside_wall = hull
        .filter(|hit| !hit.partition && hit.face.side.is_wall())
        .filter(|hit| within(origin + direction * hit.distance, STRUCTURE_REACH_M));
    let limit = hull.map_or(f32::MAX, |hit| hit.distance + 0.05);
    let item = props
        .iter()
        .filter(|(_, _, _, visibility)| **visibility != Visibility::Hidden)
        .filter_map(|(entity, prop, transform, _)| {
            props::ray_box(transform, prop.half, origin, direction)
                .map(|(distance, side)| (entity, prop, distance, side))
        })
        .filter(|&(_, _, distance, _)| {
            distance < limit && within(origin + direction * distance, ITEM_REACH_M)
        })
        .min_by(|a, b| a.2.total_cmp(&b.2));
    builder.viewed = item.map(|(entity, ..)| entity);
    if builder.stowed {
        builder.target = Target::Nothing;
        return;
    }

    // Where an item would stand. On top of an item the view is on, or flush against its
    // side; else the floor, or flush against a wall; else the ground outside. If none of
    // that is within reach, a few steps in front of the player, so there is always
    // something to see.
    let spot = || {
        let half = builder
            .item()
            .and_then(|item| models.of(item))
            .map_or(Vec3::splat(0.3), |model| model.half);
        let boxed = |at: Vec3, lift: f32, indoors: bool| Standing {
            at,
            lift,
            turn: builder.turn,
            half,
            indoors,
        };
        let on_grid = |at: Vec3| {
            if builder.snapping {
                ((at / GRID_M).round() * GRID_M).with_y(at.y)
            } else {
                at
            }
        };
        let on_item =
            item.filter(|(_, prop, ..)| prop.is_solid())
                .and_then(|(_, prop, distance, side)| {
                    let touched = met(distance);
                    let out = to_base.transform_vector3(side).normalize();
                    if out.y > 0.7 {
                        // Its top. On the grid that means the middle of it.
                        let at = if builder.snapping {
                            prop.at
                        } else {
                            touched.with_y(prop.at.y)
                        };
                        Some((at, prop.lift + prop.half.y * 2.0, prop.indoors))
                    } else if out.y.abs() < 0.3 {
                        // A side: stand flush beside it, lined up with it if nearly so.
                        let out = out.with_y(0.0).normalize();
                        let along_side = out.cross(Vec3::Y);
                        let mut along = (touched - prop.at).dot(along_side);
                        if builder.snapping && along.abs() < 0.35 {
                            along = 0.0;
                        }
                        let its_reach = Standing::of(prop).reach_toward(out);
                        let ours = boxed(prop.at, prop.lift, prop.indoors).reach_toward(out);
                        let at = prop.at + along_side * along + out * (its_reach + ours + 0.02);
                        Some((at, prop.lift, prop.indoors))
                    } else {
                        None
                    }
                });
        let on_ground = ground
            .filter(|ground| hull.is_none_or(|hit| ground.distance < hit.distance))
            .map(|ground| (on_grid(to_base.transform_point3(ground.point)), 0.0, false));
        let in_base = hull.and_then(|hit| {
            let floor = hit.face.tile[1] as f32 * STOREY_M;
            let touched = met(hit.distance);
            match hit.face.side {
                Side::Down => Some((on_grid(touched), 0.0, true)),
                Side::Up => None,
                side => {
                    // Flush against the wall, on whichever side of it the view is, and
                    // on the grid along it.
                    let out = side.offset().as_vec3();
                    let toward_viewer = -out * local_direction.dot(out).signum();
                    let tile = ((touched + toward_viewer * 0.2) / CELL_M)
                        .floor()
                        .as_ivec3()
                        .with_y(hit.face.tile[1]);
                    let indoors = site.base.contains(tile);
                    let along = on_grid(touched);
                    let gap = if hit.partition { 0.08 } else { 0.02 };
                    let against = boxed(touched, 0.0, indoors).reach_toward(out) + gap;
                    let at = along + out * (touched - along).dot(out) + toward_viewer * against;
                    Some((at.with_y(if indoors { floor } else { 0.0 }), 0.0, indoors))
                }
            }
        });
        let ahead = || {
            let feet = to_base.transform_point3(player.translation);
            let forward = to_base
                .transform_vector3(view.forward)
                .with_y(0.0)
                .normalize_or_zero();
            let at = on_grid(feet + forward * ITEM_NEAR_M);
            let tile = (at / CELL_M).floor().as_ivec3().with_y(0);
            let indoors = site.base.contains(tile) && feet.y < STOREY_M * 0.5;
            (at.with_y(0.0), 0.0, indoors)
        };
        let (at, lift, indoors) = on_item
            .or(on_ground)
            .or(in_base)
            .filter(|(at, ..)| within(site.frame.transform_point(*at), ITEM_REACH_M))
            .unwrap_or_else(ahead);
        let world = site.frame.transform_point(at);
        let wet = !indoors && planet.drawn(world.normalize()).length() < planet.sea_radius_m();
        let blocked = props::blocked(
            &site,
            &boxed(at, lift, indoors),
            props
                .iter()
                .filter(|(_, _, _, visibility)| **visibility != Visibility::Hidden)
                .map(|(entity, prop, _, _)| (entity, prop)),
            &obstacles.0,
        )
        .or(wet.then_some(Blocked::Water));
        Target::Spot {
            at,
            lift,
            indoors,
            blocked,
        }
    };

    let edge = |face: Face| Target::Edge {
        face,
        fits: site.base.can_partition(face),
        open: site
            .base
            .partition(face)
            .is_some_and(|panel| panel != Panel::Solid),
    };
    // So many cells of one wall, centred on one of them. Empty if the wall does not run
    // that far, or changes from outside wall to interior on the way.
    let stretch = |face: Face, count: i32| -> Vec<Face> {
        let along = face.side.along();
        let first = face.tile() - along * ((count - 1) / 2);
        let covered: Vec<Face> = (0..count)
            .map(|step| Face::new(first + along * step, face.side))
            .collect();
        let outside = site.base.is_hull(face);
        if covered
            .iter()
            .all(|&cell| site.base.is_hull(cell) == outside && site.base.filling(cell).is_some())
        {
            covered
        } else {
            Vec::new()
        }
    };
    // Any upright wall the view is on, outside or interior, within reach.
    let any_wall = hull
        .filter(|hit| hit.face.side.is_wall())
        .filter(|hit| within(origin + direction * hit.distance, STRUCTURE_REACH_M));
    let opening_on = |width_m: f32, run: &mut Vec<Face>| {
        any_wall.map_or(Target::Nothing, |hit| {
            *run = stretch(hit.face, cells(width_m));
            if hit.partition {
                edge(hit.face)
            } else {
                Target::Wall(hit.face)
            }
        })
    };
    let mut run = Vec::new();
    builder.target = match builder.picked().kind {
        Kind::Reshape => outside_wall.map_or(Target::Nothing, |hit| Target::Wall(hit.face)),
        // A window or a door goes in an outside wall or an interior one alike.
        Kind::Window(shapes) => opening_on(shapes[builder.variant()].1, &mut run),
        Kind::Door => opening_on(DOOR_WIDTH_M, &mut run),
        Kind::Wall(lengths) => {
            // On the grid line nearest the floor the view is on; or, looking at no
            // floor, a couple of steps ahead. The wheel turns it.
            let on_floor = hull
                .filter(|hit| !hit.partition && hit.face.side == Side::Down)
                .filter(|hit| within(origin + direction * hit.distance, STRUCTURE_REACH_M))
                .map(|hit| met(hit.distance).with_y(hit.face.tile[1] as f32 * STOREY_M + 0.01))
                .unwrap_or_else(|| {
                    let feet = to_base.transform_point3(player.translation);
                    let forward = to_base
                        .transform_vector3(view.forward)
                        .with_y(0.0)
                        .normalize_or_zero();
                    let storey = (feet.y / STOREY_M).round();
                    (feet + forward * 2.0).with_y(storey * STOREY_M + 0.01)
                });
            let cell = (on_floor / CELL_M).floor().as_ivec3();
            let across = on_floor / CELL_M - cell.as_vec3();
            // A wall stands on the east or south side of a cell.
            let face = if builder.wall_turned {
                Face::new(cell - IVec3::X * i32::from(across.x < 0.5), Side::East)
            } else {
                Face::new(cell - IVec3::Z * i32::from(across.z < 0.5), Side::South)
            };
            let count = cells(lengths[builder.variant()].1);
            let along = face.side.along();
            let first = face.tile() - along * ((count - 1) / 2);
            run = (0..count)
                .map(|step| Face::new(first + along * step, face.side))
                .collect();
            Target::Edge {
                face,
                fits: run.iter().all(|&cell| site.base.can_partition(cell)),
                open: false,
            }
        }
        Kind::Room(_) => {
            // On open ground, or across a room already there, which it then extends.
            let on_base = hull
                .filter(|hit| ground.is_none_or(|ground| hit.distance < ground.distance))
                .map(|hit| site.frame.transform_point(met(hit.distance).with_y(0.0)));
            on_base
                .or(ground.map(|ground| ground.point))
                .filter(|point| within(*point, STRUCTURE_REACH_M))
                .zip(builder.room())
                .map_or(Target::Nothing, |(point, size)| {
                    // Centred on the place aimed at.
                    let local = to_base.transform_point3(point);
                    let corner = IVec3::new(
                        (local.x / CELL_M - size.x as f32 * 0.5).round() as i32,
                        0,
                        (local.z / CELL_M - size.y as f32 * 0.5).round() as i32,
                    );
                    // The room's floor is the base's floor. Its foundation may go into
                    // the ground; the room itself may not. Checked every metre or so.
                    let every = cells(1.0).max(1) as usize;
                    let fits = (0..=size.x).step_by(every).chain([size.x]).all(|i| {
                        (0..=size.y).step_by(every).chain([size.y]).all(|j| {
                            let on_floor = (corner + IVec3::new(i, 0, j)).as_vec3() * CELL_M;
                            planet.ground.distance(site.frame.transform_point(on_floor)) > 0.02
                        })
                    });
                    let adds = (0..size.x).any(|i| {
                        (0..size.y).any(|j| !site.base.contains(corner + IVec3::new(i, 0, j)))
                    });
                    Target::Ground {
                        corner,
                        tiles: size,
                        fits,
                        adds,
                    }
                })
        }
        Kind::Items(_) => spot(),
        Kind::Move if builder.is_carrying() => spot(),
        Kind::Move => item.map_or(Target::Nothing, |(entity, ..)| Target::Prop(entity)),
        Kind::Remove => match (item, hull, outside_wall) {
            (Some((entity, ..)), _, _) => Target::Prop(entity),
            (None, Some(hit), _) if hit.partition => {
                let target = edge(hit.face);
                run = if matches!(target, Target::Edge { open: true, .. }) {
                    // The window or doorway in it.
                    site.base.run(hit.face)
                } else {
                    // The whole straight stretch of plain wall.
                    let along = hit.face.side.along();
                    let plain = |cell: IVec3| {
                        site.base.partition(Face::new(cell, hit.face.side)) == Some(Panel::Solid)
                    };
                    let mut first = hit.face.tile();
                    while plain(first - along) {
                        first -= along;
                    }
                    let mut found = Vec::new();
                    while plain(first) {
                        found.push(Face::new(first, hit.face.side));
                        first += along;
                    }
                    found
                };
                target
            }
            (None, _, Some(hit)) if site.base.panel(hit.face) != Panel::Solid => {
                run = site.base.run(hit.face);
                Target::Wall(hit.face)
            }
            _ => Target::Nothing,
        },
    };
    builder.run = run;
}

/// Works out what the thing aimed at would cost to build, and whether it can be paid.
fn afford(
    site: Res<Site>,
    claim: Res<Claim>,
    props: Query<&Prop>,
    belongings: Belongings,
    mut builder: ResMut<Builder>,
) {
    if builder.work.is_some() {
        return;
    }
    // Nothing is built or taken apart outside your claim (D94), except the claim stone
    // that makes one; and there is one claim each.
    let placing_claim = matches!(builder.picked().kind, Kind::Items(_))
        && builder.item().is_some_and(|item| item.claims)
        && !builder.is_carrying();
    let on_planet =
        target_point(&builder.target, &props).map(|point| site.frame.transform_point(point));
    builder.unclaimed = match on_planet {
        Some(_) if placing_claim && claim.stone.is_some() => {
            Some("You already have a claim; take its stone away to move it")
        }
        Some(_) if placing_claim => None,
        Some(point) if !claim.covers(point) => Some(if claim.stone.is_none() {
            "Place a claim stone to build here: Craft, then Claim stone"
        } else {
            "That is outside your claim"
        }),
        _ => None,
    };
    // Cutting an opening is paid for where the wall was solid; changing the shape of
    // one already there is free.
    let cuts = builder
        .run
        .iter()
        .any(|&cell| site.base.filling(cell) == Some(Panel::Solid));
    let price: Option<(Cost, u32)> = match (&builder.picked().kind, builder.target) {
        (
            Kind::Room(_),
            Target::Ground {
                corner,
                tiles,
                fits: true,
                adds: true,
            },
        ) => {
            let new_cells = (0..tiles.x)
                .flat_map(|i| (0..tiles.y).map(move |j| IVec3::new(i, 0, j)))
                .filter(|&step| !site.base.contains(corner + step))
                .count();
            Some((catalogue::FLOOR, catalogue::floor_units(new_cells)))
        }
        (Kind::Reshape, Target::Wall(face)) if !builder.pull => Some((
            catalogue::FLOOR,
            catalogue::floor_units(site.base.wall(face).len()),
        )),
        (Kind::Wall(_), Target::Edge { fits: true, .. }) => {
            let new_cells = builder
                .run
                .iter()
                .filter(|&&cell| site.base.partition(cell).is_none())
                .count();
            (new_cells > 0).then(|| (catalogue::WALL, catalogue::wall_units(new_cells)))
        }
        (Kind::Window(_), Target::Wall(_) | Target::Edge { .. }) if cuts => Some((
            catalogue::WINDOW,
            catalogue::window_units(builder.run.len()),
        )),
        (Kind::Door, Target::Wall(_) | Target::Edge { .. }) if cuts => Some((catalogue::DOOR, 1)),
        (Kind::Items(_), Target::Spot { blocked: None, .. }) => {
            builder.item().map(|item| (item.cost, 1))
        }
        _ => None,
    };
    builder.short = price.and_then(|(cost, times)| belongings.missing(cost, times));
    builder.price = price;
}

/// Starts, carries on and finishes building and taking apart. Both take a moment: the
/// button is held until the job is done, and letting go early leaves nothing done.
fn act(
    mut commands: Commands,
    time: Res<Time>,
    controls: Controls,
    dial: Res<Dial>,
    planet: Res<Planet>,
    models: Res<Models>,
    props: Query<&Prop>,
    mut builder: ResMut<Builder>,
    mut belongings: Belongings,
    mut notice: ResMut<Notice>,
    mut site: ResMut<Site>,
    mut did: MessageWriter<Did>,
    player: Single<(&Transform, &mut Player)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let (player_at, mut player) = player.into_inner();
    let player_at = player_at.translation;
    // A tap of the right button: stop what is being done; or put back what was picked
    // up; or put the part away.
    if std::mem::take(&mut builder.cancel) {
        if builder.work.take().is_some() {
            return;
        }
        match builder.carrying.take() {
            Some(carried) => {
                props::place(
                    &mut commands,
                    &models,
                    &site,
                    &planet,
                    carried.item,
                    carried.at,
                    carried.lift,
                    carried.turn,
                    carried.indoors,
                );
            }
            None => builder.stowed = true,
        }
        return;
    }
    let held = controls.held(Action::Use);
    let click = controls.went_down(Action::Use);
    if !held {
        builder.must_release = false;
    }

    // Carry on with what is under way, or drop it.
    if let Some(mut work) = builder.work.take() {
        let stayed = work.began_at.distance(player_at) < WANDER_M;
        let in_hand = !dial.open && belongings.holding() == Some(Tool::Hammer);
        if !held || !stayed || !in_hand {
            return;
        }
        player.since_acting = 0.0;
        if !work.hold.advance(time.delta_secs()) {
            builder.work = Some(work);
            return;
        }
        builder.must_release = true;
        match work.job {
            Job::Structure { ops, pay, refund } => {
                // As in Subnautica, nothing is taken apart if what it gives back would
                // not fit in the backpack.
                if let Some((cost, times)) = refund
                    && !belongings.room_for(cost, times)
                {
                    notice.say("No room in your backpack for what that gives back");
                    return;
                }
                if site.apply_all(&ops, &mut commands, &planet, &mut meshes) {
                    if pay.is_some() {
                        did.write(Did(Skill::Building, ONCE));
                    }
                    if let Some((cost, times)) = refund {
                        belongings.refund(cost, times);
                    }
                    if let Some((cost, times)) = pay {
                        belongings.pay(cost, times);
                    }
                }
            }
            Job::Place {
                item,
                at,
                lift,
                turn,
                indoors,
            } => {
                // Paid for on finishing. If what it costs has gone since the button
                // went down, nothing is built.
                if belongings.pay(item.cost, 1) {
                    props::place(
                        &mut commands,
                        &models,
                        &site,
                        &planet,
                        item,
                        at,
                        lift,
                        turn,
                        indoors,
                    );
                    info!("Placed a {} at {at}, {lift} up.", item.name);
                    did.write(Did(Skill::Building, ONCE));
                }
            }
            Job::TakeAway(entity) => {
                if let Ok(prop) = props.get(entity) {
                    if belongings.refund(prop.item.cost, 1) {
                        info!("Removed a {}.", prop.item.name);
                        commands.entity(entity).despawn();
                    } else {
                        notice.say("No room in your backpack for what that gives back");
                    }
                }
            }
        }
        return;
    }

    // The middle button: another of whatever the view is on.
    if controls.went_down(Action::Copy)
        && !builder.is_carrying()
        && belongings.holding() == Some(Tool::Hammer)
        && let Some(prop) = builder.viewed.and_then(|entity| props.get(entity).ok())
        && let Some((part, variant)) = catalogue::place_of(prop.item)
    {
        builder.pick(part);
        builder.variants[part] = variant;
        builder.turn = prop.turn;
        return;
    }
    if !click || builder.must_release {
        return;
    }

    if let Some(reason) = builder.unclaimed {
        notice.say(reason);
        return;
    }

    // Moving is neither building nor taking apart, and happens at once.
    match (&builder.picked().kind, builder.target) {
        (Kind::Move, Target::Prop(entity)) => {
            if let Ok(prop) = props.get(entity) {
                info!("Picked up a {}.", prop.item.name);
                builder.turn = prop.turn;
                builder.carrying = Some(Carried {
                    item: prop.item,
                    at: prop.at,
                    lift: prop.lift,
                    turn: prop.turn,
                    indoors: prop.indoors,
                });
                commands.entity(entity).despawn();
                player.since_acting = 0.0;
            }
            return;
        }
        (
            Kind::Move,
            Target::Spot {
                at,
                lift,
                indoors,
                blocked: None,
            },
        ) => {
            if let Some(carried) = builder.carrying.take() {
                props::place(
                    &mut commands,
                    &models,
                    &site,
                    &planet,
                    carried.item,
                    at,
                    lift,
                    builder.turn,
                    indoors,
                );
                info!("Placed a {} at {at}, {lift} up.", carried.item.name);
                player.since_acting = 0.0;
            }
            return;
        }
        _ => {}
    }

    // Building is paid for from what is carried, and refused when that is not enough.
    // Taking apart gives everything back.
    let can_pay = builder.short.is_none();
    let pay = builder.price;
    // What closing a window or doorway gives back.
    let opening_worth = |run: &[Face]| match run.first().and_then(|&cell| site.base.filling(cell)) {
        Some(Panel::Window { .. }) => Some((catalogue::WINDOW, catalogue::window_units(run.len()))),
        Some(Panel::Door) => Some((catalogue::DOOR, 1)),
        _ => None,
    };
    // Fills every cell of the run, in an outside wall or an interior one.
    let fill = |run: &[Face], panel: Panel| -> Vec<BuildOp> {
        run.iter()
            .map(|&cell| {
                if site.base.is_hull(cell) {
                    BuildOp::SetPanel(cell, panel)
                } else {
                    BuildOp::SetPartition(cell, Some(panel))
                }
            })
            .collect()
    };
    let building = |ops: Vec<BuildOp>| Job::Structure {
        ops,
        pay,
        refund: None,
    };
    let unbuilding = |ops: Vec<BuildOp>, refund: Option<(Cost, u32)>| Job::Structure {
        ops,
        pay: None,
        refund,
    };
    let (job, removing) = match (&builder.picked().kind, builder.target) {
        (Kind::Reshape, Target::Wall(face)) if !builder.pull && can_pay => {
            (building(vec![BuildOp::Grow(face)]), false)
        }
        (Kind::Reshape, Target::Wall(face)) if builder.pull => {
            let floor = catalogue::floor_units(site.base.wall(face).len());
            let refund = Some((catalogue::FLOOR, floor));
            (unbuilding(vec![BuildOp::Shrink(face)], refund), true)
        }
        (Kind::Window(shapes), Target::Wall(_) | Target::Edge { .. }) if can_pay => (
            building(fill(&builder.run, shapes[builder.variant()].2)),
            false,
        ),
        (Kind::Door, Target::Wall(_) | Target::Edge { .. }) if can_pay => {
            (building(fill(&builder.run, Panel::Door)), false)
        }
        // A wall is built where there is none, and leaves alone what is already there.
        (Kind::Wall(_), Target::Edge { fits: true, .. }) if can_pay => (
            building(
                builder
                    .run
                    .iter()
                    .filter(|&&cell| site.base.partition(cell).is_none())
                    .map(|&cell| BuildOp::SetPartition(cell, Some(Panel::Solid)))
                    .collect(),
            ),
            false,
        ),
        // An opening is closed first; a plain interior wall is taken away.
        (Kind::Remove, Target::Wall(_) | Target::Edge { open: true, .. }) => (
            unbuilding(
                fill(&builder.run, Panel::Solid),
                opening_worth(&builder.run),
            ),
            true,
        ),
        (Kind::Remove, Target::Edge { .. }) => (
            unbuilding(
                builder
                    .run
                    .iter()
                    .map(|&cell| BuildOp::SetPartition(cell, None))
                    .collect(),
                Some((catalogue::WALL, catalogue::wall_units(builder.run.len()))),
            ),
            true,
        ),
        (
            Kind::Room(_),
            Target::Ground {
                corner,
                tiles,
                fits: true,
                adds: true,
            },
        ) if can_pay => (
            building(vec![BuildOp::AddRoom {
                min: corner.to_array(),
                max: (corner + IVec3::new(tiles.x - 1, 0, tiles.y - 1)).to_array(),
            }]),
            false,
        ),
        (
            Kind::Items(items),
            Target::Spot {
                at,
                lift,
                indoors,
                blocked: None,
            },
        ) if can_pay => (
            Job::Place {
                item: &items[builder.variant()],
                at,
                lift,
                turn: builder.turn,
                indoors,
            },
            false,
        ),
        (Kind::Remove, Target::Prop(entity)) => (Job::TakeAway(entity), true),
        _ => return,
    };
    // How long it takes follows from what it costs.
    let seconds = match &job {
        Job::Structure { ops, .. } if ops.is_empty() => return,
        Job::Structure {
            pay: Some((cost, times)),
            ..
        } => catalogue::build_seconds(cost, *times),
        Job::Structure {
            refund: Some((cost, times)),
            ..
        } => catalogue::take_apart_seconds(cost, *times),
        // Changing the shape of an opening already there costs nothing and is quick.
        Job::Structure { .. } => catalogue::build_seconds(&[], 1),
        Job::Place { item, .. } => catalogue::build_seconds(item.cost, 1),
        Job::TakeAway(entity) => match props.get(*entity) {
            Ok(prop) => catalogue::take_apart_seconds(prop.item.cost, 1),
            Err(_) => return,
        },
    };
    builder.work = Some(Work {
        job,
        hold: Hold::new(seconds),
        removing,
        began_at: player_at,
    });
}

/// Shows work on the thing itself: an item being built grows inside its ghost, and one
/// being taken apart shrinks. Either goes back to how it was if the work is dropped.
fn animate(
    mut commands: Commands,
    site: Res<Site>,
    planet: Res<Planet>,
    models: Res<Models>,
    mut builder: ResMut<Builder>,
    mut props: Query<&mut Transform, With<Prop>>,
) {
    let (growing, taking) = match &builder.work {
        Some(Work {
            job:
                Job::Place {
                    item,
                    at,
                    lift,
                    turn,
                    indoors,
                },
            hold,
            ..
        }) => (
            Some((*item, *at, *lift, *turn, *indoors, hold.fraction())),
            None,
        ),
        Some(Work {
            job: Job::TakeAway(entity),
            hold,
            ..
        }) => (None, Some((*entity, hold.fraction()))),
        _ => (None, None),
    };
    let rising = builder.rising;
    match growing.and_then(|growing| Some((growing, models.of(growing.0)?.solid.clone()?))) {
        Some(((item, at, lift, turn, indoors, done), mesh)) => {
            let stood = props::standing(&site, &planet, at, lift + item.lift, turn, indoors);
            // Quick at first, easing to full size.
            let eased = 1.0 - (1.0 - done) * (1.0 - done);
            commands.entity(rising).insert((
                Mesh3d(mesh),
                MeshMaterial3d(models.lit()),
                stood.with_scale(Vec3::splat(0.2 + 0.8 * eased)),
                Visibility::Visible,
            ));
        }
        None => {
            commands.entity(rising).insert(Visibility::Hidden);
        }
    }
    if let Some(was) = builder.shrinking
        && taking.map(|(entity, _)| entity) != Some(was)
    {
        if let Ok(mut transform) = props.get_mut(was) {
            transform.scale = Vec3::ONE;
        }
        builder.shrinking = None;
    }
    if let Some((entity, done)) = taking
        && let Ok(mut transform) = props.get_mut(entity)
    {
        transform.scale = Vec3::splat(1.0 - 0.7 * done * done);
        builder.shrinking = Some(entity);
    }
}

/// The rectangle a stretch of wall cells covers, in the base's own space: low-left,
/// low-right, high-right, high-left, as seen from the side the wall faces.
fn bounds(run: &[Face]) -> Option<[Vec3; 4]> {
    let [origin, right, _, above] = Base::corners(*run.first()?);
    let across = (right - origin).normalize();
    let up = (above - origin).normalize();
    let (mut low, mut high) = (Vec2::MAX, Vec2::MIN);
    for corner in run.iter().flat_map(|&cell| Base::corners(cell)) {
        let at = Vec2::new((corner - origin).dot(across), (corner - origin).dot(up));
        low = low.min(at);
        high = high.max(at);
    }
    let at = |x: f32, y: f32| origin + across * x + up * y;
    Some([
        at(low.x, low.y),
        at(high.x, low.y),
        at(high.x, high.y),
        at(low.x, high.y),
    ])
}

/// The outline of the window or doorway a stretch of wall cells would be cut as.
fn opening(run: &[Face], panel: Panel) -> Option<[Vec3; 4]> {
    let [low_left, low_right, _, high_left] = bounds(run)?;
    let up = (high_left - low_left).normalize();
    let (bottom, top) = match panel {
        Panel::Solid => return None,
        Panel::Window { sill, top } => (sill, STOREY_M - top),
        Panel::Door => (0.0, DOOR_HEIGHT_M),
    };
    Some([
        low_left + up * bottom,
        low_right + up * bottom,
        low_right + up * top,
        low_left + up * top,
    ])
}

/// Shows what the tool would act on.
fn highlight(
    mut commands: Commands,
    site: Res<Site>,
    planet: Res<Planet>,
    models: Res<Models>,
    props: Query<(&Prop, &Transform)>,
    mut builder: ResMut<Builder>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut gizmos: Gizmos,
) {
    let kind = &builder.picked().kind;
    let refused = matches!(
        builder.target,
        Target::Ground { fits: false, .. }
            | Target::Ground { adds: false, .. }
            | Target::Edge { fits: false, .. }
            | Target::Spot {
                blocked: Some(_),
                ..
            }
    ) || builder.short.is_some()
        || builder.unclaimed.is_some()
        || (matches!(kind, Kind::Window(_) | Kind::Door)
            && matches!(builder.target, Target::Wall(_) | Target::Edge { .. })
            && builder.run.is_empty());
    let pulling = matches!(kind, Kind::Reshape) && builder.pull;
    let look = match (kind, builder.target) {
        (Kind::Move, Target::Prop(_)) => 1,
        _ if refused || pulling => 2,
        (Kind::Remove, _) => 2,
        (Kind::Window(_) | Kind::Door | Kind::Move, _) => 1,
        _ => 0,
    };
    let colour = [BUILD_COLOUR, OPENING_COLOUR, REFUSED_COLOUR][look];
    let in_world = |corners: [Vec3; 4]| corners.map(|corner| site.frame.transform_point(corner));
    let outline = |gizmos: &mut Gizmos, [a, b, c, d]: [Vec3; 4], colour: Color| {
        gizmos.linestrip([a, b, c, d, a], colour);
    };
    let boxed = |gizmos: &mut Gizmos, half: Vec3, stood: &Transform, colour: Color| {
        for height in [0.02, half.y * 2.0] {
            let corner =
                |x: f32, z: f32| stood.transform_point(Vec3::new(x * half.x, height, z * half.z));
            outline(
                gizmos,
                [
                    corner(-1.0, -1.0),
                    corner(1.0, -1.0),
                    corner(1.0, 1.0),
                    corner(-1.0, 1.0),
                ],
                colour,
            );
        }
    };

    // The rectangles to cover with film, in the base's own space.
    let quads: Vec<[Vec3; 4]> = match (kind, builder.target) {
        (Kind::Reshape, Target::Wall(face)) => bounds(&site.base.wall(face)).into_iter().collect(),
        // A window or door that will not fit: the cell aimed at, in red.
        (Kind::Window(_) | Kind::Door, Target::Wall(face) | Target::Edge { face, .. })
            if builder.run.is_empty() =>
        {
            vec![Base::corners(face)]
        }
        (Kind::Window(shapes), Target::Wall(_) | Target::Edge { .. }) => {
            opening(&builder.run, shapes[builder.variant()].2)
                .into_iter()
                .collect()
        }
        (Kind::Door, Target::Wall(_) | Target::Edge { .. }) => {
            opening(&builder.run, Panel::Door).into_iter().collect()
        }
        // An opening to close, or a stretch of interior wall to build or take away.
        (Kind::Remove, Target::Wall(_) | Target::Edge { open: true, .. }) => builder
            .run
            .first()
            .and_then(|&cell| site.base.filling(cell))
            .and_then(|panel| opening(&builder.run, panel))
            .into_iter()
            .collect(),
        (_, Target::Edge { .. }) => bounds(&builder.run).into_iter().collect(),
        (_, Target::Ground { corner, tiles, .. }) => {
            let low = corner.as_vec3() * CELL_M;
            let high = low + Vec3::new(tiles.x as f32 * CELL_M, STOREY_M, tiles.y as f32 * CELL_M);
            let at = |x: bool, y: bool, z: bool| {
                Vec3::new(
                    if x { high.x } else { low.x },
                    if y { high.y } else { low.y },
                    if z { high.z } else { low.z },
                )
            };
            vec![
                [
                    at(false, false, false),
                    at(true, false, false),
                    at(true, true, false),
                    at(false, true, false),
                ],
                [
                    at(false, false, true),
                    at(true, false, true),
                    at(true, true, true),
                    at(false, true, true),
                ],
                [
                    at(false, false, false),
                    at(false, false, true),
                    at(false, true, true),
                    at(false, true, false),
                ],
                [
                    at(true, false, false),
                    at(true, false, true),
                    at(true, true, true),
                    at(true, true, false),
                ],
                [
                    at(false, true, false),
                    at(true, true, false),
                    at(true, true, true),
                    at(false, true, true),
                ],
            ]
        }
        _ => Vec::new(),
    };
    // While a wall or room is being built, a stronger film climbs it; while one is being
    // taken apart, the film drains away.
    let filling = builder.filling;
    match builder.work.as_ref() {
        Some(Work {
            job: Job::Structure { .. },
            hold,
            removing,
            ..
        }) if !quads.is_empty() => {
            let done = hold.fraction();
            let covered = if *removing { 1.0 - done } else { done };
            let mut positions = Vec::new();
            for [a, b, c, d] in &quads {
                let (c, d) = (*b + (*c - *b) * covered, *a + (*d - *a) * covered);
                let lift = (*b - *a).cross(c - *a).normalize_or_zero() * 0.1;
                for offset in [lift, -lift] {
                    positions.extend([*a, *b, c, *a, c, d].map(|corner| corner + offset));
                }
            }
            let normals = vec![Vec3::Y; positions.len()];
            commands.entity(filling).insert((
                Mesh3d(
                    meshes.add(
                        Mesh::new(
                            PrimitiveTopology::TriangleList,
                            RenderAssetUsages::RENDER_WORLD,
                        )
                        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
                        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals),
                    ),
                ),
                MeshMaterial3d(builder.ghost_looks[look].clone()),
                site.frame,
                Visibility::Visible,
            ));
        }
        _ => {
            commands.entity(filling).insert(Visibility::Hidden);
        }
    }

    // Outlines are drawn afresh every frame; the film only when something changes.
    for quad in &quads {
        outline(&mut gizmos, in_world(*quad), colour);
    }
    // A box round an item that would be picked up or taken away.
    if let Target::Prop(entity) = builder.target
        && let Ok((prop, transform)) = props.get(entity)
    {
        boxed(&mut gizmos, prop.half, transform, colour);
    }

    let ghost = builder.ghost;
    match (builder.item(), builder.target) {
        (
            Some(item),
            Target::Spot {
                at,
                lift,
                indoors,
                blocked,
            },
        ) => {
            let stood =
                props::standing(&site, &planet, at, lift + item.lift, builder.turn, indoors);
            // The grid the item is snapping to, drawn on the floor or ground round it:
            // tile edges strongly, the steps between them faintly, fading with distance.
            if builder.snapping {
                let reach = 6;
                let onto = |east: i32, south: i32| {
                    let on_grid =
                        ((at / GRID_M).round() + IVec3::new(east, 0, south).as_vec3()) * GRID_M;
                    let point = site.frame.transform_point(on_grid.with_y(at.y + 0.03));
                    if indoors {
                        point
                    } else {
                        planet.drawn(point.normalize()) + point.normalize() * 0.05
                    }
                };
                let points: Vec<Vec<Vec3>> = (-reach..=reach)
                    .map(|east| (-reach..=reach).map(|south| onto(east, south)).collect())
                    .collect();
                // Fainter the further a stretch of line is from the item.
                let strength = |line: i32, along: i32| {
                    let far = ((line * line + along * along) as f32).sqrt() / reach as f32;
                    (1.0 - far).max(0.0) * 0.9
                };
                let tiles_every = (CELL_M / GRID_M).round() as i32;
                let centre = (at / GRID_M).round().as_ivec3();
                for line in -reach..=reach {
                    for along in -reach..reach {
                        let (i, j) = ((line + reach) as usize, (along + reach) as usize);
                        let fade = strength(line, along);
                        let weight = |coordinate: i32| {
                            if coordinate.rem_euclid(tiles_every) == 0 {
                                1.0
                            } else {
                                0.4
                            }
                        };
                        // A line running south, and one running east.
                        gizmos.line(
                            points[i][j],
                            points[i][j + 1],
                            Color::WHITE.with_alpha(fade * weight(centre.x + line)),
                        );
                        gizmos.line(
                            points[j][i],
                            points[j + 1][i],
                            Color::WHITE.with_alpha(fade * weight(centre.z + line)),
                        );
                    }
                }
            }
            // The item's own footprint, and whatever is in its way.
            if let Some(model) = models.of(item) {
                boxed(&mut gizmos, model.half, &stood, colour);
                if let Some(mesh) = model.solid.clone() {
                    commands.entity(ghost).insert((
                        Mesh3d(mesh),
                        MeshMaterial3d(builder.ghost_looks[look].clone()),
                        stood.with_translation(stood.translation + *stood.up() * 0.01),
                        Visibility::Visible,
                    ));
                }
            }
            if let Some(Blocked::Item(other)) = blocked
                && let Ok((prop, transform)) = props.get(other)
            {
                boxed(&mut gizmos, prop.half, transform, REFUSED_COLOUR);
            }
        }
        _ => {
            commands.entity(ghost).insert(Visibility::Hidden);
        }
    }

    if builder.shown == (quads.clone(), look) {
        return;
    }
    builder.shown = (quads.clone(), look);
    let mut film = commands.entity(builder.highlight);
    if quads.is_empty() {
        film.insert(Visibility::Hidden);
        return;
    }
    let mut positions = Vec::new();
    for [a, b, c, d] in quads {
        // A hair off the surface on both sides, so it shows from indoors and out.
        let lift = (b - a).cross(c - a).normalize_or_zero() * 0.08;
        for offset in [lift, -lift] {
            positions.extend([a, b, c, a, c, d].map(|corner| corner + offset));
        }
    }
    let normals = vec![Vec3::Y; positions.len()];
    film.insert((
        Mesh3d(
            meshes.add(
                Mesh::new(
                    PrimitiveTopology::TriangleList,
                    RenderAssetUsages::RENDER_WORLD,
                )
                .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
                .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals),
            ),
        ),
        MeshMaterial3d(builder.looks[look].clone()),
        site.frame,
        Visibility::Visible,
    ));
}

/// What each button would do right now, for the prompt under the aiming mark.
pub fn prompt(builder: &Builder, settings: &Settings) -> String {
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
    if let Some((raw, more)) = builder.short {
        return format!(
            "Not enough to build this: {more} more {} needed ({})",
            raw.name(),
            where_found(raw)
        );
    }
    let hold = settings.hold(Action::Use);
    let tap = capitalised(settings.key(Action::Use));
    let next = settings.key(Action::NextKind);
    let action = match (&builder.picked().kind, builder.target) {
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
            "The ground is too high here: only a foundation may go into it".to_string()
        }
        (Kind::Room(_), Target::Ground { adds: false, .. }) => {
            "That is all indoors already".to_string()
        }
        (Kind::Room(_), Target::Ground { .. }) => {
            format!("{hold}  build this room      Scroll  turn      {next}  size")
        }
        (Kind::Room(_), _) => "Aim at the ground, or across a room to extend it".to_string(),
        (Kind::Items(_) | Kind::Move, Target::Spot { blocked, .. }) => match blocked {
            None if builder.is_carrying() => format!(
                "{tap}  put it down      Scroll  turn      {}  put it back",
                capitalised(settings.key(Action::BuildMenu))
            ),
            None => format!("{hold}  build      Scroll  turn      {next}  next kind"),
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
    match builder.price {
        Some((cost, times)) if times > 0 => {
            format!("{action}      Costs {}", inventory::describe(cost, times))
        }
        _ => action,
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

/// Where a material comes from, for a prompt that says what is missing.
fn where_found(raw: Raw) -> &'static str {
    match raw {
        Raw::Wood => "fell trees with an axe",
        Raw::Stone => "break rocks with a pickaxe",
        Raw::Fibre => "cut plants with a knife",
        Raw::Hide | Raw::Meat => "butcher a deer",
        Raw::Ore => "break the rusty rocks with a pickaxe",
        Raw::Iron => "smelt ore at a smithy",
        Raw::Berries => "pick them from berry bushes",
        Raw::CookedMeat => "cook meat at a campfire",
    }
}
