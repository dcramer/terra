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

struct Fire {
    // x which (0 flames, 1 embers, 2 jets), y how many slots are filled, z the time.
    kind: vec4<f32>,
    // The wind, metres a second, and its strength.
    wind: vec4<f32>,
    // Each jet's from and reach; way and spread; on, and since when; the face it strikes, its
    // normal and how far off.
    jets: array<vec4<f32>, 8>,
    // The exposure weight, and the log2 of the exposure multiplier at dusk
    // (crates/terra_world/src/fire/exposure.rs).
    exposure: vec4<f32>,
    // Each flipbook set's rate (frames a second as baked), how wide its frame stands in
    // metres, its motion's scale, and whether its frames blend along their vectors (1).
    sets: array<vec4<f32>, 6>,
    // x what the ramp texture's RGB is multiplied by, y its rows.
    ramp: vec4<f32>,
    // How high up its frame each set's flame tips reach, 0 to 1 (its sidecar's `reach`):
    // x the campfire's, y the tongue's, z the torch's.
    reach: vec4<f32>,
}

// Field order must match FireExtension in crates/terra_world/src/fire.rs.
@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> fire: Fire;
@group(constants::MATERIAL_BIND_GROUP) @binding(101) var<storage, read> slots: array<vec4<f32>>;
// The flames' flipbooks (crates/terra_world/src/fire/books.rs), a set a layer: R heat, G
// cover, B soot, A distortion; their motion to the next frame at half size; the colour ramp.
@group(constants::MATERIAL_BIND_GROUP) @binding(102) var flames: texture_2d_array<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(103) var flames_sampler: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(106) var motions: texture_2d_array<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(107) var ramp: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(108) var ramp_sampler: sampler;
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

// The flipbook sets, layers in the order of books.rs SETS.
const SET_CAMPFIRE: i32 = 0;
const SET_TONGUE: i32 = 1;
const SET_TORCH: i32 = 2;
const SET_BREATH_PUFF: i32 = 3;
const SET_FIREBALL: i32 = 4;
const SET_EXPLOSION: i32 = 5;
// A set's frames and how many across (and down) its sheet; books.rs holds every set to them.
const BOOK_FRAMES: f32 = 64.0;
const BOOK_COLUMNS: f32 = 8.0;
// How bright each kind's flame is drawn, its ramp's luminance 1 at its hottest, on the screen
// whatever the eye is adapted to (a preset's, not the flipbook's): fitted on the baker's sheet
// (tools/fire/bake/sheet.ts), each set's exposure through today's tonemapper.
// The campfire at 32, its light a real one's, draws its core as the night photographs' (sRGB
// 254,235,133 against 253,224,133), its stones under it (research/2026-10-19-fire-step-back.md).
const HEARTH_BRIGHTNESS: f32 = 32.0;
const TORCH_BRIGHTNESS: f32 = 3.3;
const BLAZE_BRIGHTNESS: f32 = 2.1;
// A breath's: its puffs along the stream, the fireball where it strikes, the blast when it
// first does. Its puffs no longer sum where they overlap (`breath_fuel`), so at 3.5 the
// stream was 45% pale (breath r7), at 2.0 orange-yellow (r10).
const PUFF_BRIGHTNESS: f32 = 2.0;
// The fireball at 3.5 went 41% yellow by noon and 20% white by night through the new ramp (r27).
const BALL_BRIGHTNESS: f32 = 2.4;
// How much hotter than baked a breath's sets burn: brightness alone took its stream paler,
// not yellower (breath r3: 9 against 5.5, yellow 3% both, B/R 0.66 to 0.72), as the
// tonemapper keeps a hue; a fuel jet's flame is mostly yellow (MEASURED.md §2). At 1.5 it
// went 74% pale (r4), at 1.25 33% (r5). Once its puffs covered rather than summed, 1.2 was
// 68% yellow and 19% red and orange against at least 30 (r9), 1.0 holds (r10). The puff set
// rebaked to burn longer packs its heat about 8% lower (heat scale 1.40, was 1.52): at 1.0 it
// was 49% orange, at 1.09 31% pale at dusk with soot at its edge (r18, r20).
// The ramp filmed to a yellow-white hot end (D415) took the stream 18-24% pale at 1.05; the body
// burns cooler at 0.93 (orange-red, pale 10-12%) and the core, its first frames and its line
// near the jaw, hotter by `BREATH_CORE` 1.5, so only it reads yellow-white (breath r27).
const BREATH_HEAT: f32 = 1.0;
const BLAST_BRIGHTNESS: f32 = 5.0;
// A breath's core: how much hotter its puffs burn in their first frames, and how much
// brighter those on its line in its first third (`breath_sprite`).
const BREATH_CORE: f32 = 1.5;
// How thick a breath's baked soot is drawn: its soot channel b as cover 1 - exp(-k b).
const BREATH_SOOT: f32 = 6.0;
// How far a breath's hot flame covers what is behind it rather than adding to it, past what
// the ramp's heat alone gives (`breath_fuel`).
const BREATH_THICK: f32 = 1.0;
// Which of the ramp's rows (ramp.json) a breath burns by.
const BREATH_ROW: f32 = 0.0;

// Where a frame's place is on the sheet, x across and y up its card, kept a hair inside the
// frame so its neighbours never bleed in.
fn sheet_at(frame: f32, local: vec2<f32>) -> vec2<f32> {
    let column = frame % BOOK_COLUMNS;
    let row = floor(frame / BOOK_COLUMNS);
    let l = clamp(local, vec2(0.004), vec2(0.996));
    return vec2(column + l.x, row + 1.0 - l.y) / BOOK_COLUMNS;
}

// How a set's texel moves to the next frame, in frame widths, up the card: its motion read
// unfiltered, as SideFX Labs advises, since a filtered vector between two flows is neither.
fn flow(layer: i32, frame: f32, local: vec2<f32>) -> vec2<f32> {
    let size = vec2<f32>(textureDimensions(motions).xy);
    let texel = vec2<i32>(min(sheet_at(frame, local) * size, size - 1.0));
    let v = (textureLoad(motions, texel, layer, 0).rg - 0.5) * 2.0 * fire.sets[layer].z;
    return vec2(v.x, -v.y);
}

// A set at a moment `f` (in frames, any real number) at a place on its card: the frames either
// side, each carried along its motion to the moment between them and blended (the motion-vector
// flipbook blending of SideFX Labs and Unreal), so a tongue moves rather than fading from one
// frame to the next; crossfaded where the set's vectors bring its frames no nearer (books.rs).
fn flipbook(layer: i32, f: f32, local: vec2<f32>) -> vec4<f32> {
    let f0 = floor(f);
    let w = f - f0;
    let f1 = (f0 + 1.0) % BOOK_FRAMES;
    var a = local;
    var b = local;
    if fire.sets[layer].w > 0.5 {
        a = local - flow(layer, f0, local) * w;
        b = local + flow(layer, f1, local) * (1.0 - w);
    }
    let ta = textureSampleLevel(flames, flames_sampler, sheet_at(f0, a), layer, 0.0);
    let tb = textureSampleLevel(flames, flames_sampler, sheet_at(f1, b), layer, 0.0);
    return mix(ta, tb, w);
}

// The colour ramp at a heat (tools/fire/ramp.ts) on a fuel's row, its first wood's: the
// emission there in linear HDR, luminance 1 at its hottest, and how far it occludes rather
// than adds light.
fn ramp_row(heat: f32, row: f32) -> vec4<f32> {
    let n = f32(textureDimensions(ramp).x);
    let at = vec2((0.5 + saturate(heat) * (n - 1.0)) / n, (row + 0.5) / fire.ramp.y);
    let t = textureSampleLevel(ramp, ramp_sampler, at, 0.0);
    return vec4(t.rgb * fire.ramp.x, t.a);
}

