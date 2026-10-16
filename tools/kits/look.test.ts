// node --test tools/kits/look.test.ts
import { strict as assert } from "node:assert";
import { test } from "node:test";
import { DISTANCES, FIRST, HOLD, finestOctave, judge, lights, type Measured, parseTexels, percentile, run, type Targets } from "./look.ts";
import { ORIGIN } from "./preview.ts";

/** A picture of values made by a rule, a pixel at a time. */
const picture = (width: number, height: number, value: (x: number, y: number) => number) => {
  const values = new Float64Array(width * height);
  for (let y = 0; y < height; y++) for (let x = 0; x < width; x++) values[y * width + x] = value(x, y);
  return values;
};

test("sharpness: a one-pixel checkerboard is all finest octave, a flat or straight ramp none", () => {
  // Halving a checkerboard leaves grey everywhere, so all of its contrast is lost: 100%.
  assert.ok(Math.abs(finestOctave(picture(64, 64, (x, y) => (x + y) % 2), 64, 64) - 100) < 1e-9);
  // A ramp halves and doubles back to itself, but at its clamped edges.
  assert.ok(finestOctave(picture(64, 64, (x) => x / 64), 64, 64) < 3);
  assert.equal(finestOctave(picture(16, 16, () => 0.5), 16, 16), 0);
});

test("sharpness: blurring a picture lowers it", () => {
  let seed = 7;
  const random = () => ((seed = (seed * 1103515245 + 12345) % 2147483648) / 2147483648);
  const sharp = picture(96, 96, () => random());
  const blurred = picture(96, 96, (x, y) => {
    let sum = 0;
    for (let dy = -1; dy <= 1; dy++) for (let dx = -1; dx <= 1; dx++) sum += sharp[Math.min(95, Math.max(0, y + dy)) * 96 + Math.min(95, Math.max(0, x + dx))];
    return sum / 9;
  });
  assert.ok(finestOctave(sharp, 96, 96) > 1.5 * finestOctave(blurred, 96, 96));
});

test("percentiles", () => {
  const values = Array.from({ length: 101 }, (_, i) => 100 - i);
  assert.equal(percentile(values, 5), 5);
  assert.equal(percentile(values, 50), 50);
  assert.equal(percentile(values, 95), 95);
});

test("every light measures its surface square on from 1, 2 and 5 m, and a grey card", () => {
  for (const pattern of ["Logs", "Masonry", "Roof"]) {
    for (const light of lights(pattern)) {
      const surfaces = light.views.filter((view) => view.measure === "surface");
      assert.deepEqual(surfaces.map((view) => view.distance), DISTANCES, `${pattern} ${light.name}`);
      for (const view of surfaces) {
        const apart = Math.hypot(...view.eye.map((e, i) => e - view.at[i]));
        assert.ok(Math.abs(apart - view.distance!) < 1e-6, `${pattern} ${light.name} ${view.name}`);
      }
      assert.equal(light.views.filter((view) => view.measure === "card").length, 1, `${pattern} ${light.name}`);
    }
    assert.deepEqual(lights(pattern).map((light) => light.name), ["noon", "shade", "evening"]);
  }
});

