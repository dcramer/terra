// SPIKE: how much of the pixel the march found flame (not soot), for the tonemapper's film
// mask (`FIRE_MASK`, crates/terra_world/src/fire/film.rs), as a hearth's flame says it.
var<private> FLAME_SHARE: f32 = 0.0;
// The volume's flame as bright as a hearth's (`HEARTH_BRIGHTNESS`), through the same film.
const VOLUME_BRIGHTNESS: f32 = 12.0;
// Extinction a metre at density 1: flame gas, and soot.
const VOLUME_FLAME_K: f32 = 9.0;
const VOLUME_SOOT_K: f32 = 14.0;
const VOLUME_STEPS: i32 = 96;

// The breath along the ray through a fragment of its hull's far side, front to back: its
// light, already exposed, and how much of what is behind it it covers. The ray, its box and
// its end at the depth prepass are Bevy's volumetric fog's (bevy_pbr volumetric_fog.wesl,
// MIT/Apache) and the game's earlier rod march; the medium is emissive-absorbing, so a thick
// flame shows its outer gas: dL = sigma (Le - L) ds.
@if(!PREPASS_PIPELINE)
fn breath(in: VertexOutput) -> vec4<f32> {
    let b = breath_of(u32(in.uv.x + 0.5));
    if b.reach <= 0.0 || b.left <= 0.0 || ROD_OFF {
        discard;
    }
    let time = fire.kind.z;
    let eye = view.world_position;
    let far_side = in.world_position.xyz;
    let dir = normalize(far_side - eye);
    var far = distance(eye, far_side);
@if(DEPTH_PREPASS)
    {
        let here = depth_ndc_to_view_z(in.position.z);
        let scene = depth_ndc_to_view_z(prepass_depth(in.position, 0u));
        far = min(far, far * scene / min(here, -1.0e-4));
    }
    let o = eye - b.mouth;
    let ol = vec3(dot(o, b.lift), dot(o, b.side), dot(o, b.way));
    let dl = vec3(dot(dir, b.lift), dot(dir, b.side), dot(dir, b.way));
    let box = hull_box(b);
    let inv = 1.0 / select(dl, vec3(1.0e-6), abs(dl) < vec3(1.0e-6));
    let t0 = (box[0] - ol) * inv;
    let t1 = (box[1] - ol) * inv;
    let near = max(max(max(min(t0.x, t1.x), min(t0.y, t1.y)), min(t0.z, t1.z)), 0.0);
    if near >= far {
        discard;
    }
    let across = max(length(dl.xy), 0.25);
    let jitter = fract(52.9829189 * fract(dot(in.position.xy, vec2(0.06711056, 0.00583715)) + time * 7.13));
    var s = near;
    var first = true;
    var light = vec3(0.0);
    var through = 1.0;
    var share = 0.0;
    for (var i = 0; i < VOLUME_STEPS; i++) {
        if s >= far || through < 0.01 {
            break;
        }
        let l = ol + dl * s;
        let gas = breath_gas(l, b, time);
        if gas.w > 0.0 {
            s += max(gas.w, 0.05);
            continue;
        }
        let r = stream_radius(clamp(l.z, 0.0, b.reach), b);
        var ds = clamp(0.22 * r / across, 0.04, 0.45);
        if first {
            first = false;
            s += ds * jitter;
            continue;
        }
        ds = min(ds, far - s);
        if gas.x > 0.0 {
            let sigma = gas.x * mix(VOLUME_FLAME_K, VOLUME_SOOT_K, gas.z);
            let a = 1.0 - exp(-sigma * ds);
            let flame = ramp_row(gas.y, BREATH_ROW).rgb * VOLUME_BRIGHTNESS;
            // Soot is black, faintly lit by the flame it still wraps.
            let soot = SOOT_BLACK + ramp_row(min(gas.y + 0.15, 1.0), BREATH_ROW).rgb * VOLUME_BRIGHTNESS * 0.015;
            light += through * a * mix(flame, soot, gas.z);
            share += through * a * (1.0 - gas.z);
            through *= 1.0 - a;
        }
        s += ds;
    }
    let cover = 1.0 - through;
    if cover < 0.002 {
        discard;
    }
    FLAME_SHARE = share;
    return vec4(light * on_screen(), cover);
}

