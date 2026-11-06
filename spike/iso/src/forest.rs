//! The wood about the clearing: veteran oaks (the game's modelled trunks, their crowns
//! grown as leaf cards at the clumps `tools/blender/tips.py` found), mossy rocks along the
//! ridge's foot and on the knoll, fallen logs, stumps and ferns under the trees, and dense
//! grass and flowers in the open, all swaying in the wind (foliage.rs); the trees before the
//! party open in a dithered hole about each of them, as modern isometric games do.

use crate::foliage::{self, FoliageMaterial, Mats as FoliageMats};
use crate::nav::Nav;
use crate::scene::{self, FIRE};
use crate::terrain::{self, CAMP, Land, POND, WATER};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

/// The render layer only the sun sees.
pub const SUN_ONLY: usize = 1;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, plant.after(scene::build));
}

/// A little xorshift, so the wood is the same every run.
pub struct Rng(u32);

impl Rng {
    pub fn new(seed: u32) -> Self {
        Rng(seed.max(1))
    }
    pub fn f(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 >> 8) as f32 / (1u32 << 24) as f32
    }
    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.f()
    }
}

/// A leaf clump: where, the way its branch runs, its size and mass.
struct Tip {
    at: Vec3,
    dir: Vec3,
    size: f32,
    mass: f32,
}

fn tips(file: &str) -> Vec<Tip> {
    let text = std::fs::read_to_string(file).unwrap_or_else(|e| panic!("{file}: {e}"));
    text.lines()
        .filter(|l| l.trim_start().starts_with("(("))
        .map(|l| {
            let v: Vec<f32> = l
                .split(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-'))
                .filter(|s| !s.is_empty())
                .map(|s| s.parse().unwrap())
                .collect();
            Tip {
                at: Vec3::new(v[0], v[1], v[2]),
                dir: Vec3::new(v[3], v[4], v[5]).normalize_or(Vec3::Y),
                size: v[6],
                mass: v[7],
            }
        })
        .collect()
}

/// A mesh of quads, each with its own corners, normal per corner, atlas cell and tint.
#[derive(Default)]
pub struct Cards {
    pos: Vec<[f32; 3]>,
    nor: Vec<[f32; 3]>,
    uv: Vec<[f32; 2]>,
    col: Vec<[f32; 4]>,
    sway: Vec<[f32; 2]>,
    idx: Vec<u32>,
    /// Leaf cards flutter; others (flowers, brambles) bend from their foot.
    pub leaf: bool,
}

impl Cards {
    /// A card about `c`, `a` across and `b` up (the cell's top), in atlas cell `cell`.
    pub fn quad(
        &mut self,
        c: Vec3,
        a: Vec3,
        b: Vec3,
        cell: Vec2,
        normal: impl Fn(Vec3) -> Vec3,
        tint: [f32; 4],
    ) {
        self.graded(c, a, b, cell, normal, tint, tint);
    }

    /// A card as `quad`'s, its tint `foot` at the cell's bottom and `tip` at its top: the
    /// dark foot of a leaf in a clump (wood/lawn.rs).
    #[allow(clippy::too_many_arguments)]
    pub fn graded(
        &mut self,
        c: Vec3,
        a: Vec3,
        b: Vec3,
        cell: Vec2,
        normal: impl Fn(Vec3) -> Vec3,
        foot: [f32; 4],
        tip: [f32; 4],
    ) {
        let base = self.pos.len() as u32;
        for (x, y) in [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)] {
            let p = c + a * (x - 0.5) + b * (y - 0.5);
            self.pos.push(p.to_array());
            self.nor.push(normal(p).to_array());
            let inset = 0.004;
            self.uv.push([
                cell.x + inset + (0.25 - 2.0 * inset) * x,
                cell.y + inset + (0.25 - 2.0 * inset) * (1.0 - y),
            ]);
            self.col.push(if y > 0.5 { tip } else { foot });
            self.sway.push([y, if self.leaf { 1.0 } else { 0.0 }]);
        }
        self.idx
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    pub fn mesh(self) -> Mesh {
        Mesh::new(PrimitiveTopology::TriangleList, default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.pos)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.nor)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uv)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.col)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, self.sway)
            .with_inserted_indices(Indices::U32(self.idx))
    }
}

