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
/// together into one trunk; a slight lean, as an old oak stands. Bends are degrees about the
/// body's left, up and ahead (terra_creatures::staged::pose).
const ASLEEP = {
  upperarm_l: [-8, 0, 150],
  upperarm_r: [-8, 0, -150],
  lowerarm_l: [0, 0, 25],
  lowerarm_r: [0, 0, -25],
  thigh_l: [0, 0, -12],
  thigh_r: [0, 0, 12],
  spine_01: [2, 0, 3],
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
  neck_01: [8, 0, 0],
  upperarm_l: [20, 0, 75],
  upperarm_r: [20, 0, -75],
  lowerarm_l: [0, 0, 30],
  lowerarm_r: [0, 0, -30],
};
/// The lunge: the whole trunk tipped deep toward the walker from the hips and the crown driven
/// on after it, so the face comes down at the lens; one limb thrown out at the lens, the other
/// trailing back and low (round-6 film reviews: both limbs spread level read as a crucifix;
/// round 5: the trunk near upright at the blow). No smash clip: its arms rise over the head
/// for its first 1.8 s.
const LUNGES = {
  spine_01: [32, 0, 0],
  spine_02: [18, 0, 0],
  spine_03: [10, 0, 0],
  neck_01: [6, 0, 0],
  Head: [4, 0, 0],
  upperarm_l: [-75, -15, 20],
  upperarm_r: [35, 10, 25],
};
/// The lunge's follow-through: the trunk carried on past it by its own weight, the crown
/// whipped on after it, the leading limb at full stretch; then it settles back into LUNGES.
const THROWN = {
  ...LUNGES,
  spine_01: [38, 0, 0],
  spine_02: [20, 0, 0],
  spine_03: [12, 0, 0],
  neck_01: [12, 0, 0],
  Head: [10, 0, 0],
  upperarm_l: [-85, -15, 15],
};
/// Recovered from the throw, still leant at the walker, one limb reaching high and the other
/// low, not held out level (round-6 film reviews: a scarecrow's T from 9.5 to 12 s).
const REACHES = { spine_01: [24, 0, 0], spine_02: [14, 0, 0], spine_03: [8, 0, 0], neck_01: [-4, 0, 0], upperarm_l: [-55, 0, 25], upperarm_r: [-15, 0, -5] };
/// Coming on, leant into its steps, one limb reaching for the walker and the other dragging;
/// each footfall rocks the trunk over the planted side, the crown lagging the other way
/// (round-5 film reviews: the steps glide).
const comes = (side: number) => ({
  spine_01: [8, 0, 4 * side],
  spine_02: [4, 0, -1.5 * side],
  neck_01: [0, 0, -2 * side],
  upperarm_l: [-45, 0, 10],
  upperarm_r: [10, 0, -8],
});
/// Seconds between the footfalls of its slowed walk, each felt as a jolt in the walker's eye.
const STEP_S = 1.0;
/// How big each footfall's litter burst is, and the one the walker's eye drops to watch land
/// (round-6 film reviews: 0.45 never read, the feet never in frame, so the steps weigh nothing).
const STEP_DUST = 0.9;
const WATCHED_DUST = 1.3;
/// The footfall watched: its number, how long before it the eye drops to the root, how high
/// it looks there, metres, and how long it stays down after the landing, seconds.
const WATCHED = { step: 0, early: 0.3, up: 1.3, stays: 0.45 };
/// How far it lunges and how fast, and how fast it then comes on, metres and metres a second.
const LUNGE_M = 2.0;
const LUNGE_SPEED = 4.5;
const COMES_SPEED = 0.8;
/// How far the walker flinches back at the blow and backs away while it comes on, metres:
/// less than it gains, so it closes in (round-5 film reviews: the flinch cancelled the lunge).
const FLINCH_M = 0.3;
const BACKS_M = 2.0;
/// Where its face is over its feet, metres up and toward the walker: standing, lunging, thrown.
const FACE = { up: 3.9, ahead: 0.3 };
const FACE_LUNGES = { up: 3.5, ahead: 1.1 };
const FACE_THROWN = { up: 3.3, ahead: 1.4 };

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

  // Then it comes on, slow and heavy, gaining on the walker backing away looking up at it;
  // each footfall rocks its trunk, puffs the litter at its foot and jolts the walker's eye.
  s.frame = f(BEATS.comes);
  const away = at(flinched, BACKS_M);
  // Straight at where the walker backs to, gaining on it.
  const feetAt = (t: number) => toward(lunged, away, COMES_SPEED * (t - BEATS.comes));
  const to = feetAt(BEATS.end);
  s.call("staged.bend", { name: "cetri", bend: comes(1), blend_ms: 900 });
  s.call("staged.play", { name: "cetri", clip: "Troll_Walk", speed: 0.6, blend_ms: 900 });
  s.call("staged.move", { name: "cetri", to: [to[0], 0, to[1]], speed: COMES_SPEED, face: [away[0], 0, away[1]], base: true });
  const steps = Math.floor((BEATS.end - BEATS.comes) / STEP_S);
  const mix = (a: Vec3, b: Vec3, t: number): Vec3 => [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];
  const flat = (e: Vec3): [number, number] => [e[0], e[2]];
  const [eye0, eye1] = [on(flinched, EYE - 0.05), on(away, EYE - 0.1)];
  for (let k = 0; k < steps; k++) {
    const t0 = BEATS.comes + 0.35 + k * STEP_S;
    const side = k % 2 ? -1 : 1;
    const [a, b] = [mix(eye0, eye1, (k + 0.1) / steps), mix(eye0, eye1, (k + 1) / steps)];
    const foot = at(feetAt(t0), 0.3, 0.45 * side);
    const watched = k === WATCHED.step;
    // Once the eye drops to a root as it comes down, to see the weight land: the litter
    // bursts, the eye jolts, then it climbs back to the face.
    if (watched) {
      s.frame = f(t0 - WATCHED.early);
      s.glide(a, on(foot, WATCHED.up), Math.round(WATCHED.early * 60));
    }
    s.frame = f(t0);
    if (k > 0) s.call("staged.bend", { name: "cetri", bend: comes(side), blend_ms: 300 });
    s.call("staged.impact", { at: on(foot, 0.05), size: watched ? WATCHED_DUST : STEP_DUST, base: true });
    // The first steps watched a little lower, its planted roots and their litter nearer the
    // frame and the face still in it (at the knees, round 6's second pass lost the face off
    // the top); then up at the face as it closes. The jolt grows as it nears.
    const level = (t: number) => (t < BEATS.comes + 2.5 ? { up: 3.3, ahead: 0.3 } : FACE);
    const jolt: Vec3 = [a[0], a[1] - 0.05 - 0.015 * k - (watched ? 0.06 : 0), a[2]];
    if (watched) {
      s.glide(jolt, on(foot, WATCHED.up - 0.15), 5);
      s.frame = f(t0 + WATCHED.stays);
      s.glide(b, face(feetAt(t0 + STEP_S), flat(b), level(t0 + STEP_S)), Math.round((STEP_S - WATCHED.stays) * 60));
      continue;
    }
    s.glide(jolt, face(feetAt(t0), flat(a), level(t0)), 5);
    s.frame = f(t0) + 5;
    s.glide(b, face(feetAt(t0 + STEP_S), flat(b), level(t0 + STEP_S)), Math.round(STEP_S * 60) - 5);
  }
  s.frame = f(BEATS.end);
}

export default {
  about: "the lush forest's path in the morning: one of the oaks beside it wakes, its eyes lighting and its trunk splitting into a mouth, lunges at the walker and comes on in slow, heavy steps",
  sandbox: [],
  rolls: ROLLS,
  script,
};
