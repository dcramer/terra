//! Texture sets kept compressed on disk once made (D362), as an engine keeps its cooked
//! textures (Unreal's derived data cache): BC7 at 2,048 takes a worker a few tenths of a
//! second a set, on the workers that lay sections, so a set compressed once is read back
//! the next time rather than compressed again. Kept in the user's cache folder
//! (`terra_world::cache::folder`), under a key from the pictures read and how they are
//! stored, so a changed picture or encoder is compressed afresh; anything that cannot be
//! read or written is compressed as if there were no cache.

use super::bc::Codec;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

/// Changed whenever what is stored for the same pictures changes: an encoder, a mip filter.
const VERSION: u32 = 1;

/// A texture set's four maps, each level of each, and its cells.
pub type Stored = ([Vec<Vec<u8>>; 4], Vec<u8>);

/// A texture set's maps as read (each its texels, size and channels, or none), and how they
/// are stored, as one key.
pub fn key(
    maps: &[Option<(Vec<u8>, u32, u32, usize)>],
    sides: [u32; 4],
    codecs: [Codec; 4],
    share: f32,
) -> u64 {
    let mut hasher = std::hash::DefaultHasher::new();
    VERSION.hash(&mut hasher);
    sides.hash(&mut hasher);
    format!("{codecs:?}").hash(&mut hasher);
    share.to_bits().hash(&mut hasher);
    for map in maps {
        match map {
            Some((data, w, h, channels)) => (data, w, h, channels).hash(&mut hasher),
            None => 0u8.hash(&mut hasher),
        }
    }
    hasher.finish()
}

/// Where a key's set is kept, in the user's cache folder.
pub fn file(key: u64) -> Option<PathBuf> {
    terra_world::cache::folder().map(|folder| folder.join("arrays").join(format!("{key:016x}.bin")))
}

/// A set as stored: each map's count of levels, then each level's length and bytes; then the
/// cells' length and bytes.
pub fn to_bytes((maps, cells): &Stored) -> Vec<u8> {
    let mut out = Vec::new();
    let mut put = |data: &[u8]| {
        out.extend((data.len() as u32).to_le_bytes());
        out.extend(data);
    };
    for map in maps {
        put(&(map.len() as u32).to_le_bytes());
        for level in map {
            put(level);
        }
    }
    put(cells);
    out
}

/// A set from its bytes, if they are whole.
pub fn from_bytes(bytes: &[u8]) -> Option<Stored> {
    let mut at = 0usize;
    let mut next = || -> Option<&[u8]> {
        let n = u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?) as usize;
        let out = bytes.get(at + 4..at + 4 + n)?;
        at += 4 + n;
        Some(out)
    };
    let mut maps: [Vec<Vec<u8>>; 4] = Default::default();
    for map in &mut maps {
        let levels = u32::from_le_bytes(next()?.try_into().ok()?);
        for _ in 0..levels {
            map.push(next()?.to_vec());
        }
    }
    let cells = next()?.to_vec();
    (at == bytes.len()).then_some((maps, cells))
}

/// The set kept under a key, if it is there whole.
pub fn read(path: &Path) -> Option<Stored> {
    from_bytes(&std::fs::read(path).ok()?)
}

/// Keeps a set under a key, written whole or not at all.
pub fn write(path: &Path, stored: &Stored) {
    let part = path.with_extension("part");
    let written = path
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| std::fs::write(&part, to_bytes(stored)))
        .and_then(|()| std::fs::rename(&part, path));
    if let Err(error) = written {
        bevy::log::warn!(
            "Build: a texture set not kept in {}: {error}",
            path.display()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_set_comes_back_as_kept_and_a_cut_one_not_at_all() {
        let stored: Stored = (
            [
                vec![vec![1, 2, 3], vec![4]],
                vec![vec![5; 16]],
                vec![],
                vec![vec![9; 8], vec![7; 2], vec![6]],
            ],
            vec![8; 12],
        );
        let bytes = to_bytes(&stored);
        assert_eq!(from_bytes(&bytes), Some(stored));
        assert_eq!(from_bytes(&bytes[..bytes.len() - 1]), None);
        assert_eq!(from_bytes(&[bytes.as_slice(), &[0]].concat()), None);
    }

    #[test]
    fn a_changed_picture_or_side_is_another_key() {
        let maps = vec![Some((vec![10u8; 64], 4, 4, 4)), None];
        let codecs = [Codec::Bc7, Codec::Bc5, Codec::Bc1, Codec::Bc4];
        let sides = [2048, 2048, 1024, 1024];
        let k = key(&maps, sides, codecs, 0.2);
        assert_eq!(k, key(&maps, sides, codecs, 0.2));
        let mut changed = maps.clone();
        changed[0].as_mut().unwrap().0[5] = 11;
        assert_ne!(k, key(&changed, sides, codecs, 0.2));
        assert_ne!(k, key(&maps, [1024, 1024, 1024, 1024], codecs, 0.2));
    }
}
