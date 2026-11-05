//! The people: the party of four, the village's folk and its animals, each a rigged model
//! from the game (a body dressed in its pieces, or a creature), walking the paths the
//! search map finds and playing its idle, walk and run clips.

use crate::nav::Nav;
use crate::scene::FIRE;
use crate::terrain::Land;
use bevy::animation::transition::AnimationTransitions;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::gltf::Gltf;
use bevy::mesh::skinning::SkinnedMesh;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use std::collections::VecDeque;
use std::time::Duration;

pub fn plugin(app: &mut App) {
    app.init_resource::<Clips>()
        .add_systems(Startup, spawn)
        .add_systems(
            Update,
            (make_clips, start_animators, dress, hold, walk, stand, gait, wander).chain(),
        )
        .add_systems(
            PostUpdate,
            in_place
                .after(bevy::app::AnimationSystems)
                .before(bevy::transform::TransformSystems::Propagate),
        );
}

const BODY: &str = "models/character/male.glb";
const PIECES: &str = "models/character/male";
/// The knight (a Tripo model with its own skeleton and clips, in the spike's assets); its
/// armature is about a metre tall.
const KNIGHT: &str = "spike://models/knight.glb";
const KNIGHT_SCALE: f32 = 1.84;

/// What each kind of model calls its idle, walk, run and busy clips (a beast's grazing or
/// pecking; a bird's run is its flight).
const KINDS: &[(&str, [&str; 4])] = &[
    (BODY, ["Idle_Loop", "Walk_Loop", "Jog_Fwd_Loop", "Idle_Loop"]),
    (KNIGHT, ["idle", "walk", "run", "idle"]),
    (crate::wildlife::STAG, ["Idle", "Walk", "Run", "Eat"]),
    (crate::wildlife::HIND, ["Idle", "Walk", "Run", "Eat"]),
    (crate::wildlife::CALF, ["Idle", "Walk", "Run", "Eat"]),
    (crate::wildlife::HARE, ["Idle", "Walk", "Run", "Eat"]),
    (crate::wildlife::BOAR, ["Idle", "Walk", "Trot", "Eat"]),
    (crate::wildlife::FOX, ["Idle", "Walk", "Run", "Eat"]),
    (crate::wildlife::CHAFFINCH, ["Idle", "Hop", "Fly", "Peck"]),
    (crate::wildlife::SPARROW, ["Idle", "Hop", "Fly", "Peck"]),
    (crate::wildlife::MALLARD, ["Float", "Swim", "Fly", "Dabble"]),
    (crate::wildlife::SQUIRREL, ["Idle", "Walk", "Run", "Eat"]),
];

#[derive(Resource, Default)]
struct Clips {
    loading: Vec<(&'static str, Handle<Gltf>)>,
    ready: HashMap<&'static str, Set>,
}

#[derive(Clone)]
struct Set {
    graph: Handle<AnimationGraph>,
    nodes: [AnimationNodeIndex; 4],
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Gait {
    Idle,
    Walk,
    Run,
    Busy,
}

/// Anyone who moves: their model, how fast they walk and run, and where they are going.
#[derive(Component)]
pub struct Actor {
    pub model: &'static str,
    pub walk: f32,
    pub run: f32,
    pub path: VecDeque<Vec2>,
    pub running: bool,
    /// How far from their feet to their head, for picking them on the screen.
    pub height: f32,
    pub radius: f32,
    /// Grazing, rooting or pecking while it stands.
    pub busy: bool,
    /// How high above the land it is (a bird in flight).
    pub lift: f32,
    /// It sits on the water, not the pond's bed.
    pub swims: bool,
}

impl Actor {
    pub fn new(model: &'static str, walk: f32, run: f32, height: f32, radius: f32) -> Self {
        Actor {
            model,
            walk,
            run,
            path: VecDeque::new(),
            running: false,
            height,
            radius,
            busy: false,
            lift: 0.0,
            swims: false,
        }
    }

