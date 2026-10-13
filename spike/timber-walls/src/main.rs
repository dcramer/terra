//! The timber walls spike (README.md): the same shapes of log cubes drawn four ways, each
//! written as a model the kit sheet stands in a row of its own.
//!
//! - R, today: the kit's Tripo log models, placed and stretched as the game places them.
//! - A, round logs: a tapered, bent tube per log, displaced by bark, end grain on its cut
//!   faces, chinking in the groove between courses.
//! - B, hewn timber: squared beams with rounded, hewn faces, chinking between courses,
//!   lapped at the corners where the two walls' beams cross.
//! - C, timber as surface: the block kinds' surface (`terra_build::surface`) with a log
//!   wall's material instead of stone.
//!
//! Every shape is laid by the game's own layout from the same cubes, and stone by the
//! stone kit's surface, so only the timber's drawing differs.

mod glb;
mod model;
mod pieces;

use glam::{IVec3, Vec3};
use std::collections::{BTreeMap, BTreeSet};
use terra_build::logs::{Kits, LogKit, Pattern, Piece};
use terra_build::{Base, BuildOp, Fill, Kind, Material, Panel, Surface, Surfaces};

/// A material's height map, a byte a texel, read by ImageMagick.
pub struct Relief {
    width: usize,
    height: usize,
    data: Vec<u8>,
}

impl Relief {
    pub fn load(folder: &str) -> Relief {
        let path = format!("{ASSETS}/materials/{folder}/height.jpg");
        let size = std::process::Command::new("convert")
            .args([&path, "-format", "%w %h", "info:"])
            .output()
            .expect("ImageMagick's convert");
        let size = String::from_utf8_lossy(&size.stdout);
        let mut parts = size.split_whitespace().map(|s| s.parse::<usize>().unwrap());
        let (width, height) = (parts.next().unwrap(), parts.next().unwrap());
        let data = std::process::Command::new("convert")
            .args([&path, "-colorspace", "gray", "-depth", "8", "gray:-"])
            .output()
            .unwrap()
            .stdout;
        assert_eq!(data.len(), width * height, "{path}");
        Relief {
            width,
            height,
            data,
        }
    }

    /// The height, from 0 to 1, where the texture is at a point, repeating.
    pub fn at(&self, uv: [f32; 2]) -> f32 {
        let x = uv[0].rem_euclid(1.0) * self.width as f32 - 0.5;
        let y = uv[1].rem_euclid(1.0) * self.height as f32 - 0.5;
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (x - x0, y - y0);
        let texel = |i: f32, j: f32| {
            let i = (i as i64).rem_euclid(self.width as i64) as usize;
            let j = (j as i64).rem_euclid(self.height as i64) as usize;
            self.data[j * self.width + i] as f32 / 255.0
        };
        let top = texel(x0, y0) * (1.0 - fx) + texel(x0 + 1.0, y0) * fx;
        let bottom = texel(x0, y0 + 1.0) * (1.0 - fx) + texel(x0 + 1.0, y0 + 1.0) * fx;
        top * (1.0 - fy) + bottom * fy
    }
}

/// How far apart the four ways stand, and the yard's radius, which they sit down on.
const SPACING_M: f32 = 11.0;
const YARD_RADIUS_M: f32 = 2000.0;
pub const ASSETS: &str = "../../assets";
const LOGS: Kind = Kind(1);
const STONE: Kind = Kind(2);

fn wall(x: i32, z: i32, panel: Panel, kind: Kind) -> BuildOp {
    BuildOp::SetWall {
        tile: [x, 0, z],
        panel: Some(panel),
        kind,
    }
}

fn east_west(from: i32, to: i32, z: i32, kind: Kind) -> Vec<BuildOp> {
    (from..=to)
        .map(|x| wall(x, z, Panel::Solid, kind))
        .collect()
}

fn north_south(from: i32, to: i32, x: i32, kind: Kind) -> Vec<BuildOp> {
    (from..=to)
        .map(|z| wall(x, z, Panel::Solid, kind))
        .collect()
}

