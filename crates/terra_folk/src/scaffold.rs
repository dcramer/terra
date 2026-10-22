//! The scaffold round a building rising in a settlement's time-lapse (D381): put up once its
//! walls stand higher than a builder can reach from the ground, a lift at a time as they
//! rise, and struck when the building is finished, as a house of logs or stone is built:
//! standards (the upright poles) a little out from the wall, ledgers along them at each lift,
//! boards laid between them and the wall for the builders to stand on, a brace across each
//! side and a guard rail along the top lift. Drawn in bark and weathered boards, from the
//! building's own plan and what of its walls is up, so it reads from high as building, not as
//! a finished house.

use bevy::{
    asset::RenderAssetUsages,
    image::{ImageAddressMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor},
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use terra_build::{Base, CELL_M, State};

/// How far out of the wall the standards stand, and how far apart along it at most.
const OUT_M: f32 = 1.0;
const BAY_M: f32 = 2.4;
/// How far apart the lifts are, the first over the ground, and how far over a lift its
/// guard rail is.
const LIFT_M: f32 = 1.5;
const RAIL_M: f32 = 1.0;
/// A wall is scaffolded once it is this high: higher than a builder lays from the ground.
const FROM_M: f32 = 1.2;
/// A standard's, a ledger's and a board's section, metres.
const STANDARD_M: f32 = 0.13;
const LEDGER_M: f32 = 0.09;
const BOARD: (f32, f32) = (0.24, 0.045);
/// Boards a lift, side by side, from the wall out.
const BOARDS: usize = 3;
/// Less than this across, a building is too small to scaffold (a well, a stall).
const SMALLEST_M: f32 = 2.5;

/// Where a building's walls stand, in its own metres: the box of its walls' plan across, its
/// floor and its eaves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Walls {
    pub low: Vec2,
    pub high: Vec2,
    pub floor_m: f32,
    pub eaves_m: f32,
}

impl Walls {
    /// A building's walls from its plan as it will be: every cube but its roof's.
    pub fn of(whole: &Base) -> Option<Walls> {
        let cubes: Vec<IVec3> = whole
            .cubes()
            .filter(|(_, cube)| !cube.kind.is_roof())
            .map(|(at, _)| at)
            .collect();
        let (low, high) = cubes
            .iter()
            .fold((IVec3::MAX, IVec3::MIN), |(lo, hi), &at| {
                (lo.min(at), hi.max(at + IVec3::ONE))
            });
        if low.x > high.x {
            return None;
        }
        let eaves = eaves_layer(cubes.iter().map(|at| {
            let edge = at.x == low.x || at.x == high.x - 1 || at.z == low.z || at.z == high.z - 1;
            (at.y, edge)
        }))?;
        let walls = Walls {
            low: Vec2::new(low.x as f32, low.z as f32) * CELL_M,
            high: Vec2::new(high.x as f32, high.z as f32) * CELL_M,
            // From the ground, not a footing sunk under it on a slope.
            floor_m: low.y.max(0) as f32 * CELL_M,
            eaves_m: (eaves + 1) as f32 * CELL_M,
        };
        let across = walls.high - walls.low;
        (across.min_element() >= SMALLEST_M && walls.eaves_m - walls.floor_m >= TALLEST_M)
            .then_some(walls)
    }
}

/// The layer its eaves are on: the highest whose walls run round most of the ring its lowest
/// do, not a gable's peak or a chimney's, from each cube's layer and whether it is on the
/// footprint's edge.
fn eaves_layer(cubes: impl Iterator<Item = (i32, bool)>) -> Option<i32> {
    let mut ring: std::collections::BTreeMap<i32, u32> = Default::default();
    for (y, edge) in cubes {
        *ring.entry(y).or_default() += u32::from(edge);
    }
    let most = ring.values().copied().max()?;
    ring.iter()
        .filter(|&(_, &count)| count as f32 >= most as f32 * 0.75)
        .map(|(&y, _)| y)
        .max()
}

