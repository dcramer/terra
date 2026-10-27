// The Cetri (D534, docs/creatures.md): walking up the lush forest's path in the morning
// toward the oaks beside it, one of them, a 9.5 m oak towering over its neighbours, wakes: its eyes
// light, its trunk splits into a mouth, it lunges at the walker and then comes on in slow,
// heavy steps. The camera is the walker's eye but for two of the walk's steps, filmed from beside
// it (SIDE). Places in the start's metres, east and south.
import type { Script, Vec3 } from "../../qa/script.ts";

const ROLLS = 360;
/// Seconds into the film of each beat.
const BEATS = { stirs: 4.4, wakes: 5.0, lunge: 8.2, comes: 10.6, end: 17.6 };
/// Where it stands: beside the path, a few metres short of the nearest grown oak (16.2, 41.2).
const CETRI: [number, number] = [19.5, 38];
/// How big it stands: about 9.5 m (D534, the developers: "Make it 9-10 m"), its 7 m model
/// stood 1.36 times as big. Heights and lengths on its body below are the 7 m model's,
/// scaled by it where they are used.
const SIZE = 9.5 / 7;
/// The trees felled round the way its feet go: those taller than it within 12 m, looked for
/// every 4 m along it. The two oaks it walks out between are shorter, and stay.
const FELL = { within: 12, taller: 9.5, every: 4 };
/// A point on the path (forest.json's route, (3.7, 4.7) to (16.8, 25)): the way the walker
/// comes from the tree. It stops as the tree wakes 7.5 m short of it, so its face is big
/// enough to read (round-5 film reviews: eyes 10 px at 10 m), west of that line, clear of the
/// oak by the path that hid the lunge.
const WAY: [number, number] = [17.2, 29.2];
/// 8.5 m at 9.5 m tall (7.5 at 7 m, by the root of its size, so the face stays near the frame's top).
const NEAR = 8.5;
/// How far west of the line it stops, how far back the walk starts, and when it stops: still
/// before the eyes light, the last steps straight at the tree, clear of the brush that hid the
/// wake (round-6 film reviews: a leaf clump over the face from 4 to 5.5 s).
const NEAR_WEST = 2.0;
const WALK_M = 4.5;
const STOPS_S = 4.0;
const EYE = 1.7;

/// The ground's height under the path and the oaks, metres up of the start, every 5 m from
/// 5 east and 10 south, from a probe of tiny figures (their spawn logs). Empty: the scene
/// stands the probe instead, and the camera guesses level ground.
const GROUND: number[][] = [
  [-0.2, -0.3, -0.3, -0.2, -0.5, -0.8], // 10
  [-0.2, -0.3, -0.2, -0.1, -0.5, -0.9], // 15
  [-0.2, -0.3, -0.4, -0.5, -0.7, -1.1], // 20
  [-0.3, -0.5, -0.7, -0.9, -1.0, -1.0], // 25
  [-0.3, -0.5, -0.7, -0.9, -1.0, -0.9], // 30
  [-0.5, -0.6, -0.7, -0.7, -0.8, -0.9], // 35
  [-0.5, -0.7, -0.8, -0.8, -0.8, -1.0], // 40
  [-0.5, -0.7, -1.0, -0.9, -1.0, -1.0], // 45
];
const G0: [number, number] = [5, 10];
function ground(east: number, south: number): number {
  if (!GROUND.length) return 0;
  const u = Math.min(GROUND[0].length - 1.001, Math.max(0, (east - G0[0]) / 5));
  const v = Math.min(GROUND.length - 1.001, Math.max(0, (south - G0[1]) / 5));
  const [i, j] = [Math.floor(u), Math.floor(v)];
  const [fu, fv] = [u - i, v - j];
  const row = (r: number[]) => r[i] + (r[i + 1] - r[i]) * fu;
  return row(GROUND[j]) + (row(GROUND[j + 1]) - row(GROUND[j])) * fv;
}
const on = ([e, s]: [number, number], up: number): Vec3 => [e, ground(e, s) + up, s];
const toward = (a: [number, number], b: [number, number], m: number): [number, number] => {
  const l = Math.hypot(b[0] - a[0], b[1] - a[1]);
  return [a[0] + ((b[0] - a[0]) / l) * m, a[1] + ((b[1] - a[1]) / l) * m];
};

