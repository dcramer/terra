//! The builder tool: what it is aimed at, what its two actions do, and how the game
//! shows both before the player commits.
//!
//! Whatever the tool would act on is lit up in the world: the whole wall a push or pull
//! would move, the one tile a window would go in, or the outline of a room on the ground.
//! A line of text over the mode chips says what each button would do.

use super::{Player, View};
use crate::experiments::build::Site;
use crate::planet::Planet;
use crate::shell::Options;
use bevy::{
    asset::RenderAssetUsages,
    light::{NotShadowCaster, NotShadowReceiver},
    prelude::*,
    render::render_resource::PrimitiveTopology,
    window::{CursorGrabMode, CursorOptions},
};
use terra_build::{Base, BuildOp, Face, Panel, STOREY_M, TILE_M};
use terra_core::Ground;

/// How far from the player the builder can act.
const TOOL_REACH_M: f32 = 18.0;
const SHAPE_COLOUR: Color = Color::srgb(0.35, 0.9, 1.0);
const OPENING_COLOUR: Color = Color::srgb(1.0, 0.78, 0.25);
const REFUSED_COLOUR: Color = Color::srgb(1.0, 0.35, 0.3);

/// The systems that use the tool, for ordering against the player's movement.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct Acting;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    Shape,
    Openings,
    Room,
}

const MODES: [(Mode, KeyCode, &str); 3] = [
    (Mode::Shape, KeyCode::Digit1, "1  Shape"),
    (Mode::Openings, KeyCode::Digit2, "2  Openings"),
    (Mode::Room, KeyCode::Digit3, "3  Room"),
];

/// What the tool is pointed at.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Target {
    Nothing,
    Wall(Face),
    /// Open ground, as the lowest tile of the room that would be built there, and
    /// whether the ground is low enough for the room's floor to clear it.
    Ground {
        corner: IVec3,
        fits: bool,
    },
}

#[derive(Resource)]
struct Builder {
    mode: Mode,
    target: Target,
    /// What the highlight was last drawn for.
    shown: Option<(Mode, Target, u64)>,
    highlight: Entity,
    looks: [Handle<StandardMaterial>; 3],
}

#[derive(Component)]
struct Chip(Mode);

#[derive(Component)]
struct Prompt;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, setup).add_systems(
        Update,
        (aim, act, highlight, prompts).chain().in_set(Acting),
    );
}

fn setup(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut gizmos: ResMut<GizmoConfigStore>,
) {
    gizmos.config_mut::<DefaultGizmoConfigGroup>().0.line.width = 2.5;

    // A glowing film, brighter than anything lit, so it reads in sun and in shade.
    let mut film = |colour: Color| {
        let colour = colour.to_linear();
        materials.add(StandardMaterial {
            base_color: Color::LinearRgba(LinearRgba::new(
                colour.red * 2.2,
                colour.green * 2.2,
                colour.blue * 2.2,
                0.16,
            )),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            cull_mode: None,
            double_sided: true,
            ..default()
        })
    };
    let looks = [
        film(SHAPE_COLOUR),
        film(OPENING_COLOUR),
        film(REFUSED_COLOUR),
    ];
    let highlight = commands
        .spawn((
            Name::new("Builder highlight"),
            MeshMaterial3d(looks[0].clone()),
            Transform::default(),
            Visibility::default(),
            NotShadowCaster,
            NotShadowReceiver,
        ))
        .id();
    commands.insert_resource(Builder {
        mode: Mode::Shape,
        target: Target::Nothing,
        shown: None,
        highlight,
        looks,
    });

    // The aiming mark: a small ring in the middle of the view.
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: percent(50),
            top: percent(50),
            width: px(8),
            height: px(8),
            margin: UiRect::all(px(-4)),
            border: UiRect::all(px(1.5)),
            border_radius: BorderRadius::MAX,
            ..default()
        },
        BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.85)),
    ));

    // Along the bottom: what the buttons would do, over a row of mode chips.
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            bottom: px(28),
            width: percent(100),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: px(10),
            ..default()
        })
        .with_children(|column| {
            column
                .spawn((
                    Node {
                        padding: UiRect::axes(px(16), px(7)),
                        border_radius: BorderRadius::all(px(7)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.07, 0.08, 0.1, 0.45)),
                ))
                .with_child((
                    Prompt,
                    Text::new(""),
                    TextFont::from_font_size(16.0),
                    TextColor(Color::srgba(1.0, 1.0, 1.0, 0.95)),
                ));
            column
                .spawn(Node {
                    column_gap: px(8),
                    ..default()
                })
                .with_children(|row| {
                    for (mode, _, label) in MODES {
                        row.spawn((
                            Chip(mode),
                            Node {
                                padding: UiRect::axes(px(14), px(7)),
                                border_radius: BorderRadius::all(px(7)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.07, 0.08, 0.1, 0.55)),
                        ))
                        .with_child((
                            Text::new(label),
                            TextFont::from_font_size(15.0),
                            TextColor(Color::srgba(1.0, 1.0, 1.0, 0.9)),
                        ));
                    }
                });
        });
}

