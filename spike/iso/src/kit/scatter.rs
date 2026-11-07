//! A biome's `scatter` (D634): props and plants strewn by rule, as data. Each set is models
//! (GLBs under `assets/`, one picked a place) or the kit's plant clumps (wood/lawn.rs's
//! cards), tried at its `density` over the layout's span and kept where its mask lies (the
//! ground's own: a layout's map by `Site(name)` or a baked rule, Mud, Wet, Moss, Snow, Grime,
//! Steep), where its clumps gather (noise at two scales, so it is never an even lawn) and,
//! when it says `over`, only in that band over the water's face, fading at the band's ends.
//! Sizes spread evenly in the logarithm (a set from 0.2 to 1 has as many under 0.45 as
//! over), each turned at random and leaned up to `tilt`. The same every run: each set's
//! numbers come from its `seed` and its place in the list through terrain.rs's `hash`.
//!
//! How the industry does it: Unreal's procedural foliage and PCG, Horizon Zero Dawn's GPU
//! placement (Guerrilla, GDC 2017): density maps from the ground's layers times noise,
//! scale and rotation ranges per set. Models are spawned as scenes, the same mesh and
//! material batched by Bevy's automatic instancing; `budget` prints each set's count and
//! triangles once its models load.

use super::biome::{Mask, Plant, PlantKind, Scatter};
use super::ground::Masks;
use crate::forest::{BUTTERCUP, CAMPION, Rng};
use crate::terrain::{Land, fbm, hash, smooth};
use crate::wood::lawn::{Clump, Kind};
use bevy::gltf::{Gltf, GltfMesh};
use bevy::prelude::*;

pub fn plugin(app: &mut App) {
    app.init_resource::<Budget>().add_systems(Update, budget);
}

/// What a layout's ground offers its scatter: its maps' names and values, its height, its
/// water's face, the baked rules, where nothing is strewn (the fight's ground, a path, a
/// prop's foot) and the span (x, z) tried, the frame and a margin.
pub struct Ground<'a> {
    pub sites: &'a [&'a str],
    pub site: &'a dyn Fn(usize, Vec2) -> f32,
    pub height: &'a dyn Fn(Vec2) -> f32,
    pub water: Option<f32>,
    pub rules: Option<&'a Masks>,
    pub keep: &'a dyn Fn(Vec2) -> bool,
    pub span: (Vec2, Vec2),
}

/// One thing strewn: its foot, turn and lean, size, and which of the set's models.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    pub at: Vec3,
    pub rotation: Quat,
    pub scale: f32,
    pub pick: usize,
}

/// A set's mask at `p`, 0 to 1, or why the set cannot be read on this ground (D556).
fn mask(on: Option<&Mask>, g: &Ground, p: Vec2) -> Result<f32, String> {
    let Some(on) = on else { return Ok(1.0) };
    if let Mask::Site(name) = on {
        let k = g.sites.iter().position(|s| s == name).ok_or_else(|| {
            format!(
                "scatter on Site({name:?}): the layout has no such map; its maps are {}. Fix: \
                 name one of those, or a rule (Mud, Wet, Moss, Snow, Grime, Steep; \
                 kit/scatter.rs, D634)",
                g.sites.join(", ")
            )
        })?;
        return Ok((g.site)(k, p));
    }
    if *on == Mask::Base {
        return Ok(1.0);
    }
    g.rules.and_then(|r| r.at(on, p)).ok_or_else(|| {
        format!(
            "scatter on {on:?}: this layout bakes no such rule (Wear is the vertex's, not \
             baked; a street bakes none). Fix: use a Site(...) of its maps ({}) (kit/scatter.rs, \
             D634)",
            g.sites.join(", ")
        )
    })
}

/// A set's own numbers: its seed and its place in the list through the spike's hash.
fn rng(set: &Scatter, index: usize) -> Rng {
    Rng::new((hash(index as i32, set.seed as i32, 0x5ca7) * 16_777_216.0) as u32 + 1)
}

/// How much a set gathers at `p`: 1 everywhere at `clump.2` 0, only in the hearts at 1.
pub fn gathers(set: &Scatter, p: Vec2) -> f32 {
    let (broad, fine, amount) = set.clump;
    let seed = set.seed.wrapping_mul(7).wrapping_add(131);
    let c = 0.6 * fbm(p / broad.max(0.05), seed) + 0.4 * fbm(p / fine.max(0.05), seed + 5);
    (1.0 - amount) + amount * smooth(0.45, 0.62, c)
}

