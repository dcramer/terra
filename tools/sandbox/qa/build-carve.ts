// A clip: a window carved out of the finished cabin's wall with the brush
// (tools/sandbox/qa.ts).

import { putUp, SANDBOX, X0 } from "./lib/cabin.ts";
import type { KitSet, Scenario, Script } from "./script.ts";

function script(s: Script, set: KitSet) {
  putUp(s, set, "roof");
  s.stage("carve", "Look for: the hole carved a course at a time from the top, its new edges dressed, nothing over it falling", () => {
    s.cut([-3.5, 1.8, 5.2], [2.0, 1.3, 3.6]);
    s.wait(40);
    s.call("build.carve", { at: [X0, 2, 6], brush: 2 });
    s.wait(20);
    s.fill("carve", { seconds: 4, gap: 12 });
    s.wait(120);
  });
}

export default { about: "a window carved out of the finished cabin's wall with the brush", sandbox: SANDBOX, clip: true, script } satisfies Scenario;