/// Works out what the middle of the view is pointed at, within the player's reach.
fn aim(
    keys: Res<ButtonInput<KeyCode>>,
    planet: Res<Planet>,
    site: Res<Site>,
    camera: Single<&GlobalTransform, With<Camera3d>>,
    player: Single<&Transform, With<Player>>,
    options: Res<Options>,
    mut builder: ResMut<Builder>,
) {
    // A fixed camera is for taking pictures, not for aiming with.
    if options.vantage.is_some() {
        return;
    }
    for (mode, key, _) in MODES {
        if keys.just_pressed(key) {
            builder.mode = mode;
        }
    }
    let to_base = site.frame.compute_affine().inverse();
    let origin = camera.translation();
    let direction = *camera.forward();
    let within_reach = |point: Vec3| point.distance(player.translation) <= TOOL_REACH_M;
    let wall = site
        .base
        .raycast(
            to_base.transform_point3(origin),
            to_base.transform_vector3(direction).normalize(),
            200.0,
        )
        .filter(|hit| within_reach(origin + direction * hit.distance));
    let ground = || {
        planet
            .ground
            .raycast(origin, direction, 200.0)
            .filter(|ground| within_reach(ground.point))
    };
    builder.target = match (builder.mode, wall) {
        (Mode::Shape | Mode::Openings, Some(hit)) => Target::Wall(hit.face),
        (Mode::Room, None) => ground().map_or(Target::Nothing, |ground| {
            // Three tiles square, centred on the ground aimed at.
            let local = to_base.transform_point3(ground.point);
            let corner = IVec3::new(
                (local.x / TILE_M).floor() as i32 - 1,
                0,
                (local.z / TILE_M).floor() as i32 - 1,
            );
            // The room's floor is the base's floor; it cannot be built into a hillside.
            let fits = (0..=3).all(|i| {
                (0..=3).all(|j| {
                    let on_floor = (corner + IVec3::new(i, 0, j)).as_vec3() * TILE_M;
                    planet.ground.distance(site.frame.transform_point(on_floor)) > 0.02
                })
            });
            Target::Ground { corner, fits }
        }),
        _ => Target::Nothing,
    };
}

fn act(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    cursor: Single<&CursorOptions>,
    planet: Res<Planet>,
    builder: Res<Builder>,
    mut site: ResMut<Site>,
    mut player: Single<&mut Player>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    // A click that only captured the mouse should not also build something.
    let captured = cursor.grab_mode != CursorGrabMode::None;
    let main =
        keys.just_pressed(KeyCode::KeyE) || (captured && mouse.just_pressed(MouseButton::Left));
    let second =
        keys.just_pressed(KeyCode::KeyR) || (captured && mouse.just_pressed(MouseButton::Right));
    let op = match (builder.mode, builder.target) {
        (Mode::Shape, Target::Wall(face)) if main => Some(BuildOp::Grow(face)),
        (Mode::Shape, Target::Wall(face)) if second => Some(BuildOp::Shrink(face)),
        (Mode::Openings, Target::Wall(face)) if main => {
            Some(BuildOp::SetPanel(face, site.base.panel(face).next()))
        }
        (Mode::Openings, Target::Wall(face)) if second => {
            Some(BuildOp::SetPanel(face, Panel::Solid))
        }
        (Mode::Room, Target::Ground { corner, fits: true }) if main => Some(BuildOp::AddRoom {
            min: corner.to_array(),
            max: (corner + IVec3::new(2, 0, 2)).to_array(),
        }),
        _ => None,
    };
    if let Some(op) = op {
        player.since_acting = 0.0;
        site.apply(op, &mut commands, &planet, &mut meshes);
    }
}