fn ramp_at(heat: f32) -> vec4<f32> {
    return ramp_row(heat, 0.0);
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
// A hearth whose flames stand lower than this, in metres, is a torch (books.rs SETS).
const TORCH_M: f32 = 0.8;
// A hearth's ground card's half width over the fire's width: inside its ring of stones, so
// its edge is under them; and how dark its char is there.
const GROUND_WIDE: f32 = 0.95;
const CHAR_COVER: f32 = 0.45;
// A hearth's cards (crates/terra_world/src/fire.rs HEARTH_FLAME_CARDS): its ground, its coals,
// its body's columns (`HEARTH_COLUMNS`), then its tongues on the logs.
const HEARTH_CARDS: f32 = 16.0;
// Its body's height over its tallest tips (`tall`): standing about as high again over its
// logs as they are high, as the games' campfires do (research/2026-10-19-fire-step-back.md,
// round 12); at 0.55 to 0.7 the logs hid all but its tips, at 0.8 it read short (blind
// review 5/10 against 6/10 at 0.95). Its foot stands a little up
// among the logs (`HEARTH_BODY_UP`, over the fire's width), its hot base over the coals.
const HEARTH_BODY: f32 = 0.95;
const HEARTH_BODY_UP: f32 = 0.15;
// Its tongues: their heights over `tall`, least and most; how far out from the middle they
// rise, over the fire's width (the logs' teepee); how far they lean in along the logs; and the
// teepee's height over the fire's width, so each rises from the logs' top where it stands and
// not inside them, hidden (round 12's first try).
const TONGUE_TALL: vec2<f32> = vec2<f32>(0.26, 0.55);
const TONGUE_OUT: f32 = 0.5;
const TONGUE_LEAN: f32 = 0.3;
const TEEPEE: vec2<f32> = vec2<f32>(0.62, 0.6);
// How far the body's cover is eaten away toward its tips (alpha erosion), how much of what is
// behind its upper part it lets through, and how much hotter its foot burns.
const HEARTH_ERODE: f32 = 0.6;
const HEARTH_SEE: f32 = 0.8;
// The erosion's feather, and its floor: the threshold under the body's whole height, so its
// faint rim is eaten too and the flame has an edge, not an orange halo round it.
const HEARTH_FEATHER: f32 = 0.07;
const HEARTH_ERODE_FLOOR: f32 = 0.12;
// Its body as a few overlapping columns of flame round the stack, not one sheet behind it:
// how many, how much of the baked frame's width each shows, and how far out from the middle
// they stand, over the fire's width (some in front of the logs from any side).
const HEARTH_COLUMNS: i32 = 4;
const COLUMN_WINDOW: f32 = 0.36;
const COLUMN_OUT: f32 = 0.55;
const HEARTH_BASE_HOT: f32 = 0.15;
// How much hotter a hearth's brightest coals glow over its dullest, kelvin.
const COAL_HOT: f32 = 420.0;
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

// A baked texel drawn premultiplied by docs/fire.md's rule: its heat coloured through the
// ramp, as bright as the cover over it and the kind's brightness, its soot dark over what is
// behind; covering what is behind by its cover as far as its heat occludes, and by its soot.
// Its emission and its cover are the sim's own, never one taken from the other.
fn baked_flame(t: vec4<f32>, brightness: f32, fade: f32) -> vec4<f32> {
    return baked_fuel(t, brightness, fade, 0.0);
}

fn baked_fuel(t: vec4<f32>, brightness: f32, fade: f32, row: f32) -> vec4<f32> {
    let cover = t.g * fade;
    let soot = t.b * fade;
    let r = ramp_row(t.r, row);
    let light = r.rgb * cover * brightness + SOOT_BLACK * soot;
    return vec4(light * on_screen(), min(1.0, cover * r.a + soot));
}

// A breath's sprite drawn the same way, but occluding as a thick flame: a fuel jet's burning
// gas is optically thick, so its puffs cover what is behind them rather than sum over it.
// Occluding only as the ramp's hot end does (near 0), the stream's overlapping puffs summed
// past the tonemapper's knee to a salmon middle and over the grass to olive (breath r6).
// Its soot is the rod's (`breath_soot`): faintly lit while it still wraps the flame (`lit`
// 1), black once it has cooled; unlit by the stream it stood black against the night at the
// stream's very edge, a cut-out (breath r14).
// A breath's colour by heat alone, 0 to 1, as a blackbody runs: black, dark brown, red,
// orange, yellow, pale, white. Never a yellow made dim: a dim yellow is olive or khaki (every
// breath round to r29's "cauliflower"), so what is cooler or sootier is redder and darker.
fn breath_ramp(h: f32) -> vec3<f32> {
    let black = vec3(0.004, 0.003, 0.0025);
    let brown = vec3(0.05, 0.016, 0.006);
    let red = vec3(0.42, 0.045, 0.004);
    let deep = vec3(0.95, 0.16, 0.008);
    let orange = vec3(1.0, 0.36, 0.025);
    let yellow = vec3(1.0, 0.68, 0.07);
    let pale = vec3(1.05, 0.9, 0.32);
    var c = mix(black, brown, smoothstep(0.0, 0.15, h));
    c = mix(c, red, smoothstep(0.15, 0.32, h));
    c = mix(c, deep, smoothstep(0.32, 0.5, h));
    c = mix(c, orange, smoothstep(0.5, 0.68, h));
    c = mix(c, yellow, smoothstep(0.68, 0.84, h));
    c = mix(c, pale, smoothstep(0.84, 0.93, h));
    let gain = mix(1.0, FLAME_GAIN, smoothstep(0.2, 0.5, h));
    return mix(c * gain, vec3(0.95, 0.92, 0.82) * WHITE, smoothstep(0.93, 1.0, h));
}

// A breath's sprite, premultiplied: dissolved over its life by a hard-edged threshold on its
// body (erosion, as shipped flamethrowers are: Real-Time VFX, 80.lv), never faded by alpha,
// so it is opaque or gone and never a thinned yellow over what is behind it. `keep` is how
// much of it is left, 1 whole, 0 gone; its smoke erodes softly; its colour is by heat alone.
fn breath_eroded(heat: f32, body: f32, smoke: f32, keep: f32, luck: f32, uv: vec2<f32>) -> vec4<f32> {
    let n = noise(uv * 11.0 + vec2(luck * 31.0, luck * 17.0));
    let m = noise(uv * 4.0 - vec2(luck * 13.0, luck * 29.0));
    let e = saturate(1.0 - keep) * 0.85 + 0.04;
    let d = body + 0.18 * (n - 0.5);
    let flame = smoothstep(e, e + 0.04, d);
    let sm = smoothstep(e * 0.8, e * 0.8 + 0.3, smoke) * 0.9;
    // Cooler toward the eroding rim and as it burns out, but a flame's rim is red, never
    // brown: hard-edged and dark at the rim, every puff was outlined as a cartoon (r30/a).
    let h = heat * mix(0.8, 1.0, smoothstep(e, e + 0.35, d)) * mix(0.75, 1.0, saturate(keep)) + 0.22 * (m - 0.5);
    let fire_c = breath_ramp(clamp(h, 0.4, 1.0));
    // Its soot is brown going black, faintly glowing where the flame is hot round it.
    let soot_c = breath_ramp(0.08 + 0.12 * saturate(heat));
    let sooty = smoothstep(0.25, 0.75, sm / (sm + flame + 1.0e-3));
    let a = max(flame, sm);
    return vec4(mix(fire_c, soot_c, sooty) * a * on_screen(), a);
}
// A breath is a composite (docs/fire.md, "Composites"; Monster Hunter World's Zeno Jiva): a
// hot rod from the jaw, marched as a volume (below), wrapped in flame sprites from the baked
// sets (`breath_sprite`), with soot, drops and a light of its own.
//
// Its rod is drawn as a volume, not as cards (research/2026-10-18-dragon-breath.md §6): a
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
    // The face it strikes: how far along its way, and which way it looks (`face` 0 when it
    // strikes nothing).
    face: f32,
    normal: vec3<f32>,
}

fn breath_of(slot: u32) -> Breath {
    let j = slot * 4u;
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
    b.face = fire.jets[j + 3u].w;
    b.normal = fire.jets[j + 3u].xyz;
    b.up = vec3(dot(up, b.lift), dot(up, b.side), dot(up, b.way));
    let flown = JET_SPEED * max(time - since, 0.0);
    // Once on, the gas reaches as far as it has flown; once off, the last of it flies on
    // from the jaw and burns out in a moment rather than flying on as a cloud cut loose.
    b.front = select(1.0e5, flown, on);
    b.back = select(-1.0, flown, !on);
    b.left = select(saturate(1.0 - (time - since) / 0.45), 1.0, on);
    // The fireball where it strikes is the sprites' (`breath_sprite`), no longer marched.
    b.ball = 0.0;
    return b;
}

