//! The builder tool: what it is aimed at, what it would do there, and doing it.
//!
//! The controls follow the habit of Subnautica and games like it, as far as we could
//! confirm it (research/subnautica-2-building.md): the right mouse button is the menu,
//! the left one builds, the wheel turns what you are holding, and taking apart and moving
//! are modes with their own keys.
//!
//!   Hold right mouse   the dial of parts; move the mouse to one and let go (or hold B)
//!   Tap right mouse    put the part away, or put back what you picked up
//!   Left click         build, place, or pick up                          (or E)
//!   Scroll wheel       turn an item or a room; push and pull a wall      (or Z and X)
//!   R                  the next variant of the part
//!   Q                  take-apart mode, and back
//!   F                  move mode, and back
//!   G                  the grid on and off: items snap to it and turn in eighths
//!   Hold Alt           the opposite of the grid setting, while held
//!   Middle click       pick another of the item the view is on                 (or V)
//!   1 to 9, 0          the first ten parts directly
//!
//! Whatever the tool would do is shown in the world before you commit: the wall a push
//! would move, the outline of a window, the footprint of a room, or a ghost of the item
//! where it would stand.

use super::catalogue::{self, Item, Kind, PARTS, Part};
use super::dial::Dial;
use super::flora::Obstacles;
use super::props::{self, Blocked, Models, Prop, Standing};
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
    Base, BuildOp, DOOR_HEIGHT_M, DOOR_WIDTH_M, Face, Panel, STOREY_M, Side, TILE_M,
};
use terra_core::Ground;

