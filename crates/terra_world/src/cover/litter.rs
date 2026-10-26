//! Litter: fallen leaves, needles, twigs and cones lying on the ground near the camera, as
//! shipped games scatter small debris over their terrain's texture (Sons of the Forest's and
//! Far Cry's leaf and needle decals, Horizon Zero Dawn's ground debris), so the floor has
//! depth up close rather than reading as a picture with plants stood on it.
//!
//! - **Cards lying flat.** Each piece is a card of the litter atlas (`assets/data/litter.ron`,
//!   `textures.ts cards` from scanned leaves, `litter` recipes): a heap of fallen oak or
//!   beech leaves, needles, needles with a cone, cones, twigs, or a patch of ivy creeping over
//!   the leaves. It lies on the ground a little humped, its middle lifted a few centimetres
//!   over its edge, turned any way.
//! - **By what the ground is.** Leaves where the cover says fallen leaves, needles where it
//!   says needles, twigs among both, ivy in drifts under broadleaves; none where grass is thick or on bare paths but for a few
//!   that drift there; thicker at a tree's foot and along logs and rocks, where the cover's
//!   duff and roots are, as litter piles up against what stands in its way.
//! - **Near the camera only.** Laid in the nearest ring of the grass's squares, and sunk into
//!   the ground with distance, a piece at a time by its rank (`litter.wesl`), so it thins out
//!   rather than ending.
//!
//! - **Heaped against what stands in its way** (D294). Against a trunk, a log or a rock the
//!   litter lies in layers, a piece or two on another, and leans up onto it, as leaves blown
//!   across a floor pile against whatever stops them; so a trunk goes down into the litter
//!   rather than meeting the ground along a line.
//! - **Ivy in its own shade.** Under each piece of ivy, which stands off the leaves on its
//!   stalks, a darker copy lies on the ground a little wider, as a decal's shadow would, so
//!   it does not float.
//!
//! About 3.5 pieces a square metre on a full floor and twice that against what stands on
//! it, 8 triangles each: some 150,000 triangles drawn round the camera in the forest.

use super::SQUARE_M;
use super::blades::Samples;
use crate::footprint::Footprint;
use crate::grown::Cards;
use bevy::{
    asset::RenderAssetUsages,
    mesh::Indices,
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::{AsBindGroup, PrimitiveTopology},
    shader::ShaderRef,
};
use std::f32::consts::TAU;
use terra_gen::smoothstep;

/// The litter's cards, in `assets/data/litter.ron`.
pub const FILE: &str = "litter.ron";
/// Pieces are tried on a grid this fine, in metres.
const SPACING_M: f32 = 0.3;
/// How light the litter's cards are drawn against their scans' colour (D293).
const LITTER_VALUE: f32 = 0.72;
/// The share of leaf and needle pieces tinted green-brown with decay: at least the first,
/// and up to both together where the decay drifts lie (D503).
const DECAYED_SHARE: (f32, f32) = (0.15, 0.35);
/// How steeply litter right against a trunk, a log or a rock leans up onto it: rise over run.
const HEAP_LEAN: f32 = 0.9;
/// From how far pieces start to sink into the ground, and where the last is gone.
pub const FADE_M: [f32; 2] = [8.0, 16.0];

/// What the litter shader is told. Field order must match the struct in litter.wesl.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, PartialEq)]
pub struct LitterExtension {
    #[uniform(100)]
    pub fade: Vec4,
    /// What the climate leaves round the camera (`climate::ClimateMaps`, D327): pieces under
    /// snow sink out of sight.
    #[uniform(100)]
    pub climate_from_world: Mat4,
    #[uniform(100)]
    pub climate_on: u32,
    #[texture(101)]
    #[sampler(102)]
    pub climate_ground: Option<Handle<Image>>,
}

impl LitterExtension {
    /// Reads the climate's maps as they are now painted.
    pub fn read_climate(&mut self, maps: &crate::climate::ClimateMaps) {
        self.climate_from_world = maps.from_world;
        self.climate_on = maps.ground.is_some() as u32;
        self.climate_ground = maps.ground.clone();
    }
}

impl MaterialExtension for LitterExtension {
    fn vertex_shader() -> ShaderRef {
        "shaders/litter.wesl".into()
    }

    fn prepass_vertex_shader() -> ShaderRef {
        "shaders/litter.wesl".into()
    }