// The rod's radius so far along it, in metres: a tight rope at the jaw, opening a little,
// inside the sprites that carry the stream's turbulence; it burns out by `ROD_ENDS` of the
// way, where the stream is all puffs.
const ROD_WIDTH: f32 = 0.2;
const ROD_OFF: bool = true;
const ROD_ENDS: f32 = 0.55;
fn stream_radius(z: f32, b: Breath) -> f32 {
    return (0.22 + b.spread * z * (0.7 + 0.5 * saturate(z / b.reach))) * ROD_WIDTH;
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
    let u = length(q) + (e - 0.5) * mix(0.9, 1.8, open);
    var stream = 1.0 - smoothstep(0.78, 1.0, u);
    // Its tip tears apart into flying blobs that burn out.
    let tear = smoothstep(0.55, 1.0, zf) * 0.75;
    let blobs = noise3(vec3(q * 0.8 + 3.1, flow * 0.16 + 11.0));
    stream *= smoothstep(tear - 0.12, tear + 0.12, blobs + 0.12 * (1.0 - zf));
    stream *= smoothstep(-0.25, 0.25, l.z) * (1.0 - smoothstep(b.front - 1.5, b.front + 0.5, l.z)) * smoothstep(b.back - 0.5, b.back + 1.5, l.z);
    stream *= 1.0 - smoothstep(ROD_ENDS * 0.5, ROD_ENDS, zf);
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
    let hot_s = core * (1.0 - 0.5 * smoothstep(0.3, 0.95, u)) + 0.45 * (e - 0.5) + 0.3 * (streak - 0.5);
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

// A breath's sprites (crates/terra_world/src/fire.rs BREATH_CARDS), in index order, which is
// the order they are drawn in: the soot stain on the face it strikes, under everything; the stream's puffs, oldest first so the hot young ones near the
// jaw lie over the soot they become; the fireball rolling where it strikes; the blast when the
// stream first strikes, and its flash.
const BREATH_CARDS: f32 = 46.0;
const STAIN_CARDS: f32 = 2.0;
const PUFF_CARDS: f32 = 26.0;
const BEAM_CARDS: f32 = 3.0;
const BALL_CARDS: f32 = 10.0;
const BLAST_CARDS: f32 = 5.0;
// How long a puff burns on past where the stream strikes or ends, seconds: rolling out and
// up as black soot.
const PUFF_LINGER_S: f32 = 0.9;
// How far a landed puff's card turns from the eye to lie along the face it burns on: laid
// on it wholly, from an eye looking along the face its flame was a horizontal smear past the
// wall's end (breath r25); turned to the eye, half of it stood behind the face.
const FACE_HUG: f32 = 0.5;
// How long a fireball's own puff lives, seconds at the set's size (Froude: as the root of
// its size), and the blast's.
const BALL_S: f32 = 2.67;

// A card turned to the eye about the line `up`, its middle at `at`, `w` across and `h` along,
// turned by `spin` on the screen.
fn sprite(at: vec3<f32>, up: vec3<f32>, corner: vec2<f32>, w: f32, h: f32, spin: f32) -> vec3<f32> {
    return sprite_facing(at, up, normalize(view.world_position - at), corner, w, h, spin);
}

// The same card turned to look along `facing` rather than at the eye: laid on a face it
// burns on, as a game's surface-aligned flame cards are.
fn sprite_facing(at: vec3<f32>, up: vec3<f32>, facing: vec3<f32>, corner: vec2<f32>, w: f32, h: f32, spin: f32) -> vec3<f32> {
    let to_eye = facing;
    var right = cross(up, to_eye);
    if dot(right, right) < 1.0e-6 {
        right = cross(vec3(1.0, 0.0, 0.0), to_eye);
    }
    right = normalize(right);
    let along = cross(to_eye, right);
    let c = corner - 0.5;
    let turned = vec2(c.x * cos(spin) - c.y * sin(spin), c.x * sin(spin) + c.y * cos(spin));
    return at + right * turned.x * w + along * turned.y * h;
}

// Whether a puff born at `born` was breathed: after the jet last came on, and before it went
// off if it is off.
fn breathed(j: u32, born: f32) -> bool {
    let on = fire.jets[j * 4u + 2u].x > 0.5;
    let since = fire.jets[j * 4u + 2u].y;
    let came_on = fire.jets[j * 4u + 2u].z;
    return born >= came_on && (on || born <= since);
}

// How far what a jet feeds where it strikes still burns: wholly while it is on, gone within
// half a second once it is off, as the stream's own puffs go (`Breath::left`). Burning on for
// its own life, a fireball's puff hung on the wall after the stop as a lone glowing ball
// (breath r28's review).
fn fed(j: u32) -> f32 {
    let on = fire.jets[j * 4u + 2u].x > 0.5;
    let since = fire.jets[j * 4u + 2u].y;
    return select(1.0 - smoothstep(0.1, 0.4, fire.kind.z - since), 1.0, on);
}

// A breath's hot core near the jaw, as games draw a flamethrower's (docs/fire.md,
// "Composites"; Unreal flame bursts and flamethrowers on Real-Time VFX: a beam drawn as a
// ribbon, its material panning noise along it): a few cards along the stream's line, each
// turned to the eye about that line, shaded in `breath_beam_shade` from where the fragment is
// on the stream, so its noise flies with the gas. Widest and coolest first, the core last over
// them; the core opaque and white-yellow, its edge torn and orange-red, tearing into tongues
// toward its end. Marched as a volume, it read as a smooth cream tube (breath r24 to r28).
fn beam_radius(z: f32, b: Breath, layer: f32) -> f32 {
    // The main stream, not a needle: at a rod's radii (0.1 m at the jaw) it read as a thin
    // white line (breath r29's review); half a metre at the jaw, over a metre at the face.
    return (0.32 + 0.07 * z + b.spread * 0.2 * z) * (1.0 + 0.5 * layer);
}

fn breath_beam(corner: vec2<f32>, b: Breath, jet: u32, i: f32) -> Out {
    let layer = BEAM_CARDS - 1.0 - i;
    let ends = select(b.reach, b.face, b.face > 0.0) * mix(0.98, 0.82, layer / 2.0);
    let z1 = min(b.front, ends);
    let z0 = max(b.back, 0.0);
    if z1 - z0 < 0.2 {
        return nowhere();
    }
    let w = 3.6 * beam_radius(z1, b, layer);
    let start = z0 - 0.1;
    let to = z1 + 0.5 * w;
    // A ribbon along the line itself, turned to the eye only about it, as a beam's is: a
    // card turned wholly to the eye leaned toward it, so a stream seen at a slant spanned only
    // its middle stretch, never the jaw, one flat banded oval with a seam (breath r31's tints).
    let at = b.mouth + b.way * 0.5 * (start + to);
    var right = cross(b.way, normalize(view.world_position - at));
    if dot(right, right) < 1.0e-6 {
        right = b.side;
    }
    right = normalize(right);
    let world = b.mouth + b.way * mix(start, to, corner.y) + right * (corner.x - 0.5) * w;
    var out: Out;
    out.clip = position_world_to_clip(world);
    out.uv = corner;
    out.world = world;
    // Its jet past 2, so the flame mask counts it a flame (`fragment`); the layer; where it ends.
    out.life = vec4(f32(jet) + 2.0, layer, z1, -3.0);
    out.size = 0.3;
    out.glow = 1.0;
    return out;
}

// The beam at a fragment, premultiplied: its light, already exposed, and its cover.
@if(!PREPASS_PIPELINE)
fn breath_beam_shade(p: vec3<f32>, uv: vec2<f32>, life: vec4<f32>, soft: f32) -> vec4<f32> {
    let b = breath_of(u32(life.x - 2.0 + 0.5));
    let layer = life.y;
    let z1 = life.z;
    let time = fire.kind.z;
    let rel = p - b.mouth;
    let z = dot(rel, b.way);
    let r = beam_radius(max(z, 0.0), b, layer);
    let u = length(rel - b.way * z) / r * sign(uv.x - 0.5);
    let zf = saturate(z / max(z1, 0.1));
    let flow = JET_SPEED * time - z;
    let seed = 7.3 * layer + 3.1 * f32(life.x);
    // Panning noise along the line, finer at the jaw, coarser as it opens.
    let e = fbm(vec2(u * 2.6 + seed, flow * 1.1));
    let f = fbm(vec2(u * 6.3 - seed, flow * 3.2));
    let d = abs(u) + (e - 0.5) * mix(0.6, 1.3, zf) + (f - 0.5) * 0.35;
    var body = 1.0 - smoothstep(0.55, 0.9, d);
    // Torn into tongues toward its end, as a jet's rope breaks up.
    let tongues = noise(vec2(u * 0.9 + seed, flow * 0.3));
    let tear = smoothstep(0.45, 1.0, zf) * 0.7;
    body *= smoothstep(tear - 0.12, tear + 0.12, tongues + 0.1 * (1.0 - zf));
    // From the jaw, and gone ragged at its end and its tail, never a straight cut.
    body *= smoothstep(-0.05, 0.25, z) * smoothstep(b.back, b.back + 1.5, z);
    body *= 1.0 - smoothstep(0.75, 1.0, zf + (e - 0.5) * 0.4);
    // A soft start and end: grown out of its core over a third of a second once on, eroded
    // back to it once off (`Breath::left`), never cut (breath r29's hard stop).
    let jet = u32(life.x - 2.0 + 0.5);
    let came_on = fire.jets[jet * 4u + 2u].z;
    let grown = smoothstep(0.0, 0.35, time - came_on) * b.left;
    // Gone as the eye looks along it, where a ribbon is seen edge on; the puffs round it
    // carry the stream there, as a shipped beam's do.
    let along = abs(dot(normalize(p - view.world_position), b.way));
    let thr = mix(0.92, 0.04, grown * soft * (1.0 - smoothstep(0.88, 0.98, along)));
    let cover = smoothstep(thr, thr + 0.08, body + 0.12 * (f - 0.5));
    if cover < 0.003 {
        discard;
    }
    // Dense and opaque; white-yellow on its line at the jaw, orange-red down its length and
    // at its torn edge, by heat alone (`breath_ramp`).
    let h = mix(1.02, 0.7, smoothstep(0.0, 0.9, zf)) * (1.0 - 0.3 * smoothstep(0.2, 0.95, d)) + 0.45 * (e - 0.5) + 0.25 * (f - 0.5) - 0.05 * layer;
    return vec4(breath_ramp(saturate(h)) * cover * on_screen(), cover);
}

// The face a breath strikes, in the world: where its stream meets it, which way it looks,
// and the way up it and across it (on a floor, up it is on along the stream); striking
// nothing, the stream's end, looking back along it.
struct Face {
    at: vec3<f32>,
    along: f32,
    normal: vec3<f32>,
    up: vec3<f32>,
    side: vec3<f32>,
}

fn face_of(b: Breath) -> Face {
    var f: Face;
    let struck = b.face > 0.0;
    f.normal = select(-b.way, normalize(b.normal), struck);
    f.along = select(b.reach, b.face, struck);
    f.at = b.mouth + b.way * f.along;
    let up = normalize(b.mouth);
    var face_up = up - f.normal * dot(up, f.normal);
    if dot(face_up, face_up) < 0.04 {
        face_up = b.way - f.normal * dot(b.way, f.normal);
    }
    f.up = normalize(face_up);
    f.side = normalize(cross(f.normal, f.up));
    return f;
}

// How long a struck face takes to blacken fully, seconds of the stream on it.
const STAIN_S: f32 = 4.0;

// The soot a breath leaves on the face it strikes (docs/fire.md, "Composites": a scorch):
// a card laid on the face, a hair off it, darkening it as the stream plays on it; a plume of
// soot over the hit, as fire leaves above a burnt window or a hearth's mouth, and a char spot
// where the stream meets it. Kept to the face by depth (`fragment`), as a decal is.
fn breath_stain(corner: vec2<f32>, b: Breath, jet: u32, i: f32) -> Out {
    if b.face <= 0.0 {
        return nowhere();
    }
    let time = fire.kind.z;
    let came_on = fire.jets[jet * 4u + 2u].z;
    let struck = came_on + b.face / JET_SPEED;
    let on = fire.jets[jet * 4u + 2u].x > 0.5;
    let until = select(fire.jets[jet * 4u + 2u].y, time, on);
    let grown = saturate((until - struck) / STAIN_S);
    if grown <= 0.0 {
        return nowhere();
    }
    let f = face_of(b);
    let ball = 0.8 + (0.12 + b.spread * 0.45 * b.reach) * 0.9;
    // The plume over the hit, rising as it spreads; the char spot round the hit.
    let w = select(ball * 1.7, ball * (1.8 + 1.0 * sqrt(grown)), i < 0.5);
    let h = select(ball * 1.5, ball * (2.6 + 2.2 * sqrt(grown)), i < 0.5);
    let foot = select(-0.55, -0.2, i < 0.5);
    let at = f.at + f.normal * (0.03 + 0.01 * i) + f.up * h * (0.5 + foot);
    let world = sprite_facing(at, f.up, f.normal, corner, w, h, 0.0);
    var out: Out;
    out.clip = position_world_to_clip(world);
    out.uv = corner;
    out.world = world;
    out.life = vec4(grown, i, hash(vec2(f32(jet), came_on)), -2.0);
    out.size = 0.01;
    out.glow = 1.0;
    return out;
}

fn breath_sprite(corner: vec2<f32>, jet: u32, phase: f32) -> Out {
    let b = breath_of(jet);
    if b.reach <= 0.0 {
        return nowhere();
    }
    let time = fire.kind.z;
    let card = floor(phase * BREATH_CARDS + 0.5);
    let up = normalize(b.mouth);
    if card < STAIN_CARDS {
        return breath_stain(corner, b, jet, card);
    }
    // Drawn after the stain: the fireball and the blast, then the puffs, then the beam over
    // them, as each card covers what was drawn before it. Opaque and drawn after the beam,
    // the fireball's cards hid the stream from the jaw (r30/a).
    let k0 = card - STAIN_CARDS;
    let back = BALL_CARDS + BLAST_CARDS;
    let k = select(k0 - back, PUFF_CARDS + BEAM_CARDS + k0, k0 < back);
    // The stream's flight to where it strikes, and how wide the puffs spread round its line
    // there: a flamethrower's rope opening to a few metres where it lands.
    let flight = b.reach / JET_SPEED;
    let spread = 0.12 + b.spread * 0.45 * b.reach;
    var out: Out;
    if k < PUFF_CARDS {
        // A puff of burning fuel thrown from the jaw at the stream's speed, slowing, opening
        // out from its line, burning through the set's flame (its first dozen frames) on its
        // way and rolling out as soot where it lands.
        let life = flight + PUFF_LINGER_S;
        let rate = PUFF_CARDS / life;
        let tick = time * rate;
        let rank = PUFF_CARDS - 1.0 - k;
        let age = (fract(tick) + rank) / rate;
        let n = floor(tick) - rank;
        if !breathed(jet, time - age) {
            return nowhere();
        }
        let h1 = hash(vec2(n, f32(jet) + 0.5));
        let h2 = hash(vec2(n + 0.37, 3.1));
        let h3 = hash(vec2(n + 0.71, 7.7));
        let h4 = hash(vec2(n + 0.13, 11.3));
        let s = age / flight;
        let gone = min(s, 1.0);
        let struck = b.face > 0.0;
        let f = face_of(b);
        let normal = f.normal;
        let face = f.at;
        let face_up = f.up;
        let face_side = f.side;
        let d = f.along * gone * (1.35 - 0.35 * gone);
        let post = max(age - flight, 0.0);
        let a = h1 * 6.2831853;
        let out_of_line = b.lift * cos(a) + b.side * sin(a);
        let near = smoothstep(0.0, 0.35 * b.reach, d);
        let r = (0.08 + b.spread * 0.45 * d) * sqrt(h2) * 0.9 * mix(0.35, 1.0, near);
        // As big as the stream is wide there, and swelling as it burns out: a tight rope for
        // its first third, so its hot core reads (as wide as the rod from the jaw, it was lost).
        // The set's puff fills a fifth of its frame as it flares (cut to a quarter, the stream
        // went thin and the climbing soot showed through it as a gap, breath r22).
        // A few, in mixed sizes (a big slow middle, small quick edges), round the beam.
        let wide = mix(0.12 + 0.08 * d, 0.38 + 0.22 * d, near) * (0.6 + 0.8 * h3) + post * 1.2;
        let card_m = wide / 0.2;
        // Landed, it splashes out over the face it struck, slowing, most of it up the face
        // rather than down, and climbs it fast while it still burns, laid on it and kept off it
        // only a hand's breadth: a flamethrower's fuel spreads along a wall and its flame licks
        // up it (Monster Hunter World's Fatalis on a palisade, Elden Ring's Agheel on a
        // standing stone). Once burnt out it lifts off the face as soot rolling up over it.
        // Driven past the face as far as the stream runs on into it and lifted along its frame
        // off the stream's line, it stood behind the wall and over it (breath r24's review).
        let spread_on = post / (post + 0.25);
        let lifted = smoothstep(0.3, 0.8, post);
        let flying = b.mouth + b.way * d + out_of_line * r;
        let standing = max(0.15, dot(flying - face, normal));
        let across = face_side * sin(a) * (1.0 + 1.5 * h3) + face_up * (0.3 + 1.2 * abs(cos(a)));
        let climb = face_up * post * (3.5 + 2.5 * h4) * (0.7 + post);
        let off = max(standing, 0.4 * wide) + 0.3 * wide * lifted;
        let landed = flying - normal * (dot(flying - face, normal) - off) + across * 1.1 * spread_on + climb;
        let at = select(flying, landed, s >= 1.0 || dot(flying - face, normal) < 0.15);
        // Laid on the face from the last of its flight until it lifts off (the flame on it);
        // turned to the eye otherwise. Turned to the eye on the face, a card as wide as the
        // splash stood half behind the face from any eye looking along it.
        let hugging = select(0.0, smoothstep(0.85, 1.0, s) * (1.0 - lifted) * FACE_HUG, struck);
        let to_eye = normalize(view.world_position - at);
        var facing = normalize(mix(to_eye, normal, hugging));
        // From behind the face, the card lies on it facing the eye's side.
        facing = select(facing, -facing, dot(facing, to_eye) < 0.0);
        // Drawn along the stream as it flies, stretched along it near the jaw; up the face
        // once landed, rolled every way as it splashes, so the landed cards never stack as
        // level bands (breath r24: "stacked cards like blinds").
        let line = normalize(mix(b.way, face_up, smoothstep(0.75, 1.15, s)));
        let stretch = mix(2.4, 1.0, smoothstep(0.0, 0.6, s));
        let spin = (h4 - 0.5) * mix(0.9, 2.4, spread_on);
        let world = sprite_facing(at, line, facing, corner, card_m, card_m * stretch, spin);
        // Its edge burns out first: a puff flung out from the line plays on into the set's
        // soot in the stream's back half, so soot folds out of its edges, as a fuel jet's does.
        // Its top edge only, as soot rolls up off a flamethrower's rope (M113, Iraq photos).
        let edge = smoothstep(0.45, 1.0, h2) * smoothstep(-0.3, 0.6, cos(a));
        // The set burns from frame 0 to 21, its flame largest at 15 and cooling: its middle
        // reaches the wall at frame 12, still burning; on the face it burns on slowly for half
        // a second, each at its own moment, then rolls up as soot. Played on at 29 frames a
        // second, it was soot a third of a second after it struck, a black hole where the
        // stream met the wall (breath r24's review).
        let flown = 12.0 * pow(s, 0.85) * (1.0 + 0.7 * edge * smoothstep(0.65, 1.0, s));
        let on_face = 12.0 * (1.0 + 0.7 * edge) + 3.0 * (h3 - 0.5) + 10.0 * post + 30.0 * max(post - 0.55, 0.0);
        let frame = select(flown, on_face, s >= 1.0);
        // How much of it is left (`breath_eroded`): eroded in past the first fifth of its
        // flight, so the beam alone shows at the jaw, and eroded away over its life.
        let fade = smoothstep(0.0, 0.04, age) * smoothstep(0.12, 0.35, s) * (1.0 - smoothstep(0.3, 1.0, age / life)) * b.left;
        out.world = world;
        out.clip = position_world_to_clip(world);
        out.uv = corner;
        // Its middle near the jaw brightest, past the tonemapper's knee to white-yellow: the
        // core a fuel jet's rope shows there before it opens out.
        // Burning as bright to the wall, brightest as it splashes there.
        let core = 1.0 + BREATH_CORE * (1.0 - h2) * (1.0 - near);
        let strike = 1.0 + 0.6 * step(1.0, s) * (1.0 - smoothstep(0.0, 0.35, post));
        out.life = vec4(frame, PUFF_BRIGHTNESS * mix(1.2, 1.05, gone) * core * strike, h3, f32(SET_BREATH_PUFF));
        // A breath sprite's size is how far it is faded before what is behind it (`fragment`):
        // a metre, but a hand's breadth for a card laid on the face, which stands only that off it.
        // Its jet, for the face it is faded before (`fragment`).
        out.size = f32(jet);
        out.glow = max(fade, 1.0e-3);
        return out;
    }
    if k < PUFF_CARDS + BEAM_CARDS {
        return breath_beam(corner, b, jet, k - PUFF_CARDS);
    }
    // Where it strikes: a fireball's puffs, one after another, rolling up off what it struck
    // and burning out to a cap of soot rising over it, as long as the stream feeds it.
    // Rolled up what it struck and back off it, so it stands over the stream's foot, not
    // behind a wall's: centred a little off the hit, most of it was hidden (breath r6).
    let ball = 0.8 + spread * 0.9;
    // Off the face it struck and up it, so it stands over the splash: from the point past the
    // face the stream runs on to, it stood half behind a wall (breath r24).
    let struck_at = face_of(b);
    let middle = struck_at.at + struck_at.normal * ball * 0.9 + struck_at.up * ball * 0.9;
    if k < PUFF_CARDS + BEAM_CARDS + BALL_CARDS {
        // Many puffs a few times smaller than the ball, each rolling out over the face from
        // the hit, along it and up it, growing and turning as it goes, each at its own frame of
        // the set: seven times the ball, they stood on one middle and showed the set's first
        // frames, a round disc the same every frame (breath r27's review).
        let card_m = ball * 1.8;
        let life = BALL_S * sqrt(card_m / fire.sets[SET_FIREBALL].y) * 0.8;
        let rate = BALL_CARDS / life;
        let tick = time * rate;
        let rank = PUFF_CARDS + BEAM_CARDS + BALL_CARDS - 1.0 - k;
        let age = (fract(tick) + rank) / rate;
        let n = floor(tick) - rank;
        // Fed by what has reached it: born once the stream's front struck.
        if !breathed(jet, time - age - flight) {
            return nowhere();
        }
        let h1 = hash(vec2(n, f32(jet) + 9.5));
        let h2 = hash(vec2(n + 0.37, 13.1));
        let h3 = hash(vec2(n + 0.71, 17.7));
        let h4 = hash(vec2(n + 0.13, 21.3));
        let t = age / life;
        let a = h1 * 6.2831853;
        // Out along the face more than up it at first, then lifting as it cools.
        let way_out = struck_at.side * cos(a) * 1.4 + struck_at.up * (0.25 + 0.75 * abs(sin(a)));
        let roll = ball * (0.25 + 1.5 * sqrt(t)) * (0.6 + 0.7 * h2);
        let off = struck_at.normal * ball * (0.35 + 0.45 * h3 + 0.5 * t);
        let at = struck_at.at + off + way_out * roll + up * (ball * 0.3 + age * age * 1.2);
        let size = card_m * (0.55 + 0.6 * h3) * (0.7 + 0.6 * sqrt(t));
        let world = sprite(at, up, corner, size, size, h4 * 6.2831853 + (h2 - 0.5) * 2.5 * t);
        let fade = smoothstep(0.0, 0.08, age) * (1.0 - smoothstep(0.3, 1.0, t)) * fed(jet);
        out.world = world;
        out.clip = position_world_to_clip(world);
        out.uv = corner;
        // Past the set's first, round frames, from its eighth to fourteenth on, and slow
        // through its burning ones, so it billows as a mushroom rather than shows a ball.
        let first = 8.0 + 6.0 * h2;
        out.life = vec4(first + (63.0 - first) * pow(t, 1.6), BALL_BRIGHTNESS, h3, f32(SET_FIREBALL));
        out.size = f32(jet);
        out.glow = max(fade, 1.0e-3);
        return out;
    }
    // The blast when the stream's front first strikes: the explosion set played once, a few
    // cards of it a moment apart, and a flash over them.
    let came_on = fire.jets[jet * 4u + 2u].z;
    let struck = came_on + flight;
    let i = k - PUFF_CARDS - BEAM_CARDS - BALL_CARDS;
    if !breathed(jet, came_on) || time < struck {
        return nowhere();
    }
    let card_m = ball * 5.0 * (1.0 + 0.15 * i);
    if i >= BLAST_CARDS - 1.0 {
        // The flash: a ball of light as wide as the blast for a tenth of a second or so.
        let age = time - struck;
        if age > FLASH_S {
            return nowhere();
        }
        let world = sprite(middle, up, corner, card_m, card_m, 0.0);
        out.clip = position_world_to_clip(world);
        out.uv = corner;
        out.life = vec4(age / FLASH_S, BLAST_BRIGHTNESS, 0.5, -1.0);
        out.size = card_m;
        out.glow = 1.0;
        return out;
    }
    let age = time - struck - i * 0.09;
    // Played quick, a blast's moment: at the set's pace its sooty end still hung over the hit
    // two seconds on as a dark disc in the fireball (breath r8), once its soot was drawn thick.
    let life = BALL_S * sqrt(card_m / fire.sets[SET_EXPLOSION].y) * 0.5;
    if age < 0.0 || age > life {
        return nowhere();
    }
    let h = hash(vec2(i, came_on));
    let a = h * 6.2831853;
    let at = middle + (b.lift * cos(a) + b.side * sin(a)) * ball * 0.4 * min(i, 1.0) + up * (card_m * 0.1 + age * age * 1.5);
    let world = sprite(at, up, corner, card_m, card_m, (h - 0.5) * 0.5);
    let fade = smoothstep(0.0, 0.03, age) * (1.0 - smoothstep(0.5, 1.0, age / life));
    out.world = world;
    out.clip = position_world_to_clip(world);
    out.uv = corner;
    out.life = vec4(63.0 * age / life, BLAST_BRIGHTNESS, h, f32(SET_EXPLOSION));
    out.size = f32(jet);
    out.glow = max(fade, 1.0e-3);
    return out;
}
// How long a blast's flash lasts, seconds (docs/fire.md: 0.1 to 0.3 s).
const FLASH_S: f32 = 0.15;

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

// A tongue of a hearth's flame licking up its logs (`hearth`'s cards past its two bodies), as
// a campfire's small flames rise in the games and in footage: each born at its own place on the
// logs, leaning in along them, rising and stretching over its short life while its tip is eaten
// away (`shade`, alpha erosion) until it pinches off and is gone, then born again elsewhere.
// Its life's first number is 1 and its age, so it is soft only very near what is behind it.
fn hearth_tongue(corner: vec2<f32>, foot: vec3<f32>, up: vec3<f32>, east: vec3<f32>, north: vec3<f32>, tall: f32, heat: f32, wide: f32, seed: f32, k: f32, breathe: f32) -> Out {
    var out: Out;
    let own = fract(seed * 0.731 + k * 0.618);
    let period = 0.5 + 0.55 * fract(own * 7.13);
    let clock = fire.kind.z / period + own * 3.0;
    let cycle = floor(clock);
    let age = clock - cycle;
    let pick = hash(vec2(cycle * 1.37 + k * 7.0, seed * 13.0 + k));
    let ang = (k + pick * 0.8) * 2.39996 + seed * 6.283;
    let reach = wide * TONGUE_OUT;
    let r = reach * (0.12 + 0.88 * sqrt(fract(pick * 3.7 + k * 0.13)));
    let outward = east * cos(ang) + north * sin(ang);
    // Taller toward the middle, where the logs meet and the fire is hottest.
    var h = tall * mix(TONGUE_TALL.x, TONGUE_TALL.y, fract(pick * 5.3)) * (1.0 - 0.45 * r / reach);
    h *= breathe * (0.75 + 0.4 * age);
    let w = h * (0.4 + 0.2 * fract(pick * 9.1));
    let logs = wide * TEEPEE.x * max(1.0 - r / (wide * TEEPEE.y), 0.0);
    let base = foot + outward * r + up * (logs * (0.9 + 0.15 * fract(pick * 2.3)) + wide * 0.05 + h * 0.12 * age);
    let lean = normalize(up - outward * TONGUE_LEAN * (r / reach) + fire.wind.xyz * 0.04);
    let world = card(base, lean, corner, w, h, true, 0.0);
    out.clip = position_world_to_clip(world);
    out.uv = corner;
    out.life = vec4(1.005 + 0.99 * age, heat, own, 2.0);
    out.size = h;
    return out;
}

// A hearth's flames (crates/terra_world/src/fire.rs HEARTHS), as a campfire burns in
// footage and in The Witcher 3, Red Dead Redemption 2 and Kingdom Come: its first card the
// ground under it, charred and strewn with embers fallen from its logs, lying flat; then the
// bed of coals glowing among its logs, low and wide; then its body, about as high again over
// its logs as they are, as a few columns of flame round its stack (`HEARTH_COLUMNS`), each at
// its own moment of the flipbook; then ten small tongues licking up the logs (`hearth_tongue`). Its
// columns' tips and torn sides are eaten away sharply (round 12's step back: one opaque cone twice
// the games' height, standing behind the logs as a backdrop, its crown an orange haze).
fn hearth(corner: vec2<f32>, foot: vec3<f32>, up: vec3<f32>, tall: f32, heat: f32, wide_in: f32, seed: f32, phase: f32, wind: vec3<f32>, glow: f32) -> Out {
    var out: Out;
    let wide = select(clamp(tall * 0.5, 0.4, 1.6), wide_in, wide_in > 0.0);
    let j = i32(phase * HEARTH_CARDS + 0.5);
    let luck = fract(seed * 0.618 + f32(j) * 0.379);
    let east = normalize(cross(up, vec3(0.0, 0.0, 1.0)) + vec3(1.0e-4));
    let north = cross(east, up);
    let torch = tall < TORCH_M;
    if j == 0 {
        // The ground: flat, a little over it, inside the ring of stones round the logs.
        if torch {
            return nowhere();
        }
        let r = wide * GROUND_WIDE;
        let c = corner - vec2(0.5);
        let world = foot + up * 0.03 + (east * c.x + north * c.y) * 2.0 * r;
        out.clip = position_world_to_clip(world);
        out.uv = corner;
        out.life = vec4(1.5, heat, luck, 4.0);
        out.size = r;
        return out;
    }
    if j == 1 {
        let w = wide * 1.4;
        let h = max(0.3, tall * 0.32);
        let world = card(foot - up * h * 0.25, up, corner, w, h, true, 0.0);
        out.clip = position_world_to_clip(world);
        out.uv = corner;
        out.life = vec4(1.5, heat, luck, 3.0);
        out.size = h;
        return out;
    }
    // A baked set's frame is a whole fire, its own tongues alternating with dark (books.rs):
    // a hearth's body is a few columns, each a slice of that frame at its own moment, so it has
    // depth from every side; a torch (under TORCH_M) the one flame of its own set.
    if torch && j > 2 {
        return nowhere();
    }
    // Each column rises and falls with the fire's flicker, each by its own share.
    let breathe = 1.0 + (glow - 1.0) * (0.8 + 0.5 * luck);
    if j > 1 + HEARTH_COLUMNS {
        return hearth_tongue(corner, foot, up, east, north, tall, heat, wide, seed, f32(j - 2 - HEARTH_COLUMNS), breathe);
    }
    // A campfire's body stands about as tall as its logs are high, its tallest tips reaching
    // `tall` only now and then (research/2026-10-19-fire-step-back.md, round 12): drawn as
    // tall as `tall`, it was a cone twice the height of RDR2's and Witcher 3's for its logs.
    var flame = tall * breathe * HEARTH_BODY;
    if torch {
        flame = tall * breathe;
    }
    var at = foot + up * select(wide * HEARTH_BODY_UP, 0.0, torch);
    var window = 1.0;
    if !torch {
        // Its columns spread round the stack by the golden angle, the first in the middle and
        // tallest, the others out among the logs and lower, their feet on the logs' tops.
        let k = f32(j - 2);
        let a = seed * 6.283 + k * 2.39996 + luck * 0.6;
        let r = select(wide * COLUMN_OUT * (0.6 + 0.4 * fract(luck * 5.1)), wide * 0.04, j == 2);
        let logs = wide * TEEPEE.x * max(1.0 - r / (wide * TEEPEE.y), 0.0);
        at = foot + (east * cos(a) + north * sin(a)) * r + up * max(wide * HEARTH_BODY_UP, logs * 0.55);
        flame *= select(0.62 + 0.2 * luck, 1.0, j == 2);
        window = COLUMN_WINDOW * select(0.85 + 0.3 * fract(luck * 3.3), 1.15, j == 2);
    }
    // The frame is square, and its set's tips reach `reach` up it, so the card stands as
    // tall as the fire's flames over that: a campfire's 1.3 m tips stand 1.3 m, not the 2.2 m
    // a 3.3 m card drew over a seated troll (fire sheet, stage 3).
    let h = flame / select(fire.reach.x, fire.reach.z, torch);
    let lean = wind * 0.05 * flame;
    let world = card(at - up * h * 0.05 + lean * 0.2, up, corner, h * window, h, true, 0.0);
    out.clip = position_world_to_clip(world);
    out.uv = corner;
    // Its first number past 1 (past 2 a torch's), so it is faded only very near what is
    // behind it: a hearth's flames stand among its logs, and faded over a third of a metre
    // they were a thin tongue over bare sticks (lake film r4).
    // Its last number past 1 by half the share of the frame's width a column shows.
    out.life = vec4(select(1.5, 2.5, torch), heat, luck, 1.0 + select(window, 0.0, torch) * 0.5);
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
    if kind == 5 {
        return breath_sprite(corner, slot, phase);
    }
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
        // Thin as a drop is: at 0.3 to 0.5 m across they read as flat orange tiles round
        // the wall the breath struck (breath r1).
        let streak = clamp(length(along) * 0.06, 0.3, 2.2);
        along = normalize(along);
        let across = normalize(cross(along, to_eye));
        let size = 0.09 + 0.07 * going.w;
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
            // A grid's blaze is one baked fire, a bonfire's width, and one in three is
            // only its embers and smoke: each frame of its set is a whole fire of many
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
        // The tongue set's flames reach only `reach` up its frame (a low wall under smoke), so
        // its cards stand so much taller, square, stretched no more than half again across.
        h /= fire.reach.y;
        w = clamp(w, h, 1.5 * h);
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
    // A hearth's each at its own moment and pace, from anywhere over its coals: born on one
    // beat, evenly spaced (`phase`), they rose as a file of streaks on one slant (fire review,
    // round 2). Most die low and soon, a few ride the plume two or three flames high, as
    // RDR2's and the photographs' do.
    let own = hash(vec2(phase * 17.0 + seed, 3.0));
    let life_s = mix(1.6 + 2.2 * luck, 0.7 + 1.6 * own * own, kept);
    let t = fract(time / life_s + mix(fract(phase) * 1.37, own * 7.13, kept) + seed);
    let seconds = t * life_s;
    let climb = mix(2.5 + 3.5 * luck, 1.0 + 2.6 * own * luck, kept);
    let rise = climb * (1.0 - exp(-seconds * 0.9)) / 0.9;
    let east = normalize(cross(up, vec3(0.0, 0.0, 1.0)) + vec3(1.0e-4));
    let north = cross(east, up);
    // Tossed by the plume's eddies, two octaves of them, the farther the higher it rides.
    let slow = vec2(noise(vec2(luck * 41.0, seconds * 0.8)), noise(vec2(luck * 73.0, seconds * 0.8))) - 0.5;
    let quick = vec2(noise(vec2(own * 59.0, seconds * 3.1)), noise(vec2(own * 97.0, seconds * 3.1))) - 0.5;
    let wander = slow + quick * 0.45 * kept;
    let out_from = (vec2(hash(vec2(luck, 1.0)), hash(vec2(luck, 2.0))) - 0.5) * mix(tall * 0.8, b.z * 0.6, kept);
    let sway = mix(2.0, 0.5 + 0.6 * rise, kept);
    let at = foot + up * (rise + tall * mix(0.3, 0.12, kept)) + east * (out_from.x + wander.x * sway * t) + north * (out_from.y + wander.y * sway * t) + wind * seconds * mix(0.8, 0.4, kept);
    // Stretched along its flight: up and downwind, and as the eddies toss it.
    let flight = normalize(up * (3.0 * exp(-seconds * 0.9) + 0.4) + wind * 0.8 + (east * quick.x + north * quick.y) * 2.0 * kept);
    let to_eye = normalize(view.world_position - at);
    var across = cross(flight, to_eye);
    if dot(across, across) < 1.0e-6 {
        across = cross(up, to_eye);
    }
    across = normalize(across);
    // A hearth's a little bigger than a roof's: at 0.02 to 0.04 m none showed at 1080p from
    // six metres, the film's distance (lake film r5).
    // A hearth's points of one to three pixels at six metres, most small: at 0.035 to 0.06 m
    // they were blobs a few streaks of (fire review, round 2).
    let size = mix(0.035 + 0.03 * luck, 0.014 + 0.016 * own * own, kept);
    // A hearth's short streaks, a roof's long: drawn out five times its sparks were white
    // needles beside the flames (lake film r6's first still).
    let world = at + across * (corner.x - 0.5) * size + flight * (corner.y - 0.5) * size * mix(5.0, 2.0, kept);
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
    if kind == 0 && life.w > 3.5 {
        // A hearth's ground: ash and embers fallen from its logs, glowing under them in spots
        // that brighten as the fire flares, and charred dark out past its stones.
        let p = (uv - vec2(0.5)) * 2.0;
        let d = length(p);
        let char = 1.0 - smoothstep(0.55, 1.0, d);
        let spots = fbm(uv * 16.0 + vec2(luck * 31.0, 0.0));
        let under = 1.0 - smoothstep(0.2, 0.55, d);
        let breath = max(1.0 + 2.0 * (glow - 1.0), 0.2);
        let ember = under * smoothstep(0.45, 0.75, spots) * breath;
        light = soot(1150.0 + 350.0 * ember, ember);
        cover = max(covers(light, ember), CHAR_COVER * char);
    } else if kind == 0 && life.w > 2.5 {
        // A hearth's bed of coals: a low mound among its logs, broken into coals glowing
        // orange with dark ash in the cracks between them, each brightening and dimming slowly
        // as air reaches it, under the flames it feeds. Hotter than 1600 K or so it went past
        // the tonemapper's shoulder at night and read as a pink-white glow, not coals (r11).
        let p = (uv - vec2(0.5, 0.42)) * vec2(2.0, 2.4);
        let bed = 1.0 - smoothstep(0.3, 1.0, length(p));
        let lumps = 0.7 * noise(uv * vec2(14.0, 8.0) + vec2(luck * 17.0, 0.0))
            + 0.3 * noise(uv * vec2(31.0, 17.0) + vec2(0.0, luck * 5.0));
        let coal = smoothstep(0.42, 0.56, lumps);
        let air = 0.7 + 0.3 * noise(uv * 6.0 + vec2(time * 0.35, luck * 3.0));
        // Brighter as the fire flares, by twice its flicker, the brightest coals the most.
        let breath = air * max(1.0 + 2.0 * (glow - 1.0), 0.2);
        let glow = bed * coal * breath;
        let alpha = smoothstep(0.02, 0.3, glow) * soft;
        let kelvin = 1150.0 + COAL_HOT * glow * (0.6 + 0.4 * heat);
        light = soot(kelvin, alpha);
        // The ash between them dark, so the bed reads as lumps, in its lower half only.
        let ash = 0.45 * bed * (1.0 - coal) * (1.0 - smoothstep(0.3, 0.6, uv.y)) * soft;
        cover = max(covers(light, alpha), ash);
    } else if kind == 0 && life.w > 1.5 {
        // A tongue licking up the logs (`hearth_tongue`): a teardrop swaying more toward its
        // tip, its cover a noise rising through it eaten away by a threshold that climbs with
        // height and with the tongue's age, so its tip tears into wisps and pinches off first
        // (alpha erosion, as Unreal's and Unity's fire materials do it); white-hot at its foot
        // over the coals, orange up it, and dimmer and thinner toward its tip.
        let age = t - 1.0;
        let y = uv.y;
        let wob = (noise(vec2(y * 2.5 - time * 2.2, luck * 37.0)) - 0.5) * 0.4 * y;
        let xx = uv.x - 0.5 - wob;
        let w = 0.4 * pow(saturate(1.0 - y), 0.75) * smoothstep(-0.1, 0.25, y);
        let across = abs(xx) / max(w, 1.0e-3);
        let body = 1.0 - smoothstep(0.45, 1.0, across);
        let n = fbm(vec2(xx * 5.0 + luck * 11.0, y * 2.2 - time * 2.8));
        let edge = 0.1 + 0.7 * pow(y, 1.3) + 0.4 * age;
        let shape = smoothstep(edge - 0.05, edge + 0.05, body * (0.45 + 0.8 * n));
        let born = smoothstep(0.0, 0.12, age) * smoothstep(0.0, 0.08, y) * mix(1.0, 0.55, smoothstep(0.35, 0.9, y));
        let heat_here = saturate(0.5 + 0.4 * pow(1.0 - y, 1.6) * (0.6 + 0.4 * (1.0 - across)) - 0.15 * age);
        let drawn = baked_flame(vec4(heat_here, shape * born, 0.0, 0.0), HEARTH_BRIGHTNESS * (0.85 + 0.15 * heat), soft);
        light = drawn.rgb;
        cover = drawn.a;
    } else if kind == 0 {
        // A flame from its kind's baked set (books.rs): a hearth's tongue (its first number
        // past 1, `hearth`) the campfire's, a torch's past 2 its own, a burning building's
        // the tongue's; each card at its own moment by its luck, mirrored for half of them,
        // played at the pace a fire of its size moves from the rate and size it was baked at
        // (a flame's time goes as the root of its size, Froude's scaling).
        var layer = SET_TONGUE;
        var brightness = BLAZE_BRIGHTNESS * mix(0.6, 1.0, heat);
        if t > 2.0 {
            layer = SET_TORCH;
            brightness = TORCH_BRIGHTNESS * (0.85 + 0.15 * heat);
        } else if t > 1.0 {
            layer = SET_CAMPFIRE;
            brightness = HEARTH_BRIGHTNESS * (0.85 + 0.15 * heat);
        }
        let baked = fire.sets[layer];
        // A campfire's column (`hearth`) shows a slice of the frame's width, round a middle
        // of its own, so each is a few tongues of the whole fire.
        let window = (life.w - 1.0) * 2.0;
        let column = layer == SET_CAMPFIRE && window > 0.02;
        var across = uv.x;
        if column {
            across = 0.5 + (fract(luck * 3.7) - 0.5) * 0.4 + (uv.x - 0.5) * window;
        }
        let x = select(across, 1.0 - across, fract(luck * 7.3) > 0.5);
        let rate = baked.x * sqrt(baked.y / max(size, 0.2)) * (0.85 + 0.3 * fract(luck * 17.3));
        let f = fract(time * rate / BOOK_FRAMES + luck * 5.17) * BOOK_FRAMES;
        // Faded out at the frame's foot, sides and top, where a set's smoke leaves the frame:
        // cut off there, a burning building's soot was a grey square behind its flames.
        // The tongue set burns as a low wall with a level, white-hot foot and upright sides
        // (its source a box): standing on a wall's top or a ledge, that foot was a ruler-
        // straight cut through the flame and its sides a pale rectangle (breath r27's review),
        // so its foot and sides are eaten back raggedly, as tongues rising off embers are.
        let rag = noise(vec2(x * 7.0 + luck * 13.0, uv.y * 3.0 - time * 0.9 * rate / baked.x));
        var foot = smoothstep(0.0, 0.03, uv.y);
        var sides = smoothstep(0.0, 0.15, x) * smoothstep(0.0, 0.15, 1.0 - x);
        if layer == SET_TONGUE {
            foot = smoothstep(0.0, 0.16, uv.y + 0.12 * (rag - 0.5));
            sides = smoothstep(0.1, 0.32, min(x, 1.0 - x) + 0.2 * (rag - 0.5) + 0.1 * uv.y);
        }
        if column {
            // Its own sides torn and narrowing up it, so dark shows between the columns.
            let half = 0.5 * (1.0 - 0.45 * uv.y);
            let tear = fbm(vec2(uv.x * 4.0 + luck * 7.0, uv.y * 6.0 - time * 2.0));
            let off = abs(uv.x - 0.5) / half + 0.3 * (rag - 0.5) + 0.5 * (tear - 0.5);
            sides = 1.0 - smoothstep(0.5, 1.0, off);
        }
        let edges = foot * sides * smoothstep(0.0, 0.25, 1.0 - uv.y);
        var texel = flipbook(layer, f, vec2(x, uv.y));
        if layer == SET_CAMPFIRE {
            // A campfire's body torn toward its tips rather than fading to a haze: its cover
            // eaten by a threshold rising with height through a noise climbing up it (alpha
            // erosion); its upper part thinner, so the dark shows through it; its foot over the
            // coals hotter, white-hot, and its top dimmer (round 12's step back).
            let hy = saturate(uv.y / max(fire.reach.x, 0.1));
            let n = fbm(vec2(x * 5.0 + luck * 9.0, hy * 2.6 - time * 1.7));
            // Its cover taken over the set's thin median (0.16), so the threshold eats its
            // thin tips and leaves its body: on the raw cover it ate the whole flame.
            let erode = HEARTH_ERODE_FLOOR + HEARTH_ERODE * smoothstep(0.15, 1.0, hy);
            texel.g *= smoothstep(erode, erode + HEARTH_FEATHER, saturate(texel.g * 3.0) * (0.5 + n) * edges);
            texel.g *= mix(1.0, HEARTH_SEE, smoothstep(0.25, 0.85, hy));
            texel.r = saturate((texel.r + HEARTH_BASE_HOT * (1.0 - smoothstep(0.0, 0.25, hy))) * mix(1.0, 0.85, hy));
        }
        let drawn = baked_flame(texel, brightness, soft * select(edges, 1.0, layer == SET_CAMPFIRE));
        light = drawn.rgb;
        cover = drawn.a;
    } else if kind == 5 {
        // A breath's sprite (`breath_sprite`): its set at its own frame, mirrored for half
        // of them, faded at the frame's edges and by its age (its `glow`); or the blast's
        // flash, a soft ball of white-hot light going out.
        let layer = i32(life.w + 0.5);
        if life.w < -1.5 {
            // The soot stain on a struck face (`breath_stain`): black, thickest just over the
            // hit and thinning up and out, its edge broken by noise and streaked upward as soot
            // climbs a wall; `heat` here is which card (0 the plume, 1 the char spot).
            let n = fbm(uv * vec2(4.0, 2.5) + vec2(luck * 13.0, heat * 7.0));
            // Streaked up the face as soot is laid by flame licking up it: fingers of black
            // running up from the hit, uneven at their tops (one soft blob, breath r27).
            let streak = fbm(vec2(uv.x * 13.0 + luck * 5.0, uv.y * 0.9));
            var stain = 0.0;
            if heat < 0.5 {
                let across = abs(uv.x - 0.5) / (0.2 + 0.3 * uv.y);
                let top = uv.y + 0.4 * (n - 0.5) - 0.7 * (streak - 0.5);
                stain = (1.0 - smoothstep(0.75, 0.95, across + 0.4 * (n - 0.5)))
                    * smoothstep(0.0, 0.12, uv.y) * (1.0 - smoothstep(0.6, 0.85, top))
                    * mix(0.35, 1.0, smoothstep(0.35, 0.65, streak)) * 0.9;
            } else {
                let d = length((uv - 0.5) * vec2(2.0, 2.2));
                stain = (1.0 - smoothstep(0.35, 1.0, d + 0.5 * (n - 0.5))) * 0.75;
            }
            cover = saturate(stain * smoothstep(0.0, 1.0, t)) * soft;
            light = SOOT_BLACK * cover;
        } else if life.w < -0.5 {
            let p = length(uv - 0.5) * 2.0;
            let ball = (1.0 - smoothstep(0.0, 1.0, p)) * (1.0 - t) * (1.0 - t);
            let r = ramp_row(1.0, BREATH_ROW);
            light = r.rgb * ball * heat * on_screen();
            cover = 0.0;
        } else {
            let x = select(uv.x, 1.0 - uv.x, fract(luck * 7.3) > 0.5);
            let edges = smoothstep(0.0, 0.1, uv.y) * smoothstep(0.0, 0.12, x) * smoothstep(0.0, 0.12, 1.0 - x) * smoothstep(0.0, 0.12, 1.0 - uv.y);
            var texel = flipbook(layer, clamp(t, 0.0, BOOK_FRAMES - 1.001), vec2(x, uv.y));
            // A puff in its first frames is the fuel just out of the jaw, hotter than the
            // set's: the white-yellow core a flamethrower's rope shows for its first metres.
            // And where it strikes, from the last of its flight to its first moment on the
            // face: white-yellow at the contact, as every reference is (refs/, breath r28's
            // review). Hot, it is opaque, so the eye sees it and not what is behind it.
            let puff = layer == SET_BREATH_PUFF;
            let young = select(0.0, 1.0 - smoothstep(0.5, 4.0, t), puff);
            let contact = select(0.0, smoothstep(8.5, 11.5, t) * (1.0 - smoothstep(15.0, 20.0, t)), puff);
            let hot = max(young, contact);
            // Every breath set denser than baked, so its flame covers the sky and the smoke
            // behind it: at its baked cover a fireball over a dusk sky was khaki (breath r29,
            // B/R 0.43 against a fire photo's 0.2).
            // Fuel burns rich, so its soot is thick: the baked sets' is a thin smoke (at most
            // 0.2 to 0.6), which drawn as baked never showed (the sheet's soot/flame 0.00).
            // The fireball's and the blast's frames are mostly their smoke, so theirs is
            // thickened less: at the puffs' it hung as a black wall over the hit (breath r10).
            let thick = select(BREATH_SOOT * 0.25, BREATH_SOOT, puff);
            // A puff's soot only once it has burnt a while on the face, above the hit: from
            // the jaw on, the stream was half black smoke (breath r28's review).
            // A fireball's and a blast's only in their last frames, once risen over the hit:
            // opaque from their first, they were black holes in the fireball (r30/a).
            let smoke = (1.0 - exp(-thick * texel.b)) * select(smoothstep(28.0, 48.0, t), smoothstep(16.0, 26.0, t), puff);
            let heat_of = saturate(texel.r * 1.35 * BREATH_HEAT * (1.0 + 0.35 * hot) * min(0.9 + 0.04 * heat, 1.15));
            // Faded before what is behind it by erosion too, so it is never see-through: thinned
            // by alpha near the face it struck, it showed the wall below the wall's top and the
            // sky above it, a level line (r30/b).
            let drawn = breath_eroded(heat_of, texel.g * edges, smoke * edges, glow * soft, luck, uv);
            light = drawn.rgb;
            cover = drawn.a;
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

@if(!PREPASS_PIPELINE)
@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
    var out: FragmentOutput;
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
        // A breath's sprites are metres across: faded over a metre, so a puff against the
        // wall it strikes never shows the line it cuts.
        // A card laid on the face it burns on, a hand's breadth off it, by that (its size,
        // `breath_sprite`).
        // At most a third of a metre: faded over a metre, a puff half a metre before the wall
        // was half gone where the wall stood behind it and whole above the wall's top, a hard
        // level cut through the flame along the wall's top (breath r27's review).
        if kind == 5 {
            reach = 0.08;
        }
        let behind = depth_ndc_to_view_z(in.position.z) - depth_ndc_to_view_z(prepass_depth(in.position, 0u));
        soft = smoothstep(0.0, reach, behind);
        // A breath's flame is faded before the face it strikes by how far it stands off that
        // face's plane, the same past the wall's top as in front of it: faded by the depth
        // behind it, it was whole over the wall and thinned in front of it, a level line along
        // the wall's top (breath r27 to r29). Its jet rides in its size; the beam's in its life.
        if kind == 5 && (in.uv_b.x > -0.5 || in.uv_b.x < -2.5) {
            let jet = select(u32(in.uv_b.y + 0.5), u32(in.world_normal.x - 2.0 + 0.5), in.uv_b.x < -2.5);
            let b = breath_of(jet);
            if b.face > 0.0 {
                let f = face_of(b);
                soft = min(soft, smoothstep(0.0, 0.3, dot(in.world_position.xyz - f.at, f.normal)));
            }
        }
        // A soot stain is only where the face it lies on is (`breath_stain`): past the wall's
        // end or top, where nothing stands a hair behind it, it is gone, as a decal is.
        if kind == 5 && in.uv_b.x < -1.5 && in.uv_b.x > -2.5 {
            soft = smoothstep(-0.02, 0.005, behind) * (1.0 - smoothstep(0.12, 0.3, behind));
        }
    }
    if i32(fire.kind.x + 0.5) == 5 && in.uv_b.x < -2.5 {
        out.color = breath_beam_shade(in.world_position.xyz, in.uv, vec4(in.world_normal, in.uv_b.x), soft);
    } else {
        out.color = shade(in.uv, vec4(in.world_normal, in.uv_b.x), in.uv_b.y, soft, in.world_position.w);
    }
    // A breath's sprite writes its depth (crates/terra_world/src/fire.rs, `FireExtension`'s
    // `depth`), so where it is eroded away or never drawn it is no fragment at all: what is
    // left of a card stands in the depth, not its whole quad.
    if i32(fire.kind.x + 0.5) == 5 && out.color.a < 0.1 && max(out.color.r, max(out.color.g, out.color.b)) < 0.004 {
        discard;
    }
@if(FIRE_MASK)
    {
        // The hearths' mask (crates/terra_world/src/fire/film.rs): where a hearth's or torch's
        // flame (its life's first number past 1, not its bed of coals) gives light, so much of
        // the pixel the tonemapper takes through the film, all of it by a red of 1. Its blend
        // adds this to the alpha and leaves the colour.
        let flame = in.world_normal.x > 1.0 && in.uv_b.x < 2.5;
        out.color = vec4(0.0, 0.0, 0.0, select(0.0, min(out.color.r, 1.0), flame));
    }
    return out;
}
