//! The sky's shader compiles and validates as Bevy's shader cache builds it, without a GPU: a
//! shader that fails draws no sky at all, only an error in the log (sky stage 1 lost its clouds
//! to a function it had removed but the far land still called). As
//! `terra_walker::skinning`'s test compiles the skinning shader.

const SKY: &str = include_str!("../../../../assets/shaders/sky.wesl");
const DUSK: &str = include_str!("../../../../assets/shaders/dusk.wesl");

/// In place of Bevy's own modules, the parts of them the sky uses, as Bevy declares them
/// (`bevy_pbr/src/render/forward_io.wesl`, `mesh_view_bindings.wesl`, `mesh_view_types.wesl`).
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
    };
    @group(0) @binding(0) var<uniform> view: View;
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
            Some("forward_io") => Ok(FORWARD_IO.into()),
            Some("mesh_view_bindings") => Ok(MESH_VIEW_BINDINGS.into()),
            _ => Err(wesl::error::ResolveError::ModuleNotFound(
                path.clone(),
                "not one of the test's modules".into(),
            )),
        }
    }
}

#[test]
fn the_sky_shader_compiles_and_validates_as_bevy_would_build_it() {
    let options = wesl::CompileOptions {
        imports: true,
        condcomp: true,
        visibility: false,
        ..Default::default()
    };
    let main = wesl::syntax::ModulePath {
        origin: wesl::syntax::PathOrigin::Absolute,
        components: vec!["shaders".into(), "sky".into()],
    };
    let wgsl = wesl::compile(&main, &options, &Modules)
        .unwrap_or_else(|e| panic!("{}", e.diagnostic().render_plain()))
        .to_string();
    let module = naga::front::wgsl::parse_str(&wgsl)
        .unwrap_or_else(|e| panic!("{}", e.emit_to_string(&wgsl)));
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::default(),
    )
    .validate(&module)
    .unwrap_or_else(|e| panic!("{}", e.emit_to_string(&wgsl)));
}