/// Asleep: both arms up into its crown, the hands among the branches; the legs pressed
/// together into one trunk; a slight lean, as an old oak stands; the chest held up against the
/// idle clip's hunch (spine_03 19 degrees forward), which tipped the crown's top clumps over the
/// face from below (round-6 film reviews: a leaf tuft over it until the roar). Bends are degrees about the
/// body's left, up and ahead (terra_creatures::staged::pose).
const ASLEEP = {
  upperarm_l: [-8, 0, 150],
  upperarm_r: [-8, 0, -150],
  lowerarm_l: [0, 0, 25],
  lowerarm_r: [0, 0, -25],
  thigh_l: [0, 0, -12],
  thigh_r: [0, 0, 12],
  spine_01: [2, 0, 3],
  spine_03: [-16, 0, 0],
  neck_01: [-6, 0, 0],
};
/// Awake and roaring: the arms brought down and out from the crown, the crown thrown back.
const ROARS = { upperarm_l: [-20, 0, 35], upperarm_r: [-20, 0, -35], spine_03: [-6, 0, 0], neck_01: [-6, 0, 0] };
/// The wind-up before the lunge, slow as a great weight is gathered: the trunk drawn back, the
/// limbs hauled up, out to the sides and back, clear of the face, held a moment before the
/// throw (round-5 film reviews: antlers held over the face; round 6: the wind-up unseen, the
/// arms hanging like rope).
const WINDS = {
  spine_01: [-14, 0, 0],
  spine_02: [-8, 0, 0],
  neck_01: [22, 0, 0],
  upperarm_l: [20, 0, 75],
  upperarm_r: [20, 0, -75],
  lowerarm_l: [0, 0, 30],
  lowerarm_r: [0, 0, -30],
};
/// The lunge: the whole trunk tipped deep toward the walker from the hips and the crown driven
/// on after it, so the face comes down at the lens; one limb thrown out at the lens, the other
/// trailing back and low (round-6 film reviews: both limbs spread level read as a crucifix;
/// round 5: the trunk near upright at the blow). No smash clip: its arms rise over the head
/// for its first 1.8 s. Its face (eyes and mouth on neck_01, the upper trunk) held up at the
/// walker against the tip, as a predator's head stays on its prey: tipped with the trunk it
/// pointed 88 degrees down, the lens 72 degrees off it, and showed a slit (round-7 reviews: the
/// eyes out and the mouth shut at the lunge); now 37 down, the lens 8 off it.
const LUNGES = {
  spine_01: [28, 0, 0],
  spine_02: [14, 0, 0],
  spine_03: [4, 0, 0],
  neck_01: [-34, 0, 0],
  Head: [4, 0, 0],
  upperarm_l: [-75, -15, 20],
  upperarm_r: [35, 10, 25],
};
/// The lunge's follow-through: the trunk carried on past it by its own weight, the crown
/// whipped on after it, the leading limb at full stretch; then it settles back into LUNGES.
const THROWN = {
  ...LUNGES,
  spine_01: [34, 0, 0],
  spine_02: [16, 0, 0],
  spine_03: [6, 0, 0],
  neck_01: [-36, 0, 0],
  Head: [14, 0, 0],
  upperarm_l: [-85, -15, 15],
};
/// Recovered from the throw, still leant at the walker, one limb reaching high and the other
/// low, not held out level (round-6 film reviews: a scarecrow's T from 9.5 to 12 s).
const REACHES = { spine_01: [22, 0, 0], spine_02: [12, 0, 0], spine_03: [4, 0, 0], neck_01: [-16, 0, 0], upperarm_l: [-55, 0, 25], upperarm_r: [-15, 0, -5] };
/// Coming on, leant into its steps, one limb reaching for the walker and the other dragging;
/// the trunk rolled `roll` degrees over the planted foot, the crown lagging the other way
/// (round-5 film reviews: the steps glide); the face bowed `bow` more as it nears, so the lens
/// below stays off the brow (at 3.3 m it was 32 degrees under the face's line); `down` the
/// trunk's extra fold as its weight comes onto the foot, and `nod` the crown's after it; `swing`
/// degrees the left limb forward (the right back), against the stride.
const comes = (roll: number, bow: number, down = 0, nod = 0, swing = 0) => ({
  spine_01: [8 + down, 0, roll],
  spine_02: [4 + down * 0.5, 0, -0.4 * roll],
  neck_01: [bow, 0, -0.5 * roll],
  spine_03: [nod * 0.5, 0, 0],
  Head: [nod, 0, 0],
  upperarm_l: [ARMS.reach_l - swing, 0, -ARMS.lower],
  upperarm_r: [ARMS.reach_r + swing, 0, ARMS.lower],
});
/// Its limbs while it comes on: lowered `lower` degrees from where the clip hangs them (Giant_Walk
/// hangs them 12 from the T; round 12's bend raised them 8-10 back into it, and three reviewers
/// saw a T held through the walk), the left `reach_l` forward toward the walker and the right
/// `reach_r`, each swung `swing` against the stride: at a footfall the limb on that side is
/// back and the other forward, as a walker's arms are at heel strike, half way back over the
/// step's settle.
const ARMS = { lower: 30, reach_l: -25, reach_r: -5, swing: 16 };
/// Its walk (Giant_Walk, keyed on its own rest by keyed_walk.py) as stride.ts --falls measures
/// it on the exported file: the pace the file keeps (PACES), the cycle, and each footfall, the
/// clip's second and where the foot lands, metres to the body's left and ahead of it. Keyed at
/// its own tempo, a 1.5 s step (a person's heavy 0.8 s, scaled by the root of a 2.9 m leg), so
/// on the 7 m model it plays at rate 1 at that pace with its planted feet held; stood SIZE as
/// big it plays at the root of that slower (a 2.9 m leg made 3.9), at SIZE times the pace over
/// that, so the feet still hold (COMES_SPEED). The footfalls are the file's, in clip seconds and
/// model metres; the scene scales them. It starts at
/// `from`, the right foot in its swing, so it lands soon. The footfalls are the film's beats,
/// as a game keys them to a clip's contact frames. Troll_Walk, which this replaces, barely lifted
/// its feet (0.02-0.13 m clear) or sank its hips (0.02 m): rounds 6-9 scored its weight 3.
const WALK = {
  clip: "Giant_Walk",
  pace: 0.87,
  cycle: 3.0,
  rate: 1 / Math.sqrt(SIZE),
  from: 0.7,
  falls: [
    { at: 1.4, left: 0.63, ahead: 0.74 },
    { at: 2.9, left: -0.6, ahead: 0.74 },
  ],
};
/// The weight settling onto a planted foot, over what the clip does (its hips sink and its
/// crown nods after them, keyed_walk.py): how far the trunk folds onto the foot and the crown
/// nods on after it (Head, spine_03 half as far), degrees, as it peaks; the crown's spring
/// back past it, when and over how long, and its settle after. A mass on a spring nods far and
/// snaps back quickly (round 8: a 7-degree nod read as a rigid crown). Round 9 also held the
/// body's own time at 0.3 for 0.16 s on each contact, which read as a stall: the clip's own
/// down pose holds the weight now.
const SETTLE = { down: 3, nod: 9, nod_at: 0.3, nod_ms: 260, rebound: -4, back: 0.62, back_ms: 280, rest_ms: 450 };
/// Each footfall's shake of the walker's eye, as a camera shake on a giant's step: down `drop`
/// metres when it lands `near` metres off (more nearer, less further), a decaying bounce over
/// `frames`, the view pitched with it `pitch` times as far and thrown toward the planting foot's
/// side `jolt` times as far (the script's camera has no roll; a sideways jolt stands in for one).
/// Round 12's 8 cm read as nothing at 12 fps (all three reviewers).
const SHAKE = { drop: 0.13, near: 4.5, bounce: [-1, 0.45, -0.2, 0.05, 0], frames: [3, 4, 4, 5, 6], pitch: 1.6, jolt: 0.7 };
const SHAKE_FRAMES = SHAKE.frames.reduce((a, b) => a + b, 0);
/// How big each footfall's litter burst is: twice round 8's, which read only once in four.
const STEP_DUST = 2.0;
/// Two of the walk's footfalls are filmed from beside it, as a monster's reveal is (the
/// tracking shot of a kaiju or a Colossus walking past): seen from the walker's eye head-on,
/// the 1.6 m stride is depth motion and the 0.56 m lift 8% of the frame, half in the grass
/// (round 15). From `from` to `to` seconds a camera `off` metres from its feet, `bearing`
/// degrees round to its left from straight ahead of it and swinging `swings` more as it goes,
/// `up` metres off the ground, tracks it at its pace, closing `closes` metres and looking
/// `look` metres up its body, so the frame holds it feet to crown and each step's lift and the
/// hips' drop read against the light. On its left the camera looks east, toward the low
/// morning sun, so its legs stand dark against the bright backlit leaves rather than dark bark
/// on dark bark. Nearer side-on, or on its right, the two oaks it walks out between stand in
/// the line (flora's obstacles at (16.0, 35.9) and (20.4, 34.5): round 16's first two takes
/// were a trunk); from 12.4 s at 40-65 degrees the line is clear of every
/// trunk. At 9.5 m tall it stands 10 m off and 2 m up: 12.5 m off and 1.2 m up, the bushes
/// on its left hid all below its hips (round 16); at 52-65 degrees a near trunk crossed the
/// frame's edge (round 17). Its footfalls shake it `shake` times the walker's eye's, thrown
/// sideways as the walker's is, and puff `dust` big, for the distance: at 0.4 neither footfall
/// read at 12 fps (round-17 reviews). The walker's eye has the walk's first step before it and
/// the menace after.
const SIDE = { from: 12.5, to: 15.5, off: 10.0, closes: 1.5, bearing: 40, swings: 25, up: 2.0, look: 4.0, shake: 1.0, dust: 4.5 };
/// How far it lunges and how fast, metres and metres a second.
const LUNGE_M = 2.0 * SIZE;
const LUNGE_SPEED = 4.5;
/// At its size the clip is stood SIZE times as big and played at the root of that slower, as
/// its own step was keyed (by the root of the leg): a 1.75 s step at 1.01 m/s; staged.rs plays
/// a stride at the speed it goes over its pace times the figure's scale (20d474fc).
const COMES_SPEED = WALK.pace * SIZE * WALK.rate;
/// How far the walker flinches back at the blow and backs away while it comes on, metres:
/// less than it gains, so it closes in (round-7 reviews: it barely advanced; round 5: the
/// flinch cancelled the lunge); 4.5 m against its 7.1 m at 1.01 m/s. It scrambles
/// back over `BACKS_S` seconds, `BACKS_EASE` the power of its ease out, while the film watches
/// the walk from the side, and then stands frozen while it closes, 4 m off at the end. It
/// backs `SIDE_M` to its own right as it goes, out from behind the bush left of the frame that
/// hid the swinging leg (round 11, at 12.1 s).
const FLINCH_M = 0.3;
const BACKS_M = 4.5;
const BACKS_S = 3.6;
const BACKS_EASE = 1.6;
const SIDE_M = 1.5;
/// Where its face is over its feet, metres up and toward the walker: standing, lunging, thrown.
const FACE = { up: 3.9, ahead: 0.3 };
const FACE_LUNGES = { up: 3.5, ahead: 2.0 };
const FACE_THROWN = { up: 3.2, ahead: 2.1 };
/// Where the eye looks as it closes in, its mouth: the eyes at 4.6 m up and 1.3 m ahead, kept
/// in the frame's top third as it towers over the walker at 3.5 m.
const FACE_NEAR = { up: 4.2, ahead: 1.0 };

