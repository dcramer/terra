// The clouds' 2D noise, shared by the sky's layer (sky.wesl) and the volumes (cloud_volume.wesl)
// so both put their clouds in one place: random values on a grid, blended between, on a lattice
// that repeats every PERIOD cells so the clock's offsets (sky/clouds.rs) wrap with no seam.

// The lattice's period (clouds::PERIOD).
const PERIOD: f32 = 256.0;

fn hash(p: vec3<f32>) -> f32 {
    let q = fract(p * vec3<f32>(0.1031, 0.1030, 0.0973));
    let r = q + dot(q, q.yzx + 33.33);
    return fract((r.x + r.y) * r.z);
}

fn wrapped_hash(cell: vec3<f32>) -> f32 {
    return hash(cell - PERIOD * floor(cell / PERIOD));
}

fn wrapped_noise(p: vec3<f32>) -> f32 {
    let cell = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let x00 = mix(wrapped_hash(cell), wrapped_hash(cell + vec3<f32>(1.0, 0.0, 0.0)), u.x);
    let x10 = mix(wrapped_hash(cell + vec3<f32>(0.0, 1.0, 0.0)), wrapped_hash(cell + vec3<f32>(1.0, 1.0, 0.0)), u.x);
    let x01 = mix(wrapped_hash(cell + vec3<f32>(0.0, 0.0, 1.0)), wrapped_hash(cell + vec3<f32>(1.0, 0.0, 1.0)), u.x);
    let x11 = mix(wrapped_hash(cell + vec3<f32>(0.0, 1.0, 1.0)), wrapped_hash(cell + vec3<f32>(1.0, 1.0, 1.0)), u.x);
    return mix(mix(x00, x10, u.y), mix(x01, x11, u.y), u.z);
}
