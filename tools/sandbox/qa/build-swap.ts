// A clip: one wall of the finished cabin changed to another material in place
// (tools/sandbox/qa.ts).

import { putUp, SANDBOX, X0 } from "./lib/cabin.ts";
import type { KitSet, Scenario, Script } from "./script.ts";

function script(s: Script, set: KitSet) {
  putUp(s, set, "roof");
  s.stage("swap", `Look for: the west wall turning to ${set.other(0)}, its corners and trims following, the roof over it standing`, () => {
    s.cut([-5.0, 2.5, 8.0], [2.4, 1.3, 4.0]);
    s.wait(40);
    for (const z of [4, 6, 8, 10]) for (const y of [0, 2, 4]) s.call("build.replace", { at: [X0, y, z], brush: 2, kind: set.other(0) });
    s.wait(20);
    s.fill("swap", { seconds: 8, gap: 10 });
    s.wait(90);
  });
}

export default { about: "one wall of the finished cabin changed to another material in place", sandbox: SANDBOX, clip: true, script } satisfies Scenario;
