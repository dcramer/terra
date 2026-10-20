// SPIKE (spike/campfire-volume, D133): a campfire's flame marched through a box over its
// logs, in place of its flame cards. The march is the breath spike's (6db86f20, after Bevy's
// volumetric fog, bevy_pbr volumetric_fog.wesl, MIT/Apache): emissive-absorbing gas,
// dL = sigma (Le - L) ds, ended at the depth prepass. The gas: temperature rising off the coal
// bed in tongues, a three-octave value noise carried up with the rising gas and swept by a
// divergence-free (curl) field, the cheap analytic stand-in for Bridson's curl noise (2007);
// colour by temperature through the campfire's ramp row; density from the temperature, so
// the tips thin out to nothing.

// The box over the logs, in the hearth's frame (east, north, up), metres.
const CF_HALF: f32 = 0.5;
const CF_TALL: f32 = 1.5;
// Light at full temperature, on the ramp's luminance, as `HEARTH_BRIGHTNESS` is for a card.
const CF_BRIGHT: f32 = 12.0;
// Extinction a metre at density 1.
const CF_K: f32 = 10.0;
const CF_STEPS: i32 = 56;
// How fast the gas rises, metres a second, and how far the curl field sweeps it.
const CF_RISE: f32 = 1.5;
const CF_SWIRL: f32 = 0.22;
var<private> CF_SHARE: f32 = 0.0;

// The camera-facing quad that covers the box's bounding sphere, at the sphere's near side,
// so whatever opaque stands in front of it stands in front of all the gas.
fn campfire_card(corner: vec2<f32>, foot: vec3<f32>, up: vec3<f32>, slot: u32, heat: f32) -> Out {
    var out: Out;
    let mid = foot + up * (CF_TALL * 0.5);
    let rad = length(vec3(CF_HALF, CF_HALF, CF_TALL * 0.5)) * 1.03;
    let eye = view.world_position;
    let to = mid - eye;
    let d = length(to);
    let fwd = to / max(d, 1.0e-4);
    let inside = d <= rad + 0.2;
    let dist = select(d - rad, 0.15, inside);
    let half = select(rad * dist / sqrt(max(d * d - rad * rad, 1.0e-4)), 20.0, inside) * 1.05;
    var side = cross(fwd, up);
    side = select(normalize(side), vec3(1.0, 0.0, 0.0), dot(side, side) < 1.0e-6);
    let rise = cross(side, fwd);
    let c = (corner - vec2(0.5)) * 2.0;
    let world = eye + fwd * dist + (side * c.x + rise * c.y) * half;
    out.clip = position_world_to_clip(world);
    out.uv = corner;
    out.life = vec4(1.5, heat, f32(slot), 6.0);
    out.size = CF_TALL;
    out.world = world;
    return out;
}

// A divergence-free field: the curl of a potential of sines, so the gas it sweeps neither
// gathers nor thins, only turns.
fn cf_curl(p: vec3<f32>, t: f32) -> vec3<f32> {
    let k1 = 4.1;
    let k2 = 6.7;
    let dxy = k1 * cos(k1 * p.y + 1.7 * t);
    let dxz = k2 * cos(k2 * p.z - 2.3 * t);
    let dyz = k1 * cos(k1 * p.z + 1.9 * t);
    let dyx = k2 * cos(k2 * p.x + 1.1 * t);
    let dzx = k1 * cos(k1 * p.x - 1.3 * t);
    let dzy = k2 * cos(k2 * p.y + 2.1 * t);
    return vec3(dzy - dyz, dxz - dzx, dyx - dxy) / (k1 + k2);
}

// The gas at a point of the box (x, y across, z up, from the coal bed): its temperature 0
// to 1 and its density.
fn cf_gas(l: vec3<f32>, t: f32, seed: f32, tall: f32) -> vec2<f32> {
    let h = l.z / tall;
    if h >= 1.0 || h <= 0.0 {
        return vec2(0.0);
    }
    // The tongues stand round the stack's middle, narrowing up it, swaying as a whole.
    let sway = vec2(sin(t * 1.3 + seed * 6.0), cos(t * 0.9 + seed * 4.0)) * 0.06 * h;
    let rho = length(l.xy - sway);
    let reach = mix(0.46, 0.12, pow(h, 0.8));
    let env = 1.0 - smoothstep(reach * 0.55, reach, rho);
    if env <= 0.0 {
        return vec2(0.0);
    }
    // Carried up with the gas and swept by the curl field the more the higher it has risen.
    let swept = l + cf_curl(l * vec3(1.0, 1.0, 0.7) - vec3(0.0, 0.0, t * 0.6), t) * CF_SWIRL * (0.25 + h);
    let s = vec3(3.4, 3.4, 1.35);
    let o = vec3(seed * 17.0, seed * 31.0, 0.0);
    let n0 = noise3(swept * s + o - vec3(0.0, 0.0, t * CF_RISE * s.z));
    let n1 = noise3(swept * s * 2.03 + o + vec3(17.1, 3.7, 9.2) - vec3(0.0, 0.0, t * CF_RISE * 1.4 * s.z * 2.03));
    let n2 = noise3(swept * s * 4.07 + o + vec3(5.3, 29.4, 1.9) - vec3(0.0, 0.0, t * CF_RISE * 1.9 * s.z * 4.07));
    let n = (n0 * 0.5714 + n1 * 0.2857 + n2 * 0.1429 - 0.5) * 2.6;
    // Burning where the noise clears a threshold rising with height: whole over the coals,
    // separate licks with dark between them higher, only the odd tip near the top.
    let f = n + 0.75 * pow(1.0 - h, 1.5) * env - 0.3 - 0.75 * h - 0.4 * (1.0 - env);
    let temp = saturate(f / 0.45) * smoothstep(0.0, 0.06, l.z);
    return vec2(temp, temp * sqrt(temp));
}

