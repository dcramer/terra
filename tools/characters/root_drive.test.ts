// root_drive.ts on a root keyed by hand: the drive goes as far as the root, at the speeds it
// moves, and two clips joined go one after the other.
// node --test tools/characters/root_drive.test.ts

import assert from "node:assert/strict";
import { test } from "node:test";
import { Glb } from "./glb.ts";
import { glbBytes } from "./glb_clips.ts";
import { driven, rootDrive } from "./root_drive.ts";

/** A file of one node "root" and clips moving it ahead (+Z): each [times, distances]. */
function rooted(clips: Record<string, [number[], number[]]>): Glb {
  const floats: number[] = [];
  const accessors: object[] = [];
  const views: object[] = [];
  const put = (values: number[], type: string) => {
    views.push({ buffer: 0, byteOffset: floats.length * 4, byteLength: values.length * 4 });
    floats.push(...values);
    const count = type === "SCALAR" ? values.length : values.length / 3;
    accessors.push({ bufferView: views.length - 1, componentType: 5126, count, type, ...(type === "SCALAR" ? { min: [Math.min(...values)], max: [Math.max(...values)] } : {}) });
    return accessors.length - 1;
  };
  const animations = Object.entries(clips).map(([name, [times, ahead]]) => ({
    name,
    samplers: [{ input: put(times, "SCALAR"), output: put(ahead.flatMap((z) => [0, 0, z]), "VEC3"), interpolation: "LINEAR" }],
    channels: [{ sampler: 0, target: { node: 0, path: "translation" } }],
  }));
  const bin = Buffer.alloc(floats.length * 4);
  floats.forEach((x, k) => bin.writeFloatLE(x, k * 4));
  const json = { asset: { version: "2.0" }, scenes: [{ nodes: [0] }], nodes: [{ name: "root" }], accessors, bufferViews: views, buffers: [{ byteLength: bin.length }], animations };
  return new Glb(glbBytes(json, bin));
}

test("the drive goes as far as the root, slow then fast as it moves", () => {
  const g = rooted({ Lunge: [[0, 0.5, 1], [0, 0.5, 2]] });
  const { drive, ahead } = rootDrive(g, ["Lunge"], 1 / 30);
  assert.ok(Math.abs(ahead - 2) < 1e-5);
  assert.ok(Math.abs(driven(drive, 1) - 2) < 0.03, `the drive goes ${driven(drive, 1)} m`);
  const at = (t: number) => drive.reduce((best, k) => (Math.abs(k[0] - t) < Math.abs(best[0] - t) ? k : best))[1];
  assert.ok(Math.abs(at(0.25) - 1) < 1e-3 && Math.abs(at(0.75) - 3) < 1e-3, `1 m/s then 3: ${at(0.25)}, ${at(0.75)}`);
});

test("clips joined are driven one after the other", () => {
  const g = rooted({ Blow: [[0, 0.5], [0, 1]], Back: [[0, 1], [0, -0.5]] });
  const { drive, ahead } = rootDrive(g, ["Blow", "Back"], 1 / 30);
  assert.ok(Math.abs(ahead - 0.5) < 1e-5);
  assert.ok(Math.abs(driven(drive, 1.5) - 0.5) < 0.03, `the drive goes ${driven(drive, 1.5)} m`);
  assert.ok(drive.every(([t, v]) => (t < 0.5 ? Math.abs(v - 2) < 1e-3 : Math.abs(v + 0.5) < 1e-3)), "2 m/s for the blow, back at 0.5 after");
});

test("a number part is a pause the body stands through, as glb_clips joins it", () => {
  const g = rooted({ Blow: [[0, 0.5], [0, 1]] });
  const { drive, ahead } = rootDrive(g, ["0.25", "Blow"], 1 / 30);
  assert.ok(Math.abs(ahead - 1) < 1e-5);
  assert.ok(Math.abs(driven(drive, 0.75) - 1) < 0.03, `the drive goes ${driven(drive, 0.75)} m`);
  assert.ok(drive.every(([t, v]) => (t <= 0.25 ? v === 0 : Math.abs(v - 2) < 1e-3)), "still for the pause, then 2 m/s");
});
