// Fire's flames, embers and jets (D377, crates/terra_world/src/fire.rs): cards placed here
// from each blaze's slot of a buffer, the card's seed and the time, as smoke's are (D325).
// Every corner of the mesh is at the origin; its first texture coordinate is the corner, its
// second the slot and the card's place among the slot's.
//
// A blaze's flame is a body sheet standing where it burns and flamelets breaking off its top
// (research/2026-10-16-real-time-fire-rendering.md), upright and turned to the eye about the
// up: its shape a mask eroded by gradient-noise turbulence scrolled up through it, as games'
// procedural fire is drawn, its colour glowing soot's at the temperature of that part of it,
// from 1050 K at its edges to 1650 K in a fierce one's heart (`soot`). An ember is a spark
// thrown up on the plume, slowing, carried downwind, stretched along its flight and winking
// out. A jet is a stream of flame breathed out along a way, widening in a cone, hottest at
// the mouth. All are light, premultiplied over what is behind them and covering a little of
// it as soot does, in the camera's exposure; faded in where they meet what stands behind.

import bevy_pbr::render::{
    mesh_view_bindings::view,
    view_transformations::{position_world_to_clip, depth_ndc_to_view_z},
};

@if(PREPASS_PIPELINE)
import bevy_pbr::prepass::io::{Vertex, VertexOutput, decompress_vertex};
@if(!PREPASS_PIPELINE)
import bevy_pbr::render::forward_io::{Vertex, VertexOutput, FragmentOutput, decompress_vertex};
@if(DEPTH_PREPASS && !PREPASS_PIPELINE)
import bevy_pbr::prepass::utils::prepass_depth;
@if(!PREPASS_PIPELINE)
import bevy_pbr::render::mesh_view_bindings::{view_transmission_texture, view_transmission_sampler};

struct Fire {
    // x which (0 flames, 1 embers, 2 jets), y how many slots are filled, z the time.
    kind: vec4<f32>,
    // The wind, metres a second, and its strength.
    wind: vec4<f32>,
    // Each jet's from and reach; way and spread; on, and since when.
    jets: array<vec4<f32>, 6>,
    // The exposure weight, and the log2 of the exposure multiplier at dusk
    // (crates/terra_world/src/fire/exposure.rs).
    exposure: vec4<f32>,
}

// Field order must match FireExtension in crates/terra_world/src/fire.rs.
@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> fire: Fire;
@group(constants::MATERIAL_BIND_GROUP) @binding(101) var<storage, read> slots: array<vec4<f32>>;
@group(constants::MATERIAL_BIND_GROUP) @binding(102) var flipbook: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(103) var flipbook_sampler: sampler;
// What breaths have let go: soot, then drops, each where and when, then its going and luck.
@group(constants::MATERIAL_BIND_GROUP) @binding(104) var<storage, read> puffs: array<vec4<f32>>;
// The exposure the eye is adapted to (crates/terra_world/src/adaptation.rs): x the stops the
// tonemapper multiplied the last frame by, after Bevy's auto exposure.
@group(constants::MATERIAL_BIND_GROUP) @binding(105) var<storage, read> adapted: vec4<f32>;
// What a fire drawn on the screen's scale is multiplied by for the main pass: the exposure
// weight's factor (docs/fire.md, "The exposure rule"), `(k / k_dusk)^(1 - w)` for the frame's
// exposure multiplier `k`, the camera's times the eye's adaptation, so at w = 1 it is on the
// screen's scale whatever the eye does; then the adaptation the tonemapper applies, undone.
fn on_screen() -> f32 {
    let stops = log2(view.exposure) + adapted.x;
    let weighted = exp2((1.0 - fire.exposure.x) * (stops - fire.exposure.y));
    return weighted / exp2(adapted.x);
}

// How many soot puffs come before the drops; must match SOOT_PUFFS in fire.rs.
const SOOT_PUFFS: u32 = 320u;
// How long a drop burns as it falls, in seconds.
const DROP_S: f32 = 2.0;

// The flipbook's frames, how many across its sheet, and how many it plays a second for a
// flame two and a half metres tall (`tools/textures/flames.ts`): filmed at 29.97 a second, of
// a wood fire whose flames stand about half a metre, slowed for a taller one as a flame's
// time goes as the root of its size (Froude's scaling).
const FLIPBOOK_FRAMES: f32 = 96.0;
const FLIPBOOK_COLUMNS: f32 = 12.0;
const FLIPBOOK_FPS: f32 = 13.4;
// The filmed flame's blue, which the sheet does not keep, from its green (sRGB): it rises
// only as the green nears white; must match BLUE_K and BLUE_P in flames.ts.
const FOOTAGE_BLUE_K: f32 = 0.23;
const FOOTAGE_BLUE_P: f32 = 4.0;

// A frame of the flipbook at a place on its card, x across and y up, kept a hair inside the
// frame so its neighbours never bleed in.
fn flipbook_at(frame: f32, x: f32, y: f32) -> vec4<f32> {
    let column = frame % FLIPBOOK_COLUMNS;
    let row = floor(frame / FLIPBOOK_COLUMNS);
    let rows = FLIPBOOK_FRAMES / FLIPBOOK_COLUMNS;
    let at = vec2((column + clamp(x, 0.01, 0.99)) / FLIPBOOK_COLUMNS, (row + 1.0 - clamp(y, 0.01, 0.99)) / rows);
    return textureSampleLevel(flipbook, flipbook_sampler, at, 0.0);
}

// The filmed fire at a moment `f` (in frames, any real number) at a place on its card: its
// light as filmed (linear, premultiplied by how much it covers), its soot and how much it
// covers, 0 to 1; the two frames either side blended, which at the footage's own rate is
// close to its motion.
struct Filmed {
    light: vec3<f32>,
    soot: f32,
    cover: f32,
}

fn footage_at(f: f32, x: f32, y: f32) -> Filmed {
    let f0 = floor(f);
    let w = f - f0;
    let f1 = (f0 + 1.0) % FLIPBOOK_FRAMES;
    let t = mix(flipbook_at(f0, x, y), flipbook_at(f1, x, y), w);
    // A dim or thin texel's green held under its red by how bright it is, as a dim flame is
    // red: the luma key keeps the footage's dull yellow glow (lit smoke, motion blur) at a
    // fraction of its cover, which over stone and sky read lime and olive (castle film r11's
    // probe, sRGB about 90,95,40 among the tongues).
    // A thin texel is held redder still, its fringe orange-brown and not olive over grass
    // and trees; and a texel whose green nears white is given back the blue the sheet does
    // not keep, so a core clips pale and white as a filmed fire's does.
    let thin = mix(0.55, 1.0, smoothstep(0.15, 0.6, t.a));
    let g = min(t.g, t.r * mix(0.3, 0.96, smoothstep(0.25, 0.85, t.r)) * thin);
    let blue = max(FOOTAGE_BLUE_K * pow(g, FOOTAGE_BLUE_P), 0.75 * g * smoothstep(0.8, 1.0, g));
    let srgb = vec3(t.r, g, blue);
    return Filmed(pow(srgb, vec3(2.2)), t.b, t.a);
}

// A card's corner placed: where on screen, the corner across and up the card, how far
// through its life, how hot, its luck and what it is, and how tall it is, in metres.
struct Out {
    clip: vec4<f32>,
    uv: vec2<f32>,
    life: vec4<f32>,
    size: f32,
    // Where the corner is in the world (a breath's hull's only).
    world: vec3<f32>,
    // Its fire's flicker now, a share of its mean (a blaze's, a hearth's; 0 for one of its own).
    glow: f32,
};

