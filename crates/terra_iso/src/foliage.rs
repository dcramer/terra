//! Foliage that lives: the grass, flowers, leaf cards and bark drawn with the standard
//! material extended (foliage.wesl) so the wind sways them, and the wood before the party
//! cut through with a dithered hole about each of them rather than vanishing whole, as
//! Pillars of Eternity II and Pathfinder: Kingmaker see their figures through the trees.
//! A vertex's sway weight rides in `uv_b` (x how far up the blade, y 1 for a leaf card).

use crate::camera::MainCamera;
use crate::party::Member;
use bevy::asset::embedded_asset;
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;

pub type FoliageMaterial = ExtendedMaterial<StandardMaterial, FoliageExt>;

#[derive(Clone, Copy, Debug, Default, Reflect, ShaderType)]
pub struct Foliage {
    pub wind: Vec4,
    pub leaf: Vec4,
    pub right: Vec4,
    pub up: Vec4,
    pub forward: Vec4,
    pub party: [Vec4; 6],
    /// x: how far each texel of the card goes to its own grey before the vertex colour
    /// tints it, 0 (as it is) to 1 (a plant set's `grey`, kit/scatter.rs).
    pub tone: Vec4,
    /// xyz: the camera; w: play's near band (m, `Near`), 0 off (the loop's stills and films).
    pub eye: Vec4,
    /// Ground cover's hole (`Hole::Cover`): xyz the hero's middle, w its radius (`COVER`),
    /// set from rim.rs `Cut`, so in play only; 0 cuts nothing (a still).
    pub cover: Vec4,
}

/// Which hole a foliage material opens (`leaf.y`): none (grass, blooms), the party's
/// (`HOLE`, the crowns and bark, stills too), or the cover's about the hero in play alone
/// (`COVER`, the scatter's plant cards, kit/scatter.rs `plants`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hole {
    None,
    Party,
    Cover,
}

impl Hole {
    fn code(self) -> f32 {
        match self {
            Hole::None => 0.0,
            Hole::Party => 1.0,
            Hole::Cover => 2.0,
        }
    }

    /// The hole a material's `leaf.y` says it opens.
    pub fn of(leaf: Vec4) -> Hole {
        if leaf.y > 1.5 {
            Hole::Cover
        } else if leaf.y > 0.5 {
            Hole::Party
        } else {
            Hole::None
        }
    }
}

/// Ground cover's hole about his middle on the screen (m): smaller than the crowns' `HOLE`,
/// as rim.rs `CUT` is for the cover's models, since a plant card before his legs or chest
/// is a metre from him at most; whole within 0.72 of it.
pub const COVER: f32 = 1.2;

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct FoliageExt {
    #[uniform(100)]
    pub foliage: Foliage,
}

const SHADER: &str = "embedded://iso/shaders/foliage.wesl";

impl MaterialExtension for FoliageExt {
    fn vertex_shader() -> ShaderRef {
        SHADER.into()
    }
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }
    fn prepass_vertex_shader() -> ShaderRef {
        SHADER.into()
    }
    fn prepass_fragment_shader() -> ShaderRef {
        SHADER.into()
    }
}

/// The wind's direction over the clearing (from the west-south-west).
pub const WIND: Vec2 = Vec2::new(0.86, 0.5);
/// The hole's radius about each of the party (m, on the screen).
pub const HOLE: f32 = 2.8;

/// One the wood opens about as about the party, not one of it (a scene's fighters).
#[derive(Component)]
pub struct Seen;

/// Play's cut (demo/occlude.rs): a tree any part of which comes nearer the camera than this
/// (m) fades out whole (`Tree`), so no bough nears the lens; and the hole about the hero
/// opens from a hand's breadth before him, not a metre. Absent, the stills' and films' cut is
/// as it was.
#[derive(Resource, Clone, Copy)]
pub struct Near(pub f32);

