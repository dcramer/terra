#!/usr/bin/env node
// The ground takes a fire's light, and its stones stay orange (a continuous check,
// tools/continuous/checks.json): the yard at night, shot from a fixed camera 2.5 m from where a campfire will stand, before it is
// staged and once it burns. The ground beside the stones, inside the hearth light's reach, must
// brighten by at least GAIN against ground beyond its reach, measured in linear light. Before
// 82f9d55c (D420 revised) the near floor was not the ground's mesh at all (fire round 3,
// 2026-10-19: untextured grey, which ground.wesl's marker did not colour and a 20x hearth light
// left the same to three digits): an updated texture's GPU copy was dropped before the new one
// passed the upload cap, so the ground went undrawn and no point or spot light reached what
// showed in its place. This check holds the light on the ground, whatever drops it next. And
// the ring's firelit stones must read orange on the screen (`STONE_HUE`, `STONE_SATURATION`):
// strong red firelight past the Neutral tonemapper's shoulder took them salmon pink, round after
// round, unseen until a review.
//
//   node tools/sandbox/groundlight.ts [--out target/check/groundlight] [--from <dir>]
//
// Builds the release sandbox, then runs it through the GPU scheduler (`cargo xtask gpu still`).
// --from measures a folder a run wrote instead. A line a stage; exits 1 below the bar.
// Node 22.18 or later; no dependencies.

import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { type Picture, readPng } from "./png.ts";

const ROOT = resolve(import.meta.dirname, "../..");
/// The camera, held each frame (`build.look`, the base's metres), and the campfire it looks at.
const EYE = [-10, 1.7, 12.5];
const AT = [-10, 0.35, 10];
const FIRE = [-10, 0, 10];
/// Shot before the fire is staged, and once it has burned up.
const BEFORE = 150;
const STAGED = 160;
const AFTER = 330;
/// Boxes on the picture, as fractions of its width and height: the ground left and right of
/// the stones, 1 to 2 m from the fire's foot; and the far ground, 20 m and more off, past
/// the light's 14 m reach, which takes out what changes over the whole picture.
const NEAR = [
  [0.25, 0.57, 0.35, 0.72],
  [0.65, 0.57, 0.75, 0.72],
];
const FAR = [[0.05, 0.05, 0.3, 0.09]];
/// The least the near ground may brighten, against the far: 2.69 at main 9d6608c0, and
/// about 1 with the ground undrawn; set between, so the fire's tuning may move.
const GAIN = 1.5;

function linear(v: number): number {
  const c = v / 255;
  return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
}

/// The mean linear luminance of boxes on a picture.
export function light(p: Picture, boxes: number[][]): number {
  let sum = 0;
  let n = 0;
  for (const [x0, y0, x1, y1] of boxes) {
    for (let y = Math.floor(y0 * p.height); y < Math.floor(y1 * p.height); y++) {
      for (let x = Math.floor(x0 * p.width); x < Math.floor(x1 * p.width); x++) {
        const i = (y * p.width + x) * 4;
        sum += 0.2126 * linear(p.rgba[i]) + 0.7152 * linear(p.rgba[i + 1]) + 0.0722 * linear(p.rgba[i + 2]);
        n++;
      }
    }
  }
  return n ? sum / n : 0;
}

/// Boxes on the ring's stones left, right and in front of the logs, clear of the flames and
/// the coals, as `NEAR` is; and the share of their pixels, the brightest by luma, taken as
/// their firelit faces.
const STONES = [
  [0.395, 0.6, 0.435, 0.73],
  [0.565, 0.57, 0.605, 0.72],
  [0.455, 0.725, 0.495, 0.765],
];
const LIT_SHARE = 0.2;
/// The hues a firelit stone may take on the screen, degrees: orange, as the references' stones
/// read by a fire. Strong red firelight past the Neutral tonemapper's shoulder is blended
/// toward white, and the sRGB curve turns that pale red pink (fire rounds 13, 21, 26, 27).
export const STONE_HUE = [20, 40];
/// The least saturation a firelit stone keeps: a pale stone at hue 22 and 0.56 read salmon
/// pink at main 6fab1cba (fire round 27).
export const STONE_SATURATION = 0.6;
/// The mean sRGB colour of the brightest share of the stones' boxes, its HSV hue in degrees
/// and its saturation.
export function stoneHue(p: Picture): { rgb: number[]; hue: number; saturation: number } {
  const px: number[][] = [];
  for (const [x0, y0, x1, y1] of STONES) {
    for (let y = Math.floor(y0 * p.height); y < Math.floor(y1 * p.height); y++) {
      for (let x = Math.floor(x0 * p.width); x < Math.floor(x1 * p.width); x++) {
        const i = (y * p.width + x) * 4;
        px.push([p.rgba[i], p.rgba[i + 1], p.rgba[i + 2]]);
      }
    }
  }
  const luma = (c: number[]) => 0.2126 * linear(c[0]) + 0.7152 * linear(c[1]) + 0.0722 * linear(c[2]);
  px.sort((a, b) => luma(b) - luma(a));
  const lit = px.slice(0, Math.max(1, Math.floor(px.length * LIT_SHARE)));
  const rgb = [0, 1, 2].map((k) => lit.reduce((s, c) => s + c[k], 0) / lit.length);
  const [r, g, b] = rgb;
  const hi = Math.max(r, g, b);
  const span = hi - Math.min(r, g, b);
  let hue = 0;
  if (span > 0) hue = hi === r ? 60 * (((g - b) / span) % 6) : hi === g ? 60 * ((b - r) / span + 2) : 60 * ((r - g) / span + 4);
  if (hue < 0) hue += 360;
  return { rgb, hue, saturation: hi > 0 ? span / hi : 0 };
}
/// How much more the near ground brightened than the far, from before to after; undefined
/// when a box is black, so not lit ground.
export function gain(before: Picture, after: Picture): number | undefined {
  const [nb, na, fb, fa] = [light(before, NEAR), light(after, NEAR), light(before, FAR), light(after, FAR)];
  if (Math.min(nb, na, fb, fa) < 1e-4) return undefined;
  return na / nb / (fa / fb);
}

