//! What a blow that lands shows on what it struck (D456): the body flashes, its skin and what
//! it wears and holds lit for the set's `landed.flash_s` (3 to 4 frames at 60, as Enshrouded's
//! struck flash), as bright as `landed.flash` (0 none: D487 proposes the contact's `burst`
//! instead), in place of a long freeze. Presentation only: nothing the fight's rules read.
//! Materials are shared between bodies, so each mesh of the struck body shows a lit copy of
//! its own (one a material, shared) and gets its own back after, as a hit flash is drawn by
//! swapping in an overlay or a material instance in other engines (Unreal's dynamic material
//! instances).

use bevy::prelude::*;
use std::collections::{HashMap, HashSet};
use terra_walker::character::BodyMaterial;

/// What a flash adds to what a material gives off, in nits: not weighted by the camera's
/// exposure, so it reads alike by day and by night. A warm white.
const GLOW: LinearRgba = LinearRgba::rgb(0.9, 0.85, 0.75);

/// A body struck: it flashes for `seconds` (the set's `landed.flash_s`), as bright as
/// `strength` (the set's `landed.flash`, 1 the full glow).
#[derive(Message, Clone, Copy, Debug)]
pub struct Lit {
    pub body: Entity,
    pub seconds: f32,
    pub strength: f32,
}

/// A body flashing: how much longer, and each of its meshes with the material it had and the
/// lit one it shows.
#[derive(Component)]
pub struct Flashing {
    left: f32,
    parts: Vec<(Entity, Was)>,
}

/// A mesh's own material, and the lit copy it shows while flashing.
enum Was {
    Standard(Handle<StandardMaterial>, AssetId<StandardMaterial>),
    Body(Handle<BodyMaterial>, AssetId<BodyMaterial>),
}

/// The lit copies, one a material and strength, however many bodies flash with it.
#[derive(Resource, Default)]
struct Copies {
    standard: HashMap<(AssetId<StandardMaterial>, u32), Handle<StandardMaterial>>,
    body: HashMap<(AssetId<BodyMaterial>, u32), Handle<BodyMaterial>>,
}

pub fn plugin(app: &mut App) {
    app.add_message::<Lit>()
        .init_resource::<Copies>()
        .add_systems(Update, (light, fade).chain());
}

/// A material lit: what it gives off is the flash's, as strong as asked, whatever map it had.
fn lit(own: &StandardMaterial, strength: f32) -> StandardMaterial {
    StandardMaterial {
        emissive: GLOW * strength,
        emissive_texture: None,
        emissive_exposure_weight: 0.0,
        ..own.clone()
    }
}

/// Each body struck shows its meshes lit; one flashing already flashes on from now.
fn light(
    mut struck: MessageReader<Lit>,
    mut commands: Commands,
    children: Query<&Children>,
    standard: Query<&MeshMaterial3d<StandardMaterial>>,
    skins: Query<&MeshMaterial3d<BodyMaterial>>,
    mut flashing: Query<&mut Flashing>,
    mut standards: ResMut<Assets<StandardMaterial>>,
    mut bodies: ResMut<Assets<BodyMaterial>>,
    mut copies: ResMut<Copies>,
) {
    let mut done = HashSet::new();
    for &Lit {
        body,
        seconds,
        strength,
    } in struck.read()
    {
        let strong = strength.to_bits();
        if let Ok(mut flash) = flashing.get_mut(body) {
            flash.left = flash.left.max(seconds);
            continue;
        }
        if !done.insert(body) {
            continue;
        }
        let mut parts = Vec::new();
        for part in std::iter::once(body).chain(children.iter_descendants(body)) {
            if let Ok(own) = standard.get(part) {
                let copy = match copies.standard.get(&(own.id(), strong)) {
                    Some(copy) => copy.clone(),
                    None => {
                        let Some(base) = standards.get(&own.0).map(|own| lit(own, strength)) else {
                            continue;
                        };
                        let copy = standards.add(base);
                        copies.standard.insert((own.id(), strong), copy.clone());
                        copy
                    }
                };
                parts.push((part, Was::Standard(own.0.clone(), copy.id())));
                commands.entity(part).insert(MeshMaterial3d(copy));
            } else if let Ok(own) = skins.get(part) {
                let copy = match copies.body.get(&(own.id(), strong)) {
                    Some(copy) => copy.clone(),
                    None => {
                        let Some((base, extension)) = bodies
                            .get(&own.0)
                            .map(|own| (lit(&own.base, strength), own.extension))
                        else {
                            continue;
                        };
                        let copy = bodies.add(BodyMaterial { base, extension });
                        copies.body.insert((own.id(), strong), copy.clone());
                        copy
                    }
                };
                parts.push((part, Was::Body(own.0.clone(), copy.id())));
                commands.entity(part).insert(MeshMaterial3d(copy));
            }
        }
        commands.entity(body).insert(Flashing {
            left: seconds,
            parts,
        });
    }
}

