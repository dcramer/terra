//! Every shader in `assets/shaders` compiles and validates as Bevy's shader cache builds it
//! for the game, without a GPU (D549): a shader that fails draws nothing, only an error in
//! the log, and sky stage 1 lost its clouds to a function it had removed but the far land
//! still called. Unreal fails the cook on a shader that does not compile; this is ours.
//!
//! The test is Bevy's own path, not a stand-in for it: a headless app (no renderer, as Bevy's
//! `no_renderer` example) loads every shader library Bevy's plugins register and every file of
//! ours through the asset server, as a run does; Bevy's `ShaderCache` resolves the imports and
//! composes each with WESL, and naga validates what it gives, as wgpu does before a driver
//! sees it.
//!
//! A shader is compiled with the definitions the game's pipelines give it, each set read off
//! the code that makes them, not guessed: [`DRAWS`] is every material of ours, which stage
//! each file is, how it blends, whether it is drawn into the prepass and the shadow maps, the
//! attributes of the meshes it is drawn on and the definitions its `specialize` adds; the
//! views are the main camera's at both quality presets (`scene.rs`), with the prepasses its
//! anti-aliasing and occlusion ask for. From those the forward pass, the depth, normal and
//! motion prepass, and the sun's shadow pass are specialized as `bevy_pbr`'s
//! `MeshPipeline::specialize` and `PrepassPipeline::specialize` do. A file no material,
//! pipeline or importer names fails the test until it says how the game uses it.

use bevy::{
    asset::LoadState,
    audio::AudioPlugin,
    log::LogPlugin,
    prelude::*,
    render::{RenderPlugin, settings::WgpuSettings},
    shader::{ShaderCache, ShaderCacheError, ShaderCacheSource, ShaderDefVal, ValidateShader},
    window::ExitCondition,
    winit::WinitPlugin,
};
use std::collections::BTreeSet;
use std::path::PathBuf;

/// How a material blends (`Material::alpha_mode`, or its base's `alpha_mode`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Blending {
    Opaque,
    Mask,
    Blend,
    Premultiplied,
}
use Blending::*;

/// What a mesh drawn with a material carries, as `MeshPipeline::specialize` reads its layout
/// and key: its vertex attributes, a skin, and a visibility range (dithered between levels).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Part {
    Normals,
    Uvs,
    UvsB,
    Tangents,
    Colours,
    Skin,
    Dither,
}
use Part::*;

/// A material of ours as the game draws it: each stage's file (by name in `assets/shaders`;
/// `None` is Bevy's own), how it blends, where it is drawn, its meshes and the definitions
/// its `specialize` adds, one set per pipeline it makes.
struct Draw {
    material: &'static str,
    vertex: Option<&'static str>,
    fragment: Option<&'static str>,
    prepass_vertex: Option<&'static str>,
    prepass_fragment: Option<&'static str>,
    alpha: &'static [Blending],
    /// Drawn into the shadow maps: some mesh with it has no `NotShadowCaster`.
    shadows: bool,
    meshes: &'static [&'static [Part]],
    own: &'static [&'static [&'static str]],
}

const DRAW: Draw = Draw {
    material: "",
    vertex: None,
    fragment: None,
    prepass_vertex: None,
    prepass_fragment: None,
    alpha: &[Opaque],
    shadows: false,
    meshes: &[],
    own: &[&[]],
};

/// A glTF model's mesh, as the props, kits and bodies come: normals, UVs and tangents.
const MODEL: &[Part] = &[Normals, Uvs, Tangents];
/// Bevy's sphere: normals and UVs.
const SPHERE: &[Part] = &[Normals, Uvs];
/// The cards the GPU places (fire, smoke, drift, lakes' mist): each corner's UV and its
/// card's numbers in the second UV.
const CARDS: &[Part] = &[Normals, Uvs, UvsB];

