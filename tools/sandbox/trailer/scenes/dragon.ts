// A dragon burns a castle (D377, D382): the burg (terra_build::burg) standing in the morning
// on its rise; a dragon coming in over the forest and making strafing passes along its walls
// and over its roofs, breathing fire as it goes, then hovering at the keep to breathe into
// it; the fire taking the hoardings, roofs and timber storeys, and the towers and the keep
// coming down when their bond timbers burn through. Places are in the castle's own metres:
// east, up and south of the middle of its gate on the south wall's outer face, the ward to
// the north (-z), as `build.look` takes them. Frames are a sixtieth of a second each.

import type { Script, Vec3 } from "../../qa/script.ts";

/// The frame the film rolls on: before it the castle is put up and laid and the world
/// streams in round the first shot.
const ROLLS = 900;
/// The dragon's cruising speed and how slowly it eases into a hover (fliers.ron).
const CRUISE = 24;
const HOVER = 3;

/// The frame so many seconds into the film.
const at = (seconds: number) => ROLLS + Math.round(seconds * 60);

type Flight = { points: Vec3[]; t: number[]; seconds: number };

/// How `play::fliers` flies a path from `from` through `through`: a Catmull-Rom spline laid
/// out half a metre a point, at its cruising speed, slowing over its last 40 m to its hover
/// speed. Each laid-out point and when, in seconds from the call, it is reached.
function flight(from: Vec3, through: Vec3[], speed = CRUISE): Flight {
  const all = [from, ...through];
  const points: Vec3[] = [from];
  const p = (i: number) => all[Math.max(0, Math.min(all.length - 1, i))];
  for (let i = 0; i < all.length - 1; i++) {
    const [p0, p1, p2, p3] = [p(i - 1), p(i), p(i + 1), p(i + 2)];
    const steps = Math.max(1, Math.ceil(Math.hypot(p2[0] - p1[0], p2[1] - p1[1], p2[2] - p1[2]) / 0.5));
    for (let s = 1; s <= steps; s++) {
      const t = s / steps;
      const [t2, t3] = [t * t, t * t * t];
      points.push([0, 1, 2].map((k) => 0.5 * (2 * p1[k] + (p2[k] - p0[k]) * t + (2 * p0[k] - 5 * p1[k] + 4 * p2[k] - p3[k]) * t2 + (3 * p1[k] - p0[k] - 3 * p2[k] + p3[k]) * t3)) as Vec3);
    }
  }
  const lengths = points.slice(1).map((q, i) => Math.hypot(q[0] - points[i][0], q[1] - points[i][1], q[2] - points[i][2]));
  const total = lengths.reduce((a, b) => a + b, 0);
  const t = [0];
  let done = 0;
  for (const length of lengths) {
    const v = HOVER + (speed - HOVER) * Math.min(1, Math.max(0, total - done) / 40);
    done += length;
    t.push(t[t.length - 1] + length / v);
  }
  return { points, t, seconds: t[t.length - 1] };
}

/// When, in seconds after a flight was called, it passes nearest a point.
function passes(f: Flight, point: Vec3): number {
  let best = 0;
  let nearest = Infinity;
  f.points.forEach((q, i) => {
    const d = Math.hypot(q[0] - point[0], q[1] - point[1], q[2] - point[2]);
    if (d < nearest) [best, nearest] = [i, d];
  });
  return f.t[best];
}

/// Flies the dragon along a path from where it is, called on a frame; returns its flight.
function fly(s: Script, frame: number, from: Vec3, through: Vec3[], facing?: Vec3, speed = CRUISE): Flight {
  s.frame = frame;
  s.call("flight.path", { through, speed, ...(facing ? { facing } : {}), base: true });
  return flight(from, through, speed);
}

/// Breathes fire along a line from `a` to `b` from one frame to another, aimed a little
/// further along each tenth of a second, as a strafing breath sweeps (Elden Ring's fly-by).
function sweep(s: Script, from: number, to: number, a: Vec3, b: Vec3) {
  for (let frame = from; frame < to; frame += 6) {
    const t = (frame - from) / Math.max(1, to - from);
    s.frame = frame;
    s.call("flight.breathe", { on: true, at: [0, 1, 2].map((k) => a[k] + (b[k] - a[k]) * t), base: true });
  }
  s.frame = to;
  s.call("flight.breathe", { on: false });
}

/// A camera move from a frame: a cut to an eye looking at a point, or a glide there over so
/// many frames from wherever it is.
function cut(s: Script, frame: number, eye: Vec3, look: Vec3) {
  s.frame = frame;
  s.cut(eye, look);
}
function glide(s: Script, frame: number, eye: Vec3, look: Vec3, frames: number) {
  s.frame = frame;
  s.glide(eye, look, frames);
}

