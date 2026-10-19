import assert from "node:assert/strict";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { readPng, writePng } from "./png.ts";

test("a picture written with per-row filters reads back exactly", () => {
  const [width, height] = [37, 23];
  const rgba = new Uint8Array(width * height * 4);
  let seed = 7;
  for (let i = 0; i < rgba.length; i++) {
    // Smooth ramps in some rows, noise in others, so every filter is chosen somewhere.
    seed = (seed * 1103515245 + 12345) >>> 0;
    const y = Math.floor(i / (width * 4));
    rgba[i] = y % 3 === 0 ? seed >>> 24 : (i * 3 + y * 5) & 255;
  }
  const path = join(mkdtempSync(join(tmpdir(), "png-")), "p.png");
  writePng(path, { width, height, rgba });
  assert.deepEqual(readPng(path), { width, height, rgba });
});
