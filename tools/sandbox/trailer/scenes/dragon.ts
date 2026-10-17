// A dragon burns a castle (D377, D382), in about thirty seconds, most of them fire and
// collapse: the burg (terra_build::burg) standing in the morning, a dragon coming in over the
// trees and strafing its south wall toward the camera, its breath running along the
// hoardings and over the towers' and the gatehouse's timber storeys just ahead of it; braking
// round into a hover low off the keep to breathe into its door, then rising to breathe along
// its jettied timber storey; the south-west tower and the curtain by it coming down, the keep
// on its burnt-out bond timbers with the dragon backing off over it and roaring, the
// south-east tower and the curtain by it as the dragon goes. Every breath aimed at the
// building, a little ahead and below the dragon's way, from close enough to strike it:
// `node tools/sandbox/flightplot.ts dragon <png>` casts each into the building and says so.
// Places are in the castle's own metres: east, up and south of the middle of its gate on the
// south wall's outer face, the ward to the north (-z), as `build.look` takes them. Frames are
// a sixtieth of a second each. Where the camera follows the dragon, it looks where the dragon
// will be, worked out here as `play::fliers` flies it.

import type { Script, Vec3 } from "../../qa/script.ts";
import { Course as Planned, mix, passes, type Plan, seconds, wings } from "../flight.ts";

/// The frame the film rolls on: before it the castle is put up and laid, its textures come
/// in and the world streams round the first shot.
const ROLLS = 1800;
/// How the dragon flies (fliers.ron), and how long it rears back before its breath comes.
const WINGS = wings("Dragon");
const WINDUP = 48;

/// The frame so many seconds into the film.
const at = (seconds: number) => ROLLS + Math.round(seconds * 60);

/// The dragon's whole course as the remote is told it (`trailer/flight.ts`, as the game flies
/// it), so the camera can follow it.
class Course {
  planned: Planned;
  constructor(start: Vec3, face: Vec3) {
    this.planned = new Planned(start, face, WINGS);
  }

  /// Where the dragon is on a frame.
  at(frame: number): Vec3 {
    return this.planned.at(frame);
  }

  /// Flies it along a course from where it is on a frame; returns the plan.
  fly(s: Script, frame: number, through: Vec3[], facing?: Vec3, speed?: number): Plan {
    s.frame = frame;
    s.call("flight.path", { through, ...(speed ? { speed } : {}), ...(facing ? { facing } : {}), base: true });
    return this.planned.fly(frame, through, facing, speed);
  }

  /// Holds it hovering at a point, facing another, from a frame.
  hover(s: Script, frame: number, point: Vec3, facing: Vec3) {
    this.planned.hover(frame, point, facing);
    s.frame = frame;
    s.call("flight.hover", { at: point, facing, base: true });
  }
}

/// Breathes fire from one frame to another, its aim each tenth of a second where `aim` says
/// for that frame; it rears back first, its wind-up, aimed where the breath will start.
function breathe(s: Script, from: number, to: number, aim: (frame: number) => Vec3) {
  s.frame = from - WINDUP;
  s.call("flight.breathe", { on: true, at: aim(from), base: true });
  for (let frame = from; frame < to; frame += 6) {
    s.frame = frame;
    s.call("flight.breathe", { on: true, at: aim(frame), base: true });
  }
  s.frame = to;
  s.call("flight.breathe", { on: false });
}

/// A breath swept along a line of points on the building, from the first to the last, evenly
/// in time over each stretch by its length, as a hovering dragon sweeps its flame.
function sweep(s: Script, from: number, to: number, points: Vec3[]) {
  const lengths = points.slice(1).map((p, i) => Math.hypot(p[0] - points[i][0], p[1] - points[i][1], p[2] - points[i][2]));
  const total = lengths.reduce((a, b) => a + b, 0) || 1;
  breathe(s, from, to, (frame) => {
    let d = (Math.max(0, frame - from) / Math.max(1, to - from)) * total;
    for (const [i, length] of lengths.entries()) {
      if (d <= length || i === lengths.length - 1) return mix(points[i], points[i + 1], Math.min(1, d / (length || 1)));
      d -= length;
    }
    return points[0];
  });
}

/// A strafing breath from one frame to another along a line on the building from `a` to `b`:
/// aimed at the point of the line `lead` metres ahead of the dragon's body along it, so the
/// flame runs along the wall just ahead of it, forward and down from its mouth, as a
/// strafing dragon breathes (Elden Ring's fly-by).
function strafe(s: Script, course: Course, from: number, to: number, a: Vec3, b: Vec3, lead: number) {
  const along: Vec3 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
  const length = Math.hypot(...along);
  breathe(s, from, to, (frame) => {
    const here = course.at(frame);
    const t = ((here[0] - a[0]) * along[0] + (here[1] - a[1]) * along[1] + (here[2] - a[2]) * along[2]) / (length * length) + lead / length;
    return mix(a, b, Math.max(0, Math.min(1, t)));
  });
}