/// Every material of ours, read off its `Material` or `MaterialExtension` impl and where its
/// meshes are made and spawned.
const DRAWS: &[Draw] = &[
    Draw {
        material: "SkyMaterial (sky.rs): opaque, premultiplied over the atmosphere; NotShadowCaster",
        fragment: Some("sky"),
        alpha: &[Opaque, Premultiplied],
        meshes: &[SPHERE],
        ..DRAW
    },
    Draw {
        material: "VeilMaterial (sky/volume.rs): premultiplied, NotShadowCaster",
        fragment: Some("cloud_veil"),
        alpha: &[Premultiplied],
        meshes: &[SPHERE],
        ..DRAW
    },
    Draw {
        material: "GroundExtension (planet.rs), on the terrain's chunks (terrain.rs); GROUND_LOW cheap",
        fragment: Some("ground"),
        meshes: &[&[Normals, Uvs, UvsB, Colours]],
        own: &[&[], &["GROUND_LOW"]],
        ..DRAW
    },
    Draw {
        material: "WaterExtension (planet/sea.rs), on the sea's ico sphere (planet.rs): blended, NotShadowCaster",
        fragment: Some("water"),
        alpha: &[Blend],
        meshes: &[SPHERE],
        ..DRAW
    },
    Draw {
        material: "LakeExtension (lakes.rs): blended, NotShadowCaster",
        fragment: Some("lake"),
        alpha: &[Blend],
        meshes: &[CARDS],
        ..DRAW
    },
    Draw {
        material: "MistExtension (lakes.rs): blended, NotShadowCaster",
        vertex: Some("mist"),
        fragment: Some("mist"),
        prepass_vertex: Some("mist"),
        prepass_fragment: Some("mist"),
        alpha: &[Blend],
        meshes: &[CARDS],
        ..DRAW
    },
    Draw {
        material: "FarExtension (woods.rs): masked, NotShadowCaster",
        vertex: Some("far_forest"),
        fragment: Some("far_forest"),
        prepass_vertex: Some("far_forest"),
        prepass_fragment: Some("far_forest"),
        alpha: &[Mask],
        meshes: &[&[Normals, Uvs, UvsB, Colours]],
        ..DRAW
    },
    Draw {
        material: "FoliageExtension (grown/mod.rs), on grown trees (grown/build.rs): masked, shadow cards cast",
        vertex: Some("foliage"),
        fragment: Some("foliage_fragment"),
        prepass_vertex: Some("foliage"),
        prepass_fragment: Some("foliage_prepass"),
        alpha: &[Mask],
        shadows: true,
        meshes: &[
            &[Normals, Uvs, UvsB, Colours],
            &[Normals, Uvs, UvsB, Colours, Dither],
        ],
        ..DRAW
    },
    Draw {
        material: "FoliageExtension (terra_sandbox experiments/lush.rs): masked, with tangents",
        vertex: Some("foliage"),
        fragment: Some("foliage_fragment"),
        prepass_vertex: Some("foliage"),
        prepass_fragment: Some("foliage_prepass"),
        alpha: &[Mask],
        shadows: true,
        meshes: &[&[Normals, Uvs, UvsB, Tangents, Colours, Dither]],
        ..DRAW
    },
    Draw {
        material: "GrassExtension (cover/mod.rs), on blades (cover/blades.rs): masked, NotShadowCaster",
        vertex: Some("grass"),
        prepass_vertex: Some("grass"),
        alpha: &[Mask],
        meshes: &[&[Normals, Uvs, UvsB, Colours]],
        ..DRAW
    },
    Draw {
        material: "LitterExtension (cover/litter.rs): masked, NotShadowCaster",
        vertex: Some("litter"),
        prepass_vertex: Some("litter"),
        alpha: &[Mask],
        meshes: &[&[Normals, Uvs, UvsB, Colours]],
        ..DRAW
    },
    Draw {
        material: "WindExtension (flora.rs), on flora's models (flora/model.rs) and meadows",
        vertex: Some("wind"),
        fragment: Some("flora"),
        prepass_vertex: Some("wind"),
        alpha: &[Opaque, Mask],
        shadows: true,
        meshes: &[&[Normals, Uvs, UvsB, Colours], &[Normals, Uvs, Colours]],
        ..DRAW
    },
    Draw {
        material: "FireExtension (fire/material.rs), on fire.rs's cards: premultiplied, NotShadowCaster; \
                   the hearths' mask, a breath's march and the matte's sparks",
        vertex: Some("fire"),
        fragment: Some("fire"),
        prepass_vertex: Some("fire"),
        prepass_fragment: Some("fire"),
        alpha: &[Premultiplied],
        meshes: &[CARDS],
        own: &[&[], &["FIRE_MASK"], &["MARCHED_DEPTH"], &["FLAME_MATTE"]],
        ..DRAW
    },
    Draw {
        material: "FireSmokeExtension (fire/smoke.rs), on fire.rs's cards: blended, NotShadowCaster",
        vertex: Some("fire_smoke"),
        fragment: Some("fire_smoke"),
        prepass_vertex: Some("fire_smoke"),
        prepass_fragment: Some("fire_smoke"),
        alpha: &[Blend],
        meshes: &[CARDS],
        ..DRAW
    },
    Draw {
        material: "Glow (fire/glow.rs), on the props' models",
        fragment: Some("fire_glow"),
        alpha: &[Opaque, Mask],
        shadows: true,
        meshes: &[MODEL],
        ..DRAW
    },
    Draw {
        material: "HaloExtension (terra_folk town/windows.rs), on its quad: added, NotShadowCaster",
        vertex: Some("window_halo"),
        fragment: Some("window_halo"),
        prepass_vertex: Some("window_halo"),
        prepass_fragment: Some("window_halo"),
        alpha: &[Premultiplied],
        meshes: &[CARDS],
        ..DRAW
    },
    Draw {
        material: "SpillExtension (terra_folk town/windows.rs), on its pool cards: added, NotShadowCaster",
        fragment: Some("window_spill"),
        alpha: &[Premultiplied],
        meshes: &[&[Normals, Uvs, Colours]],
        ..DRAW
    },
    Draw {
        material: "SmokeExtension (smoke.rs): blended, NotShadowCaster",
        vertex: Some("smoke"),
        fragment: Some("smoke"),
        prepass_vertex: Some("smoke"),
        prepass_fragment: Some("smoke"),
        alpha: &[Blend],
        meshes: &[CARDS],
        ..DRAW
    },
    Draw {
        material: "DriftExtension (drift.rs): blended, NotShadowCaster",
        vertex: Some("drift"),
        fragment: Some("drift"),
        prepass_vertex: Some("drift"),
        prepass_fragment: Some("drift"),
        alpha: &[Blend],
        meshes: &[CARDS],
        ..DRAW
    },
    Draw {
        material: "DustFade (terra_building loss/dust.rs): blended, NotShadowCaster",
        fragment: Some("dust"),
        alpha: &[Blend],
        meshes: &[&[Normals, Uvs]],
        ..DRAW
    },
    Draw {
        material: "BuildArrays (terra_bases arrays/mod.rs), on built meshes (draw.rs): masked; BUILD_VIEW with a view on",
        fragment: Some("build"),
        prepass_fragment: Some("build_prepass"),
        alpha: &[Mask],
        shadows: true,
        meshes: &[
            &[Normals, Uvs, UvsB, Tangents, Colours],
            &[Normals, Uvs, UvsB, Tangents, Colours, Dither],
        ],
        own: &[&[], &["BUILD_VIEW"]],
        ..DRAW
    },
    Draw {
        material: "PieceView (terra_bases views.rs), over a kit's models and built meshes",
        fragment: Some("build_view_piece"),
        alpha: &[Opaque, Mask, Blend],
        shadows: true,
        meshes: &[MODEL, &[Normals, Uvs, UvsB, Tangents, Colours, Dither]],
        ..DRAW
    },
    Draw {
        material: "GhostMaterial (terra_bases draw.rs): blended, no prepass, no shadows",
        fragment: Some("ghost"),
        alpha: &[Blend],
        meshes: &[MODEL, &[Normals, Uvs, UvsB, Tangents, Colours]],
        ..DRAW
    },
    Draw {
        material: "BodyMask (terra_walker character.rs), on skinned bodies",
        vertex: Some("body_mask"),
        prepass_vertex: Some("body_mask"),
        alpha: &[Opaque, Mask],
        shadows: true,
        meshes: &[
            &[Normals, Uvs, Tangents, Skin],
            &[Normals, Uvs, Tangents, Skin, Dither],
        ],
        ..DRAW
    },
    Draw {
        material: "MossExtension (terra_sandbox lush/floor.rs), on the floor's models, their alpha kept",
        fragment: Some("mossy"),
        alpha: &[Opaque, Mask],
        shadows: true,
        meshes: &[MODEL],
        ..DRAW
    },
];

