//! Doors and shutters (construction-001, rule 7 and gap 15; D192, D207, D225): props a
//! base holds in a hole of built cubes, fitted by `terra_build`'s operations. Each is drawn
//! from its kind's model, a leaf or a pair hung on the hole's sides as
//! `terra_build::fitting` says, so what is seen is what stops a walker; it swings open and
//! shut when used, the player's through the same operation the remote uses, a village's
//! straight on its house. The kinds and their models are the catalogue's (`builds.ron`'s
//! `fittings`), told to this layer as [`FittingLooks`].

use super::act::Building;
use super::{Built, Site};
use crate::play::interact::{CanInteract, Interactable, Interacted};
use bevy::asset::LoadState;
use bevy::camera::primitives::MeshAabb;
use bevy::camera::visibility::VisibilityRange;
use bevy::gltf::GltfAssetLabel;
use bevy::prelude::*;
use bevy::render::render_resource::Face;
use std::collections::BTreeMap;
use terra_build::fitting::{Hang, Leaf};
use terra_build::ops::{Context, Op};
use terra_build::{Prop, PropKind, Rules};

/// How long a leaf takes to swing all the way, in seconds.
const SWING_S: f32 = 0.7;
/// How thick a door's leaf is drawn, and a shutter's, its hardware included, whatever its
/// model's own depth.
const DOOR_DRAWN_M: f32 = 0.08;
const SHUTTER_DRAWN_M: f32 = 0.05;
/// How far off doors and shutters are drawn: as far as a building's near detail.
const SEEN_M: f32 = 140.0;

/// Each kind of door's and shutters' model, by the kind of prop it is, as the catalogue
/// says: its file, and whether its hinges are on its left seen from its front.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct FittingLooks(pub BTreeMap<PropKind, FittingLook>);

#[derive(Clone, Debug, PartialEq)]
pub struct FittingLook {
    pub path: String,
    pub hinge_left: bool,
}

/// A model loaded: its mesh, its material and the same drawn mirrored, and the box round it
/// in its own space.
#[derive(Clone, Debug)]
struct Model {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
    mirrored: Handle<StandardMaterial>,
    min: Vec3,
    max: Vec3,
}

/// The models, as they load, by kind.
#[derive(Resource, Default)]
struct Models {
    wanted: Option<FittingLooks>,
    loading: BTreeMap<PropKind, (Handle<Mesh>, Handle<StandardMaterial>)>,
    ready: BTreeMap<PropKind, Model>,
    /// Goes up every time models finish loading, so what is drawn is drawn again.
    generation: u64,
}

/// A door or shutters drawn where a base has one fitted: whose base (a village house's
/// entity, or none for the player's), its lowest cube, and how it was fitted.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Fitted {
    pub owner: Option<Entity>,
    pub prop: Prop,
}

/// How far open it stands now, 0 shut to 1 open, as it swings toward where it is.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Swing(pub f32);

/// A leaf's hinge, turning it about the up axis by up to `swing` radians.
#[derive(Component, Clone, Copy, Debug)]
struct Hinged {
    axis: Vec3,
    swing: f32,
}

/// The fitted prop being previewed where the hammer would fit it, drawn as a ghost; set by
/// the hammer.
#[derive(Resource, Default, Clone, Copy, PartialEq)]
pub struct FitPreview(pub Option<Prop>);

/// The ghost of a [`FitPreview`].
#[derive(Component)]
struct Previewed(Prop);

pub fn plugin(app: &mut App) {
    app.init_resource::<FittingLooks>()
        .init_resource::<Models>()
        .init_resource::<FitPreview>()
        .add_systems(Update, (load, show, swing, preview).chain())
        .add_systems(
            FixedUpdate,
            open_and_shut.after(crate::play::interact::Focusing),
        );
}

