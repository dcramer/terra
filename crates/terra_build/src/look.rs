//! What a construction material is held to on the material bench (`cargo xtask material`,
//! tools/kits/material.ts, D386): the numbers agents measured by hand to judge a material
//! (research/texture-detail-2026-10-15.md, research/colour-balance-2026-10-15.md), as
//! targets its pattern's file gives every material of it, so a change that makes one worse
//! fails the bench rather than waiting for a review. And the photograph of the real thing a
//! material is judged beside, which its pack names.
//!
//! Glossary: the *finest octave* is the share of a picture's contrast lost by halving it and
//! doubling it again, its sharpness; an *18% grey card* is a matte card of known reflectance
//! stood in the same light, the photographer's reference for colour and exposure.

use crate::mesh::Material;
use std::collections::BTreeMap;

/// A pattern's targets on the bench. Shares are percentages, as `cargo xtask texels` prints
/// them; values are sRGB, 0 to 255, as a person sees them.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Targets {
    /// The texture its walls, or a roof's covering, are measured on: `Masonry` for a block
    /// material, `Log` for a coursed one, `Covering` for a roof.
    pub surface: Material,
    /// The fewest texels a metre its surface's colour carries, either way along it, at the
    /// high preset (2,048 a side, D362): the industry's 512 to 1,024 for a first-person
    /// close-up.
    pub density: f32,
    /// The least finest octave, as a percentage of its contrast, of its surface seen square
    /// on from 1, 2 and 5 m at noon.
    pub sharpness: [f32; 3],
    /// Its surface's brightness against the grey card stood by it in the same light and the
    /// same frame, in linear light, least and most: lit (noon and evening), and in shade
    /// (noon, the face away from the sun). A wall at 0.2 of the card in shade reads near black
    /// (D373).
    pub lit: [f32; 2],
    pub shade: [f32; 2],
    /// In shade with the light view on (D385), every surface an 18% grey lit as it is: its
    /// brightness against the card's, least and most, the sky's light its occlusion leaves it.
    /// Its red over blue is held to the card's for every material alike (material.ts `SKY_TINT`).
    pub sky: [f32; 2],
    /// The spread of its surface's values at 2 m at noon, its 95th percentile less its 5th,
    /// least and most: too little is flat, too much is noise or a black crush.
    pub values: [f32; 2],
    /// The most of each of its texture sets' area drawn stretched (`cargo xtask texels`).
    pub stretch: Stretch,
}

impl Default for Targets {
    fn default() -> Self {
        Self {
            surface: Material::Masonry,
            density: 0.0,
            sharpness: [0.0; 3],
            lit: [0.0, f32::MAX],
            shade: [0.0, f32::MAX],
            sky: [0.0, f32::MAX],
            values: [0.0, 255.0],
            stretch: Stretch::default(),
        }
    }
}

/// How much of a material's area may be drawn stretched past 1.5:1 (smeared) and past 2.5:1
/// (streaked), as percentages: any texture set's, unless `roles` gives one its own, as a
/// modelled piece's atlas, whose own UVs are measured against each other.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Stretch {
    pub smeared: f32,
    pub streaked: f32,
    pub roles: BTreeMap<Material, [f32; 2]>,
}

impl Default for Stretch {
    fn default() -> Self {
        Self {
            smeared: 100.0,
            streaked: 100.0,
            roles: BTreeMap::new(),
        }
    }
}

impl Stretch {
    /// The most of a texture set's area that may be smeared and streaked.
    pub fn of(&self, role: Material) -> [f32; 2] {
        self.roles
            .get(&role)
            .copied()
            .unwrap_or([self.smeared, self.streaked])
    }
}

/// A photograph of the real material, which the bench shows beside its shots. Kept under
/// `art/`; committed only when it is CC0, otherwise kept on the machine that fetched it
/// (named `*.local.jpg`, which git ignores), fetched again from `source` where it is missing.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reference {
    /// Its path from the repository's root.
    pub photo: String,
    /// A direct link to the file it was fetched from.
    pub source: String,
    /// Its licence, as its source gives it: `CC0`, `CC BY-SA 4.0`.
    pub licence: String,
    /// What it shows, and where.
    pub about: String,
}

impl Reference {
    /// Whether the photograph may be committed: only CC0 and the public domain may.
    pub fn may_commit(&self) -> bool {
        let licence = self.licence.to_ascii_lowercase();
        licence == "cc0" || licence.contains("public domain")
    }

    /// What is wrong with it as data, if anything: a photograph not CC0 must be kept local.
    pub fn problem(&self) -> Option<String> {
        if !self.may_commit() && !self.photo.contains(".local.") {
            return Some(format!(
                "its reference photograph is {}, not CC0, so it is kept local: name it `*.local.jpg`, which git ignores, not {}",
                self.licence, self.photo
            ));
        }
        if !self.source.starts_with("https://") {
            return Some(format!(
                "its reference photograph's source is a direct https link to the file, not `{}`",
                self.source
            ));
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kit::Pattern;

    #[test]
    fn a_role_of_its_own_overrides_the_materials_stretch() {
        let stretch = Stretch {
            smeared: 10.0,
            streaked: 1.0,
            roles: [(Material::Piece, [45.0, 4.0])].into(),
        };
        assert_eq!(stretch.of(Material::Covering), [10.0, 1.0]);
        assert_eq!(stretch.of(Material::Piece), [45.0, 4.0]);
    }

    #[test]
    fn a_photograph_not_cc0_stays_local() {
        let photo = |photo: &str, licence: &str| Reference {
            photo: photo.into(),
            source: "https://upload.wikimedia.org/a.jpg".into(),
            licence: licence.into(),
            about: String::new(),
        };
        assert!(
            photo("art/kits/logs/reference.jpg", "CC0")
                .problem()
                .is_none()
        );
        assert!(
            photo("art/kits/logs/reference.jpg", "CC BY-SA 4.0")
                .problem()
                .is_some()
        );
        assert!(
            photo("art/kits/logs/reference.local.jpg", "CC BY-SA 4.0")
                .problem()
                .is_none()
        );
    }

    #[test]
    fn every_shipped_material_has_targets_on_its_own_surface() {
        // A pattern's targets measured on a texture its materials do not have would measure
        // nothing, and pass.
        for (name, kit) in crate::kit::Kit::read_folder(&crate::kit::tests::assets()).unwrap() {
            let look = &kit.look;
            assert!(
                kit.textures.contains_key(&look.surface),
                "{name}: its pattern's bench measures {:?}, which it has no texture for",
                look.surface
            );
            assert!(look.density > 0.0, "{name}: its pattern sets no density");
            let expected = match kit.pattern {
                Pattern::Masonry => Material::Masonry,
                Pattern::Logs => Material::Log,
                Pattern::Roof => Material::Covering,
            };
            assert_eq!(look.surface, expected, "{name}");
            // A pattern held to a brightness in shade is held to the sky's light too, so a
            // surface that stops taking it fails the bench (D386).
            if look.shade[1] < f32::MAX {
                assert!(
                    look.sky[0] > 0.0 && look.sky[1] < f32::MAX,
                    "{name}: its pattern sets a shade target but no sky target"
                );
            }
            if let Some(reference) = &kit.reference {
                assert_eq!(reference.problem(), None, "{name}");
            }
        }
    }
}