/// Shaders of ours drawn by a pipeline of their own, which gives them no definitions.
const PIPELINES: &[(&str, &str)] = &[
    (
        "adaptation",
        "the eye's adaptation, a compute pipeline (adaptation.rs)",
    ),
    ("cloud_volume", "the clouds' march (sky/volume/render.rs)"),
    (
        "fire_haze",
        "the heat haze, a FullscreenMaterial (fire/haze.rs)",
    ),
];

/// Shaders of ours that stand in for one of Bevy's, at Bevy's path, as the game puts them
/// there (`terra_walker::skinning`); each is compiled through the Bevy shaders that import
/// it, as every skinned mesh without a vertex shader of its own is drawn.
const STAND_INS: &[(&str, &str)] = &[("skinning", "bevy_pbr::render::skinning")];

/// Bevy's shaders that only a renderer loads, so the test loads them itself: the mesh and
/// prepass shaders, which the mesh and prepass pipelines load as they are made.
const MESH_SHADER: &str = "embedded://bevy_pbr/render/mesh.wesl";
const PREPASS_SHADER: &str = "embedded://bevy_pbr/prepass/prepass.wesl";

/// The definitions every pipeline gets from the pipeline cache on a desktop GPU
/// (`bevy_render`'s `PipelineCache::new`): storage buffers are available.
fn global() -> Vec<ShaderDefVal> {
    vec![
        ShaderDefVal::UInt("AVAILABLE_STORAGE_BUFFER_BINDINGS".into(), 8),
        ShaderDefVal::Bool("AVAILABLE_STORAGE_BUFFER_BINDINGS__GE_3".into(), true),
        ShaderDefVal::Bool("AVAILABLE_STORAGE_BUFFER_BINDINGS__GE_6".into(), true),
    ]
}

