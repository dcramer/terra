//! A rim on the people and beasts: their glTF materials are swapped, as their scenes come
//! in, for the same material with a fresnel term added to the emission, so a figure's edge
//! catches a thin light of the hour's sky and it reads against the grass (as Pillars of
//! Eternity and Pathfinder light their figures). A light could not do it alone: Bevy
//! culls a light by the view, not the mesh, so a back light also whitens every grass blade.
//! The same material shades what stands under the oaks: their crowns hide the sky, so the
//! sky's fill on a beast, a fern or a rock there is dimmed as the ground's is (the canopy
//! map, scene::canopy drawn over the land), not lit as if in the open.

use crate::party::Actor;
use crate::scene::Hour;
use bevy::asset::embedded_asset;
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;

pub type RimMaterial = ExtendedMaterial<StandardMaterial, RimExt>;

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct RimExt {
    /// The rim's colour as it reaches the screen (the glTF materials' emission is not
    /// scaled by the exposure); alpha is the fresnel's power.
    #[uniform(100)]
    pub rim: LinearRgba,
    /// How much sky reaches each point of the land, 1 in the open (scene::canopy), over
    /// `SPAN` metres about the middle.
    #[texture(101)]
    #[sampler(102)]
    pub canopy: Handle<Image>,
    /// x: how much of the rim the canopy leaves (0 dims it as the sky's fill, 1 keeps it
    /// whole: a fighter's rim is the ARPGs' lift, not the sky's light, iso-art j28).
    /// y, z: a fighter's hard band, whole where the surface faces the view under y (the
    /// cosine), gone over z; z zero is the fresnel of power `rim.a` (the party's). w: the
    /// band's share on this body (`Fighter::rim`).
    #[uniform(103)]
    pub lift: Vec4,
    /// The ground's scans (kit/ground.rs `STACK`, one array layer each), for a stone's moss.
    #[texture(104, dimension = "2d_array")]
    #[sampler(105)]
    pub scans: Handle<Image>,
    /// A stone's (`Stone`): x 1 on a stone, y the floor it sits in (m), z the moss scan's
    /// layer, w the metres it covers; all zero on anything else.
    #[uniform(106)]
    pub stone: Vec4,
    /// A stone's moss: rgb the scan's tint (as the ground's moss layer's), a its cover.
    #[uniform(107)]
    pub moss: Vec4,
    /// x: the share of a metal texel's base colour taken to its own grey, by its metal
    /// (`Fighter::steel`); y: the share of the sky's reflection it is denied, its specular
    /// occlusion (`Fighter::with_sky`); z: its fill's strength (`Fighter::with_fill`); w
    /// unused.
    #[uniform(108)]
    pub metal: Vec4,
    /// Ground cover's cut before the hero (`Cut`, a `Shade`'s or `Stone`'s only): xyz his
    /// middle, w the hole's radius (m), 0 cuts nothing.
    #[uniform(109)]
    pub near: Vec4,
    /// x: the frame (0..64), turning the cut's dither as foliage.wesl's `leaf.w`.
    #[uniform(110)]
    pub hole: Vec4,
}

/// Where ground cover (a `Shade`'s or `Stone`'s materials: ferns, rocks, logs) opens before
/// the hero, his middle, set each frame in play (demo/occlude.rs); none in a still. The
/// judge on j25's film: "the hero vanishes into ferns" (c3, c5): Diablo IV and Path of Exile
/// 2 dither out whatever stands between the camera and the player, low props too. Cut in
/// the camera's prepass as in the main pass (rim_prepass.wesl), or the fern's depth there
/// still hides him behind a hole.
#[derive(Resource, Default)]
pub struct Cut(pub Option<Vec3>);

/// The cover's hole's radius about his middle on the screen (m): his 1.8 m body is 0.9 m
/// above and below it, and the hole thins from `0.72` of it.
pub const CUT: f32 = 1.3;

/// The canopy map's width in metres, about the world's middle.
const SPAN: f32 = 128.0;
const TEXELS: u32 = 256;

/// The canopy map every shaded material reads.
#[derive(Resource)]
struct Canopy(Handle<Image>);

/// A model shaded by the canopy without a rim (a fern, a rock, a log in the wood).
#[derive(Component)]
pub struct Shade;