/// A tree that fades as one object in play: trunk, boughs and crown together, over `FADE`
/// seconds, when any of it comes within `Near` of the camera, and back when all of it is
/// `HYSTERESIS` past that. At rest it is drawn whole or not at all: no screen door stands
/// over it, only its 0.3 s turn dithers. The Witcher 3's per-instance tree fade and the
/// isometric ARPGs' (Grim Dawn, Path of Exile) whole-prop fades: a per-pixel fade by distance
/// left a dithered band across every crown the near sphere cut (iso-forest-demo j17). Its
/// meshes carry the fade in their `MeshTag` (0 drawn to `TAG` gone), read by foliage.wesl.
#[derive(Component, Default, Clone, Copy, Debug)]
pub struct Tree {
    /// 0 drawn to 1 gone.
    pub fade: f32,
    /// Whether it is fading out (or out).
    pub gone: bool,
}

/// How long a tree takes to fade out or in (s).
pub const FADE: f32 = 0.3;
/// How far past `Near` all of a faded tree must be before it comes back (m), so one at the
/// band's edge does not flicker.
pub const HYSTERESIS: f32 = 0.75;
/// A mesh tag's whole fade (foliage.wesl `TAG`).
pub const TAG: u32 = 1000;

impl Tree {
    /// One step of `dt` s, with the tree's nearest point `nearest` m from the camera and the
    /// near band `near` m: whether it should be gone, then the fade toward it, linear (the
    /// shader's dither is the curve).
    pub fn step(&mut self, nearest: f32, near: f32, dt: f32) {
        if nearest < near {
            self.gone = true;
        } else if nearest > near + HYSTERESIS {
            self.gone = false;
        }
        let to = if self.gone { 1.0 } else { 0.0 };
        let d = dt / FADE;
        self.fade = if to > self.fade {
            (self.fade + d).min(to)
        } else {
            (self.fade - d).max(to)
        };
    }

    /// Its mesh tag.
    pub fn tag(&self) -> u32 {
        (self.fade * TAG as f32).round() as u32
    }
}

/// How far `eye` is from the box `aabb` placed by `at` (m): the eye taken into the box's
/// frame, clamped to it and taken back, so a scaled or turned box is met exactly.
pub fn box_distance(eye: Vec3, aabb: &bevy::camera::primitives::Aabb, at: &GlobalTransform) -> f32 {
    let affine = at.affine();
    let local = affine.inverse().transform_point3(eye);
    let (lo, hi) = (aabb.min(), aabb.max());
    let near = local.clamp(Vec3::from(lo), Vec3::from(hi));
    affine.transform_point3(near).distance(eye)
}

/// How much of a cuttable surface at `world` the hole drops (0 kept, 1 gone), the twin of
/// foliage.wesl's `cut` without its dither and its tree's whole fade (`Tree`): `party` the
/// hole's middles, `r` its radius (`HOLE`, `COVER`), `axes` the camera's right, up and
/// forward, `eye` its place and whether it is play (w over 0).
pub fn fade(world: Vec3, party: &[Vec3], r: f32, axes: [Vec3; 3], eye: Vec4) -> f32 {
    let smooth = |e0: f32, e1: f32, x: f32| {
        let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    };
    let play = eye.w > 0.0;
    let (lo, hi) = if play { PLAY_BEFORE } else { (0.4, 1.4) };
    let mut f: f32 = 0.0;
    for m in party {
        let d = world - *m;
        let s = Vec2::new(d.dot(axes[0]), d.dot(axes[1])).length();
        let before = -d.dot(axes[2]);
        f = f.max((1.0 - smooth(r * 0.72, r, s)) * smooth(lo, hi, before));
    }
    f
}

/// Play's whole-tree fade: each tree's nearest part to the camera, its `Tree` stepped, and
/// its meshes' tags set where they changed.
fn fade_trees(
    time: Res<Time>,
    near: Option<Res<Near>>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
    mut trees: Query<(&mut Tree, &Children)>,
    parts: Query<(&bevy::camera::primitives::Aabb, &GlobalTransform), With<Mesh3d>>,
    mut tags: Query<&mut bevy::mesh::MeshTag>,
    mut commands: Commands,
) {
    let (Some(near), Ok(cam)) = (near, camera.single()) else {
        return;
    };
    let eye = cam.translation();
    for (mut tree, children) in &mut trees {
        let nearest = children
            .iter()
            .filter_map(|c| parts.get(c).ok())
            .map(|(aabb, at)| box_distance(eye, aabb, at))
            .fold(f32::INFINITY, f32::min);
        if !nearest.is_finite() {
            continue;
        }
        tree.step(nearest, near.0, time.delta_secs());
        let tag = tree.tag();
        for c in children.iter().filter(|c| parts.contains(*c)) {
            match tags.get_mut(c) {
                Ok(mut t) if t.value != tag => t.value = tag,
                Ok(_) => {}
                Err(_) => {
                    commands.entity(c).insert(bevy::mesh::MeshTag::new(tag));
                }
            }
        }
    }
}