    // Shaded only where in front, after the prepass (`grown::test_depth_early`).
    fn specialize(
        _pipeline: &bevy::pbr::MaterialExtensionPipeline,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        _layout: &bevy::mesh::MeshVertexBufferLayoutRef,
        key: bevy::pbr::MaterialExtensionKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        crate::grown::test_depth_early(descriptor, key.mesh_key);
        Ok(())
    }
}

pub type LitterMaterial = ExtendedMaterial<StandardMaterial, LitterExtension>;

/// A kind of piece: the cards it is drawn from, and how wide it lies, least and most.
#[derive(Clone)]
struct Kind {
    cells: Vec<Vec2>,
    size: (f32, f32),
}

/// The litter's cards as the squares are built with them: each kind's cells and how far
/// across a cell is.
#[derive(Clone)]
pub struct LitterCards {
    across: f32,
    leaves: Kind,
    mixed: Kind,
    needles: Kind,
    cones: Kind,
    twigs: Kind,
    ivy: Kind,
}

impl LitterCards {
    pub fn from(cards: &Cards) -> Self {
        let kind = |names: &[&str], size| Kind {
            cells: names.iter().map(|name| cards.get(name).cell).collect(),
            size,
        };
        Self {
            across: cards.get("oakfall").across,
            leaves: kind(&["oakfall", "oakmould", "beechfall"], (0.3, 0.45)),
            mixed: kind(&["leafneedle"], (0.3, 0.42)),
            needles: kind(&["needles", "needledark", "needlecone"], (0.32, 0.48)),
            cones: kind(&["cones"], (0.26, 0.34)),
            twigs: kind(&["twigs"], (0.4, 0.6)),
            ivy: kind(&["ivy", "ivydark"], (0.35, 0.5)),
        }
    }
}

/// A cheap, well-mixed hash, as the blades' own.
fn hash(seed: u64, i: i32, j: i32, stream: u64) -> f32 {
    super::blades::hash(seed, i, j, stream)
}