// How bright glowing soot is taken to be, on the luminances `soot` works from.
const LUMINANCE: f32 = 1.0;
// Glowing soot is kept under the tonemapper's knee as it is shown on the screen: the camera's
// exposure and the eye's adaptation together, the adaptation applied after the scene is drawn
// (Bevy's auto exposure, in the tonemapping pass). Kept under the knee of `view.exposure`
// alone, a flame was darkened with the rest of a sunlit frame by the stop the day lets it
// darken (scene.rs, ADAPT_DARKER): its yellow hearts at half brightness, olive (castle films
// r5 and r6, sRGB 162,149,89), and with a stop of headroom guessed for it never white.
const KNEE: f32 = 1.0;
// The white a white-hot core is lifted toward on the screen, far past the tonemapper's peak
// so it clips there as fire does on film (4% of a barn fire's flame pixels are white); how
// bright on the screen soot's green rises to its red (yellow), and how bright its blue then
// rises too (white), late and steep so only the hottest few in a hundred clip: lifted from 1.0
// to 2.0 the bodies of a wall's flames went white with pastel rims, their blue half their red
// where a barn fire's is a fifth (castle film r7, B/R 0.51 against 0.19).
const WHITE: f32 = 4.0;
const YELLOW_FROM: f32 = 0.9;
const YELLOW_TO: f32 = 1.6;
const WHITE_FROM: f32 = 1.5;
const WHITE_TO: f32 = 2.6;
// How far in front of what is behind it a card fades in, in metres, so it never shows a
// straight edge where it goes into a wall or the ground: a flame's, an ember's, a jet's.
// How many kept fires are drawn as hearths (crates/terra_world/src/fire.rs HEARTHS).
const HEARTHS: u32 = 16u;
const SOFT_M: vec3<f32> = vec3<f32>(0.35, 0.1, 0.5);

fn hash(p: vec2<f32>) -> f32 {
    let q = fract(p * vec2<f32>(0.1031, 0.1030));
    let r = q + dot(q, q.yx + 33.33);
    return fract((r.x + r.y) * r.x);
}

fn noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash(i), hash(i + vec2(1.0, 0.0)), u.x), mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), u.x), u.y);
}

fn fbm(p: vec2<f32>) -> f32 {
    return 0.55 * noise(p) + 0.3 * noise(p * 2.1 + 3.7) + 0.15 * noise(p * 4.3 + 9.1);
}

// A gradient for a lattice point, and gradient noise from them, about -0.7 to 0.7: smoother
// than value noise, without its blocky grain.
fn gradient(p: vec2<f32>) -> vec2<f32> {
    let q = vec2(dot(p, vec2(127.1, 311.7)), dot(p, vec2(269.5, 183.3)));
    return -1.0 + 2.0 * fract(sin(q) * 43758.5453);
}