/// The atlas's cells (materials/foliage/cards.png, 4 by 4).
pub const OAKBRANCH: Vec2 = Vec2::new(0.5, 0.5);
pub const OAK: Vec2 = Vec2::new(0.0, 0.0);
pub const DAISY: Vec2 = Vec2::new(0.75, 0.25);
pub const BUTTERCUP: Vec2 = Vec2::new(0.0, 0.5);
pub const CAMPION: Vec2 = Vec2::new(0.25, 0.5);
pub const BRAMBLE: Vec2 = Vec2::new(0.0, 0.75);

/// A crown's shape: its middle and half-sizes, as the clumps lie.
#[derive(Clone, Copy)]
#[allow(dead_code)]
struct Shell {
    middle: Vec3,
    half: Vec3,
}

/// A crown of cards at a tree's clumps, normals bent out from its middle so it shades as
/// one mass, its inside darker.
fn crown(tips: &[Tip], rng: &mut Rng) -> (Mesh, Shell) {
    let n = tips.len() as f32;
    let middle = tips.iter().map(|t| t.at).sum::<Vec3>() / n;
    let (lo, hi) = tips.iter().fold((Vec3::MAX, Vec3::MIN), |(l, h), t| {
        (l.min(t.at), h.max(t.at))
    });
    let half = ((hi - lo) * 0.5 + 1.0).max(Vec3::splat(1.0));
    let shell = Shell { middle, half };
    let mut cards = Cards {
        leaf: true,
        ..default()
    };
    for t in tips {
        let count = (3.0 + t.mass * 0.35).min(7.0) as usize;
        for _ in 0..count {
            let jitter = Vec3::new(rng.f() - 0.5, rng.f() - 0.5, rng.f() - 0.5) * t.size * 1.3;
            let c = t.at + t.dir * t.size * 0.4 + jitter;
            // Up the card runs roughly along the branch, turned about it at random.
            let up = (t.dir + Vec3::new(rng.f() - 0.5, rng.f() * 0.6, rng.f() - 0.5)).normalize();
            let side = up.any_orthonormal_vector();
            let turn = Quat::from_axis_angle(up, rng.f() * std::f32::consts::TAU);
            let across = turn * side;
            let s = t.size * rng.range(1.7, 2.4);
            let out = (c - middle) / half;
            let depth = out.length().min(1.2);
            // Deeper in the crown, darker; a few clumps yellower.
            let shade = 0.28 + 0.6 * (depth / 1.2).powf(1.2);
            let warm = rng.f() * 0.1;
            let tint = [
                shade * (0.74 + warm),
                shade * (0.92 + warm * 0.3),
                shade * 0.62,
                1.0,
            ];
            let cell = if rng.f() < 0.75 { OAKBRANCH } else { OAK };
            let card_n = across.cross(up).normalize();
            cards.quad(
                c,
                across * s,
                up * s,
                cell,
                |p| {
                    let o = ((p - middle) / half).normalize_or(Vec3::Y);
                    (o * 0.75 + card_n * 0.1 + Vec3::Y * 0.35).normalize()
                },
                tint,
            );
        }
    }
    (cards.mesh(), shell)
}

/// A tree's crown of leaf cards.
#[derive(Component)]
pub struct Crown;