/// The litter of the square whose corner, east and south of the start, is `corner`, on the
/// sample grid the blades are grown from; in the square's own frame (east, up, south from its
/// middle). None where nothing lies.
pub(super) fn lay(
    samples: &Samples,
    cards: &LitterCards,
    seed: u64,
    corner: Vec2,
    clear: &[Footprint],
    to_world: impl Fn(Vec3) -> Vec3,
) -> Option<Mesh> {
    let middle = corner + Vec2::splat(SQUARE_M / 2.0);
    let mut mesh = PieceMesh::default();
    let steps = (SQUARE_M / SPACING_M) as i32;
    let first = (corner / SPACING_M).round().as_ivec2();
    for b in 0..steps {
        for a in 0..steps {
            let (gi, gj) = (first.x + a, first.y + b);
            let h = |stream| hash(seed, gi, gj, stream);
            let at = Vec2::new(gi as f32 + h(1), gj as f32 + h(2)) * SPACING_M;
            let [_, grass, _, _, litter, moss, needles, roots, _, heap, ..] = samples.at(at);
            let floor = (litter + needles).min(1.0);
            // Thicker against trunks, logs and rocks, where the duff is; thinner in grass
            // and on moss.
            let density = floor
                * (0.4 + 0.6 * roots.max(heap))
                * (1.0 - 0.85 * smoothstep(0.15, 0.6, grass))
                * (1.0 - 0.6 * moss);
            if h(3) >= density * 0.8 {
                continue;
            }
            let needly = needles / (litter + needles).max(1e-3);
            let pick = h(4);
            let twig_share = 0.07 + 0.18 * roots;
            // Ivy creeps over the leaves in drifts a few metres across, under broadleaves.
            let ivy = smoothstep(0.55, 0.78, drift(seed, at)) * (1.0 - needly) * litter;
            let kind = if h(15) < ivy * 0.8 {
                &cards.ivy
            } else if pick < twig_share {
                &cards.twigs
            } else if h(5) < needly {
                if h(6) < 0.12 {
                    &cards.cones
                } else {
                    &cards.needles
                }
            } else if needles > 0.15 && h(6) < 0.5 {
                &cards.mixed
            } else {
                &cards.leaves
            };
            let cell = kind.cells[(h(7) * kind.cells.len() as f32) as usize % kind.cells.len()];
            let size = kind.size.0 + (kind.size.1 - kind.size.0) * h(8);
            // Against something, which way it stands (up the heap's slope), and how steeply
            // the litter leans up onto it.
            let step = 0.25;
            let heap_at = |p: Vec2| samples.at(p)[9];
            let toward = Vec2::new(
                heap_at(at + Vec2::X * step) - heap_at(at - Vec2::X * step),
                heap_at(at + Vec2::Y * step) - heap_at(at - Vec2::Y * step),
            )
            .normalize_or_zero();
            let lean = toward * HEAP_LEAN * heap * heap;
            let rise = |p: Vec2| samples.at(p)[0];
            let place = |p: Vec2, lift: f32| {
                let up_onto = (p - at).dot(lean).max(0.0);
                Vec3::new(p.x - middle.x, rise(p) + lift + up_onto, p.y - middle.y)
            };
            let centre = place(at, 0.0);
            if clear
                .iter()
                .any(|footprint| footprint.covers(to_world(centre), size * 0.5))
            {
                continue;
            }
            // Darker in the damp under moss and duff, a little lighter on top of a deep floor;
            // an umber, as the floor's own, rather than the scans' orange (D293).
            let value = LITTER_VALUE * (0.6 + 0.35 * h(9)) * (1.0 - 0.25 * roots);
            // Ivy's leaves are dark and glossy, darker than the scan's in the shade it grows in.
            let value = if std::ptr::eq(kind, &cards.ivy) {
                value * 0.6
            } else {
                value
            };
            let is_ivy = std::ptr::eq(kind, &cards.ivy);
            // Some of the leaves gone green-brown with decay and the moss on them, more in
            // the ground's own decayed patches, so the litter is mixed rather than brown alone
            // (D503).
            let decayed = !is_ivy
                && h(21) < DECAYED_SHARE.0 + DECAYED_SHARE.1 * drift(seed.wrapping_add(7), at);
            let tint = if decayed {
                Vec3::new(value * 0.72, value * (1.0 + 0.1 * h(10)), value * 0.62)
            } else {
                Vec3::new(
                    value * (0.82 + 0.1 * h(10)),
                    value,
                    value * (0.95 + 0.1 * h(11)),
                )
            };
            if is_ivy {
                // Its shade on the leaves under it.
                mesh.piece(
                    at,
                    h(12) * TAU,
                    size * 1.12,
                    0.004,
                    cell,
                    cards.across,
                    Vec3::new(0.14, 0.12, 0.1),
                    h(14),
                    place,
                );
            }
            // A heap: against something, a piece or two more lies on this one, each turned
            // its own way and a little smaller, higher up.
            let layers = 1 + (h(16) < heap * 0.85) as u32 + (h(17) < heap * 0.45) as u32;
            for layer in 0..layers {
                let k = layer as f32;
                let on = |p: Vec2, lift: f32| place(p, lift + k * (0.025 + 0.02 * h(18)));
                mesh.piece(
                    at + Vec2::new(h(19) - 0.5, h(20) - 0.5) * 0.12 * k,
                    (h(12) + 0.37 * k) * TAU,
                    size * (1.0 - 0.12 * k),
                    // Ivy stands off the leaves on its stalks.
                    if is_ivy {
                        0.035 + 0.04 * h(13)
                    } else {
                        0.012 + 0.03 * size * h(13) + 0.03 * heap
                    },
                    cell,
                    cards.across,
                    tint * (1.0 + 0.08 * k),
                    h(14),
                    on,
                );
            }
        }
    }
    (!mesh.indices.is_empty()).then(|| mesh.mesh())
}

/// Smooth noise from 0 to 1 over the floor, varying over a few metres: where drifts lie.
fn drift(seed: u64, at: Vec2) -> f32 {
    let cell = at / 2.5;
    let (i, j) = (cell.x.floor() as i32, cell.y.floor() as i32);
    let (fx, fy) = (cell.x - i as f32, cell.y - j as f32);
    let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let corner = |a: i32, b: i32| hash(seed, i + a, j + b, 99);
    let top = corner(0, 0) + (corner(1, 0) - corner(0, 0)) * sx;
    let bottom = corner(0, 1) + (corner(1, 1) - corner(0, 1)) * sx;
    top + (bottom - top) * sy
}