test("an hour's run shoots its lights' views, each once held, and stands its cards first", () => {
  const noon = run("logs", "Logs", 12, "calls.jsonl", "shots/look/logs-12");
  const expected = lights("Logs").filter((light) => light.hour === 12).flatMap((light) => light.views);
  assert.equal(noon.views.length, expected.length);
  const vantages = noon.args.filter((_, i) => noon.args[i - 1] === "--vantage");
  assert.equal(vantages.length, noon.views.length);
  // Each view from its frame, shot before the next one starts.
  vantages.forEach((vantage, i) => {
    const from = Number(vantage.split("@")[1]);
    assert.ok(noon.frames[i] > from && noon.frames[i] < (i === 0 ? FIRST : from + HOLD), vantage);
  });
  // Moved from the preview's metres to the start's, as the kit sheet lays it.
  const first = vantages[0].split("@")[0].split(",").map(Number);
  assert.deepEqual(first.slice(0, 3), [noon.views[0].eye[0] + ORIGIN[0], noon.views[0].eye[1], noon.views[0].eye[2] + ORIGIN[1]]);
  assert.equal(noon.args[noon.args.indexOf("--time") + 1], "12");
  assert.equal(noon.args[noon.args.indexOf("--sheet-rows") + 1], "logs: preview");
  const cards = noon.calls.trim().split("\n").map((line) => JSON.parse(line));
  assert.ok(cards.length >= 2 && cards.every((call) => call.method === "sandbox.card" && call.frame < 10));
  const evening = run("logs", "Logs", 18, "calls.jsonl", "x");
  assert.ok(evening.views.every((view) => lights("Logs").find((light) => light.name === "evening")!.views.includes(view)));
});

const targets: Targets = {
  surface: "Log",
  density: 1000,
  sharpness: [15, 15, 12],
  lit: [0.3, 1.5],
  shade: [0.25, 1.5],
  values: [30, 160],
  stretch: { smeared: 2, streaked: 0.5, roles: { Piece: [45, 4] } },
};
const good: Measured = {
  density: [2048, 1024],
  sharpness: [20, 18, 14],
  values: { p5: 40, p50: 90, p95: 150 },
  lit: [0.6, 0.7],
  shade: 0.4,
  cards: { noon: { redOverBlue: 1.04, seen: [97, 95, 95] }, shade: { redOverBlue: 0.32, seen: [28, 38, 54] }, evening: { redOverBlue: 2.3, seen: [140, 109, 96] } },
  stretch: new Map([
    ["Log", [0.8, 0]],
    ["Piece", [41, 3]],
  ]),
};

test("a material within every target holds; each one missed fails alone", () => {
  assert.ok(judge(good, targets).every((line) => line.ok));
  const fails = (measured: Measured) => judge(measured, targets).filter((line) => !line.ok).map((line) => line.what);
  assert.deepEqual(fails({ ...good, density: [2048, 512] }), ["texel density"]);
  assert.deepEqual(fails({ ...good, sharpness: [20, 10, 14] }), ["sharpness, 2 m"]);
  assert.deepEqual(fails({ ...good, shade: 0.2 }), ["against the grey card, shade"]);
  assert.deepEqual(fails({ ...good, values: { p5: 80, p50: 90, p95: 100 } }), ["value range, 2 m noon"]);
  // The noon card is held neutral (D370); the shade's blue and the evening's amber are the light's.
  assert.deepEqual(fails({ ...good, cards: { ...good.cards, noon: { redOverBlue: 1.4, seen: [140, 120, 110] } } }), ["grey card red/blue, noon"]);
  // A piece's atlas has its own bound; the log's is the material's.
  assert.deepEqual(fails({ ...good, stretch: new Map([["Log", [3, 0]], ["Piece", [41, 3]]]) }), ["stretch, Log"]);
});

test("what was not measured fails, so a bench that saw nothing never passes", () => {
  const lines = judge({ sharpness: [], lit: [], cards: {} }, targets);
  assert.ok(lines.length > 5 && lines.every((line) => !line.ok));
});

test("reads texels' table by kit and texture set", () => {
  const text = "kit              material     area m²    >1.5:1    >2.5:1\nlogs             Log          31148.4      0.8%      0.0%\nshakes           Piece         1969.3     41.2%      3.0%\n    0.20 m² at 1.0,2.0,3.0 facing 0,1,0, up to 3.1:1, in 1: x\n";
  const read = parseTexels(text);
  assert.deepEqual(read.get("logs")?.get("Log"), [0.8, 0]);
  assert.deepEqual(read.get("shakes")?.get("Piece"), [41.2, 3]);
  assert.equal(read.size, 2);
});
