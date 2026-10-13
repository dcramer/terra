//! The role pieces spike (README.md): which way of giving each exposed face, edge and corner
//! of a cube a look chosen by its role and neighbours brings stone, logs and roofs on par
//! with Enshrouded's blending (D211, research/voxel-building-look.md)?
//!
//! Every shape (`shapes.rs`) is drawn by every candidate, side by side in one model per
//! shape that the kit sheet stands on the yard:
//!
//! 1. Today (`today.rs`): what the game draws.
//! 2. Role layers and masks (`layers.rs`): the same surface, each part of it in a material
//!    and a shade chosen by its role (`roles.rs`), no new geometry.
//! 3. Profiles and corner pieces (`profiles.rs`): 2 plus Blender-drawn profiles swept along
//!    role edges and Blender-modelled corner stones fitted to them, standing out up to an
//!    eighth of a cube (D192 as amended); and, for reference, flush within the cubes.
//! 4. Modules (`modules.rs`): every exposed face of a block kind a Blender-modelled module
//!    chosen by its role, modelled to the same profiles, standing out as 3's do.

mod glb;
mod layers;
mod masonry;
mod model;
mod modules;
mod profiles;
mod roles;
mod shapes;
mod today;

use glam::Vec3;
use std::collections::BTreeMap;
use terra_build::kit::{Kit, Kits};
use terra_build::relief::Relief;
use terra_build::{Material, Surface, Surfaces};

pub const ASSETS: &str = "../../assets";
/// How far apart the candidates stand, and the yard's radius, which they sit down on.
const SPACING_M: f32 = 12.0;
const YARD_RADIUS_M: f32 = 2000.0;

/// Surfaces by the folder of the material they are drawn in: a kit's material under
/// `assets/materials`, `plain`, or a spike texture under `tex/`.
pub type Drawn = BTreeMap<String, Surface>;

pub fn add(into: &mut Drawn, folder: String, from: Surface) {
    if from.positions.is_empty() {
        return;
    }
    let to = into.entry(folder).or_default();
    to.positions.extend(from.positions);
    to.normals.extend(from.normals);
    to.colours.extend(from.colours);
    to.uvs.extend(from.uvs);
}

/// One surface added to another.
pub fn add_to(to: &mut Surface, from: Surface) {
    to.positions.extend(from.positions);
    to.normals.extend(from.normals);
    to.colours.extend(from.colours);
    to.uvs.extend(from.uvs);
}

/// The folder a kit's material is drawn from.
pub fn folder(kit: &Kit, material: Material) -> String {
    if material == Material::Plain {
        return "plain".into();
    }
    kit.textures
        .get(&material)
        .map(|t| t.path.trim_start_matches("materials/").to_string())
        .unwrap_or_else(|| "plain".into())
}

pub fn add_surfaces(into: &mut Drawn, kit: &Kit, from: Surfaces) {
    for (material, surface) in from.0 {
        add(into, folder(kit, material), surface);
    }
}

/// The materials' height maps, by folder, read by ImageMagick.
pub struct Reliefs(BTreeMap<String, Relief>);

impl Reliefs {
    pub fn load(folders: &[&str]) -> Reliefs {
        let mut out = BTreeMap::new();
        for folder in folders {
            let path = format!("{ASSETS}/materials/{folder}/height.jpg");
            if !std::path::Path::new(&path).exists() {
                continue;
            }
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
            out.insert(
                folder.to_string(),
                Relief::new(width, height, data).expect(&path),
            );
        }
        Reliefs(out)
    }

    pub fn get(&self, folder: &str) -> Option<&Relief> {
        self.0.get(folder)
    }

    /// The height map of a kit's material, if it has one.
    pub fn of(&self, kit: &Kit, material: Material) -> Option<&Relief> {
        self.0.get(&folder(kit, material))
    }
}

fn read_kit(name: &str) -> Kit {
    let text = std::fs::read_to_string(format!("{ASSETS}/data/kits/{name}")).unwrap();
    ron::from_str(&text).unwrap()
}

/// The candidates, as letters for the camera and names for the sheets.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Candidate {
    Today,
    Layers,
    Profiles { overhang: bool },
    Modules { overhang: bool },
}