/// The shapes, in cubes: a name, the operations, and how much of the timber is in.
fn shapes() -> Vec<(&'static str, Vec<BuildOp>, f32)> {
    let mut openings = east_west(0, 11, 0, LOGS);
    openings.extend((2..=3).map(|x| wall(x, 0, Panel::Door, LOGS)));
    openings.extend((7..=8).map(|x| wall(x, 0, Panel::WINDOW, LOGS)));
    vec![
        ("straight", east_west(0, 5, 0, LOGS), 1.0),
        (
            "L",
            [east_west(0, 5, 0, LOGS), north_south(1, 5, 0, LOGS)].concat(),
            1.0,
        ),
        (
            "T",
            [east_west(0, 9, 0, LOGS), north_south(1, 5, 4, LOGS)].concat(),
            1.0,
        ),
        ("openings", openings, 1.0),
        (
            "part-built",
            [east_west(0, 7, 0, LOGS), north_south(1, 4, 0, LOGS)].concat(),
            0.45,
        ),
        (
            "stone-L",
            [east_west(0, 5, 0, LOGS), north_south(1, 5, 0, STONE)].concat(),
            1.0,
        ),
        (
            "stone-T",
            [east_west(0, 9, 0, STONE), north_south(1, 5, 4, LOGS)].concat(),
            1.0,
        ),
    ]
}

fn read_kit(name: &str) -> LogKit {
    let text = std::fs::read_to_string(format!("{ASSETS}/data/kits/{name}")).unwrap();
    ron::from_str(&text).unwrap()
}

/// The folder each of a kit's materials is drawn from.
fn folder(kit: &LogKit, material: Material) -> String {
    kit.textures
        .get(&material)
        .map(|t| t.path.trim_start_matches("materials/").to_string())
        .unwrap_or_else(|| "rough_wood".into())
}

fn add(into: &mut BTreeMap<String, Surface>, folder: String, from: Surface) {
    let to = into.entry(folder).or_default();
    to.positions.extend(from.positions);
    to.normals.extend(from.normals);
    to.colours.extend(from.colours);
    to.uvs.extend(from.uvs);
}

fn add_all(into: &mut BTreeMap<String, Surface>, kit: &LogKit, from: Surfaces) {
    for (material, surface) in from.0 {
        add(into, folder(kit, material), surface);
    }
}

/// A block kind's cubes as its surface, as the game draws them (`draw_blocks`).
fn blocks(
    base: &Base,
    kind: Kind,
    cubes: &BTreeSet<[i32; 3]>,
    look: &terra_build::surface::Look,
    relief: &Relief,
    shift: [f32; 2],
    colour: [f32; 3],
) -> Surface {
    let hides = |at: IVec3| {
        base.cube(at).is_some_and(|cube| {
            cube.fill != Fill::Open
                && cube.kind != kind
                && (!base.in_wall(at) || cube.kind == Kind::PLAIN)
        })
    };
    let height = |uv: [f32; 2]| relief.at([uv[0] + shift[0], uv[1] + shift[1]]);
    let mut out = Surface::default();
    terra_build::surface::draw(cubes, &hides, look, &height, colour, &mut out);
    for uv in &mut out.uvs {
        uv[0] += shift[0];
        uv[1] += shift[1];
    }
    out
}

#[derive(Clone, Copy, PartialEq)]
enum Way {
    Today,
    Round,
    Hewn,
    Surface,
}

