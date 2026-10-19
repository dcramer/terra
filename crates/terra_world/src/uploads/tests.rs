//! The upload cap against Bevy's own `prepare_assets`, with a stand-in asset and no GPU.

use super::*;
use bevy::ecs::schedule::Schedule;
use bevy::ecs::system::lifetimeless::SRes;

/// A stand-in for a texture: its size and which version of it this is.
#[derive(Asset, TypePath, Clone)]
struct Picture {
    bytes: usize,
    version: u32,
}

/// What the GPU holds of a picture: the version uploaded.
struct Uploaded(u32);

#[derive(Resource, Default)]
struct Prepared;

impl RenderAsset for Uploaded {
    type SourceAsset = Picture;
    type Param = SRes<Prepared>;

    fn byte_len(picture: &Picture) -> Option<usize> {
        Some(picture.bytes)
    }

    fn prepare_asset(
        picture: Picture,
        _: AssetId<Picture>,
        _: &mut SystemParamItem<Self::Param>,
        _: Option<&Self>,
    ) -> Result<Self, PrepareAssetError<Picture>> {
        Ok(Uploaded(picture.version))
    }
}

const CAP: usize = 100;

fn world() -> World {
    let mut world = World::new();
    world.init_resource::<ExtractedAssets<Uploaded>>();
    world.init_resource::<RenderAssets<Uploaded>>();
    world.init_resource::<bevy::render::render_asset::PrepareNextFrameAssets<Uploaded>>();
    world.init_resource::<Updates<Uploaded>>();
    world.init_resource::<HeldCap>();
    world.init_resource::<Prepared>();
    world.insert_resource(RenderAssetBytesPerFrameLimiter {
        max_bytes: Some(CAP),
        ..default()
    });
    world
}

/// One frame: the extract hands over `pictures`, then the schedule prepares them.
fn frame(world: &mut World, schedule: &mut Schedule, pictures: &[(u32, usize, u32)]) {
    world
        .resource_mut::<RenderAssetBytesPerFrameLimiter>()
        .reset();
    let mut extracted = world.resource_mut::<ExtractedAssets<Uploaded>>();
    for &(index, bytes, version) in pictures {
        let id = id(index);
        extracted.extracted.push((id, Picture { bytes, version }));
        extracted.added.insert(id);
    }
    schedule.run(world);
    let mut extracted = world.resource_mut::<ExtractedAssets<Uploaded>>();
    extracted.added.clear();
    extracted.extracted.clear();
}

fn id(index: u32) -> AssetId<Picture> {
    AssetId::from(bevy::asset::uuid::Uuid::from_u128(u128::from(index) + 1))
}

/// The version drawn of each picture, `None` for one undrawn.
fn drawn(world: &World, indices: &[u32]) -> Vec<Option<u32>> {
    let drawn = world.resource::<RenderAssets<Uploaded>>();
    indices
        .iter()
        .map(|&i| drawn.get(id(i)).map(|u| u.0))
        .collect()
}

#[test]
fn bevy_alone_leaves_an_updated_asset_undrawn_while_it_waits() {
    // The bug this module is for, so the test below is known to test something.
    let mut world = world();
    let mut schedule = Schedule::default();
    schedule.add_systems(prepare_assets::<Uploaded>);
    frame(&mut world, &mut schedule, &[(0, 10, 1), (1, 10, 1)]);
    assert_eq!(drawn(&world, &[0, 1]), [Some(1), Some(1)]);
    frame(&mut world, &mut schedule, &[(0, 120, 2), (1, 120, 2)]);
    assert_eq!(drawn(&world, &[0, 1]), [Some(2), None]);
}

#[test]
fn an_updated_asset_stays_drawn_until_its_replacement_is_uploaded() {
    let mut world = world();
    let mut schedule = Schedule::default();
    schedule.add_systems(
        (
            take::<Uploaded>,
            prepare_assets::<Uploaded>,
            swap::<Uploaded>,
        )
            .chain(),
    );
    frame(
        &mut world,
        &mut schedule,
        &[(0, 10, 1), (1, 10, 1), (2, 10, 1)],
    );
    assert_eq!(drawn(&world, &[0, 1, 2]), [Some(1); 3]);

    // Three updates of 120 bytes against a cap of 100: one a frame, the others drawn as they were.
    frame(
        &mut world,
        &mut schedule,
        &[(0, 120, 2), (1, 120, 2), (2, 120, 2)],
    );
    assert_eq!(drawn(&world, &[0, 1, 2]), [Some(2), Some(1), Some(1)]);
    // A newer update of a waiting one takes its place in the queue.
    frame(&mut world, &mut schedule, &[(2, 120, 3)]);
    assert_eq!(drawn(&world, &[0, 1, 2]), [Some(2), Some(2), Some(1)]);
    frame(&mut world, &mut schedule, &[]);
    assert_eq!(drawn(&world, &[0, 1, 2]), [Some(2), Some(2), Some(3)]);
    assert_eq!(world.resource::<Updates<Uploaded>>().waiting(), 0);
}

#[test]
fn new_assets_go_first_updates_still_go_up_and_a_removed_update_is_dropped() {
    let mut world = world();
    let mut schedule = Schedule::default();
    schedule.add_systems(
        (
            take::<Uploaded>,
            prepare_assets::<Uploaded>,
            swap::<Uploaded>,
        )
            .chain(),
    );
    frame(&mut world, &mut schedule, &[(0, 10, 1)]);
    frame(&mut world, &mut schedule, &[(1, 120, 1), (0, 80, 2)]);
    // The new picture goes first, and one update a frame goes up whatever the cap.
    assert_eq!(drawn(&world, &[0, 1]), [Some(2), Some(1)]);
    frame(&mut world, &mut schedule, &[(2, 120, 1), (3, 120, 1)]);
    assert_eq!(drawn(&world, &[2, 3]), [Some(1), None]);

    // Waiting updates never starve a new picture waiting in Bevy's queue.
    frame(&mut world, &mut schedule, &[(0, 120, 3), (1, 120, 2)]);
    assert_eq!(drawn(&world, &[0, 1, 3]), [Some(3), Some(1), Some(1)]);
    world
        .resource_mut::<ExtractedAssets<Uploaded>>()
        .removed
        .insert(id(1));
    frame(&mut world, &mut schedule, &[]);
    assert_eq!(drawn(&world, &[0, 1, 3]), [Some(3), None, Some(1)]);
    assert_eq!(world.resource::<Updates<Uploaded>>().waiting(), 0);
}

#[test]
fn a_mesh_is_never_held_back_and_the_cap_is_put_back() {
    let mut world = world();
    let mut schedule = Schedule::default();
    schedule.add_systems((lift, prepare_assets::<Uploaded>, lower).chain());
    frame(
        &mut world,
        &mut schedule,
        &[(0, 120, 1), (1, 120, 1), (2, 120, 1)],
    );
    assert_eq!(drawn(&world, &[0, 1, 2]), [Some(1); 3]);
    let limiter = world.resource::<RenderAssetBytesPerFrameLimiter>();
    assert_eq!(limiter.max_bytes, Some(CAP));
    assert_eq!(limiter.bytes_written.load(Ordering::Relaxed), 0);
}
