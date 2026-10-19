//! Bevy's glTF loader behind a gate, so a model's loading never nests inside another's.
//!
//! Bevy 0.20's glTF loader decodes a model's images in a scope on the IO pool, and a thread
//! blocked in a scope runs whatever else is queued on the pool meanwhile, other glTF loads
//! among them, each opening a scope of its own: load, scope, load, scope, ... on one thread's
//! stack, a step for every model loading at once (bevyengine/bevy#15271, open). The yard's
//! startup on Windows went 84 deep, about 24 KiB a step, and overflowed a 2 MiB IO thread
//! every time; a town's hundreds of models would overflow any size. So the loader here is
//! Bevy's own, unchanged, behind a gate that lets as many loads through as the IO pool has
//! threads (D184, D361: Bevy's plugin replaced, its code untouched). A load waiting at the
//! gate yields to the pool instead of running inside the scope that picked it up, so one
//! thread holds at most that many loads on its stack however many models are asked for,
//! and a model's images still decode in parallel.
//!
//! The gate also reads a model's cooked copy where the bake has made one (D188,
//! `terra_core::cooked`): the same model with its pictures block-compressed with their
//! mipmaps, found by the key of the bytes asked for, so a changed model is read as it is
//! until it is cooked again. Only where the GPU samples BC; elsewhere (a software renderer)
//! the model as made.

use std::sync::Arc;

use async_lock::Semaphore;
use bevy::{
    asset::{
        AssetLoader, LoadContext,
        io::{Reader, SliceReader},
    },
    gltf::{
        DefaultGltfImageSampler, Gltf, GltfError, GltfLoader, GltfLoaderSettings, GltfMaterial,
        GltfMesh, GltfNode, GltfPlugin, GltfPrimitive, GltfSkin, extensions::GltfExtensionHandlers,
    },
    image::{CompressedImageFormatSupport, CompressedImageFormats},
    prelude::*,
    tasks::IoTaskPool,
};

/// Stands in for Bevy's `GltfPlugin`, with its settings, registering the gated loader in
/// place of Bevy's: `DefaultPlugins` take it where `GltfPlugin` was (see `replace`).
#[derive(Default)]
pub struct GatedGltfPlugin(pub GltfPlugin);

/// `DefaultPlugins` with the gated loader in place of Bevy's.
pub fn replace(plugins: bevy::app::PluginGroupBuilder) -> bevy::app::PluginGroupBuilder {
    plugins
        .add_before::<GltfPlugin>(GatedGltfPlugin::default())
        .disable::<GltfPlugin>()
}

impl Plugin for GatedGltfPlugin {
    // As `GltfPlugin::build` (bevy_gltf 0.20), with this loader's type preregistered.
    fn build(&self, app: &mut App) {
        app.init_asset::<Gltf>()
            .init_asset::<GltfNode>()
            .init_asset::<GltfPrimitive>()
            .init_asset::<GltfMesh>()
            .init_asset::<GltfSkin>()
            .init_asset::<GltfMaterial>()
            .preregister_asset_loader::<GatedGltfLoader>(&["gltf", "glb"])
            .init_resource::<GltfExtensionHandlers>();
    }

    // As `GltfPlugin::finish` (bevy_gltf 0.20), the loader then put behind the gate.
    fn finish(&self, app: &mut App) {
        let settings = &self.0;
        let supported_compressed_formats = app
            .world()
            .get_resource::<CompressedImageFormatSupport>()
            .map_or(CompressedImageFormats::NONE, |support| support.0);
        let sampler = DefaultGltfImageSampler::new(&settings.default_sampler);
        let default_sampler = sampler.get_internal();
        app.insert_resource(sampler);
        let extensions = app.world().resource::<GltfExtensionHandlers>().0.clone();
        let loader = GltfLoader {
            supported_compressed_formats,
            custom_vertex_attributes: settings.custom_vertex_attributes.clone(),
            default_sampler,
            default_convert_coordinates: settings.convert_coordinates,
            extensions,
            default_skinned_mesh_bounds_policy: settings.skinned_mesh_bounds_policy,
            default_mesh_compression: settings.mesh_compression.clone(),
        };
        let threads = IoTaskPool::try_get().map_or(1, |pool| pool.thread_num().max(1));
        let cooked = crate::cooked::enable(supported_compressed_formats);
        app.register_asset_loader(GatedGltfLoader {
            loader,
            gate: Arc::new(Semaphore::new(threads)),
            cooked,
        });
    }
}

/// Bevy's glTF loader, run by no more loads at once than the gate lets through.
#[derive(TypePath)]
pub struct GatedGltfLoader {
    loader: GltfLoader,
    gate: Arc<Semaphore>,
    /// Where cooked models are kept, where the GPU can sample them.
    cooked: Option<std::path::PathBuf>,
}

impl AssetLoader for GatedGltfLoader {
    type Asset = Gltf;
    type Settings = GltfLoaderSettings;
    type Error = GltfError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        settings: &GltfLoaderSettings,
        load_context: &mut LoadContext<'_>,
    ) -> Result<Gltf, GltfError> {
        let _through = self.gate.acquire().await;
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let cooked = self.cooked.as_ref().and_then(|folder| {
            let path = terra_core::cooked::find(folder, &terra_core::cooked::model_name(&bytes))?;
            std::fs::read(path).ok()
        });
        let mut read = SliceReader::new(cooked.as_deref().unwrap_or(&bytes));
        self.loader.load(&mut read, settings, load_context).await
    }

    fn extensions(&self) -> &[&str] {
        self.loader.extensions()
    }
}