fn main() {
    let logs = read_kit("logs.ron");
    let stone = read_kit("stone.ron");
    let stone_relief = Relief::load("stone_wall");
    let bark = Relief::load(pieces::BARK);
    let beams = Relief::load(pieces::LOG_WALL);
    let out = format!("{ASSETS}/models/spike");
    std::fs::create_dir_all(&out).unwrap();
    let tripo: Vec<model::Model> = ["Log", "Log2"]
        .map(|name| model::Model::load(&format!("{out}/{name}.glb"), &out))
        .into();
    let long_log = model::Model::load(&format!("{out}/LogLong.glb"), &out);
    let door = model::Model::load(&format!("{ASSETS}/models/kits/Door.glb"), &out);
    let window = model::Model::load(&format!("{ASSETS}/models/kits/Window.glb"), &out);
    let mut by_shape: Vec<(String, Vec<glb::Shape>)> = Vec::new();
    for (column, (letter, way)) in [
        ("R", Way::Today),
        ("A", Way::Round),
        ("B", Way::Hewn),
        ("C", Way::Surface),
    ]
    .into_iter()
    .enumerate()
    {
        // C lays the log cubes as blocks, as the stone kit does.
        let mut timber = logs.clone();
        if way == Way::Surface {
            timber.pattern = Pattern::Masonry;
        }
        let kits = Kits(vec![timber.clone(), stone.clone()]);
        for (slot, (name, ops, built)) in shapes().into_iter().enumerate() {
            let mut base = Base::default();
            for op in ops {
                base.apply(op);
            }
            let (_, layouts) = base.mesh_with_kits(&kits);
            let pieces = &layouts[0].pieces;
            let done = ((pieces.len() as f32) * built).round() as usize;
            let mut surfaces: BTreeMap<String, Surface> = BTreeMap::new();
            // The timber.
            let mut blocks_in = BTreeSet::new();
            for piece in &pieces[..done] {
                let mut s = Surfaces::default();
                match (way, piece) {
                    (
                        _,
                        Piece::Opening {
                            min,
                            max,
                            along_x,
                            door: is_door,
                        },
                    ) => {
                        let m = if *is_door { &door } else { &window };
                        let x = if *along_x { Vec3::X } else { Vec3::Z };
                        let size = *max - *min;
                        let (width, depth) = if *along_x {
                            (size.x, size.z)
                        } else {
                            (size.z, size.x)
                        };
                        let at = m.fill(
                            (*min + *max) * 0.5,
                            x,
                            Vec3::Y,
                            Vec3::new(width, size.y, depth),
                        );
                        m.place(at, surfaces.entry(m.key.clone()).or_default());
                    }
                    (
                        Way::Today,
                        Piece::Log {
                            from,
                            to,
                            split,
                            seed,
                            ..
                        },
                    ) => {
                        let length = from.distance(*to);
                        let m = if length > 3.6 {
                            &long_log
                        } else {
                            &tripo[*seed as usize % 2]
                        };
                        let at = m.log(*from, *to, *split, *seed, timber.diameter);
                        m.place(at, surfaces.entry(m.key.clone()).or_default());
                        terra_build::logs::draw_chinking(piece, &timber, &mut s);
                    }
                    (Way::Round, Piece::Log { .. }) => {
                        pieces::round(piece, &timber, &bark, &mut surfaces)
                    }
                    (Way::Hewn, Piece::Log { .. }) => pieces::hewn(piece, &timber, &mut surfaces),
                    (Way::Surface, Piece::Block { cube }) => {
                        blocks_in.insert(cube.to_array());
                    }
                    _ => terra_build::logs::draw(piece, &timber, &mut s),
                }
                add_all(&mut surfaces, &timber, s);
            }
            if way == Way::Surface {
                let look = terra_build::surface::Look {
                    bevel: pieces::LOG_WALL_BEVEL,
                    relief: pieces::LOG_WALL_RELIEF,
                    steps: 10,
                    metres: pieces::LOG_WALL_METRES,
                };
                add(
                    &mut surfaces,
                    pieces::LOG_WALL.into(),
                    blocks(
                        &base,
                        LOGS,
                        &blocks_in,
                        &look,
                        &beams,
                        [0.0, pieces::LOG_WALL_SHIFT],
                        pieces::LOG_WALL_TINT,
                    ),
                );
            }
            // The stone, as the game draws it.
            let mut stone_cubes = BTreeSet::new();
            for piece in &layouts[1].pieces {
                if let Piece::Block { cube } = piece {
                    stone_cubes.insert(cube.to_array());
                } else {
                    let mut s = Surfaces::default();
                    terra_build::logs::draw(piece, &stone, &mut s);
                    add_all(&mut surfaces, &stone, s);
                }
            }
            if !stone_cubes.is_empty() {
                let look = terra_build::surface::Look {
                    bevel: stone.bevel,
                    relief: stone.relief,
                    steps: 10,
                    metres: stone.textures[&Material::Masonry].metres,
                };
                add(
                    &mut surfaces,
                    "stone_wall".into(),
                    blocks(
                        &base,
                        STONE,
                        &stone_cubes,
                        &look,
                        &stone_relief,
                        [0.0, 0.0],
                        [1.0; 3],
                    ),
                );
            }
            // Centred on its node, its foot a little into the ground.
            let (mut low, mut high) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
            for s in surfaces.values() {
                for p in &s.positions {
                    low = low.min(*p);
                    high = high.max(*p);
                }
            }
            let centre = ((low + high) * 0.5).with_y(0.0);
            for s in surfaces.values_mut() {
                for p in &mut s.positions {
                    *p -= centre;
                }
            }
            // The four ways side by side, east to west, each down on the yard's round.
            let x = (column as f32 - 1.5) * SPACING_M;
            if by_shape.len() <= slot {
                by_shape.push((name.to_string(), Vec::new()));
            }
            by_shape[slot].1.push(glb::Shape {
                name: format!("TW-{letter}-{name}"),
                at: Vec3::new(x, -0.03 - x * x / (2.0 * YARD_RADIUS_M), 0.0),
                surfaces,
            });
        }
    }
    for (name, shapes) in by_shape {
        let path = format!("{out}/Timber-{name}.glb");
        let counts = glb::write(&path, &format!("Timber {name}"), &shapes);
        println!("{path}: triangles R, A, B, C {counts:?}");
    }
}
