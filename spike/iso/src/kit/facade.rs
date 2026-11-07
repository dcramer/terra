//! A street front's face as bays of a modular facade kit (D634): each module a scene of one
//! GLB (`assets/models/iso/city/facade-apartments.glb`, Poly Haven's scanned apartments
//! facade, CC0: a bay `width` wide and a `storey` tall, facing +z, its foot at y 0, centred
//! across). The front's span is cut into the nearest whole number of bays, each stretched
//! along to fill it (a stretch of 0.85 to 1.15, as a modular city's kit tolerates before
//! its windows read squashed); the street storey takes its `ground` modules in turn, every
//! column above keeps one of `upper` up the building, a `top` (the crown's parapet) caps
//! each column and a `pier` stands at each end. Behind the openings the block's face is set
//! back `back` metres, near black; a lit card on it behind a bay shows through the glass as
//! a room with its light on, `shops` of the street storey's and `lit` of those above.
//! How shipped games build streets so: modular kits on a grid (Bethesda's Fallout 4 kits,
//! Joel Burgess's GDC 2013 talk "Skyrim's Modular Approach to Level Design").

use super::light::Colour;
use super::street::Side;
use crate::forest::Rng;
use bevy::prelude::*;
use serde::Deserialize;

/// A front's bays: the kit's GLB (under `assets/`), its scenes by index for the street
/// storey (`ground`, taken in turn along), the storeys above (`upper`, one a column), the
/// crown (`top`) and the end piers (`pier`); a bay's `width` and `storey` (m); how far the
/// block's face stands `back` behind the modules' face; `lit` of the upper bays and `shops`
/// (share, nits) of the street storey's glowing in one of `colours` at `nits`.
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Bays {
    pub model: String,
    pub ground: Vec<usize>,
    pub upper: Vec<usize>,
    pub top: usize,
    pub pier: usize,
    #[serde(default = "three")]
    pub width: f32,
    #[serde(default = "three")]
    pub storey: f32,
    #[serde(default = "set_back")]
    pub back: f32,
    #[serde(default)]
    pub lit: f32,
    #[serde(default)]
    pub shops: (f32, f32),
    #[serde(default)]
    pub colours: Vec<Colour>,
    #[serde(default)]
    pub nits: f32,
    /// The front's plaster painted: the GLB's materials named with `plaster` tinted so; none,
    /// as they come. Each front its own, so a street reads as several buildings.
    #[serde(default)]
    pub paint: Option<Colour>,
    /// How far grime darkens and browns a column of bays at most (0 to 1), each column its
    /// own share of it by a hash of the front and the column, the street storey's half again.
    #[serde(default)]
    pub grime: f32,
    #[serde(default = "plaster")]
    pub plaster: String,
    /// Rooms behind the glass, `back` deep (kit/rooms.rs): each bay's floored and walled,
    /// lit, dark, screen-lit, blinded, curtained or shuttered by a hash; none, a flat lit
    /// card on the set-back face behind each lit bay.
    #[serde(default)]
    pub rooms: Option<super::rooms::Rooms>,
}

fn plaster() -> String {
    "plaster".into()
}

/// A module whose plaster is to be painted: its materials' name and the tint, its meshes'
/// materials so named swapped for tinted copies once its scene is in (traps.md: a module's
/// material matched and replaced; Bevy 0.20 labels a GLB's materials `GltfMaterial`, so
/// they are matched by the meshes' `GltfMaterialName`).
#[derive(Component)]
pub struct Paint {
    plaster: String,
    tint: LinearRgba,
}

pub fn plugin(app: &mut App) {
    app.add_systems(Update, paint);
}

/// Grime's colour, multiplied in: a sooty brown.
const GRIME: (f32, f32, f32) = (0.42, 0.37, 0.3);
/// Grime comes in this many steps, so a front's bays share a few materials.
const STEPS: f32 = 4.0;

impl Bays {
    /// The tint of the plaster in column `column`, storey `storey` of the front over
    /// `along`: the paint, weathered by the column's share of the grime (a hash of the
    /// front's start and the column, in `STEPS`), the street storey's half again.
    pub fn tint(&self, along: (f32, f32), column: usize, storey: usize) -> Option<LinearRgba> {
        let paint = self.paint?.color().to_linear();
        let h = crate::terrain::hash(column as i32, along.0.to_bits() as i32, 0x9a1e);
        let share = (0.25 + 0.75 * ((h * STEPS).floor() + 1.0) / STEPS).min(1.0);
        let g = (self.grime * share * if storey == 0 { 1.5 } else { 1.0 }).clamp(0.0, 1.0);
        let k = |p: f32, d: f32| p * (1.0 - g + g * d);
        Some(LinearRgba::rgb(
            k(paint.red, GRIME.0),
            k(paint.green, GRIME.1),
            k(paint.blue, GRIME.2),
        ))
    }
}