/// The main camera at a quality preset (`scene.rs`): HDR, so tonemapped after; TAA, so a
/// depth and motion vector prepass and jitter; distance fog and the sky's environment map;
/// at high quality SSAO and contact shadows, so a normal prepass, and temporal soft shadows.
struct View {
    name: &'static str,
    occlusion: bool,
}

const VIEWS: &[View] = &[
    View {
        name: "high quality",
        occlusion: true,
    },
    View {
        name: "low quality",
        occlusion: false,
    },
];

/// The mesh's definitions, as both pipelines read its layout: normals and tangents only where
/// the pass has a use for them, a previous skin only beside motion vectors.
fn mesh_defs(mesh: &[Part], normals: bool, motion: bool) -> Vec<&'static str> {
    let mut defs = vec!["VERTEX_POSITIONS"];
    for has in mesh {
        defs.extend_from_slice(match has {
            Normals if normals => &["VERTEX_NORMALS"],
            Tangents if normals => &["VERTEX_TANGENTS"],
            Normals | Tangents => &[],
            Uvs => &["VERTEX_UVS", "VERTEX_UVS_A"],
            UvsB => &["VERTEX_UVS", "VERTEX_UVS_B"],
            Colours => &["VERTEX_COLORS"],
            Skin if motion => &["SKINNED", "HAS_PREVIOUS_SKIN"],
            Skin => &["SKINNED"],
            Dither => &["VISIBILITY_RANGE_DITHER"],
        });
    }
    defs
}