/// A stone set in the land whose floor is `floor` (m) at its foot: shaded as a `Shade`, with
/// moss on what faces up and soil grime where it meets the ground (rim.wesl). The judge on
/// round 261107-0507: "boulders smooth beige uniformly sunlit lumps with one roughness, no
/// lichen/moss, no settling into the ground"; Titan Quest II's and the drone refs' stones
/// are mossed on their tops and grimed at their feet. The moss is the ground's own scan
/// (moss_004) laid from above in world space, as Unreal's and Megascans' world-aligned
/// blend of a moss layer over a rock asset by its normal's height.
#[derive(Component)]
pub struct Stone {
    pub floor: f32,
}

/// The ground's scans the stones' moss is drawn from (kit/ground.rs loads the same array).
#[derive(Resource)]
struct Scans(Handle<Image>);

/// A stone's moss: the ground's moss layer's tint (biomes/forest.ron, `moss_004` 5.0, 6.0,
/// 3.2) a little dulled, as moss on a stone dries paler and browner than the bed's; its
/// cover's share of a stone's upward faces. At (3.6, 4.4, 2.3) the sunlit tops read mint
/// green (round 261107-0549): darker and warmer, an olive moss.
const MOSS: Vec4 = Vec4::new(2.2, 2.5, 0.95, 0.75);

/// A fighter (not an `Actor`) that takes the rim too: every body the ruin's fight poses,
/// in every scene (ruin::fight's `cast`). Its materials are swapped some frames after its
/// model is in, once the ruin's hit (effects/hit.rs) has found the `StandardMaterial`s it
/// flashes; the flash is then copied onto the swapped ones (`mirror`), so the fighter
/// both rims and flashes. Its rim is the fighters' own (`fighter_rim`), stronger than the
/// party's, and `value` darkens its albedo where its coat is the floor's tone; `rim`
/// scales its band (a leafy crown's every leaf edge takes it, so it frosts). `steel` greys
/// its metal (`with_steel`), `sky` is the share of the sky's reflection it keeps
/// (`with_sky`), `fill` the strength of its own fill light (`with_fill`).
#[derive(Component)]
pub struct Fighter {
    frames: u32,
    value: f32,
    rim: f32,
    steel: f32,
    sky: f32,
    fill: f32,
}

impl Default for Fighter {
    fn default() -> Self {
        Self::valued(1.0, 1.0)
    }
}

impl Fighter {
    /// A fighter whose albedo is scaled by `value` and its rim by `rim`.
    pub fn valued(value: f32, rim: f32) -> Self {
        Self {
            frames: 0,
            value,
            rim,
            steel: 0.0,
            sky: 1.0,
            fill: 0.0,
        }
    }

    /// Its fill light, 0 for none: its albedo lit from the camera's side at `fill` (rim.wesl),
    /// whatever shade or canopy it stands in. The ARPGs light their figures so (Diablo IV's
    /// and Path of Exile's character lights: a light that follows the body and lights it
    /// alone): the forest's foes took most of their light from the rim band, so at a rim of
    /// 0.35, which cuts its white streaks, they stood near black, 8 to 21 L* under the
    /// ground about them, and their albedo barely moved them (iso-forest-demo j12, j14).
    pub fn with_fill(mut self, fill: f32) -> Self {
        self.fill = fill.max(0.0);
        self
    }

    /// The share, 0 to 1, of its metal's base colour taken to its own grey, by each texel's
    /// metal (rim.wesl), the dielectric texels of the same texture left as they are. A
    /// metal's base colour is the colour of everything it reflects (glTF's metal-roughness
    /// model), and iron's is near grey (F0 about 0.56, 0.57, 0.58 linear, Real-Time
    /// Rendering's table of metals); a pack's plate painted green-grey
    /// turns every reflection of the sky mint. Graded in the shader, not the texels, since
    /// a pack's textures are external and shared by every piece (kit/finish.rs grades a
    /// glTF's embedded images only).
    pub fn with_steel(mut self, share: f32) -> Self {
        self.steel = share.clamp(0.0, 1.0);
        self
    }

    /// How bright, 0 to 1 of the sky, the wood about it is in its metal's reflection: a
    /// horizon occlusion of what it mirrors (rim.wesl). The sky's fill (Bevy's ambient
    /// light; a biome's environment map where it has one) is one colour from every
    /// direction, so a plate's sides and underside mirrored the sky as brightly as its top
    /// and read an even, plastic white (iso-hero j9's judges); real plate mirrors the sky
    /// above it and the dark trees and ground beside and below it.
    pub fn with_sky(mut self, share: f32) -> Self {
        self.sky = share.clamp(0.0, 1.0);
        self
    }