/// The camera on a frame: an eye looking at a point.
function cut(s: Script, frame: number, eye: Vec3, look: Vec3) {
  s.frame = frame;
  s.cut(eye, look);
}

/// The camera from one frame to another, its eye moving from `a` to `b` (eased), looking at
/// the dragon where it will be, `lead` seconds ahead, blended with a point by `hold` (0 the
/// dragon alone, 1 the point alone): a tracking shot, as an operator pans with a flier. Every
/// frame is set, so nothing of the shot before shows through.
function track(s: Script, course: Course, from: number, to: number, a: Vec3, b: Vec3, point: Vec3, hold: number, lead = 0.15) {
  for (let frame = from; frame <= to; frame++) {
    const t = (frame - from) / Math.max(1, to - from);
    const eased = t * t * (3 - 2 * t);
    const dragon = course.at(frame + Math.round(lead * 60));
    cut(s, frame, mix(a, b, eased), mix(dragon, point, hold));
  }
}

/// A held shot from one frame to another: the eye drifting from `a` to `b`, its aim from one
/// point to another, every frame set.
function drift(s: Script, from: number, to: number, a: Vec3, b: Vec3, lookA: Vec3, lookB: Vec3) {
  for (let frame = from; frame <= to; frame++) {
    const t = (frame - from) / Math.max(1, to - from);
    const eased = t * t * (3 - 2 * t);
    cut(s, frame, mix(a, b, eased), mix(lookA, lookB, eased));
  }
}

/// The keep: the middle of its foot, its faces' half width, the middle of its jettied timber
/// storey's height; and in cubes its extent and its course of bond timbers over its ground
/// storey (`terra_build::scenes::burg`).
const KEEP: Vec3 = [0, 0, -19.75];
const KEEP_HALF = 4.5;
const KEEP_JETTY = 16.5;
const KEEP_CUBES = { x0: -9, x1: 8, z0: -48, z1: -31, lacing: 5 };

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
  const spawn: Vec3 = [-170, 30, 22];
  s.frame = 400;
  s.call("flight.spawn", { kind: "Dragon", at: spawn, facing: [0, 30, 20], base: true });
  const course = new Course(spawn, [0, 30, 20]);
  const k = KEEP;

  // 0-6.5 s: the castle standing in the morning, the camera low by the south-east tower
  // looking west along the south wall; the dragon comes in over the trees beyond the
  // south-west tower and strafes the wall toward the camera, a few metres out from its
  // hoardings and above them, its breath running along their outer face and roofs a little
  // ahead of it, over the south-west tower's timber storey, the gatehouse's and the
  // south-east tower's; it passes by the camera and brakes round into its hover.
  const set = at(-9);
  const first = course.fly(s, set, [
    [-110, 24, 12],
    [-50, 17.5, 6.5],
    [-20, 16.5, 6],
    [10, 16.5, 6],
    [24, 17, 5.5],
    [30, 16, -4],
    [28, 13, -13],
    [14, 10, -12],
  ], [k[0], 1.5, k[2] + KEEP_HALF], 16);
  const wallFrom = set + Math.round(passes(first, [-36, 17, 6]) * 60);
  const wallTo = set + Math.round(passes(first, [16, 16.5, 6]) * 60);
  const eastFrom = set + Math.round(passes(first, [29, 16.5, -1]) * 60);
  const eastTo = set + Math.round(passes(first, [28, 13, -13]) * 60);
  const hovering = set + Math.round(seconds(first) * 60);
  s.mark("first-pass", "The castle standing; the dragon comes in over the trees and strafes the south wall and the east", at(0), eastTo);
  strafe(s, course, wallFrom, wallTo, [-21, 12.5, 2.6], [21, 12.5, 2.6], 13);
  // Banking north round the south-east tower, along the east wall's hoardings.
  strafe(s, course, eastFrom, eastTo, [23.3, 10.5, -3], [23.3, 10.5, -28], 15);
  track(s, course, at(0), eastTo, [34, 3, 13], [38, 5, 16], [-12, 11, 1], 0.55);

  // The hover at the keep, low off its south-east, the camera high behind the dragon:
  // its breath into the keep's door and its ground storey, which fills with fire; then it
  // rises to breathe along the jettied timber storey, its south face and its east.
  s.mark("hover", "It hovers at the keep and breathes into its door, then along its timber storey", eastTo + 1, at(16));
  const face = k[2] + KEEP_HALF + 0.1;
  // High over the south wall west of the south-east tower, clear of its roof, looking down
  // past the dragon to the keep, away from the low sun.
  track(s, course, eastTo + 1, at(16), [14, 27, 10], [11, 26, 12], [k[0], 4, face], 0.35);
  sweep(s, hovering - 90, hovering + 60, [
    [k[0] - 0.5, 0.8, face],
    [k[0] + 0.5, 2.2, face],
  ]);
  const up = hovering + 65;
  course.hover(s, up, [12, 19, -7], [k[0], KEEP_JETTY, k[2]]);
  sweep(s, up + 50, up + 220, [
    [k[0] - KEEP_HALF + 0.5, KEEP_JETTY, face + 0.5],
    [k[0] + KEEP_HALF + 0.6, KEEP_JETTY, face + 0.5],
    [k[0] + KEEP_HALF + 0.6, KEEP_JETTY + 2, k[2] - KEEP_HALF],
  ]);

  // The towers and the keep come down. The fire took their timber storeys and the hoardings
  // that join them to the curtain; what is left of each gives way, harmed as an attack harms
  // (`build.damage`, D305), and the building's own destruction brings the stone down whole, in
  // fragments and dust, each fall in the middle of its shot, looking away from the low sun.
  s.mark("falling", "The towers and the keep come down in fire and dust", at(16), at(30));
  // The south-west tower, and the curtain east of it toward the gate.
  drift(s, at(16), at(20), [-1, 6, 31], [1, 6.5, 29], [-17, 9, -1], [-15, 7, -1]);
  fell(s, at(17), SW);
  breach(s, at(18.2), -31, -10);
  // The keep: its bond timbers over its burnt-out ground storey give way and it comes down,
  // the dragon backing off from it as it goes, then roaring over it; the camera high enough
  // over the south wall to see the keep whole past the gatehouse.
  course.hover(s, at(20.2), [12, 26, -10], [k[0], 12, k[2]]);
  track(s, course, at(20) + 1, at(25), [14, 26, 22], [16, 27, 25], [k[0], 10, k[2]], 0.65);
  s.frame = at(21.3);
  const kc = KEEP_CUBES;
  s.call("build.damage", { at: [kc.x0 - 1, kc.lacing, kc.z0 - 1], to: [kc.x1 + 1, kc.lacing, kc.z1 + 1] });
  s.frame = at(22.6);
  s.call("flight.roar", { seconds: 2.2 });
  // The south-east tower and the curtain west of it, the dragon going off over the ruin.
  drift(s, at(25) + 1, at(30), [44, 5, 24], [46, 6, 27], [17, 9, -2], [14, 8, -3]);
  fell(s, at(25.8), SE);
  breach(s, at(26.8), 9, 30);
  course.fly(s, at(26), [
    [0, 30, -10],
    [-40, 46, -60],
    [-160, 80, -240],
  ]);
  s.frame = at(30);
}

