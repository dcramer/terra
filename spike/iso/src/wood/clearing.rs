//! The fight's ground (judge forest-r4: "even grass round the fight", graded like dusk): a
//! trodden clearing of earth under the cast (`trodden`, which terrain::bare reads, so the
//! ground's earth and the grass's gap agree), the scanned leaf litter on it and thickest at
//! its rim, small mossy stones in it and ferns about it, stones breaking the pond's
//! waterline; and a sun break over it, as an ARPG's art direction opens one on its fight:
//! the oaks whose crowns' shade falls on the fight (dress::OAKS[0]'s, 4 to 12 m down the
//! sun from its trunk, lay across it at r4) cast none, so it stands in sun among the others'.

use super::{FOE, KNIGHT, SECOND, TARGET};
use crate::forest::{Crown, Rng};
use crate::terrain::{self, Land, fbm, smooth};
use bevy::asset::RenderAssetUsages;
use bevy::image::ImageLoaderSettings;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

/// The litter atlas's leaf cells (4 by 4, materials/litter/README order): oak leaves,
/// mouldering oak, beech, twigs.
const LEAVES: [u32; 4] = [0, 1, 2, 7];
/// The clearing's middle, between the second creature and the foe, its half lengths along
/// the fight's line and across it (m).
const MIDDLE: Vec2 = Vec2::new(3.1, 5.35);
const ALONG: f32 = 5.5;
const ACROSS: f32 = 3.0;
/// The oak whose crown shades the fight by day, by its trunk (dress::OAKS[0]).
const SHADE_OAK: Vec2 = Vec2::new(0.3, 11.6);
/// How near the fight's middle a crown's shade may fall before it is cast out (m).
const BREAK: f32 = 3.5;
/// Litter cards tried a square metre of the clearing's box, small stones and ferns in all,
/// and stones at the pond's waterline.
const LEAF_TRIES: f32 = 14.0;
/// A litter card's side (m): an oak leaf is about 0.45 of an atlas cell, so 0.18 to 0.3 m
/// lays leaves 8 to 13 cm long, a beech's 4 to 6 (forest judge 261106-2239: at 0.3 to
/// 0.6 they lay the size of the hero's head).
const LEAF_CARD: (f32, f32) = (0.18, 0.3);
const STONES: usize = 12;
const FERNS: usize = 10;
const SHORE_STONES: usize = 7;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn.after(crate::scene::build))
        .add_systems(PostStartup, sun_break);
}

/// How trodden the fight's ground is (0..1): an oval along the fight's line, its edge ragged
/// at two sizes so it is no disc, patchy in its middle where a few tufts hold.
pub fn trodden(p: Vec2) -> f32 {
    let a = (FOE - SECOND).normalize();
    let d = p - MIDDLE;
    let e = Vec2::new(d.dot(a) / ALONG, d.dot(a.perp()) / ACROSS).length();
    let wobble = fbm(p * 0.6, 131) - 0.5;
    let ragged = fbm(p * 2.4, 137) - 0.5;
    let t = smooth(1.0, 0.55, e + wobble * 0.3 + ragged * 0.15);
    t * (0.8 + 0.2 * smooth(0.3, 0.6, fbm(p * 0.9, 139)))
}

/// How thick the leaves lie: at the clearing's rim most, blown off its middle, a thin fall
/// on the grass about.
fn leafy(p: Vec2) -> f32 {
    let t = trodden(p);
    let rim = smooth(0.03, 0.3, t) * smooth(0.95, 0.45, t);
    0.15 + 0.6 * rim + 0.25 * smooth(0.55, 0.7, fbm(p * 0.5, 141))
}

/// Flat cards on the ground, each a cell of the litter atlas, each corner on the land.
#[derive(Default)]
struct Cards {
    pos: Vec<[f32; 3]>,
    uv: Vec<[f32; 2]>,
    col: Vec<[f32; 4]>,
    idx: Vec<u32>,
}