/// The tinted copies made: one a source material and tint.
type Made =
    std::collections::HashMap<(AssetId<StandardMaterial>, [u32; 4]), Handle<StandardMaterial>>;

/// Each painted module's plaster swapped for its tinted copy, one copy a tint.
fn paint(
    mut commands: Commands,
    roots: Query<(Entity, &Paint)>,
    children: Query<&Children>,
    mut meshes: Query<(
        &mut MeshMaterial3d<StandardMaterial>,
        Option<&bevy::gltf::GltfMaterialName>,
    )>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut made: Local<Made>,
) {
    for (root, p) in &roots {
        let mut seen = false;
        for e in children.iter_descendants(root) {
            let Ok((mut m, name)) = meshes.get_mut(e) else {
                continue;
            };
            seen = true;
            if !name.is_some_and(|n| n.contains(p.plaster.as_str())) {
                continue;
            }
            let key = (m.0.id(), p.tint.to_f32_array().map(f32::to_bits));
            let h = match made.get(&key) {
                Some(h) => h.clone(),
                None => {
                    let Some(src) = materials.get(&m.0) else {
                        seen = false;
                        break;
                    };
                    let mut c = src.clone();
                    let b = c.base_color.to_linear();
                    c.base_color = Color::linear_rgba(
                        b.red * p.tint.red,
                        b.green * p.tint.green,
                        b.blue * p.tint.blue,
                        b.alpha,
                    );
                    let h = materials.add(c);
                    made.insert(key, h.clone());
                    h
                }
            };
            m.0 = h;
        }
        if seen {
            commands.entity(root).remove::<Paint>();
        }
    }
}

fn three() -> f32 {
    3.0
}

fn set_back() -> f32 {
    0.4
}

/// The stretch along a module may take before its windows read squashed or spread.
pub const STRETCH: (f32, f32) = (0.85, 1.15);

/// The bays across a span `along` of bays `width` wide: each one's middle and its stretch
/// along, the nearest whole number of them meeting edge to edge from end to end.
pub fn columns(along: (f32, f32), width: f32) -> Vec<(f32, f32)> {
    let span = along.1 - along.0;
    let n = (span / width).round().max(1.0) as usize;
    let w = span / n as f32;
    (0..n)
        .map(|i| (along.0 + (i as f32 + 0.5) * w, w / width))
        .collect()
}

impl Bays {
    /// The bays as laid out on a front `along` and `height` tall, or what is wrong and the
    /// fix (D556): some modules for each storey, the front a whole number of storeys, the
    /// stretch inside `STRETCH`.
    pub fn check(&self, along: (f32, f32), height: f32) -> Result<(), String> {
        if self.ground.is_empty() || self.upper.is_empty() {
            return Err(format!(
                "the front's bays from x {} to {} name no `ground` or no `upper` modules. Fix: \
                 at least one scene index in each (kit/facade.rs, D634)",
                along.0, along.1
            ));
        }
        let storeys = height / self.storey;
        if (storeys - storeys.round()).abs() > 0.01 || storeys.round() < 1.0 {
            return Err(format!(
                "the front from x {} to {} is {height} m tall, not a whole number of its bays' \
                 {} m storeys: the top storey's modules would stand over the roof or short of \
                 it. Fix: its `height` to {} (kit/facade.rs, D634)",
                along.0,
                along.1,
                self.storey,
                storeys.round().max(1.0) * self.storey
            ));
        }
        let room = super::rooms::FRONT + 1.0;
        if self.rooms.is_some() && self.back < room {
            return Err(format!(
                "the front from x {} to {} has rooms behind its glass but its `back` is {} m: \
                 a room starts {} m behind the modules' face, clear of their reveals, and \
                 under a metre deep shows no floor. Fix: its `back` to {room} or more, near \
                 2 for a room seen from above (kit/rooms.rs, D634)",
                along.0,
                along.1,
                self.back,
                super::rooms::FRONT
            ));
        }
        let k = columns(along, self.width)[0].1;
        if !(STRETCH.0..=STRETCH.1).contains(&k) {
            return Err(format!(
                "the front from x {} to {} ({} m) takes its {} m bays stretched {k:.2} along, \
                 outside {:?}: its windows would read squashed or spread. Fix: its `along` to \
                 a span near a whole number of bays (kit/facade.rs, D634)",
                along.0,
                along.1,
                along.1 - along.0,
                self.width,
                STRETCH
            ));
        }
        Ok(())
    }
}