/// The pieces of a square, as the litter shader reads them.
#[derive(Default)]
struct PieceMesh {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    /// The piece's rank, which says how far off it sinks away.
    ranks: Vec<[f32; 2]>,
    colours: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

/// Vertices to a side of a piece: a grid, so it humps over the ground and follows it.
const SIDE: u32 = 3;

impl PieceMesh {
    /// A piece at `at` on the square's map, turned `angle`, `size` across, its middle `hump`
    /// over its edge, drawn from the atlas cell at `cell`; `place` puts a point of the map on
    /// the ground, lifted so far.
    #[allow(clippy::too_many_arguments)]
    fn piece(
        &mut self,
        at: Vec2,
        angle: f32,
        size: f32,
        hump: f32,
        cell: Vec2,
        across: f32,
        tint: Vec3,
        rank: f32,
        place: impl Fn(Vec2, f32) -> Vec3,
    ) {
        let (s, c) = (libm::sinf(angle), libm::cosf(angle));
        let u_axis = Vec2::new(c, s);
        let v_axis = Vec2::new(-s, c);
        let base = self.positions.len() as u32;
        let last = (SIDE - 1) as f32;
        for j in 0..SIDE {
            for i in 0..SIDE {
                let (u, v) = (i as f32 / last, j as f32 / last);
                let (x, y) = (u - 0.5, v - 0.5);
                let p = at + (u_axis * x + v_axis * y) * size;
                // Humped: highest at its middle, its edge on the ground.
                let r2 = (x * x + y * y) * 4.0;
                let lift = 0.006 + hump * (1.0 - r2.min(1.0));
                self.positions.push(place(p, lift).to_array());
                // Leaning out of the hump, a little.
                let slope = Vec2::new(x, y) * hump * 4.0 / size;
                let lean = u_axis * slope.x + v_axis * slope.y;
                self.normals
                    .push(Vec3::new(lean.x, 1.0, lean.y).normalize().to_array());
                self.uvs.push((cell + Vec2::new(u, v) * across).to_array());
                self.ranks.push([rank, 0.0]);
                self.colours.push([tint.x, tint.y, tint.z, 1.0]);
            }
        }
        for j in 0..SIDE - 1 {
            for i in 0..SIDE - 1 {
                let a = base + j * SIDE + i;
                self.indices
                    .extend([a, a + SIDE, a + 1, a + 1, a + SIDE, a + SIDE + 1]);
            }
        }
    }

    fn mesh(self) -> Mesh {
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, self.ranks)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.colours)
        .with_inserted_indices(Indices::U32(self.indices));
        // The atlas's normal map needs tangents, worked out as Bevy reads them.
        if let Err(problem) = mesh.generate_tangents() {
            warn!("litter: no tangents: {problem}");
        }
        mesh
    }
}

#[cfg(test)]
mod tests {
    use super::super::blades::SAMPLED;
    use super::*;

    fn cards() -> LitterCards {
        LitterCards::from(&Cards::read_file(FILE))
    }

    /// A square's samples: everywhere the same.
    fn samples(litter: f32, needles: f32, grass: f32, roots: f32) -> Samples {
        let side = (SQUARE_M / 0.5) as usize + 1;
        let mut values = [0.0; SAMPLED];
        values[1] = grass;
        values[4] = litter;
        values[6] = needles;
        values[7] = roots;
        Samples {
            corner: Vec2::ZERO,
            side,
            values: vec![values; side * side],
        }
    }

    fn pieces(samples: &Samples) -> usize {
        lay(samples, &cards(), 7, Vec2::ZERO, &[], |p| p)
            .map_or(0, |mesh| mesh.indices().map_or(0, |i| i.len()) / (6 * 4))
    }

    #[test]
    fn litter_lies_on_the_forest_floor_thickest_against_what_stands_there() {
        let bare = pieces(&samples(0.0, 0.0, 0.0, 0.0));
        let floor = pieces(&samples(1.0, 0.0, 0.0, 0.0));
        let piled = pieces(&samples(1.0, 0.0, 0.0, 1.0));
        let grassy = pieces(&samples(1.0, 0.0, 0.9, 0.0));
        assert_eq!(bare, 0, "nothing on bare ground");
        // A full floor: some 3 to 8 pieces a square metre.
        let area = SQUARE_M * SQUARE_M;
        assert!(
            (3.0..8.0).contains(&(floor as f32 / area)),
            "{floor} pieces"
        );
        assert!(piled as f32 > floor as f32 * 1.8, "{piled} against {floor}");
        assert!(grassy * 5 < floor, "{grassy} in grass against {floor}");
    }

