//! The undergrowth's variants, grown once for the app and drawn in each world's materials.
//!
//! Every kind's variants grow from the forest's own seed and the shared cards, so they are
//! the same in every world the forest is planted in; only their materials (the world's wind
//! and bark) and their shadows' filing differ. So their meshes are grown on the first entry
//! and kept for the app, as the tree kinds are (`kinds::Grown`): grown again on each entry,
//! the middle layer's shrubs alone were 0.8 to 2.2 s of the crossing's longest frame, and
//! every crossing gave the GPU new slabs of the same meshes (instances j10).

use super::{
    BRAMBLE_TINT, Band, Drawn, HOLLY_TINT, Lay, Look, Part, THICKET_TINT, Under, VARIANTS, bracken,
    branches, ensure_tangents, twigs, variant_tint,
};
use crate::experiments::lush::floor::placements::Placement;
use crate::experiments::lush::shade::ShadowSet;
use crate::experiments::lush::thicket::{self, Stool};
use crate::experiments::lush::warmed;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use terra_gen::Rng;
use terra_world::grown::{build as grown_build, plants};

/// Each kind's variants, by kind, without the world's materials; a kind is grown the first
/// time the forest is planted.
#[derive(Resource, Default)]
pub(in crate::experiments::lush) struct Variants(HashMap<u64, Vec<Bare>>);

/// Which of the world's materials a part is drawn in.
#[derive(Clone, Copy)]
enum Paint {
    /// The floor's cards.
    Atlas,
    /// The middle layer's cards.
    Thicket,
    Bark,
    PineBark,
    SpruceBark,
}

/// A variant's meshes, as `Drawn` without its materials.
#[derive(Clone, Default)]
pub(super) struct Bare {
    parts: Vec<(Handle<Mesh>, Paint, Band, bool)>,
    /// Its shadow stand-ins, and whether each is cut to the leaves' outline.
    shadows: Vec<(Handle<Mesh>, bool)>,
}

impl Bare {
    fn one(mesh: Handle<Mesh>, paint: Paint) -> Self {
        Self {
            parts: vec![(mesh, paint, Band::All, false)],
            shadows: Vec::new(),
        }
    }
}

