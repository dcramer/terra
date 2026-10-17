// A flier's moves, to judge how it flies (play::fliers, research/2026-10-17-large-flier-motion.md)
// before it is filmed in a scene: the castle (terra_build::burg) standing as a backdrop for
// scale, and over the meadow south of it a dragon cruising in, an S of turns left and right,
// a climb and a dive, a tight turn that slows it, braking into a hover, rearing back and
// breathing at the grass, and away again. Each move in its own shot, the camera beside it,
// so its bank, its head into each turn and its tail behind it, its beats and glides show.
// Places are in the castle's own metres, east, up and south of its gate (`build.look`).

import type { Script, Vec3 } from "../../qa/script.ts";
import { Course, mix, seconds, wings } from "../flight.ts";

const ROLLS = 900;
const WINGS = wings("Dragon");
const WINDUP = 48;
const at = (seconds: number) => ROLLS + Math.round(seconds * 60);

function script(s: Script) {
  s.call("build.example", { name: "burg" });
  s.frame = 2;
  s.orbit([0, 0, -19], { radius: 40, height: 16, from: 150, to: 270, frames: 700, look: 8 });
  const spawn: Vec3 = [-260, 40, 160];
  s.frame = 300;
  s.call("flight.spawn", { kind: "Dragon", at: spawn, facing: [0, 30, 60], base: true });
  const course = new Course(spawn, [0, 30, 60], WINGS);
  const fly = (frame: number, through: Vec3[], facing?: Vec3) => {
    s.frame = frame;
    s.call("flight.path", { through, ...(facing ? { facing } : {}), base: true });
    return course.fly(frame, through, facing);
  };
  // The camera at an eye, its aim on the dragon (`lead` seconds ahead) blended with a point.
  const follow = (from: number, to: number, eye: Vec3, point: Vec3, hold: number) => {
    for (let frame = from; frame <= to; frame++) {
      s.frame = frame;
      s.cut(eye, mix(course.at(frame + 9), point, hold));
    }
  };

  // In at cruise from the south-west, an S of turns (left, then right), a climb and a dive,
  // a tight turn back, and braking into a hover over the meadow.
  const inbound = fly(at(-8), [
    [-150, 30, 110],
    [-90, 24, 80],
    [-50, 22, 50],
    [-20, 24, 70],
    [10, 26, 90],
    [40, 24, 70],
    [60, 34, 40],
    [80, 48, 30],
    [110, 30, 50],
    [120, 22, 80],
    [100, 20, 110],
    [70, 18, 100],
    [40, 16, 70],
    [24, 14, 55],
  ], [0, 0, 40]);
  s.mark("s-turns", "Cruising in and an S of turns", at(0), at(8));
  follow(at(0), at(8), [-10, 6, 120], [0, 20, 60], 0.15);
  s.mark("climb-dive", "A climb, a dive and a tight turn", at(8), at(16));
  follow(at(8), at(16), [70, 8, 125], [80, 30, 60], 0.2);
  // Hovering a second after it stops, it rears back and breathes at the grass in front of
  // it, sweeping across, and then goes.
  const hovering = at(-8) + Math.round(seconds(inbound) * 60);
  const [breath, done, away] = [hovering + 60, hovering + 60 + 210, hovering + 300];
  s.mark("hover", "Braking into a hover, rearing back and breathing", at(16), away + 60);
  follow(at(16), away + 60, [48, 10, 78], [10, 6, 45], 0.35);
  s.frame = breath - WINDUP;
  s.call("flight.breathe", { on: true, at: [6, 0, 42], base: true });
  for (let frame = breath; frame < done; frame += 6) {
    s.frame = frame;
    s.call("flight.breathe", { on: true, at: mix([6, 0, 42], [-6, 0, 50], (frame - breath) / (done - breath)), base: true });
  }
  s.frame = done;
  s.call("flight.breathe", { on: false });
  // Away from the hover, gathering speed, climbing out.
  fly(away, [
    [0, 18, 70],
    [-40, 30, 110],
    [-100, 50, 160],
  ]);
  s.mark("away", "Away from the hover, climbing out", away + 61, away + 420);
  follow(away + 61, away + 420, [30, 5, 120], [-20, 20, 100], 0.2);
  s.frame = away + 420;
}

export default {
  about: "a dragon's moves over the meadow before the castle: cruising in, an S of turns, a climb and a dive, a tight turn, braking into a hover, rearing back and breathing, and away, to judge how it flies",
  sandbox: ["--free-build"],
  rolls: ROLLS,
  script,
};
