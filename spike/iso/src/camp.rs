//! The party's camp about the fire, dressed as a night's camp is: fallen trunks dragged up to
//! sit on, a cooking tripod over the fire, bedrolls and packs, a bucket, a bowl, an axe by a
//! heap of gathered branches, and twigs and bark trodden into the earth. The trunks, stump,
//! branches, bark, bucket, bowl and axe are Poly Haven scans (CC0), cut to budget with their
//! textures by tools/blender/scan.py; the tripod and bedroll are Tripo models made through
//! tools/props (art/props/camp_*). The fire's stones are sooted.

use bevy::gltf::GltfMaterialName;
use bevy::prelude::*;

use crate::forest::Rng;
use crate::nav::Nav;
use crate::scene::{self, FIRE};
use crate::terrain::{self, Land};

const CAMP: &str = "spike://models/camp";

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, build.after(scene::build))
        .add_systems(Update, (soot, unshadowed));
}

/// A model whose meshes cast no shadow: the campfire's own logs and stones, which the
/// fire's light sits among, beside the pot hung over them, and
/// would throw across the camp as hard black spokes (a real fire is a broad flame that
/// wraps round them).
#[derive(Component)]
pub struct NoShadow;

fn unshadowed(
    mut commands: Commands,
    added: Query<Entity, Added<Mesh3d>>,
    parents: Query<&ChildOf>,
    marked: Query<(), With<NoShadow>>,
) {
    for e in &added {
        if parents.iter_ancestors(e).any(|a| marked.contains(a)) {
            commands.entity(e).insert(bevy::light::NotShadowCaster);
        }
    }
}

/// A model of the camp, at `p` on the land, turned `yaw` and laid to the slope.
fn put(
    commands: &mut Commands,
    asset: &AssetServer,
    land: &Land,
    file: &str,
    p: Vec2,
    yaw: f32,
    scale: f32,
    sink: f32,
) -> Entity {
    let at = Vec3::new(p.x, land.height(p) - sink, p.y);
    let e = scene::model(commands, asset, &format!("{CAMP}/{file}"), at, yaw, scale);
    let n = land.normal(p);
    commands.entity(e).insert(
        Transform::from_translation(at)
            .with_rotation(Quat::from_rotation_arc(Vec3::Y, n) * Quat::from_rotation_y(yaw))
            .with_scale(Vec3::splat(scale)),
    );
    e
}

/// The yaw that lays a model's length (its X) across the line from the fire, so it faces it.
fn across(offset: Vec2) -> f32 {
    let t = Vec2::new(-offset.y, offset.x);
    (-t.y).atan2(t.x)
}

