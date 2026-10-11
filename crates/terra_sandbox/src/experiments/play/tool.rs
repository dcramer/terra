//! The builder tool: what it is aimed at, what it would do there, and doing it.
//!
//! You pick a part from the dial (`dial.rs`), cycle through its variants, and aim.
//! Whatever the tool would do is shown in the world before you commit: the wall a push
//! would move, the outline of a window, the footprint of a room, or a ghost of the item
//! where it would stand.
//!
//!   Hold Q        the dial of parts; move the mouse to one and let go
//!   1 to 9, 0     the first ten parts directly
//!   Scroll, or C  the next variant of the part
//!   R             turn an item an eighth of a circle
//!   Left click    build      (E does the same)
//!   Right click   take away  (F does the same)

use super::catalogue::{Item, Kind, PARTS, Part};
use super::dial::Dial;
use super::props::{self, Models, Prop};
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
    Wall(Face),
    /// Open ground, as the lowest tile of the room that would be built there, its size
    /// in tiles, and whether the ground is low enough for the room's floor to clear it.
    Ground {
        corner: IVec3,
        tiles: IVec2,
        fits: bool,
    },
    /// Somewhere an item could stand, in the base's own space.
    Spot {
        at: Vec3,
        indoors: bool,
        fits: bool,
    },
    /// An item already placed.
    Prop(Entity),
}

#[derive(Resource)]
pub struct Builder {
    /// Which part is picked, as its place in the catalogue.
    pub part: usize,
    /// The variant last used of each part.
    variants: Vec<usize>,
    /// How far the next item is turned, in degrees.
    turn: f32,
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
    }

    /// The item that would be placed, if the picked part is an item.
    fn item(&self) -> Option<&'static Item> {
        match self.picked().kind {
            Kind::Items(items) => Some(&items[self.variant()]),
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
    gizmos.config_mut::<DefaultGizmoConfigGroup>().0.line.width = 2.5;

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
        variants: vec![0; PARTS.len()],
        turn: 0.0,
        target: Target::Nothing,
        shown: (Vec::new(), 0),
        highlight,
        ghost,
        looks,
        ghost_looks,
    });
}

/// Picks parts by number, cycles variants and turns items.
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
    for (part, key) in NUMBERS.into_iter().enumerate() {
        if keys.just_pressed(key) {
            builder.pick(part);
        }
    }
    if dial.open {
        return;
    }
    let count = builder.picked().variants();
    let step = if keys.just_pressed(KeyCode::KeyC) || scroll.delta.y < 0.0 {
        1
    } else if scroll.delta.y > 0.0 {
        count - 1
    } else {
        0
    };
    if step != 0 {
        let part = builder.part;
        builder.variants[part] = (builder.variants[part] + step) % count;
    }
    if keys.just_pressed(KeyCode::KeyR) {
        builder.turn = (builder.turn + 45.0) % 360.0;
    }
}

