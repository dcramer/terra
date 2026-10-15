// A clip: "add a roof" over the built cabin, row by row through its four stages
// (tools/sandbox/qa.ts).

import { C, putUp, SANDBOX } from "./lib/cabin.ts";
import type { KitSet, Scenario, Script } from "./script.ts";

function script(s: Script, set: KitSet) {
  putUp(s, set, "walls");
  s.stage("roof", "Look for: each row through frame, battens, covering, trims; gables and eaves whole at every step", () => {
    s.cut([12.5, 7.0, 12.0], [C[0], 2.4, C[2]]);
    s.wait(20);
    s.call("build.lay", { what: "roof", over: [8, 0, 7], kind: set.wall(0), style: "Gable" });
    s.wait(40);
    s.fill("roof", { seconds: 18, gap: 18 });
    s.wait(90);
  });
}

export default { about: "a roof added over the built cabin, row by row through its four stages", sandbox: SANDBOX, clip: true, script } satisfies Scenario;
