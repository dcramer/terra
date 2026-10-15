// Edits to the castle, a base of some 140,000 cubes, timed to the screen (D185): blocks laid
// and built against a gate tower, holes carved in the curtain wall, then the tower's foot
// knocked out and the tower brought down. The castle is the player's base here, put up whole
// (`build.example`), so each edit takes the hammer's own path to the screen; the trace says
// how long each took (`Build: change on screen`). The castle's kinds are its own, so the
// set only names the blocks laid.

import type { KitSet, Scenario, Script, Vec3 } from "./script.ts";

/// The east gate tower, in cubes: sixteen a side, eighteen metres high.
const [TX0, TX1, TZ0, TZ1] = [3, 18, -14, 1];
const TOWER: Vec3 = [5.5, 0, -3.25];

function script(s: Script, set: KitSet) {
  s.stage("castle", "The castle, put up whole as the base", () => {
    s.call("build.example", { name: "castle" });
    s.orbit(TOWER, { radius: 34, height: 12, from: 140, to: 175, frames: 360, look: 7 });
    s.wait(360);
  });

  s.stage("place", "Blocks laid and built against the gate tower, one at a time", () => {
    s.glide([12, 3.2, 8], [7, 0.8, 0.8], 120);
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
    s.glide([-12, 3.5, 9], [-14, 2.5, -0.5], 120);
    s.wait(120);
    for (let i = 0; i < 4; i++) {
      s.call("build.carve", { at: [-34 + i * 6, 4 + (i % 2) * 2, -1], brush: 2 });
      s.wait(20);
      s.fill(`carve ${i}`, { seconds: 1.5, gap: 10 });
      s.wait(40);
    }
  });

  s.stage("tower", "The gate tower's foot knocked out: the tower comes down", () => {
    s.glide([34, 10, 26], [6, 7, -4], 150);
    s.wait(180);
    s.call("build.damage", { at: [TX0, 0, TZ0], to: [TX1, 1, TZ1] });
    s.orbit(TOWER, { radius: 30, height: 9, from: 52, to: 20, frames: 420, look: 3 });
    s.wait(420);
  });
}

export default {
  about: "the castle, 140,000 cubes, as the base: blocks placed, holes carved and a gate tower knocked down, each edit timed to the screen",
  sandbox: ["--experiment", "play", "--level", "yard", "--free-build", "--at", "8,24", "--face", "-30,24"],
  script,
} satisfies Scenario;
