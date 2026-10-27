//! What a frame puts on the GPU of the sections about to be shown (D541): a budget of
//! corners a frame, spent oldest first, and one mesh past it only if the frame has put
//! nothing yet, so a mesh larger than the budget still goes up, alone.
//!
//! As shipped engines bound streaming uploads per frame rather than per request: Unity's
//! async upload pipeline gives texture and mesh uploads a time slice a frame
//! (`QualitySettings.asyncUploadTimeSlice`), Unreal's Nanite installs at most
//! `r.Nanite.Streaming.MaxPageInstallsPerFrame` pages a frame, however many were asked
//! for. Every section shown let one mesh past the budget before, so the castle's 4 to 8
//! sections laid again as the camera neared put 42 to 154 MiB up in one frame, and Bevy's
//! mesh allocator and preparing took 10 to 24 ms of it.

use super::{DETAIL_UPLOAD_CORNERS, Parts, UPLOAD_CORNERS};
use bevy::prelude::*;

/// What is left of a frame's budgets: building's own, and groups' details streaming in.
pub(super) struct FrameBudget {
    left: [usize; 2],
    /// Whether the frame has put a mesh yet.
    put: bool,
}

impl FrameBudget {
    pub(super) fn new() -> Self {
        Self::of([UPLOAD_CORNERS, DETAIL_UPLOAD_CORNERS])
    }

    fn of(left: [usize; 2]) -> Self {
        Self { left, put: false }
    }

    /// Puts what of `parts` the budget has room for, the details' if `detail`, its arrays'
    /// meshes kept on the CPU too if `cpu`; whether all of it is put.
    pub(super) fn put(
        &mut self,
        parts: &mut Parts,
        meshes: &mut Assets<Mesh>,
        cpu: bool,
        detail: bool,
    ) -> bool {
        let left = &mut self.left[usize::from(detail)];
        let mut spend = match (*left, self.put) {
            (0, false) => 1,
            (left, _) => left,
        };
        let (all, count) = parts.put(meshes, cpu, &mut spend);
        *left = spend.min(*left);
        self.put |= count > 0;
        all
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::asset::RenderAssetUsages;
    use bevy::render::render_resource::PrimitiveTopology;

    fn section(meshes: &[usize]) -> Parts {
        let made = |corners: usize| {
            super::super::Staged::Made(Box::new(
                Mesh::new(
                    PrimitiveTopology::TriangleList,
                    RenderAssetUsages::RENDER_WORLD,
                )
                .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0f32; 3]; corners]),
            ))
        };
        Parts {
            layers: Vec::new(),
            parts: Vec::new(),
            keys: Vec::new(),
            plain: None,
            next: meshes.iter().map(|&corners| made(corners)).collect(),
            shell: None,
            ghosts: [None, None],
            far: None,
            proxy: None,
            centre: Vec3::ZERO,
            bounds: None,
        }
    }

    /// Corners each frame puts until every section is put, with the frame each was done.
    fn frames(budget: [usize; 2], sections: &mut [(Parts, bool)]) -> (Vec<usize>, Vec<usize>) {
        let mut meshes = Assets::<Mesh>::default();
        let (mut put, mut done) = (Vec::new(), vec![usize::MAX; sections.len()]);
        while done.contains(&usize::MAX) {
            let mut frame = FrameBudget::of(budget);
            let before: usize = meshes.iter().map(|(_, m)| m.count_vertices()).sum();
            for (i, (parts, detail)) in sections.iter_mut().enumerate() {
                if done[i] == usize::MAX && frame.put(parts, &mut meshes, false, *detail) {
                    done[i] = put.len();
                }
            }
            let after: usize = meshes.iter().map(|(_, m)| m.count_vertices()).sum();
            put.push(after - before);
            assert!(put.len() < 100, "never all put");
        }
        (put, done)
    }

    #[test]
    fn sections_larger_than_the_budget_go_up_one_mesh_a_frame_not_one_each() {
        // Four sections of one mesh each, every mesh three times the budget: the castle's
        // sections laid again as the camera nears, which all went up in one frame.
        let mut sections: Vec<_> = (0..4).map(|_| (section(&[300]), false)).collect();
        let (put, done) = frames([100, 40], &mut sections);
        assert_eq!(put, vec![300; 4]);
        assert_eq!(done, vec![0, 1, 2, 3], "oldest first");
    }

    #[test]
    fn a_frame_fills_its_budget_and_goes_one_mesh_past_it_at_most() {
        let mut sections: Vec<_> = (0..5).map(|_| (section(&[30, 30]), false)).collect();
        let (put, _) = frames([100, 40], &mut sections);
        // 300 corners in all: each frame puts meshes until it has spent 100, the last
        // taking it past by less than a mesh.
        assert_eq!(put.iter().sum::<usize>(), 300);
        assert!(put.iter().all(|&corners| corners <= 100 + 30), "{put:?}");
        assert_eq!(put.len(), 3, "{put:?}");
    }

    #[test]
    fn details_spend_their_own_budget_beside_buildings() {
        // A building's change that spends its whole budget leaves the details theirs.
        let mut sections = vec![(section(&[100]), false), (section(&[40]), true)];
        let (put, done) = frames([100, 40], &mut sections);
        assert_eq!(put, vec![140]);
        assert_eq!(done, vec![0, 0]);
    }
}