/// Lights up what the tool would act on.
fn highlight(
    mut commands: Commands,
    site: Res<Site>,
    mut builder: ResMut<Builder>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut gizmos: Gizmos,
) {
    let colour = match (builder.mode, builder.target) {
        (_, Target::Ground { fits: false, .. }) => REFUSED_COLOUR,
        (Mode::Openings, _) => OPENING_COLOUR,
        _ => SHAPE_COLOUR,
    };
    // The rectangles to light, in the base's own space.
    let quads: Vec<[Vec3; 4]> = match (builder.mode, builder.target) {
        (Mode::Shape, Target::Wall(face)) => site
            .base
            .wall(face)
            .into_iter()
            .map(Base::corners)
            .collect(),
        (_, Target::Wall(face)) => vec![Base::corners(face)],
        (_, Target::Ground { corner, .. }) => {
            let low = corner.as_vec3() * TILE_M;
            let high = low + Vec3::new(3.0 * TILE_M, STOREY_M, 3.0 * TILE_M);
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
        (_, Target::Nothing) => Vec::new(),
    };
    // The outlines are drawn afresh every frame; the film only when something changes.
    for quad in &quads {
        let [a, b, c, d] = quad.map(|corner| site.frame.transform_point(corner));
        gizmos.linestrip([a, b, c, d, a], colour);
    }
    let showing = Some((builder.mode, builder.target, site.revision));
    if builder.shown == showing {
        return;
    }
    builder.shown = showing;
    let mut film = commands.entity(builder.highlight);
    if quads.is_empty() {
        film.remove::<Mesh3d>();
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
    let look = match colour {
        REFUSED_COLOUR => 2,
        OPENING_COLOUR => 1,
        _ => 0,
    };
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
    ));
}

/// Says what each button would do, and shows which mode is on.
fn prompts(
    builder: Res<Builder>,
    options: Res<Options>,
    cursor: Single<&CursorOptions>,
    _view: Res<View>,
    mut prompt: Single<&mut Text, With<Prompt>>,
    mut chips: Query<(&Chip, &mut BackgroundColor)>,
) {
    let waiting = cursor.grab_mode == CursorGrabMode::None && !options.scripted();
    let text = match (builder.mode, builder.target) {
        _ if waiting => "Click to take control",
        (Mode::Shape, Target::Wall(_)) => {
            "Left click  push the wall out      Right click  pull it in"
        }
        (Mode::Shape, _) => "Aim at a wall to move it",
        (Mode::Openings, Target::Wall(_)) => {
            "Left click  window or door      Right click  solid wall"
        }
        (Mode::Openings, _) => "Aim at a wall to open it",
        (Mode::Room, Target::Ground { fits: true, .. }) => "Left click  build a room here",
        (Mode::Room, Target::Ground { fits: false, .. }) => "The ground is too high here",
        (Mode::Room, _) => "Aim at open ground nearby",
    };
    if prompt.0 != text {
        prompt.0 = text.to_string();
    }
    for (chip, mut background) in &mut chips {
        let colour = if chip.0 == builder.mode {
            Color::srgba(0.9, 0.46, 0.17, 0.92)
        } else {
            Color::srgba(0.07, 0.08, 0.1, 0.55)
        };
        if background.0 != colour {
            background.0 = colour;
        }
    }
}
