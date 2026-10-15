// A clip: a storey on the finished cabin, as a player must build one: the roof taken off,
// the storey built on the walls, a roof added over it (tools/sandbox/qa.ts). Nothing but a
// roof is built on a roof's cubes.

import { C, putUp, roofOff, SANDBOX, X0, X1, Z0, Z1 } from "./lib/cabin.ts";
import type { KitSet, Scenario, Script } from "./script.ts";

function script(s: Script, set: KitSet) {
  putUp(s, set, "roof");
  s.stage("storey", "Look for: the roof coming off last in first out, the storey's courses on the walls, the new roof whole", () => {
    s.cut([14, 8.5, 14], [C[0], 3.2, C[2]]);
    s.wait(30);
    roofOff(s);
    s.wait(20);
    s.call("build.lay", { what: "room", min: [X0, 1, Z0], max: [X1, 1, Z1], kind: set.wall(1) });
    s.wait(30);
    s.fill("storey", { seconds: 10, gap: 12 });
    s.call("build.lay", { what: "roof", over: [8, 1, 7], kind: set.wall(1), style: "Gable" });
    s.wait(30);
    s.fill("upper roof", { seconds: 8, gap: 14 });
    s.wait(90);
  });
}

export default { about: "a storey on the finished cabin: the roof off, the storey built, a roof over it", sandbox: SANDBOX, clip: true, script } satisfies Scenario;