/// A flash runs out, and each mesh gets its own material back, unless something else gave it
/// another meanwhile (a look put on or taken off).
fn fade(
    time: Res<Time>,
    mut commands: Commands,
    mut flashing: Query<(Entity, &mut Flashing)>,
    standard: Query<&MeshMaterial3d<StandardMaterial>>,
    skins: Query<&MeshMaterial3d<BodyMaterial>>,
) {
    for (body, mut flash) in &mut flashing {
        flash.left -= time.delta_secs();
        if flash.left > 0.0 {
            continue;
        }
        for (part, was) in flash.parts.drain(..) {
            match was {
                Was::Standard(own, copy) if standard.get(part).is_ok_and(|m| m.id() == copy) => {
                    commands.entity(part).try_insert(MeshMaterial3d(own));
                }
                Was::Body(own, copy) if skins.get(part).is_ok_and(|m| m.id() == copy) => {
                    commands.entity(part).try_insert(MeshMaterial3d(own));
                }
                _ => {}
            }
        }
        commands.entity(body).remove::<Flashing>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use terra_motion::moves::MoveSet;
    use terra_walker::shake;

    /// A frame at 60.
    const FRAME_S: f32 = 1.0 / 60.0;

    fn set() -> MoveSet {
        let path = terra_world::data::path(super::super::SET);
        MoveSet::read(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    /// A body struck shows its meshes lit for the set's flash, frame by frame at 60, and then
    /// its own materials again; a body sharing its material is never lit. The frames it shows
    /// lit are the combat scorecard's `hit.flash.frames` (D456: 3 to 4).
    #[test]
    fn a_struck_body_flashes_for_a_few_frames_and_gets_its_own_materials_back() {
        let landed = set().landed;
        let mut world = World::new();
        world.init_resource::<Time>();
        world.init_resource::<Messages<Lit>>();
        world.init_resource::<Copies>();
        world.init_resource::<Assets<StandardMaterial>>();
        world.init_resource::<Assets<BodyMaterial>>();
        let (skin, steel) = {
            let mut skins = world.resource_mut::<Assets<StandardMaterial>>();
            (
                skins.add(Color::srgb(0.6, 0.4, 0.3)),
                skins.add(Color::srgb(0.5, 0.5, 0.55)),
            )
        };
        let body_skin = world
            .resource_mut::<Assets<BodyMaterial>>()
            .add(BodyMaterial {
                base: StandardMaterial::from(Color::srgb(0.6, 0.4, 0.3)),
                extension: Default::default(),
            });
        let mesh = |world: &mut World, material: Handle<StandardMaterial>| {
            world.spawn(MeshMaterial3d(material)).id()
        };
        let sword = mesh(&mut world, steel.clone());
        let arm = mesh(&mut world, skin.clone());
        let torso = world.spawn(MeshMaterial3d(body_skin.clone())).id();
        let struck = world.spawn_empty().add_children(&[sword, arm, torso]).id();
        let other_arm = mesh(&mut world, skin.clone());
        world.spawn_empty().add_children(&[other_arm]);

        world.write_message(Lit {
            body: struck,
            seconds: landed.flash_s,
            strength: 1.0,
        });
        let glows = |world: &World, part: Entity| {
            let standard = world.resource::<Assets<StandardMaterial>>();
            let skins = world.resource::<Assets<BodyMaterial>>();
            let emissive = world
                .get::<MeshMaterial3d<StandardMaterial>>(part)
                .and_then(|m| standard.get(&m.0))
                .map(|m| m.emissive)
                .or_else(|| {
                    world
                        .get::<MeshMaterial3d<BodyMaterial>>(part)
                        .and_then(|m| skins.get(&m.0))
                        .map(|m| m.base.emissive)
                })
                .unwrap();
            emissive.red > 0.5
        };
        let (light, fade) = (world.register_system(light), world.register_system(fade));
        let mut frames = 0;
        for _ in 0..30 {
            world
                .resource_mut::<Time>()
                .advance_by(Duration::from_secs_f32(FRAME_S));
            world.run_system(light).unwrap();
            world.run_system(fade).unwrap();
            let lit = [sword, arm, torso].map(|part| glows(&world, part));
            assert!(lit.iter().all(|&l| l == lit[0]), "the whole body at once");
            assert!(!glows(&world, other_arm), "only the body struck");
            frames += lit[0] as u32;
            world.resource_mut::<Messages<Lit>>().update();
        }
        assert!(
            (1..=6).contains(&frames),
            "a few frames, not a freeze: {frames}"
        );
        assert_eq!(
            world
                .get::<MeshMaterial3d<StandardMaterial>>(sword)
                .unwrap()
                .0,
            steel
        );
        assert_eq!(
            world
                .get::<MeshMaterial3d<StandardMaterial>>(arm)
                .unwrap()
                .0,
            skin
        );
        assert_eq!(
            world.get::<MeshMaterial3d<BodyMaterial>>(torso).unwrap().0,
            body_skin
        );
        assert!(world.get::<Flashing>(struck).is_none());
        println!("measured hit.flash.frames {frames}");
    }

    /// A landed blow's punch of the view, frame by frame at 60 from the frame it lands: all of
    /// it at once, then less each frame, settled (within a pixel, 0.05 degrees at 1080 lines
    /// and 55 degrees) in about Enshrouded's 0.1 s. Its size and the time it takes to settle
    /// are the combat scorecard's `hit.kick.peak` and `hit.kick.settle` (D456).
    #[test]
    fn a_landed_blow_punches_the_view_and_it_settles_within_a_tenth_of_a_second_or_so() {
        let landed = set().landed;
        let tipped: Vec<f32> = (0..60)
            .map(|frame| shake::punched(landed.kick_deg, landed.kick_s, frame as f32 * FRAME_S))
            .collect();
        assert!(tipped.windows(2).all(|w| w[1] <= w[0]), "{tipped:?}");
        let peak = tipped[0];
        let settled = tipped.iter().position(|&deg| deg < 0.05).unwrap();
        assert!(peak > 0.0 && settled > 0, "{tipped:?}");
        assert!(tipped[settled..].iter().all(|&deg| deg < 0.05));
        println!("measured hit.kick.peak {peak}");
        println!("measured hit.kick.settle {}", settled as f32 * FRAME_S);
    }
}