function script(s: Script) {
  const f = (t: number) => ROLLS + Math.round(t * 60);
  // Along the walker's line from the tree, and across it to the west.
  const d = toward([0, 0], [WAY[0] - CETRI[0], WAY[1] - CETRI[1]], 1);
  const west: [number, number] = d[1] < 0 ? [d[1], -d[0]] : [-d[1], d[0]];
  const at = (o: [number, number], ahead: number, side = 0): [number, number] => [o[0] + d[0] * ahead + west[0] * side, o[1] + d[1] * ahead + west[1] * side];
  const stop = at(CETRI, NEAR, NEAR_WEST);
  // Where its face is, seen from an eye.
  const face = (feet: [number, number], eye: [number, number], h = FACE): Vec3 => on(toward(feet, eye, h.ahead * SIZE), h.up * SIZE);
  s.frame = 30;
  if (!GROUND.length) {
    for (let j = 0; j < 8; j++)
      for (let i = 0; i < 6; i++)
        s.call("staged.spawn", {
          name: `probe_${i}_${j}`,
          model: "models/nature/forest/mossy_rock_b_6.glb",
          at: [G0[0] + 5 * i, 0, G0[1] + 5 * j],
          facing: [G0[0] + 5 * i + 1, 0, G0[1] + 5 * j],
          scale: 0.05,
          base: true,
        });
  }
  s.call("staged.spawn", { name: "cetri", model: "models/characters/cetri.glb", bend: ASLEEP, at: [CETRI[0], 0, CETRI[1]], facing: [stop[0], 0, stop[1]], scale: SIZE, turn_deg_s: 25, base: true });
  // No lantern by the path: its flame stood as an orange stake by the walking foot in the side
  // shot (round-17 reviews), and the eyes' own glow lights the face.
  s.frame = 40;
  // Asleep it barely stirs: its idle slowed until only the wind would move it.
  s.call("staged.play", { name: "cetri", clip: "Troll_Idle", speed: 0.04, blend_ms: 0 });

  // The walk up the path, looking at the oaks ahead; it slows and stops as the tree wakes.
  s.frame = 60;
  const start = at(CETRI, NEAR + WALK_M, NEAR_WEST);
  s.cut(on(start, EYE), on(CETRI, 4.2 * SIZE));
  s.frame = ROLLS;
  s.glide(on(stop, EYE), face(CETRI, stop), Math.round(STOPS_S * 60));

  // Its limbs first stir out of the crown, drawing the hands' leaves off the face (round-6
  // film reviews: a leaf clump over the face until 5.5 s); the eyes stay dark until it wakes
  // (round-17 reviews: lit at 5.6 s, they gave the sleeping tree away).
  s.frame = f(BEATS.stirs);
  s.call("staged.bend", { name: "cetri", bend: ROARS, blend_ms: 2200 });
  s.frame = f(BEATS.wakes);
  s.call("staged.glow", { name: "cetri", strength: 2.5, blend_ms: 900 });
  s.call("staged.play", { name: "cetri", clip: "Troll_Roar", speed: 0.6, once: true, blend_ms: 1200 });
  s.call("staged.morph", { name: "cetri", weights: { Wake: 1 }, blend_ms: 1400 });

  // The lunge: a slow wind-up, then the whole trunk thrown at the walker as it lurches a step
  // in, carried past by its weight, the crown whipping after it; the walker flinches a short
  // step back only once it lands, the eye kept on the face throughout.
  s.frame = f(BEATS.lunge - 1.1);
  s.call("staged.bend", { name: "cetri", bend: WINDS, blend_ms: 750 });
  // The eyes flare as it gathers itself and stay hot through the lunge (round 7: out at its peak).
  s.call("staged.glow", { name: "cetri", strength: 3.4, blend_ms: 900 });
  s.glide(on(stop, EYE), on(CETRI, (FACE.up + 0.3) * SIZE), 60);
  const lunged = at(CETRI, LUNGE_M, 0.2);
  s.frame = f(BEATS.lunge);
  s.call("staged.bend", { name: "cetri", bend: LUNGES, blend_ms: 420 });
  s.call("staged.move", { name: "cetri", to: [lunged[0], 0, lunged[1]], speed: LUNGE_SPEED, face: [stop[0], 0, stop[1]], base: true });
  s.glide(on(stop, EYE), face(lunged, stop, FACE_LUNGES), 25);
  const lands = f(BEATS.lunge) + 26;
  const flinched = at(stop, FLINCH_M);
  s.frame = lands;
  s.call("staged.bend", { name: "cetri", bend: THROWN, blend_ms: 320 });
  s.call("staged.impact", { at: on(at(lunged, 0.7), 0.05), size: 1.0, base: true });
  s.glide(on(flinched, EYE - 0.12), face(lunged, flinched, FACE_THROWN), 12);
  s.frame = lands + 12;
  s.glide(on(flinched, EYE - 0.05), face(lunged, flinched, FACE_THROWN), 24);
  s.frame = f(BEATS.lunge + 1.0);
  s.call("staged.bend", { name: "cetri", bend: REACHES, blend_ms: 1100 });
  s.glide(on(flinched, EYE - 0.05), face(lunged, flinched, FACE_LUNGES), 66);

  // Then it comes on, slow and heavy, gaining on the walker backing away looking up at it.
  // Each beat is keyed to its feet: a footfall puffs the litter where that foot lands, folds
  // the trunk onto it and holds it there a moment, nods the crown after it and shakes the eye.
  s.frame = f(BEATS.comes);
  const away = at(flinched, BACKS_M, SIDE_M);
  const way = toward([0, 0], [away[0] - lunged[0], away[1] - lunged[1]], 1);
  const left: [number, number] = [way[1], -way[0]];
  // The footfalls in the film's time.
  const falls: { t: number; left: number; ahead: number }[] = [];
  for (let n = 0; n < 16; n++) {
    const fall = WALK.falls[n % 2];
    const clip = fall.at + Math.floor(n / 2) * WALK.cycle;
    if (clip < WALK.from) continue;
    const t = BEATS.comes + (clip - WALK.from) / WALK.rate;
    if (t > BEATS.end - 0.25) break;
    falls.push({ t, left: fall.left, ahead: fall.ahead });
  }
  // Straight at where the walker backs to, gaining on it.
  const feetAt = (t: number) => toward(lunged, away, COMES_SPEED * Math.max(0, t - BEATS.comes));
  const to = feetAt(BEATS.end);
  const u = (t: number) => Math.min(1, Math.max(0, (t - BEATS.comes) / (BEATS.end - BEATS.comes)));
  const bow = (t: number) => 4 + 16 * u(t);
  // No tree taller than it near the way its feet go, so it towers (round-17 reviews: conifers
  // over it): felled every few metres from where it sleeps to where it ends, before the start.
  const comesAt = s.frame;
  s.frame = 30;
  [[CETRI, lunged], [lunged, to]].forEach(([a, b]) => {
    const long = Math.hypot(b[0] - a[0], b[1] - a[1]);
    for (let m = 0; m <= long; m += FELL.every) s.call("world.fell", { at: toward(a, b, m), within: FELL.within, taller_than: FELL.taller });
  });
  s.call("world.fell", { at: to, within: FELL.within, taller_than: FELL.taller });
  s.frame = comesAt;
  s.call("staged.bend", { name: "cetri", bend: comes(0, bow(BEATS.comes)), blend_ms: 900 });
  s.call("staged.play", { name: "cetri", clip: WALK.clip, speed: WALK.rate, from_s: WALK.from, blend_ms: 900 });
  s.call("staged.move", { name: "cetri", to: [to[0], 0, to[1]], speed: COMES_SPEED, face: [away[0], 0, away[1]], base: true });
  const mix = (a: Vec3, b: Vec3, t: number): Vec3 => [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];
  const lift = (a: Vec3, m: number): Vec3 => [a[0], a[1] + m, a[2]];
  const [eye0, eye1] = [on(flinched, EYE - 0.05), on(away, EYE - 0.1)];
  const backs = (t: number) => Math.min(1, Math.max(0, (t - BEATS.comes) / BACKS_S));
  const eyeAt = (t: number) => mix(eye0, eye1, 1 - (1 - backs(t)) ** BACKS_EASE);
  // Back at the walker's eye, the look climbs from its mouth to its face as it closes.
  const level = (t: number) => {
    const k = Math.min(1, Math.max(0, (t - SIDE.to) / 2.0));
    return { up: 3.3 + (FACE_NEAR.up - 3.3) * k, ahead: 0.3 + (FACE_NEAR.ahead - 0.3) * k };
  };
  const look = (t: number): Vec3 => {
    const e = eyeAt(t);
    return face(feetAt(t), [e[0], e[2]], level(t));
  };
  // Where each foot lands, and how far a footfall drops an eye standing there.
  const footOf = (fall: { t: number; left: number; ahead: number }): [number, number] => {
    const feet = feetAt(fall.t);
    const [ahead, side] = [fall.ahead * SIZE, fall.left * SIZE];
    return [feet[0] + way[0] * ahead + left[0] * side, feet[1] + way[1] * ahead + left[1] * side];
  };
  const dropAt = (fall: { t: number; left: number; ahead: number }, e: Vec3) => {
    const foot = footOf(fall);
    return SHAKE.drop * Math.min(1.6, Math.max(0.7, SHAKE.near / Math.hypot(foot[0] - e[0], foot[1] - e[2])));
  };
  // Each footfall: a puff of litter where the foot lands, the trunk folded onto it, the crown
  // nodding on after the trunk has stopped, snapping back past it, then settling.
  falls.forEach((fall) => {
    // Rolled over the planted foot: a turn about ahead leans the trunk to the body's right.
    const roll = fall.left > 0 ? -4 : 4;
    // The limb on the planting side back, the other forward.
    const swing = fall.left > 0 ? -ARMS.swing : ARMS.swing;
    s.frame = f(fall.t);
    s.call("staged.impact", { at: on(footOf(fall), 0.05), size: fall.t > SIDE.from && fall.t < SIDE.to ? SIDE.dust : STEP_DUST, base: true });
    s.call("staged.bend", { name: "cetri", bend: comes(roll, bow(fall.t), SETTLE.down, 0, swing), blend_ms: 160 });
    s.frame = f(fall.t + SETTLE.nod_at);
    s.call("staged.bend", { name: "cetri", bend: comes(roll, bow(fall.t), SETTLE.down, SETTLE.nod, swing), blend_ms: SETTLE.nod_ms });
    s.frame = f(fall.t + SETTLE.back);
    s.call("staged.bend", { name: "cetri", bend: comes(roll * 0.5, bow(fall.t + 0.6), 0, SETTLE.rebound, swing * 0.8), blend_ms: SETTLE.back_ms });
    s.frame = f(fall.t + SETTLE.back + SETTLE.back_ms / 1000);
    s.call("staged.bend", { name: "cetri", bend: comes(roll * 0.5, bow(fall.t + 1), 0, 0, -swing * 0.5), blend_ms: SETTLE.rest_ms });
  });
  // The tracking shot, set frame by frame: round to its left, keeping pace, shaken by each
  // footfall's bounce, `t` seconds into it a share of its drop.
  const bounce = (t: number) => {
    let [from, was] = [0, 0];
    for (let i = 0; i < SHAKE.bounce.length; i++) {
      const next = from + SHAKE.frames[i] / 60;
      if (t < next) return was + ((SHAKE.bounce[i] - was) * (t - from)) / (next - from);
      [from, was] = [next, SHAKE.bounce[i]];
    }
    return 0;
  };
  for (let n = f(SIDE.from); n < f(SIDE.to); n++) {
    const t = (n - ROLLS) / 60;
    const k = (t - SIDE.from) / (SIDE.to - SIDE.from);
    const feet = feetAt(t);
    const a = ((SIDE.bearing + SIDE.swings * k) * Math.PI) / 180;
    const round: [number, number] = [way[0] * Math.cos(a) + left[0] * Math.sin(a), way[1] * Math.cos(a) + left[1] * Math.sin(a)];
    const off = SIDE.off - SIDE.closes * k;
    const eye = on([feet[0] + round[0] * off, feet[1] + round[1] * off], SIDE.up);
    const shake = falls.reduce((sum, fall) => sum + (t >= fall.t ? bounce(t - fall.t) * dropAt(fall, eye) * SIDE.shake : 0), 0);
    // Thrown sideways across the view toward the planting foot, standing in for a roll.
    const jolt = falls.reduce((sum, fall) => sum - (t >= fall.t ? bounce(t - fall.t) * dropAt(fall, eye) * SIDE.shake * SHAKE.jolt * Math.sign(fall.left) : 0), 0);
    const aim = on([feet[0] + way[0] * 0.8 + round[1] * jolt, feet[1] + way[1] * 0.8 - round[0] * jolt], SIDE.look + shake * SHAKE.pitch);
    s.frame = n;
    s.cut(lift(eye, shake), aim);
  }
  // The walker's eye, from `a` to `b` seconds, its first frame cut to if `cuts`: looking up at
  // it as it closes, each footfall shaking the eye, down and thrown toward the planting foot's
  // side, more the nearer it lands.
  const walker = (a: number, b: number, cuts: boolean) => {
    s.frame = f(a);
    if (cuts) s.cut(eyeAt(a), look(a));
    let from = f(a);
    for (const fall of falls.filter((fall) => fall.t > a && fall.t < b)) {
      s.frame = from;
      s.glide(eyeAt(fall.t), look(fall.t), Math.max(1, f(fall.t) - from));
      const drop = dropAt(fall, eyeAt(fall.t));
      s.frame = f(fall.t);
      let t = fall.t;
      SHAKE.bounce.forEach((bounced, i) => {
        t += SHAKE.frames[i] / 60;
        const thrown = lift(look(t), bounced * drop * SHAKE.pitch);
        const jolt = -bounced * drop * SHAKE.jolt * Math.sign(fall.left);
        s.glide(lift(eyeAt(t), bounced * drop), [thrown[0] + left[0] * jolt, thrown[1], thrown[2] + left[1] * jolt], SHAKE.frames[i]);
        s.frame += SHAKE.frames[i];
      });
      from = s.frame;
    }
    s.frame = from;
    s.glide(eyeAt(b), look(b), Math.max(1, f(b) - s.frame));
  };
  // Its first step seen as the walker scrambles back; then from beside it; then back to the
  // walker, frozen, as it closes to the end.
  walker(BEATS.comes, SIDE.from, false);
  walker(SIDE.to, BEATS.end, true);
  s.frame = f(BEATS.end);
}

export default {
  about: "the lush forest's path in the morning: one of the oaks beside it wakes, its eyes lighting and its trunk splitting into a mouth, lunges at the walker and comes on in slow, heavy steps",
  sandbox: [],
  rolls: ROLLS,
  script,
};