/// A forward pass in the view (`MeshPipeline::specialize`).
fn forward(view: &View, alpha: Blending, mesh: &[Part]) -> Vec<&'static str> {
    let mut defs = vec![
        "MESH_PIPELINE",
        "VERTEX_OUTPUT_INSTANCE_INDEX",
        "VIEW_PROJECTION_PERSPECTIVE",
        "DEPTH_PREPASS",
        "MOTION_VECTOR_PREPASS",
        "TEMPORAL_JITTER",
        "ENVIRONMENT_MAP",
        "DISTANCE_FOG",
        "PCSS_SAMPLERS_AVAILABLE",
        "MULTIPLE_LIGHT_PROBES_IN_ARRAY",
        "MULTIPLE_LIGHTMAPS_IN_ARRAY",
        "IRRADIANCE_VOLUMES_ARE_USABLE",
        "CLUSTERED_DECALS_ARE_USABLE",
    ];
    if view.occlusion {
        defs.extend_from_slice(&[
            "NORMAL_PREPASS",
            "LOAD_PREPASS_NORMALS",
            "CONTACT_SHADOWS",
            "SHADOW_FILTER_METHOD_TEMPORAL",
        ]);
        // Only where it writes depth.
        if matches!(alpha, Opaque | Mask) {
            defs.push("SCREEN_SPACE_AMBIENT_OCCLUSION");
        }
    } else {
        defs.push("SHADOW_FILTER_METHOD_HARDWARE_2X2");
    }
    defs.extend_from_slice(match alpha {
        Mask => &["MAY_DISCARD"],
        Premultiplied => &["PREMULTIPLY_ALPHA", "BLEND_PREMULTIPLIED_ALPHA"],
        Opaque | Blend => &[],
    });
    defs.extend(mesh_defs(mesh, true, true));
    defs
}

/// The view's depth, normal and motion vector prepass (`PrepassPipeline::specialize`): only
/// what writes depth is drawn in it.
fn prepass(view: &View, alpha: Blending, mesh: &[Part]) -> Option<Vec<&'static str>> {
    if !matches!(alpha, Opaque | Mask) {
        return None;
    }
    let mut defs = vec![
        "PREPASS_PIPELINE",
        "VERTEX_OUTPUT_INSTANCE_INDEX",
        "VIEW_PROJECTION_PERSPECTIVE",
        "DEPTH_PREPASS",
        "MOTION_VECTOR_PREPASS",
        "MOTION_VECTOR_PREPASS_OR_DEFERRED_PREPASS",
        "PREPASS_FRAGMENT",
        "MULTIPLE_LIGHTMAPS_IN_ARRAY",
    ];
    if view.occlusion {
        defs.extend_from_slice(&["NORMAL_PREPASS", "NORMAL_PREPASS_OR_DEFERRED_PREPASS"]);
    }
    if alpha == Mask {
        defs.push("MAY_DISCARD");
    }
    defs.extend(mesh_defs(mesh, view.occlusion, true));
    Some(defs)
}

