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
    /// xyz: the camera; w: play's near fade (m, `Near`), 0 off (the loop's stills and films).
    pub eye: Vec4,
}

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
const HOLE: f32 = 2.8;

/// One the wood opens about as about the party, not one of it (a scene's fighters).
#[derive(Component)]
pub struct Seen;

/// Play's cut (demo/occlude.rs): a cuttable leaf or bark nearer the camera than this (m)
/// dithers out, so no bough nears the lens; and the hole about the hero opens from a hand's
/// breadth before him, not a metre. Absent, the stills' and films' cut is as it was.
#[derive(Resource, Clone, Copy)]
pub struct Near(pub f32);

/// How much of a cuttable surface at `world` the hole drops (0 kept, 1 gone), the twin of
/// foliage.wesl's `cut` without its dither: `party` the hole's middles, `axes` the camera's
/// right, up and forward, `eye` its place and play's near fade (0 off).
pub fn fade(world: Vec3, party: &[Vec3], axes: [Vec3; 3], eye: Vec4) -> f32 {
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
        f = f.max((1.0 - smooth(HOLE * 0.72, HOLE, s)) * smooth(lo, hi, before));
    }
    if play {
        let near = world.distance(eye.xyz());
        f = f.max(1.0 - smooth(eye.w * 0.7, eye.w, near));
    }
    f
}

/// Play's ramp of the hole along the view (m before the hero's middle): wholly open from
/// 0.45, so a trunk at his shoulder is cut, not only one a metre and more before him.
const PLAY_BEFORE: (f32, f32) = (0.05, 0.45);

pub fn plugin(app: &mut App) {
    embedded_asset!(app, "shaders/foliage.wesl");
    app.add_plugins(MaterialPlugin::<FoliageMaterial>::default())
        .init_resource::<Mats>()
        .add_systems(PostUpdate, breathe.after(TransformSystems::Propagate));
}

#[derive(Resource, Default)]
pub struct Mats(Vec<Handle<FoliageMaterial>>);

/// A foliage material: `bend` how far a blade's tip goes in the wind (m), `flutter` how far a
/// leaf card does, `cut` whether it opens about the party.
pub fn material(
    materials: &mut Assets<FoliageMaterial>,
    mats: &mut Mats,
    base: StandardMaterial,
    bend: f32,
    flutter: f32,
    cut: bool,
) -> Handle<FoliageMaterial> {
    let h = materials.add(ExtendedMaterial {
        base,
        extension: FoliageExt {
            foliage: Foliage {
                wind: Vec4::new(WIND.x, WIND.y, bend, 0.0),
                leaf: Vec4::new(flutter, if cut { 1.0 } else { 0.0 }, HOLE, 0.0),
                ..default()
            },
        },
    });
    mats.0.push(h.clone());
    h
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