/// Play's ramp of the hole along the view (m before the hero's middle): wholly open from
/// 0.45, so a trunk at his shoulder is cut, not only one a metre and more before him.
const PLAY_BEFORE: (f32, f32) = (0.05, 0.45);

pub fn plugin(app: &mut App) {
    embedded_asset!(app, "shaders/foliage.wesl");
    app.add_plugins(MaterialPlugin::<FoliageMaterial>::default())
        .init_resource::<Mats>()
        .add_systems(
            PostUpdate,
            (breathe, fade_trees, cover).after(TransformSystems::Propagate),
        );
}

#[derive(Resource, Default)]
pub struct Mats(Vec<Handle<FoliageMaterial>>);

/// A foliage material: `bend` how far a blade's tip goes in the wind (m), `flutter` how far a
/// leaf card does, `hole` the hole it opens.
pub fn material(
    materials: &mut Assets<FoliageMaterial>,
    mats: &mut Mats,
    base: StandardMaterial,
    bend: f32,
    flutter: f32,
    hole: Hole,
) -> Handle<FoliageMaterial> {
    let r = if hole == Hole::Cover { COVER } else { HOLE };
    let h = materials.add(ExtendedMaterial {
        base,
        extension: FoliageExt {
            foliage: Foliage {
                wind: Vec4::new(WIND.x, WIND.y, bend, 0.0),
                leaf: Vec4::new(flutter, hole.code(), r, 0.0),
                ..default()
            },
        },
    });
    mats.0.push(h.clone());
    h
}

/// The cover's hole (rim.rs `Cut`, his middle, set in play alone by demo/occlude.rs `aim`)
/// to every `Hole::Cover` material: the scatter's plant cards before him dither out as the
/// cover's models do, so a fern card at his chest no longer buries him (iso-forest-demo
/// j27: 47.6% of a box on him green at frame 490). A still has no `Cut`, so cuts nothing.
pub fn cover(
    at: Option<Res<crate::rim::Cut>>,
    mats: Res<Mats>,
    mut materials: ResMut<Assets<FoliageMaterial>>,
) {
    let open = at.and_then(|c| c.0).map_or(Vec4::ZERO, |m| m.extend(COVER));
    for h in &mats.0 {
        let stale = materials.get(h).is_some_and(|m| {
            let f = &m.extension.foliage;
            Hole::of(f.leaf) == Hole::Cover && f.cover != open
        });
        if stale && let Some(mut m) = materials.get_mut(h) {
            m.extension.foliage.cover = open;
        }
    }
}

