//! A mesh's vertex buffer, as our patched bevy_mesh packs it (patches/bevy_mesh, D435), is
//! laid out as the pipeline reads it: each vertex at its stride, each attribute at the
//! offset Bevy's vertex buffer layout gives it. The patch packs runs of vertices in ordinary
//! memory before one copy into the GPU's staging memory, so a mesh spanning several runs,
//! the last one short, is the case that could go wrong.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{
    Mesh, MeshVertexAttribute, MeshVertexAttributeId, MeshVertexBufferLayouts, PrimitiveTopology,
    VertexAttributeValues, VertexFormat,
};

/// A two-byte attribute, a size the patch copies without a fixed-size path.
const ATTRIBUTE_PAIR: MeshVertexAttribute =
    MeshVertexAttribute::new("Terra_Test_Pair", 988_540_917, VertexFormat::Uint8x2);

/// More vertices than one run of the patch's packing holds (64 KiB, about 800 of these).
const VERTICES: usize = 2_000;

/// A mesh with an attribute of each size the game's meshes use, every value distinct.
fn mesh() -> (Mesh, Vec<(MeshVertexAttributeId, Vec<u8>)>) {
    let n = VERTICES;
    let f = |i: usize, k: usize| i as f32 * 0.25 + k as f32 * 1000.0;
    let attributes: Vec<(MeshVertexAttribute, VertexAttributeValues)> = vec![
        (
            Mesh::ATTRIBUTE_POSITION,
            (0..n)
                .map(|i| [f(i, 0), f(i, 1), f(i, 2)])
                .collect::<Vec<_>>()
                .into(),
        ),
        (
            Mesh::ATTRIBUTE_NORMAL,
            (0..n)
                .map(|i| [f(i, 3), f(i, 4), f(i, 5)])
                .collect::<Vec<_>>()
                .into(),
        ),
        (
            Mesh::ATTRIBUTE_UV_0,
            (0..n)
                .map(|i| [f(i, 6), f(i, 7)])
                .collect::<Vec<_>>()
                .into(),
        ),
        (
            Mesh::ATTRIBUTE_UV_1,
            (0..n)
                .map(|i| [f(i, 8), f(i, 9)])
                .collect::<Vec<_>>()
                .into(),
        ),
        (
            Mesh::ATTRIBUTE_TANGENT,
            (0..n)
                .map(|i| [f(i, 10), f(i, 11), f(i, 12), f(i, 13)])
                .collect::<Vec<_>>()
                .into(),
        ),
        (
            Mesh::ATTRIBUTE_COLOR,
            (0..n)
                .map(|i| [f(i, 14), f(i, 15), f(i, 16), f(i, 17)])
                .collect::<Vec<_>>()
                .into(),
        ),
        (
            Mesh::ATTRIBUTE_JOINT_INDEX,
            VertexAttributeValues::Uint16x4((0..n).map(|i| [i as u16, 1, 2, 3]).collect()),
        ),
        (
            ATTRIBUTE_PAIR,
            VertexAttributeValues::Uint8x2((0..n).map(|i| [i as u8, (i / 256) as u8]).collect()),
        ),
    ];
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    let mut columns = Vec::new();
    for (attribute, values) in attributes {
        columns.push((attribute.id, values.get_bytes().to_vec()));
        mesh.insert_attribute(attribute, values);
    }
    (mesh, columns)
}

/// The vertex buffer the pipeline expects, built from Bevy's layout of the mesh and the
/// attributes' own bytes, a value at a time.
fn expected(mesh: &Mesh, columns: &[(MeshVertexAttributeId, Vec<u8>)]) -> Vec<u8> {
    let layout = mesh.get_mesh_vertex_buffer_layout(&mut MeshVertexBufferLayouts::default());
    let stride = layout.0.layout().array_stride as usize;
    let mut buffer = vec![0; stride * VERTICES];
    let mut written = vec![false; buffer.len()];
    for (id, bytes) in columns {
        let at = layout
            .0
            .attribute_ids()
            .iter()
            .position(|a| a == id)
            .expect("in the layout");
        let attribute = &layout.0.layout().attributes[at];
        let size = attribute.format.size() as usize;
        for (vertex, value) in bytes.chunks_exact(size).enumerate() {
            let offset = vertex * stride + attribute.offset as usize;
            buffer[offset..offset + size].copy_from_slice(value);
            written[offset..offset + size].fill(true);
        }
    }
    assert!(
        written.iter().all(|w| *w),
        "the layout leaves no byte of a vertex unset"
    );
    buffer
}

#[test]
fn a_meshs_packed_vertex_buffer_is_laid_out_as_its_vertex_layout_says() {
    let (mesh, columns) = mesh();
    let expected = expected(&mesh, &columns);
    assert_eq!(expected.len(), mesh.get_vertex_buffer_size());

    assert!(
        mesh.create_packed_vertex_buffer_data() == expected,
        "create_packed_vertex_buffer_data"
    );

    // As the mesh allocator writes it: into a slice of staging memory that held other bytes.
    let mut staged = vec![0x55; expected.len()];
    mesh.write_packed_vertex_buffer_data(wgpu::WriteOnly::from_mut(staged.as_mut_slice()));
    let first_wrong = staged.iter().zip(&expected).position(|(a, b)| a != b);
    assert_eq!(
        first_wrong, None,
        "write_packed_vertex_buffer_data: first wrong byte"
    );
}
