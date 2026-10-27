#!/usr/bin/env node
// The debug menu works as a person uses it (D81): one running sandbox started with
// --debug-menu, its clock held looking at the clouds away from the sun, as tunecheck.ts does.
// The backtick opens the menu (a key press, as a person's), and it must be drawn over the game;
// a click on the heaped-cover slider, where the menu says it drew it, must set the tunable so
// `tune.get` reads it, set by the menu, and change the sky beside the menu; a value set over the
// remote (`tune.set`) must show on the menu's slider; the slider's reset must put it back; the
// backtick must close it. Any step that fails exits 1, saying what it saw instead.
//
//   node tools/sandbox/menucheck.ts [--out ~/.cache/terra-review/debug-menu] [--name sky.cloud.heaped_cover] [--value 0.25] [--quality high|low]
//
// Under WSL it runs on the Windows GPU (`r start --windows --release`); start it as
// `cargo xtask gpu still -- node tools/sandbox/menucheck.ts`. A line a stage; the shots and a
// sheet of them (where ImageMagick is installed) in --out. Node 22.18 or later. At the low
// preset, whose sky has no heaped clouds, slide the drift: `--quality low --name
// sky.cloud.drift --value 2`. Both are continuous checks (tools/continuous/checks.json).

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
    out: { type: "string", default: join(homedir(), ".cache/terra-review/debug-menu") },
    name: { type: "string", default: "sky.cloud.heaped_cover" },
    value: { type: "string", default: "0.25" },
    quality: { type: "string", default: "high" },
  },
});
const out = values.out as string;
const name = values.name as string;
const value = Number(values.value);
/// tunecheck.ts's view: away from the sun at 15:00, where the volumetric clouds draw.
const VIEW = ["--experiment", "play", "--fresh", "--time", "15", "--quality", values.quality as string, "--vantage-ground", "--vantage", "0,4,0,-600,450,300", "--debug-menu"];
/// The sky over the horizon, as shares of the picture's height.
const SKY = [90 / 1080, 590 / 1080];
/// The frame-time readout (D77), whose numbers change from frame to frame, is above this, in
/// the screen's points (the menu stands at 116).
const READOUT = 100;
/// The menu's shadow, around it, in points.
const SHADOW = 24;
/// The menu must change most of what it covers when it opens, and little else beyond what the
/// game itself changes from one shot to the next (the far clouds' noise); the sky beside it
/// must change with the value (tunecheck's bar).
const COVERED = 30;
const ELSEWHERE = 0.5;
const MOVED = 2;
/// How near the slider's value must come to the one aimed at, as a share of its range: a
/// click lands to the pixel, a pixel being about a two-hundredth of the track.
const NEAR = 0.02;
const SETTLE_MS = 1500;

type Box = [number, number, number, number];
/// The share of pixels that changed within `rows` (shares of the height), inside the box or
/// outside it.
function changedShare(a: Grey, b: Grey, box: Box | null, inside: boolean, rows = [0, 1]): number {
  const [top, bottom] = rows.map((row) => Math.round(row * a.height));
  let n = 0;
  let total = 0;
  for (let y = top; y < bottom; y++) {
    for (let x = 0; x < a.width; x++) {
      const within = box !== null && x >= box[0] && x < box[0] + box[2] && y >= box[1] && y < box[1] + box[3];
      if (within !== inside) continue;
      const i = y * a.width + x;
      total++;
      if (Math.abs(a.v[i] - b.v[i]) > CHANGED) n++;
    }
  }
  return total ? (100 * n) / total : 0;
}

const failures: string[] = [];
const verdict = (ok: boolean, line: string) => {
  console.log(`${ok ? "✓" : "✗"} ${line}`);
  if (!ok) failures.push(line);
};