/// Grass blades, solid and tapered as Ghost of Tsushima's are, over a square of the land,
/// as dense as the open ground is grassy; their normals lean up so they shade with the
/// ground they stand on.
fn grass(land: &Land, lo: Vec2, size: f32, rng: &mut Rng, nav: &Nav) -> Option<Mesh> {
    let mut pos = Vec::new();
    let mut nor = Vec::new();
    let mut col = Vec::new();
    let mut sway: Vec<[f32; 2]> = Vec::new();
    let mut idx: Vec<u32> = Vec::new();
    let tries = (size * size * 70.0) as usize;
    for _ in 0..tries {
        let p = lo + Vec2::new(rng.f(), rng.f()) * size;
        let woods = terrain::woods(p);
        let clump = terrain::fbm(p * 0.35, 51);
        // In clumps and drifts, not a lawn; none where the ground is trodden bare.
        let want = (1.0 - woods * 0.9)
            * terrain::smooth(0.38, 0.62, clump + 0.2 * (1.0 - woods))
            * terrain::smooth(0.6, 0.15, terrain::bare(p));
        if rng.f() > want || land.height(p) < WATER + 0.12 || land.slope(p) > 0.55 {
            continue;
        }
        let _ = nav;
        let h0 = land.height(p);
        let tall = rng.range(0.12, 0.3) * (0.6 + 0.6 * clump) * (1.0 + 0.5 * woods);
        let width = rng.range(0.02, 0.035);
        let yaw = rng.f() * std::f32::consts::TAU;
        let side = Vec3::new(yaw.cos(), 0.0, yaw.sin()) * width;
        let lean = Vec3::new(rng.f() - 0.5, 0.0, rng.f() - 0.5) * tall * 0.7;
        let hue = terrain::fbm(p * 0.12, 57);
        let dry = terrain::smooth(0.55, 0.8, hue) * 0.6 + rng.f() * 0.15;
        let base = pos.len() as u32;
        let segs = 3;
        for k in 0..=segs {
            let t = k as f32 / segs as f32;
            let centre = Vec3::new(p.x, h0, p.y) + Vec3::Y * tall * t + lean * t * t;
            let w = 1.0 - t * 0.9;
            pos.push((centre - side * w).to_array());
            pos.push((centre + side * w).to_array());
            let n = (Vec3::Y * 0.8 + side.cross(Vec3::Y).normalize() * 0.2).normalize();
            nor.push(n.to_array());
            nor.push(n.to_array());
            let g = 0.35 + 0.65 * t;
            let c = [
                g * (0.2 + 0.25 * dry),
                g * (0.34 + 0.04 * dry),
                g * (0.07 + 0.03 * dry),
                1.0,
            ];
            col.push(c);
            col.push(c);
            sway.push([t, 0.0]);
            sway.push([t, 0.0]);
        }
        for k in 0..segs as u32 {
            let a = base + k * 2;
            idx.extend_from_slice(&[a, a + 1, a + 3, a, a + 3, a + 2]);
        }
    }
    if idx.is_empty() {
        return None;
    }
    Some(
        Mesh::new(PrimitiveTopology::TriangleList, default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, nor)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, col)
            // A first UV set as well: the standard material wants one beside the second.
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0f32, 0.0]; sway.len()])
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, sway)
            .with_inserted_indices(Indices::U32(idx)),
    )
}

/// Oaks a scene stands beyond the wood's own (the iso forest's, framing its fight).
#[derive(Resource, Default)]
pub struct Extra(pub Vec<Spot>);

#[allow(clippy::too_many_arguments)]
/// Where an oak stands, how large and turned.
#[derive(Clone, Copy)]
pub struct Spot {
    pub at: Vec2,
    pub scale: f32,
    pub yaw: f32,
}

/// The oaks' places, the same every run: in the wood, off the water and the steep, apart.
/// The ground reads them too, for the shade under each crown (scene::cover).
pub fn tree_spots(land: &Land) -> Vec<Spot> {
    let mut rng = Rng::new(0x7ee5_0a11);
    let mut trees: Vec<Spot> = Vec::new();
    let mut tries = 0;
    while trees.len() < 80 && tries < 20_000 {
        tries += 1;
        let p = Vec2::new(rng.range(-50.0, 50.0), rng.range(-50.0, 50.0));
        let spacing = rng.range(5.5, 8.5);
        let scale = rng.range(0.55, 0.85);
        // Seen from above at the camera's slant, a crown stands up the screen from its
        // trunk (about a metre for each metre up): it must lie over the wood too, not
        // hang over the clearing, or it reads as a bush floating by the camp.
        let toward = crate::camera::offset();
        let crown = p - Vec2::new(toward.x, toward.z).normalize() * 12.0 * scale;
        if terrain::woods(p) < 0.6
            || terrain::woods(crown) < 0.3
            || terrain::pond(p) > -0.3
            || land.slope(p) > 0.5
            || trees.iter().any(|t| t.at.distance(p) < spacing)
        {
            continue;
        }
        trees.push(Spot {
            at: p,
            scale,
            yaw: rng.f() * std::f32::consts::TAU,
        });
    }
    trees
}