impl Candidate {
    fn code(self) -> &'static str {
        match self {
            Candidate::Today => "1",
            Candidate::Layers => "2",
            Candidate::Profiles { overhang: true } => "3",
            Candidate::Profiles { overhang: false } => "3f",
            Candidate::Modules { overhang: true } => "4",
            Candidate::Modules { overhang: false } => "4f",
        }
    }
}

/// The overhang is the main version (D192 as amended on 2026-10-13); 3 flush is kept for
/// reference, last.
const ALL: [Candidate; 5] = [
    Candidate::Today,
    Candidate::Layers,
    Candidate::Profiles { overhang: true },
    Candidate::Modules { overhang: true },
    Candidate::Profiles { overhang: false },
];

/// A patch of ground under a case whose ground is not level, in the ground's own texture,
/// its edges dropped down below the yard's.
fn ground_patch(case: &shapes::Case, low: Vec3, high: Vec3) -> Surface {
    let mut out = Surface::default();
    let (x0, x1, z0, z1) = (low.x - 2.5, high.x + 2.5, low.z - 3.0, high.z + 3.0);
    let n = 40;
    let at = |i: usize, j: usize| {
        let x = x0 + (x1 - x0) * i as f32 / n as f32;
        let z = z0 + (z1 - z0) * j as f32 / n as f32;
        let edge = i == 0 || j == 0 || i == n || j == n;
        let y = if edge {
            -1.0 + case.sink
        } else {
            (case.ground)(x, z)
        };
        Vec3::new(x, y, z)
    };
    for i in 0..n {
        for j in 0..n {
            let q = [at(i, j), at(i, j + 1), at(i + 1, j + 1), at(i + 1, j)];
            for t in [[0, 1, 2], [0, 2, 3]] {
                let c = t.map(|k| q[k]);
                let normal = (c[1] - c[0]).cross(c[2] - c[0]).normalize_or(Vec3::Y);
                out.positions.extend(c);
                out.normals.extend([normal; 3]);
                out.colours.extend([[1.0; 3]; 3]);
                out.uvs.extend(c.map(|p| [p.x / 3.0, p.z / 3.0]));
            }
        }
    }
    out
}

/// The ground's own texture for a patch: the second of the yard's stack, its grass.
fn ground_texture(out: &str) {
    let dir = format!("{out}/tex/ground");
    std::fs::create_dir_all(&dir).unwrap();
    for (from, to) in [("color", "color.jpg"), ("normal", "normal.jpg")] {
        let status = std::process::Command::new("convert")
            .args([
                &format!("{ASSETS}/materials/ground/{from}.jpg"),
                "-crop",
                "1024x1024+0+1024",
                &format!("{dir}/{to}"),
            ])
            .status()
            .unwrap();
        assert!(status.success());
    }
    // Rough, not metal: glTF's green and blue.
    let status = std::process::Command::new("convert")
        .args(["-size", "8x8", "xc:rgb(255,235,0)", &format!("{dir}/mr.png")])
        .status()
        .unwrap();
    assert!(status.success());
}

