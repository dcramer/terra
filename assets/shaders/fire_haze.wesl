// Heat haze over the fires (docs/fire.md "Heat haze", crates/terra_world/src/fire/haze.rs): the
// HDR picture before the tonemapper, sampled again a little off where the hot air over a fire
// stands, the offset two octaves of noise rising with the air. As Unreal's and HDRP's
// distortion: the scene behind seen through it, flames and all, so nothing is greyed or drawn
// over; only bent. Each column is a box on the screen: its foot, how high it reaches and its
// half width there, all in the picture's 0 to 1, and how far it bends, as a share of that.

import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput;

const COLUMNS: u32 = 4u;

struct HeatHaze {
    // Per column: the foot's x and y, the top's y, the half width at the foot.
    columns: array<vec4<f32>, 4>,
    // Per column: how far it bends (of the half width) and its seed.
    looks: array<vec4<f32>, 4>,
    time: f32,
    count: u32,
    aspect: f32,
    pad: f32,
}

@group(0) @binding(0) var screen: texture_2d<f32>;
@group(0) @binding(1) var screen_sampler: sampler;
@group(0) @binding(2) var<uniform> haze: HeatHaze;

fn hash(p: vec2<f32>) -> f32 {
    let q = fract(p * vec2<f32>(0.1031, 0.1030));
    let r = q + dot(q, q.yx + 33.33);
    return fract((r.x + r.y) * r.x);
}

// Signed value noise, -1 to 1.
fn noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let n = mix(mix(hash(i), hash(i + vec2(1.0, 0.0)), u.x), mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), u.x), u.y);
    return n * 2.0 - 1.0;
}

// The picture at a point between pixels, filtered by hand (the pass's sampler is the nearest
// texel's, which would bend the picture in steps of whole pixels).
fn sample(uv: vec2<f32>) -> vec4<f32> {
    let size = vec2<f32>(textureDimensions(screen));
    let p = clamp(uv * size - 0.5, vec2(0.0), size - 1.001);
    let i = vec2<i32>(floor(p));
    let f = fract(p);
    let a = textureLoad(screen, i, 0);
    let b = textureLoad(screen, i + vec2(1, 0), 0);
    let c = textureLoad(screen, i + vec2(0, 1), 0);
    let d = textureLoad(screen, i + vec2(1, 1), 0);
    return mix(mix(a, b, f.x), mix(c, d, f.x), f.y);
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    var offset = vec2<f32>(0.0);
    for (var k = 0u; k < min(haze.count, COLUMNS); k++) {
        let c = haze.columns[k];
        let l = haze.looks[k];
        let w = max(c.w, 1e-5);
        // Where in the column, in its foot's half widths: across, and up from the foot.
        let up = (c.y - in.uv.y) / w;
        let high = max((c.y - c.z) / w, 1e-3);
        // The column widens as the hot air mixes in, near twice as wide by its top.
        let across = (in.uv.x - c.x) * haze.aspect / w / (1.0 + 0.8 * clamp(up / high, 0.0, 1.0));
        let side = 1.0 - smoothstep(0.35, 1.0, abs(across));
        let rise = smoothstep(0.0, 0.12 * high, up) * (1.0 - smoothstep(0.4 * high, high, up));
        let mask = side * rise;
        if (mask <= 0.0) {
            continue;
        }
        // The air rises about four half widths a second, its cells a third and a seventh of
        // a half width, the small ones faster, swaying as they go.
        let q = vec2(across, up) + l.y * 17.0;
        let t = haze.time;
        let big = vec2(noise(q * 3.0 + vec2(0.0, -t * 4.0 * 3.0)), noise(q * 3.0 + vec2(5.2, -t * 4.0 * 3.0 + 1.3)));
        let small = vec2(noise(q * 7.0 + vec2(1.7, -t * 6.0 * 7.0)), noise(q * 7.0 + vec2(9.4, -t * 6.0 * 7.0 + 2.1)));
        // Hot air bends the light across more than along its rise.
        let bend = (0.65 * big + 0.35 * small) * vec2(1.0, 0.6);
        offset += bend * l.x * w * mask * vec2(1.0 / haze.aspect, 1.0);
    }
    if (all(offset == vec2(0.0))) {
        return textureLoad(screen, vec2<i32>(in.position.xy), 0);
    }
    return sample(in.uv + offset);
}
