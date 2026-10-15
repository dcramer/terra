// Edits to the castle, a base of some 140,000 cubes, timed to the screen (D185): blocks laid
// and built against a gate tower, holes carved in the curtain wall, then the tower cut from
// the walls either side and its foot knocked out, so it comes down. The castle is the
// player's base here, put up whole (`build.example`), so each edit takes the hammer's own path
// to the screen; the trace says how long each took (`Build: change on screen`). The castle's
// kinds are its own; the set names the blocks laid.

import type { KitSet, Scenario, Script, Vec3 } from "./script.ts";

/// The east gate tower, in cubes: sixteen a side, eighteen metres high, set into the
/// curtain's south face, which runs six cubes thick from z -6 to -1 and twenty-four high.
const [TX0, TX1, TZ0, TZ1] = [3, 18, -14, 1];
const TOWER: Vec3 = [5.5, 0, -3.25];

function script(s: Script, set: KitSet) {
  // Outside the castle: its curtain and towers fill everything north of the gate's face.
  s.solid([-41, -1, -81], [41, 20, 1.2]);

  s.stage("castle", "The castle, put up whole as the base", () => {
    s.call("build.example", { name: "castle" });
    s.orbit(TOWER, { radius: 34, height: 12, from: -35, to: 25, frames: 420, look: 7 });
    s.wait(420);
  });

  s.stage("place", "Blocks laid and built against the gate tower, one at a time", () => {
    s.glide([13, 3.2, 9], [7, 0.8, 0.8], 120);
    s.wait(90);
    for (let i = 0; i < 6; i++) {
      const at = [TX0 + 2 + i * 2, i % 2, TZ1 + 1];
      s.call("build.lay", { what: "block", min: at, max: at, kind: set.wall() });
      s.wait(30);
      s.fill(`block ${i}`, { seconds: 1, gap: 12 });
      s.wait(30);
    }
  });

  s.stage("carve", "Holes carved in the curtain wall", () => {
    s.glide([-12, 3.5, 9], [-14, 2.5, -0.5], 150);
    s.wait(150);
    for (let i = 0; i < 4; i++) {
      s.call("build.carve", { at: [-34 + i * 6, 4 + (i % 2) * 2, -1], brush: 2 });
      s.wait(20);
      s.fill(`carve ${i}`, { seconds: 1.5, gap: 10 });
      s.wait(40);
    }
  });

  s.stage("tower", "The gate tower cut from the walls and its foot knocked out: it comes down", () => {
    s.glide([34, 10, 30], [6, 7, -4], 150);
    s.wait(180);
    // Cut from the curtain east of it, and over the gate from the other tower, standing
    // still on its foot; then the foot knocked out.
    s.call("build.damage", { at: [TX1 + 1, 0, -6], to: [TX1 + 2, 23, -1] });
    s.wait(60);
    s.call("build.damage", { at: [-3, 10, -6], to: [2, 23, -1] });
    s.wait(90);
    s.call("build.damage", { at: [TX0, 0, TZ0], to: [TX1, 1, TZ1] });
    s.orbit(TOWER, { radius: 32, height: 9, from: 48, to: 15, frames: 480, look: 3 });
    s.wait(480);
  });
}

export default {
  about: "the castle, 140,000 cubes, as the base: blocks placed, holes carved and a gate tower brought down, each edit timed to the screen",
  sandbox: ["--experiment", "play", "--level", "yard", "--free-build", "--at", "6,9", "--face", "6,30"],
  script,
} satisfies Scenario;
