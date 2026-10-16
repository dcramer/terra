// A dragon burns a castle (D377, D382): the burg (terra_build::burg) standing in the morning;
// a dragon coming in over the trees and strafing its south wall, the hoardings catching as
// its breath sweeps along them; banking round to pass over the hall and the keep; hovering at
// the keep to breathe into its windows and along its bond timbers, first from one corner then
// from the other; the fire taking the hoardings, roofs and timber storeys; the towers and the
// keep coming down when their bond timbers burn through. Places are in the castle's own
// metres: east, up and south of the middle of its gate on the south wall's outer face, the
// ward to the north (-z), as `build.look` takes them. Frames are a sixtieth of a second each.
// Where the camera follows the dragon, it looks where the dragon will be, worked out here as
// `play::fliers` flies it.

import type { Script, Vec3 } from "../../qa/script.ts";

/// The frame the film rolls on: before it the castle is put up and laid, its textures come
/// in and the world streams round the first shot.
const ROLLS = 1800;
/// The dragon's cruising speed and how slowly it eases into a hover (fliers.ron).
const CRUISE = 24;
const HOVER = 3;
/// How fast it eases into a hover, a share a second (`play::fliers::SETTLE`).
const SETTLE = 1.6;

/// The frame so many seconds into the film.
const at = (seconds: number) => ROLLS + Math.round(seconds * 60);

type Flight = { points: Vec3[]; t: number[]; seconds: number };

