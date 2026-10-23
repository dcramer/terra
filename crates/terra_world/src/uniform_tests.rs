//! A material's uniform is the size its shader's struct is. A Rust copy that misses a field
//! the shader gained has a smaller buffer than the pipeline wants, and wgpu refuses the
//! pipeline at the first draw (the lush far forest's, after D416 grew `Far`, 49ac4528).
//!
//! The struct is read from the shader's source and the material's layout built as Bevy builds
//! it, on wgpu's stub device, so no card is needed.

use bevy::render::render_resource::{AsBindGroup, BindingType};
use bevy::render::renderer::RenderDevice;

/// The bytes the material binds its uniform at `binding`.
fn uniform_size<M: AsBindGroup>(device: &RenderDevice, binding: u32) -> u64 {
    M::bind_group_layout_entries(device, false)
        .iter()
        .find(|entry| entry.binding == binding)
        .and_then(|entry| match entry.ty {
            BindingType::Buffer {
                min_binding_size, ..
            } => min_binding_size.map(|n| n.get()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no sized uniform at binding {binding}"))
}

/// The bytes of the struct `shader` binds as a uniform at `binding`, for structs of 16-byte
/// fields (vec4s, arrays of them, 4x4 matrices), which need no padding rules.
fn shader_struct_size(shader: &str, binding: u32) -> u64 {
    let path = format!(
        "{}/../../assets/shaders/{shader}",
        env!("CARGO_MANIFEST_DIR")
    );
    let source = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let after = source
        .split(&format!("@binding({binding})"))
        .nth(1)
        .unwrap_or_else(|| panic!("{shader} binds nothing at {binding}"));
    let declaration = after.split(';').next().unwrap_or_default();
    let type_name = declaration.rsplit(':').next().unwrap_or_default().trim();
    let body = source
        .split(&format!("struct {type_name} {{"))
        .nth(1)
        .and_then(|rest| rest.split('}').next())
        .unwrap_or_else(|| panic!("{shader} has no struct {type_name}"));
    body.lines()
        .map(|line| line.split("//").next().unwrap_or_default().trim())
        .filter(|line| line.contains(':'))
        .map(|field| {
            let ty = field.split_once(':').map(|(_, t)| t).unwrap_or_default();
            let ty = ty.trim().trim_end_matches(',').replace(' ', "");
            if ty == "vec4<f32>" {
                16
            } else if ty == "mat4x4<f32>" {
                64
            } else if let Some(n) = ty
                .strip_prefix("array<vec4<f32>,")
                .and_then(|rest| rest.strip_suffix('>'))
            {
                16 * n
                    .parse::<u64>()
                    .unwrap_or_else(|_| panic!("array length in {field}"))
            } else {
                panic!("{shader}: {field}: teach shader_struct_size this type's size")
            }
        })
        .sum()
}

#[test]
fn the_far_forests_uniform_is_its_shaders() {
    let (device, _queue) = wgpu::Device::noop(&wgpu::DeviceDescriptor::default());
    let device = RenderDevice::from(device);
    assert_eq!(
        uniform_size::<crate::woods::FarWoods>(&device, 100),
        shader_struct_size("far_forest.wesl", 100),
        "woods::FarExtension and far_forest.wesl's Far differ: add the field to both"
    );
}