function script(s: Script) {
  // Setting up, not filmed: the castle put up whole as the base, the dragon far off over the
  // forest to the south-west, the camera on the first shot while the world streams.
  s.call("build.example", { name: "burg" });
  cut(s, 2, [64, 4.5, 74], [0, 9, -18]);
  const spawn: Vec3 = [-150, 50, 100];
  s.frame = 400;
  s.call("flight.spawn", { kind: "Dragon", at: spawn, facing: [0, 20, 0], base: true });

  // 0-5 s: the castle standing in the morning, the camera drifting in.
  s.frame = at(0);
  s.mark("standing", "The castle standing in the morning", at(0), at(5));
  glide(s, at(0), [54, 6, 62], [0, 10, -18], 300);

  // The first pass: in over the meadow, east along the south wall over its hoardings and the
  // gatehouse, banking hard round the east side and north over the north-east tower; then
  // south over the hall and the keep; then a hover off the keep's south-east corner.
  const route: Vec3[] = [
    [-90, 34, 55],
    [-45, 22, 6],
    [0, 21, -1],
    [40, 25, -6],
    [62, 32, -30],
    [45, 38, -62],
    [10, 36, -64],
    [-2, 35, -40],
    [8, 29, -10],
    [21, 22, -3],
  ];
  const go = at(1);
  const passOne = fly(s, go, spawn, route, [0, 8, -20], 30);
  const wallFrom = go + Math.round(passes(passOne, [-34, 23, 2]) * 60);
  const wallTo = go + Math.round(passes(passOne, [30, 24, -3]) * 60);
  s.mark("first-pass", "The dragon comes in over the meadow and strafes the south wall", at(5), wallTo + 40);
  cut(s, at(5), [-62, 3, 46], [-8, 12, -4]);
  glide(s, at(5.05), [-54, 4, 40], [6, 14, -4], wallTo - at(5.05) + 60);
  sweep(s, wallFrom, wallTo, [-24, 10, 1.5], [26, 10, 1.5]);

  const hallFrom = go + Math.round(passes(passOne, [10, 36, -64]) * 60);
  const keepTo = go + Math.round(passes(passOne, [-2, 35, -40]) * 60);
  const hovering = go + Math.round(passOne.seconds * 60);
  s.mark("second-pass", "It banks round and passes over the hall and the keep, breathing on their roofs", wallTo + 40, hovering);
  cut(s, wallTo + 40, [-52, 32, -84], [0, 14, -30]);
  glide(s, wallTo + 42, [-44, 30, -78], [4, 14, -24], hovering - wallTo);
  sweep(s, hallFrom, keepTo + 20, [-15, 14, -32], [1, 22, -21]);

  // The hover: into the keep through its door and windows, then along its bond timbers.
  const hoverTo = hovering + 330;
  s.mark("hover", "It hovers at the keep and breathes into it, through its door and windows and along its bond timbers", hovering, hoverTo);
  cut(s, hovering, [40, 8, 30], [10, 14, -12]);
  glide(s, hovering + 2, [34, 10, 36], [8, 14, -12], 330);
  sweep(s, hovering + 20, hovering + 170, [-1, 1.5, -15.4], [4, 6, -15.4]);
  sweep(s, hovering + 180, hovering + 300, [4.6, 5.6, -16], [4.6, 6, -23]);

  // A pass west over the south-west tower's top and the west wall's hoardings, then circling.
  const turnFrom: Vec3 = [21, 22, -3];
  const passTwo = fly(s, hoverTo, turnFrom, [
    [40, 30, 22],
    [5, 30, 36],
    [-26, 26, 10],
    [-30, 26, -16],
    [-40, 32, -60],
    [-10, 44, -95],
    [35, 50, -80],
    [45, 52, -20],
    [20, 50, 25],
  ]);
  const towerFrom = hoverTo + Math.round(passes(passTwo, [-26, 26, 10]) * 60);
  const towerTo = hoverTo + Math.round(passes(passTwo, [-34, 28, -30]) * 60);
  s.mark("burning", "The fire takes the hoardings, the roofs and the timber storeys", hoverTo, at(36));
  cut(s, hoverTo, [-78, 8, 60], [-4, 11, -18]);
  glide(s, hoverTo + 2, [-70, 20, 56], [-2, 10, -18], at(36) - hoverTo);
  sweep(s, towerFrom, towerTo, [-21, 13, -2], [-23, 10, -26]);

  // The towers and the keep come down; the dragon roars over them and goes.
  s.mark("falling", "The towers and the keep come down", at(36), at(48));
  cut(s, at(36), [48, 4.5, 36], [2, 10, -16]);
  glide(s, at(36.05), [55, 6.5, 44], [2, 9, -16], 720);
  s.frame = at(44);
  s.call("flight.hover", { at: [10, 34, 10], facing: [0, 10, -20], base: true });
  s.frame = at(44.5);
  s.call("flight.roar", { seconds: 2.7 });
  fly(s, at(47), [10, 34, 10], [
    [0, 50, -60],
    [-40, 90, -300],
  ]);
  s.frame = at(48);
}

export default {
  about: "a dragon burns the castle: strafing passes along its walls and over its roofs, a hover at the keep, the fire spreading, the towers and the keep coming down",
  sandbox: ["--free-build"],
  rolls: ROLLS,
  script,
};
