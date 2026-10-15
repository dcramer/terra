// node --test tools/kits/preview.test.ts
import { strict as assert } from "node:assert";
import { test } from "node:test";
import { HOLD, ORIGIN, run, views } from "./preview.ts";

test("a material's preview is asked of the kit sheet alone, and each view pictured once it is held", () => {
  const { args, frames } = run("river_cobble", "block", "18", "shots/preview/river_cobble-18");
  assert.equal(args[args.indexOf("--sheet-rows") + 1], "river_cobble: preview");
  assert.equal(args[args.indexOf("--time") + 1], "18");
  const vantages = args.filter((_, i) => args[i - 1] === "--vantage");
  assert.equal(vantages.length, views("block").length);
  // Each view from its frame, pictured just before the next one's.
  vantages.forEach((vantage, i) => assert.ok(vantage.endsWith(`@${i * HOLD}`), vantage));
  frames.forEach((frame, i) => assert.ok(frame > i * HOLD && frame < (i + 1) * HOLD));
  assert.equal(args[args.indexOf("--shots") + 1], `shots/preview/river_cobble-18@${frames.join(",")}`);
});

test("its views are taken from 10 m and 2 m, at the preview's place west of the start", () => {
  for (const pattern of ["block", "coursed", "frame", "roof"]) {
    const all = views(pattern);
    assert.ok(all.some(([name]) => name.startsWith("10 m")) && all.some(([name]) => name.startsWith("2 m")), pattern);
    for (const [name, [ex, ey, ez, ax, ay, az]] of all) {
      // West of the start, where the sheet lays it, eye above the ground, looking at it.
      assert.ok(ex < 0 && ax < ORIGIN[0] + 20 && ax > ORIGIN[0] - 5, `${pattern} ${name}`);
      assert.ok(ey > 0 && ay > 0, `${pattern} ${name}`);
      const distance = Math.hypot(ex - ax, ey - ay, ez - az);
      const near = name.startsWith("2 m");
      assert.ok(near ? distance < 5 : distance > 9, `${pattern} ${name}: ${distance.toFixed(1)} m`);
    }
  }
});
