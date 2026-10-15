// The shapes film: every shape a cube of a set's kit can be cut to (terra_build's `Shape`,
// each that faces a way in all four facings), laid and built in two rows on a floor of it,
// and a wall of it carved by each brush (D226, D231).

import type { KitSet, Scenario, Script } from "./script.ts";

const FACINGS = ["North", "East", "South", "West"];
const SHAPES: unknown[] = [
  "Whole",
  "Half",
  "UpperHalf",
  "Post",
  ...["Stair", "Slope", "InvertedStair", "InvertedSlope", "OuterCorner", "InnerCorner"].flatMap((shape) => FACINGS.map((facing) => ({ [shape]: facing }))),
];
/// The two rows, a cube apart between shapes, in cubes.
const ROWS = [-8, -6];
const FIRST_X = -12;

function script(s: Script, set: KitSet) {
  const kind = set.wall(0);
  const half = Math.ceil(SHAPES.length / 2);

  s.stage("floor", "A floor to stand them on", () => {
    s.call("storage.place", { item: "Claim stone", at: [0, 0, 0] });
    s.baseMovesAt(s.frame + 1, [0, -0.465, 0]);
    s.wait(2);
    s.call("build.lay", { what: "floor", min: [FIRST_X - 1, 0, -13], max: [FIRST_X + 2 * half, 0, -5], kind });
    s.wait(2);
    s.call("build.fill", { steps: 100000 });
    s.cut([-4, 4.5, 2], [1, 0, -3.5]);
    s.glide([2, 4.5, 3], [1, 0, -3.5], 240);
    s.wait(240);
  });

  s.stage("shapes", `Every shape in ${kind}`, () => {
    SHAPES.forEach((shape, i) => {
      const at = [FIRST_X + 2 * (i % half), 0, ROWS[Math.floor(i / half)]];
      s.call("build.lay", { what: "block", min: at, max: at, kind, shape });
    });
    s.wait(30);
    s.fill("shapes", { seconds: 8, gap: 8 });
    // Along the rows, low, from the west end to the east.
    s.cut([-6.5, 1.4, -1], [-5, 0.3, -3.5]);
    s.glide([6.5, 1.4, -1], [8, 0.3, -3.5], 900);
    s.wait(900);
  });

  s.stage("carve", "A wall carved by each brush", () => {
    s.call("build.lay", { what: "block", min: [FIRST_X, 1, -12], max: [FIRST_X + 9, 4, -12], kind });
    s.wait(2);
    s.call("build.fill", { steps: 100000 });
    s.glide([-3.5, 2.5, 0], [-3.5, 1.2, -6], 150);
    s.wait(150);
    for (const [x, brush] of [
      [FIRST_X + 1, 1],
      [FIRST_X + 3, 2],
      [FIRST_X + 6, 4],
    ]) {
      s.call("build.carve", { at: [x, 2, -12], brush });
      s.wait(20);
      s.fill(`carve ${brush}`, { seconds: 2, gap: 8 });
      s.wait(60);
    }
    s.wait(120);
  });
}

export default {
  about: "every cube shape of a set's kit in all four facings, and a wall carved by each brush",
  sandbox: ["--experiment", "play", "--free-build", "--time", "10", "--at", "0,3", "--face", "-8,14"],
  script,
} satisfies Scenario;
