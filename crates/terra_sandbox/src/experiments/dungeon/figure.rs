//! A person at a dungeon's goal (`theme::Figure`, the trapped miner): the base body, dressed
//! in its looks, playing its clip over and over, as the crowd experiment dresses and plays
//! its bodies. Round 2's mine judges read the bare body standing in its bind pose on the goal's
//! block as a model that failed to load.

use bevy::gltf::Gltf;
use bevy::prelude::*;
use bevy::world_serialization::{WorldAssetRoot, WorldInstanceReady};
use terra_dungeon::theme::Figure;
use terra_game::{appearance::Appearance, character};

/// Spawns the figure under `parent` at `placed` with `extra`, its body `model`: dressed and
/// its clip played once its scene is in the world.
pub(super) fn spawn<'a>(
    parent: &'a mut ChildSpawnerCommands,
    assets: &AssetServer,
    (figure, model): (&Figure, &str),
    placed: Transform,
    extra: impl Bundle,
) -> EntityCommands<'a> {
    let body: Handle<Gltf> = assets.load(model.to_string());
    let scene = assets.load(GltfAssetLabel::Scene(0).from_asset(model.to_string()));
    let looks: Vec<_> = (figure.wears.iter())
        .map(|item| assets.load(GltfAssetLabel::Scene(0).from_asset(character::look(item))))
        .collect();
    let clip = figure.clip.clone();
    let mut spawned = parent.spawn((
        character::Body,
        Appearance::default(),
        WorldAssetRoot(scene),
        placed,
        extra,
    ));
    spawned.observe(
        move |ready: On<WorldInstanceReady>,
              mut commands: Commands,
              children: Query<&Children>,
              mut players: Query<&mut AnimationPlayer>,
              gltfs: Res<Assets<Gltf>>,
              mut graphs: ResMut<Assets<AnimationGraph>>| {
            for look in &looks {
                character::wear(&mut commands, look.clone(), ready.entity);
            }
            let Some(handle) = gltfs
                .get(&body)
                .and_then(|gltf| gltf.named_animations.get(clip.as_str()).cloned())
            else {
                error!("Dungeon figure: the body has no clip {clip}");
                return;
            };
            let Some(entity) = children
                .iter_descendants(ready.entity)
                .find(|each| players.contains(*each))
            else {
                return;
            };
            let (graph, node) = AnimationGraph::from_clip(handle);
            commands
                .entity(entity)
                .insert(AnimationGraphHandle(graphs.add(graph)));
            if let Ok(mut player) = players.get_mut(entity) {
                player.play(node).repeat();
            }
        },
    );
    spawned
}