/// Where the set `index` of a biome's scatter stands on `g`, or what is wrong with it and
/// the fix (D556).
pub fn place(set: &Scatter, index: usize, g: &Ground) -> Result<Vec<Placed>, String> {
    let name = set
        .models
        .first()
        .map_or_else(|| format!("{:?}", set.plant), Clone::clone);
    let bad = |why: &str| {
        Err(format!(
            "scatter set {index} ({name}): {why} (kit/scatter.rs, D634)"
        ))
    };
    if set.models.is_empty() == set.plant.is_none() {
        return bad("give it `models: [...]` or a `plant: (...)`, one of the two");
    }
    let (lo, hi) = set.scale;
    if !(lo > 0.0 && lo <= hi) || set.density < 0.0 {
        return bad("its `scale` wants 0 < low <= high and its `density` 0 or more");
    }
    if set.over.is_some() && g.water.is_none() {
        return bad("it stands `over` the water, but this layout has none. Fix: drop `over`");
    }
    mask(set.on.as_ref(), g, g.span.0).map_err(|e| format!("scatter set {index} ({name}): {e}"))?;
    let mut rng = rng(set, index);
    let (a, b) = g.span;
    let tries = ((b - a).x * (b - a).y * set.density).round() as usize;
    let mut out = Vec::new();
    for _ in 0..tries {
        let p = a + Vec2::new(rng.f(), rng.f()) * (b - a);
        // Every number is drawn whether or not the place is kept, so a set's places do
        // not shift when another rule changes.
        let (keep, size, yaw, lean, axis, pick) =
            (rng.f(), rng.f(), rng.f(), rng.f(), rng.f(), rng.f());
        if (g.keep)(p) {
            continue;
        }
        let h = (g.height)(p);
        let mut w = mask(set.on.as_ref(), g, p)? * gathers(set, p);
        if let (Some((from, to)), Some(level)) = (set.over, g.water) {
            let d = h - level;
            if d < from || d > to {
                continue;
            }
            let fade = (0.2 * (to - from)).min(0.04);
            w *= smooth(from, from + fade, d) * smooth(to, to - fade, d);
        }
        if keep >= w {
            continue;
        }
        let scale = lo * (hi / lo).powf(size);
        let tilt = (set.tilt * lean).to_radians();
        let axis = Quat::from_rotation_y(axis * std::f32::consts::TAU) * Vec3::X;
        out.push(Placed {
            at: Vec3::new(p.x, h - set.sink, p.y),
            rotation: Quat::from_axis_angle(axis, tilt)
                * Quat::from_rotation_y(yaw * std::f32::consts::TAU),
            scale,
            pick: ((pick * set.models.len() as f32) as usize).min(set.models.len().max(1) - 1),
        });
    }
    Ok(out)
}

/// What the scatter put down: a line a set, the models whose triangles are counted once
/// they load and the plant sets' cards, counted as `draw_plants` builds them.
#[derive(Resource, Default)]
pub struct Budget {
    pub sets: Vec<Set>,
    /// The first of the plant sets `plants` pushed last, whose cards `draw_plants` counts.
    drawn: usize,
    pub done: bool,
}

/// A set's line: its name, how many it put down, each model with how many of it, and a
/// plant set's cards' triangles (two a card).
pub struct Set {
    pub name: String,
    pub count: usize,
    pub models: Vec<(Handle<Gltf>, usize)>,
    pub cards: usize,
}

impl Budget {
    /// Count the cards of the plant sets `plants` pushed last, from each clump's triangles in
    /// the clumps' order (wood/lawn.rs's `cards`).
    fn count_cards(&mut self, each: &[usize]) {
        let fix = "Fix: draw the clumps `plants` returned, all of them, straight after it \
                   (kit/scatter.rs, D634)";
        let mut rest = each;
        for set in &mut self.sets[self.drawn..] {
            let Some((mine, more)) = rest.split_at_checked(set.count) else {
                panic!(
                    "scatter: {} put down {} clumps but only {} were drawn. {fix}",
                    set.name,
                    set.count,
                    rest.len()
                );
            };
            set.cards = mine.iter().sum();
            rest = more;
        }
        assert!(
            rest.is_empty(),
            "scatter: {} clumps drawn that no plant set put down. {fix}",
            rest.len()
        );
        self.drawn = self.sets.len();
    }