fn build(mut commands: Commands, asset: Res<AssetServer>, land: Res<Land>, mut nav: ResMut<Nav>) {
    let mut rng = Rng::new(0xca3b_f1e5);
    // Two fallen trunks hauled up to sit on, a third of a ring apart, and a stump.
    for (d, len) in [
        (Vec2::new(-1.75, -1.05), 0.75),
        (Vec2::new(0.35, 2.05), 0.8),
    ] {
        let yaw = across(d) + rng.range(-0.15, 0.15);
        let p = FIRE + d;
        nav.block_rect(p, Vec2::new(1.5 * len, 0.16), yaw);
        put(&mut commands, &asset, &land, "trunk.glb", p, yaw, len, 0.06);
    }
    // The cooking tripod over the fire, its pot over the coals; it casts no shadow, as its
    // legs and pot about the fire's one point of light threw black wedges across the camp.
    let tripod = put(
        &mut commands,
        &asset,
        &land,
        "CampTripod.glb",
        FIRE,
        0.4,
        1.2,
        0.02,
    );
    commands.entity(tripod).insert(NoShadow);
    // Bedrolls on the fire's far side, heads away from it, a pack by each.
    for (d, turn) in [
        (Vec2::new(2.35, -0.55), 0.1),
        (Vec2::new(1.55, -2.1), -0.25),
    ] {
        let p = FIRE + d;
        let yaw = (-d.x).atan2(-d.y) + turn;
        nav.block_rect(p, Vec2::new(0.45, 1.0), yaw);
        put(
            &mut commands,
            &asset,
            &land,
            "CampBedroll.glb",
            p,
            yaw,
            1.0,
            0.0,
        );
    }
    // A bucket of water and a bowl by the fire; an axe laid by the gathered wood.
    put(
        &mut commands,
        &asset,
        &land,
        "bucket.glb",
        FIRE + Vec2::new(1.35, 1.25),
        0.7,
        1.0,
        0.01,
    );
    put(
        &mut commands,
        &asset,
        &land,
        "bowl.glb",
        FIRE + Vec2::new(-0.95, 0.75),
        0.0,
        1.0,
        0.0,
    );
    // The wood heap: branches gathered from the wood, crossed over each other.
    let heap = FIRE + Vec2::new(-2.6, 1.3);
    nav.block_circle(heap, 0.6);
    for i in 0..9 {
        let p = heap + Vec2::new(rng.range(-0.35, 0.35), rng.range(-0.3, 0.3));
        let yaw = 0.6 + rng.range(-0.5, 0.5) + if i % 3 == 0 { 1.4 } else { 0.0 };
        let e = put(
            &mut commands,
            &asset,
            &land,
            &format!("branch_{}.glb", 1 + i % 3),
            p,
            yaw,
            rng.range(0.9, 1.2),
            -0.05 * (i / 3) as f32,
        );
        let _ = e;
    }
    let axe = commands
        .spawn((
            WorldAssetRoot(
                asset.load(GltfAssetLabel::Scene(0).from_asset(format!("{CAMP}/axe.glb"))),
            ),
            Transform::from_translation(Vec3::new(
                heap.x + 0.7,
                land.height(heap) + 0.03,
                heap.y + 0.45,
            ))
            .with_rotation(Quat::from_rotation_y(2.2) * Quat::from_rotation_x(-1.52)),
        ))
        .id();
    let _ = axe;
    // A big old stump at the clearing's edge.
    let stump = FIRE + Vec2::new(-5.2, -2.6);
    nav.block_circle(stump, 0.7);
    put(
        &mut commands,
        &asset,
        &land,
        "stump.glb",
        stump,
        1.3,
        0.75,
        0.12,
    );

    // Twigs and bark trodden into the camp's earth, thickest at its edge, none in the fire.
    let mut placed = 0;
    let mut tries = 0;
    while placed < 34 && tries < 2000 {
        tries += 1;
        let p = FIRE + Vec2::new(rng.range(-4.5, 4.5), rng.range(-4.5, 4.5));
        let d = p.distance(FIRE);
        if d < 1.25 || terrain::bare(p) < 0.25 || rng.f() > terrain::bare(p) {
            continue;
        }
        let file = if rng.f() < 0.55 {
            format!("bark_{}.glb", 1 + (rng.f() * 4.0) as usize % 4)
        } else {
            format!("branch_{}.glb", 1 + (rng.f() * 2.0) as usize % 2)
        };
        let scale = rng.range(0.45, 0.8);
        put(
            &mut commands,
            &asset,
            &land,
            &file,
            p,
            rng.f() * 6.28,
            scale,
            0.02,
        );
        placed += 1;
    }
}

/// The fire's stones blackened with soot and its ring of ground darker, as a fire used for
/// days leaves them (the hero campfire's fieldstones are clean scans).
fn soot(
    added: Query<(&GltfMaterialName, &MeshMaterial3d<StandardMaterial>), Added<GltfMaterialName>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut done: Local<bevy::platform::collections::HashSet<AssetId<StandardMaterial>>>,
) {
    for (name, handle) in &added {
        if !done.insert(handle.0.id()) {
            continue;
        }
        let factor = match name.0.as_str() {
            "Stone" => 0.4,
            "Ground" => 0.6,
            // The bedroll's Tripo wool came out pale; a camp's blankets are dark and worn.
            n if n.starts_with("tripo_mat_515f618a") => 0.45,
            _ => continue,
        };
        if let Some(mut m) = materials.get_mut(&handle.0) {
            let c = m.base_color.to_linear();
            m.base_color = Color::LinearRgba(LinearRgba::new(
                c.red * factor,
                c.green * factor,
                c.blue * factor,
                c.alpha,
            ));
        }
    }
}