/// The sun's shadow cascades (`queue_shadows` and the prepass pipeline): depth alone,
/// orthographic, with what does not blend as opaque cut where it may discard.
fn shadow(alpha: Blending, mesh: &[Part]) -> Vec<&'static str> {
    let mut defs = vec![
        "PREPASS_PIPELINE",
        "VERTEX_OUTPUT_INSTANCE_INDEX",
        "VIEW_PROJECTION_ORTHOGRAPHIC",
        "DEPTH_PREPASS",
        "MULTIPLE_LIGHTMAPS_IN_ARRAY",
    ];
    if alpha != Opaque {
        defs.push("MAY_DISCARD");
    }
    defs.extend(mesh_defs(mesh, false, false));
    defs
}

/// One shader compiled with one set of definitions, and what it is for.
struct Compile {
    path: String,
    defs: Vec<&'static str>,
    what: String,
}

fn shader_path(name: &str) -> String {
    format!("shaders/{name}.wesl")
}

/// Every pipeline a draw makes in every view, by the files each stage uses.
fn compiles_of(draw: &Draw, out: &mut Vec<Compile>) {
    let mut add = |file: Option<&str>, defs: &[&'static str], what: String| {
        if let Some(file) = file {
            out.push(Compile {
                path: shader_path(file),
                defs: defs.to_vec(),
                what,
            });
        }
    };
    for &alpha in draw.alpha {
        for mesh in draw.meshes {
            for own in draw.own {
                let with = |pass: &str| {
                    format!(
                        "{pass}, {alpha:?}, on a mesh with {mesh:?}, with {own:?}: {}",
                        draw.material
                    )
                };
                for view in VIEWS {
                    let mut defs = forward(view, alpha, mesh);
                    defs.extend_from_slice(own);
                    let what = with(&format!("the {} forward pass", view.name));
                    add(draw.vertex, &defs, what.clone());
                    add(draw.fragment, &defs, what);
                    if let Some(mut defs) = prepass(view, alpha, mesh) {
                        defs.extend_from_slice(own);
                        let what = with(&format!("the {} prepass", view.name));
                        add(draw.prepass_vertex, &defs, what.clone());
                        add(draw.prepass_fragment, &defs, what);
                    }
                }
                if draw.shadows {
                    let mut defs = shadow(alpha, mesh);
                    defs.extend_from_slice(own);
                    let what = with("the shadow pass");
                    add(draw.prepass_vertex, &defs, what.clone());
                    // A fragment stage only to cut what may discard.
                    if alpha != Opaque {
                        add(draw.prepass_fragment, &defs, what);
                    }
                }
            }
        }
    }
}

/// The skinning stand-in through Bevy's mesh and prepass vertex shaders, both ways Bevy hands
/// a skin its joints: storage buffers, and uniform buffers where a vertex shader may not
/// read storage.
fn stand_in_compiles(bevys: &str, out: &mut Vec<Compile>) {
    let mesh = &[Normals, Uvs, Tangents, Skin];
    for uniform in [&[][..], &["SKINS_USE_UNIFORM_BUFFERS"]] {
        let view = &VIEWS[0];
        let passes = [
            (MESH_SHADER, forward(view, Opaque, mesh), "the forward pass"),
            (
                PREPASS_SHADER,
                prepass(view, Opaque, mesh).expect("opaque"),
                "the prepass",
            ),
            (PREPASS_SHADER, shadow(Opaque, mesh), "the shadow pass"),
        ];
        for (through, mut defs, pass) in passes {
            defs.extend_from_slice(uniform);
            out.push(Compile {
                path: through.into(),
                defs,
                what: format!("{pass} in {bevys}'s place, through {through}, with {uniform:?}"),
            });
        }
    }
}

