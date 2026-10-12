//! The map (D76): the land around the start, shown only where you have been. The rest
//! stays dark until you go there. It shows where you are, for now, and your claim; it
//! does not show where anything else is, quests least of all (D92).
//!
//! The land is drawn from the world's own rules, not from what has been loaded, so the
//! map costs a few thousand ground samples once, and a bit per patch of land seen.
//!
//!   M             open and close the map

use super::claim::Claim;
use super::controls::{Action, Controls};
use super::{IsLocalPlayer, LocalPlayer, Player};
use crate::build::Site;
use crate::planet::Planet;
use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};

/// The map's size in pixels, and the ground each pixel covers.
const PIXELS: usize = 128;
const METRES_PER_PIXEL: f32 = 4.0;
/// How far around you the land is seen as you go.
const SEEN_M: f32 = 34.0;
/// How big the map is drawn on screen.
const SHOWN_PX: f32 = 520.0;
const FOG: [u8; 4] = [22, 24, 30, 235];

#[derive(Resource)]
struct Chart {
    image: Handle<Image>,
    /// The land's colour at each pixel, worked out once.
    land: Vec<[u8; 4]>,
    open: bool,
}

/// Which pixels of the map a player has seen. You know the land you have been near (D93).
#[derive(Component)]
struct Explored(Vec<bool>);

impl Default for Explored {
    fn default() -> Self {
        Self(vec![false; PIXELS * PIXELS])
    }
}

#[derive(Component)]
struct Sheet;

#[derive(Component)]
struct You;

#[derive(Component)]
struct Yours;

pub fn plugin(app: &mut App) {
    app.register_required_components::<Player, Explored>()
        .add_systems(PostStartup, draw_land)
        .add_systems(Update, (explore, toggle, place_marks).chain());
}

/// Where a pixel of the map is on the planet.
fn pixel_to_planet(site: &Site, x: usize, y: usize) -> Vec3 {
    let east = (x as f32 - PIXELS as f32 * 0.5) * METRES_PER_PIXEL;
    let south = (y as f32 - PIXELS as f32 * 0.5) * METRES_PER_PIXEL;
    site.start.transform_point(Vec3::new(east, 0.0, south))
}

/// Which pixel a point on the planet falls in, if it is on the map.
fn planet_to_pixel(site: &Site, point: Vec3) -> Option<(usize, usize)> {
    let local = site
        .start
        .compute_affine()
        .inverse()
        .transform_point3(point);
    let x = local.x / METRES_PER_PIXEL + PIXELS as f32 * 0.5;
    let y = local.z / METRES_PER_PIXEL + PIXELS as f32 * 0.5;
    let inside = (0.0..PIXELS as f32).contains(&x) && (0.0..PIXELS as f32).contains(&y);
    inside.then_some((x as usize, y as usize))
}