    /// The printout and its total: a line a set, its count and triangles, models' and
    /// cards'; `None` while a model (`tris`) is still loading.
    fn report(&self, tris: impl Fn(&Handle<Gltf>) -> Option<usize>) -> Option<(usize, String)> {
        let mut lines = Vec::new();
        let (mut models, mut cards) = (0, 0);
        for set in &self.sets {
            let mut t = set.cards;
            for (h, n) in &set.models {
                t += tris(h)? * n;
            }
            models += t - set.cards;
            cards += set.cards;
            lines.push(format!(
                "  {:>5} {}: {}k triangles",
                set.count,
                set.name,
                t / 1000
            ));
        }
        let total = models + cards;
        Some((
            total,
            format!(
                "scatter: {}k triangles, {}k of models and {}k of plant cards\n{}",
                total / 1000,
                models / 1000,
                cards / 1000,
                lines.join("\n")
            ),
        ))
    }
}

/// Spawn every model set of `sets` on `g` (plant sets are the layout's to draw, `plants`),
/// recording each in the budget; panics naming the fix on a bad set (D556).
pub fn spawn(
    commands: &mut Commands,
    asset: &AssetServer,
    budget: &mut Budget,
    biome: &str,
    sets: &[Scatter],
    g: &Ground,
) {
    for (i, set) in sets.iter().enumerate() {
        let placed = place(set, i, g).unwrap_or_else(|e| panic!("biome {biome:?}: {e}"));
        if set.plant.is_some() {
            continue;
        }
        let mut counts = vec![0; set.models.len()];
        for p in &placed {
            counts[p.pick] += 1;
            let e = crate::scene::model(commands, asset, &set.models[p.pick], p.at, 0.0, p.scale);
            commands.entity(e).insert(
                Transform::from_translation(p.at)
                    .with_rotation(p.rotation)
                    .with_scale(Vec3::splat(p.scale)),
            );
        }
        let models = set
            .models
            .iter()
            .zip(counts)
            .map(|(m, n)| (asset.load(m.clone()), n));
        budget.sets.push(Set {
            name: set.models.join(" "),
            count: placed.len(),
            models: models.collect(),
            cards: 0,
        });
    }
}

/// The plant sets of `sets` as the kit's clumps (drawn by wood/lawn.rs's `draw`), each
/// tinted between its colours by broad noise and darkened at random.
pub fn plants(budget: &mut Budget, biome: &str, sets: &[Scatter], g: &Ground) -> Vec<Clump> {
    let mut out = Vec::new();
    budget.drawn = budget.sets.len();
    for (i, set) in sets.iter().enumerate() {
        let Some(Plant {
            kind,
            tint,
            flowers,
        }) = &set.plant
        else {
            continue;
        };
        if tint.is_empty() {
            panic!(
                "biome {biome:?}: scatter set {i}'s plant has no `tint`. Fix: give it one or \
                 more colours, `tint: [(0.3, 0.5, 0.15)]` (kit/scatter.rs, D634)"
            );
        }
        let placed = place(set, i, g).unwrap_or_else(|e| panic!("biome {biome:?}: {e}"));
        let mut rng = rng(set, i + 1000);
        let last = tint.len() - 1;
        for p in &placed {
            let at = p.at.xz();
            // Along the palette by broad noise, so neighbours share a colour.
            let hue = smooth(0.3, 0.7, fbm(at * 0.21, set.seed + 73)) * last as f32;
            let k = (hue as usize).min(last.saturating_sub(1));
            let (from, to) = (Vec3::from(tint[k]), Vec3::from(tint[(k + 1).min(last)]));
            let dark = rng.range(0.55, 1.0);
            let flower =
                (rng.f() < *flowers).then(|| if rng.f() < 0.7 { BUTTERCUP } else { CAMPION });
            out.push(Clump {
                at,
                radius: p.scale,
                kind: match kind {
                    PlantKind::Tuft => Kind::Tuft,
                    PlantKind::Fern => Kind::Fern,
                    PlantKind::Leaves => Kind::Leaves,
                    PlantKind::Mat => Kind::Mat,
                },
                tint: from.lerp(to, hue - k as f32) * dark,
                flower,
            });
        }
        budget.sets.push(Set {
            name: format!("{kind:?} clumps"),
            count: placed.len(),
            models: Vec::new(),
            cards: 0,
        });
    }
    out
}