fn gnoise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = dot(gradient(i), f);
    let b = dot(gradient(i + vec2(1.0, 0.0)), f - vec2(1.0, 0.0));
    let c = dot(gradient(i + vec2(0.0, 1.0)), f - vec2(0.0, 1.0));
    let d = dot(gradient(i + vec2(1.0, 1.0)), f - vec2(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}


// Glowing soot at a temperature and in an amount (how much of it there is along the eye's
// ray, 0 to 1), as the camera sees it: its colour from soot's emission at that temperature
// (red, with green rising from 0.04 at 1000 K to 0.21 at 1600 K, never the yellow or white of
// the 3000 K colours a wood flame has not got), its luminance from about 50 nits at 1100 K to
// 1150 at 1400 and 5700 at 1650, a sunlit white's 0.005 to 0.6, times the amount; then kept
// under the tonemapper's knee per channel, as film shoulders it, with a white-hot lift only
// where it is far brighter than the exposure, so a fire by day stays orange with a yellow
// heart and never goes salmon pink. The amount is taken before the shoulder, as a camera
// takes light: a thin or dim flame is dull red, since only red is near the knee, and goes
// yellow only where there is light enough to lift its green there. Shouldered first and then
// thinned, a dim flame kept its hot one's yellow at a fraction of its brightness, which over
// charred timber and smoke is olive (castle film r5: hue 50-60 at sRGB 153,148,92).
fn soot(kelvin: f32, amount: f32) -> vec3<f32> {
    let g = clamp(0.042 + 0.000278 * (kelvin - 1000.0), 0.03, 0.26);
    let slope = select(0.0105, 0.0064, kelvin > 1400.0);
    let nits = exp(7.05 + (kelvin - 1400.0) * slope) * LUMINANCE * amount;
    let unit = 0.2126 + 0.7152 * g + 0.0722 * 0.02;
    let shown = exp2(adapted.x);
    let screen = nits * view.exposure * shown;
    let exposed = vec3(1.0, g, 0.02) * (screen / unit);
    let kept = KNEE * (vec3(1.0) - exp(-exposed / KNEE));
    // White-hot as soot glows hotter: its green rising to its red first (yellow), and only
    // then all three past the peak (white). Lifted straight toward white an orange passes
    // through salmon, white blobs with pink rims (castle film r7's first probe).
    let yellow = smoothstep(YELLOW_FROM, YELLOW_TO, screen);
    let white = pow(smoothstep(WHITE_FROM, WHITE_TO, screen), 3.0);
    let hot = vec3(kept.r, mix(kept.g, max(kept.g, 0.88 * kept.r), yellow), mix(kept.b, max(kept.b, 0.2 * kept.r), yellow));
    return mix(hot, vec3(WHITE), white) / shown;
}

// A flame's or a breath's colour on the screen by how hot it burns there, 0 to 1, as Unreal's
// and most games' fire materials colour a flipbook's heat through a gradient: dull red at its
// thinnest rims, orange, then the saturated yellow that is nearly all of a flamethrower's
// stream on film (98% of an M7's flame pixels are hue 40-60), pale yellow, and white-hot past
// the tonemapper's peak only in the hottest cores. Each step is kept under the knee (Khronos
// neutral's peak of 1) until the white, so it stays saturated: lifted toward white from
// orange, or added over a blue sky and summed past the peak card over card, the neutral
// tonemapper takes an orange straight to salmon pink (castle film r7, the breath 12% yellow).
// On the screen whatever the eye is adapted to, as film exposes for fire by day or night.
// Its blue is a twentieth of its red until the pale, so the yellow is a fire's and not a
// khaki's (a yellow with a fifth of its red in blue, linear, read olive over sky and smoke in
// r8's second probe, sRGB B/R 0.45); and it is shown a little past the knee, since the frame
// showed it about 0.6 as bright as asked for (sRGB 195,147,89 for a mean asked as 241,192,101).
const FLAME_GAIN: f32 = 1.7;
// Its stops are spread as real fire's colours are measured (research/2026-10-18-dragon-breath.md,
// §4: a burning building's flame pixels 53% deep red-orange, hue under 25; 29% orange; 9%
// yellow; 1% white): for a flame whose heat is spread evenly, the lower half of it is deep
// red-orange, the next third orange, and only its top fifth yellow and pale. Spread as r8's
// were, a wall's flames were 63% orange and 16% yellow, a hedge of bright tongues.
fn flame_colour(h: f32) -> vec3<f32> {
    let rim = vec3(0.4, 0.035, 0.003);
    let deep = vec3(0.95, 0.13, 0.008);
    let orange = vec3(1.0, 0.33, 0.02);
    let yellow = vec3(1.0, 0.7, 0.06);
    let pale = vec3(1.05, 0.9, 0.3);
    let white = vec3(0.95, 0.92, 0.82) * WHITE;
    var c = mix(rim, deep, smoothstep(0.0, 0.4, h));
    c = mix(c, orange, smoothstep(0.4, 0.75, h));
    c = mix(c, yellow, smoothstep(0.75, 0.88, h));
    c = mix(c, pale, smoothstep(0.88, 0.95, h));
    return mix(c * FLAME_GAIN, white, smoothstep(0.95, 1.0, h));
}

// How much of what is behind it a flame covers, for what it gives and its alpha. Its light is
// already shouldered, so its red is near one wherever it is hot and only its green grows: a
// hot yellow over a blue sky at half cover summed to more green than red, a pale olive. So
// the yellower it glows the more it covers, keeping red over green over blue as a camera
// sees fire; a dull red flame stays thin, as it is.
fn covers(light: vec3<f32>, alpha: f32) -> f32 {
    let yellow = light.g / max(light.r, 1.0e-4);
    return alpha * mix(0.4, 0.97, smoothstep(0.2, 0.75, yellow));
}

// The black of soot as the screen shows it, a little warm.
const SOOT_BLACK: vec3<f32> = vec3<f32>(0.012, 0.01, 0.009);
// A burning building's flame from the filmed fire: its light as filmed, brought to the screen
// by FOOTAGE_GAIN whatever the eye is adapted to, as film exposes for fire; its green taken
// down a little by FOOTAGE_GREEN for a cooler, sootier blaze than the filmed one (real building
// fires by day are about half deep red-orange, research/2026-10-18-dragon-breath.md §4); and
// its soot, the black wisps off its tongues' tips, laid over what is behind where there is no
// flame.
// The footage's hot cores, its yellow-white where its green nears its red, keep all their
// green and are lifted past the tonemapper's knee to clip there as a barn fire's do: taken
// down with the rest, every wall was 0-2% yellow against a building fire's 9%, its brightest
// sRGB 211-221 against 235-252 (castle film r11).
fn wall_flame(filmed: Filmed, heat: f32, soft: f32) -> vec4<f32> {
    let hot = smoothstep(0.3, 0.7, filmed.light.g);
    let green = mix(mix(FOOTAGE_GREEN * 0.8, FOOTAGE_GREEN, heat), 1.0, hot);
    let gain = FOOTAGE_GAIN * mix(0.6, 1.0, heat) * mix(1.0, FOOTAGE_CORE_GAIN, hot);
    let light = filmed.light * vec3(1.0, green, green) * gain * soft;
    let flame = filmed.cover * soft;
    let black = saturate(filmed.soot * WALL_SOOT) * (1.0 - flame) * soft;
    let cover = flame + black * WALL_SOOT_COVER;
    return vec4((light + SOOT_BLACK * black) * on_screen(), cover);
}
const FOOTAGE_GAIN: f32 = 1.25;
// At 0.85 a wall's flame pixels were 71-73% deep red against a building fire's 53 (r12's
// first probe).
const FOOTAGE_GREEN: f32 = 0.95;
const FOOTAGE_CORE_GAIN: f32 = 1.6;
const WALL_SOOT: f32 = 1.2;
const WALL_SOOT_COVER: f32 = 0.8;
// A breath is drawn as a volume, not as cards (research/2026-10-18-dragon-breath.md §6): a
// box round its stream and fireball, marched through along each ray from the eye, as
// Unreal's Niagara fluids and EmberGen's volumes are drawn in real time. The gas in it is a
// field of 3D noise carried along the stream from the mouth, so each parcel keeps its
// pattern as it flies and swells; its temperature takes it through the fire's gradient
// (white-yellow on the stream's line at the jaw, yellow through its core, orange round
// that, deep red at its rims and as it cools), its density sets how much it covers, and
// where it has cooled it is soot. Its tip tears apart, and where it strikes it rolls up
// into a fireball capped with black soot. Shaded front to back, each step's colour covering
// what is behind it by how dense it is, so it never sums past the tonemapper's peak.

// How fast the breath flies from the jaw, metres a second: slower than the 34 m/s a
// flamethrower's fuel leaves at, so the stream reads as a rope, not a flash.
const JET_SPEED: f32 = 24.0;
// How much a metre of the densest flame covers (extinction, per metre), from its cooler rims
// to its yellow core, and of soot: the hot core opaque, so it is what the eye sees through
// the thinner orange round it, and the soot as black as film shows it. At 5 a metre the
// whole breath let the sky through as salmon pink (castle film r11).
const BREATH_EXTINCTION: f32 = 18.0;
const BREATH_RIM_EXTINCTION: f32 = 5.0;
const SOOT_EXTINCTION: f32 = 6.0;
// The fireball is opaque at its face, so the eye sees its outer gas and not the soot inside
// it: as thin as the stream's rims, a ray ended early on the tower inside it showed a brighter
// front with the tower's straight edges, in a ball brown with the soot behind (castle film r11).
const BALL_EXTINCTION: f32 = 26.0;
// The most steps a ray takes through a breath.
const BREATH_STEPS: i32 = 128;

// A hash of a lattice point, 0 to 1 (PCG, after Jarzynski and Olano 2020).
fn hash3(p: vec3<i32>) -> f32 {
    var h = (u32(p.x) * 73856093u) ^ (u32(p.y) * 19349663u) ^ (u32(p.z) * 83492791u);
    h = h * 747796405u + 2891336453u;
    h = ((h >> ((h >> 28u) + 4u)) ^ h) * 277803737u;
    h = (h >> 22u) ^ h;
    return f32(h) * (1.0 / 4294967295.0);
}

// Value noise in three dimensions, 0 to 1.
fn noise3(p: vec3<f32>) -> f32 {
    let i = vec3<i32>(floor(p));
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = mix(hash3(i), hash3(i + vec3(1, 0, 0)), u.x);
    let b = mix(hash3(i + vec3(0, 1, 0)), hash3(i + vec3(1, 1, 0)), u.x);
    let c = mix(hash3(i + vec3(0, 0, 1)), hash3(i + vec3(1, 0, 1)), u.x);
    let d = mix(hash3(i + vec3(0, 1, 1)), hash3(i + vec3(1, 1, 1)), u.x);
    return mix(mix(a, b, u.y), mix(c, d, u.y), u.z);
}

// Three octaves of it, 0 to 1.
fn fbm3(p: vec3<f32>) -> f32 {
    let a = noise3(p);
    let b = noise3(p * 2.03 + vec3(17.1, 3.7, 9.2));
    let c = noise3(p * 4.07 + vec3(5.3, 29.4, 1.9));
    return (a * 0.5714 + b * 0.2857 + c * 0.1429);
}

// A jet as the shader marches it: its mouth and frame (across, across, along), the world's
// up in that frame, how far it reaches and spreads, and where its gas is: the front of what
// has been breathed (once on), the back of it (once off), how grown its fireball is and how
// much of it is left.
struct Breath {
    mouth: vec3<f32>,
    lift: vec3<f32>,
    side: vec3<f32>,
    way: vec3<f32>,
    up: vec3<f32>,
    reach: f32,
    spread: f32,
    front: f32,
    back: f32,
    ball: f32,
    left: f32,
}

fn breath_of(slot: u32) -> Breath {
    let j = slot * 3u;
    var b: Breath;
    b.mouth = fire.jets[j].xyz;
    b.reach = fire.jets[j].w;
    b.way = normalize(fire.jets[j + 1u].xyz);
    b.spread = fire.jets[j + 1u].w;
    let on = fire.jets[j + 2u].x > 0.5;
    let since = fire.jets[j + 2u].y;
    let time = fire.kind.z;
    let up = normalize(b.mouth);
    var side = cross(b.way, up);
    if dot(side, side) < 1.0e-4 {
        side = vec3(1.0, 0.0, 0.0);
    }
    b.side = normalize(side);
    b.lift = normalize(cross(b.side, b.way));
    b.up = vec3(dot(up, b.lift), dot(up, b.side), dot(up, b.way));
    let flown = JET_SPEED * max(time - since, 0.0);
    // Once on, the gas reaches as far as it has flown; once off, the last of it flies on
    // from the jaw and burns out in a moment rather than flying on as a cloud cut loose.
    b.front = select(1.0e5, flown, on);
    b.back = select(-1.0, flown, !on);
    b.left = select(saturate(1.0 - (time - since) / 0.45), 1.0, on);
    // The fireball where it strikes rolls up as the stream's front reaches it.
    b.ball = select(1.0, smoothstep(b.reach * 0.85, b.reach * 1.25, flown), on) * b.left;
    return b;
}

// The stream's radius so far along it, in metres: a tight rope at the jaw, opening into a
// cone that widens as it slows.
fn stream_radius(z: f32, b: Breath) -> f32 {
    return 0.22 + b.spread * z * (0.7 + 0.5 * saturate(z / b.reach));
}

// The fireball's radius, 2 to 3 times the stream's width where it strikes.
fn ball_radius(b: Breath) -> f32 {
    return 0.6 + 1.4 * stream_radius(b.reach, b);
}

// Its middle stands most of its radius back from where the stream strikes, rolled up off it:
// centred near the wall, half of it was behind the stone, and rays ended on the wall after
// a sliver of its thin edge, so the windows showed through it (castle film r12).
fn ball_middle(b: Breath, rb: f32) -> vec3<f32> {
    return vec3(0.0, 0.0, b.reach - 0.85 * rb) + b.up * rb * 0.45;
}

// The gas at a point in the jet's frame: how dense (0 to about 1), how hot (0 to 1, the
// gradient's), how much of it is soot (0 to 1); and how far the point is outside any gas,
// at least, so a ray can step over empty air.
fn breath_gas(l: vec3<f32>, b: Breath, time: f32) -> vec4<f32> {
    let rb = ball_radius(b) * (0.45 + 0.55 * b.ball);
    let mid = ball_middle(b, rb);
    let rel = l - mid;
    let zc = clamp(l.z, 0.0, b.reach);
    let r = stream_radius(zc, b);
    let rho = length(l.xy);
    let slope = 1.0 + b.spread * 1.7;
    let empty = min(
        max((rho - 2.0 * r) / slope, max(-l.z - 0.3, l.z - b.reach - 0.5)),
        length(rel) - rb * 1.55,
    );
    if empty > 0.0 {
        return vec4(0.0, 0.0, 0.0, empty);
    }
    let zf = saturate(l.z / b.reach);
    // How much of the stream has been breathed since this parcel was: the same on a parcel
    // as it flies, so its noise flies with it; the gas boils a little as it goes (zf).
    let flow = JET_SPEED * time - l.z;
    let open = smoothstep(0.0, 0.35, zf);
    let wander = vec2(noise3(vec3(flow * 0.11, 1.7, zf * 2.0)), noise3(vec3(flow * 0.11, 7.3, zf * 2.0))) - 0.5;
    let q = (l.xy - wander * r * 0.8 * open) / r;
    let m = vec3(q * 2.6, flow * 0.6 + zf * 2.4);
    let e = fbm3(m + 0.6 * vec3(noise3(m * 0.5 + 4.1), noise3(m * 0.5 + 9.7), 0.0));
    // Solid to near its edge, the edge sharp and torn by the noise, as a fuel jet's on film
    // is a rope of nearly even light whose edge meets the sky (refs/fire-real MEASURED.md
    // §4); a soft edge let the sky through its whole width as salmon pink (castle film r11).
    let u = length(q) + (e - 0.5) * mix(0.7, 1.3, open);
    var stream = 1.0 - smoothstep(0.78, 1.0, u);
    // Its tip tears apart into flying blobs that burn out.
    let tear = smoothstep(0.55, 1.0, zf) * 0.75;
    let blobs = noise3(vec3(q * 0.8 + 3.1, flow * 0.16 + 11.0));
    stream *= smoothstep(tear - 0.12, tear + 0.12, blobs + 0.12 * (1.0 - zf));
    stream *= smoothstep(-0.25, 0.25, l.z) * (1.0 - smoothstep(b.front - 1.5, b.front + 0.5, l.z)) * smoothstep(b.back - 0.5, b.back + 1.5, l.z);
    // Hottest on its line, white-yellow at the jaw and yellow through the first half of its
    // core, orange round that and deep red-orange at its rims, cooling along it to orange at
    // the tip: a burning fuel jet is about a quarter yellow (MEASURED.md §2).
    // Even through the body and falling off only in its outer tenth, since the gas is opaque
    // and what the eye sees is its outer layer: graded from the axis out the core was hidden
    // under orange (castle film r12's first probe, yellow 0-7%), and from 0.93 at the jaw to
    // 0.68 at the tip, cooling from u 0.6, the stream was a khaki cone (r11, yellow 0%).
    // Streaked along its flow and boiling at its edge, so it is a rope of burning fuel with
    // yellow and orange strands, not a smooth glassy cone (held at 0.12, r12's first probe).
    let core = mix(0.95, 0.8, smoothstep(0.2, 1.0, zf)) + 0.05 * (1.0 - smoothstep(0.0, 0.2, zf));
    let streak = noise3(vec3(q * 1.7 + 5.3, flow * 0.35));
    let hot_s = core * (1.0 - 0.3 * smoothstep(0.85, 1.05, u)) + 0.3 * (e - 0.5) + 0.16 * (streak - 0.5);
    // The fireball, billowing up and rolling as it burns: overlapping balls with yellow and
    // orange faces under a black cap of soot (the M2's end, MEASURED.md §11), its surface
    // broken by warped noise into lumps, not one smooth ball.
    var ball = 0.0;
    var hot_b = 0.0;
    if b.ball > 0.01 && length(rel) < rb * 1.55 {
        let p = rel / rb - b.up * time * 0.6 + vec3(0.0, 0.0, time * 0.25);
        let lumps = fbm3(p * 1.6);
        let warp = vec3(noise3(p * 2.5 + 3.3), noise3(p * 2.5 + 7.9), noise3(p * 2.5 + 1.4)) - 0.5;
        let nb = fbm3(p * 5.0 + warp * 2.2);
        let d = length(rel) / rb;
        ball = (1.0 - smoothstep(0.7, 0.95, d + (lumps - 0.5) * 1.1 + (nb - 0.5) * 0.35)) * b.ball;
        let high = dot(rel, b.up) / rb;
        // Bright faces and dark red folds between the lumps, as film shows a fuel fireball:
        // with half this spread its face was one flat orange (castle film r12).
        // A lump stands out where `lumps` and `nb` are low, so that is where it is hottest:
        // signed the other way, its bulges were its coolest gas and the surface the eye sees
        // was brown (castle film r11, yellow 0%). Its top cools to the cap of soot.
        // At 0.9 its faces were cream and white with no cap (r12's first probe).
        hot_b = 0.82 - 0.1 * d - 1.4 * (lumps - 0.5) - 0.9 * (nb - 0.5) - 0.9 * smoothstep(0.1, 0.85, high);
        // The stream goes into the ball and is lost in it: kept where the ball was half
        // dense, its hotter gas lifted the ball's colour inside its cone, a pale wedge with
        // straight edges (castle film r12).
        stream *= 1.0 - smoothstep(0.02, 0.3, ball);
    }
    let dense = stream + ball;
    if dense < 1.0e-3 {
        return vec4(0.0, 0.0, 0.0, 0.0);
    }
    let hot = (stream * hot_s + ball * hot_b) / dense;
    // Soot where it has cooled: at the stream's torn edges once it has rolled open and at
    // its tip, all over the fireball's cooled cap; a fuel jet near the jaw is a clean rope.
    let sooty = mix(smoothstep(0.3, 0.8, zf) * smoothstep(0.75, 1.0, u + 0.4 * zf), 1.0, ball / dense);
    let soot_part = (1.0 - smoothstep(0.3, 0.45, hot)) * sooty;
    // Its last, negative so a march still reads only a positive one as empty air: how much
    // of the gas is the fireball's.
    return vec4(min(dense, 1.0) * b.left, hot, soot_part, -ball / dense);
}

// The breath's gas as the screen shows it by how hot it is: the fire's gradient without the
// gain a card's flame is given for the sky it lets through, since the gas is opaque: lifted
// 1.7 times past the tonemapper's knee, its orange came out desaturated, a flat salmon
// (sRGB 230,142,82, castle film r12's first probe), and its yellow no yellower than that.
// Its yellows and pales are lifted 1.5 past the knee, where the tonemapper keeps a yellow
// yellow and takes a pale toward white, and the white-hot jaw past its peak: held at the knee,
// the stream's core was a dim khaki, 0% yellow against a flamethrower's 25% (castle film r11).
fn breath_colour(h: f32) -> vec3<f32> {
    let c = flame_colour(h);
    let lift = mix(1.0, 1.5, smoothstep(0.72, 0.88, h)) / FLAME_GAIN;
    return mix(c * lift, c, smoothstep(0.95, 1.0, h));
}

// Its soot: black where it has cooled, brown-grey and faintly lit where it still wraps
// hot gas, so a cap billows rather than standing as one flat black shape.
fn breath_soot(h: f32) -> vec3<f32> {
    return mix(SOOT_BLACK, vec3(0.12, 0.07, 0.04), smoothstep(0.0, 0.45, h));
}

// The breath along the ray through a fragment of its hull's far side: its light, already
// exposed, and how much of what is behind it it covers.
@if(!PREPASS_PIPELINE)
fn breath(in: VertexOutput) -> vec4<f32> {
    let b = breath_of(u32(in.uv.x + 0.5));
    if b.reach <= 0.0 || b.left <= 0.0 {
        discard;
    }
    let time = fire.kind.z;
    let eye = view.world_position;
    let far_side = in.world_position.xyz;
    let dir = normalize(far_side - eye);
    var far = distance(eye, far_side);
@if(DEPTH_PREPASS)
    {
        // What stands in front of the hull's far side ends the ray there.
        let here = depth_ndc_to_view_z(in.position.z);
        let scene = depth_ndc_to_view_z(prepass_depth(in.position, 0u));
        far = min(far, far * scene / min(here, -1.0e-4));
    }
    let o = eye - b.mouth;
    let ol = vec3(dot(o, b.lift), dot(o, b.side), dot(o, b.way));
    let dl = vec3(dot(dir, b.lift), dot(dir, b.side), dot(dir, b.way));
    // Where the ray comes into the hull (zero if the eye is in it).
    let box = hull_box(b);
    let inv = 1.0 / select(dl, vec3(1.0e-6), abs(dl) < vec3(1.0e-6));
    let t0 = (box[0] - ol) * inv;
    let t1 = (box[1] - ol) * inv;
    let near = max(max(max(min(t0.x, t1.x), min(t0.y, t1.y)), min(t0.z, t1.z)), 0.0);
    if near >= far {
        discard;
    }
    // A step is a fraction of the stream's radius across it, longer along it, where it
    // changes slowly; started at a different place each pixel and frame, which TAA averages.
    let across = max(length(dl.xy), 0.25);
    let jitter = fract(52.9829189 * fract(dot(in.position.xy, vec2(0.06711056, 0.00583715)) + time * 7.13));
    var s = near;
    var first = true;
    var light = vec3(0.0);
    var through = 1.0;
    for (var i = 0; i < BREATH_STEPS; i++) {
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
        var ds = clamp(0.3 * r / across, 0.05, 0.7);
        if first {
            // The first step into gas is jittered, so the steps never band.
            first = false;
            s += ds * jitter;
            continue;
        }
        ds = min(ds, far - s);
        if gas.x > 0.0 {
            let soot_part = gas.z;
            let stream = mix(BREATH_RIM_EXTINCTION, BREATH_EXTINCTION, smoothstep(0.55, 0.8, gas.y));
            let flame = mix(stream, BALL_EXTINCTION, -gas.w);
            let sigma = gas.x * mix(flame, SOOT_EXTINCTION, soot_part);
            let a = 1.0 - exp(-sigma * ds);
            let c = mix(breath_colour(saturate(gas.y)), breath_soot(gas.y), soot_part);
            light += through * a * c;
            through *= 1.0 - a;
        }
        s += ds;
    }
    let cover = 1.0 - through;
    if cover < 0.002 {
        discard;
    }
    return vec4(light * on_screen(), cover);
}

// The hull round a breath, in its frame: across both ways and along, its least and most.
fn hull_box(b: Breath) -> array<vec3<f32>, 2> {
    let rb = ball_radius(b);
    // The ball reaches 1.55 of its radius from a middle up to 0.45 of it off the stream's line.
    let wide = max(2.0 * stream_radius(b.reach, b), rb * 2.1) + 0.5;
    return array<vec3<f32>, 2>(vec3(-wide, -wide, -0.5), vec3(wide, wide, b.reach + rb * 1.6));
}

// A corner of a breath's hull placed round its jet.
fn hull(corner: vec3<f32>, id: vec2<f32>) -> Out {
    let b = breath_of(u32(id.x + 0.5));
    if b.reach <= 0.0 || b.left <= 0.0 {
        return nowhere();
    }
    let box = hull_box(b);
    let l = mix(box[0], box[1], corner);
    let world = b.mouth + b.lift * l.x + b.side * l.y + b.way * l.z;
    var out: Out;
    out.clip = position_world_to_clip(world);
    out.uv = vec2(id.x, 0.0);
    out.life = vec4(0.0);
    out.size = 0.0;
    out.world = world;
    return out;
}

fn nowhere() -> Out {
    var out: Out;
    out.clip = vec4(0.0, 0.0, -1.0, 1.0);
    out.uv = vec2(0.0);
    out.life = vec4(0.0);
    out.size = 0.0;
    out.world = vec3(0.0);
    return out;
}

// A card at a point, `w` wide and `h` tall, its foot or middle there, upright about `up`
// or turned wholly to the eye.
fn card(at: vec3<f32>, up: vec3<f32>, corner: vec2<f32>, w: f32, h: f32, upright: bool, spin: f32) -> vec3<f32> {
    let to_eye = normalize(view.world_position - at);
    var right = cross(up, to_eye);
    if dot(right, right) < 1.0e-6 {
        right = cross(vec3(1.0, 0.0, 0.0), to_eye);
    }
    right = normalize(right);
    // Upright cards lean back to face an eye looking down on them, as they would show their
    // edges otherwise: from over a burning roof, a wall of crossed edges.
    let facing_eye = cross(to_eye, right);
    var card_up = normalize(mix(up, facing_eye, smoothstep(0.3, 0.75, abs(dot(to_eye, up)))));
    if !upright {
        card_up = facing_eye;
    }
    let c = vec2(corner.x - 0.5, select(corner.y - 0.5, corner.y, upright));
    let turned = vec2(c.x * cos(spin) - c.y * sin(spin), c.x * sin(spin) + c.y * cos(spin));
    return at + right * turned.x * w + card_up * turned.y * h;
}

// A hearth's flames (crates/terra_world/src/fire.rs HEARTHS), as a campfire burns in
// footage and in The Witcher 3, Red Dead Redemption 2 and Kingdom Come: its first card the
// bed of coals glowing among its logs, low and wide; then a tongue over its middle as tall as
// it was made, and four shorter ones round it over the logs, each of its own height and its
// own moment of the flipbook, leaning and licking as they will.
fn hearth(corner: vec2<f32>, foot: vec3<f32>, up: vec3<f32>, tall: f32, heat: f32, wide_in: f32, seed: f32, phase: f32, wind: vec3<f32>, glow: f32) -> Out {
    var out: Out;
    let wide = select(clamp(tall * 0.5, 0.4, 1.6), wide_in, wide_in > 0.0);
    let j = i32(phase * 6.0 + 0.5);
    let luck = fract(seed * 0.618 + f32(j) * 0.379);
    let east = normalize(cross(up, vec3(0.0, 0.0, 1.0)) + vec3(1.0e-4));
    let north = cross(east, up);
    if j == 0 {
        let w = wide * 1.4;
        let h = max(0.3, tall * 0.32);
        let world = card(foot - up * h * 0.25, up, corner, w, h, true, 0.0);
        out.clip = position_world_to_clip(world);
        out.uv = corner;
        out.life = vec4(1.5, heat, luck, 3.0);
        out.size = h;
        return out;
    }
    // Each tongue rises and falls with the fire's flicker, some more than others.
    let breathe = 1.0 + (glow - 1.0) * (0.8 + 0.5 * luck);
    // A fire is made as tall as its flames reach over its logs, which stand about four
    // fifths as tall: each tongue must clear them, or the logs hide it (lake film r5).
    var flame = tall * breathe * 1.1;
    var at = foot;
    var w = wide * 2.4;
    if j > 1 {
        // Round the ring over the logs, each lower than the middle one, licking up their
        // outsides.
        let a = f32(j) * 1.571 + seed * 6.283 + (luck - 0.5) * 0.8;
        at = foot + (east * cos(a) + north * sin(a)) * wide * (0.28 + 0.15 * luck);
        flame = tall * breathe * (0.72 + 0.3 * luck);
        w = wide * (1.25 + 0.4 * luck);
    }
    let h = flame * 1.6;
    let lean = wind * 0.05 * flame;
    let world = card(at - up * h * 0.06 + lean * 0.2, up, corner, w, h, true, 0.0);
    out.clip = position_world_to_clip(world);
    out.uv = corner;
    // Its first number past 1, so it is faded only very near what is behind it: a hearth's
    // flames stand among its logs, and faded over a third of a metre they were a thin
    // tongue over bare sticks (lake film r4).
    out.life = vec4(1.5, heat, luck, 1.0);
    out.size = h;
    return out;
}

fn place(corner: vec2<f32>, id: vec2<f32>) -> Out {
    let kind = i32(fire.kind.x + 0.5);
    let time = fire.kind.z;
    let slot = u32(id.x + 0.5);
    let phase = id.y;
    let wind = fire.wind.xyz;
    var out: Out;
    if kind == 4 {
        // A burning drop of the breath's fuel, falling from where it left the stream: going
        // on as the stream went, slowed by the air, and falling, stretched along its flight
        // as a streak (research/2026-10-18-dragon-breath.md).
        let i = (SOOT_PUFFS + slot) * 2u;
        let born = puffs[i];
        let going = puffs[i + 1u];
        let s = time - born.w;
        if s < 0.0 || s > DROP_S {
            return nowhere();
        }
        let up = normalize(born.xyz);
        let drag = (1.0 - exp(-s * 1.2)) / 1.2;
        let at = born.xyz + going.xyz * drag - up * 4.9 * s * s;
        let flight = going.xyz * exp(-s * 1.2) - up * 9.8 * s;
        let to_eye = normalize(view.world_position - at);
        var along = flight - to_eye * dot(flight, to_eye);
        if dot(along, along) < 1.0e-6 {
            along = up;
        }
        // Long enough to read as a falling streak forty metres off, as drops of burning fuel
        // do in footage, smeared by the camera's shutter.
        let streak = clamp(length(along) * 0.09, 0.6, 3.2);
        along = normalize(along);
        let across = normalize(cross(along, to_eye));
        let size = 0.28 + 0.2 * going.w;
        let world = at + across * (corner.x - 0.5) * size + along * (corner.y - 0.8) * streak;
        out.clip = position_world_to_clip(world);
        out.uv = corner;
        out.life = vec4(s / DROP_S, 1.0, going.w, 5.0);
        out.size = size;
        return out;
    }
    let count = u32(fire.kind.y + 0.5);
    if slot >= count {
        return nowhere();
    }
    let a = slots[slot * 2u];
    let b = slots[slot * 2u + 1u];
    let foot = a.xyz;
    let tall = a.w;
    let heat = b.x;
    let seed = b.y;
    let broad = b.z;
    // Its flicker now (crates/terra_world/src/fire/light.rs `glow`), which its light burns by
    // too: its flames are as much brighter and taller, its sparks and coals brighter still.
    let glow = max(b.w, 0.0);
    if tall <= 0.0 {
        return nowhere();
    }
    let up = normalize(foot);
    let luck = fract(seed * 0.618 + phase * 3.71);
    if kind == 3 {
        // Heat haze: a card over the flame, from half its height to twice it, through which
        // what is behind shimmers.
        let w = tall * 1.5;
        let h = tall * 1.6;
        let world = card(foot + up * tall * 0.45, up, corner, w, h, true, 0.0);
        out.clip = position_world_to_clip(world);
        out.uv = corner;
        out.life = vec4(0.0, heat, luck, 4.0);
        out.size = h;
        return out;
    }
    // A kept fire's flames are a hearth's (`hearth`), drawn by cards of their own.
    let kept_slot = select(0.0, 1.0, slot < u32(fire.kind.w + 0.5));
    let is_hearth = slot < min(u32(fire.kind.w + 0.5), HEARTHS);
    if kind == 0 && (phase >= 1.0) != is_hearth {
        return nowhere();
    }
    if kind == 0 && is_hearth {
        var tongue = hearth(corner, foot, up, tall, heat, b.z, seed, phase - 1.0, wind, glow);
        tongue.glow = glow;
        return tongue;
    }
    // A hearth's sparks are cards of their own too, more of them than a burning roof's.
    if kind == 1 && (phase >= 1.0) != is_hearth {
        return nowhere();
    }
    if kind == 0 {
        // A burning surface's flame (research/2026-10-16-real-time-fire-rendering.md): a few
        // cards of a simulated fire's flipbook standing where it burns, a big one over the
        // blaze and two smaller either side of it, each its own fire and its own moment. The
        // flipbook's flame fills about the lower two thirds of its card, so the cards stand
        // half again as tall as the flame. Its height breathes at the puffing rate of a fire
        // that wide, 1.5 / sqrt(D) Hz.
        // Blazes stand on a two-metre grid: each is put off its cell's middle by up to most
        // of a metre and made taller or shorter by its own seed, so a burning roof is a fire
        // and not rows of the same flame.
        let own = vec3(fract(seed * 12.9898), fract(seed * 78.233), fract(seed * 37.719));
        // A broad surface burning as one blaze is a sheet of fire its own width.
        let wide = select(clamp(tall * 0.8, 0.8, 2.8), broad, broad > 0.0);
        // Its height breathes with its flicker, at the puffing rate of a fire that wide.
        let breathe = 1.0 + (glow - 1.0);
        // A kept fire is as tall as it was made, its flames sized to clear its own logs; a
        // grid's blazes each a little taller or shorter.
        let flame = tall * breathe * mix(0.5 + 1.2 * own.z * own.z, 1.0, kept_slot);
        let east = normalize(cross(up, vec3(0.0, 0.0, 1.0)) + vec3(1.0e-4));
        let north = cross(east, up);
        let lean = wind * 0.05 * flame;
        var w = wide * 1.9;
        var h = flame * 1.75;
        // A kept fire (a campfire) stands just where it is, not off a grid cell's middle.
        var at = foot + (east * (own.x - 0.5) + north * (own.y - 0.5)) * max(1.5, wide * 0.45) * (1.0 - kept_slot);
        if kept_slot < 0.5 {
            // A grid's blaze is one filmed fire, a bonfire's width, and one in three is
            // only its embers and smoke: each frame of the footage is a whole fire of many
            // tongues, so three to a blaze two metres apart, each wider than the cell, were
            // a hedge of overlapping flames (castle film r11), where a burning building is
            // separate fires with its timbers dark between them.
            if phase > 0.01 || fract(seed * 53.17) < 0.33 {
                return nowhere();
            }
            w = wide * 1.35;
        }
        if phase > 0.01 {
            let side = select(-1.0, 1.0, phase > 0.5);
            w = wide * 1.2;
            h = flame * 1.3;
            at = foot + east * side * wide * (0.25 + 0.2 * luck) + north * (fract(luck * 3.1) - 0.5) * wide * 0.5;
        }
        // Each card its own width, height and lean by its luck, so a burning wall's flames
        // are not one cut-out stamped along it (castle film r7).
        w *= 0.8 + 0.45 * fract(luck * 4.3);
        h *= 0.7 + 0.7 * pow(fract(luck * 6.1), 1.5);
        let tilt = (fract(luck * 9.7) - 0.5) * 0.22;
        let world = card(at - up * h * 0.04 + lean * 0.2, up, corner, w, h, true, tilt);
        out.clip = position_world_to_clip(world);
        out.uv = corner;
        out.life = vec4(0.5, heat, luck, 1.0);
        out.size = h;
        out.glow = glow;
        return out;
    }
    // An ember: only some blazes throw them, the hotter the more; a hearth a few always,
    // sparks rising from its coals a metre or two and winking out, not flung over the trees.
    let kept = select(0.0, 1.0, is_hearth);
    if luck > heat * 0.9 && !is_hearth {
        return nowhere();
    }
    let life_s = mix(1.6 + 2.2 * luck, 0.9 + 1.3 * luck, kept);
    let t = fract(time / life_s + fract(phase) * 1.37 + seed);
    let seconds = t * life_s;
    let rise = mix(2.5 + 3.5 * luck, 1.4 + 1.6 * luck, kept) * (1.0 - exp(-seconds * 0.9)) / 0.9;
    let east = normalize(cross(up, vec3(0.0, 0.0, 1.0)) + vec3(1.0e-4));
    let north = cross(east, up);
    let wander = vec2(noise(vec2(luck * 41.0, seconds * 0.8)), noise(vec2(luck * 73.0, seconds * 0.8))) - 0.5;
    let out_from = (vec2(hash(vec2(luck, 1.0)), hash(vec2(luck, 2.0))) - 0.5) * mix(tall * 0.8, b.z * 0.5, kept);
    let sway = mix(2.0, 0.6, kept);
    let at = foot + up * (rise + tall * mix(0.3, 0.12, kept)) + east * (out_from.x + wander.x * sway * t) + north * (out_from.y + wander.y * sway * t) + wind * seconds * mix(0.8, 0.4, kept);
    // Stretched along its flight: up and downwind.
    let flight = normalize(up * (3.0 * exp(-seconds * 0.9) + 0.4) + wind * 0.8);
    let to_eye = normalize(view.world_position - at);
    var across = cross(flight, to_eye);
    if dot(across, across) < 1.0e-6 {
        across = cross(up, to_eye);
    }
    across = normalize(across);
    // A hearth's a little bigger than a roof's: at 0.02 to 0.04 m none showed at 1080p from
    // six metres, the film's distance (lake film r5).
    let size = mix(0.035 + 0.03 * luck, 0.035 + 0.025 * luck, kept);
    // A hearth's short streaks, a roof's long: drawn out five times its sparks were white
    // needles beside the flames (lake film r6's first still).
    let world = at + across * (corner.x - 0.5) * size + flight * (corner.y - 0.5) * size * mix(5.0, 2.5, kept);
    out.clip = position_world_to_clip(world);
    out.uv = corner;
    out.life = vec4(t, heat, luck, 2.0);
    out.glow = glow;
    return out;
}

// `soft` is how far it has faded in front of what is behind it: taken as less soot glowing,
// so a flame meeting the roof it stands on goes dull red there rather than a dim yellow.
fn shade(uv_in: vec2<f32>, life: vec4<f32>, size: f32, soft: f32, glow: f32) -> vec4<f32> {
    let kind = i32(fire.kind.x + 0.5);
    let time = fire.kind.z;
    let t = life.x;
    let heat = life.y;
    let luck = life.z;
    let uv = uv_in;
    // What it gives, already exposed, and how much of what is behind it it covers.
    var light = vec3(0.0);
    var cover = 0.0;
    if kind == 0 && life.w > 2.5 {
        // A hearth's bed of coals: a low glow among its logs, broken by noise into coals
        // brighter and darker, each brightening and dimming slowly as air reaches it, deep
        // red at the bed's edges and orange in its middle, under the flames it feeds.
        let p = (uv - vec2(0.5, 0.45)) * vec2(2.0, 2.2);
        let bed = 1.0 - smoothstep(0.35, 1.0, length(p));
        let coals = fbm(uv * vec2(9.0, 5.0) + vec2(luck * 17.0, 0.0));
        // Brighter as the fire flares, by twice its flicker, the brightest coals the most.
        let breath = (0.8 + 0.2 * coals) * max(1.0 + 2.0 * (glow - 1.0), 0.2);
        let glow = bed * smoothstep(0.3, 0.75, coals) * breath;
        let alpha = smoothstep(0.02, 0.35, glow) * soft;
        // Hot enough to read orange by day: at 1000 to 1500 K it was a dull red lost under
        // the logs (lake film r5).
        let kelvin = 1150.0 + 420.0 * glow * (0.6 + 0.4 * heat) + 120.0 * bed;
        light = soot(kelvin, alpha);
        cover = covers(light, alpha);
    } else if kind == 0 {
        // The filmed fire's frame from the flipbook (`tools/textures/flames.ts`) at its own
        // moment by the card's luck, mirrored for half of them, played at the rate a fire this
        // tall moves (a flame's time goes as the root of its size, Froude's scaling): its light
        // as filmed, see-through where it was dim, and its soot drawn dark over what is behind
        // where there is no flame, the black wisps that break off a burning building's tongues.
        let x = select(uv.x, 1.0 - uv.x, fract(luck * 7.3) > 0.5);
        let rate = FLIPBOOK_FPS * sqrt(2.5 / max(size, 0.5)) * (0.8 + 0.4 * fract(luck * 17.3));
        let f = fract(time * rate / FLIPBOOK_FRAMES + luck * 5.17) * FLIPBOOK_FRAMES;
        var filmed = footage_at(f, x, uv.y);
        let foot = smoothstep(0.0, 0.03, uv.y);
        filmed.cover *= foot;
        filmed.light *= foot;
        let drawn = wall_flame(filmed, heat, soft);
        light = drawn.rgb;
        cover = drawn.a;
        if t > 1.0 {
            // A hearth's tongue (its first number past 1, `hearth`): a campfire's flames as
            // filmed, the footage's own, solid through their body, without the soot or the
            // cooling a building's blaze is given.
            cover = filmed.cover * soft;
            light = filmed.light * FOOTAGE_GAIN * (0.85 + 0.15 * heat) * soft * on_screen();
        }
    } else if kind == 1 {
        let p = (uv - 0.5) * vec2(2.0, 2.0);
        let spark = 1.0 - smoothstep(0.2, 1.0, length(p));
        let wink = 0.6 + 0.4 * sin(time * (9.0 + luck * 14.0) + luck * 50.0);
        let alpha = spark * pow(1.0 - t, 1.5) * wink * soft;
        light = soot(1520.0 - t * 250.0, alpha);
    } else if kind == 4 {
        // A drop: a hot bright streak, its head hottest, cooling to red as it burns out.
        let x = abs(uv.x - 0.5) * 2.0;
        let streak = (1.0 - smoothstep(0.3, 1.0, x)) * smoothstep(0.0, 0.7, uv.y) * (1.0 - smoothstep(0.85, 1.0, uv.y));
        let wink = 0.75 + 0.25 * sin(time * (11.0 + luck * 9.0) + luck * 40.0);
        let alpha = streak * (1.0 - smoothstep(0.6, 1.0, t)) * wink * soft;
        light = soot(mix(1640.0, 1300.0, t) + 80.0 * uv.y, alpha);
        cover = covers(light, alpha);
    }
    // The fire's flicker, which its light burns by too: its flames as much brighter, its
    // sparks by twice as much (its coals' is in their glow above).
    if kind == 0 && life.w < 2.5 {
        light *= glow;
    } else if kind == 1 {
        light *= max(1.0 + 2.0 * (glow - 1.0), 0.2);
    }
    if max(light.r, cover) < 0.002 {
        discard;
    }
    return vec4(light, cover);
}

@vertex
fn vertex(vertex_in: Vertex) -> VertexOutput {
    let vertex = decompress_vertex(vertex_in, vertex_in.instance_index);
    var placed: Out;
    if i32(fire.kind.x + 0.5) == 2 {
        placed = hull(vertex.position, vertex.uv_b);
    } else {
        placed = place(vertex.uv, vertex.uv_b);
    }
    var out: VertexOutput;
    out.position = placed.clip;
@if(!PREPASS_PIPELINE) {
    // The fire's flicker rides in the position's w, read only by `shade`.
    out.world_position = vec4(placed.world, select(1.0, placed.glow, placed.glow > 0.0));
    // The card's life rides where a normal would.
    out.world_normal = placed.life.xyz;
    out.uv = placed.uv;
    out.uv_b = vec2(placed.life.w, placed.size);
}
@if(VERTEX_OUTPUT_INSTANCE_INDEX)
    out.instance_index = vertex_in.instance_index;
    return out;
}

@if(PREPASS_PIPELINE && PREPASS_FRAGMENT)
@fragment
fn fragment(in: VertexOutput) -> bevy_pbr::prepass::io::FragmentOutput {
    discard;
}

@if(PREPASS_PIPELINE && !PREPASS_FRAGMENT)
@fragment
fn fragment(in: VertexOutput) {
    discard;
}

// What is behind a heat haze card, shimmering: the opaque scene as Bevy copies it for
// transmissive materials, read a few pixels off (0.2 to 0.5% of the screen's width) by
// noise rising at a metre or two a second, as games draw heat haze; the most a third of the
// way up the card, none at its edges.
@if(!PREPASS_PIPELINE)
fn haze(in: VertexOutput) -> vec4<f32> {
    let time = fire.kind.z;
    let heat = in.world_normal.y;
    let rise = 1.5 / max(in.uv_b.y, 0.5);
    let p = in.uv - vec2(0.5, 0.35);
    let mask = (1.0 - smoothstep(0.15, 0.5, length(p * vec2(1.0, 0.75)))) * (0.4 + 0.6 * heat);
    if mask < 0.01 {
        discard;
    }
    let q = in.uv * vec2(5.0, 4.0) - vec2(0.0, time * rise * 4.0);
    let off = vec2(gnoise(q), gnoise(q + vec2(5.2, 1.3))) * 0.006 * mask;
    let screen = in.position.xy / view.viewport.zw;
    let behind = textureSampleLevel(view_transmission_texture, view_transmission_sampler, screen + off, 0.0).rgb;
    return vec4(behind, mask);
}

@if(!PREPASS_PIPELINE)
@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
    var out: FragmentOutput;
    if i32(fire.kind.x + 0.5) == 3 {
        out.color = haze(in);
        return out;
    }
    if i32(fire.kind.x + 0.5) == 2 {
        out.color = breath(in);
        return out;
    }
    // Faded in over the last of its way in front of what is behind it (soft particles).
    var soft = 1.0;
@if(DEPTH_PREPASS)
    {
        let kind = i32(fire.kind.x + 0.5);
        var reach = select(select(SOFT_M.z, SOFT_M.y, kind == 1 || kind == 4), SOFT_M.x, kind == 0);
        if kind == 0 && in.world_normal.x > 1.0 {
            reach = 0.06;
        }
        let behind = depth_ndc_to_view_z(in.position.z) - depth_ndc_to_view_z(prepass_depth(in.position, 0u));
        soft = smoothstep(0.0, reach, behind);
    }
    out.color = shade(in.uv, vec4(in.world_normal, in.uv_b.x), in.uv_b.y, soft, in.world_position.w);
    return out;
}
