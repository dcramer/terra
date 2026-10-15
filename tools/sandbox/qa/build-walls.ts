// A clip: the cabin's footings and walls going in, from its plan (tools/sandbox/qa.ts).

import { C, putUp, SANDBOX } from "./lib/cabin.ts";
import type { KitSet, Scenario, Script } from "./script.ts";

/// How far up a step's cubes are, in cubes, as its words give it ("(10 cubes, 3 up)").
const up = (what: string) => Number(/(-?\d+) up\)/.exec(what)?.[1] ?? 0);

function script(s: Script, set: KitSet) {
  putUp(s, set, "plan");
  s.stage("walls", "Look for: footings first, then each wall's courses rising level, corners meeting as they rise", () => {
    s.cut([12.5, 4.5, 13.5], [C[0] + 0.5, 0.6, C[2] + 0.5]);
    const start = s.frame;
    const steps = s.fill("walls", { seconds: 22, gap: 10, weigh: (what) => (up(what) < 0 ? 0.3 : 1) });
    // In close at a corner where two walls meet, for the second half.
    if (steps.length) {
      const half = s.frame;
      s.frame = start + Math.round((half - start) * 0.55);
      s.cut([9.8, 2.4, 8.4], [6.7, 1.0, 5.6]);
      s.frame = half;
    }
    s.wait(90);
  });
}

export default { about: "the cabin's footings and walls going in, course by course", sandbox: SANDBOX, clip: true, script } satisfies Scenario;
