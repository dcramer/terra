//! The sky's shaders compile and validate as Bevy's shader cache builds them, without a GPU: a
//! shader that fails draws nothing, only an error in the log (sky stage 1 lost its clouds to a
//! function it had removed but the far land still called). As `terra_walker::skinning`'s test
//! compiles the skinning shader.

const SKY: &str = include_str!("../../../../assets/shaders/sky.wesl");
const DUSK: &str = include_str!("../../../../assets/shaders/dusk.wesl");
const CLOUD_NOISE: &str = include_str!("../../../../assets/shaders/cloud_noise.wesl");
const CLOUD_VOLUME: &str = include_str!("../../../../assets/shaders/cloud_volume.wesl");
const CLOUD_VEIL: &str = include_str!("../../../../assets/shaders/cloud_veil.wesl");

/// In place of Bevy's own modules, the parts of them the sky uses, as Bevy declares them
/// (`bevy_pbr/src/render/forward_io.wesl`, `mesh_view_bindings.wesl`, `mesh_view_types.wesl`,
/// `bevy_render/src/view.wesl`).
const FORWARD_IO: &str = "
    struct VertexOutput {
        @builtin(position) position: vec4<f32>,
        @location(0) world_position: vec4<f32>,
        @location(1) world_normal: vec3<f32>,
    };
";
const MESH_VIEW_BINDINGS: &str = "
    struct View {
        world_position: vec3<f32>,
        exposure: f32,
        viewport: vec4<f32>,
    };
    @group(0) @binding(0) var<uniform> view: View;
";
const VIEW: &str = "
    struct View {
        world_from_clip: mat4x4<f32>,
        world_position: vec3<f32>,
        exposure: f32,
        viewport: vec4<f32>,
    };
";
/// The constants module Bevy's cache makes from its shader definitions (`shader_cache.rs`).
const CONSTANTS: &str = "const MATERIAL_BIND_GROUP = 3u;\n";

struct Modules;

impl wesl::Resolver for Modules {
    fn resolve_source<'a>(
        &'a self,
        path: &wesl::syntax::ModulePath,
    ) -> Result<std::borrow::Cow<'a, str>, wesl::error::ResolveError> {
        if path.origin == wesl::syntax::PathOrigin::Package("constants".into())
            && path.components.is_empty()
        {
            return Ok(CONSTANTS.into());
        }
        match path.components.last().map(String::as_str) {
            Some("sky") => Ok(SKY.into()),
            Some("dusk") => Ok(DUSK.into()),
            Some("cloud_noise") => Ok(CLOUD_NOISE.into()),
            Some("cloud_volume") => Ok(CLOUD_VOLUME.into()),
            Some("cloud_veil") => Ok(CLOUD_VEIL.into()),
            Some("forward_io") => Ok(FORWARD_IO.into()),
            Some("mesh_view_bindings") => Ok(MESH_VIEW_BINDINGS.into()),
            Some("view") => Ok(VIEW.into()),
            _ => Err(wesl::error::ResolveError::ModuleNotFound(
                path.clone(),
                "not one of the test's modules".into(),
            )),
        }
    }
}

/// Compiles `shaders/<name>.wesl` with its imports and validates it as naga would on a GPU.
fn compiles(name: &str) {
    let options = wesl::CompileOptions {
        imports: true,
        condcomp: true,
        visibility: false,
        ..Default::default()
    };
    let main = wesl::syntax::ModulePath {
        origin: wesl::syntax::PathOrigin::Absolute,
        components: vec!["shaders".into(), name.into()],
    };
    let wgsl = wesl::compile(&main, &options, &Modules)
        .unwrap_or_else(|e| panic!("{name}: {}", e.diagnostic().render_plain()))
        .to_string();
    let module = naga::front::wgsl::parse_str(&wgsl)
        .unwrap_or_else(|e| panic!("{name}: {}", e.emit_to_string(&wgsl)));
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::default(),
    )
    .validate(&module)
    .unwrap_or_else(|e| panic!("{name}: {}", e.emit_to_string(&wgsl)));
}

#[test]
fn the_sky_shader_compiles_and_validates_as_bevy_would_build_it() {
    compiles("sky");
}

#[test]
fn the_volumetric_clouds_march_and_veil_compile_and_validate() {
    compiles("cloud_volume");
    compiles("cloud_veil");
}