@if(!PREPASS_PIPELINE)
fn campfire_volume(in: VertexOutput) -> vec4<f32> {
    let slot = u32(in.world_normal.z + 0.5);
    let a = slots[slot * 2u];
    let b = slots[slot * 2u + 1u];
    let foot = a.xyz;
    let heat = b.x;
    let seed = b.y;
    let glow = max(b.w, 0.0);
    let up = normalize(foot);
    let east = normalize(cross(up, vec3(0.0, 0.0, 1.0)) + vec3(1.0e-4));
    let north = cross(east, up);
    let time = fire.kind.z;
    let eye = view.world_position;
    let card = in.world_position.xyz;
    let dir = normalize(card - eye);
    var far = 1.0e6;
@if(DEPTH_PREPASS)
    {
        let here = depth_ndc_to_view_z(in.position.z);
        let scene = depth_ndc_to_view_z(prepass_depth(in.position, 0u));
        far = distance(eye, card) * scene / min(here, -1.0e-4);
    }
    // The wind leans the gas over, more the higher it is.
    let wind = fire.wind.xyz;
    let lean = vec2(dot(wind, east), dot(wind, north)) * 0.04;
    let o = eye - foot;
    let ol = vec3(dot(o, east), dot(o, north), dot(o, up));
    let dl = vec3(dot(dir, east), dot(dir, north), dot(dir, up));
    let lo = vec3(-CF_HALF, -CF_HALF, 0.0);
    let hi = vec3(CF_HALF, CF_HALF, CF_TALL);
    let inv = 1.0 / select(dl, vec3(1.0e-6), abs(dl) < vec3(1.0e-6));
    let t0 = (lo - ol) * inv;
    let t1 = (hi - ol) * inv;
    let near = max(max(max(min(t0.x, t1.x), min(t0.y, t1.y)), min(t0.z, t1.z)), 0.0);
    let leave = min(min(max(t0.x, t1.x), max(t0.y, t1.y)), max(t0.z, t1.z));
    far = min(far, leave);
    if near >= far {
        discard;
    }
    // Its flicker raises and lowers its tips, as the cards' height breathes.
    let tall = CF_TALL * (1.0 + (glow - 1.0) * 0.8);
    let ds = max((far - near) / f32(CF_STEPS), 0.02);
    let jitter = fract(52.9829189 * fract(dot(in.position.xy, vec2(0.06711056, 0.00583715)) + time * 7.13));
    var s = near + ds * jitter;
    var light = vec3(0.0);
    var through = 1.0;
    for (var i = 0; i < CF_STEPS; i++) {
        if s >= far || through < 0.01 {
            break;
        }
        var l = ol + dl * s;
        l = vec3(l.xy - lean * l.z, l.z);
        let gas = cf_gas(l, time, seed, tall);
        if gas.y > 0.0 {
            let st = min(ds, far - s);
            let alpha = 1.0 - exp(-gas.y * CF_K * st);
            // Hottest over the coals, cooling up the tongue and out to its edges.
            let hue = saturate(gas.x * mix(1.05, 0.7, saturate(l.z / tall)));
            light += through * alpha * ramp_row(hue, 0.0).rgb;
            through *= 1.0 - alpha;
        }
        s += ds;
    }
    var cover = 1.0 - through;
    if cover < 0.002 {
        discard;
    }
    light *= CF_BRIGHT * (0.85 + 0.15 * heat) * glow * on_screen();
    // By day pale and nearly see-through, as the hearth's cards go (`shade`).
    let day = smoothstep(1.0, 3.5, fire.exposure.y - log2(view.exposure) - adapted.x);
    let luma = dot(light, vec3(0.2126, 0.7152, 0.0722));
    light = mix(light, mix(light, vec3(luma) * vec3(1.15, 0.95, 0.55), 0.4), day) * mix(1.0, 0.5, day);
    cover *= mix(1.0, 0.3, day);
    CF_SHARE = cover;
    return vec4(light, cover);
}