const add = (a: Vec3, b: Vec3): Vec3 => [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
const mix = (a: Vec3, b: Vec3, t: number): Vec3 => [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];
const length = (a: Vec3, b: Vec3) => Math.hypot(b[0] - a[0], b[1] - a[1], b[2] - a[2]);

/// How `play::fliers` flies a path from `from` through `through`: a Catmull-Rom spline laid
/// out half a metre a point, at its speed, slowing over its last 40 m to its hover speed.
/// Each laid-out point and when, in seconds from the call, it is reached.
function flight(from: Vec3, through: Vec3[], speed = CRUISE): Flight {
  const all = [from, ...through];
  const points: Vec3[] = [from];
  const p = (i: number) => all[Math.max(0, Math.min(all.length - 1, i))];
  for (let i = 0; i < all.length - 1; i++) {
    const [p0, p1, p2, p3] = [p(i - 1), p(i), p(i + 1), p(i + 2)];
    const steps = Math.max(1, Math.ceil(length(p1, p2) / 0.5));
    for (let s = 1; s <= steps; s++) {
      const t = s / steps;
      const [t2, t3] = [t * t, t * t * t];
      points.push([0, 1, 2].map((k) => 0.5 * (2 * p1[k] + (p2[k] - p0[k]) * t + (2 * p0[k] - 5 * p1[k] + 4 * p2[k] - p3[k]) * t2 + (3 * p1[k] - p0[k] - 3 * p2[k] + p3[k]) * t3)) as Vec3);
    }
  }
  const lengths = points.slice(1).map((q, i) => length(points[i], q));
  const total = lengths.reduce((a, b) => a + b, 0);
  const t = [0];
  let done = 0;
  for (const piece of lengths) {
    const v = HOVER + (speed - HOVER) * Math.min(1, Math.max(0, total - done) / 40);
    done += piece;
    t.push(t[t.length - 1] + piece / v);
  }
  return { points, t, seconds: t[t.length - 1] };
}

/// When, in seconds after a flight was called, it passes nearest a point.
function passes(f: Flight, point: Vec3): number {
  let best = 0;
  let nearest = Infinity;
  f.points.forEach((q, i) => {
    const d = length(q, point);
    if (d < nearest) [best, nearest] = [i, d];
  });
  return f.t[best];
}

/// Where a flight is so many seconds after it was called.
function where(f: Flight, seconds: number): Vec3 {
  let i = 0;
  while (i + 1 < f.t.length && f.t[i + 1] < seconds) i++;
  if (i + 1 >= f.t.length) return f.points[f.points.length - 1];
  const span = f.t[i + 1] - f.t[i];
  return mix(f.points[i], f.points[i + 1], span > 0 ? (seconds - f.t[i]) / span : 0);
}

/// The dragon's whole course, as the remote is told it: each flight and hover from its frame,
/// so the camera can follow it.
class Course {
  private legs: { frame: number; flight?: Flight; hover?: Vec3 }[] = [];
  private start: Vec3;
  constructor(start: Vec3) {
    this.start = start;
  }

  /// Where the dragon is on a frame.
  at(frame: number): Vec3 {
    let here = this.start;
    for (const [i, leg] of this.legs.entries()) {
      if (leg.frame > frame) break;
      const next = this.legs[i + 1]?.frame ?? Infinity;
      const until = Math.min(frame, next);
      const seconds = (until - leg.frame) / 60;
      if (leg.flight) {
        here = where(leg.flight, seconds);
      } else if (leg.hover) {
        here = mix(here, leg.hover, 1 - Math.exp(-SETTLE * seconds));
      }
    }
    return here;
  }

  /// Flies it along a path from where it is on a frame; returns the flight.
  fly(s: Script, frame: number, through: Vec3[], facing?: Vec3, speed = CRUISE): Flight {
    const f = flight(this.at(frame), through, speed);
    this.legs.push({ frame, flight: f });
    s.frame = frame;
    s.call("flight.path", { through, speed, ...(facing ? { facing } : {}), base: true });
    return f;
  }

  /// Holds it hovering at a point, facing another, from a frame.
  hover(s: Script, frame: number, point: Vec3, facing: Vec3) {
    this.legs.push({ frame, hover: point });
    s.frame = frame;
    s.call("flight.hover", { at: point, facing, base: true });
  }
}

/// Breathes fire along a line from `a` to `b` from one frame to another, aimed a little
/// further along each tenth of a second, as a strafing breath sweeps (Elden Ring's fly-by).
function sweep(s: Script, from: number, to: number, a: Vec3, b: Vec3) {
  for (let frame = from; frame < to; frame += 6) {
    const t = (frame - from) / Math.max(1, to - from);
    s.frame = frame;
    s.call("flight.breathe", { on: true, at: mix(a, b, t), base: true });
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

/// The camera from one frame to another, its eye moving from `a` to `b` (eased), looking at
/// the dragon where it will be, `lead` seconds ahead, blended with a point by `hold` (0 the
/// dragon alone, 1 the point alone): a tracking shot, as an operator pans with a flier.
function track(s: Script, course: Course, from: number, to: number, a: Vec3, b: Vec3, point: Vec3, hold: number, lead = 0.15) {
  for (let frame = from; frame <= to; frame++) {
    const t = (frame - from) / Math.max(1, to - from);
    const eased = t * t * (3 - 2 * t);
    const dragon = course.at(frame + Math.round(lead * 60));
    cut(s, frame, mix(a, b, eased), mix(dragon, point, hold));
  }
}

/// The keep: the middle of its foot, how high its bond timbers run, and its faces.
const KEEP: Vec3 = [0, 0, -19.75];
const BAND = 2.7;
const KEEP_HALF = 4.5;

function script(s: Script) {
  // Setting up, not filmed: the castle put up whole as the base, the camera going round it
  // close and then out to every shot's distance while it is laid, so each part is laid as
  // finely as it will be seen and its textures are in; the dragon far off over the trees.
  s.call("build.example", { name: "burg" });
  s.frame = 2;
  s.orbit([0, 0, -19], { radius: 36, height: 14, from: -30, to: 330, frames: 1100, look: 8 });
  cut(s, 1150, [-60, 12, 50], [0, 10, -18]);
  cut(s, 1300, [-44, 26, -74], [0, 12, -24]);
  cut(s, 1450, [34, 10, 22], [6, 14, -14]);
  cut(s, 1600, [48, 5, 52], [0, 11, -18]);
  const spawn: Vec3 = [-130, 48, 80];
  s.frame = 400;
  s.call("flight.spawn", { kind: "Dragon", at: spawn, facing: [0, 20, 0], base: true });
  const course = new Course(spawn);

  // 0-5 s: the castle standing in the morning light, the camera drifting in.
  s.mark("standing", "The castle standing in the morning", at(0), at(4.5));
  glide(s, at(0), [41, 6, 44], [0, 11, -18], 270);

  // The first pass, in over the trees from the south-west and east along the south wall over
  // its hoardings and the gatehouse, the camera low by the south-east tower looking back
  // along the wall at it; the hoardings catch as the breath sweeps along them; it passes
  // over the camera, the camera turning with it, and banks round to hover off the keep's
  // south-east corner.
  const k = KEEP;
  const first = course.fly(s, at(0.5), [
    [-70, 30, 40],
    [-40, 21, 5],
    [0, 20, -0.5],
    [34, 24, -4],
    [44, 28, -22],
    [30, 26, -24],
    [19, 21, -12],
  ], [k[0], 6, k[2]], 26);
  const wallFrom = at(0.5) + Math.round(passes(first, [-30, 21, 3]) * 60);
  const wallTo = at(0.5) + Math.round(passes(first, [28, 23, -3]) * 60);
  const hovering = at(0.5) + Math.round(first.seconds * 60);
  s.mark("first-pass", "The dragon comes in over the trees and strafes the south wall", at(4.5), wallTo + 50);
  cut(s, at(4.5), [36, 3, 13], [-30, 14, 1]);
  glide(s, at(4.5) + 1, [33, 3.5, 12], [-24, 15, 1], Math.max(30, wallFrom - at(4.5) - 30));
  sweep(s, wallFrom, wallTo, [-24, 10, 1.6], [24, 10, 1.6]);
  track(s, course, wallTo - 50, wallTo + 50, [33, 3.5, 12], [31, 4, 14], [10, 14, -2], 0.35);

  // The hover, off the keep's south-east corner: into its door and its ground storey's
  // windows, then along its bond timbers on its south and east faces; the ground storey
  // under them fills with fire. The camera close, the dragon and the keep in the frame.
  s.mark("hover", "It hovers at the keep and breathes into its foot, through its door and along its bond timbers", wallTo + 50, hovering + 480);
  cut(s, wallTo + 50, [38, 7, 20], [12, 15, -10]);
  glide(s, wallTo + 51, [32, 8, 26], [8, 13, -12], hovering + 160 - wallTo - 51);
  // Closer, from behind and beside it, its breath going into the keep's foot.
  cut(s, hovering + 190, [33, 25, 4], [4, 4, -16]);
  glide(s, hovering + 191, [30, 23, 10], [4, 4, -16], 290);
  sweep(s, hovering + 10, hovering + 190, [k[0] - 1, 0.8, k[2] + KEEP_HALF], [k[0] + 2, 2, k[2] + KEEP_HALF]);
  sweep(s, hovering + 200, hovering + 330, [k[0] - KEEP_HALF, BAND, k[2] + KEEP_HALF], [k[0] + KEEP_HALF, BAND, k[2] + KEEP_HALF]);
  sweep(s, hovering + 340, hovering + 470, [k[0] + KEEP_HALF, BAND, k[2] + KEEP_HALF], [k[0] + KEEP_HALF, BAND, k[2] - KEEP_HALF]);

  // A pass west over the gatehouse, the south-west tower and the west wall's hoardings, then
  // round over the north and down over the hall's roof; the camera high over the north-west
  // following it, then wide from the south-west with the castle ablaze.
  const turn = hovering + 480;
  const round = course.fly(s, turn, [
    [24, 30, 16],
    [-6, 30, 22],
    [-21, 28, 2],
    [-24, 30, -30],
    [-20, 40, -70],
    [10, 34, -64],
    [40, 40, -40],
    [55, 46, 10],
  ]);
  const towerFrom = turn + Math.round(passes(round, [-6, 30, 22]) * 60);
  const towerTo = turn + Math.round(passes(round, [-24, 30, -30]) * 60);
  const hallFrom = turn + Math.round(passes(round, [-20, 40, -70]) * 60);
  const hallTo = turn + Math.round(passes(round, [40, 40, -40]) * 60);
  s.mark("round", "A pass over the west wall and the hall's roof; the fire takes the castle", turn, at(34));
  track(s, course, turn, hallFrom - 20, [-62, 12, 50], [-58, 16, 46], [-6, 12, -14], 0.6);
  sweep(s, towerFrom, towerTo, [-20.5, 14, -1], [-23, 10, -30]);
  track(s, course, hallFrom - 19, hallTo + 40, [-50, 30, -84], [-42, 28, -78], [0, 14, -30], 0.55);
  sweep(s, hallFrom, hallTo, [-14, 14, -32], [12, 14, -32]);
  cut(s, hallTo + 41, [-66, 10, 52], [-4, 12, -18]);
  glide(s, hallTo + 42, [-60, 20, 48], [-2, 11, -18], Math.max(60, at(34) - hallTo - 42));

  // The towers and the keep come down; the dragon comes back over them, roars and goes off
  // over the trees.
  s.mark("falling", "The towers and the keep come down in fire and dust", at(34), at(48));
  cut(s, at(34), [42, 3.5, 32], [0, 12, -19]);
  glide(s, at(34) + 1, [48, 4.5, 40], [0, 11, -19], 840);
  course.hover(s, at(43), [14, 34, 6], [0, 10, -20]);
  s.frame = at(43.5);
  s.call("flight.roar", { seconds: 2.7 });
  course.fly(s, at(46.3), [
    [0, 50, -60],
    [-40, 90, -300],
  ]);
  s.frame = at(48);
}

export default {
  about: "a dragon burns the castle: a strafing pass along its south wall, a pass over its hall and keep, hovers at the keep breathing into it, the fire taking the castle and its towers and keep coming down",
  sandbox: ["--free-build"],
  rolls: ROLLS,
  script,
};