/// A building whose walls stand lower than this is built from the ground and trestles; one
/// taller, a storey and more, is scaffolded.
const TALLEST_M: f32 = 3.4;

/// How high a building's walls are built, in its own metres: the top of the highest of its
/// layers, from the lowest up, each mostly laid, of what it plans that is not its roof's. Not
/// its highest cube: a frame's posts stand to its eaves before its walls are begun.
pub fn built_to(base: &Base) -> Option<f32> {
    top_of(
        base.cubes()
            .filter(|(_, cube)| !cube.kind.is_roof())
            .map(|(at, cube)| (at.y, cube.state == State::Built)),
    )
}

/// The top of the highest of the layers, from the lowest up, each mostly laid, of cubes
/// given by their layer and whether they are laid.
fn top_of(cubes: impl Iterator<Item = (i32, bool)>) -> Option<f32> {
    let mut layers: std::collections::BTreeMap<i32, (u32, u32)> = Default::default();
    for (y, laid) in cubes {
        let layer = layers.entry(y).or_default();
        layer.0 += 1;
        layer.1 += u32::from(laid);
    }
    let mut top = None;
    for (y, (all, built)) in layers {
        if (built as f32) < all as f32 * MOSTLY {
            break;
        }
        top = Some((y + 1) as f32 * CELL_M);
    }
    top
}

/// A layer is built once this share of it is laid.
const MOSTLY: f32 = 0.6;

/// How many lifts a scaffold has with its walls built to `top_m`: none until they are higher
/// than a builder reaches from the ground, then one a lift reached, below the eaves.
pub fn lifts(walls: &Walls, top_m: f32) -> u32 {
    if top_m - walls.floor_m < FROM_M {
        return 0;
    }
    let mut count = 0;
    while {
        let next = walls.floor_m + LIFT_M * (count + 1) as f32;
        next <= top_m + 0.05 && next <= walls.eaves_m - 0.4
    } {
        count += 1;
    }
    count.max(1)
}

/// A straight piece of the scaffold, from one end to the other, so wide and so deep.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Piece {
    pub from: Vec3,
    pub to: Vec3,
    pub wide: f32,
    pub deep: f32,
    /// A board, drawn in boards' wood, or a pole, in bark.
    pub board: bool,
}

/// The scaffold's pieces round `walls` with `lifts` lifts.
pub fn pieces(walls: &Walls, lifts: u32) -> Vec<Piece> {
    let mut out = Vec::new();
    if lifts == 0 {
        return out;
    }
    let (low, high) = (
        walls.low - Vec2::splat(OUT_M),
        walls.high + Vec2::splat(OUT_M),
    );
    let corners = [
        Vec2::new(low.x, low.y),
        Vec2::new(high.x, low.y),
        Vec2::new(high.x, high.y),
        Vec2::new(low.x, high.y),
    ];
    let top_lift = walls.floor_m + LIFT_M * lifts as f32;
    let standing_to = top_lift + RAIL_M + 0.2;
    let at = |p: Vec2, y: f32| Vec3::new(p.x, y, p.y);
    for side in 0..4 {
        let (a, b) = (corners[side], corners[(side + 1) % 4]);
        let length = a.distance(b);
        let bays = (length / BAY_M).ceil().max(1.0) as usize;
        let along = (b - a) / length;
        // Into the building from this side.
        let inward = Vec2::new(-along.y, along.x);
        // The standards, from the corner to the one before the next corner.
        for i in 0..bays {
            let p = a + (b - a) * (i as f32 / bays as f32);
            out.push(Piece {
                from: at(p, walls.floor_m - 0.3),
                to: at(p, standing_to),
                wide: STANDARD_M,
                deep: STANDARD_M,
                board: false,
            });
        }
        // At each lift a ledger along the standards and boards from it to the wall.
        for lift in 1..=lifts {
            let y = walls.floor_m + LIFT_M * lift as f32;
            out.push(Piece {
                from: at(a, y - LEDGER_M),
                to: at(b, y - LEDGER_M),
                wide: LEDGER_M,
                deep: LEDGER_M,
                board: false,
            });
            for board in 0..BOARDS {
                let into = inward * (0.12 + board as f32 * (BOARD.0 + 0.03) + BOARD.0 * 0.5);
                out.push(Piece {
                    from: at(a + along * 0.1 + into, y + BOARD.1 * 0.5),
                    to: at(b - along * 0.1 + into, y + BOARD.1 * 0.5),
                    wide: BOARD.0,
                    deep: BOARD.1,
                    board: true,
                });
            }
        }
        // The guard rail along the top lift, and a brace from the foot of the first bay to the
        // top lift at the next standard.
        out.push(Piece {
            from: at(a, top_lift + RAIL_M),
            to: at(b, top_lift + RAIL_M),
            wide: LEDGER_M,
            deep: LEDGER_M,
            board: false,
        });
        let next = a + (b - a) * (1.0 / bays as f32);
        out.push(Piece {
            from: at(a - inward * 0.12, walls.floor_m),
            to: at(next - inward * 0.12, top_lift),
            wide: LEDGER_M,
            deep: LEDGER_M,
            board: false,
        });
    }
    out
}