impl Cards {
    fn add(&mut self, land: &Land, at: Vec2, size: f32, turn: f32, cell: u32, tint: Color) {
        let base = self.pos.len() as u32;
        let lift = 0.02 + base as f32 * 1.0e-6;
        let (s, c) = (turn.sin(), turn.cos());
        let (u0, v0) = ((cell % 4) as f32 * 0.25, (cell / 4) as f32 * 0.25);
        for (dx, dz, u, v) in [
            (-0.5, -0.5, 0.0, 0.0),
            (0.5, -0.5, 0.25, 0.0),
            (0.5, 0.5, 0.25, 0.25),
            (-0.5, 0.5, 0.0, 0.25),
        ] {
            let q = at + Vec2::new(dx * c - dz * s, dx * s + dz * c) * size;
            self.pos.push([q.x, land.height(q) + lift, q.y]);
            self.uv.push([u0 + u, v0 + v]);
            self.col.push(tint.to_linear().to_f32_array());
        }
        // Wound to face up.
        self.idx
            .extend([base, base + 3, base + 1, base + 1, base + 3, base + 2]);
    }

    fn mesh(self) -> Mesh {
        let n = self.pos.len();
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32, 1.0, 0.0]; n])
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uv)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.col)
        .with_inserted_indices(Indices::U32(self.idx))
        .with_generated_tangents()
        .expect("the cards have normals and UVs")
    }
}

/// Clear of every one of the cast's feet by `r` metres.
fn off_the_cast(p: Vec2, r: f32) -> bool {
    [KNIGHT, FOE, TARGET, SECOND]
        .iter()
        .all(|c| c.distance(p) > r)
}

fn spawn(
    mut commands: Commands,
    asset: Res<AssetServer>,
    land: Res<Land>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let (lo, hi) = (MIDDLE - Vec2::splat(6.0), MIDDLE + Vec2::splat(6.0));
    let area = (hi.x - lo.x) * (hi.y - lo.y);
    let at = |rng: &mut Rng| Vec2::new(rng.range(lo.x, hi.x), rng.range(lo.y, hi.y));
    let on = |p: Vec2| Vec3::new(p.x, land.height(p), p.y);

    let mut cards = Cards::default();
    let mut rng = Rng::new(0xc1ea_f5);
    for _ in 0..(area * LEAF_TRIES) as usize {
        let p = at(&mut rng);
        if terrain::pond(p) > -0.04 || p.distance(crate::scene::FIRE) < 1.5 {
            continue;
        }
        if rng.f() < leafy(p) {
            let cell = LEAVES[(rng.f() * 4.0) as usize % 4];
            let v = rng.range(0.6, 1.0);
            cards.add(
                &land,
                p,
                rng.range(LEAF_CARD.0, LEAF_CARD.1),
                rng.range(0.0, 6.28),
                cell,
                Color::srgb(v, v * 0.92, v * 0.85),
            );
        }
    }
    let linear = |file: &str| {
        asset
            .load_builder()
            .with_settings(|s: &mut ImageLoaderSettings| s.is_srgb = false)
            .load(format!("materials/litter/{file}"))
    };
    commands.spawn((
        Mesh3d(meshes.add(cards.mesh())),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color_texture: Some(asset.load("materials/litter/cards.png")),
            normal_map_texture: Some(linear("normal.png")),
            alpha_mode: AlphaMode::Mask(0.5),
            perceptual_roughness: 0.85,
            reflectance: 0.3,
            ..default()
        })),
        bevy::light::NotShadowCaster,
    ));

    // Small mossy stones on the trodden earth, ferns about its rim, stones in the shallows
    // at the pond's waterline (pond 0 is the shore; 0.07 is about 0.4 m out).
    let rock = |rng: &mut Rng| {
        let i = 1 + (rng.f() * 7.0) as usize % 7;
        format!("models/nature/forest/mossy_rock_b_{i}.glb")
    };
    let mut place = |rng: &mut Rng,
                     n: usize,
                     ok: &dyn Fn(Vec2) -> bool,
                     what: &dyn Fn(&mut Rng) -> (String, f32, f32)| {
        let (mut placed, mut tries) = (0, 0);
        while placed < n && tries < 4_000 {
            tries += 1;
            let p = Vec2::new(rng.range(lo.x, hi.x + 3.0), rng.range(lo.y, hi.y));
            if !ok(p) {
                continue;
            }
            let (path, scale, sink) = what(rng);
            let e = crate::scene::model(
                &mut commands,
                &asset,
                &path,
                on(p) - Vec3::Y * sink,
                rng.f() * 6.28,
                scale,
            );
            commands.entity(e).insert(crate::rim::Shade);
            placed += 1;
        }
    };
    let mut rng = Rng::new(0x5_70e6);
    place(
        &mut rng,
        STONES,
        &|p: Vec2| {
            let t = trodden(p);
            t > 0.1 && t < 0.85 && off_the_cast(p, 0.9) && terrain::pond(p) < -0.06
        },
        &|rng: &mut Rng| {
            let s = rng.range(0.2, 0.35);
            (rock(rng), s, 0.2 * s)
        },
    );
    place(
        &mut rng,
        FERNS,
        &|p: Vec2| {
            let t = trodden(p);
            t > 0.0 && t < 0.2 && super::dress::free(p)
        },
        &|rng: &mut Rng| {
            (
                "models/nature/forest/fern.glb".to_string(),
                rng.range(0.8, 1.3),
                0.05,
            )
        },
    );
    place(
        &mut rng,
        SHORE_STONES,
        &|p: Vec2| {
            let w = terrain::pond(p);
            (0.0..0.07).contains(&w) && p.x < 12.0 && p.y < 9.5
        },
        &|rng: &mut Rng| {
            let s = rng.range(0.45, 0.8);
            (rock(rng), s, 0.15 * s)
        },
    );
}

