// A cap tree's cap (`terra_world::grown::caps`, docs/realm-two.md): smooth skin in its
// kind's colour, mottled and spotted pale, over gills that shine in their own, where bark
// would be. A lathed limb says it is skin in its texture coordinates (`build::lathe`): how
// far round the cap in x, 0 to 1, and y below zero.

// How many gills round a cap.
const GILLS: f32 = 140.0;

struct CapSkin {
    colour: vec3<f32>,
    rough: f32,
    // What its gills shine, to be added to its light.
    glow: vec3<f32>,
}

fn is_cap_skin(uv: vec2<f32>) -> bool {
    return uv.y < -0.5;
}

// `tint` its kind's colour; `facing_up` its surface against the sky's way, 1 its top and -1
// its underside; `broad` and `fine` noise 0 to 1, a metre or two across and a hand across;
// `gills` their colour and how brightly they shine.
fn cap_skin(tint: vec3<f32>, uv: vec2<f32>, facing_up: f32, broad: f32, fine: f32, gills: vec4<f32>) -> CapSkin {
    var skin: CapSkin;
    // Its top: darker and lighter in broad patches, with pale warts where the fine noise
    // peaks, fewer toward its rim.
    let wart = smoothstep(0.8, 0.86, fine);
    let top = mix(tint * (0.3 + 0.3 * broad), mix(tint, vec3<f32>(1.0), 0.45) * 0.7, wart);
    // Its underside: gills from the stalk to the rim, each a ridge with shade between,
    // thinned to their mean where they are finer than a pixel.
    let round = uv.x * GILLS;
    let fineness = clamp(1.0 - fwidth(round) * 1.5, 0.0, 1.0);
    let gill = mix(0.5, 0.5 + 0.5 * cos(round * 6.2831853), fineness);
    let under = 1.0 - smoothstep(-0.35, 0.1, facing_up);
    skin.colour = mix(top, gills.rgb * (0.25 + 0.75 * gill), under);
    skin.rough = mix(0.5, 0.9, under);
    skin.glow = gills.rgb * gills.a * under * (0.2 + 0.8 * gill);
    return skin;
}