/// Loads each kind's model when the catalogue names them, and when all have loaded draws
/// every door and shutters again.
fn load(
    looks: Res<FittingLooks>,
    mut models: ResMut<Models>,
    assets: Res<AssetServer>,
    meshes: Res<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if models.wanted.as_ref() != Some(&*looks) {
        let loading = looks
            .0
            .iter()
            .map(|(&kind, look)| {
                let mesh = assets.load(
                    GltfAssetLabel::Primitive {
                        mesh: 0,
                        primitive: 0,
                    }
                    .from_asset(look.path.clone()),
                );
                // The loader keeps each material as a standard one too, under its label
                // and "/std".
                let material = assets.load(format!("{}#Material0/std", look.path));
                (kind, (mesh, material))
            })
            .collect();
        models.loading = loading;
        models.ready.clear();
        models.wanted = Some(looks.clone());
        return;
    }
    if models.loading.is_empty() {
        return;
    }
    let mut ready = BTreeMap::new();
    for (&kind, (mesh, material)) in &models.loading {
        if let Some(LoadState::Failed(problem)) = assets.get_load_state(mesh) {
            error!("A door's or shutters' model did not load: {problem}");
            continue;
        }
        let (Some(found), Some(made)) = (meshes.get(mesh), materials.get(material)) else {
            return;
        };
        let Some(aabb) = found.get_aabb() else {
            continue;
        };
        // Drawn mirrored, for one of a pair, its faces' winding is turned, so the other
        // side of each is culled, as the glTF loader does for a mirrored node.
        let mirrored = StandardMaterial {
            cull_mode: Some(Face::Front),
            ..made.clone()
        };
        ready.insert(
            kind,
            Model {
                mesh: mesh.clone(),
                material: material.clone(),
                mirrored: materials.add(mirrored),
                min: Vec3::from(aabb.min()),
                max: Vec3::from(aabb.max()),
            },
        );
    }
    info!("Doors and shutters: {} models loaded.", ready.len());
    models.ready = ready;
    models.loading.clear();
    models.generation += 1;
}

/// Where a leaf's model goes, from its hinge, standing shut, in the base's own space less
/// `origin`: its model's own space turned and scaled so its hinge edge is on the hinge,
/// its foot on the foot, its front out for a door and the way it opens for shutters, as
/// thick as such a leaf is drawn. Whether it is drawn mirrored.
fn placed(model: &Model, leaf: &Leaf, hinge_left: bool) -> (Transform, bool) {
    let pose = leaf.pose(0.0);
    let size = (model.max - model.min).max(Vec3::splat(1e-3));
    let thick = if leaf.door {
        DOOR_DRAWN_M
    } else {
        SHUTTER_DRAWN_M
    };
    // The model's X along the leaf from its hinge edge; its Z, its front, out of the
    // leaf's through way for a door, against it for shutters.
    let x = if hinge_left {
        pose.across
    } else {
        -pose.across
    };
    let z = if leaf.door {
        pose.through
    } else {
        -pose.through
    };
    let scale = Vec3::new(leaf.width / size.x, leaf.height / size.y, thick / size.z);
    // What it does to the model's own space, which may mirror it: drawn then as a turn
    // and a scale negative along the model's X, with the mirrored material.
    let linear = Mat3::from_cols(x * scale.x, Vec3::Y * scale.y, z * scale.z);
    let mirrored = x.cross(Vec3::Y).dot(z) < 0.0;
    let turn_x = if mirrored { -x } else { x };
    // The model's hinge edge, foot and middle of its depth, on the hinge, in the middle of
    // the leaf's thickness.
    let hinge_edge = if hinge_left { model.min.x } else { model.max.x };
    let anchor = Vec3::new(hinge_edge, model.min.y, (model.min.z + model.max.z) * 0.5);
    let transform = Transform {
        translation: pose.through * (leaf.thick * 0.5) - linear * anchor,
        rotation: Quat::from_mat3(&Mat3::from_cols(turn_x, Vec3::Y, z)),
        scale: Vec3::new(if mirrored { -scale.x } else { scale.x }, scale.y, scale.z),
    };
    (transform, mirrored)
}

/// The middle of a prop's foot, in its base's own space: where it is used from.
fn foot(prop: &Prop) -> Vec3 {
    let low = IVec3::from_array(prop.min).as_vec3() * terra_build::CELL_M;
    let high = (IVec3::from_array(prop.max) + IVec3::ONE).as_vec3() * terra_build::CELL_M;
    Vec3::new((low.x + high.x) * 0.5, low.y, (low.z + high.z) * 0.5)
}

