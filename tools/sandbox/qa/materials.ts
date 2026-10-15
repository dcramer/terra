// The materials film: a corner of wall built in a set and changed in place to every other
// wall kit in turn, and back (D232): each kit's look where it meets the ground, its corner
// and its top, and how fast a change of material goes in, timed by the trace. Kits are
// read from assets/data/kits/, so one added is filmed with no code.

import type { KitSet, Scenario, Script, Vec3 } from "./script.ts";

const CORNER: Vec3 = [2.5, 1.2, 4.5];

function script(s: Script, set: KitSet) {
  const own = set.wall(0);
  const others = set.kits.filter((kit) => kit.roof && kit.name !== own).map((kit) => kit.name);
  // The two arms of the corner, in boxes of the brush's two cubes.
  const boxes: number[][] = [];
  for (const y of [0, 2, 4]) {
    for (const x of [4, 6, 8, 10]) boxes.push([x, y, 8]);
    for (const z of [10, 12]) boxes.push([4, y, z]);
  }

  s.stage("built", `A corner of ${own}`, () => {
    s.call("storage.place", { item: "Claim stone", at: [0, 0, 0] });
    s.baseMovesAt(s.frame + 1, [0, -0.465, 0]);
    s.wait(2);
    s.call("build.lay", { what: "wall", min: [4, 0, 8], max: [11, 0, 8], kind: own });
    s.call("build.lay", { what: "wall", min: [4, 0, 8], max: [4, 0, 13], kind: own });
    s.wait(2);
    s.call("build.fill", { steps: 100000 });
    s.orbit(CORNER, { radius: 9, height: 2.5, from: 120, to: 160, frames: 300 });
    s.wait(300);
  });

  [...others, own].forEach((kind, i) => {
    s.stage(`to-${kind}`, `Changed in place to ${kind}`, () => {
      s.orbit(CORNER, { radius: 8.5, height: 2.2 + (i % 2), from: 150 + i * 12, to: 165 + i * 12, frames: 420 });
      for (const at of boxes) s.call("build.replace", { at, brush: 2, kind });
      s.wait(20);
      s.fill(`to ${kind}`, { seconds: 5, gap: 6 });
      s.wait(60);
    });
  });
}

export default {
  about: "a corner of wall changed in place to every wall kit in turn and back, the speed of each change timed",
  sandbox: ["--experiment", "play", "--free-build", "--time", "10", "--at", "0,3", "--face", "-8,14"],
  script,
} satisfies Scenario;
