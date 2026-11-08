//! A kind's clips as its animation graph has them, and the graph made from its model.

use super::{Kinds, WARNING};
use crate::species::{Bestiary, MOST_BLOWS};
use bevy::gltf::Gltf;
use bevy::prelude::*;
use bevy::world_serialization::WorldAsset;

use super::Species;

#[derive(Clone, Copy)]
pub(super) struct Clips {
    pub(super) idle: AnimationNodeIndex,
    pub(super) walk: AnimationNodeIndex,
    /// Its trot, or its run for a kind that does not trot.
    pub(super) trot: AnimationNodeIndex,
    pub(super) run: AnimationNodeIndex,
    pub(super) eat: AnimationNodeIndex,
    pub(super) attack: AnimationNodeIndex,
    pub(super) death: AnimationNodeIndex,
    pub(super) fly: AnimationNodeIndex,
    /// Its other blows' clips (`SpeciesDef::blows`), and its warning's.
    pub(super) blows: [Option<AnimationNodeIndex>; MOST_BLOWS],
    pub(super) warn: Option<AnimationNodeIndex>,
}

impl Clips {
    /// The clip of the blow a creature is dealing ([`Creature::blow`]).
    pub(super) fn blow(&self, blow: u8) -> AnimationNodeIndex {
        let other = match blow {
            0 => None,
            WARNING => self.warn,
            other => self.blows.get(other as usize - 1).copied().flatten(),
        };
        other.unwrap_or(self.attack)
    }
}

/// Each kind's scene, its animation graph and the clips in it, or nothing if a model
/// lacks its scene or a clip its kind asks for.
pub(super) fn animated(
    kinds: &Kinds,
    gltfs: &Assets<Gltf>,
    bestiary: &Bestiary,
    graphs: &mut Assets<AnimationGraph>,
) -> Option<Vec<(Species, Handle<WorldAsset>, Handle<AnimationGraph>, Clips)>> {
    let mut ready = Vec::new();
    for (species, model) in &kinds.models {
        let gltf = gltfs.get(model)?;
        let Some(scene) = gltf.default_scene.clone() else {
            error!("{} has no scene.", bestiary.name(*species));
            return None;
        };
        let def = bestiary.get(*species);
        let about = &def.clips;
        let clip = |name: &str| gltf.named_animations.get(name).cloned();
        let mut names = vec![
            &about.idle,
            &about.walk,
            &about.run,
            about.eat.as_ref().unwrap_or(&about.idle),
            &about.attack,
            &about.death,
            about.fly.as_ref().unwrap_or(&about.run),
            about.trot.as_ref().map_or(&about.run, |(trot, _)| trot),
        ];
        // Then its other blows' clips, three at most, and its warning's.
        let others = def.blows.len().min(MOST_BLOWS);
        names.extend(def.blows.iter().take(MOST_BLOWS).map(|blow| &blow.clip));
        names.extend(def.warns.as_ref().map(|(warn, _)| warn));
        let Some(clips) = names
            .iter()
            .map(|name| clip(name))
            .collect::<Option<Vec<_>>>()
        else {
            error!("{} is missing an animation.", bestiary.name(*species));
            return None;
        };
        let (graph, nodes) = AnimationGraph::from_clips(clips);
        ready.push((
            *species,
            scene,
            graphs.add(graph),
            Clips {
                idle: nodes[0],
                walk: nodes[1],
                run: nodes[2],
                eat: nodes[3],
                attack: nodes[4],
                death: nodes[5],
                fly: nodes[6],
                trot: nodes[7],
                blows: std::array::from_fn(|at| (at < others).then(|| nodes[8 + at])),
                warn: def.warns.as_ref().map(|_| nodes[8 + others]),
            },
        ));
    }
    Some(ready)
}