/// The day's sun, toward it (scene.rs: elevation 52, azimuth -40 from the south).
fn toward_sun() -> Vec3 {
    let (el, az) = (52f32.to_radians(), (-40f32).to_radians());
    Vec3::new(el.cos() * az.sin(), el.sin(), el.cos() * az.cos())
}

/// Whether an oak's crown (about 5 to 14 m up at scale 1, `TALL` times as high at its true
/// proportions) throws its shade within `BREAK` metres of the fight's middle. At j27's
/// first still, OAKS[0]'s alone cast out, the knight still stood in another crown's shade.
fn shades_the_fight(trunk: Vec2, scale: f32) -> bool {
    let t = toward_sun();
    let down = -Vec2::new(t.x, t.z) / t.y;
    let up = scale * crate::forest::TALL;
    let (a, b) = (trunk + down * 5.0 * up, trunk + down * 14.0 * up);
    let ab = b - a;
    let k = ((MIDDLE - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0);
    MIDDLE.distance(a + ab * k) < BREAK
}

/// The sun break: the oaks that shade the fight cast no shadow, crown, trunk and boughs (with
/// the crowns alone cast out, the boughs' long shadow still lay over the knight: j27's
/// second still).
fn sun_break(
    mut commands: Commands,
    crowns: Query<&ChildOf, With<Crown>>,
    roots: Query<(&Transform, &Children)>,
) {
    for parent in &crowns {
        let Ok((t, parts)) = roots.get(parent.parent()) else {
            continue;
        };
        if shades_the_fight(t.translation.xz(), t.scale.x) {
            for e in parts.to_vec() {
                commands.entity(e).insert(bevy::light::NotShadowCaster);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ground is trodden under every one of the cast, not in the pond or among the far
    /// oaks; and the shade oak is the one whose crown's shadow (9 m up, under the day's sun at
    /// 52 degrees from azimuth -40, scene.rs) falls on the fight.
    #[test]
    fn the_fight_stands_on_trodden_ground_in_the_sun() {
        for c in [KNIGHT, FOE, TARGET, SECOND] {
            assert!(trodden(c) > 0.3, "{c} stands on grass: {}", trodden(c));
        }
        assert!(
            trodden(terrain::POND) < 0.01,
            "the pond's middle is trodden"
        );
        assert!(trodden(SHADE_OAK) < 0.01, "the shade oak's foot is trodden");
        assert!(
            super::super::dress::OAKS
                .iter()
                .any(|(o, ..)| *o == SHADE_OAK),
            "the shade oak is not one of the forest's oaks"
        );
        let toward = toward_sun();
        let crown = SHADE_OAK.extend(9.0).xzy();
        let shadow = crown - toward * (9.0 / toward.y);
        let miss = shadow.xz().distance(MIDDLE);
        assert!(
            miss < 2.5,
            "the shade oak's shadow lands {miss} m from the fight"
        );
        let (_, scale, _) = super::super::dress::OAKS
            .iter()
            .find(|(o, ..)| *o == SHADE_OAK)
            .unwrap();
        assert!(shades_the_fight(SHADE_OAK, *scale));
        // The break is the fight's: the wood about keeps its dapple.
        let wood = crate::forest::tree_spots(&Land::new());
        let open = wood
            .iter()
            .filter(|s| shades_the_fight(s.at, s.scale))
            .count();
        assert!(
            open <= 4,
            "{open} of the wood's {} oaks cast no shade",
            wood.len()
        );
    }
}
