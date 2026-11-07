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
}

/// The canopy map's width in metres, about the world's middle.
const SPAN: f32 = 128.0;
const TEXELS: u32 = 256;

/// The canopy map every shaded material reads.
#[derive(Resource)]
struct Canopy(Handle<Image>);

/// A model shaded by the canopy without a rim (a fern, a rock, a log in the wood).
#[derive(Component)]
pub struct Shade;

/// A fighter (not an `Actor`) that takes the rim too: every body the ruin's fight poses,
/// in every scene (ruin::fight's `cast`). Its materials are swapped some frames after its
/// model is in, once the ruin's hit (effects/hit.rs) has found the `StandardMaterial`s it
/// flashes; the flash is then copied onto the swapped ones (`mirror`), so the fighter
/// both rims and flashes. Its rim is the fighters' own (`fighter_rim`), stronger than the
/// party's, and `value` darkens its albedo where its coat is the floor's tone; `rim`
/// scales its band (a leafy crown's every leaf edge takes it, so it frosts).
#[derive(Component)]
pub struct Fighter {
    frames: u32,
    value: f32,
    rim: f32,
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
        }
    }
}

/// How many frames a fighter's model is in before its materials are swapped.
const FIGHTER_WAIT: u32 = 8;

impl MaterialExtension for RimExt {
    fn fragment_shader() -> ShaderRef {
        "embedded://iso/shaders/rim.wesl".into()
    }
}

pub fn plugin(app: &mut App) {
    embedded_asset!(app, "shaders/rim.wesl");
    app.add_plugins(MaterialPlugin::<RimMaterial>::default())
        .init_resource::<Swapped>()
        .init_resource::<Mirrored>()
        .add_systems(Startup, canopy_map)
        .add_systems(Update, (swap, fighters, tint, mirror).chain());
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
    Actor,
    Hide,
    Fighter,
}

#[derive(Resource, Default)]
struct Swapped(HashMap<(AssetId<StandardMaterial>, [u32; 3], Kind), Handle<RimMaterial>>);

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
        Hour::Day => LinearRgba::new(1.7, 1.9, 2.3, 0.0),
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
        Kind::Fighter => Vec4::new(0.8, 0.12, 0.4, 1.0),
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
    standard: Res<Assets<StandardMaterial>>,
    mut rims: ResMut<Assets<RimMaterial>>,
    mut swapped: ResMut<Swapped>,
    hour: Option<Res<Hour>>,
    canopy: Res<Canopy>,
) {
    let rim = rim_for(hour.map_or(Hour::Dusk, |h| *h));
    for (e, mat) in &added {
        let (tint, kind) = match parents.iter_ancestors(e).find_map(|a| actors.get(a).ok()) {
            Some((tint, hide)) => (
                tint.map_or(Vec3::ONE, |t| t.0),
                if hide { Kind::Hide } else { Kind::Actor },
            ),
            None if parents.iter_ancestors(e).any(|a| shaded.contains(a)) => {
                (Vec3::ONE, Kind::Shade)
            }
            None => continue,
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
            &canopy,
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
    canopy: &Canopy,
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
    if matches!(kind, Kind::Fighter | Kind::Hide) {
        base.reflectance = FIGHTER_REFLECTANCE;
    }
    let h = rims.add(ExtendedMaterial {
        base,
        extension: RimExt {
            rim,
            canopy: canopy.0.clone(),
            lift: lift(kind),
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
    canopy: Res<Canopy>,
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
                &canopy,
                mat,
                Vec3::splat(f.value),
                (Kind::Fighter, rim),
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
            Kind::Shade => continue,
            Kind::Actor | Kind::Hide => rim_for(*hour),
            Kind::Fighter => fighter_rim(Some(*hour)),
        };
        if let Some(mut m) = rims.get_mut(h) {
            m.extension.rim = rim;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