impl Lay<'_, '_, '_> {
    /// Each kind's variants in this world's materials, grown first if the app has not yet.
    pub(super) fn grow(&mut self, placement: &Placement) -> Vec<Drawn> {
        if placement.kind == Under::Fern {
            // The scan is loaded, and drawn under this world's season.
            let (mesh, material) = self.plant_scan("fern", Color::srgb(0.6, 0.64, 0.58));
            return vec![Drawn::one(mesh, Look::Scan(material))];
        }
        let key = placement.kind as u64;
        let bare = match self.variants.0.get(&key) {
            Some(bare) => bare.clone(),
            None => {
                let bare = self.grow_bare(placement.kind);
                self.variants.0.insert(key, bare.clone());
                bare
            }
        };
        bare.iter()
            .map(|bare| self.dress(bare, placement.shadow_m))
            .collect()
    }

    /// A variant in this world's materials, its shadows filed for the camera to bring near.
    fn dress(&mut self, bare: &Bare, shadow_m: f32) -> Drawn {
        let mut drawn = Drawn::default();
        for (mesh, paint, band, solid) in &bare.parts {
            let material = match paint {
                Paint::Atlas => &self.atlas,
                Paint::Thicket => &self.thicket,
                Paint::Bark => &self.bark,
                Paint::PineBark => &self.pine_bark,
                Paint::SpruceBark => &self.spruce_bark,
            };
            drawn.parts.push(Part {
                mesh: mesh.clone(),
                look: Look::Foliage(material.clone()),
                band: *band,
                solid: *solid,
            });
        }
        let shadows: Vec<_> = bare
            .shadows
            .iter()
            .map(|(mesh, leafy)| {
                let atlas = self.atlas.clone();
                let material = self.shrinking_material(leafy.then_some(&atlas), shadow_m);
                (mesh.clone(), material)
            })
            .collect();
        if !shadows.is_empty() {
            drawn.shadow_set = Some(self.shade.add_set(ShadowSet {
                parts: shadows,
                end_m: shadow_m,
            }));
        }
        drawn
    }

    /// A plant of the middle layer's meshes: what stands first is what the plant's own entity
    /// draws, its leaves near or a snag's trunk, so each plant is as few entities as it can
    /// be; then its wood, its leaves farther off, and its shadow.
    fn layered(&mut self, shrub: thicket::Shrub, bark: Paint) -> Bare {
        let mut bare = Bare::default();
        let [wood_near, wood_far] = shrub.wood;
        let leafy = shrub.leaves.is_some();
        let wood = |lay: &mut Self, build: grown_build::Build, band: Band, solid: bool| {
            if build.indices.is_empty() {
                return None;
            }
            let mut mesh = build.mesh(true);
            ensure_tangents(&mut mesh);
            if solid {
                // Kept on the CPU too, for its footing to be fitted to (D267).
                mesh.asset_usage = bevy::asset::RenderAssetUsages::all();
            }
            Some((lay.meshes.add(mesh), bark, band, solid))
        };
        if let Some(trunk) = shrub.trunk {
            bare.parts.extend(wood(self, trunk, Band::All, true));
        }
        if let Some([near, far]) = shrub.leaves {
            for (build, band) in [(near, Band::Near), (far, Band::Far)] {
                let mesh = self.meshes.add(build.mesh(false));
                bare.parts.push((mesh, Paint::Thicket, band, false));
            }
        }
        // A leafy plant's stems are thin, and drawn at one level; a dead one's branches are
        // all there is of it, and have two.
        if leafy {
            bare.parts.extend(wood(self, wood_near, Band::All, false));
        } else {
            bare.parts.extend(wood(self, wood_near, Band::Near, false));
            bare.parts.extend(wood(self, wood_far, Band::Far, false));
        }
        if let Some(shadow) = shrub.shadow {
            let mesh = self.meshes.add(thicket::shrinking(&shadow.mesh(false)));
            bare.shadows.push((mesh, leafy));
        }
        bare
    }

    /// A variant of the floor's rosettes, the fronds' or the herbs'.
    fn rosette(&mut self, kind: Under, n: u64, dice: &mut Rng) -> Bare {
        let (picture, tint, size) = if kind == Under::Frond {
            // Some young and small, some old and broad.
            let size = [1.0, 0.8, 1.25, 0.65, 1.1, 0.9][n as usize];
            ("fern", Vec3::new(0.54, 0.6, 0.52), size)
        } else {
            // A herb is a third of a metre across, not a metre.
            ("herb", Vec3::new(0.6, 0.62, 0.54), 0.4)
        };
        let tint = variant_tint(dice, tint);
        // Lady fern's shuttlecocks, broad and arching, and male fern's, upright and narrow,
        // as two kinds of each.
        let upright = n % 2 == 1;
        let def = if kind == Under::Frond {
            terra_world::grown::RosetteDef {
                card: "fern".into(),
                tint: (1.0, 1.0, 1.0),
                leaves: if upright { (6, 9) } else { (9, 15) },
                tilt: if upright { (55.0, 78.0) } else { (25.0, 60.0) },
                stalk: (0.03, 0.97),
                tip: (0.97, 0.03),
            }
        } else {
            terra_world::grown::RosetteDef {
                card: "herb".into(),
                tint: (1.0, 1.0, 1.0),
                leaves: (4, 7),
                tilt: (15.0, 45.0),
                stalk: (0.5, 0.97),
                tip: (0.5, 0.05),
            }
        };
        let mut build = plants::rosette(dice, &def, self.cards.get(picture), tint);
        for p in &mut build.positions {
            for c in p.iter_mut() {
                *c *= size;
            }
        }
        Bare::one(self.meshes.add(build.mesh(false)), Paint::Atlas)
    }

    /// A kind's variants grown from the forest's seed (the fern's scan apart).
    fn grow_bare(&mut self, kind: Under) -> Vec<Bare> {
        let mut dice = self
            .seed
            .derive("undergrowth kinds")
            .index(kind as u64)
            .rng();
        let cards = self.cards;
        let card = |name: &str| cards.get(name).clone();
        let mut out = Vec::new();
        for n in 0..VARIANTS {
            let atlas = Paint::Atlas;
            let bare = match kind {
                Under::Bracken | Under::Verge => {
                    let tint = variant_tint(&mut dice, Vec3::new(0.62, 0.64, 0.52));
                    let build = bracken(&mut dice, &card("fern"), tint);
                    Bare::one(self.meshes.add(build.mesh(false)), atlas)
                }
                Under::Fern => unreachable!("the fern is its scan, loaded in each world"),
                Under::Frond | Under::Herb => self.rosette(kind, n, &mut dice),
                Under::Hazel => {
                    // Young stools on the floor, a metre or two: rods, a vase, a fan, each
                    // with its stems showing, at the size their leaves are (D276).
                    let tint = variant_tint(&mut dice, THICKET_TINT);
                    let stool = [Stool::Rods, Stool::Vase, Stool::Fan][n as usize % 3];
                    let tallest = dice.range(1.3, 2.3);
                    let shrub = thicket::coppice(&mut dice, &card("hazel"), tint, stool, tallest);
                    self.layered(shrub, Paint::Bark)
                }
                Under::Bilberry => {
                    let tint = variant_tint(&mut dice, Vec3::new(0.66, 0.7, 0.62));
                    let def = terra_world::grown::BushDef {
                        leaves: "berry".into(),
                        tint: (1.0, 1.0, 1.0),
                        clumps: (6, 9),
                        spread: 1.7,
                    };
                    let build = plants::bush(&mut dice, &def, &card("berry"), tint);
                    // Shaded as the middle layer's leaves, not greyed as the floor's pictures
                    // are: its card is composed from scanned leaves as theirs are (D359).
                    Bare::one(self.meshes.add(build.mesh(false)), Paint::Thicket)
                }
                Under::Scrub => {
                    // Young oak scrub, darker than the oak spray's own lime.
                    let tint = variant_tint(&mut dice, THICKET_TINT);
                    let shrub = thicket::scrub(&mut dice, &card("oak"), tint);
                    self.layered(shrub, Paint::Bark)
                }
                Under::Sapling => {
                    // A young tree, warmed as the grown ones' crowns are (D598).
                    let tint = variant_tint(&mut dice, warmed(THICKET_TINT));
                    let leaves = if n % 2 == 0 { "oak" } else { "hazel" };
                    let shrub = thicket::sapling(&mut dice, &card(leaves), tint);
                    self.layered(shrub, Paint::Bark)
                }
                Under::Branches => {
                    let mut build = branches(&mut dice);
                    let mut mesh = std::mem::take(&mut build).mesh();
                    mesh.generate_tangents()
                        .expect("sticks have normals and texture places");
                    Bare::one(self.meshes.add(mesh), Paint::Bark)
                }
                Under::Twigs => {
                    let mut mesh = twigs(&mut dice).mesh();
                    mesh.generate_tangents()
                        .expect("twigs have normals and texture places");
                    Bare::one(self.meshes.add(mesh), Paint::Bark)
                }
                Under::Flowers => {
                    let tint = variant_tint(&mut dice, Vec3::new(0.9, 0.9, 0.86));
                    let name = ["daisy", "buttercup", "campion"][n as usize % 3];
                    let build = plants::tuft(&mut dice, 2 + (n % 2) as u32, &card(name), tint);
                    Bare::one(self.meshes.add(build.mesh(false)), atlas)
                }
                Under::Coppice => {
                    let tint = variant_tint(&mut dice, THICKET_TINT);
                    let stool =
                        [Stool::Vase, Stool::Fan, Stool::Rods, Stool::Overstood][n as usize % 4];
                    let tallest = match stool {
                        Stool::Vase => dice.range(2.8, 4.6),
                        Stool::Fan => dice.range(2.6, 4.2),
                        Stool::Rods => dice.range(2.0, 3.2),
                        Stool::Overstood => dice.range(4.0, 5.4),
                    };
                    let shrub = thicket::coppice(&mut dice, &card("hazel"), tint, stool, tallest);
                    self.layered(shrub, Paint::Bark)
                }
                Under::Holly => {
                    // Dark, glossy, nearly black-green in the shade.
                    let tint = variant_tint(&mut dice, HOLLY_TINT);
                    let shrub = thicket::holly(&mut dice, &card("holly"), tint);
                    self.layered(shrub, Paint::Bark)
                }
                Under::Spruce => {
                    let tint = variant_tint(&mut dice, warmed(Vec3::new(1.6, 1.75, 0.7)));
                    let height = dice.range(1.4, 4.4);
                    let shrub = thicket::evergreen(&mut dice, &card("spruce"), tint, height, 0.5);
                    self.layered(shrub, Paint::SpruceBark)
                }
                Under::Bramble => {
                    let tint = variant_tint(&mut dice, BRAMBLE_TINT);
                    let shrub = thicket::bramble(&mut dice, &card("bramble"), tint);
                    self.layered(shrub, Paint::Bark)
                }
                Under::Snag => {
                    // Weathered silver-grey, the bark half gone.
                    let conifer = n % 2 == 0;
                    let grey = dice.range(0.82, 1.0);
                    let shrub =
                        thicket::snag(&mut dice, conifer, Vec3::new(1.0, 0.97, 0.92) * grey);
                    let bark = if conifer {
                        Paint::PineBark
                    } else {
                        Paint::Bark
                    };
                    self.layered(shrub, bark)
                }
            };
            out.push(bare);
        }
        out
    }
}