mkdirSync(out, { recursive: true });
const shots = { closed: join(out, "closed.png"), still: join(out, "closed-again.png"), open: join(out, "open.png"), slid: join(out, "slid.png"), remote: join(out, "remote.png") };
/// Where the menu was drawn, in the screen's points, and the screen's size in them.
let drawn: sandbox.Area | null = null;
let screen = [1, 1];
const row = (menu: sandbox.Menu) => menu.shown?.tunables.find((t) => t.name === name);
try {
  await sandbox.start(VIEW, { windows: sandbox.onWsl, release: true });
  await sandbox.step(240);
  const t = await sandbox.tunable(name);
  console.log(`✓ started: ${sandbox.describeTunable(t)}`);
  await sandbox.sleep(SETTLE_MS);
  await sandbox.shot(shots.closed);
  await sandbox.sleep(SETTLE_MS);
  await sandbox.shot(shots.still);

  await sandbox.press("backtick");
  await sandbox.waitFor("Debug menu: open", { timeout: 20 });
  let menu = await sandbox.waitUntil("the menu drawn with its slider", () => sandbox.menu({ search: name }), (m) => row(m) !== undefined, 20);
  // A window's first frame only measures it; read where it is once laid out.
  await sandbox.sleep(500);
  menu = await sandbox.menu();
  const window = menu.window as sandbox.Area;
  await sandbox.sleep(SETTLE_MS);
  await sandbox.shot(shots.open);
  drawn = window;
  screen = menu.screen;
  const shotWidth = readPng(shots.open).width;
  console.log(`✓ the backtick opened it, on ${menu.page}, at ${window.map(Math.round).join(",")} (left, top, width, height) of a ${screen.map(Math.round).join("x")}-point screen, shot ${shotWidth} pixels wide`);

  const slider = row(menu)!;
  const [x, y] = sandbox.sliderSpot(t, slider, value);
  await sandbox.call("sandbox.click", { x, y, frames: 3 });
  await sandbox.waitFor(`Tune: ${name}`, { timeout: 20 });
  const slid = await sandbox.tunable(name);
  const near = Math.abs(slid.value - value) <= NEAR * (t.max - t.min);
  verdict(near && slid.set_by === "debug menu", `a click on its slider at ${x},${y} set ${name} to ${slid.value} (aimed at ${value}), by ${slid.set_by}, as tune.get reads it`);
  await sandbox.sleep(SETTLE_MS);
  await sandbox.shot(shots.slid);

  const other = Math.round((t.min + 0.75 * (t.max - t.min)) * 100) / 100;
  await sandbox.tune(name, other);
  menu = await sandbox.waitUntil("the menu showing the remote's value", () => sandbox.menu(), (m) => row(m)?.value === other, 10).catch(() => sandbox.menu());
  verdict(row(menu)?.value === other && row(menu)?.set_by === "remote", `tune.set ${name} ${other} shows on the menu: ${row(menu)?.value}, by ${row(menu)?.set_by}`);
  await sandbox.sleep(SETTLE_MS);
  await sandbox.shot(shots.remote);

  const reset = row(menu)!.reset;
  await sandbox.call("sandbox.click", { x: reset[0] + reset[2] / 2, y: reset[1] + reset[3] / 2 });
  const back = await sandbox.waitUntil(`${name} reset`, () => sandbox.tunable(name), (b) => b.value === b.default, 10).catch(() => sandbox.tunable(name));
  verdict(back.value === back.default && back.set_by === "default", `its reset put it back to ${back.value} (default ${back.default}), by ${back.set_by}`);

  await sandbox.press("backtick");
  const closed = await sandbox.waitUntil("the menu closed", () => sandbox.menu(), (m) => !m.open, 10).catch(() => sandbox.menu());
  verdict(!closed.open, `the backtick closed it again`);
} finally {
  await sandbox.stop().catch(() => {});
}
const [closed, still, open, slid] = [shots.closed, shots.still, shots.open, shots.slid].map((path) => grey(readPng(path)));
// What is compared is the shots halved (`grey`): the menu's points are scaled to its pixels.
const scale = closed.width / screen[0];
const inPixels = (area: number[]) => area.map((v) => Math.round(v * scale)) as Box;
const box = drawn && inPixels(drawn);
const shadowed = drawn && inPixels([drawn[0] - SHADOW, drawn[1] - SHADOW, drawn[2] + 2 * SHADOW, drawn[3] + 2 * SHADOW]);
const belowReadout = [READOUT / screen[1], 1];
const covered = changedShare(closed, open, box, true);
const elsewhere = changedShare(closed, open, shadowed, false, belowReadout);
const noise = changedShare(closed, still, shadowed, false, belowReadout);
verdict(covered >= COVERED && elsewhere <= noise + ELSEWHERE, `opened, it is drawn over the game: ${covered.toFixed(1)}% of where it stands changed (at least ${COVERED}%), ${elsewhere.toFixed(2)}% elsewhere (at most ${ELSEWHERE}% over the game's own ${noise.toFixed(2)}% from shot to shot)`);
const moved = changedShare(open, slid, box, false, SKY);
verdict(moved >= MOVED, `the sky beside it changed with the slider: ${moved.toFixed(2)}% of its pixels (at least ${MOVED}%)`);
const sheet = join(out, "sheet.jpg");
const [tool, words] = sandbox.imageMagick(["montage", shots.closed, shots.open, shots.slid, shots.remote, "-tile", "2x2", "-geometry", "960x540+4+4", "-quality", "85", sheet]);
const made = spawnSync(tool, words);
console.log(made.status === 0 ? `  closed, open, slid to ${value}, set over the remote: ${sheet}` : `  shots in ${out}`);
if (failures.length) console.log(`✗ ${failures.length} failed; the run's log is shots/remote.log`);
process.exit(failures.length ? 1 : 0);