/// The pieces of one wood as a mesh: each a box, its texture along it a metre a repeat.
pub fn mesh(pieces: &[Piece], board: bool) -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    for piece in pieces.iter().filter(|p| p.board == board) {
        let length = piece.from.distance(piece.to);
        if length < 1.0e-3 {
            continue;
        }
        let along = (piece.to - piece.from) / length;
        let side = if along.y.abs() > 0.9 {
            Vec3::X
        } else {
            along.cross(Vec3::Y).normalize()
        };
        let up = side.cross(along).normalize();
        let (w, d) = (side * piece.wide * 0.5, up * piece.deep * 0.5);
        // Four long faces, then the two ends.
        let faces = [(w, d), (d, -w), (-w, -d), (-d, w)];
        for (out, across) in faces {
            let n = out.normalize();
            let first = positions.len() as u32;
            let width = across.length() * 2.0;
            for (end, s) in [(piece.from, 0.0), (piece.to, length)] {
                for (k, t) in [(-1.0, 0.0), (1.0, width)] {
                    positions.push((end + out + across * k).to_array());
                    normals.push(n.to_array());
                    uvs.push([s, t]);
                }
            }
            indices.extend([first, first + 2, first + 1, first + 1, first + 2, first + 3]);
        }
        for (end, n) in [(piece.from, -along), (piece.to, along)] {
            let first = positions.len() as u32;
            for (a, b) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                positions.push((end + w * a + d * b).to_array());
                normals.push(n.to_array());
                uvs.push([a * piece.wide * 0.5, b * piece.deep * 0.5]);
            }
            indices.extend([first, first + 1, first + 2, first, first + 2, first + 3]);
        }
    }
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    // Faces wound either way are drawn the same, so the normals decide how each is lit.
    mesh
}

/// The scaffold's two woods: its poles in bark, its boards weathered planks.
#[derive(Resource, Clone)]
pub struct ScaffoldLooks {
    pub poles: Handle<StandardMaterial>,
    pub boards: Handle<StandardMaterial>,
}