/// The flanking towers' corners, in cubes (`terra_build::scenes::burg`): each a ring 14 a
/// side from there, a cube clear of the curtain all round.
const TOWER = 14;
const SW: [number, number] = [-48, -10];
const SE: [number, number] = [34, -10];

/// The south curtain (cubes: z from -3 to -1, 18 high, its hoardings to 23 and out over its
/// outer face) gives way from one x to another: its foot harmed past mending, and a slot cut
/// through it and its hoardings at each end, so all of it between comes down.
function breach(s: Script, frame: number, x0: number, x1: number) {
  s.frame = frame;
  const [z0, z1] = [-4, 2];
  s.call("build.damage", { at: [x0, 0, z0], to: [x1, 1, z1] });
  s.call("build.damage", { at: [x0, 2, z0], to: [x0, 26, z1] });
  s.call("build.damage", { at: [x1, 2, z0], to: [x1, 26, z1] });
}

/// A tower gives way: its foot, and the cube clear round it where its hoardings' bridges
/// still join it to the curtain, harmed past mending, so all that stands on them comes down.
function fell(s: Script, frame: number, [x, z]: [number, number]) {
  s.frame = frame;
  const [x0, x1, z0, z1] = [x - 1, x + TOWER, z - 1, z + TOWER];
  s.call("build.damage", { at: [x0, 0, z0], to: [x1, 1, z1] });
  s.call("build.damage", { at: [x0, 2, z0], to: [x0, 30, z1] });
  s.call("build.damage", { at: [x1, 2, z0], to: [x1, 30, z1] });
  s.call("build.damage", { at: [x0, 2, z0], to: [x1, 30, z0] });
  s.call("build.damage", { at: [x0, 2, z1], to: [x1, 30, z1] });
}

export default {
  about: "a dragon burns the castle in thirty seconds: a strafing pass along its south wall, a hover at the keep breathing into its door and along its timber storey, its towers and keep coming down",
  sandbox: ["--free-build"],
  rolls: ROLLS,
  script,
};