/// Works out what the middle of the view is pointed at, within the player's reach.
fn aim(
    planet: Res<Planet>,
    site: Res<Site>,
    options: Res<Options>,
    dial: Res<Dial>,
    camera: Single<&GlobalTransform, With<Camera3d>>,
    player: Single<&Transform, With<Player>>,
    props: Query<(Entity, &Prop, &Transform, &Visibility)>,
    mut builder: ResMut<Builder>,
) {
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
    let hull = site.base.raycast(local_origin, local_direction, 200.0);
    let ground = planet.ground.raycast(origin, direction, 200.0);
    let wall = hull
        .filter(|hit| hit.face.side.is_wall())
        .filter(|hit| within(origin + direction * hit.distance, STRUCTURE_REACH_M));

    builder.target = match builder.picked().kind {
        Kind::Reshape | Kind::Window(_) | Kind::Door => {
            wall.map_or(Target::Nothing, |hit| Target::Wall(hit.face))
        }
        Kind::Room(shapes) if hull.is_none() => ground
            .filter(|ground| within(ground.point, STRUCTURE_REACH_M))
            .map_or(Target::Nothing, |ground| {
                // Centred on the ground aimed at.
                let tiles = IVec2::from_array(shapes[builder.variant()].tiles);
                let local = to_base.transform_point3(ground.point);
                let corner = IVec3::new(
                    (local.x / TILE_M - tiles.x as f32 * 0.5).round() as i32,
                    0,
                    (local.z / TILE_M - tiles.y as f32 * 0.5).round() as i32,
                );
                // The room's floor is the base's floor; it cannot be built into a hillside.
                let fits = (0..=tiles.x).all(|i| {
                    (0..=tiles.y).all(|j| {
                        let on_floor = (corner + IVec3::new(i, 0, j)).as_vec3() * TILE_M;
                        planet.ground.distance(site.frame.transform_point(on_floor)) > 0.02
                    })
                });
                Target::Ground {
                    corner,
                    tiles,
                    fits,
                }
            }),
        Kind::Room(_) => Target::Nothing,
        Kind::Items(_) => {
            // The floor if the view meets one, the foot of a wall if it meets that, and
            // otherwise the ground outside.
            let on_ground = ground
                .filter(|ground| hull.is_none_or(|hit| ground.distance < hit.distance))
                .map(|ground| (to_base.transform_point3(ground.point), false));
            let in_base = hull.and_then(|hit| {
                let floor = hit.face.tile[1] as f32 * STOREY_M;
                match hit.face.side {
                    Side::Down => Some((local_origin + local_direction * hit.distance, true)),
                    Side::Up => None,
                    _ => {
                        let short = local_origin + local_direction * (hit.distance - 0.45);
                        let tile = (short / TILE_M).floor().as_ivec3().with_y(hit.face.tile[1]);
                        let indoors = site.base.contains(tile);
                        Some((short.with_y(if indoors { floor } else { 0.0 }), indoors))
                    }
                }
            });
            on_ground
                .or(in_base)
                .filter(|(at, _)| within(site.frame.transform_point(*at), ITEM_REACH_M))
                .map_or(Target::Nothing, |(at, indoors)| {
                    let world = site.frame.transform_point(at);
                    let dry = planet.surface(world.normalize()).length() > planet.sea_radius_m();
                    Target::Spot {
                        at,
                        indoors,
                        fits: props::fits(&site, at, indoors) && (indoors || dry),
                    }
                })
        }
        Kind::Remove => {
            // The nearest item the view passes through, short of any wall.
            let limit = hull.map_or(f32::MAX, |hit| hit.distance + 0.3);
            let nearest = props
                .iter()
                .filter(|(_, _, _, visibility)| **visibility != Visibility::Hidden)
                .filter_map(|(entity, prop, transform, _)| {
                    let middle = transform.translation + *transform.up() * prop.half.y;
                    let along = (middle - origin).dot(direction);
                    let off = (origin + direction * along).distance(middle);
                    let size = prop.half.max_element();
                    (along > 0.0
                        && along < limit
                        && off < size.max(0.3)
                        && within(middle, ITEM_REACH_M))
                    .then_some((entity, along))
                })
                .min_by(|a, b| a.1.total_cmp(&b.1));
            match (nearest, wall) {
                (Some((entity, _)), _) => Target::Prop(entity),
                (None, Some(hit)) if site.base.panel(hit.face) != Panel::Solid => {
                    Target::Wall(hit.face)
                }
                _ => Target::Nothing,
            }
        }
    };
}