/// Works out the colour of the land once, from the planet's rules.
fn draw_land(
    mut commands: Commands,
    site: Res<Site>,
    planet: Res<Planet>,
    mut images: ResMut<Assets<Image>>,
) {
    let sea = planet.sea_radius_m();
    let mut heights = vec![0.0; PIXELS * PIXELS];
    let mut land = vec![[0u8; 4]; PIXELS * PIXELS];
    for y in 0..PIXELS {
        for x in 0..PIXELS {
            let point = planet.ground_at(pixel_to_planet(&site, x, y));
            heights[y * PIXELS + x] = point.length() - sea;
        }
    }
    for y in 0..PIXELS {
        for x in 0..PIXELS {
            let height = heights[y * PIXELS + x];
            // Light from the north-west, so hills read.
            let west = heights[y * PIXELS + x.saturating_sub(1)];
            let north = heights[y.saturating_sub(1) * PIXELS + x];
            let shade = (1.0 + ((west - height) + (north - height)) * 0.35).clamp(0.6, 1.3);
            let colour: [f32; 3] = if height < 0.0 {
                let deep = (-height / 6.0).clamp(0.0, 1.0);
                [0.32 - 0.12 * deep, 0.5 - 0.15 * deep, 0.62 - 0.1 * deep]
            } else {
                let point = planet.ground_at(pixel_to_planet(&site, x, y));
                let cover = planet.cover(point, point.normalize());
                let grass = cover.grass;
                [
                    (0.72 * (1.0 - grass) + 0.42 * grass) * shade,
                    (0.66 * (1.0 - grass) + 0.55 * grass) * shade,
                    (0.5 * (1.0 - grass) + 0.3 * grass) * shade,
                ]
            };
            let byte = |value: f32| (value.clamp(0.0, 1.0) * 255.0) as u8;
            land[y * PIXELS + x] = [byte(colour[0]), byte(colour[1]), byte(colour[2]), 255];
        }
    }
    let image = images.add(Image::new_fill(
        Extent3d {
            width: PIXELS as u32,
            height: PIXELS as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &FOG,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    ));
    commands
        .spawn((
            Sheet,
            Node {
                position_type: PositionType::Absolute,
                left: percent(50),
                top: percent(50),
                width: px(SHOWN_PX),
                height: px(SHOWN_PX),
                margin: UiRect::new(px(-SHOWN_PX * 0.5), px(0), px(-SHOWN_PX * 0.5), px(0)),
                border: UiRect::all(px(3)),
                border_radius: BorderRadius::all(px(6)),
                ..default()
            },
            BorderColor::all(Color::srgb(0.55, 0.45, 0.32)),
            ImageNode::new(image.clone()),
            Visibility::Hidden,
        ))
        .with_children(|sheet| {
            sheet.spawn((
                Yours,
                Node {
                    position_type: PositionType::Absolute,
                    width: px(14),
                    height: px(14),
                    border: UiRect::all(px(2)),
                    border_radius: BorderRadius::all(px(7)),
                    ..default()
                },
                BorderColor::all(Color::srgb(1.0, 0.78, 0.3)),
                Visibility::Hidden,
            ));
            sheet.spawn((
                You,
                Node {
                    position_type: PositionType::Absolute,
                    width: px(9),
                    height: px(9),
                    border_radius: BorderRadius::all(px(5)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.95, 0.4, 0.15)),
            ));
        });
    commands.insert_resource(Chart {
        image,
        land,
        open: false,
    });
}

/// Marks the land around each player as seen by them, and shows on the map what this
/// machine's player has seen.
fn explore(
    site: Res<Site>,
    mut players: Query<(&Transform, &mut Explored, Has<LocalPlayer>), With<Player>>,
    chart: Option<Res<Chart>>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(chart) = chart else { return };
    for (at, mut explored, ours) in &mut players {
        let newly = newly_seen(&site, at.translation, &explored.0);
        if newly.is_empty() {
            continue;
        }
        for &index in &newly {
            explored.0[index] = true;
        }
        if !ours {
            continue;
        }
        let Some(data) = images
            .get_mut(&chart.image)
            .and_then(|image| image.into_inner().data.as_mut())
        else {
            continue;
        };
        for index in newly {
            data[index * 4..index * 4 + 4].copy_from_slice(&chart.land[index]);
        }
    }
}

/// The pixels near a point not yet seen.
fn newly_seen(site: &Site, point: Vec3, seen: &[bool]) -> Vec<usize> {
    let Some((cx, cy)) = planet_to_pixel(site, point) else {
        return Vec::new();
    };
    let reach = (SEEN_M / METRES_PER_PIXEL).ceil() as i64;
    let mut newly = Vec::new();
    for dy in -reach..=reach {
        for dx in -reach..=reach {
            let (x, y) = (cx as i64 + dx, cy as i64 + dy);
            let inside = (0..PIXELS as i64).contains(&x) && (0..PIXELS as i64).contains(&y);
            let near = ((dx * dx + dy * dy) as f32).sqrt() * METRES_PER_PIXEL <= SEEN_M;
            if inside && near && !seen[y as usize * PIXELS + x as usize] {
                newly.push(y as usize * PIXELS + x as usize);
            }
        }
    }
    newly
}

fn toggle(
    controls: Controls,
    chart: Option<ResMut<Chart>>,
    mut sheet: Single<&mut Visibility, With<Sheet>>,
) {
    let Some(mut chart) = chart else { return };
    if controls.went_down(Action::Map) {
        chart.open = !chart.open;
    }
    let wanted = if chart.open {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    if **sheet != wanted {
        **sheet = wanted;
    }
}

/// Puts you and your claim where they are on the map.
#[allow(clippy::type_complexity)]
fn place_marks(
    site: Res<Site>,
    claim: Single<&Claim, IsLocalPlayer>,
    player: Single<&Transform, IsLocalPlayer>,
    mut you: Single<&mut Node, (With<You>, Without<Yours>)>,
    yours: Single<(&mut Node, &mut Visibility), (With<Yours>, Without<You>)>,
) {
    let scale = SHOWN_PX / PIXELS as f32;
    if let Some((x, y)) = planet_to_pixel(&site, player.translation) {
        you.left = px(x as f32 * scale - 4.5);
        you.top = px(y as f32 * scale - 4.5);
    }
    let (mut yours, mut shown) = yours.into_inner();
    match claim.stone.and_then(|(_, at)| planet_to_pixel(&site, at)) {
        Some((x, y)) => {
            yours.left = px(x as f32 * scale - 7.0);
            yours.top = px(y as f32 * scale - 7.0);
            *shown = Visibility::Inherited;
        }
        None => *shown = Visibility::Hidden,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pixel_and_its_place_on_the_planet_agree() {
        let site_frame = Transform::from_xyz(0.0, 2000.0, 0.0);
        let to_pixel = |point: Vec3| {
            let local = site_frame
                .compute_affine()
                .inverse()
                .transform_point3(point);
            (
                local.x / METRES_PER_PIXEL + PIXELS as f32 * 0.5,
                local.z / METRES_PER_PIXEL + PIXELS as f32 * 0.5,
            )
        };
        let east = (30.0 - PIXELS as f32 * 0.5) * METRES_PER_PIXEL;
        let south = (90.0 - PIXELS as f32 * 0.5) * METRES_PER_PIXEL;
        let point = site_frame.transform_point(Vec3::new(east, 0.0, south));
        let (x, y) = to_pixel(point);
        assert!((x - 30.0).abs() < 1e-3 && (y - 90.0).abs() < 1e-3);
    }
}
