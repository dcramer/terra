// The kit's rain (kit/weather.rs, iso-kit j19): one mesh of quads, each a seed (its position,
// 0 to 1 each way), moved here. A streak falls through the box and wraps, stretched along
// its velocity by the shutter and turned to face the eye about that axis; a splash is a
// small crown at the floor, flicked up and fading over its life, then somewhere else. Each
// is lit by the lamps near it (weather.rs `lit`, kept the same) and added over the frame.

import bevy_pbr::render::{
    mesh_view_bindings::view,
    view_transformations::position_world_to_clip,
};

struct Lamp {
    at: vec4<f32>,
    colour: vec4<f32>,
    dir: vec4<f32>,
    cone: vec4<f32>,
}

// Must match weather.rs `Look`, field for field.
struct Look {
    lo: vec4<f32>,
    size: vec4<f32>,
    wind: vec4<f32>,
    drops: vec4<f32>,
    floor: vec4<f32>,
    ambient: vec4<f32>,
    lamps: array<Lamp, 16>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> look: Look;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var floor_map: texture_2d<f32>;

struct Vertex {
    @location(0) seed: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) uv_b: vec2<f32>,
}

struct Out {
    @builtin(position) position: vec4<f32>,
    @location(0) colour: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) kind: f32,
}

fn hash2(p: vec2<f32>) -> vec2<f32> {
    let q = vec2(dot(p, vec2(127.1, 311.7)), dot(p, vec2(269.5, 183.3)));
    return fract(sin(q) * 43758.5453);
}

// The floor's surface (x) and whether water stands there (y), the nearest cell.
fn floor_at(xz: vec2<f32>) -> vec2<f32> {
    let n = vec2<i32>(textureDimensions(floor_map));
    let c = clamp(vec2<i32>(round((xz - look.floor.xy) / look.floor.z)), vec2(0), n - 1);
    return textureLoad(floor_map, c, 0).xy;
}

fn lit(p: vec3<f32>) -> vec3<f32> {
    var e = look.ambient.rgb;
    for (var i = 0; i < i32(look.drops.z); i += 1) {
        let l = look.lamps[i];
        let d = l.at.xyz - p;
        let d2 = max(dot(d, d), 0.04);
        let r = d2 / (l.at.w * l.at.w);
        let f = pow(clamp(1.0 - r * r, 0.0, 1.0), 2.0);
        var s = 1.0;
        if l.colour.w > 0.5 {
            let c = dot(-normalize(d), l.dir.xyz);
            let k = smoothstep(l.dir.w, l.cone.x, c);
            s = k * k;
        }
        e += l.colour.rgb * f * s / d2;
    }
    return e;
}

const HIDDEN: vec4<f32> = vec4(2.0, 2.0, 2.0, 1.0);

@vertex
fn vertex(v: Vertex) -> Out {
    var out: Out;
    let t = look.lo.w;
    let size = look.size.xyz;
    let eye = view.world_position;
    out.uv = v.uv;
    out.kind = v.uv_b.x;
    if v.uv_b.x < 0.5 {
        let vel = vec3(look.wind.x, -look.size.w, look.wind.y);
        let p = look.lo.xyz + fract(v.seed + vel * t / size) * size;
        let fl = floor_at(p.xz);
        let axis = normalize(vel);
        let len = length(vel) * look.wind.z;
        let side = normalize(cross(axis, eye - p));
        let world = p + axis * len * (v.uv.y - 0.5) + side * look.wind.w * v.uv.x;
        out.colour = lit(p) * look.drops.x / 3.14159265 * view.exposure;
        out.position = select(position_world_to_clip(world), HIDDEN, p.y < fl.x);
        return out;
    }
    let life = look.drops.w;
    let ph = t / life + v.seed.y;
    let k = floor(ph);
    let a = fract(ph);
    let xz = look.lo.xz + hash2(v.seed.xz * 97.0 + k * 0.618) * size.xz;
    let fl = floor_at(xz);
    // Lying on the floor (2 cm over its sampled surface, clear of the paving's relief): seen
    // from the ARPG's camera a standing crown is foreshortened to a sub-pixel line (iso-kit
    // j20), where a drop's hit reads from above as a bright fleck spreading to a ring. A
    // drop's crown is a few cm across: at 5 to 18 cm the rings read as a sheet of bubbles
    // (round 261107-0056).
    let p = vec3(xz.x, fl.x + 0.02, xz.y);
    let r = mix(0.012, 0.04, sqrt(a)) * look.drops.y;
    let world = p + vec3(r * v.uv.x, 0.0, r * (2.0 * v.uv.y - 1.0));
    // A crown of droplets glints back the lamp over it, 25 times a streak's share: at three
    // it was dimmer than the wet paving under it and did not show (iso-kit j20).
    out.colour = lit(p + vec3(0.0, 0.05, 0.0)) * look.drops.x * 25.0 / 3.14159265 * view.exposure
        * (1.0 - a) * (1.0 - a);
    out.position = select(position_world_to_clip(world), HIDDEN, fl.y > 0.5);
    return out;
}

@fragment
fn fragment(in: Out) -> @location(0) vec4<f32> {
    if in.kind < 0.5 {
        // A streak: soft across, its ends fading.
        let across = 1.0 - in.uv.x * in.uv.x;
        let along = sin(3.14159265 * in.uv.y);
        return vec4(in.colour * across * across * along, 0.0);
    }
    // A splash from above: the hit's bright fleck inside a ring of thrown droplets.
    let q = vec2(in.uv.x, 2.0 * in.uv.y - 1.0);
    let d = length(q);
    let ring = exp(-pow((d - 0.7) / 0.15, 2.0));
    let drops = step(0.55, fract(sin(floor(atan2(q.y, q.x) * 6.0) * 91.7) * 4375.85));
    let fleck = exp(-pow(d / 0.22, 2.0));
    return vec4(in.colour * (ring * (0.4 + 0.6 * drops) + fleck), 0.0);
}
