// The Cetri (D534, docs/creatures.md): walking up the lush forest's path in the morning
// toward the oaks beside it, one of them, a 7 m oak like its neighbours, wakes: its eyes
// light, its trunk splits into a mouth, it lunges at the walker and then comes on in slow,
// heavy steps. The camera is the walker's eye. Places in the start's metres, east and south.
import type { Script, Vec3 } from "../../qa/script.ts";

const ROLLS = 360;
/// Seconds into the film of each beat.
const BEATS = { glow: 4.2, wakes: 5.0, lunge: 8.2, comes: 10.6, end: 16.5 };
/// Where it stands: beside the path, a few metres short of the nearest grown oak (16.2, 41.2).
const CETRI: [number, number] = [19.5, 38];
/// A point on the path (forest.json's route, (3.7, 4.7) to (16.8, 25)): the way the walker
/// comes from the tree. It stops as the tree wakes 7.5 m short of it, so its face is big
/// enough to read (round-5 film reviews: eyes 10 px at 10 m), west of that line, clear of the
/// oak by the path that hid the lunge.
const WAY: [number, number] = [17.2, 29.2];
const NEAR = 7.5;
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
/// trunk's extra fold as its weight comes onto the foot, and `nod` the crown's after it.
const comes = (roll: number, bow: number, down = 0, nod = 0) => ({
  spine_01: [8 + down, 0, roll],
  spine_02: [4 + down * 0.5, 0, -0.4 * roll],
  neck_01: [bow, 0, -0.5 * roll],
  Head: [nod, 0, 0],
  upperarm_l: [-45, 0, 10],
  upperarm_r: [10, 0, -8],
});
/// Its walk (Troll_Walk) as measured on its file (research/2026-10-26-giant-walker-weight.md): the pace
/// the file keeps, the cycle, and each footfall, the clip's second and where the foot lands,
/// metres to the body's left and ahead of it. It plays at the slowest a stride clip is played
/// (terra_walker::pace, 0.5) with the body going at that pace, so its planted feet hold and a
/// step takes 1.4 to 1.65 s, a 2.9 m leg's slow walk (a person's 0.8 s, scaled by the root of
/// the leg's length); it starts at `from`, the right foot swinging, so it lands soon. The
/// footfalls are the film's beats, as a game keys them to a clip's contact frames (round 7
/// struck them every 1.0 s on a 1.54 s step, out of step with the feet).
const WALK = {
  pace: 2.254,
  cycle: 1.542,
  rate: 0.5,
  from: 0.5,
  falls: [
    { at: 0.88, left: 0.65, ahead: 0.87 },
    { at: 1.59, left: -0.54, ahead: 0.62 },
  ],
};
/// The weight settling onto a planted foot: the body's own time held at `rate` for `s`
/// seconds from `after` past the contact, at the bottom of its stride (the clip's lowest is
/// 0.1-0.18 s on), as a heavy walk sits in its down pose; how far the trunk folds onto the
/// foot and the crown nods after it, degrees, the crown's spring back past it, and when.
const SETTLE = { after: 0.12, s: 0.16, rate: 0.3, down: 3, nod: 7, rebound: -2.5, back: 0.5 };
/// Each footfall's shake of the walker's eye, as a camera shake on a giant's step: down `drop`
/// metres when it lands `near` metres off (more nearer, less further), a decaying bounce over
/// `frames`, the view pitched with it `pitch` times as far.
const SHAKE = { drop: 0.08, near: 4.5, bounce: [-1, 0.45, -0.2, 0.05, 0], frames: [3, 4, 4, 5, 6], pitch: 1.6 };
const SHAKE_FRAMES = SHAKE.frames.reduce((a, b) => a + b, 0);
/// How big each footfall's litter burst is.
const STEP_DUST = 1.0;
/// The first footfall is watched: the eye pitched down `dip` metres at the face (about 7
/// degrees, the face kept in the frame's top third; round 7 looked at the roots and lost it),
/// from `early` seconds before, its shake and burst larger; `stays` seconds down after it.
const WATCHED = { dip: 0.65, early: 0.35, stays: 0.5, shake: 1.4, dust: 1.25 };
/// How far it lunges and how fast, metres and metres a second.
const LUNGE_M = 2.0;
const LUNGE_SPEED = 4.5;
const COMES_SPEED = WALK.pace * WALK.rate;
/// How far the walker flinches back at the blow and backs away while it comes on, metres:
/// less than it gains, so it closes in, the face from 5 m to 3.3 m off between 12 and 16.5 s
/// (round-7 reviews: it barely advanced; round 5: the flinch cancelled the lunge).
const FLINCH_M = 0.3;
const BACKS_M = 3.4;
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
  const face = (feet: [number, number], eye: [number, number], h = FACE): Vec3 => on(toward(feet, eye, h.ahead), h.up);
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
  s.call("staged.spawn", { name: "cetri", model: "models/characters/cetri.glb", bend: ASLEEP, at: [CETRI[0], 0, CETRI[1]], facing: [stop[0], 0, stop[1]], turn_deg_s: 25, base: true });
  // A lantern left burning in its roots by the path, as a traveller sets one down: a faint warm
  // light at its foot on its shaded side (round-5 film reviews: the face unread in the shade).
  const lantern = at(CETRI, 1.2, -1.0);
  s.call("staged.spawn", { name: "lantern", model: "models/props/Lantern.glb", at: [lantern[0], 0, lantern[1]], facing: [stop[0], 0, stop[1]], base: true, burns: [0.14, 0.06, 0.9], burns_up: 0.2 });
  s.frame = 40;
  // Asleep it barely stirs: its idle slowed until only the wind would move it.
  s.call("staged.play", { name: "cetri", clip: "Troll_Idle", speed: 0.04, blend_ms: 0 });

  // The walk up the path, looking at the oaks ahead; it slows and stops as the tree wakes.
  s.frame = 60;
  const start = at(CETRI, NEAR + WALK_M, NEAR_WEST);
  s.cut(on(start, EYE), on(CETRI, 4.2));
  s.frame = ROLLS;
  s.glide(on(stop, EYE), face(CETRI, stop), Math.round(STOPS_S * 60));

  s.frame = f(BEATS.glow);
  s.call("staged.glow", { name: "cetri", strength: 2.5, blend_ms: 1600 });
  // Its limbs first stir out of the crown as the eyes light, drawing the hands' leaves off
  // the face (round-6 film reviews: a leaf clump over the face until 5.5 s).
  s.frame = f(BEATS.glow + 0.2);
  s.call("staged.bend", { name: "cetri", bend: ROARS, blend_ms: 2200 });
  s.frame = f(BEATS.wakes);
  s.call("staged.play", { name: "cetri", clip: "Troll_Roar", speed: 0.6, once: true, blend_ms: 1200 });
  s.call("staged.morph", { name: "cetri", weights: { Wake: 1 }, blend_ms: 1400 });

  // The lunge: a slow wind-up, then the whole trunk thrown at the walker as it lurches a step
  // in, carried past by its weight, the crown whipping after it; the walker flinches a short
  // step back only once it lands, the eye kept on the face throughout.
  s.frame = f(BEATS.lunge - 1.1);
  s.call("staged.bend", { name: "cetri", bend: WINDS, blend_ms: 750 });
  // The eyes flare as it gathers itself and stay hot through the lunge (round 7: out at its peak).
  s.call("staged.glow", { name: "cetri", strength: 3.4, blend_ms: 900 });
  s.glide(on(stop, EYE), on(CETRI, FACE.up + 0.3), 60);
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
  const away = at(flinched, BACKS_M);
  const way = toward([0, 0], [away[0] - lunged[0], away[1] - lunged[1]], 1);
  const left: [number, number] = [way[1], -way[0]];
  // The footfalls in the film's time; each settle's hold delays the clip and the going after
  // it, `lost(t)` seconds by a time.
  const falls: { t: number; left: number; ahead: number }[] = [];
  const lost = (t: number) => falls.reduce((sum, fall) => sum + Math.min(SETTLE.s, Math.max(0, t - fall.t - SETTLE.after)) * (1 - SETTLE.rate), 0);
  for (let n = 0; n < 16; n++) {
    const fall = WALK.falls[n % 2];
    const clip = fall.at + Math.floor(n / 2) * WALK.cycle;
    if (clip < WALK.from) continue;
    const t = BEATS.comes + (clip - WALK.from) / WALK.rate + lost(Infinity);
    if (t > BEATS.end - 0.25) break;
    falls.push({ t, left: fall.left, ahead: fall.ahead });
  }
  // Straight at where the walker backs to, gaining on it.
  const feetAt = (t: number) => toward(lunged, away, COMES_SPEED * Math.max(0, t - BEATS.comes - lost(t)));
  const to = feetAt(BEATS.end);
  const u = (t: number) => Math.min(1, Math.max(0, (t - BEATS.comes) / (BEATS.end - BEATS.comes)));
  const bow = (t: number) => 4 + 16 * u(t);
  s.call("staged.bend", { name: "cetri", bend: comes(0, bow(BEATS.comes)), blend_ms: 900 });
  s.call("staged.play", { name: "cetri", clip: "Troll_Walk", speed: WALK.rate, from_s: WALK.from, blend_ms: 900 });
  s.call("staged.move", { name: "cetri", to: [to[0], 0, to[1]], speed: COMES_SPEED, face: [away[0], 0, away[1]], base: true });
  const mix = (a: Vec3, b: Vec3, t: number): Vec3 => [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];
  const lift = (a: Vec3, m: number): Vec3 => [a[0], a[1] + m, a[2]];
  const [eye0, eye1] = [on(flinched, EYE - 0.05), on(away, EYE - 0.1)];
  const eyeAt = (t: number) => mix(eye0, eye1, u(t));
  // The first steps watched a little lower, the face still in frame, then up at it as it closes.
  const level = (t: number) => {
    const k = Math.min(1, Math.max(0, (t - BEATS.comes - 1.5) / 3.5));
    return { up: 3.3 + (FACE_NEAR.up - 3.3) * k, ahead: 0.3 + (FACE_NEAR.ahead - 0.3) * k };
  };
  const look = (t: number, dip = 0): Vec3 => {
    const e = eyeAt(t);
    return lift(face(feetAt(t), [e[0], e[2]], level(t)), -dip);
  };
  falls.forEach((fall, k) => {
    const watched = k === 0;
    const dip = watched ? WATCHED.dip : 0;
    // On to this footfall from the last's shake (or the walk's start).
    const from = k ? f(falls[k - 1].t) + SHAKE_FRAMES + (k === 1 ? Math.round(WATCHED.stays * 60) : 0) : f(BEATS.comes);
    const starts = watched ? f(fall.t - WATCHED.early) : from;
    if (watched && starts > from) {
      s.frame = from;
      s.glide(eyeAt(fall.t - WATCHED.early), look(fall.t - WATCHED.early), starts - from);
    }
    s.frame = starts;
    s.glide(eyeAt(fall.t), look(fall.t, dip), Math.max(1, f(fall.t) - starts));
    const feet = feetAt(fall.t);
    const foot: [number, number] = [feet[0] + way[0] * fall.ahead + left[0] * fall.left, feet[1] + way[1] * fall.ahead + left[1] * fall.left];
    // Rolled over the planted foot: a turn about ahead leans the trunk to the body's right.
    const roll = fall.left > 0 ? -4 : 4;
    s.frame = f(fall.t);
    s.call("staged.impact", { at: on(foot, 0.05), size: watched ? WATCHED.dust : STEP_DUST, base: true });
    s.call("staged.bend", { name: "cetri", bend: comes(roll, bow(fall.t), SETTLE.down), blend_ms: 160 });
    s.frame = f(fall.t + SETTLE.after);
    s.call("staged.rate", { name: "cetri", rate: SETTLE.rate });
    s.frame = f(fall.t + SETTLE.after + SETTLE.s);
    s.call("staged.rate", { name: "cetri", rate: 1 });
    // The crown nods on after the trunk has stopped, then springs back past it.
    s.frame = f(fall.t + 0.18);
    s.call("staged.bend", { name: "cetri", bend: comes(roll, bow(fall.t), SETTLE.down, SETTLE.nod), blend_ms: 240 });
    s.frame = f(fall.t + SETTLE.back);
    s.call("staged.bend", { name: "cetri", bend: comes(roll * 0.5, bow(fall.t + 1), 0, SETTLE.rebound), blend_ms: 650 });
    // The walker's eye shaken by it, more the nearer it lands.
    const e = eyeAt(fall.t);
    const near = Math.hypot(foot[0] - e[0], foot[1] - e[2]);
    const drop = SHAKE.drop * Math.min(1.6, Math.max(0.7, SHAKE.near / near)) * (watched ? WATCHED.shake : 1);
    s.frame = f(fall.t);
    let t = fall.t;
    SHAKE.bounce.forEach((b, i) => {
      t += SHAKE.frames[i] / 60;
      s.glide(lift(eyeAt(t), b * drop), lift(look(t, dip), b * drop * SHAKE.pitch), SHAKE.frames[i]);
      s.frame += SHAKE.frames[i];
    });
    // The watched one stays down a moment on the landing before the eye climbs on.
    if (watched) {
      const up = fall.t + SHAKE_FRAMES / 60 + WATCHED.stays;
      s.glide(eyeAt(up), look(up, dip * 0.5), Math.round(WATCHED.stays * 60));
    }
  });
  // Up at the face to the end.
  s.frame = f(falls[falls.length - 1].t) + SHAKE_FRAMES;
  s.glide(eyeAt(BEATS.end), look(BEATS.end), Math.max(1, f(BEATS.end) - s.frame));
  s.frame = f(BEATS.end);
}

export default {
  about: "the lush forest's path in the morning: one of the oaks beside it wakes, its eyes lighting and its trunk splitting into a mouth, lunges at the walker and comes on in slow, heavy steps",
  sandbox: [],
  rolls: ROLLS,
  script,
};