    /// Its swapped materials' kind: one material a steel and sky share and fill.
    fn kind(&self) -> Kind {
        Kind::Fighter([self.steel, self.sky, self.fill].map(f32::to_bits))
    }
}

/// How many frames a fighter's model is in before its materials are swapped.
const FIGHTER_WAIT: u32 = 8;

impl MaterialExtension for RimExt {
    fn fragment_shader() -> ShaderRef {
        "embedded://iso/shaders/rim.wesl".into()
    }

    fn prepass_fragment_shader() -> ShaderRef {
        "embedded://iso/shaders/rim_prepass.wesl".into()
    }
}

pub fn plugin(app: &mut App) {
    embedded_asset!(app, "shaders/rim.wesl");
    embedded_asset!(app, "shaders/rim_prepass.wesl");
    // A biome's finishes (kit/finish.rs) set a model's material as it loads, before it is
    // spawned and swapped here, so the swapped copy carries them.
    app.add_plugins((
        MaterialPlugin::<RimMaterial>::default(),
        crate::kit::finish::plugin,
    ))
    .init_resource::<Swapped>()
    .init_resource::<Mirrored>()
    .init_resource::<Cut>()
    .add_systems(Startup, (canopy_map, scans))
    .add_systems(Update, (swap, fighters, tint, mirror).chain())
    .add_systems(PostUpdate, cut);
}

/// The cover's hole (`Cut`) and the frame, to every `Shade`'s and `Stone`'s material.
pub fn cut(
    at: Res<Cut>,
    frames: Res<bevy::diagnostic::FrameCount>,
    swapped: Res<Swapped>,
    mut rims: ResMut<Assets<RimMaterial>>,
) {
    let near = at.0.map_or(Vec4::ZERO, |m| m.extend(CUT));
    for ((.., kind), h) in &swapped.0 {
        if !matches!(kind, Kind::Shade | Kind::Stone(_)) {
            continue;
        }
        if near == Vec4::ZERO && rims.get(h).is_some_and(|m| m.extension.near == near) {
            continue;
        }
        if let Some(mut m) = rims.get_mut(h) {
            m.extension.near = near;
            m.extension.hole.x = (frames.0 % 64) as f32;
        }
    }
}

/// The ground's scan array, loaded as kit/ground.rs loads it.
fn scans(mut commands: Commands, asset: Res<AssetServer>) {
    use bevy::image::{ImageArrayLayout, ImageLoaderSettings};
    let rows = crate::kit::ground::STACK.len() as u32;
    let h = asset
        .load_builder()
        .with_settings(move |s: &mut ImageLoaderSettings| {
            s.array_layout = Some(ImageArrayLayout::RowCount { rows });
            s.is_srgb = true;
        })
        .load("materials/ground/color.jpg");
    commands.insert_resource(Scans(h));
}

/// The stone uniform for a floor: the moss scan's layer and metres from the stack.
fn stone(floor: f32) -> Vec4 {
    let stack = crate::kit::ground::STACK;
    let (i, m) = stack
        .iter()
        .enumerate()
        .find(|(_, (n, _))| *n == "moss_004")
        .map_or((0, 1.1), |(i, (_, m))| (i, *m));
    Vec4::new(1.0, floor, i as f32, m)
}