    /// Litter heaps against what stands on the floor (D294): with something standing east of
    /// the square, the heap rising toward it, more pieces lie in the square's east half than
    /// its west, and they lie higher there, piled on each other and leaning up onto it.
    #[test]
    fn litter_heaps_up_against_what_stands_there() {
        let side = (SQUARE_M / 0.5) as usize + 1;
        let mut values = Vec::with_capacity(side * side);
        for _ in 0..side {
            for a in 0..side {
                let mut value = [0.0; SAMPLED];
                value[4] = 1.0;
                value[9] = a as f32 / (side - 1) as f32;
                values.push(value);
            }
        }
        let samples = Samples {
            corner: Vec2::ZERO,
            side,
            values,
        };
        let mesh = lay(&samples, &cards(), 7, Vec2::ZERO, &[], |p| p).expect("litter");
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("no positions");
        };
        // Each piece's middle vertex: east of the square's middle or west, and how high.
        let middles: Vec<[f32; 3]> = positions
            .chunks((SIDE * SIDE) as usize)
            .map(|piece| piece[(SIDE * SIDE / 2) as usize])
            .collect();
        let half = |east: bool| -> (usize, f32) {
            let lying: Vec<f32> = middles
                .iter()
                .filter(|p| (p[0] > 0.0) == east)
                .map(|p| p[1])
                .collect();
            (
                lying.len(),
                lying.iter().sum::<f32>() / lying.len().max(1) as f32,
            )
        };
        let ((west, west_high), (east, east_high)) = (half(false), half(true));
        assert!(
            east as f32 > west as f32 * 1.3,
            "{east} pieces east, {west} west"
        );
        assert!(
            east_high > west_high + 0.015,
            "east {east_high:.3} m high, west {west_high:.3} m"
        );
    }

    #[test]
    fn needles_lie_under_conifers_and_leaves_under_broadleaves() {
        let cards = cards();
        let cells = |samples: &Samples| -> Vec<[f32; 2]> {
            let mesh = lay(samples, &cards, 7, Vec2::ZERO, &[], |p| p).expect("litter");
            let Some(bevy::mesh::VertexAttributeValues::Float32x2(uvs)) =
                mesh.attribute(Mesh::ATTRIBUTE_UV_0)
            else {
                panic!("no uvs");
            };
            uvs.iter()
                .step_by((SIDE * SIDE) as usize)
                .copied()
                .collect()
        };
        let of = |kind: &Kind, uv: [f32; 2]| {
            kind.cells
                .iter()
                .any(|cell| cell.distance(Vec2::from(uv)) < 1e-4)
        };
        let under_oaks = cells(&samples(1.0, 0.0, 0.0, 0.0));
        let under_pines = cells(&samples(0.0, 1.0, 0.0, 0.0));
        let share = |uvs: &[[f32; 2]], kind: &Kind| {
            uvs.iter().filter(|uv| of(kind, **uv)).count() as f32 / uvs.len() as f32
        };
        assert!(share(&under_oaks, &cards.leaves) + share(&under_oaks, &cards.ivy) > 0.85);
        assert!(
            share(&under_oaks, &cards.ivy) > 0.02,
            "ivy in drifts under broadleaves"
        );
        assert_eq!(share(&under_pines, &cards.ivy), 0.0);
        assert_eq!(share(&under_oaks, &cards.needles), 0.0);
        assert!(share(&under_pines, &cards.needles) > 0.7);
        assert_eq!(share(&under_pines, &cards.leaves), 0.0);
    }

    #[test]
    fn the_litter_is_mixed_with_green_brown_decay_not_brown_alone() {
        let under_oaks = samples(1.0, 0.0, 0.0, 0.0);
        let mesh = lay(&under_oaks, &cards(), 7, Vec2::ZERO, &[], |p| p).expect("litter");
        let Some(bevy::mesh::VertexAttributeValues::Float32x4(colours)) =
            mesh.attribute(Mesh::ATTRIBUTE_COLOR)
        else {
            panic!("no colours");
        };
        // Told by its blue: a brown leaf's tint is as blue as it is green, a decayed one's
        // well under, its green over its red.
        let pieces: Vec<_> = colours.iter().step_by((SIDE * SIDE) as usize).collect();
        let decayed = pieces
            .iter()
            .filter(|c| c[2] < 0.75 * c[1] && c[1] > 1.2 * c[0])
            .count() as f32
            / pieces.len() as f32;
        assert!(
            (0.12..0.6).contains(&decayed),
            "{:.0}% of {} pieces green-brown",
            100.0 * decayed,
            pieces.len()
        );
    }
}
