#!/usr/bin/env node
// Tunables reach the picture live (D547): one running sandbox, its clock stopped, the sky shot
// looking away from the sun on the high preset, where the volumetric clouds draw; then a
// tunable the clouds' march reads is set over the remote (`tune.set`) and the sky shot again,
// then put back and shot a third time. The sky must change when the value does (a share of the
// sky band's pixels past CHANGED) and come back when it is put back, or it exits 1: a value
// that was copied once at the start, or never reached the shader, fails.
//
//   node tools/sandbox/tunecheck.ts [--out ~/.cache/terra-review/tunables] [--name sky.cloud.heaped_cover] [--value 0.25]
//
// Under WSL it runs on the Windows GPU (`r start --windows --release`); start it as
// `cargo xtask gpu still -- node tools/sandbox/tunecheck.ts`. A line a stage; the three shots
// and a sheet of them (where ImageMagick is installed) in --out. Node 22.18 or later.

import { spawnSync } from "node:child_process";
import { mkdirSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import { parseArgs } from "node:util";
import * as sandbox from "./client.ts";
import { CHANGED, type Grey, grey } from "./frames.ts";
import { readPng } from "./png.ts";

const { values } = parseArgs({
  options: {
    out: { type: "string", default: join(homedir(), ".cache/terra-review/tunables") },
    name: { type: "string", default: "sky.cloud.heaped_cover" },
    value: { type: "string", default: "0.25" },
  },
});
const out = values.out as string;
const name = values.name as string;
const value = Number(values.value);
/// Looking away from the sun at 15:00, as the sky's stages judge clouds (sky-stage2 handoff).
const VIEW = ["--experiment", "play", "--fresh", "--time", "15", "--quality", "high", "--vantage-ground", "--vantage", "0,4,0,-600,450,300"];
/// The sky over the horizon, rows of the 1080-row picture (the stage's crop, 1920x500+0+90).
const SKY = [90, 590];
/// The sky must change in at least this share of its pixels when the value does, and in no
/// more than the other when it is put back (a still clock leaves only TAA's last noise).
const MOVED = 2;
const BACK = 0.3;
/// Real time a value is drawn for before its shot.
const SETTLE_MS = 1500;

function changedShare(a: Grey, b: Grey): number {
  const [top, bottom] = SKY.map((row) => Math.round((row * a.height) / 1080));
  let n = 0;
  let total = 0;
  for (let y = top; y < bottom; y++) {
    for (let x = 0; x < a.width; x++) {
      const i = y * a.width + x;
      total++;
      if (Math.abs(a.v[i] - b.v[i]) > CHANGED) n++;
    }
  }
  return (100 * n) / total;
}

mkdirSync(out, { recursive: true });
const shots = { before: join(out, "before.png"), tuned: join(out, "tuned.png"), back: join(out, "back.png") };
let failed = false;
try {
  await sandbox.start(VIEW, { windows: sandbox.onWsl, release: true });
  // Let the ground, the light probes and TAA settle, then hold the clock so the clouds stand.
  await sandbox.step(240);
  const t = await sandbox.tunable(name);
  console.log(`✓ started: ${sandbox.describeTunable(t)}`);
  // The clock stays stopped from here: frames are still drawn, so TAA settles on each value
  // in the real second it is given, and the clouds stand where they were.
  await sandbox.sleep(SETTLE_MS);
  await sandbox.shot(shots.before);
  const { was } = await sandbox.tune(name, value);
  await sandbox.sleep(SETTLE_MS);
  await sandbox.shot(shots.tuned);
  console.log(`✓ set ${name} ${was} -> ${(await sandbox.tunable(name)).value} over the remote, in the same run`);
  await sandbox.tune(name, "default");
  await sandbox.sleep(SETTLE_MS);
  await sandbox.shot(shots.back);
} finally {
  await sandbox.stop().catch(() => {});
}
const [before, tuned, back] = [shots.before, shots.tuned, shots.back].map((path) => grey(readPng(path)));
const moved = changedShare(before, tuned);
const returned = changedShare(before, back);
const verdict = (ok: boolean) => (ok ? "✓" : "✗");
console.log(`${verdict(moved >= MOVED)} the sky changed with the value: ${moved.toFixed(2)}% of its pixels (at least ${MOVED}%)`);
console.log(`${verdict(returned <= BACK)} and came back with the default: ${returned.toFixed(2)}% still differ (at most ${BACK}%)`);
failed = moved < MOVED || returned > BACK;
const sheet = join(out, "sheet.jpg");
const [tool, words] = sandbox.imageMagick([shots.before, shots.tuned, shots.back, "-resize", "640x", "-append", "-quality", "85", sheet]);
const made = spawnSync(tool, words);
console.log(made.status === 0 ? `  before, ${name}=${value}, back: ${sheet}` : `  shots in ${out}`);
process.exit(failed ? 1 : 0);