impl ScaffoldLooks {
    pub fn new(assets: &AssetServer, materials: &mut Assets<StandardMaterial>) -> Self {
        let load = |path: String, srgb: bool| -> Handle<Image> {
            assets
                .load_builder()
                .with_settings(move |settings: &mut ImageLoaderSettings| {
                    settings.is_srgb = srgb;
                    settings.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                        address_mode_u: ImageAddressMode::Repeat,
                        address_mode_v: ImageAddressMode::Repeat,
                        ..ImageSamplerDescriptor::linear()
                    });
                })
                .load(path)
        };
        let mut made = |folder: &str, tint: Color| {
            materials.add(StandardMaterial {
                base_color: tint,
                base_color_texture: Some(load(format!("materials/{folder}/color.jpg"), true)),
                normal_map_texture: Some(load(format!("materials/{folder}/normal.jpg"), false)),
                perceptual_roughness: 0.92,
                reflectance: 0.2,
                double_sided: true,
                cull_mode: None,
                ..default()
            })
        };
        ScaffoldLooks {
            poles: made("pine_bark", Color::srgb(0.95, 0.9, 0.85)),
            boards: made("weathered_planks", Color::srgb(1.0, 0.95, 0.88)),
        }
    }
}

/// A building's scaffold as it stands: its walls, how many lifts, and what draws it.
/// Where each scaffolded building's top lift is, so its builders at work stand on its boards
/// rather than on the grass under them (`townsfolk::place`): by the town's building index, its
/// frame, its walls and how many lifts are up.
#[derive(Resource, Default)]
pub struct Decks(pub bevy::platform::collections::HashMap<usize, (Transform, Walls, u32)>);

/// How far out of the wall a builder on the boards stands: on the middle of them.
const ON_BOARDS_M: f32 = 0.42;
/// How far from the walls a builder at work is taken up onto the boards; further, they work
/// at the stacked logs on the ground.
const TAKEN_UP_M: f32 = 4.0;

/// Where a builder standing at `at` (the building's own metres, across) works on its top lift:
/// the boards along the nearest wall, and the way to that wall; none if they stand too far
/// off, or inside its walls.
pub fn on_deck(walls: &Walls, lifts: u32, at: Vec2) -> Option<(Vec3, Vec2)> {
    if lifts == 0 {
        return None;
    }
    let inside = at.cmpgt(walls.low).all() && at.cmplt(walls.high).all();
    let nearest = at.clamp(walls.low, walls.high);
    let off = at - nearest;
    if inside || off.length() > TAKEN_UP_M {
        return None;
    }
    // Out from the nearest wall square on, not round a corner, where no board is.
    let out = if off.x.abs() >= off.y.abs() {
        Vec2::new(off.x.signum(), 0.0)
    } else {
        Vec2::new(0.0, off.y.signum())
    };
    let along = nearest.clamp(walls.low + Vec2::splat(0.4), walls.high - Vec2::splat(0.4));
    let on = along * out.abs().yx() + nearest * out.abs() + out * ON_BOARDS_M;
    let y = walls.floor_m + LIFT_M * lifts as f32;
    Some((Vec3::new(on.x, y, on.y), -out))
}

#[derive(Component, Default)]
pub struct Scaffolded {
    walls: Option<Option<Walls>>,
    lifts: u32,
    drawn: Vec<Entity>,
    /// When, in the time-lapse's seconds, its lifts last changed.
    changed_t: f32,
}

/// A finished building's scaffold is struck a lift at a time, from the top, one every so many
/// of the time-lapse's seconds, as it is taken down, not gone at once; a tall one's lifts
/// faster, so all of it is down within `STRUCK_S`: a lift every 1.2 s, a wall tower's nine
/// stood ten seconds past its finish, still up at the film's end.
const STRIKE_S: f32 = 1.2;
const STRUCK_S: f32 = 2.4;

/// How often, in frames, each rising building's scaffold is looked at again.
const EVERY: u32 = 6;

