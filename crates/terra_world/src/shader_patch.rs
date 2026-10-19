//! Patches one of Bevy's shader libraries where it is loaded, as an asset (D184: its code
//! untouched, the few lines we need changed in memory). The library is held until it has
//! loaded, given to an edit, and replaced by what the edit returns; if the edit cannot find
//! the lines it changes (Bevy's source moved on), the library is left as it is and a warning
//! says what is lost.

use bevy::prelude::*;
use bevy::shader::{Shader, Source};

/// Patches the library at `path` (an `embedded://` path) with `edit` once it has loaded,
/// or warns with `missing` if `edit` finds nothing to change.
pub fn patch_library(
    app: &mut App,
    path: &'static str,
    edit: fn(&str) -> Option<String>,
    missing: &'static str,
) {
    app.add_systems(
        Update,
        move |mut state: Local<Patching>,
              assets: Res<AssetServer>,
              mut shaders: ResMut<Assets<Shader>>| {
            match &*state {
                Patching::Start => *state = Patching::Waiting(assets.load(path)),
                Patching::Waiting(handle) => {
                    let Some(shader) = shaders.get(handle) else {
                        return;
                    };
                    let patched = match &shader.source {
                        Source::Wesl(source) | Source::Wgsl(source) => edit(source),
                        Source::SpirV(_) => None,
                    };
                    match patched {
                        Some(source) => {
                            if let Some(mut shader) = shaders.get_mut(handle) {
                                shader.source = match shader.source {
                                    Source::Wgsl(_) => Source::Wgsl(source.into()),
                                    _ => Source::Wesl(source.into()),
                                };
                            }
                        }
                        None => warn!("{missing} (not found in {path})"),
                    }
                    *state = Patching::Done;
                }
                Patching::Done => {}
            }
        },
    );
}

/// The library, held so it stays loaded, until it is patched.
#[derive(Default)]
pub enum Patching {
    #[default]
    Start,
    Waiting(Handle<Shader>),
    Done,
}

/// `source` with `from` swapped for `to`, if `from` is in it exactly once.
pub fn swap_once(source: &str, from: &str, to: &str) -> Option<String> {
    let source = source.replace("\r\n", "\n");
    (source.matches(from).count() == 1).then(|| source.replace(from, to))
}
