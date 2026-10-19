//! Uploads to the GPU spread over frames, never a drawn thing left undrawn (D420).
//!
//! Textures go up at most 128 MiB a frame (Bevy's `RenderAssetBytesPerFrame`; Godot's staging
//! pool is 128 MiB), not every texture of a place staged on its first frame. Two things are
//! ours beside Bevy's `prepare_assets`, because Bevy 0.20 drops an updated asset's GPU copy
//! before its replacement passes the cap, and the thing goes undrawn until its turn
//! (`render_asset.rs`, "we remove previous here"): the settlement's ground, re-made as it is
//! worn, went missing for a second and the lake showed through.
//!
//! - **An update to a drawn asset is double-buffered** ([`take`], [`swap`]): it waits in its own
//!   queue, the copy on the GPU drawn meanwhile, and replaces it once prepared, as Godot's and Unreal's
//!   texture streaming keep the resident copy bound until the new one lands.
//! - **Meshes are not held back** ([`lift`], [`lower`]): Bevy's mesh allocator writes every
//!   extracted mesh's vertices and indices into its slabs before `prepare_assets::<RenderMesh>`
//!   runs, and frees the old ones, whatever the cap; holding the `RenderMesh` back saves no
//!   upload and only leaves the mesh undrawn.

use bevy::asset::AssetId;
use bevy::ecs::system::{StaticSystemParam, SystemParamItem};
use bevy::prelude::*;
use bevy::render::{
    Render, RenderApp, RenderSystems,
    mesh::RenderMesh,
    render_asset::{
        ExtractedAssets, PrepareAssetError, RenderAsset, RenderAssetBytesPerFrame,
        RenderAssetBytesPerFrameLimiter, RenderAssets, prepare_assets,
    },
    texture::GpuImage,
};
use std::sync::atomic::Ordering;

/// What may go up to the GPU in a frame.
pub const BYTES_PER_FRAME: usize = 128 * 1024 * 1024;

pub fn plugin(app: &mut App) {
    app.insert_resource(RenderAssetBytesPerFrame::new(BYTES_PER_FRAME));
    let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
        return;
    };
    render_app
        .init_resource::<Updates<GpuImage>>()
        .init_resource::<HeldCap>()
        .add_systems(
            Render,
            (
                take::<GpuImage>.before(prepare_assets::<GpuImage>),
                swap::<GpuImage>
                    .after(prepare_assets::<GpuImage>)
                    .before(lift),
                lift.after(prepare_assets::<GpuImage>)
                    .before(prepare_assets::<RenderMesh>),
                lower.after(prepare_assets::<RenderMesh>),
            )
                .in_set(RenderSystems::PrepareAssets),
        );
}

/// Updates of drawn assets waiting for room under the cap, oldest first.
#[derive(Resource)]
pub struct Updates<A: RenderAsset>(Vec<(AssetId<A::SourceAsset>, A::SourceAsset)>);

impl<A: RenderAsset> Default for Updates<A> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<A: RenderAsset> Updates<A> {
    pub fn waiting(&self) -> usize {
        self.0.len()
    }
}

fn exhausted(limiter: &RenderAssetBytesPerFrameLimiter) -> bool {
    limiter
        .max_bytes
        .is_some_and(|max| limiter.bytes_written.load(Ordering::Relaxed) >= max)
}

/// Takes this frame's updates of drawn assets out of Bevy's hands, into their own queue; their
/// drawn copies stay as they are. Runs before Bevy's `prepare_assets::<A>`.
pub fn take<A: RenderAsset>(
    mut extracted: ResMut<ExtractedAssets<A>>,
    drawn: Res<RenderAssets<A>>,
    mut updates: ResMut<Updates<A>>,
) {
    let extracted = &mut *extracted;
    updates.0.retain(|(id, _)| !extracted.removed.contains(id));
    // `added` keeps the id, so Bevy skips any older copy of it still in its own queue.
    let (mine, theirs) = std::mem::take(&mut extracted.extracted)
        .into_iter()
        .partition(|(id, _)| drawn.get(*id).is_some());
    extracted.extracted = theirs;
    for (id, asset) in mine {
        updates.0.retain(|(waiting, _)| *waiting != id);
        updates.0.push((id, asset));
    }
}

/// Uploads the waiting updates the cap has room for after Bevy's new assets, each replacing its
/// drawn copy, oldest first; the rest wait, their old copy drawn. New assets go first, as they
/// are not drawn at all until uploaded; one update a frame goes up whatever the cap, as Bevy
/// uploads one new asset, so neither starves. Runs after Bevy's `prepare_assets::<A>`.
pub fn swap<A: RenderAsset>(
    mut drawn: ResMut<RenderAssets<A>>,
    mut updates: ResMut<Updates<A>>,
    limiter: Res<RenderAssetBytesPerFrameLimiter>,
    param: StaticSystemParam<A::Param>,
) {
    let mut param = param.into_inner();
    let mut queue = std::mem::take(&mut updates.0).into_iter();
    let mut first = true;
    for (id, asset) in queue.by_ref() {
        if !first && exhausted(&limiter) {
            updates.0.push((id, asset));
            break;
        }
        first = false;
        prepare_one(id, asset, &mut drawn, &mut updates, &limiter, &mut param);
    }
    updates.0.extend(queue);
}

fn prepare_one<A: RenderAsset>(
    id: AssetId<A::SourceAsset>,
    asset: A::SourceAsset,
    drawn: &mut RenderAssets<A>,
    updates: &mut Updates<A>,
    limiter: &RenderAssetBytesPerFrameLimiter,
    param: &mut SystemParamItem<A::Param>,
) {
    let bytes = A::byte_len(&asset).unwrap_or(0);
    match A::prepare_asset(asset, id, param, drawn.get(id)) {
        Ok(prepared) => {
            drawn.insert(id, prepared);
            if limiter.max_bytes.is_some() {
                limiter.bytes_written.fetch_add(bytes, Ordering::Relaxed);
            }
        }
        Err(PrepareAssetError::RetryNextUpdate(asset)) => updates.0.push((id, asset)),
        Err(PrepareAssetError::AsBindGroupError(e)) => {
            error!("{} bind group failed: {e}", core::any::type_name::<A>());
        }
    }
}

/// The cap set aside while meshes are prepared.
#[derive(Resource, Default)]
pub struct HeldCap(Option<usize>);

/// Lifts the cap for `prepare_assets::<RenderMesh>`: a mesh's data is already in its slab.
pub fn lift(mut limiter: ResMut<RenderAssetBytesPerFrameLimiter>, mut held: ResMut<HeldCap>) {
    held.0 = limiter.max_bytes.take();
}

/// Puts the cap back after the meshes.
pub fn lower(mut limiter: ResMut<RenderAssetBytesPerFrameLimiter>, mut held: ResMut<HeldCap>) {
    limiter.max_bytes = held.0.take();
}

#[cfg(test)]
mod tests;