fn act(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    cursor: Single<&CursorOptions>,
    planet: Res<Planet>,
    models: Res<Models>,
    builder: Res<Builder>,
    props: Query<&Prop>,
    mut site: ResMut<Site>,
    mut player: Single<&mut Player>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    // A click that only captured the mouse should not also build something.
    let captured = cursor.grab_mode != CursorGrabMode::None;
    let build =
        keys.just_pressed(KeyCode::KeyE) || (captured && mouse.just_pressed(MouseButton::Left));
    let take =
        keys.just_pressed(KeyCode::KeyF) || (captured && mouse.just_pressed(MouseButton::Right));
    if !build && !take {
        return;
    }
    let op = match (&builder.picked().kind, builder.target) {
        (Kind::Reshape, Target::Wall(face)) if build => Some(BuildOp::Grow(face)),
        (Kind::Reshape, Target::Wall(face)) => Some(BuildOp::Shrink(face)),
        (Kind::Window(shapes), Target::Wall(face)) if build => {
            Some(BuildOp::SetPanel(face, shapes[builder.variant()].1))
        }
        (Kind::Door, Target::Wall(face)) if build => Some(BuildOp::SetPanel(face, Panel::Door)),
        (Kind::Window(_) | Kind::Door | Kind::Remove, Target::Wall(face)) => {
            Some(BuildOp::SetPanel(face, Panel::Solid))
        }
        (
            Kind::Room(_),
            Target::Ground {
                corner,
                tiles,
                fits: true,
            },
        ) if build => Some(BuildOp::AddRoom {
            min: corner.to_array(),
            max: (corner + IVec3::new(tiles.x - 1, 0, tiles.y - 1)).to_array(),
        }),
        (
            Kind::Items(_),
            Target::Spot {
                at,
                indoors,
                fits: true,
            },
        ) if build => {
            if let Some(item) = builder.item() {
                props::place(
                    &mut commands,
                    &models,
                    &site,
                    &planet,
                    item,
                    at,
                    0.0,
                    builder.turn,
                    indoors,
                );
                info!("Placed a {} at {at}.", item.name);
                player.since_acting = 0.0;
            }
            None
        }
        (Kind::Remove, Target::Prop(entity)) => {
            commands.entity(entity).despawn();
            if let Ok(prop) = props.get(entity) {
                info!("Removed a {}.", prop.item.name);
            }
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
    let look = match (kind, builder.target) {
        (
            _,
            Target::Ground { fits: false, .. } | Target::Spot { fits: false, .. } | Target::Prop(_),
        ) => 2,
        (Kind::Remove, _) => 2,
        (Kind::Window(_) | Kind::Door, _) => 1,
        _ => 0,
    };
    let colour = [BUILD_COLOUR, OPENING_COLOUR, REFUSED_COLOUR][look];
    let in_world = |corners: [Vec3; 4]| corners.map(|corner| site.frame.transform_point(corner));
    let outline = |gizmos: &mut Gizmos, [a, b, c, d]: [Vec3; 4]| {
        gizmos.linestrip([a, b, c, d, a], colour);
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
        outline(&mut gizmos, in_world(*quad));
    }
    if let Target::Prop(entity) = builder.target
        && let Ok((prop, transform)) = props.get(entity)
    {
        // A box round the item that would go.
        for height in [0.0, prop.half.y * 2.0] {
            let corner = |x: f32, z: f32| {
                transform.transform_point(Vec3::new(x * prop.half.x, height, z * prop.half.z))
            };
            outline(
                &mut gizmos,
                [
                    corner(-1.0, -1.0),
                    corner(1.0, -1.0),
                    corner(1.0, 1.0),
                    corner(-1.0, 1.0),
                ],
            );
        }
    }

    // A ghost of the item where it would stand.
    let ghost = builder.ghost;
    match (builder.item(), builder.target) {
        (Some(item), Target::Spot { at, indoors, .. }) => {
            if let Some(mesh) = models.of(item).and_then(|model| model.solid.clone()) {
                commands.entity(ghost).insert((
                    Mesh3d(mesh),
                    MeshMaterial3d(builder.ghost_looks[look].clone()),
                    props::standing(&site, &planet, at, item.lift + 0.01, builder.turn, indoors),
                    Visibility::Visible,
                ));
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
        let lift = (b - a).cross(c - a).normalize_or_zero() * 0.03;
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
    match (&builder.picked().kind, builder.target) {
        (Kind::Reshape, Target::Wall(_)) => {
            "Left click  push the wall out      Right click  pull it in"
        }
        (Kind::Window(_), Target::Wall(_)) => {
            "Left click  cut this window      Right click  solid wall      Scroll  shape"
        }
        (Kind::Door, Target::Wall(_)) => "Left click  cut a door      Right click  solid wall",
        (Kind::Reshape | Kind::Window(_) | Kind::Door, _) => "Aim at a wall",
        (Kind::Room(_), Target::Ground { fits: true, .. }) => {
            "Left click  build this room      Scroll  size"
        }
        (Kind::Room(_), Target::Ground { fits: false, .. }) => "The ground is too high here",
        (Kind::Room(_), _) => "Aim at open ground nearby",
        (Kind::Items(_), Target::Spot { fits: true, .. }) => {
            "Left click  place      R  turn      Scroll  kind"
        }
        (Kind::Items(_), Target::Spot { fits: false, .. }) => "It cannot stand there",
        (Kind::Items(_), _) => "Aim at a floor or at the ground nearby",
        (Kind::Remove, Target::Prop(_)) => "Left click  take this away",
        (Kind::Remove, Target::Wall(_)) => "Left click  close this opening",
        (Kind::Remove, _) => "Aim at an item, a window or a door",
    }
}

/// Whether the tool is waiting for the player to take the mouse.
pub fn waiting(cursor: &CursorOptions, options: &Options, _view: &View) -> bool {
    cursor.grab_mode == CursorGrabMode::None && !options.scripted()
}