    pub fn gait(&self) -> Gait {
        match (self.path.is_empty(), self.running) {
            (true, _) if self.busy => Gait::Busy,
            (true, _) => Gait::Idle,
            (false, false) => Gait::Walk,
            (false, true) => Gait::Run,
        }
    }
}

/// One of the party.
#[derive(Component)]
pub struct Member {
    pub slot: usize,
    pub name: &'static str,
    pub class: &'static str,
    pub barks: [&'static str; 3],
    pub portrait: Handle<Image>,
}

/// One of the village's folk, who is no one's to command.
#[derive(Component)]
pub struct Folk {
    pub name: &'static str,
}

#[derive(Component)]
pub struct Selected;

#[derive(Component)]
struct Animator {
    player: Entity,
    gait: Gait,
}

/// A clothing piece waiting to be bound to its body's skeleton.
#[derive(Component)]
struct Piece {
    body: Entity,
}

/// A clothing piece, for good (Piece goes once it is bound).
#[derive(Component)]
struct Clothing;

/// A held thing waiting for its body's hand.
#[derive(Component)]
struct Held {
    path: &'static str,
    socket: &'static str,
    at: Transform,
}

/// Walks to somewhere near home now and then.
#[derive(Component)]
pub struct Wander {
    home: Vec2,
    reach: f32,
    wait: f32,
}

/// Their model, inside the actor, turned to face where they walk.
#[derive(Component)]
struct Model;

struct Dress {
    pieces: &'static [&'static str],
    held: &'static [(&'static str, &'static str)],
}

const RANGER: &[&str] = &[
    "male_head",
    "male_ranger_body",
    "male_ranger_arms",
    "male_ranger_legs",
    "male_ranger_boots",
];
const PEASANT: &[&str] = &[
    "male_head",
    "male_peasant_body",
    "male_peasant_arms",
    "male_peasant_legs",
    "male_peasant_feet",
];

fn spawn(
    mut commands: Commands,
    asset: Res<AssetServer>,
    mut clips: ResMut<Clips>,
    mut images: ResMut<Assets<Image>>,
) {
    for (model, _) in KINDS {
        clips.loading.push((model, asset.load(*model)));
    }
    let party = [
        (
            "Aldric",
            "Fighter",
            Dress {
                pieces: &[],
                held: &[],
            },
            FIRE + Vec2::new(-1.5, 1.6),
            ["Aye?", "Lead on.", "My blade is yours."],
        ),
        (
            "Maren",
            "Ranger",
            Dress {
                pieces: RANGER,
                held: &[("models/held/bow.glb", "socket.hand.L")],
            },
            FIRE + Vec2::new(1.5, 1.4),
            ["Yes?", "I'll scout ahead.", "Quietly, now."],
        ),
        (
            "Tobin",
            "Thief",
            Dress {
                pieces: PEASANT,
                held: &[("models/held/knife.glb", "socket.hand.R")],
            },
            FIRE + Vec2::new(-1.9, 0.2),
            ["What?", "Easy money.", "Don't look at me."],
        ),
        (
            "Osric",
            "Cleric",
            Dress {
                pieces: PEASANT,
                held: &[("models/held/hammer.glb", "socket.hand.R")],
            },
            FIRE + Vec2::new(1.9, 0.8),
            ["Peace.", "The light guides us.", "I am ready."],
        ),
    ];
    for (slot, (name, class, dress, at, barks)) in party.into_iter().enumerate() {
        let portrait = images.add(Image::new_target_texture(
            96,
            120,
            bevy::render::render_resource::TextureFormat::Rgba8Unorm,
            Some(bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb),
        ));
        let to_fire = FIRE - at;
        let yaw = to_fire.x.atan2(to_fire.y);
        let knight = dress.pieces.is_empty();
        let (actor, body) = if knight {
            self::knight(&mut commands, &asset, at, yaw)
        } else {
            person(&mut commands, &asset, at, &dress, yaw)
        };
        // The camera is the body's child, so it is in the body's scale.
        let s = if knight { KNIGHT_SCALE } else { 1.0 };
        let face = if knight { 1.6 } else { 1.66 };
        commands.entity(actor).insert(Member {
            slot,
            name,
            class,
            barks,
            portrait: portrait.clone(),
        });
        // A portrait, drawn live from in front of their face.
        commands.spawn((
            Camera3d::default(),
            Camera {
                order: -1 - slot as isize,
                clear_color: Color::srgb(0.08, 0.07, 0.06).into(),
                ..default()
            },
            bevy::camera::RenderTarget::Image(portrait.into()),
            Projection::from(PerspectiveProjection {
                fov: 16f32.to_radians(),
                ..default()
            }),
            Transform::from_xyz(0.0, (face + 0.06) / s, 1.6 / s)
                .looking_at(Vec3::new(0.0, face / s, 0.0), Vec3::Y),
            ChildOf(body),
            PortraitOf(actor),
        ));
    }

}

#[derive(Component)]
pub struct PortraitOf(#[allow(dead_code)] pub Entity);

fn person(
    commands: &mut Commands,
    asset: &AssetServer,
    at: Vec2,
    dress: &Dress,
    yaw: f32,
) -> (Entity, Entity) {
    let actor = commands
        .spawn((
            Transform::from_xyz(at.x, 0.0, at.y),
            Visibility::default(),
            Actor::new(BODY, 1.45, 3.6, 1.85, 0.32),
        ))
        .id();
    let body = commands
        .spawn((
            WorldAssetRoot(asset.load(GltfAssetLabel::Scene(0).from_asset(BODY))),
            Transform::from_rotation(Quat::from_rotation_y(yaw)),
            Model,
            ChildOf(actor),
        ))
        .id();
    for piece in dress.pieces {
        commands.spawn((
            WorldAssetRoot(
                asset.load(GltfAssetLabel::Scene(0).from_asset(format!("{PIECES}/{piece}.gltf"))),
            ),
            Transform::default(),
            Piece { body },
            Clothing,
            ChildOf(body),
        ));
    }
    for (path, socket) in dress.held {
        commands.spawn((
            Held {
                path,
                socket,
                at: Transform::default(),
            },
            ChildOf(body),
            Transform::default(),
            Visibility::default(),
        ));
    }
    (actor, body)
}

/// The knight in his harness, sword in hand and shield on his arm.
fn knight(commands: &mut Commands, asset: &AssetServer, at: Vec2, yaw: f32) -> (Entity, Entity) {
    let actor = commands
        .spawn((
            Transform::from_xyz(at.x, 0.0, at.y),
            Visibility::default(),
            Actor::new(KNIGHT, 1.4, 3.4, 1.85, 0.34),
        ))
        .id();
    let body = commands
        .spawn((
            WorldAssetRoot(asset.load(GltfAssetLabel::Scene(0).from_asset(KNIGHT))),
            Transform::from_rotation(Quat::from_rotation_y(yaw))
                .with_scale(Vec3::splat(KNIGHT_SCALE)),
            Model,
            InPlace {
                bone: "pelvis",
                found: None,
            },
            ChildOf(actor),
        ))
        .id();
    // The male body's hand sockets, brought into the knight's smaller bones.
    let grip = |x: f32| Transform {
        translation: Vec3::new(x, 0.047, -0.003),
        rotation: Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
        scale: Vec3::splat(1.0 / KNIGHT_SCALE),
    };
    for (path, socket, at) in [
        ("models/held/arming_sword.glb", "hand_r", grip(-0.026)),
        ("models/held/heater_shield.glb", "hand_l", grip(0.026)),
    ] {
        commands.spawn((
            Held { path, socket, at },
            ChildOf(body),
            Transform::default(),
            Visibility::default(),
        ));
    }
    (actor, body)
}

/// A beast or bird: its actor, and its model turned to `yaw` and scaled.
pub fn creature(
    commands: &mut Commands,
    asset: &AssetServer,
    actor: Actor,
    at: Vec2,
    yaw: f32,
    scale: f32,
) -> Entity {
    let model = actor.model;
    let actor = commands
        .spawn((Transform::from_xyz(at.x, 0.0, at.y), Visibility::default(), actor))
        .id();
    commands.spawn((
        WorldAssetRoot(asset.load(GltfAssetLabel::Scene(0).from_asset(model))),
        Transform::from_rotation(Quat::from_rotation_y(yaw)).with_scale(Vec3::splat(scale)),
        Model,
        ChildOf(actor),
    ));
    actor
}

/// Each kind's idle, walk and run, made into a graph once its file has loaded.
fn make_clips(
    mut clips: ResMut<Clips>,
    gltfs: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
) {
    let clips = &mut *clips;
    clips.loading.retain(|(model, handle)| {
        let Some(gltf) = gltfs.get(handle) else {
            return true;
        };
        let names = KINDS.iter().find(|(m, _)| m == model).unwrap().1;
        let found: Vec<Handle<AnimationClip>> = names
            .iter()
            .map(|n| {
                gltf.named_animations.get(*n).cloned().unwrap_or_else(|| {
                    let have: Vec<_> = gltf.named_animations.keys().collect();
                    warn!("{model} has no clip {n}; it has {have:?}");
                    gltf.animations[0].clone()
                })
            })
            .collect();
        let (graph, nodes) = AnimationGraph::from_clips(found);
        clips.ready.insert(
            model,
            Set {
                graph: graphs.add(graph),
                nodes: [nodes[0], nodes[1], nodes[2], nodes[3]],
            },
        );
        false
    });
}

fn start_animators(
    mut commands: Commands,
    clips: Res<Clips>,
    actors: Query<(Entity, &Actor), Without<Animator>>,
    children: Query<&Children>,
    parents: Query<&ChildOf>,
    pieces: Query<(), With<Clothing>>,
    mut players: Query<&mut AnimationPlayer>,
) {
    for (entity, actor) in &actors {
        let Some(set) = clips.ready.get(actor.model) else {
            continue;
        };
        // The body's own player, not one inside a clothing piece (whose scene may come in
        // first, which left the body in its bind pose when it did).
        let Some(player) = children.iter_descendants(entity).find(|e| {
            players.contains(*e) && !parents.iter_ancestors(*e).any(|a| pieces.contains(a))
        }) else {
            continue;
        };
        let mut transitions = AnimationTransitions::new();
        let mut p = players.get_mut(player).unwrap();
        // Start each somewhere along its idle, so a crowd does not breathe as one.
        transitions
            .play(&mut p, set.nodes[0], Duration::ZERO)
            .repeat()
            .seek_to(entity.index().index() as f32 * 0.37 % 2.0);
        commands
            .entity(player)
            .insert((AnimationGraphHandle(set.graph.clone()), transitions));
        commands.entity(entity).insert(Animator {
            player,
            gait: Gait::Idle,
        });
    }
}

/// Binds each clothing piece's skinned meshes to its body's joints, by their names.
fn dress(
    mut commands: Commands,
    pieces: Query<(Entity, &Piece)>,
    children: Query<&Children>,
    names: Query<&Name>,
    mut skinned: Query<&mut SkinnedMesh>,
    players: Query<(), With<AnimationPlayer>>,
) {
    for (entity, piece) in &pieces {
        let meshes: Vec<Entity> = children
            .iter_descendants(entity)
            .filter(|e| skinned.contains(*e))
            .collect();
        if meshes.is_empty() {
            continue;
        }
        let mut joints: HashMap<&str, Entity> = HashMap::default();
        for e in children.iter_descendants(piece.body) {
            if e == entity || children.iter_descendants(entity).any(|d| d == e) {
                continue;
            }
            if let Ok(name) = names.get(e) {
                joints.entry(name.as_str()).or_insert(e);
            }
        }
        if !joints.contains_key("pelvis") {
            continue;
        }
        for mesh in meshes {
            let mut skin = skinned.get_mut(mesh).unwrap();
            for joint in skin.joints.iter_mut() {
                if let Some(&ours) = names.get(*joint).ok().and_then(|n| joints.get(n.as_str())) {
                    *joint = ours;
                }
            }
            commands.entity(mesh).insert(NoFrustumCulling);
        }
        // The piece's own skeleton no longer moves anything; nor should its player run.
        for e in children.iter_descendants(entity) {
            if players.contains(e) {
                commands.entity(e).remove::<AnimationPlayer>();
            }
        }
        commands.entity(entity).remove::<Piece>();
    }
}

/// Hangs each held thing in its hand's socket once the body's skeleton is there.
fn hold(
    mut commands: Commands,
    asset: Res<AssetServer>,
    held: Query<(Entity, &Held, &ChildOf)>,
    children: Query<&Children>,
    names: Query<&Name>,
) {
    for (entity, h, parent) in &held {
        let Some(socket) = children
            .iter_descendants(parent.parent())
            .find(|e| names.get(*e).is_ok_and(|n| n.as_str() == h.socket))
        else {
            continue;
        };
        commands.entity(entity).despawn();
        commands.spawn((
            WorldAssetRoot(asset.load(GltfAssetLabel::Scene(0).from_asset(h.path))),
            h.at,
            ChildOf(socket),
        ));
    }
}

/// A model whose clips carry root motion (Tripo's walk and run move the pelvis forward up
/// to 1.5 and 3.5 m along its local x): the walker moves the actor itself, so the bone is
/// held over the actor's feet, or the body walks away from its ring, its shadow and its
/// portrait camera and snaps back at each loop.
#[derive(Component)]
struct InPlace {
    bone: &'static str,
    found: Option<Entity>,
}

fn in_place(
    mut bodies: Query<(Entity, &mut InPlace)>,
    children: Query<&Children>,
    names: Query<&Name>,
    mut bones: Query<&mut Transform>,
) {
    for (body, mut pin) in &mut bodies {
        if pin.found.is_none() {
            pin.found = children
                .iter_descendants(body)
                .find(|e| names.get(*e).is_ok_and(|n| n.as_str() == pin.bone));
        }
        if let Some(bone) = pin.found
            && let Ok(mut tr) = bones.get_mut(bone)
        {
            tr.translation.x = 0.0;
        }
    }
}

/// Follows each actor's path, turning toward where they go.
fn walk(
    time: Res<Time>,
    mut actors: Query<(&mut Actor, &mut Transform, &Children), Without<Model>>,
    mut models: Query<&mut Transform, With<Model>>,
) {
    let dt = time.delta_secs();
    if dt == 0.0 {
        return;
    }
    for (mut actor, mut tr, kids) in &mut actors {
        let Some(&next) = actor.path.front() else {
            continue;
        };
        let here = tr.translation.xz();
        let speed = if actor.running { actor.run } else { actor.walk };
        let to = next - here;
        let step = speed * dt;
        let moved = if to.length() <= step {
            actor.path.pop_front();
            next
        } else {
            here + to.normalize() * step
        };
        if actor.path.is_empty() {
            actor.running = false;
        }
        tr.translation = Vec3::new(moved.x, tr.translation.y, moved.y);
        if to.length() > 0.01 {
            // The models face +Z.
            let want = Quat::from_rotation_y(to.x.atan2(to.y));
            for kid in kids {
                if let Ok(mut m) = models.get_mut(*kid) {
                    m.rotation = m.rotation.slerp(want, (dt * 10.0).min(1.0));
                }
            }
        }
    }
}

/// Everyone stands on the land.
fn stand(land: Res<Land>, mut actors: Query<(&mut Transform, &Actor)>) {
    for (mut tr, actor) in &mut actors {
        let ground = land.height(tr.translation.xz());
        let y = if actor.swims {
            // A swimmer's body rides at the waterline, its legs under it.
            crate::terrain::WATER - 0.11 * tr.scale.y
        } else {
            ground + actor.lift
        };
        if tr.translation.y != y {
            tr.translation.y = y;
        }
    }
}

/// Changes each animator's clip when its actor starts or stops walking or running.
fn gait(
    clips: Res<Clips>,
    mut actors: Query<(&Actor, &mut Animator)>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
) {
    for (actor, mut animator) in &mut actors {
        let gait = actor.gait();
        if gait == animator.gait {
            continue;
        }
        let Some(set) = clips.ready.get(actor.model) else {
            continue;
        };
        let Ok((mut player, mut transitions)) = players.get_mut(animator.player) else {
            continue;
        };
        let node = set.nodes[gait as usize];
        transitions
            .play(&mut player, node, Duration::from_millis(220))
            .repeat();
        animator.gait = gait;
    }
}

/// The folk and animals walk to somewhere near home now and then.
fn wander(time: Res<Time>, nav: Res<Nav>, mut folk: Query<(&mut Actor, &mut Wander, &Transform)>) {
    let dt = time.delta_secs();
    for (mut actor, mut wander, tr) in &mut folk {
        if !actor.path.is_empty() {
            continue;
        }
        wander.wait -= dt;
        if wander.wait > 0.0 {
            continue;
        }
        let t =
            time.elapsed_secs() * 12.9898 + tr.translation.x * 78.233 + tr.translation.z * 37.719;
        let r1 = (t.sin() * 43758.545).fract().abs();
        let r2 = ((t * 1.7).cos() * 24634.634).fract().abs();
        let goal =
            wander.home + Vec2::from_angle(r1 * std::f32::consts::TAU) * wander.reach * r2.sqrt();
        if let Some(path) = nav.path(tr.translation.xz(), goal) {
            actor.path = path.into();
        }
        wander.wait = 3.0 + r2 * 6.0;
    }
}
