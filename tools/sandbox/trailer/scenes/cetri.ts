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
/// The walker's eye up the path (forest.json's route, (3.7, 4.7) to (16.8, 25)), then off it toward the oaks.
const WALK: [[number, number], [number, number]] = [
  [12.5, 18.5],
  [17.2, 29.2],
];
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
  thigh_l: [0, 0, -4],
  thigh_r: [0, 0, 4],
  spine_01: [2, 0, 3],
};
/// Awake and roaring: the arms brought down and out from the crown, the crown thrown back.
const ROARS = { upperarm_l: [-20, 0, 35], upperarm_r: [-20, 0, -35], spine_03: [-6, 0, 0], neck_01: [-6, 0, 0] };
/// The lunge: the whole trunk tipped forward from the hips, the crown lagging behind it.
const LUNGES = { spine_01: [14, 0, 0], spine_02: [10, 0, 0], spine_03: [6, 0, 0], neck_01: [-5, 0, 0], Head: [-6, 0, 0] };
/// Coming on: still leant into its steps.
const COMES = { spine_01: [7, 0, 0], spine_02: [4, 0, 0] };

function script(s: Script) {
  const f = (t: number) => ROLLS + Math.round(t * 60);
  const far = WALK[1];
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
  s.call("staged.spawn", { name: "cetri", model: "models/characters/cetri.glb", bend: ASLEEP, at: [CETRI[0], 0, CETRI[1]], facing: [far[0], 0, far[1]], turn_deg_s: 25, base: true });
  s.frame = 40;
  // Asleep it barely stirs: its idle slowed until only the wind would move it.
  s.call("staged.play", { name: "cetri", clip: "Troll_Idle", speed: 0.04, blend_ms: 0 });

  // The walk up the path, looking at the oaks ahead; it slows and stops as the tree wakes.
  const look = (h: number): Vec3 => on(CETRI, h);
  s.frame = 60;
  s.cut(on(WALK[0], EYE), look(4.5));
  s.frame = ROLLS;
  s.glide(on(WALK[1], EYE), look(4.8), Math.round(BEATS.lunge * 60));

  s.frame = f(BEATS.glow);
  s.call("staged.glow", { name: "cetri", strength: 4, blend_ms: 1600 });
  s.frame = f(BEATS.wakes);
  s.call("staged.bend", { name: "cetri", bend: ROARS, blend_ms: 2200 });
  s.call("staged.play", { name: "cetri", clip: "Troll_Roar", speed: 0.6, once: true, blend_ms: 1200 });
  s.call("staged.morph", { name: "cetri", weights: { Wake: 1 }, blend_ms: 1400 });

  // The lunge: the trunk tips at the walker, who flinches back a few steps.
  s.frame = f(BEATS.lunge);
  s.call("staged.play", { name: "cetri", clip: "Troll_Smash", speed: 0.7, once: true, blend_ms: 500 });
  s.call("staged.bend", { name: "cetri", bend: LUNGES, blend_ms: 700 });
  const back = toward(CETRI, WALK[1], Math.hypot(WALK[1][0] - CETRI[0], WALK[1][1] - CETRI[1]) + 3);
  s.glide(on(back, EYE - 0.15), look(4.2), 70);

  // Then it comes on, slow and heavy, and the walker backs away looking up at it.
  s.frame = f(BEATS.comes);
  const to = toward(CETRI, back, 4);
  s.call("staged.bend", { name: "cetri", bend: COMES, blend_ms: 900 });
  s.call("staged.play", { name: "cetri", clip: "Troll_Walk", speed: 0.75, blend_ms: 900 });
  s.call("staged.move", { name: "cetri", to: [to[0], 0, to[1]], speed: 0.6, base: true });
  const away = toward(CETRI, back, Math.hypot(back[0] - CETRI[0], back[1] - CETRI[1]) + 2.5);
  s.glide(on(away, EYE - 0.1), on(to, 5), Math.round((BEATS.end - BEATS.comes) * 60));
  s.frame = f(BEATS.end);
}

export default {
  about: "the lush forest's path in the morning: one of the oaks beside it wakes, its eyes lighting and its trunk splitting into a mouth, lunges at the walker and comes on in slow, heavy steps",
  sandbox: [],
  rolls: ROLLS,
  script,
};
