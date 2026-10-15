// A clip: openings in the built cabin, a window cut, shutters and a door fitted, the door
// opened (tools/sandbox/qa.ts).

import { CUT, DOOR, putUp, SANDBOX, Z0 } from "./lib/cabin.ts";
import type { KitSet, Scenario, Script } from "./script.ts";

function script(s: Script, set: KitSet) {
  putUp(s, set, "walls");
  s.stage("window", "Look for: the window cut course by course, its dressing, the shutters filling it", () => {
    s.cut([4.5, 1.8, -2.5], [4.5, 1.3, 2.0]);
    s.wait(30);
    s.call("build.opening", { tiles: CUT, door: false });
    s.wait(20);
    s.fill("cut", { seconds: 3, gap: 12 });
    s.wait(40);
    s.call("build.fit", { at: [CUT[0][0], 2, Z0], kind: "Louvred shutters" });
    s.wait(120);
  });
  s.stage("door", "Look for: the door filling its frame, then swinging open about its hinge", () => {
    s.cut([4.6, 1.5, 9.4], [4.5, 1.1, 5.4]);
    s.wait(30);
    s.call("build.fit", { at: DOOR[0], kind: "Plank door" });
    s.wait(90);
    s.call("build.door", { at: DOOR[0], open: true });
    s.wait(150);
  });
}

export default { about: "a window cut and shuttered, a door fitted and opened, in the built cabin", sandbox: SANDBOX, clip: true, script } satisfies Scenario;