/// What naga says of a module, as wgpu asks it to before any driver sees it
/// (`wgpu-core`'s `create_shader_module`).
fn validate(_: &(), source: ShaderCacheSource, _: &ValidateShader) -> Result<(), ShaderCacheError> {
    let ShaderCacheSource::Wgsl(wgsl) = source else {
        return Err(ShaderCacheError::CreateShaderModule("not WGSL".into()));
    };
    let module = naga::front::wgsl::parse_str(&wgsl)
        .map_err(|e| ShaderCacheError::CreateShaderModule(e.emit_to_string(&wgsl)))?;
    // With binding arrays, which Bevy uses where the device has them (`MULTIPLE_LIGHT_PROBES_IN_ARRAY`).
    let capabilities = naga::valid::Capabilities::default()
        | naga::valid::Capabilities::TEXTURE_AND_SAMPLER_BINDING_ARRAY
        | naga::valid::Capabilities::TEXTURE_AND_SAMPLER_BINDING_ARRAY_NON_UNIFORM_INDEXING;
    naga::valid::Validator::new(naga::valid::ValidationFlags::all(), capabilities)
        .validate(&module)
        .map(|_| ())
        .map_err(|e| ShaderCacheError::CreateShaderModule(e.emit_to_string(&wgsl)))
}

fn assets() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

/// Every `.wesl` and `.wgsl` file in `assets/shaders`: its name without the extension, and
/// its path in the assets.
fn ours() -> Vec<(String, String)> {
    let mut files: Vec<(String, String)> = std::fs::read_dir(assets().join("shaders"))
        .expect("assets/shaders")
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            let extension = path.extension()?.to_str()?;
            if extension != "wesl" && extension != "wgsl" {
                return None;
            }
            let name = path.file_stem()?.to_str()?.to_string();
            Some((name, format!("shaders/{}", path.file_name()?.to_str()?)))
        })
        .collect();
    files.sort();
    files
}

/// A headless app with Bevy's plugins and no renderer, every shader of Bevy's and ours
/// loaded.
fn loaded(files: &[(String, String)]) -> App {
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .build()
            .disable::<WinitPlugin>()
            .disable::<AudioPlugin>()
            .disable::<LogPlugin>()
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                ..default()
            })
            .set(AssetPlugin {
                file_path: assets().to_string_lossy().into_owned(),
                ..default()
            })
            .set(RenderPlugin {
                render_creation: WgpuSettings {
                    backends: None,
                    ..default()
                }
                .into(),
                ..default()
            }),
    );
    // As `App::run` does before the first frame: some plugins load shaders as they finish.
    app.finish();
    app.cleanup();
    let server = app.world().resource::<AssetServer>().clone();
    let paths: Vec<String> = files
        .iter()
        .map(|(_, path)| path.clone())
        .chain([MESH_SHADER.to_string(), PREPASS_SHADER.to_string()])
        .collect();
    let handles: Vec<Handle<Shader>> = paths.iter().map(|path| server.load(path)).collect();
    // Bevy's libraries load in the background as ours do; done when ours are and the count
    // has stood still a while.
    let (mut count, mut still) = (0, 0);
    let started = std::time::Instant::now();
    while still < 20 {
        app.update();
        std::thread::sleep(std::time::Duration::from_millis(5));
        for (handle, path) in handles.iter().zip(&paths) {
            if let Some(LoadState::Failed(error)) = server.get_load_state(handle) {
                panic!("{path} did not load: {error}");
            }
        }
        let shaders = app.world().resource::<Assets<Shader>>();
        let ours_in = handles.iter().all(|h| shaders.contains(h));
        still = if shaders.len() == count && ours_in {
            still + 1
        } else {
            0
        };
        count = shaders.len();
        assert!(
            started.elapsed().as_secs() < 60,
            "the shaders did not load in a minute"
        );
    }
    app
}

