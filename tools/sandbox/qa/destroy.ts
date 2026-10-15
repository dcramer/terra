// A clip: a finished house of the set brought down (D305): its ground course knocked out
// all round, so everything over it, walls, gables and roof, has nothing under it and comes
// down in fragments that fall, bounce, settle and fade (tools/sandbox/qa.ts). The cracks,
// a tower and the ground giving way are `harm`, not in the default run.

import { C, putUp, SANDBOX, X0, X1, Z0, Z1 } from "./lib/cabin.ts";
import type { KitSet, Scenario, Script } from "./script.ts";

function script(s: Script, set: KitSet) {
  putUp(s, set, "roof");
  s.stage("collapse", "Look for: everything over the knocked-out course coming down at once, fragments in the walls' and roof's own materials, settling and fading, nothing left hanging", () => {
    s.cut([13.5, 4.5, 15.0], [C[0], 1.6, C[2]]);
    s.wait(75);
    s.call("build.damage", { at: [X0, 0, Z0], to: [X1, 1, Z1] });
    s.wait(600);
  });
}

export default { about: "a finished house of the set brought down by knocking out its ground course", sandbox: SANDBOX, clip: true, script } satisfies Scenario;