/// The seed the plant clumps' cards are turned by.
const CARDS: u32 = 0x5ca7;

/// Draw the plant clumps `plants` returned on the land (the foliage plugin's materials),
/// counting their cards' triangles in the budget.
#[allow(clippy::too_many_arguments)]
pub fn draw_plants(
    commands: &mut Commands,
    asset: &AssetServer,
    budget: &mut Budget,
    land: &Land,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<crate::foliage::FoliageMaterial>,
    mats: &mut crate::foliage::Mats,
    clumps: &[Clump],
) {
    let each = crate::wood::lawn::draw(
        commands, asset, land, meshes, materials, mats, clumps, CARDS,
    );
    budget.count_cards(&each);
}

/// Print the scatter's budget once every model has loaded: a line a set, its count and
/// triangles, models' and plant cards', and the total.
fn budget(
    mut b: ResMut<Budget>,
    gltfs: Res<Assets<Gltf>>,
    gltf_meshes: Res<Assets<GltfMesh>>,
    meshes: Res<Assets<Mesh>>,
) {
    if b.done || b.sets.is_empty() {
        return;
    }
    let tris = |h: &Handle<Gltf>| -> Option<usize> {
        let g = gltfs.get(h)?;
        let mut n = 0;
        for m in &g.meshes {
            for p in &gltf_meshes.get(m)?.primitives {
                let mesh = meshes.get(&p.mesh)?;
                n += mesh.indices().map_or(mesh.count_vertices(), |i| i.len()) / 3;
            }
        }
        Some(n)
    };
    let Some((_, report)) = b.report(tris) else {
        return;
    };
    println!("{report}");
    b.done = true;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(density: f32, on: Option<Mask>) -> Scatter {
        Scatter {
            models: vec!["models/a.glb".into()],
            plant: None,
            density,
            scale: (1.0, 1.0),
            on,
            clump: (6.0, 1.5, 0.0),
            tilt: 0.0,
            sink: 0.0,
            over: None,
            seed: 3,
        }
    }

    /// A ground 20 m square: one map full east of x 0 and empty west of it, the land rising
    /// 0.1 m a metre east, the water's face at 0.
    fn with(f: impl FnOnce(&Ground)) {
        let site = |_: usize, p: Vec2| if p.x > 0.0 { 1.0 } else { 0.0 };
        let height = |p: Vec2| p.x * 0.1;
        let keep = |_: Vec2| false;
        f(&Ground {
            sites: &["east"],
            site: &site,
            height: &height,
            water: Some(0.0),
            rules: None,
            keep: &keep,
            span: (Vec2::splat(-10.0), Vec2::splat(10.0)),
        });
    }

    /// The count follows the mask: none where it is empty, the density where it is full.
    #[test]
    fn density_follows_the_mask() {
        with(|g| {
            let all = place(&set(2.0, Some(Mask::Site("east".into()))), 0, g).unwrap();
            assert!(all.iter().all(|p| p.at.x > 0.0), "a place west of the mask");
            let want = 2.0 * 200.0;
            let n = all.len() as f32;
            assert!(
                (n - want).abs() < want * 0.12,
                "{n} placed where {want} were wanted"
            );
        });
    }

    /// Sizes spread evenly in the logarithm: the smallest and largest several-fold apart,
    /// half under the geometric mean.
    #[test]
    fn sizes_spread_several_fold() {
        with(|g| {
            let mut s = set(2.0, None);
            s.scale = (0.2, 1.0);
            let all = place(&s, 0, g).unwrap();
            let (lo, hi) = all.iter().fold((9.0f32, 0.0f32), |(a, b), p| {
                (a.min(p.scale), b.max(p.scale))
            });
            assert!(hi / lo > 4.0, "sizes {lo} to {hi}");
            let under = all.iter().filter(|p| p.scale < 0.2f32.sqrt()).count() as f32;
            let half = all.len() as f32 / 2.0;
            assert!(
                (under - half).abs() < half * 0.12,
                "{under} of {} under the mean",
                all.len()
            );
        });
    }

    /// A set `over` the water stands only in its band over the face, none under it.
    #[test]
    fn none_in_the_water_where_forbidden() {
        with(|g| {
            let mut s = set(4.0, None);
            s.over = Some((0.0, 0.3));
            let all = place(&s, 0, g).unwrap();
            assert!(!all.is_empty());
            for p in &all {
                assert!(
                    (0.0..=0.3).contains(&p.at.y),
                    "a place {} over the water",
                    p.at.y
                );
            }
        });
    }

    /// Clumped, the counts in 2 m cells spread far wider than an even strew's (the index of
    /// dispersion, a Poisson strew's 1).
    #[test]
    fn clumps_gather() {
        let dispersion = |amount: f32| {
            let mut d = 0.0;
            with(|g| {
                let mut s = set(3.0, None);
                s.clump = (5.0, 1.2, amount);
                let mut cells = [0.0f32; 100];
                for p in place(&s, 0, g).unwrap() {
                    let c = ((p.at.xz() + 10.0) / 2.0).floor().min(Vec2::splat(9.0));
                    cells[c.y as usize * 10 + c.x as usize] += 1.0;
                }
                let mean = cells.iter().sum::<f32>() / 100.0;
                let var = cells.iter().map(|c| (c - mean).powi(2)).sum::<f32>() / 100.0;
                d = var / mean;
            });
            d
        };
        let (even, gathered) = (dispersion(0.0), dispersion(1.0));
        assert!(even < 1.6, "an even strew's dispersion is {even}");
        assert!(
            gathered > 3.0 * even,
            "a clumped strew's dispersion is {gathered}"
        );
    }

    /// A bad set says what to fix: a map the layout lacks, a rule it does not bake, or
    /// neither models nor a plant.
    #[test]
    fn a_bad_set_names_the_fix() {
        with(|g| {
            let e = place(&set(1.0, Some(Mask::Site("moss".into()))), 0, g).unwrap_err();
            assert!(e.contains("its maps are east"), "{e}");
            let e = place(&set(1.0, Some(Mask::Mud)), 0, g).unwrap_err();
            assert!(e.contains("Fix"), "{e}");
            let mut s = set(1.0, None);
            s.models.clear();
            assert!(place(&s, 0, g).unwrap_err().contains("one of the two"));
        });
    }

    /// Every model a biome strews is in assets/.
    #[test]
    fn every_biomes_models_exist() {
        let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        for name in ["forest", "marsh", "ruin", "neon-street"] {
            let b = super::super::biome::Biome::load(name).unwrap();
            for s in &b.scatter {
                for m in &s.models {
                    assert!(assets.join(m).exists(), "{name}: {m} is not in assets/");
                }
            }
        }
    }

    /// The budget's total is the set's real count: the models' triangles and every plant
    /// card drawn, two a card, each plant set its own clumps' cards.
    #[test]
    fn the_budget_counts_the_plant_cards() {
        let land = Land::new();
        // Two triangles a card, from the mesh's corners (four a card), not its indices.
        let quads = |c: &[Clump]| {
            let (leaves, stems, _) = crate::wood::lawn::cards(&land, c, CARDS);
            (leaves.mesh().count_vertices() + stems.mesh().count_vertices()) / 4 * 2
        };
        with(|g| {
            let plant = |kind| {
                let mut s = set(1.5, None);
                s.models.clear();
                s.plant = Some(Plant {
                    kind,
                    tint: vec![(0.3, 0.5, 0.15)],
                    flowers: 0.3,
                });
                s
            };
            let mut b = Budget::default();
            b.sets.push(Set {
                name: "a".into(),
                count: 3,
                models: vec![(Handle::default(), 3)],
                cards: 0,
            });
            let sets = [plant(PlantKind::Mat), plant(PlantKind::Leaves)];
            let clumps = plants(&mut b, "test", &sets, g);
            let (_, _, each) = crate::wood::lawn::cards(&land, &clumps, CARDS);
            b.count_cards(&each);
            let (total, report) = b.report(|_| Some(100)).unwrap();
            let first = b.sets[1].count;
            assert!(first > 0 && b.sets[2].count > 0, "{report}");
            assert_eq!(b.sets[1].cards, quads(&clumps[..first]), "{report}");
            assert_eq!(total, 300 + quads(&clumps), "{report}");
            assert!(report.contains("of plant cards"), "{report}");
        });
    }
}