/// Spawns a prop's leaves under `parent`, each on its hinge, turned `open` of the way.
fn spawn_leaves(
    commands: &mut Commands,
    parent: Entity,
    prop: &Prop,
    model: &Model,
    look: &FittingLook,
    open: f32,
    material: Option<MeshMaterial3d<crate::build::draw::GhostMaterial>>,
) {
    let origin = foot(prop);
    for leaf in prop.leaves() {
        let pose = leaf.pose(0.0);
        let axis = pose.across.cross(leaf.opens).normalize_or(Vec3::Y);
        let (at, mirrored) = placed(model, &leaf, look.hinge_left);
        let hinge = commands
            .spawn((
                Name::new("Leaf"),
                Hinged {
                    axis,
                    swing: leaf.swing,
                },
                Transform::from_translation(leaf.hinge - origin)
                    .with_rotation(Quat::from_axis_angle(axis, leaf.swing * open)),
                Visibility::default(),
                ChildOf(parent),
            ))
            .id();
        let mut part = commands.spawn((Mesh3d(model.mesh.clone()), at, ChildOf(hinge)));
        match &material {
            Some(ghost) => part.insert(ghost.clone()),
            None if mirrored => part.insert(MeshMaterial3d(model.mirrored.clone())),
            None => part.insert(MeshMaterial3d(model.material.clone())),
        };
    }
}

/// Draws each prop every base has fitted, as it is, whenever a base or the models change;
/// a prop only opened or shut is left to swing.
#[allow(clippy::type_complexity)]
fn show(
    mut commands: Commands,
    site: Option<Res<Site>>,
    builds: Query<(Entity, &Built)>,
    models: Res<Models>,
    looks: Res<FittingLooks>,
    mut drawn: Query<(Entity, &mut Fitted)>,
    mut seen: Local<BTreeMap<Option<Entity>, (u64, u64)>>,
) {
    let mut bases: Vec<(Option<Entity>, &super::Build)> = Vec::new();
    if let Some(site) = site.as_deref() {
        bases.push((None, &site.build));
    }
    bases.extend(
        builds
            .iter()
            .map(|(entity, built)| (Some(entity), &built.0)),
    );
    let mut changed = BTreeMap::new();
    for (owner, build) in &bases {
        let now = (build.revision, models.generation);
        if seen.get(owner) != Some(&now) {
            seen.insert(*owner, now);
            changed.insert(*owner, *build);
        }
    }
    if changed.is_empty() {
        return;
    }
    // What each changed base has fitted, by its lowest cube.
    let mut wanted: BTreeMap<(Option<Entity>, [i32; 3]), Prop> = changed
        .iter()
        .flat_map(|(&owner, build)| {
            build
                .base
                .props()
                .map(move |prop| ((owner, prop.min), *prop))
        })
        .collect();
    for (entity, mut fitted) in &mut drawn {
        if !changed.contains_key(&fitted.owner) {
            continue;
        }
        let key = (fitted.owner, fitted.prop.min);
        match wanted.get(&key) {
            // Only opened or shut: it swings there.
            Some(prop)
                if Prop {
                    open: fitted.prop.open,
                    ..*prop
                } == fitted.prop =>
            {
                fitted.prop = *prop;
                wanted.remove(&key);
            }
            _ => commands.entity(entity).despawn(),
        }
    }
    for ((owner, _), prop) in wanted {
        let (Some(model), Some(look)) = (models.ready.get(&prop.kind), looks.0.get(&prop.kind))
        else {
            continue;
        };
        let Some(build) = changed.get(&owner) else {
            continue;
        };
        let door = prop.hang == Hang::Door;
        let open = if prop.open { 1.0 } else { 0.0 };
        let mut entity = commands.spawn((
            Name::new(if door { "Door" } else { "Shutters" }),
            Fitted { owner, prop },
            Swing(open),
            Transform::from_translation(foot(&prop)),
            Visibility::default(),
            VisibilityRange {
                start_margin: 0.0..0.0,
                end_margin: SEEN_M - 10.0..SEEN_M,
                use_aabb: false,
            },
            ChildOf(build.root),
        ));
        entity.insert(CanInteract(if door {
            Interactable::Door
        } else {
            Interactable::Shutters
        }));
        let parent = entity.id();
        spawn_leaves(&mut commands, parent, &prop, model, look, open, None);
    }
}

/// Swings each prop's leaves toward where it stands, open or shut, eased in and out.
fn swing(
    time: Res<Time>,
    mut props: Query<(&Fitted, &mut Swing, &Children)>,
    mut hinges: Query<(&Hinged, &mut Transform)>,
) {
    let step = time.delta_secs() / SWING_S;
    for (fitted, mut swing, children) in &mut props {
        let target = if fitted.prop.open { 1.0 } else { 0.0 };
        if swing.0 == target {
            continue;
        }
        swing.0 = if target > swing.0 {
            (swing.0 + step).min(target)
        } else {
            (swing.0 - step).max(target)
        };
        let eased = swing.0 * swing.0 * (3.0 - 2.0 * swing.0);
        for &child in children {
            if let Ok((hinged, mut transform)) = hinges.get_mut(child) {
                transform.rotation = Quat::from_axis_angle(hinged.axis, hinged.swing * eased);
            }
        }
    }
}

