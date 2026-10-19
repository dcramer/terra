//! Cooked textures (D188): the contract between the bake (`cargo xtask cook`,
//! `terra_bake`) and the game that loads what it baked. Shipped engines cook a texture
//! offline into the block-compressed format the GPU samples, with its mipmaps, and keep it
//! in a derived-data cache keyed by the source (Unreal's DDC, Godot's `.godot/imported`);
//! the game reads the cooked copy where one is fresh and the source otherwise. Here the key
//! is the source file's bytes and how it is cooked, so a changed picture or model is a
//! different key: never a stale copy, only an uncooked one until the bake runs again.

pub mod mips;

use std::path::{Path, PathBuf};

/// Raised when what the bake writes changes, so every cooked file is made again.
pub const VERSION: u64 = 1;

/// The user's cache folder's `terra/`: XDG's on Linux, `~/.cache` where there is a home,
/// Windows' local application data; none where there is no such folder.
pub fn cache_folder() -> Option<PathBuf> {
    let cache = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
        .or_else(|| std::env::var_os("LOCALAPPDATA").map(PathBuf::from))?;
    Some(cache.join("terra"))
}

/// Where cooked files are kept, shared by every checkout as the key says what each is.
pub fn folder() -> Option<PathBuf> {
    Some(cache_folder()?.join("cooked"))
}

/// What a cooked picture is, which picks its format: colour (BC7 in sRGB), a tangent-space
/// normal (BC5, its two components; the shader makes the third from them), two channels as
/// stored (BC5, a flipbook's motion), or other data (BC7, or BC4 for one channel).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Colour,
    Normal,
    Two,
    Data,
}

/// How a picture is cooked, as both sides name it: its kind, the layers of an array it is
/// stacked as (1 for a plain picture), and whether it gets mipmaps.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Cook {
    pub kind: Kind,
    pub layers: u32,
    pub mips: bool,
}

impl Cook {
    fn tag(self) -> String {
        let kind = match self.kind {
            Kind::Colour => 'c',
            Kind::Normal => 'n',
            Kind::Two => 't',
            Kind::Data => 'd',
        };
        format!("{kind}{}{}", self.layers, if self.mips { "m" } else { "" })
    }
}

/// The cooked file's name for a picture's source bytes cooked as `cook`.
pub fn picture_name(source: &[u8], cook: Cook) -> String {
    format!("{:016x}-{}.ktx2", key(source), cook.tag())
}

/// The cooked file's name for a model (a `.glb`), its pictures cooked by their use.
pub fn model_name(source: &[u8]) -> String {
    format!("{:016x}.glb", key(source))
}

/// The key of a source's bytes under this [`VERSION`]: XXH3, fast enough to hash every
/// model as it is read.
pub fn key(source: &[u8]) -> u64 {
    twox_hash::XxHash3_64::oneshot_with_seed(VERSION, source)
}

/// The cooked file `name` in `folder`, if it is there.
pub fn find(folder: &Path, name: &str) -> Option<PathBuf> {
    let path = folder.join(name);
    path.is_file().then_some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_changed_source_or_cook_is_another_file() {
        let cook = Cook {
            kind: Kind::Colour,
            layers: 1,
            mips: true,
        };
        let a = picture_name(b"one", cook);
        assert_eq!(a, picture_name(b"one", cook));
        assert_ne!(a, picture_name(b"two", cook));
        assert_ne!(a, picture_name(b"one", Cook { layers: 6, ..cook }));
        assert_ne!(
            a,
            picture_name(
                b"one",
                Cook {
                    kind: Kind::Normal,
                    ..cook
                }
            )
        );
        assert_ne!(
            a,
            picture_name(
                b"one",
                Cook {
                    mips: false,
                    ..cook
                }
            )
        );
    }
}