/// The canopy map: the glade's and the forest's oaks over their land; all open in the
/// ruin (no land) and the marsh (its own land, not the forest's trees).
fn canopy_map(
    mut commands: Commands,
    land: Option<Res<crate::terrain::Land>>,
    args: Res<crate::Args>,
    mut images: ResMut<Assets<Image>>,
) {
    use crate::kit::biome::Layout;
    use bevy::asset::RenderAssetUsages;
    use bevy::image::ImageSampler;
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
    let open = matches!(args.layout, Some(Layout::Ruin | Layout::Marsh));
    let wooded = land.filter(|_| !open);
    let trees = wooded.as_ref().map(|l| crate::forest::tree_spots(l));
    let mut data = Vec::with_capacity((TEXELS * TEXELS) as usize);
    for y in 0..TEXELS {
        for x in 0..TEXELS {
            let p = (Vec2::new(x as f32, y as f32) + 0.5) / TEXELS as f32 - 0.5;
            let c = trees
                .as_ref()
                .map_or(1.0, |t| crate::scene::canopy(p * SPAN, t));
            data.push((c.clamp(0.0, 1.0) * 255.0) as u8);
        }
    }
    let mut image = Image::new(
        Extent3d {
            width: TEXELS,
            height: TEXELS,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::R8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::linear();
    commands.insert_resource(Canopy(images.add(image)));
}

/// A colour the model's own is multiplied by (a pale Tripo coat brought to the beast's).
#[derive(Component, Clone, Copy)]
pub struct Tint(pub Vec3);

/// A dark hide (the forest's moose): an actor whose sky sheen is the fighters' 1%, not
/// glTF's 4%. Under the oaks a 4% sheen and the haze turned its coat the shaded ground's
/// blue-grey whatever its `Tint` (iso-art j47: on the mask's core, not its bloomed halo
/// that j38 measured, untinted dL* -4.3 and blue-grey; tinted at this sheen -5.2, brown).
#[derive(Component)]
pub struct Hide;

/// What a swapped material is: shaded only, an actor's rim, a hide's (an actor's rim at a
/// fighter's sheen), a fighter's rim.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Kind {
    Shade,
    /// A stone, its floor's bits (`Stone`).
    Stone(u32),
    Actor,
    Hide,
    /// A fighter, its steel and sky shares' and its fill's bits (`Fighter::kind`).
    Fighter([u32; 3]),
}

#[derive(Resource, Default)]
pub struct Swapped(HashMap<(AssetId<StandardMaterial>, [u32; 3], Kind), Handle<RimMaterial>>);

/// The rim for the hour: faint by day, the sky's blue at dusk and by night.
fn rim_for(hour: Hour) -> LinearRgba {
    match hour {
        Hour::Day => LinearRgba::new(0.10, 0.10, 0.09, 3.6),
        Hour::Dusk => LinearRgba::new(0.08, 0.09, 0.13, 3.4),
        Hour::Night => LinearRgba::new(0.07, 0.10, 0.22, 3.2),
    }
}

/// The fighters' rim: the ARPGs' cold lift on every body against the warm local lights
/// (Diablo IV's and Last Epoch's night frames), blue-white, about 5 times the party's, so
/// it reads as an edge at 70 px a figure. Drawn as a hard band on the outline (`lift`),
/// not a fresnel: a power-3.4 fresnel's tail washed the ogre lavender and frosted the tree
/// monster's crown (iso-art j30's judges); the band, a few pixels wide, takes four times the
/// light to read. With no hour (the ruin, the marsh) it is the night's.
fn fighter_rim(hour: Option<Hour>) -> LinearRgba {
    match hour.unwrap_or(Hour::Night) {
        // By day a little stronger and whiter than j30's, so the forest's dark bark shows
        // an edge on the shaded floor (judge 261107-0507: "near-black silhouettes").
        Hour::Day => LinearRgba::new(2.2, 2.35, 2.6, 0.0),
        Hour::Dusk => LinearRgba::new(1.8, 2.2, 3.8, 0.0),
        Hour::Night => LinearRgba::new(2.0, 2.7, 5.0, 0.0),
    }
}

/// The rim the canopy leaves on each kind (`RimExt::lift`).
fn lift(kind: Kind) -> Vec4 {
    match kind {
        // The band from 66 to 83 degrees off the view: on a round body seen from above,
        // the outer tenth of its radius, 2 to 3 px at 70 px a figure (iso-art j30; at 73
        // degrees Tripo's coarse normals barely reached it and it did not read).
        // w: the body's share of it (`Fighter::rim`).
        Kind::Fighter(..) => Vec4::new(0.8, 0.12, 0.4, 1.0),
        _ => Vec4::ZERO,
    }
}

#[allow(clippy::too_many_arguments)]
fn swap(
    mut commands: Commands,
    added: Query<
        (Entity, &MeshMaterial3d<StandardMaterial>),
        Added<MeshMaterial3d<StandardMaterial>>,
    >,
    parents: Query<&ChildOf>,
    actors: Query<(Option<&Tint>, Has<Hide>), With<Actor>>,
    shaded: Query<(), With<Shade>>,
    stones: Query<&Stone>,
    standard: Res<Assets<StandardMaterial>>,
    mut rims: ResMut<Assets<RimMaterial>>,
    mut swapped: ResMut<Swapped>,
    hour: Option<Res<Hour>>,
    tex: (Res<Canopy>, Res<Scans>),
) {
    let rim = rim_for(hour.map_or(Hour::Dusk, |h| *h));
    for (e, mat) in &added {
        let (tint, kind) = match parents.iter_ancestors(e).find_map(|a| actors.get(a).ok()) {
            Some((tint, hide)) => (
                tint.map_or(Vec3::ONE, |t| t.0),
                if hide { Kind::Hide } else { Kind::Actor },
            ),
            None => {
                if let Some(s) = parents.iter_ancestors(e).find_map(|a| stones.get(a).ok()) {
                    (Vec3::ONE, Kind::Stone(s.floor.to_bits()))
                } else if parents.iter_ancestors(e).any(|a| shaded.contains(a)) {
                    (Vec3::ONE, Kind::Shade)
                } else {
                    continue;
                }
            }
        };
        let rim = if matches!(kind, Kind::Actor | Kind::Hide) {
            rim
        } else {
            LinearRgba::NONE
        };
        let Some(handle) = rimmed_as(
            &mut swapped,
            &standard,
            &mut rims,
            (&tex.0, &tex.1),
            mat,
            tint,
            (kind, rim),
        ) else {
            continue;
        };
        commands
            .entity(e)
            .remove::<MeshMaterial3d<StandardMaterial>>()
            .insert(MeshMaterial3d(handle));
    }
}

/// A fighter's specular (Bevy's 0.5 is glTF's dielectric 4%): on Tripo's dark bark and
/// hide a colourless sky sheen at 4% lifted a body 4 to 5 L* toward grey whatever its
/// albedo (forest by day, iso-art j34); at 0.15 (about 1%) the albedo's value step reads.
const FIGHTER_REFLECTANCE: f32 = 0.15;

/// The swapped material for `mat` tinted by `tint` with `rim`, made the first time.
fn rimmed_as(
    swapped: &mut Swapped,
    standard: &Assets<StandardMaterial>,
    rims: &mut Assets<RimMaterial>,
    (canopy, scans): (&Canopy, &Scans),
    mat: &MeshMaterial3d<StandardMaterial>,
    tint: Vec3,
    (kind, rim): (Kind, LinearRgba),
) -> Option<Handle<RimMaterial>> {
    let key = (mat.id(), tint.to_array().map(f32::to_bits), kind);
    if let Some(h) = swapped.0.get(&key) {
        return Some(h.clone());
    }
    let mut base = standard.get(&mat.0)?.clone();
    let c = base.base_color.to_linear();
    base.base_color =
        LinearRgba::new(c.red * tint.x, c.green * tint.y, c.blue * tint.z, c.alpha).into();
    if matches!(kind, Kind::Fighter(..) | Kind::Hide) {
        base.reflectance = FIGHTER_REFLECTANCE;
    }
    let (stone, moss) = match kind {
        Kind::Stone(floor) => (stone(f32::from_bits(floor)), MOSS),
        _ => (Vec4::ZERO, Vec4::ZERO),
    };
    let metal = match kind {
        Kind::Fighter(bits) => {
            let [steel, sky, fill] = bits.map(f32::from_bits);
            Vec4::new(steel, 1.0 - sky, fill, 0.0)
        }
        _ => Vec4::ZERO,
    };
    let h = rims.add(ExtendedMaterial {
        base,
        extension: RimExt {
            rim,
            canopy: canopy.0.clone(),
            lift: lift(kind),
            scans: scans.0.clone(),
            stone,
            moss,
            metal,
            near: Vec4::ZERO,
            hole: Vec4::ZERO,
        },
    });
    swapped.0.insert(key, h.clone());
    Some(h)
}

/// The fighters' materials swapped once their models have been in for `FIGHTER_WAIT`
/// frames, each remembered beside its original for `mirror`.
#[allow(clippy::too_many_arguments)]
fn fighters(
    mut commands: Commands,
    mut fighters: Query<(Entity, &mut Fighter)>,
    children: Query<&Children>,
    skins: Query<&MeshMaterial3d<StandardMaterial>>,
    standard: Res<Assets<StandardMaterial>>,
    mut rims: ResMut<Assets<RimMaterial>>,
    mut swapped: ResMut<Swapped>,
    mut mirrored: ResMut<Mirrored>,
    hour: Option<Res<Hour>>,
    tex: (Res<Canopy>, Res<Scans>),
) {
    let rim = fighter_rim(hour.map(|h| *h));
    for (body, mut f) in &mut fighters {
        if f.frames > FIGHTER_WAIT {
            continue;
        }
        if !children.iter_descendants(body).any(|e| skins.contains(e)) {
            continue;
        }
        f.frames += 1;
        if f.frames <= FIGHTER_WAIT {
            continue;
        }
        for e in children.iter_descendants(body) {
            let Ok(mat) = skins.get(e) else { continue };
            let Some(h) = rimmed_as(
                &mut swapped,
                &standard,
                &mut rims,
                (&tex.0, &tex.1),
                mat,
                Vec3::splat(f.value),
                (f.kind(), rim),
            ) else {
                continue;
            };
            // The body's own share of the band (the swap key holds its value, one body's).
            if let Some(mut m) = rims.get_mut(&h)
                && m.extension.lift.w != f.rim
            {
                m.extension.lift.w = f.rim;
            }
            if !mirrored.0.iter().any(|(_, r, _)| *r == h) {
                mirrored.0.push((mat.0.clone(), h.clone(), f.value));
            }
            commands
                .entity(e)
                .remove::<MeshMaterial3d<StandardMaterial>>()
                .insert(MeshMaterial3d(h));
        }
    }
}

/// The fighters' originals beside their swapped materials and the body's value.
#[derive(Resource, Default)]
struct Mirrored(Vec<(Handle<StandardMaterial>, Handle<RimMaterial>, f32)>);

/// Copies the hit flash (effects/hit.rs sets the original's emission) onto the swapped
/// material, touching it only when it changed. Scaled by the body's value: the flash
/// lights the base colour's texture, which the value's tint of the base colour does not
/// reach, so a darkened body flashed at its pale raw coat (the ogre milky, the forest's
/// treant at the rocks' value: a still stands mid-flash, iso-art j30).
fn mirror(
    mirrored: Res<Mirrored>,
    standard: Res<Assets<StandardMaterial>>,
    mut rims: ResMut<Assets<RimMaterial>>,
) {
    for (from, to, value) in &mirrored.0 {
        let (Some(a), Some(b)) = (standard.get(from), rims.get(to)) else {
            continue;
        };
        let c = a.emissive;
        let e = LinearRgba::new(c.red * value, c.green * value, c.blue * value, c.alpha);
        if e == b.base.emissive && a.emissive_texture == b.base.emissive_texture {
            continue;
        }
        let tex = a.emissive_texture.clone();
        if let Some(mut m) = rims.get_mut(to) {
            m.base.emissive = e;
            m.base.emissive_texture = tex;
        }
    }
}

fn tint(hour: Option<Res<Hour>>, swapped: Res<Swapped>, mut rims: ResMut<Assets<RimMaterial>>) {
    let Some(hour) = hour else { return };
    if !hour.is_changed() {
        return;
    }
    for ((.., kind), h) in &swapped.0 {
        let rim = match kind {
            Kind::Shade | Kind::Stone(_) => continue,
            Kind::Actor | Kind::Hide => rim_for(*hour),
            Kind::Fighter(..) => fighter_rim(Some(*hour)),
        };
        if let Some(mut m) = rims.get_mut(h) {
            m.extension.rim = rim;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fighter's steel share greys its own swapped materials only: another fighter
    /// wearing the same material (the staged cast in the stills) keeps its metal's colour.
    #[test]
    fn steel_greys_only_the_fighter_that_asks() {
        let mut standard = Assets::<StandardMaterial>::default();
        let mut rims = Assets::<RimMaterial>::default();
        let mut swapped = Swapped::default();
        let tex = (Canopy(Handle::default()), Scans(Handle::default()));
        let plate = MeshMaterial3d(standard.add(StandardMaterial::default()));
        let mut made = |f: &Fighter| {
            let kind = (f.kind(), LinearRgba::NONE);
            let to = (&tex.0, &tex.1);
            rimmed_as(
                &mut swapped,
                &standard,
                &mut rims,
                to,
                &plate,
                Vec3::ONE,
                kind,
            )
            .unwrap()
        };
        let cast = made(&Fighter::default());
        let hero = made(&Fighter::default().with_steel(1.0));
        let shaded = made(&Fighter::default().with_steel(1.0).with_sky(0.3));
        assert_ne!(cast, hero, "the hero's steel shares the cast's material");
        assert_ne!(
            hero, shaded,
            "a fighter's sky share shares another's material"
        );
        assert_eq!(made(&Fighter::default()), cast);
        let filled = made(&Fighter::default().with_fill(0.8));
        let metal = |h: &Handle<RimMaterial>| rims.get(h).unwrap().extension.metal;
        assert_eq!((metal(&cast).x, metal(&hero).x), (0.0, 1.0));
        // The sky it is denied: none by default, so the cast and every other body keep the
        // whole of it; 0.7 where it keeps 0.3.
        assert_eq!((metal(&cast).y, metal(&hero).y), (0.0, 0.0));
        assert!(
            (metal(&shaded).y - 0.7).abs() < 1e-6,
            "{:?}",
            metal(&shaded)
        );
        // A fill is its own material too, its strength in the shader's metal.z; none on
        // the cast, so every body that does not ask is lit as before.
        assert_ne!(filled, cast, "a foe's fill shares the cast's material");
        assert_eq!((metal(&cast).z, metal(&filled).z), (0.0, 0.8));
    }

    /// A darkened fighter's hit flash is darkened with it, so it never shows the pale raw
    /// coat the value hides; the swapped material follows the flash as it changes.
    #[test]
    fn the_flash_on_a_darkened_body_is_darkened_with_it() {
        let mut app = App::new();
        let mut standard = Assets::<StandardMaterial>::default();
        let mut rims = Assets::<RimMaterial>::default();
        let from = standard.add(StandardMaterial::default());
        let to = rims.add(ExtendedMaterial {
            base: StandardMaterial::default(),
            extension: RimExt::default(),
        });
        app.insert_resource(standard)
            .insert_resource(rims)
            .insert_resource(Mirrored(vec![(from.clone(), to.clone(), 0.5)]))
            .add_systems(Update, mirror);
        for flash in [0.6, 0.2] {
            let mut standard = app.world_mut().resource_mut::<Assets<StandardMaterial>>();
            standard.get_mut(&from).unwrap().emissive = LinearRgba::rgb(flash, flash, flash);
            app.update();
            let rims = app.world().resource::<Assets<RimMaterial>>();
            let e = rims.get(&to).unwrap().base.emissive;
            assert!(
                (e.red - flash * 0.5).abs() < 1e-6,
                "flash {flash}: swapped {e:?}"
            );
        }
    }

    /// The cover's hole reaches the ferns', rocks' and logs' materials alone, follows the
    /// hero, and closes when there is none (a still).
    #[test]
    fn the_cut_opens_cover_alone_about_the_hero() {
        let mut app = App::new();
        app.init_resource::<Assets<RimMaterial>>()
            .init_resource::<Swapped>()
            .init_resource::<Cut>()
            .init_resource::<bevy::diagnostic::FrameCount>()
            .add_systems(Update, cut);
        let mut standard = Assets::<StandardMaterial>::default();
        let plate = MeshMaterial3d(standard.add(StandardMaterial::default()));
        let tex = (Canopy(Handle::default()), Scans(Handle::default()));
        let kinds = [
            Kind::Shade,
            Kind::Stone(0.5f32.to_bits()),
            Fighter::default().kind(),
            Kind::Actor,
        ];
        let made: Vec<_> = kinds
            .iter()
            .map(|k| {
                let world = app.world_mut();
                let mut swapped = world.remove_resource::<Swapped>().unwrap();
                let mut rims = world.remove_resource::<Assets<RimMaterial>>().unwrap();
                let to = (&tex.0, &tex.1);
                let h = rimmed_as(
                    &mut swapped,
                    &standard,
                    &mut rims,
                    to,
                    &plate,
                    Vec3::ONE,
                    (*k, LinearRgba::NONE),
                )
                .unwrap();
                world.insert_resource(swapped);
                world.insert_resource(rims);
                h
            })
            .collect();
        let near = |app: &App| -> Vec<Vec4> {
            let rims = app.world().resource::<Assets<RimMaterial>>();
            made.iter()
                .map(|h| rims.get(h).unwrap().extension.near)
                .collect()
        };
        for at in [Vec3::new(1.0, 0.9, -2.0), Vec3::new(4.0, 1.2, 3.0)] {
            app.world_mut().resource_mut::<Cut>().0 = Some(at);
            app.update();
            let open = at.extend(CUT);
            assert_eq!(near(&app), [open, open, Vec4::ZERO, Vec4::ZERO]);
        }
        app.world_mut().resource_mut::<Cut>().0 = None;
        app.update();
        assert_eq!(near(&app), [Vec4::ZERO; 4], "a still cuts nothing");
    }
}