fn main() {
    let only: Vec<String> = std::env::args().skip(1).collect();
    let logs = read_kit("logs.ron");
    let stone = read_kit("stone.ron");
    let kits = Kits(vec![logs.clone(), stone.clone()]);
    // The role candidates' dressed stone: a worn, even limestone (Poly Haven's CC0
    // `worn_rock_natural_01`), finer than today's boulder, so dressed reads against rubble.
    let mut dressed = stone.clone();
    dressed.textures.insert(
        Material::Dressed,
        terra_build::kit::Texture {
            path: "materials/worn_rock_natural_01".into(),
            metres: [1.0, 1.0],
            depth: 0.0,
            reflectance: terra_build::kit::matte(),
            strips: None,
        },
    );
    let role_kits = Kits(vec![logs.clone(), dressed]);
    let reliefs = Reliefs::load(&[
        "stone_wall",
        "grey_stone_path",
        "rock_boulder_dry",
        "pine_bark",
        "rough_wood",
        "worn_rock_natural_01",
    ]);
    let out = format!("{ASSETS}/models/spike");
    std::fs::create_dir_all(&out).unwrap();
    ground_texture(&out);
    let door = model::Model::load(&format!("{ASSETS}/models/kits/Door.glb"), &out);
    let window = model::Model::load(&format!("{ASSETS}/models/kits/Window.glb"), &out);
    let pieces = profiles::Pieces::load();
    let set = modules::Set::load();
    // An empty model, to stand the spike's rows clear of the kit sheet's dug slope.
    let mut speck = Surface::default();
    for p in [Vec3::new(0.0, -5.0, 0.0), Vec3::new(0.01, -5.0, 0.0), Vec3::new(0.0, -5.0, 0.01)] {
        speck.positions.push(p);
        speck.normals.push(Vec3::Y);
        speck.colours.push([1.0; 3]);
        speck.uvs.push([0.0; 2]);
    }
    glb::write(
        &format!("{out}/RolePieces-spacer.glb"),
        "spacer",
        &[glb::Shape {
            name: "spacer".into(),
            at: Vec3::ZERO,
            surfaces: [("plain".to_string(), speck)].into_iter().collect(),
        }],
    );
    let mut counts = String::new();
    for case in shapes::cases() {
        if !only.is_empty() && !only.iter().any(|name| case.name.contains(name.as_str())) {
            continue;
        }
        let mut shapes_out = Vec::new();
        for (column, candidate) in ALL.into_iter().enumerate() {
            let mut drawn = match candidate {
                Candidate::Today => today::draw(&case, &kits, &reliefs, &door, &window),
                Candidate::Layers => layers::draw(&case, &role_kits, &reliefs, &door, &window),
                Candidate::Profiles { overhang } => profiles::draw(
                    &case, &role_kits, &reliefs, &door, &window, &pieces, overhang,
                ),
                Candidate::Modules { overhang } => modules::draw(
                    &case, &role_kits, &reliefs, &door, &window, &pieces, &set, overhang,
                ),
            };
            // Stood on the yard: its ground's height at its middle at the yard's.
            for s in drawn.values_mut() {
                for p in &mut s.positions {
                    p.y -= case.sink;
                }
            }
            let (mut low, mut high) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
            for (folder, s) in &drawn {
                if folder.starts_with("tex/Door") || folder.starts_with("tex/Window") {
                    continue;
                }
                for p in &s.positions {
                    low = low.min(*p);
                    high = high.max(*p);
                }
            }
            if case.name.contains("slope") {
                let mut patch = ground_patch(&case, low, high);
                for p in &mut patch.positions {
                    p.y -= case.sink;
                }
                add(&mut drawn, "tex/ground".into(), patch);
            }
            let centre = ((low + high) * 0.5).with_y(0.0);
            for s in drawn.values_mut() {
                for p in &mut s.positions {
                    *p -= centre;
                }
            }
            let triangles: usize = drawn
                .iter()
                .filter(|(f, _)| !f.starts_with("tex/"))
                .map(|(_, s)| s.positions.len() / 3)
                .sum();
            let draws = drawn.keys().filter(|f| !f.starts_with("tex/")).count();
            // The cubes, and their faces open to the air, which is what a surface or a
            // module set draws.
            let solid = |at: glam::IVec3| {
                case.base
                    .cube(at)
                    .is_some_and(|c| c.fill == terra_build::Fill::Solid)
            };
            let cubes: Vec<glam::IVec3> = case
                .base
                .cubes()
                .filter(|(_, c)| c.fill == terra_build::Fill::Solid)
                .map(|(at, _)| at)
                .collect();
            let faces: usize = cubes
                .iter()
                .map(|&at| {
                    roles::FACES
                        .iter()
                        .filter(|&&(axis, side)| !solid(at + roles::step(axis, side)))
                        .count()
                })
                .sum();
            counts.push_str(&format!(
                "{}\t{}\t{}\t{}\t{}\t{}\n",
                case.name,
                candidate.code(),
                triangles,
                draws,
                cubes.len(),
                faces
            ));
            let x = (column as f32 - 2.0) * SPACING_M;
            shapes_out.push(glb::Shape {
                name: format!("RP-{}-{}", candidate.code(), case.name),
                at: Vec3::new(x, -0.03 - x * x / (2.0 * YARD_RADIUS_M), 0.0),
                surfaces: drawn,
            });
        }
        let path = format!("{out}/RolePieces-{}.glb", case.name);
        let tris = glb::write(&path, &format!("RolePieces {}", case.name), &shapes_out);
        println!("{path}: triangles {tris:?}");
    }
    std::fs::write("out/counts.tsv", counts).unwrap();
}