#[allow(clippy::too_many_arguments)]
fn plant(
    mut commands: Commands,
    asset: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<FoliageMaterial>>,
    mut mats: ResMut<FoliageMats>,
    land: Res<Land>,
    mut nav: ResMut<Nav>,
    extra: Option<Res<Extra>>,
) {
    let mut rng = Rng::new(0x51ed_270b);
    let on = |p: Vec2| Vec3::new(p.x, land.height(p), p.y);
    let cards = StandardMaterial {
        base_color_texture: Some(asset.load("materials/foliage/cards.png")),
        alpha_mode: AlphaMode::Mask(0.45),
        cull_mode: None,
        perceptual_roughness: 0.75,
        reflectance: 0.25,
        diffuse_transmission: 0.2,
        ..default()
    };
    let leaves = foliage::material(&mut materials, &mut mats, cards.clone(), 0.0, 0.035, true);
    let blooms = foliage::material(&mut materials, &mut mats, cards, 0.06, 0.0, false);
    let bark = scene::material(&asset, "oak_bark", Color::srgb(0.62, 0.58, 0.52));
    let bark = foliage::material(&mut materials, &mut mats, bark, 0.0, 0.0, true);

    // The oaks: two modelled trunks, each with its crown.
    let kinds = [
        (
            "models/nature/forest/OakBough.glb",
            "assets/data/trees/oak_bough.ron",
        ),
        (
            "models/nature/forest/OakBough2.glb",
            "assets/data/trees/oak_bough_2.ron",
        ),
    ];
    let root = std::env::var("BEVY_ASSET_ROOT").unwrap_or_else(|_| "../..".into());
    let kinds: Vec<(Handle<Mesh>, Handle<Mesh>, Shell)> = kinds
        .iter()
        .map(|(model, data)| {
            let (mesh, shell) = crown(&tips(&format!("{root}/{data}")), &mut rng);
            (
                asset.load(
                    GltfAssetLabel::Primitive {
                        mesh: 0,
                        primitive: 0,
                    }
                    .from_asset(*model),
                ),
                meshes.add(mesh),
                shell,
            )
        })
        .collect();
    let mut spots = tree_spots(&land);
    spots.extend(extra.iter().flat_map(|e| e.0.iter().copied()));
    let trees: Vec<Vec2> = spots.iter().map(|t| t.at).collect();
    for (i, t) in spots.iter().enumerate() {
        let (p, scale, yaw) = (&t.at, t.scale, t.yaw);
        let (trunk, crown, shell) = &kinds[i % kinds.len()];
        let rot = Quat::from_rotation_y(yaw);
        nav.block_circle(*p, 0.9 * scale);
        let at = on(*p) - Vec3::Y * 0.15;
        let root = commands
            .spawn((
                Transform::from_translation(at)
                    .with_rotation(rot)
                    .with_scale(Vec3::splat(scale)),
                Visibility::default(),
            ))
            .id();
        commands.spawn((
            Mesh3d(trunk.clone()),
            MeshMaterial3d(bark.clone()),
            ChildOf(root),
        ));
        commands.spawn((
            Mesh3d(crown.clone()),
            MeshMaterial3d(leaves.clone()),
            ChildOf(root),
            Crown,
        ));
        let _ = shell;
    }

    // Rocks: a tumble along the ridge's foot, a few crowning the knoll, boulders in the wood.
    let rocks: Vec<String> = (1..=6)
        .map(|i| format!("models/nature/forest/mossy_rock_a_{i}.glb"))
        .chain((1..=7).map(|i| format!("models/nature/forest/mossy_rock_b_{i}.glb")))
        .collect();
    let rock =
        |commands: &mut Commands, nav: &mut Nav, rng: &mut Rng, p: Vec2, (lo, hi): (f32, f32)| {
            let scale = rng.range(lo, hi);
            let big = rng.f() < 0.55;
            let path = if big {
                &rocks[(rng.f() * 6.0) as usize % 6]
            } else {
                &rocks[6 + (rng.f() * 7.0) as usize % 7]
            };
            let foot = if big { 1.3 } else { 0.55 } * scale;
            nav.block_circle(p, foot * 0.8);
            let e = scene::model(
                commands,
                &asset,
                path,
                on(p) - Vec3::Y * 0.25 * scale,
                rng.f() * 6.28,
                scale,
            );
            commands.entity(e).insert(crate::rim::Shade);
        };
    let mut x = -46.0;
    while x < 46.0 {
        let z = terrain::ridge_z(x) + rng.range(-1.0, 1.5);
        let p = Vec2::new(x, z);
        rock(&mut commands, &mut nav, &mut rng, p, (1.0, 2.1));
        if rng.f() < 0.5 {
            let q = p + Vec2::new(rng.range(-1.0, 1.0), rng.range(1.0, 2.5));
            rock(&mut commands, &mut nav, &mut rng, q, (0.6, 1.1));
        }
        x += rng.range(2.4, 4.2);
    }
    for i in 0..7 {
        let a = i as f32 / 7.0 * std::f32::consts::TAU + rng.f();
        let p = Vec2::new(-15.0, 5.0) + Vec2::from_angle(a) * rng.range(0.5, 3.2);
        rock(&mut commands, &mut nav, &mut rng, p, (0.8, 1.6));
    }
    let mut placed = 0;
    while placed < 22 {
        let p = Vec2::new(rng.range(-40.0, 40.0), rng.range(-12.0, 40.0));
        if terrain::woods(p) < 0.4
            || terrain::pond(p) > -0.2
            || trees.iter().any(|t| t.distance(p) < 2.0)
        {
            continue;
        }
        rock(&mut commands, &mut nav, &mut rng, p, (0.5, 1.2));
        placed += 1;
    }
    // Three stones by the pond's edge, and one at the camp to sit on.
    for p in [
        POND + Vec2::new(-6.8, -2.0),
        POND + Vec2::new(-5.5, 3.6),
        POND + Vec2::new(2.0, -5.6),
    ] {
        rock(&mut commands, &mut nav, &mut rng, p, (0.6, 0.8));
    }

    // Fallen logs and stumps among the trees.
    let mut placed = 0;
    while placed < 9 {
        let p = Vec2::new(rng.range(-36.0, 36.0), rng.range(-10.0, 36.0));
        if terrain::woods(p) < 0.5
            || terrain::pond(p) > -0.2
            || trees.iter().any(|t| t.distance(p) < 3.0)
        {
            continue;
        }
        let yaw = rng.f() * 6.28;
        let scale = rng.range(1.6, 2.4);
        nav.block_rect(p, Vec2::new(0.85, 0.22) * scale, yaw);
        let e = scene::model(
            &mut commands,
            &asset,
            "models/props/Log.glb",
            on(p) - Vec3::Y * 0.12,
            yaw,
            scale,
        );
        let n = land.normal(p);
        commands.entity(e).insert((
            crate::rim::Shade,
            Transform::from_translation(on(p) - Vec3::Y * 0.12)
                .with_rotation(Quat::from_rotation_arc(Vec3::Y, n) * Quat::from_rotation_y(yaw))
                .with_scale(Vec3::splat(scale)),
        ));
        placed += 1;
    }
    for i in 0..6 {
        let p =
            trees[(i * 7 + 3) % trees.len()] + Vec2::new(rng.range(-4.0, 4.0), rng.range(2.0, 4.0));
        if terrain::pond(p) > -0.2 {
            continue;
        }
        nav.block_circle(p, 0.6);
        let e = scene::model(
            &mut commands,
            &asset,
            "models/nature/forest/stump.glb",
            on(p) - Vec3::Y * 0.08,
            rng.f() * 6.28,
            rng.range(0.9, 1.3),
        );
        commands.entity(e).insert(crate::rim::Shade);
    }

    // Ferns under the trees, in drifts.
    let mut placed = 0;
    let mut tries = 0;
    while placed < 260 && tries < 20_000 {
        tries += 1;
        let p = Vec2::new(rng.range(-44.0, 44.0), rng.range(-30.0, 44.0));
        let drift = terrain::fbm(p * 0.15, 61);
        if terrain::woods(p) < 0.35
            || drift < 0.5
            || land.slope(p) > 0.6
            || terrain::pond(p) > -0.25
        {
            continue;
        }
        if trees.iter().any(|t| t.distance(p) < 1.0) {
            continue;
        }
        let e = scene::model(
            &mut commands,
            &asset,
            "models/nature/forest/fern.glb",
            on(p) - Vec3::Y * 0.05,
            rng.f() * 6.28,
            rng.range(0.8, 1.5),
        );
        commands.entity(e).insert(crate::rim::Shade);
        placed += 1;
    }

    // Grass, in 8 m squares; flowers among it in the open; brambles at the wood's edge.
    let blades = StandardMaterial {
        cull_mode: None,
        perceptual_roughness: 0.7,
        reflectance: 0.3,
        ..default()
    };
    let blades = foliage::material(&mut materials, &mut mats, blades, 0.09, 0.0, false);
    let chunk = 8.0;
    let reach = 40.0;
    let mut z = -reach;
    while z < reach {
        let mut x = -reach;
        while x < reach {
            if let Some(mesh) = grass(&land, Vec2::new(x, z), chunk, &mut rng, &nav) {
                // No shadow: the fire's light threw each blade's long and spiky across the camp.
                commands.spawn((
                    Mesh3d(meshes.add(mesh)),
                    MeshMaterial3d(blades.clone()),
                    bevy::light::NotShadowCaster,
                ));
            }
            x += chunk;
        }
        z += chunk;
    }
    let mut flowers = Cards::default();
    for _ in 0..900 {
        let p = CAMP + Vec2::new(rng.range(-16.0, 16.0), rng.range(-16.0, 16.0));
        let woods = terrain::woods(p);
        if land.height(p) < WATER + 0.15 || p.distance(FIRE) < 2.5 || land.slope(p) > 0.5 {
            continue;
        }
        let (cell, size) = if woods > 0.35 && woods < 0.85 {
            if rng.f() < 0.5 {
                (BRAMBLE, rng.range(0.6, 1.0))
            } else {
                continue;
            }
        } else if woods < 0.3 && terrain::fbm(p * 0.2, 71) > 0.5 {
            match (rng.f() * 3.0) as u32 {
                0 => (DAISY, rng.range(0.25, 0.4)),
                1 => (BUTTERCUP, rng.range(0.3, 0.45)),
                _ => (CAMPION, rng.range(0.35, 0.5)),
            }
        } else {
            continue;
        };
        let base = on(p);
        let yaw = rng.f() * std::f32::consts::PI;
        for k in 0..2 {
            let a = yaw + k as f32 * std::f32::consts::FRAC_PI_2;
            let across = Vec3::new(a.cos(), 0.0, a.sin()) * size;
            flowers.quad(
                base + Vec3::Y * size * 0.48,
                across,
                Vec3::Y * size,
                cell,
                |_| Vec3::Y,
                [1.0; 4],
            );
        }
    }
    commands.spawn((Mesh3d(meshes.add(flowers.mesh())), MeshMaterial3d(blooms)));
}