/// The wind's time, the camera's axes and the party's places, to every foliage material.
fn breathe(
    time: Res<Time>,
    mats: Res<Mats>,
    mut materials: ResMut<Assets<FoliageMaterial>>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
    party: Query<
        (&GlobalTransform, Option<&crate::ruin::walk::WalkIn>),
        Or<(With<Member>, With<Seen>)>,
    >,
    land: Res<crate::terrain::Land>,
    frames: Res<bevy::diagnostic::FrameCount>,
    near: Option<Res<Near>>,
) {
    let Ok(cam) = camera.single() else {
        return;
    };
    let eye = cam.translation().extend(near.map_or(0.0, |n| n.0));
    // The party, and the camp fire they gather at.
    let mut at = [Vec4::ZERO; 6];
    let fire = crate::scene::FIRE;
    at[0] = Vec3::new(fire.x, land.height(fire) + 0.6, fire.y).extend(1.0);
    // Each of them, and where one walking in to fight will stand (ruin/walk.rs): the film's
    // hero walks in under the oak before the fight, which a still shows open about him; cut
    // only about him as he goes, it stood shut over the camp till he got there, a flat green
    // burst of leaf cards over the forest film's first 2 to 3 s. In a still he stands at his
    // walk's end, so the second hole is the first's.
    let places = party.iter().flat_map(|(t, walk)| {
        let end = walk
            .filter(|w| w.fight)
            .map(|w| w.to.extend(w.floor.at(w.to)).xzy());
        std::iter::once(t.translation()).chain(end)
    });
    for (slot, p) in at[1..].iter_mut().zip(places) {
        *slot = (p + Vec3::Y * 0.9).extend(1.0);
    }
    let t = time.elapsed_secs_wrapped();
    for h in &mats.0 {
        if let Some(mut m) = materials.get_mut(h) {
            let f = &mut m.extension.foliage;
            f.wind.w = t;
            f.right = cam.right().extend(0.0);
            f.up = cam.up().extend(0.0);
            f.forward = cam.forward().extend(0.0);
            f.party = at;
            // The frame, for the hole's dither to turn each frame and TAA to average.
            f.leaf.w = (frames.0 % 64) as f32;
            f.eye = eye;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The cover's hole reaches the plant cards' materials alone (never the crowns', bark's,
    /// grass's or blooms'), follows the hero, and closes when there is no `Cut` (a still);
    /// the crowns keep their own party hole and radius throughout.
    #[test]
    fn the_cover_hole_opens_the_plant_cards_alone_and_only_in_play() {
        let mut app = App::new();
        app.init_resource::<Assets<FoliageMaterial>>()
            .init_resource::<Mats>()
            .add_systems(Update, cover);
        let holes = [Hole::Cover, Hole::Party, Hole::None, Hole::Cover];
        let made: Vec<_> = holes
            .iter()
            .map(|h| {
                let world = app.world_mut();
                let mut mats = world.remove_resource::<Mats>().unwrap();
                let mut materials = world.resource_mut::<Assets<FoliageMaterial>>();
                let m = material(&mut materials, &mut mats, default(), 0.0, 0.0, *h);
                world.insert_resource(mats);
                m
            })
            .collect();
        let read = |app: &App| -> Vec<(Hole, f32, Vec4)> {
            let materials = app.world().resource::<Assets<FoliageMaterial>>();
            made.iter()
                .map(|h| {
                    let f = materials.get(h).unwrap().extension.foliage;
                    (Hole::of(f.leaf), f.leaf.z, f.cover)
                })
                .collect()
        };
        let shut = |app: &App| read(app).iter().all(|r| r.2 == Vec4::ZERO);
        // A still: no `Cut` at all, then one that cuts nothing.
        app.update();
        assert!(shut(&app));
        app.init_resource::<crate::rim::Cut>();
        app.update();
        assert!(shut(&app));
        for at in [Vec3::new(1.0, 0.9, -2.0), Vec3::new(4.0, 1.2, 3.0)] {
            app.world_mut().resource_mut::<crate::rim::Cut>().0 = Some(at);
            app.update();
            let open = at.extend(COVER);
            assert_eq!(
                read(&app),
                [
                    (Hole::Cover, COVER, open),
                    (Hole::Party, HOLE, Vec4::ZERO),
                    (Hole::None, HOLE, Vec4::ZERO),
                    (Hole::Cover, COVER, open),
                ]
            );
        }
        app.world_mut().resource_mut::<crate::rim::Cut>().0 = None;
        app.update();
        assert!(shut(&app));
    }

    /// The check's twin of the shader's hole: a cover card before his chest within `COVER`
    /// on the screen is dropped; one a crown's reach aside (inside `HOLE`, outside `COVER`)
    /// is kept by the cover's hole, not the crowns'; one behind him is kept.
    #[test]
    fn the_cover_hole_is_smaller_than_the_crowns() {
        let middle = Vec3::new(0.0, 0.9, 0.0);
        let axes = [Vec3::X, Vec3::Y, Vec3::NEG_Z];
        let eye = Vec4::new(0.0, 0.9, 20.0, 10.0);
        let before = middle + Vec3::Z;
        let aside = before + Vec3::X * 2.0;
        let behind = middle - Vec3::Z;
        assert!(fade(before, &[middle], COVER, axes, eye) > 0.99);
        assert!(fade(aside, &[middle], COVER, axes, eye) < 0.01);
        assert!(fade(aside, &[middle], HOLE, axes, eye) > 0.5);
        assert!(fade(behind, &[middle], COVER, axes, eye) < 0.01);
    }
}