/// How far from the player walls and rooms can be worked on, and items placed.
const STRUCTURE_REACH_M: f32 = 18.0;
const ITEM_REACH_M: f32 = 9.0;
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
    /// The edge between two tiles, where an interior wall stands or could.
    Edge {
        face: Face,
        fits: bool,
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
    /// Set by a tap of the right mouse button: put away, or put back.
    pub cancel: bool,
    pub target: Target,
    /// The rectangles the highlight was last drawn for, and in which look.
    shown: (Vec<[Vec3; 4]>, usize),
    highlight: Entity,
    ghost: Entity,
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

    pub fn pick(&mut self, part: usize) {
        self.part = part.min(PARTS.len() - 1);
        self.stowed = false;
        self.before_mode = None;
    }

    pub fn is_carrying(&self) -> bool {
        self.carrying.is_some()
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

    /// The footprint of the room that would be built, in tiles.
    fn room(&self) -> Option<IVec2> {
        match self.picked().kind {
            Kind::Room(shapes) => {
                let [east, south] = shapes[self.variant()].tiles;
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
    app.add_systems(Startup, setup)
        .add_systems(Update, (choose, aim, act, highlight).chain().in_set(Acting));
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
    commands.insert_resource(Builder {
        part: 0,
        stowed: true,
        variants: vec![0; PARTS.len()],
        turn: 0.0,
        room_turned: false,
        snap: true,
        snapping: true,
        viewed: None,
        wheel: 0,
        before_mode: None,
        carrying: None,
        cancel: false,
        target: Target::Nothing,
        shown: (Vec::new(), 0),
        highlight,
        ghost,
        looks,
        ghost_looks,
    });
}

/// Reads the keys and the wheel: picking parts by number, switching modes, cycling
/// variants, and turning what is in hand.
fn choose(
    keys: Res<ButtonInput<KeyCode>>,
    scroll: Res<AccumulatedMouseScroll>,
    dial: Res<Dial>,
    mut builder: ResMut<Builder>,
) {
    const NUMBERS: [KeyCode; 10] = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
        KeyCode::Digit7,
        KeyCode::Digit8,
        KeyCode::Digit9,
        KeyCode::Digit0,
    ];
    builder.wheel = 0;
    if dial.open || builder.is_carrying() && keys.any_just_pressed(NUMBERS) {
        return;
    }
    for (part, key) in NUMBERS.into_iter().enumerate() {
        if keys.just_pressed(key) {
            builder.pick(part);
        }
    }
    if !builder.is_carrying() {
        if keys.just_pressed(KeyCode::KeyQ) {
            builder.toggle(catalogue::part_of(|kind| matches!(kind, Kind::Remove)));
        }
        if keys.just_pressed(KeyCode::KeyF) {
            builder.toggle(catalogue::part_of(|kind| matches!(kind, Kind::Move)));
        }
    }
    builder.snapping = builder.snap != keys.pressed(KeyCode::AltLeft);
    if keys.just_pressed(KeyCode::KeyG) {
        builder.snap = !builder.snap;
        if builder.snap {
            builder.turn = (builder.turn / GRID_TURN).round() * GRID_TURN % 360.0;
        }
    }
    if builder.stowed {
        return;
    }
    if keys.just_pressed(KeyCode::KeyR) && !builder.is_carrying() {
        let (part, count) = (builder.part, builder.picked().variants());
        let back = keys.pressed(KeyCode::ShiftLeft);
        builder.variants[part] =
            (builder.variants[part] + if back { count - 1 } else { 1 }) % count;
    }
    // One notch at a time, however the wheel reports it. Z and X stand in for the wheel.
    let notch = |amount: f32| (amount > 0.0) as i32 - (amount < 0.0) as i32;
    builder.wheel = notch(scroll.delta.y) + keys.just_pressed(KeyCode::KeyX) as i32
        - keys.just_pressed(KeyCode::KeyZ) as i32;
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
                    let tile = ((touched + toward_viewer * 0.2) / TILE_M)
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
            let tile = (at / TILE_M).floor().as_ivec3().with_y(0);
            let indoors = site.base.contains(tile) && feet.y < STOREY_M * 0.5;
            (at.with_y(0.0), 0.0, indoors)
        };
        let (at, lift, indoors) = on_item
            .or(on_ground)
            .or(in_base)
            .filter(|(at, ..)| within(site.frame.transform_point(*at), ITEM_REACH_M))
            .unwrap_or_else(ahead);
        let world = site.frame.transform_point(at);
        let wet = !indoors && planet.surface(world.normalize()).length() < planet.sea_radius_m();
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

    builder.target = match builder.picked().kind {
        Kind::Reshape | Kind::Window(_) | Kind::Door => {
            outside_wall.map_or(Target::Nothing, |hit| Target::Wall(hit.face))
        }
        Kind::Wall(_) => {
            // An interior wall already there; or the nearest edge of the floor tile the
            // view is on; or, looking at neither, of the tile a couple of steps ahead.
            let reachable =
                hull.filter(|hit| within(origin + direction * hit.distance, STRUCTURE_REACH_M));
            let nearest_edge = |on_floor: Vec3| {
                let tile = (on_floor / TILE_M).floor().as_ivec3();
                let across = on_floor / TILE_M - tile.as_vec3();
                let edges = [
                    (across.x, Side::West),
                    (1.0 - across.x, Side::East),
                    (across.z, Side::North),
                    (1.0 - across.z, Side::South),
                ];
                edges
                    .into_iter()
                    .min_by(|a, b| a.0.total_cmp(&b.0))
                    .map(|(_, side)| Face::new(tile, side))
            };
            let ahead = || {
                let feet = to_base.transform_point3(player.translation);
                let forward = to_base
                    .transform_vector3(view.forward)
                    .with_y(0.0)
                    .normalize_or_zero();
                let storey = (feet.y / STOREY_M).round();
                nearest_edge((feet + forward * 2.0).with_y(storey * STOREY_M + 0.01))
            };
            reachable
                .and_then(|hit| match hit.face.side {
                    _ if hit.partition => Some(hit.face),
                    Side::Down => nearest_edge(
                        met(hit.distance).with_y(hit.face.tile[1] as f32 * STOREY_M + 0.01),
                    ),
                    _ => None,
                })
                .or_else(ahead)
                .map_or(Target::Nothing, |face| Target::Edge {
                    face,
                    fits: site.base.can_partition(face),
                })
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
                .map_or(Target::Nothing, |(point, tiles)| {
                    // Centred on the place aimed at.
                    let local = to_base.transform_point3(point);
                    let corner = IVec3::new(
                        (local.x / TILE_M - tiles.x as f32 * 0.5).round() as i32,
                        0,
                        (local.z / TILE_M - tiles.y as f32 * 0.5).round() as i32,
                    );
                    // The room's floor is the base's floor. Its foundation may go into
                    // the ground; the room itself may not.
                    let fits = (0..=tiles.x).all(|i| {
                        (0..=tiles.y).all(|j| {
                            let on_floor = (corner + IVec3::new(i, 0, j)).as_vec3() * TILE_M;
                            planet.ground.distance(site.frame.transform_point(on_floor)) > 0.02
                        })
                    });
                    let adds = (0..tiles.x).any(|i| {
                        (0..tiles.y).any(|j| !site.base.contains(corner + IVec3::new(i, 0, j)))
                    });
                    Target::Ground {
                        corner,
                        tiles,
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
            (None, Some(hit), _) if hit.partition => Target::Edge {
                face: hit.face,
                fits: true,
            },
            (None, _, Some(hit)) if site.base.panel(hit.face) != Panel::Solid => {
                Target::Wall(hit.face)
            }
            _ => Target::Nothing,
        },
    };
}

fn act(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    cursor: Single<&CursorOptions>,
    planet: Res<Planet>,
    models: Res<Models>,
    props: Query<&Prop>,
    mut builder: ResMut<Builder>,
    mut site: ResMut<Site>,
    mut player: Single<&mut Player>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    // A tap of the right button: put back what was picked up, or put the part away.
    if std::mem::take(&mut builder.cancel) {
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
    // A click that only captured the mouse should not also build something.
    let captured = cursor.grab_mode != CursorGrabMode::None;
    let click =
        keys.just_pressed(KeyCode::KeyE) || (captured && mouse.just_pressed(MouseButton::Left));
    // The middle button: another of whatever the view is on.
    let copy =
        keys.just_pressed(KeyCode::KeyV) || (captured && mouse.just_pressed(MouseButton::Middle));
    if copy
        && !builder.is_carrying()
        && let Some(prop) = builder.viewed.and_then(|entity| props.get(entity).ok())
        && let Some((part, variant)) = catalogue::place_of(prop.item)
    {
        builder.pick(part);
        builder.variants[part] = variant;
        builder.turn = prop.turn;
        return;
    }
    let op = match (&builder.picked().kind, builder.target) {
        // A wall goes out a tile for a click or a notch away, and in for a notch back.
        (Kind::Reshape, Target::Wall(face)) if click || builder.wheel > 0 => {
            Some(BuildOp::Grow(face))
        }
        (Kind::Reshape, Target::Wall(face)) if builder.wheel < 0 => Some(BuildOp::Shrink(face)),
        (Kind::Window(shapes), Target::Wall(face)) if click => {
            Some(BuildOp::SetPanel(face, shapes[builder.variant()].1))
        }
        (Kind::Door, Target::Wall(face)) if click => Some(BuildOp::SetPanel(face, Panel::Door)),
        (Kind::Wall(shapes), Target::Edge { face, fits: true }) if click => Some(
            BuildOp::SetPartition(face, Some(shapes[builder.variant()].1)),
        ),
        (Kind::Remove, Target::Wall(face)) if click => Some(BuildOp::SetPanel(face, Panel::Solid)),
        (Kind::Remove, Target::Edge { face, .. }) if click => {
            Some(BuildOp::SetPartition(face, None))
        }
        (
            Kind::Room(_),
            Target::Ground {
                corner,
                tiles,
                fits: true,
                adds: true,
            },
        ) if click => Some(BuildOp::AddRoom {
            min: corner.to_array(),
            max: (corner + IVec3::new(tiles.x - 1, 0, tiles.y - 1)).to_array(),
        }),
        (
            Kind::Items(_) | Kind::Move,
            Target::Spot {
                at,
                lift,
                indoors,
                blocked: None,
            },
        ) if click => {
            if let Some(item) = builder.item() {
                props::place(
                    &mut commands,
                    &models,
                    &site,
                    &planet,
                    item,
                    at,
                    lift,
                    builder.turn,
                    indoors,
                );
                info!("Placed a {} at {at}, {lift} up.", item.name);
                builder.carrying = None;
                player.since_acting = 0.0;
            }
            None
        }
        (Kind::Move, Target::Prop(entity)) if click => {
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
            None
        }
        (Kind::Remove, Target::Prop(entity)) if click => {
            if let Ok(prop) = props.get(entity) {
                info!("Removed a {}.", prop.item.name);
            }
            commands.entity(entity).despawn();
            player.since_acting = 0.0;
            None
        }
        _ => None,
    };
    if let Some(op) = op {
        player.since_acting = 0.0;
        site.apply(op, &mut commands, &planet, &mut meshes);
    }
}

/// The outline of an opening on a wall tile, as it would be cut.
fn opening(face: Face, panel: Panel) -> Option<[Vec3; 4]> {
    let [low_left, low_right, _, high_left] = Base::corners(face);
    let across = (low_right - low_left).normalize();
    let up = (high_left - low_left).normalize();
    let (left, right, bottom, top) = match panel {
        Panel::Solid => return None,
        Panel::Window {
            left,
            right,
            sill,
            top,
        } => (left, TILE_M - right, sill, STOREY_M - top),
        Panel::Door => {
            let side = (TILE_M - DOOR_WIDTH_M) * 0.5;
            (side, TILE_M - side, 0.0, DOOR_HEIGHT_M)
        }
    };
    let at = |x: f32, y: f32| low_left + across * x + up * y;
    Some([
        at(left, bottom),
        at(right, bottom),
        at(right, top),
        at(left, top),
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
    );
    let look = match (kind, builder.target) {
        (Kind::Move, Target::Prop(_)) => 1,
        _ if refused => 2,
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
        (Kind::Reshape, Target::Wall(face)) => site
            .base
            .wall(face)
            .into_iter()
            .map(Base::corners)
            .collect(),
        (Kind::Window(shapes), Target::Wall(face)) => opening(face, shapes[builder.variant()].1)
            .into_iter()
            .collect(),
        (Kind::Door, Target::Wall(face)) => opening(face, Panel::Door).into_iter().collect(),
        (Kind::Remove, Target::Wall(face)) => {
            opening(face, site.base.panel(face)).into_iter().collect()
        }
        // An interior wall: the whole edge, and the opening it would have.
        (Kind::Wall(shapes), Target::Edge { face, .. }) => std::iter::once(Base::corners(face))
            .chain(opening(face, shapes[builder.variant()].1))
            .collect(),
        (_, Target::Edge { face, .. }) => vec![Base::corners(face)],
        (_, Target::Ground { corner, tiles, .. }) => {
            let low = corner.as_vec3() * TILE_M;
            let high = low + Vec3::new(tiles.x as f32 * TILE_M, STOREY_M, tiles.y as f32 * TILE_M);
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
                let tiles_every = (TILE_M / GRID_M).round() as i32;
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
pub fn prompt(builder: &Builder) -> &'static str {
    if builder.stowed {
        return "Hold right mouse  build menu";
    }
    match (&builder.picked().kind, builder.target) {
        (Kind::Reshape, Target::Wall(_)) => {
            "Left click  push the wall out      Scroll  push and pull"
        }
        (Kind::Window(_), Target::Wall(_)) => "Left click  cut this window      R  shape",
        (Kind::Door, Target::Wall(_)) => "Left click  cut a door",
        (Kind::Reshape | Kind::Window(_) | Kind::Door, _) => "Aim at an outside wall",
        (Kind::Wall(_), Target::Edge { fits: true, .. }) => {
            "Left click  build this wall      R  kind"
        }
        (Kind::Wall(_), Target::Edge { fits: false, .. }) => {
            "Interior walls go between two tiles of a room"
        }
        (Kind::Wall(_), _) => "Aim at a floor indoors",
        (Kind::Room(_), Target::Ground { fits: false, .. }) => {
            "The ground is too high here: only a foundation may go into it"
        }
        (Kind::Room(_), Target::Ground { adds: false, .. }) => "That is all indoors already",
        (Kind::Room(_), Target::Ground { .. }) => {
            "Left click  build this room      Scroll  turn      R  size"
        }
        (Kind::Room(_), _) => "Aim at the ground, or across a room to extend it",
        (Kind::Items(_) | Kind::Move, Target::Spot { blocked, .. }) => match blocked {
            None if builder.is_carrying() => {
                "Left click  put it down      Scroll  turn      Right click  put it back"
            }
            None => "Left click  place      Scroll  turn      R  next kind",
            Some(Blocked::Structure) => "It would stick through a wall",
            Some(Blocked::Item(_)) => "The outlined item is in the way",
            Some(Blocked::Nature) => "A tree or a boulder is in the way",
            Some(Blocked::Water) => "Not in the water",
        },
        (Kind::Items(_), _) => "Aim at a floor or at the ground nearby",
        (Kind::Move, Target::Prop(_)) => "Left click  pick this up",
        (Kind::Move, _) => "Aim at an item to move it      F  done",
        (Kind::Remove, Target::Prop(_)) => "Left click  take this away",
        (Kind::Remove, Target::Edge { .. }) => "Left click  take this wall away",
        (Kind::Remove, Target::Wall(_)) => "Left click  close this opening",
        (Kind::Remove, _) => "Aim at an item, an interior wall, a window or a door      Q  done",
    }
}

/// The keys worth knowing while a part is in hand.
pub fn legend(builder: &Builder) -> &'static str {
    if builder.stowed {
        ""
    } else if builder.snap {
        "Q  take apart     F  move     G  grid off     Alt  off the grid     Middle click  copy"
    } else {
        "Q  take apart     F  move     G  grid on     Alt  on the grid     Middle click  copy"
    }
}

/// Whether the tool is waiting for the player to take the mouse.
pub fn waiting(cursor: &CursorOptions, options: &Options) -> bool {
    cursor.grab_mode == CursorGrabMode::None && !options.scripted()
}
