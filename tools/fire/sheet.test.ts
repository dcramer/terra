// The fire sheet's lit-ground number on pictures whose answers are known by construction.
import assert from "node:assert/strict";
import { test } from "node:test";
import type { Picture } from "../sandbox/png.ts";
import { judge, type Metrics } from "./measure.ts";
import { measure } from "./measure.ts";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { calls, dirAt, FIRES, firesAt, floored, HELD_SKY, PRESETS, sampled, sandboxArgs, shotsOf, surround } from "./sheet.ts";

test("every sheet run is staged under the held sky: the dome's 2D clouds, by a switch volume.rs owns", () => {
  const held = (args: string[]) => args.some((a, i) => a === "--tune" && args[i + 1] === "sky.cloud.volume=0");
  for (const hour of ["noon", "dusk", "night"]) for (const preset of PRESETS) assert.ok(held(sandboxArgs(hour, preset, "d", "c.jsonl")), `${hour} ${preset || "play"}`);
  assert.deepEqual(HELD_SKY, ["--tune", "sky.cloud.volume=0"]);
  // A run ends on a name no plugin registered (tune.rs), so it must be the switch's own, read as one.
  const volume = readFileSync(new URL("../../crates/terra_world/src/sky/volume.rs", import.meta.url), "utf8");
  const name = /pub static (\w+): Tunable = Tunable::new\(\s*"sky\.cloud\.volume"/.exec(volume)?.[1];
  assert.ok(name, "volume.rs declares sky.cloud.volume");
  assert.match(volume, new RegExp(`tune::register\\(&\\[[^\\]]*&${name}\\b`));
  assert.match(volume, new RegExp(`${name}\\.get\\(\\) >= 0\\.5`));
});

/// A picture of one colour, with a flame-white box and, if asked, the band below it lit.
function picture(ground: [number, number, number], lit: boolean): Picture {
  const [width, height] = [90, 90];
  const rgba = new Uint8Array(width * height * 4);
  for (let y = 0; y < height; y++)
    for (let x = 0; x < width; x++) {
      const inFlame = x >= 30 && x < 60 && y >= 20 && y < 50;
      const below = y >= 50 && lit;
      const [r, g, b] = inFlame ? [255, 255, 255] : below ? ground : [0, 0, 0];
      rgba.set([r, g, b, 255], 4 * (y * width + x));
    }
  return { width, height, rgba };
}

const BOX = { x: 30, y: 20, w: 30, h: 30 };

test("the band round a fire leaves its flame out, so a bright flame over dark ground reads dark", () => {
  assert.equal(surround(picture([0, 0, 0], false), BOX).light, 0);
});

test("ground the fire lights orange reads lit and warm", () => {
  const { light, warmth } = surround(picture([255, 120, 40], true), BOX);
  assert.ok(light > 0.1, `${light}`);
  assert.ok(warmth > 5, `${warmth}`);
  const grey = surround(picture([128, 128, 128], true), BOX);
  assert.ok(Math.abs(grey.warmth - 1) < 1e-9);
});

test("a fire's own floor lets its soot under the photographs' least, its most and the other numbers still the class's", () => {
  const bands = { soot: { median: 0.4, min: 0.13, max: 0.92 }, dim: { median: 20, min: 5, max: 63 } };
  const m = { soot: 0.05, dim: -20 } as Metrics;
  const by = (b: Parameters<typeof judge>[1]) => Object.fromEntries(judge(m, b).map((h) => [h.called, h.holds]));
  assert.deepEqual(by(bands), { "dim %": false, "soot/flame": false });
  assert.deepEqual(by(floored(bands, { soot: 0 })), { "dim %": false, "soot/flame": true });
  assert.equal(judge({ soot: 1.2 } as Metrics, floored(bands, { soot: 0 })).find((h) => h.called === "soot/flame")?.holds, false);
});

test("a fire's own ceiling lets its pale over the photographs' most, its least and the other numbers still the class's", () => {
  const bands = { pale: { median: 0.2, min: 0, max: 3.4 }, yellow: { median: 11, min: 1.6, max: 20.6 } };
  const m = { pale: 10, yellow: 30 } as Metrics;
  const by = (b: Parameters<typeof judge>[1]) => Object.fromEntries(judge(m, b).map((h) => [h.called, h.holds]));
  assert.deepEqual(by(bands), { "pale %": false, "yellow %": false });
  assert.deepEqual(by(floored(bands, {}, { pale: 12.2 })), { "pale %": true, "yellow %": false });
  assert.equal(judge({ pale: 15 } as Metrics, floored(bands, {}, { pale: 12.2 })).find((h) => h.called === "pale %")?.holds, false);
  assert.equal(FIRES.find((f) => f.name === "campfire")?.ceilings, undefined);
});

test("a row's numbers are the mean of its shots', each measured in its own crop", () => {
  // Two shots of a flame, the second taller and higher, over differently lit ground.
  const flame = (top: number, h: number, ground: [number, number, number]) => {
    const pic = picture(ground, true);
    for (let y = top; y < top + h; y++)
      for (let x = 30; x < 60; x++) pic.rgba.set([255, y % 3 ? 190 : 120, 40, 255], 4 * (y * pic.width + x));
    return { pic, box: { x: 30, y: top, w: 30, h } };
  };
  const shots = [flame(20, 30, [80, 40, 20]), flame(10, 40, [200, 100, 40])];
  const one = shots.map(({ pic, box }) => measure(pic, box));
  const { metrics, ground } = sampled(shots);
  assert.notEqual(one[0].yellow, one[1].yellow);
  for (const k of ["yellow", "orange", "soot", "fine", "flamePx"] as const) assert.ok(Math.abs(metrics[k] - (one[0][k] + one[1][k]) / 2) < 1e-9, k);
  const lights = shots.map(({ pic, box }) => surround(pic, box).light);
  assert.notEqual(lights[0], lights[1]);
  assert.ok(Math.abs(ground.light - (lights[0] + lights[1]) / 2) < 1e-12);
});

test("each fire is shot five times over two seconds, its own camera held through every shot", () => {
  const all = calls();
  for (const fire of FIRES) {
    const shots = shotsOf(fire);
    assert.ok(shots.length >= 5 && (shots.at(-1)! - shots[0]) / 60 >= 2, fire.name);
    for (const f of shots) {
      assert.ok(all.some((c) => c.frame === f && c.method === "fire.seen"), `${fire.name} ${f}`);
      const looks = all.filter((c) => c.frame === f && c.method === "build.look");
      assert.deepEqual(looks.map((c) => c.params), [{ from: fire.eye, at: fire.at }], `${fire.name} ${f}: one camera, its own`);
    }
  }
});

test("the breath is shot again at ultra, as the films draw it, in a run of its own, and play's run as before", () => {
  const ultra = firesAt("ultra");
  assert.deepEqual(
    ultra.map((f) => [f.class, f.box]),
    [
      ["flamethrower", "stream"],
      ["fireball", "ball"],
    ],
  );
  assert.deepEqual(PRESETS, ["", "ultra"]);
  const cards = sandboxArgs("dusk", "", "d", "c.jsonl");
  const marched = sandboxArgs("dusk", "ultra", "d/ultra", "c.jsonl");
  assert.ok(!cards.includes("--quality"));
  assert.deepEqual(marched.slice(marched.indexOf("--quality"), marched.indexOf("--quality") + 2), ["--quality", "ultra"]);
  // The marched run shoots its own fires alone, the breath lit before the first shot.
  const shotsAt = (args: string[]) => args[args.indexOf("--shots") + 1].split("@")[1].split(",").map(Number);
  assert.deepEqual(shotsAt(marched), shotsOf(ultra[0]));
  assert.ok(shotsAt(cards).every((f) => firesAt("").some((fire) => shotsOf(fire).includes(f))));
  assert.ok(calls(ultra).find((c) => c.method === "flight.breathe")!.frame < shotsOf(ultra[0])[0]);
  assert.equal(dirAt("noon", "ultra"), join("noon", "ultra"));
  assert.equal(dirAt("noon", ""), "noon");
});
