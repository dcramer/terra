//! Meshes made on many threads at once (`terra_gen::par::map`) and put in the world's
//! `Assets<Mesh>` after, in one go on the main thread: each is given its handle at once,
//! reserved from the assets (Bevy's `Assets::reserve_handle`, as its asset loaders do from
//! their own threads), so whatever is made of it can hold it before it is in. A world's
//! entering grew its thirty trees one after the next, 2.1 s of a 12 s frame at the lake
//! (crossing-freeze j1).

use bevy::prelude::*;
use std::sync::Mutex;

/// Where meshes go while they are made off the main thread, for [`MeshSink::put_in`].
pub struct MeshSink<'a> {
    assets: &'a Assets<Mesh>,
    made: Mutex<Vec<(AssetId<Mesh>, Mesh)>>,
}

impl<'a> MeshSink<'a> {
    pub fn new(assets: &'a Assets<Mesh>) -> Self {
        Self {
            assets,
            made: Mutex::new(Vec::new()),
        }
    }

    /// A mesh made, with the handle it will be found by once put in.
    pub fn add(&self, mesh: impl Into<Mesh>) -> Handle<Mesh> {
        let handle = self.assets.reserve_handle();
        let mut made = self
            .made
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        made.push((handle.id(), mesh.into()));
        handle
    }

    /// The meshes made, to be put in once nothing borrows the assets.
    pub fn made(self) -> Vec<(AssetId<Mesh>, Mesh)> {
        self.made
            .into_inner()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Puts the meshes made in, each under the handle it was given.
    pub fn put_in(made: Vec<(AssetId<Mesh>, Mesh)>, assets: &mut Assets<Mesh>) {
        for (id, mesh) in made {
            if let Err(why) = assets.insert(id, mesh) {
                error!(
                    "a mesh made off the main thread could not be put in under its handle ({why}); \
                     its handle was let go before it was in, so nothing draws it"
                );
            }
        }
    }

    /// Makes with `make`, its meshes into `sink`, then puts them in: a whole build of meshes
    /// on many threads, as one call.
    pub fn making<T>(assets: &mut Assets<Mesh>, make: impl FnOnce(&MeshSink) -> T) -> T {
        let sink = MeshSink::new(assets);
        let out = make(&sink);
        let made = sink.made();
        Self::put_in(made, assets);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::asset::RenderAssetUsages;
    use bevy::mesh::{Indices, PrimitiveTopology};

    fn triangle(lift: f32) -> Mesh {
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_POSITION,
            vec![[0.0, lift, 0.0], [1.0, lift, 0.0], [0.0, lift, 1.0]],
        )
        .with_inserted_indices(Indices::U32(vec![0, 1, 2]))
    }

    fn lift_of(mesh: &Mesh) -> f32 {
        match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
            Some(bevy::mesh::VertexAttributeValues::Float32x3(points)) => points[0][1],
            _ => f32::NAN,
        }
    }

    /// Meshes made on many threads are each found under the handle their maker was given.
    #[test]
    fn meshes_made_on_many_threads_are_found_by_their_handles() {
        let mut assets = Assets::<Mesh>::default();
        let lifts: Vec<f32> = (0..64).map(|n| n as f32).collect();
        let handles = MeshSink::making(&mut assets, |sink| {
            terra_gen::par::map(&lifts, |&lift| sink.add(triangle(lift)))
        });
        assert_eq!(assets.len(), lifts.len());
        for (lift, handle) in lifts.iter().zip(&handles) {
            let mesh = assets.get(handle).expect("each handle finds its mesh");
            assert_eq!(lift_of(mesh), *lift);
        }
    }
}