/// Puts up, raises and strikes each rising building's scaffold as its walls go up.
#[allow(clippy::too_many_arguments)]
pub fn scaffold(
    mut commands: Commands,
    rising: Res<crate::rising::Rising>,
    looks: Option<Res<ScaffoldLooks>>,
    assets: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut builds: Query<(
        Entity,
        &terra_bases::Built,
        &crate::rising::Rises,
        Option<&mut Scaffolded>,
    )>,
    mut frame: Local<u32>,
    decks: Option<ResMut<Decks>>,
) {
    let Some(mut decks) = decks else {
        commands.init_resource::<Decks>();
        return;
    };
    *frame += 1;
    if !(*frame).is_multiple_of(EVERY) {
        return;
    }
    let Some(looks) = looks else {
        commands.insert_resource(ScaffoldLooks::new(&assets, &mut materials));
        return;
    };
    for (entity, built, rise, scaffolded) in &mut builds {
        let Some(mut scaffolded) = scaffolded else {
            commands.entity(entity).insert(Scaffolded::default());
            continue;
        };
        let up = rising.up.get(rise.index).copied().unwrap_or(true);
        let wanted = if up {
            let all = match scaffolded.walls {
                Some(Some(walls)) => lifts(&walls, walls.eaves_m).max(1),
                _ => 1,
            };
            let every = STRIKE_S.min(STRUCK_S / all as f32);
            if scaffolded.lifts == 0 || rising.t < scaffolded.changed_t + every {
                continue;
            }
            scaffolded.lifts - 1
        } else if !rise.planned {
            0
        } else {
            let walls = *scaffolded
                .walls
                .get_or_insert_with(|| Walls::of(&rise.whole));
            match (walls, built_to(&built.base)) {
                (Some(walls), Some(top)) => lifts(&walls, top),
                _ => 0,
            }
        };
        if wanted == scaffolded.lifts {
            continue;
        }
        scaffolded.lifts = wanted;
        scaffolded.changed_t = rising.t;
        for drawn in scaffolded.drawn.drain(..) {
            commands.entity(drawn).despawn();
        }
        let Some(Some(walls)) = scaffolded.walls else {
            continue;
        };
        if wanted == 0 {
            decks.0.remove(&rise.index);
            continue;
        }
        decks.0.insert(rise.index, (built.frame, walls, wanted));
        let all = pieces(&walls, wanted);
        for (board, material) in [(false, &looks.poles), (true, &looks.boards)] {
            let drawn = commands
                .spawn((
                    Name::new(format!("Scaffold of building {}", rise.index)),
                    Mesh3d(meshes.add(mesh(&all, board))),
                    MeshMaterial3d(material.clone()),
                    built.frame,
                ))
                .id();
            scaffolded.drawn.push(drawn);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn house() -> Walls {
        Walls {
            low: Vec2::new(0.0, 0.0),
            high: Vec2::new(8.0, 6.0),
            floor_m: 0.0,
            eaves_m: 5.0,
        }
    }

    /// A builder beside a wall stands on the top lift's boards out from it, facing it; one
    /// inside the walls, or off by the stacked logs, stays on the ground.
    #[test]
    fn a_builder_by_a_scaffolded_wall_stands_on_its_boards_facing_it() {
        let walls = house();
        let (on, facing) = on_deck(&walls, 2, Vec2::new(3.0, -1.2)).expect("by the north wall");
        assert!((on.y - 2.0 * LIFT_M).abs() < 1e-4, "{on}");
        assert!(
            (on.z + ON_BOARDS_M).abs() < 1e-4 && (on.x - 3.0).abs() < 1e-4,
            "{on}"
        );
        assert_eq!(facing, Vec2::new(0.0, 1.0));
        let (on, facing) = on_deck(&walls, 1, Vec2::new(9.5, 5.9)).expect("by the east wall");
        assert!(on.x > walls.high.x && on.x < walls.high.x + OUT_M, "{on}");
        assert!(
            on.z < walls.high.y,
            "on a board along the wall, not past its corner: {on}"
        );
        assert_eq!(facing, Vec2::new(-1.0, 0.0));
        assert!(on_deck(&walls, 2, Vec2::new(4.0, 3.0)).is_none(), "inside");
        assert!(
            on_deck(&walls, 2, Vec2::new(4.0, -5.0)).is_none(),
            "at the logs"
        );
        assert!(
            on_deck(&walls, 0, Vec2::new(3.0, -1.2)).is_none(),
            "no scaffold"
        );
    }

    /// No scaffold while a builder lays from the ground; a lift for each one the walls have
    /// reached, never one at or over the eaves.
    #[test]
    fn a_lift_goes_up_as_the_walls_reach_it() {
        let walls = house();
        assert_eq!(lifts(&walls, 0.5), 0);
        assert_eq!(lifts(&walls, 1.0), 0);
        assert_eq!(lifts(&walls, 1.6), 1);
        assert_eq!(lifts(&walls, 2.9), 1);
        assert_eq!(lifts(&walls, 3.0), 2);
        assert_eq!(lifts(&walls, 5.0), 3);
        assert_eq!(lifts(&walls, 9.0), 3);
    }

    /// Every standard stands outside the walls, clear of them, and every board between the
    /// standards and the wall, at a lift's height; and a scaffold of more lifts is taller.
    #[test]
    fn the_scaffold_stands_round_the_walls_not_in_them() {
        let walls = house();
        let inside = |p: Vec3| {
            p.x > walls.low.x - 0.01
                && p.x < walls.high.x + 0.01
                && p.z > walls.low.y - 0.01
                && p.z < walls.high.y + 0.01
        };
        let two = pieces(&walls, 2);
        assert!(!two.is_empty());
        for piece in &two {
            if piece.board {
                let middle = (piece.from + piece.to) * 0.5;
                assert!(!inside(middle), "{piece:?}");
                // Within the standards' line.
                assert!(
                    middle.x > walls.low.x - OUT_M - 0.01 && middle.x < walls.high.x + OUT_M + 0.01
                );
                let lift = (middle.y - walls.floor_m) / LIFT_M;
                assert!((lift - lift.round()).abs() < 0.05, "{}", middle.y);
            } else {
                assert!(!inside(piece.from) || !inside(piece.to), "{piece:?}");
            }
        }
        let tallest = |pieces: &[Piece]| {
            pieces
                .iter()
                .map(|p| p.from.y.max(p.to.y))
                .fold(f32::MIN, f32::max)
        };
        assert!(tallest(&pieces(&walls, 3)) > tallest(&two) + 1.0);
        assert!(pieces(&walls, 0).is_empty());
    }

    /// The walls' height is their courses', not their tallest post's: a frame's posts stand
    /// to the eaves before a course is laid between them.
    #[test]
    fn the_walls_are_as_high_as_their_courses_not_their_posts() {
        // Four layers of a wall of 20 cubes each; its four posts to the eaves at layer 9.
        let mut cubes = Vec::new();
        for y in 0..10 {
            for i in 0..20 {
                let post = i < 4;
                let laid = post || y < 4;
                cubes.push((y, laid));
            }
        }
        assert_eq!(top_of(cubes.into_iter()), Some(4.0 * CELL_M));
        assert_eq!(top_of(std::iter::empty()), None);
        assert_eq!(top_of([(0, false), (0, false)].into_iter()), None);
    }

    /// The eaves are where the walls run all round, not a gable's peak over them.
    #[test]
    fn the_eaves_are_under_the_gables() {
        // A ring of 20 cubes a layer to layer 5, then gables of 2 cubes a layer to layer 9.
        let mut cubes = Vec::new();
        for y in 0..6 {
            cubes.extend((0..20).map(|_| (y, true)));
            cubes.extend((0..30).map(|_| (y, false)));
        }
        for y in 6..10 {
            cubes.extend((0..2).map(|_| (y, true)));
        }
        assert_eq!(eaves_layer(cubes.into_iter()), Some(5));
    }

    /// Each piece is a closed box: six faces, four corners each.
    #[test]
    fn each_piece_is_a_box() {
        let walls = house();
        let all = pieces(&walls, 1);
        let poles = all.iter().filter(|p| !p.board).count();
        let mesh = mesh(&all, false);
        assert_eq!(mesh.count_vertices(), poles * 24);
    }
}