/// A front's bays on `side`, its face on the wall line `wall` (m from the road's middle),
/// `height` tall: the modules, the crowns and piers, and the lit cards behind the openings.
#[allow(clippy::too_many_arguments)]
pub fn spawn(
    commands: &mut Commands,
    asset: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    side: Side,
    along: (f32, f32),
    height: f32,
    wall: f32,
    b: &Bays,
    rng: &mut Rng,
) {
    let s = side.z();
    let turn = Quat::from_rotation_y(side.facing());
    let scene = |i: usize| asset.load(GltfAssetLabel::Scene(i).from_asset(b.model.clone()));
    // A module, its plaster painted as its column and storey weather (`Bays::tint`); the
    // end piers the paint itself.
    let mut module = |i: usize, at: Vec3, k: f32, tint: Option<LinearRgba>| {
        let mut e = commands.spawn((
            WorldAssetRoot(scene(i)),
            Transform::from_translation(at)
                .with_rotation(turn)
                .with_scale(Vec3::new(k, 1.0, 1.0)),
        ));
        if let Some(tint) = tint {
            e.insert(Paint {
                plaster: b.plaster.clone(),
                tint,
            });
        }
    };
    let storeys = (height / b.storey).round().max(1.0) as usize;
    let face = s * wall;
    let cols = columns(along, b.width);
    for (c, &(x, k)) in cols.iter().enumerate() {
        for st in 0..storeys {
            let i = if st == 0 {
                b.ground[c % b.ground.len()]
            } else {
                b.upper[c % b.upper.len()]
            };
            let tint = b.tint(along, c, st);
            module(i, Vec3::new(x, st as f32 * b.storey, face), k, tint);
        }
        let tint = b.tint(along, c, storeys);
        module(
            b.top,
            Vec3::new(x, storeys as f32 * b.storey, face),
            k,
            tint,
        );
    }
    let clean = b.paint.map(|p| p.color().to_linear());
    for x in [along.0, along.1] {
        for st in 0..storeys {
            module(b.pier, Vec3::new(x, st as f32 * b.storey, face), 1.0, clean);
        }
    }
    if let Some(r) = &b.rooms {
        super::rooms::spawn(
            commands, meshes, materials, side, along, &cols, storeys, wall, b, r,
        );
        return;
    }
    if b.colours.is_empty() {
        return;
    }
    // A room's light seen through the glass: a card on the set-back face, a little in front
    // of it, its brightness varied a room to the next (0.6 to 1.2 of its storey's nits).
    let card = meshes.add(Rectangle::new(1.0, 1.0));
    let z = s * (wall + b.back - 0.02);
    for &(x, k) in &cols {
        for st in 0..storeys {
            let (share, nits) = if st == 0 { b.shops } else { (b.lit, b.nits) };
            let (on, pick, dim) = (rng.f(), rng.f(), rng.f());
            if on >= share {
                continue;
            }
            let c = b.colours[(pick * b.colours.len() as f32) as usize % b.colours.len()];
            let m = materials.add(StandardMaterial {
                base_color: Color::BLACK,
                emissive: c.color().to_linear() * nits * (0.6 + 0.6 * dim),
                perceptual_roughness: 0.6,
                ..default()
            });
            commands.spawn((
                Mesh3d(card.clone()),
                MeshMaterial3d(m),
                Transform::from_xyz(x, (st as f32 + 0.5) * b.storey, z)
                    .with_rotation(turn)
                    .with_scale(Vec3::new(b.width * k * 0.96, b.storey * 0.92, 1.0)),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kit::biome::Biome;

    fn bays(width: f32) -> Bays {
        Bays {
            model: "m.glb".into(),
            ground: vec![0],
            upper: vec![4],
            top: 8,
            pier: 9,
            width,
            storey: 3.0,
            back: 0.4,
            lit: 0.0,
            shops: (0.0, 0.0),
            colours: vec![],
            nits: 0.0,
            paint: None,
            grime: 0.0,
            plaster: "plaster".into(),
            rooms: None,
        }
    }

    /// A painted front weathers a column of bays apart from the next by its grime alone, the
    /// same each run, its street storey dirtier; with no grime every bay is its paint.
    #[test]
    fn columns_weather_apart_and_repeat() {
        let mut b = bays(3.0);
        b.paint = Some(Colour::Srgb(0.9, 0.8, 0.6));
        let paint = Colour::Srgb(0.9, 0.8, 0.6).color().to_linear();
        assert_eq!(b.tint((-9.0, 6.0), 3, 2), Some(paint));
        b.grime = 0.4;
        let col: Vec<_> = (0..10)
            .map(|c| b.tint((-9.0, 6.0), c, 2).unwrap())
            .collect();
        let again: Vec<_> = (0..10)
            .map(|c| b.tint((-9.0, 6.0), c, 2).unwrap())
            .collect();
        assert_eq!(col, again);
        let mut kinds = col.iter().map(|c| c.red.to_bits()).collect::<Vec<_>>();
        kinds.sort();
        kinds.dedup();
        assert!(kinds.len() >= 3, "ten columns took {} grimes", kinds.len());
        for (c, t) in col.iter().enumerate() {
            assert!(t.red < paint.red && t.blue < paint.blue);
            let low = b.tint((-9.0, 6.0), c, 0).unwrap();
            assert!(low.red < t.red, "column {c}'s street storey is no dirtier");
        }
        b.paint = None;
        assert_eq!(b.tint((-9.0, 6.0), 3, 2), None);
    }

    /// Bays fill a front's span from end to end with no gap or overlap, each stretched
    /// alike: their edges, summed widths and ends known from the span alone.
    #[test]
    fn bays_fill_the_span_with_no_gap() {
        for i in 0..400 {
            let x0 = -40.0 + i as f32 * 0.37;
            let span = 2.6 + i as f32 * 0.11;
            let cols = columns((x0, x0 + span), 3.0);
            let w = 3.0 * cols[0].1;
            assert!(
                (cols[0].0 - w / 2.0 - x0).abs() < 1e-3,
                "first bay off the start"
            );
            let last = cols[cols.len() - 1].0 + w / 2.0;
            assert!((last - (x0 + span)).abs() < 1e-3, "last bay off the end");
            for p in cols.windows(2) {
                assert!(((p[1].0 - p[0].0) - w).abs() < 1e-3, "a gap between bays");
                assert_eq!(p[0].1, p[1].1, "bays stretched unlike");
            }
            assert!((cols.len() as f32 * w - span).abs() < 1e-2);
        }
    }

    /// Every street biome's fronts with bays pass the check: the stretch inside 0.85 to
    /// 1.15, a whole number of storeys.
    #[test]
    fn the_streets_bays_fit() {
        let st = Biome::load("neon-street").unwrap().street.unwrap();
        let mut some = 0;
        for f in &st.fronts {
            let Some(b) = &f.bays else { continue };
            b.check(f.along, f.height).unwrap_or_else(|e| panic!("{e}"));
            let k = columns(f.along, b.width)[0].1;
            assert!((STRETCH.0..=STRETCH.1).contains(&k), "stretch {k}");
            some += 1;
        }
        assert!(some > 0, "the neon street has no front with bays");
    }

    /// A front too short for its bays, or not a whole number of storeys, is refused with
    /// the fix.
    #[test]
    fn a_squashed_or_broken_front_says_the_fix() {
        let e = bays(3.0).check((0.0, 4.0), 9.0).unwrap_err();
        assert!(e.contains("stretched") && e.contains("Fix"), "{e}");
        let e = bays(3.0).check((0.0, 12.0), 16.0).unwrap_err();
        assert!(e.contains("to 15") && e.contains("Fix"), "{e}");
        assert!(bays(3.0).check((0.0, 12.0), 15.0).is_ok());
        let mut b = bays(3.0);
        b.rooms = Some(crate::kit::parse("(tv: 0.1)", "test", "rooms.rs").unwrap());
        let e = b.check((0.0, 12.0), 15.0).unwrap_err();
        assert!(e.contains("`back` to") && e.contains("Fix"), "{e}");
        b.back = 2.0;
        assert!(b.check((0.0, 12.0), 15.0).is_ok());
    }
}