function run(out: string): boolean {
  const build = spawnSync("cargo", ["build", "-q", "--release", "-p", "terra_sandbox"], { cwd: ROOT, encoding: "utf8" });
  if (build.status !== 0) {
    console.log(`groundlight: ✗ build\n${build.stderr.split("\n").slice(-20).join("\n")}`);
    return false;
  }
  console.log("groundlight: built");
  mkdirSync(out, { recursive: true });
  const calls: object[] = [];
  for (let frame = 60; frame <= AFTER; frame++) calls.push({ frame, method: "build.look", params: { from: EYE, at: AT } });
  calls.push({ frame: STAGED, method: "staged.spawn", params: { name: "campfire", model: "models/props/Campfire.glb", at: FIRE, facing: [FIRE[0], 0, FIRE[2] + 5], base: true, burns: [1.5, 0.7, 0.9] } });
  const file = join(out, "calls.jsonl");
  writeFileSync(file, calls.map((c) => JSON.stringify(c)).join("\n") + "\n");
  const sandbox = [resolve(ROOT, "target/release/terra_sandbox"), "--offscreen", "--clean", "--picture", "1920x1080", "--experiment", "play", "--level", "yard", "--free-build", "--mute",
    "--at", "0,0", "--face", "0,30", "--time", "22", "--date", "09-22", "--calls", file, "--shots", `${join(out, "f")}@${BEFORE},${AFTER}`];
  const done = spawnSync("cargo", ["xtask", "gpu", "still", "--", "timeout", "600", ...sandbox], { cwd: ROOT, encoding: "utf8", env: { ...process.env, BEVY_ASSET_ROOT: ROOT }, maxBuffer: 1 << 28 });
  writeFileSync(join(out, "run.log"), `${done.stdout}${done.stderr}`);
  if (done.status !== 0) {
    console.log(`groundlight: ✗ run exited ${done.status}, see ${join(out, "run.log")}`);
    return false;
  }
  console.log("groundlight: ran");
  return true;
}

if (import.meta.main) {
  const argv = process.argv.slice(2);
  const flag = (name: string) => {
    const i = argv.indexOf(name);
    return i < 0 ? undefined : argv[i + 1];
  };
  const from = flag("--from");
  const out = resolve(from ?? flag("--out") ?? join(ROOT, "target/check/groundlight"));
  if (!from && !run(out)) process.exit(1);
  const still = (frame: number) => join(out, `f-${String(frame).padStart(5, "0")}.png`);
  if (!existsSync(still(BEFORE)) || !existsSync(still(AFTER))) {
    console.log(`groundlight: ✗ no stills in ${out}`);
    process.exit(1);
  }
  const g = gain(readPng(still(BEFORE)), readPng(still(AFTER)));
  const holds = g !== undefined && g >= GAIN;
  const told = g === undefined ? "a box is black, not lit ground" : `${g.toFixed(2)}x against the far ground`;
  console.log(`groundlight: ${holds ? "✓" : "✗"} the ground beside the fire brightens ${told} (at least ${GAIN}), stills in ${out}`);
  const stone = stoneHue(readPng(still(AFTER)));
  const orange = stone.hue >= STONE_HUE[0] && stone.hue <= STONE_HUE[1] && stone.saturation >= STONE_SATURATION;
  const rgb = stone.rgb.map((c) => c.toFixed(0)).join(",");
  console.log(`groundlight: ${orange ? "✓" : "✗"} the firelit stones are orange, hue ${stone.hue.toFixed(0)}° saturation ${stone.saturation.toFixed(2)} (sRGB ${rgb}; hue ${STONE_HUE[0]}-${STONE_HUE[1]}°, saturation at least ${STONE_SATURATION})`);
  process.exit(holds && orange ? 0 : 1);
}