/// What the test compiles, and the files it cannot say how the game uses.
fn plan(files: &[(String, String)]) -> (Vec<Compile>, Vec<String>) {
    let mut compiles = Vec::new();
    for draw in DRAWS {
        compiles_of(draw, &mut compiles);
    }
    for (name, why) in PIPELINES {
        compiles.push(Compile {
            path: shader_path(name),
            defs: Vec::new(),
            what: format!("as {why}"),
        });
    }
    for (_, bevys) in STAND_INS {
        stand_in_compiles(bevys, &mut compiles);
    }
    let named: BTreeSet<String> = compiles
        .iter()
        .map(|c| c.path.clone())
        .chain(STAND_INS.iter().map(|(name, _)| shader_path(name)))
        .collect();
    let mut unknown = Vec::new();
    for named in &named {
        if named.starts_with("shaders/") && !files.iter().any(|(_, path)| path == named) {
            unknown.push(format!("✗ {named}: named here, but not in assets/shaders"));
        }
    }
    // A library is compiled alone too, with the forward pass's definitions, so a function
    // no importer calls yet is validated as well.
    let sources: Vec<String> = files
        .iter()
        .map(|(_, path)| std::fs::read_to_string(assets().join(path)).expect("a shader"))
        .collect();
    for (name, path) in files {
        if named.contains(path) {
            continue;
        }
        let import = format!("super::{name}::");
        if sources.iter().any(|source| source.contains(&import)) {
            compiles.push(Compile {
                path: path.clone(),
                defs: forward(&VIEWS[0], Opaque, MODEL),
                what: "alone, as a library the others import".into(),
            });
        } else {
            unknown.push(format!(
                "✗ {path}: no material, pipeline or importer here says how the game uses it; \
                 add it to DRAWS or PIPELINES"
            ));
        }
    }
    (compiles, unknown)
}

#[test]
fn every_shader_compiles_and_validates_as_the_games_pipelines_specialize_it() {
    let files = ours();
    assert!(files.len() > 20, "found only {} shaders", files.len());
    let (compiles, mut failures) = plan(&files);
    let app = loaded(&files);
    let server = app.world().resource::<AssetServer>();
    let shaders = app.world().resource::<Assets<Shader>>();
    let mut cache: ShaderCache<(), ()> = ShaderCache::new((), validate);
    for (id, shader) in shaders.iter() {
        let mut shader = shader.clone();
        shader.shader_defs.splice(0..0, global());
        cache.set_shader(id, shader);
    }
    // The stand-ins go in Bevy's place before anything compiles, as the game swaps them in.
    for (name, bevys) in STAND_INS {
        let import = bevy::shader::ShaderImport::Custom(bevys.to_string());
        let (id, bevy_shader) = shaders
            .iter()
            .find(|(_, shader)| shader.import_path == import)
            .unwrap_or_else(|| panic!("Bevy has no shader {bevys}"));
        let source = std::fs::read_to_string(assets().join(shader_path(name))).expect("a shader");
        let mut shader = Shader::from_wesl(source, bevy_shader.path.clone());
        shader.shader_defs.splice(0..0, global());
        cache.set_shader(id, shader);
    }
    let mut done = BTreeSet::new();
    for (pipeline, compile) in compiles.iter().enumerate() {
        let mut defs = compile.defs.clone();
        defs.sort_unstable();
        defs.dedup();
        if !done.insert((compile.path.clone(), defs.clone())) {
            continue;
        }
        let mut vals = vec![ShaderDefVal::UInt(
            "MATERIAL_BIND_GROUP".into(),
            bevy::pbr::MATERIAL_BIND_GROUP_INDEX as u32,
        )];
        // Always given by the mesh pipeline: ScreenSpaceTransmission's default quality.
        vals.push(ShaderDefVal::Int(
            "SCREEN_SPACE_SPECULAR_TRANSMISSION_BLUR_TAPS".into(),
            8,
        ));
        vals.extend(defs.iter().map(|&def| ShaderDefVal::from(def.to_string())));
        let id = server.load::<Shader>(compile.path.clone()).id();
        if let Err(error) = cache.get(pipeline, id, &vals) {
            failures.push(format!("✗ {} as {}:\n{error}", compile.path, compile.what));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} compiles failed:\n{}",
        failures.len(),
        done.len(),
        failures.join("\n\n")
    );
}