/// Draws the ghost of what the hammer would fit, where it would go, while it is aimed.
fn preview(
    mut commands: Commands,
    wanted: Res<FitPreview>,
    site: Option<Res<Site>>,
    models: Res<Models>,
    looks: Res<FittingLooks>,
    look: Option<Res<super::BuildLook>>,
    shown: Query<(Entity, &Previewed)>,
) {
    let have = shown.iter().next().map(|(_, previewed)| previewed.0);
    if have == wanted.0 && !models.is_changed() {
        return;
    }
    for (entity, _) in &shown {
        commands.entity(entity).despawn();
    }
    let (Some(prop), Some(site), Some(look)) = (wanted.0, site, look) else {
        return;
    };
    let (Some(model), Some(fitting)) = (models.ready.get(&prop.kind), looks.0.get(&prop.kind))
    else {
        return;
    };
    let parent = commands
        .spawn((
            Name::new("Fitting preview"),
            Previewed(prop),
            Transform::from_translation(foot(&prop)),
            Visibility::default(),
            ChildOf(site.root),
        ))
        .id();
    let ghost = MeshMaterial3d(look.looks.ghost());
    spawn_leaves(
        &mut commands,
        parent,
        &prop,
        model,
        fitting,
        0.0,
        Some(ghost),
    );
}

/// Opens or shuts a door or shutters someone uses: the player's base's through the same
/// operation the remote uses, a village house's straight on its base.
fn open_and_shut(
    mut used: MessageReader<Interacted>,
    fitted: Query<&Fitted>,
    mut built: Query<&mut Built>,
    mut building: Building,
) {
    for interacted in used.read() {
        let Some(found) = interacted
            .on(Interactable::Door)
            .or(interacted.on(Interactable::Shutters))
            .and_then(|entity| fitted.get(entity).ok())
        else {
            continue;
        };
        let open = !found.prop.open;
        let what = match (found.prop.hang, open) {
            (Hang::Door, true) => "open the door",
            (Hang::Door, false) => "shut the door",
            (Hang::Shutters, true) => "open the shutters",
            (Hang::Shutters, false) => "shut the shutters",
        };
        let ops = [Op::SetOpen {
            at: found.prop.min,
            open,
        }];
        match found.owner {
            None => {
                let _ = building.act(Some(interacted.player), what, &ops);
            }
            Some(owner) => {
                if let Ok(mut built) = built.get_mut(owner) {
                    let rules = Rules::default();
                    if let Err(why) = built.act(&ops, &Context::free(&rules)) {
                        info!("Fitted: could not {what}: {}", why.message());
                    } else {
                        info!("Fitted: {what} in a village house.");
                    }
                }
            }
        }
    }
}

/// Fits doors and shutters into every hole of a house that is not the player's, as its
/// builders would have: a doorway, a hole from its storey's floor, takes a door, and a
/// window, a hole over a sill, shutters, of the kinds `choose` gives for each hang and size
/// (wide by high, in cubes), with whether it stands open. Says how many were fitted.
pub(crate) fn fit_all(
    build: &mut super::Build,
    choose: impl Fn(Hang, (i32, i32), usize) -> Option<(PropKind, bool)>,
) -> usize {
    let base = &build.base;
    let mut holes: BTreeMap<[i32; 3], IVec3> = BTreeMap::new();
    for (at, cube) in base.cubes() {
        if cube.fill != terra_build::Fill::Open || !base.in_wall(at) {
            continue;
        }
        if let Some((low, _)) = super::plans::hole(base, at) {
            holes.entry(low.to_array()).or_insert(at);
        }
    }
    let mut ops = Vec::new();
    for (index, (low, at)) in holes.into_iter().enumerate() {
        let hang = super::plans::hang_for(base, IVec3::from_array(low));
        let Some(probe) = super::plans::prop_for(base, at, PropKind(0), hang) else {
            continue;
        };
        let Some((kind, open)) = choose(hang, super::plans::hole_size(&probe), index) else {
            continue;
        };
        ops.push(Op::FitProp(Prop {
            kind,
            open,
            ..probe
        }));
    }
    let rules = Rules::default();
    let cx = Context::free(&rules);
    let mut fitted = 0;
    // One at a time, so a hole the rules refuse (one not held all round) is left open.
    for op in ops {
        if build.act(std::slice::from_ref(&op), &cx).is_ok() {
            fitted += 1;
        }
    }
    fitted
}
