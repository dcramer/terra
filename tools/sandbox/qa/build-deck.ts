// A clip: a deck a storey up on four posts beside the finished cabin, and stairs up to it
// (tools/sandbox/qa.ts).

import { POSTS, putUp, SANDBOX } from "./lib/cabin.ts";
import type { KitSet, Scenario, Script } from "./script.ts";

function script(s: Script, set: KitSet) {
  putUp(s, set, "roof");
  s.stage("deck", "Look for: footings, then posts, then the deck's frame and boards on them, the stairs bearing on its rim", () => {
    s.cut([17.0, 4.5, 16.5], [9.8, 1.3, 5.0]);
    s.wait(20);
    for (const at of POSTS) s.call("build.lay", { what: "post", min: at, max: at, kind: set.wall(2) });
    s.call("build.lay", { what: "floor", min: [17, 1, 5], max: [22, 1, 10], kind: set.wall(2) });
    s.call("build.lay", { what: "flight", min: [18, -1, 17], max: [18, -1, 17], kind: set.wall(2), facing: "North", wide: 1.0 });
    s.wait(40);
    s.fill("deck", { seconds: 12, gap: 10 });
    s.wait(90);
  });
}

export default { about: "a deck on posts beside the finished cabin, and stairs up to it", sandbox: SANDBOX, clip: true, script } satisfies Scenario;
