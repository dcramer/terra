// SPIKE (spike/breath-volume, D133): the whole breath as one raymarched volume, the cinematic
// tier's question. Its fields are split as production pyro's are (Houdini's and EmberGen's
// density, temperature and soot; Fong et al., "Production Volume Rendering", SIGGRAPH 2017):
// each a function of the point and the time, the temperature coloured through the ramp (a
// blackbody's, tools/fire/ramp.ts), the density and soot absorbing (Beer-Lambert). Its noise
// is carried along the stream with the gas (each parcel's `flow`) and domain-warped, so the
// gas boils as it flies. The gas at a point in the jet's frame: how dense, how hot (the ramp's
// heat, 0 to 1), how much of it is soot (0 to 1); w how far outside any gas the point is
// (positive: empty, a distance safe to step over).
fn breath_gas(l: vec3<f32>, b: Breath, time: f32) -> vec4<f32> {
    let rb = ball_radius(b) * (0.45 + 0.55 * b.ball);
    let mid = ball_middle(b, rb);
    let rel = l - mid;
    let zc = clamp(l.z, 0.0, b.reach);
    let r = stream_radius(zc, b);
    let rho = length(l.xy);
    let slope = 1.0 + b.spread * 1.7;
    let empty = min(
        max((rho - 1.8 * r) / slope, max(-l.z - 0.3, l.z - b.reach - 0.3)),
        select(1.0e5, length(rel) - rb * 1.6, b.ball > 0.01),
    );
    if empty > 0.0 {
        return vec4(0.0, 0.0, 0.0, empty);
    }
    let zf = saturate(l.z / b.reach);
    let flow = JET_SPEED * time - l.z;
    let open = smoothstep(0.0, 0.4, zf);
    // Its line wanders as a hose's does, more the further it has flown.
    let wander = vec2(noise3(vec3(flow * 0.09, 1.7, 0.0)), noise3(vec3(flow * 0.09, 7.3, 0.0))) - 0.5;
    let q = (l.xy - wander * r * 0.9 * open) / r;
    // The noise across by the stream's radius, along by the parcel, warped by itself.
    let p = vec3(q * 1.3, flow * 0.55);
    let w = vec3(noise3(p * 0.6 + 4.1), noise3(p * 0.6 + 9.7), noise3(p * 0.6 + 2.3)) - 0.5;
    let e = fbm3(p + w * mix(0.8, 2.2, open));
    let fine = noise3(p * 3.1 + w * 1.5 + 13.0);
    let u = length(q) + (e - 0.5) * mix(0.7, 1.9, open) + (fine - 0.5) * 0.35 * open;
    var stream = 1.0 - smoothstep(0.55, 1.0, u);
    // Its tip breaks into blobs.
    let tear = smoothstep(0.5, 1.0, zf) * 0.6;
    stream *= smoothstep(tear - 0.15, tear + 0.15, e + 0.15 * (1.0 - zf));
    stream *= smoothstep(-0.2, 0.2, l.z) * (1.0 - smoothstep(b.front - 1.5, b.front + 0.5, l.z)) * smoothstep(b.back - 0.5, b.back + 1.5, l.z);
    // Hottest on its line at the jaw, cooling outward in hot and cool strands, and along it.
    let core = mix(1.0, 0.78, smoothstep(0.1, 1.0, zf));
    let hot_s = core - 0.55 * smoothstep(0.15, 1.0, u) + 0.35 * (e - 0.5) + 0.2 * (fine - 0.5);
    // The fireball where it strikes: lumpy balls rolling up off the face, hot inside and at
    // the contact, cooling up to a cap of soot.
    var ball = 0.0;
    var hot_b = 0.0;
    var soot_b = 0.0;
    if b.ball > 0.01 && length(rel) < rb * 1.6 {
        let pb = rel / rb - b.up * time * 0.7;
        let wb = vec3(noise3(pb * 1.8 + 3.3), noise3(pb * 1.8 + 7.9), noise3(pb * 1.8 + 1.4)) - 0.5;
        let lumps = fbm3(pb * 1.5 + wb * 1.6);
        let nb = noise3(pb * 4.5 + wb * 2.0);
        let d = length(rel) / rb;
        ball = (1.0 - smoothstep(0.55, 1.0, d + (lumps - 0.5) * 1.2 + (nb - 0.5) * 0.3)) * b.ball;
        let high = dot(rel, b.up) / rb;
        hot_b = 0.95 - 0.45 * d - 1.1 * (lumps - 0.5) - 0.5 * (nb - 0.5) - 0.55 * smoothstep(-0.1, 0.9, high);
        soot_b = smoothstep(0.35, 0.95, high + 0.6 * (lumps - 0.5)) * smoothstep(0.5, 1.0, d + 0.3 * (nb - 0.5));
        stream *= 1.0 - smoothstep(0.05, 0.4, ball);
    }
    let dense = stream + ball;
    if dense < 1.0e-3 {
        return vec4(0.0);
    }
    let hot = (stream * hot_s + ball * hot_b) / dense;
    // Soot where the stream has cooled toward its tip.
    let soot_s = smoothstep(0.35, 0.9, zf) * (1.0 - smoothstep(0.2, 0.45, hot_s));
    let soot = (stream * soot_s + ball * soot_b) / dense;
    return vec4(min(dense, 1.0) * b.left, saturate(hot), soot, 0.0);
}

