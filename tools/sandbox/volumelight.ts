#!/usr/bin/env node
// The light of a patch of a still with a light volume on against off, as a ratio (D71), and
// the den's continuous check made from it (tools/continuous/checks.json, D485). One running
// game: each view is held once unshot (a run's first stills are of a world still arriving:
// they read 0.4 and 3.9 of the later ones), then shot with the volume, with the named volume
// moved out of the world (`sandbox.place`, so what it held is lit by the sky's environment
// map, as everything outside a volume is), and with it put back: the same trees, grass and
// clock in all three, which two runs never had. The ratio is the mean of on and back over
// off, and back over on says how steady the run was.
//
//   cargo xtask gpu still -- node tools/sandbox/volumelight.ts [--out target/check/volumelight]
//   node tools/sandbox/volumelight.ts --from <dir>
//   cargo xtask gpu still -- node tools/sandbox/volumelight.ts --volume "Den light" --view x,y,z,x,y,z \
//       [--patch x0,y0,x1,y1] [--out <dir>] [-- <sandbox options>]
//   node tools/sandbox/volumelight.ts --from <dir> --volume "Den light" [--patch x0,y0,x1,y1]
//
// With no --volume it is the check, on seed 3's den at 13:00: a trunk on the land over the
// den, inside the den's volume, must be lit as it is without it, within SAME (the volume once
// held the land's own shade of the sky there and a trunk read 0.8); and the chamber looking
// toward the mouth must stay dark and not blue (the sky must not leak in), and be lit by the
// volume at all (so the switch is known to have switched). --view is an eye and what it looks
// at, metres east, up and south of the start, and --patch fractions of the picture (all of it
// by default); a volume's name is `r find`'s; the world is the check's unless sandbox options
// follow `--`. A line a stage; exits 1 when the check fails. Node 22.18 or later; no
// dependencies.

import { existsSync, rmSync } from "node:fs";
import { join, resolve } from "node:path";
import { parseArgs } from "node:util";
import { call, ROOT, shot, start, step, stop, waitUntil } from "./client.ts";
import { type Picture, readPng } from "./png.ts";

type Box = [number, number, number, number];
type View = { name: string; vantage: number[] };
type Place = [number, number, number];
type Den = { mouth: Place; floor: Place; ahead: Place };

/// The den's check. The world, the volume's name (`den_light`), and the frames a view is
/// held before its still (the exposure eases).
const DEN = ["--experiment", "play", "--fresh", "--save", "shots/volumelight.ron", "--radius", "30000", "--seed", "3", "--time", "13", "--mute"];
const DEN_VOLUME = "Den light";
const HELD_FRAMES = 240;
const PASSES = ["warm", "on", "off", "back"];
/// On the level view: the bark of the trunk 10 m from the mouth, clear of its boughs; and,
/// read and not judged, the grass in the corner (leaves and blades differ more, see the row
/// in scripts.md).
const TRUNK: Box = [0.525, 0.1, 0.6, 0.25];
const GRASS: Box = [0.85, 0.88, 1, 1];
const WHOLE: Box = [0, 0, 1, 1];
/// How far the trunk may stand from itself without the volume, and its still with the volume
/// put back from its first (a run less steady than that has measured nothing). The trunk
/// reads 1.05 on main (two runs, 2026-11-09): what stands in the den's box under trees is a
/// little brighter than outside it, a fault not yet fixed and not this bar's to hide if it
/// grows; with the open air's cells lit as the den's (the fault planted, D632) it reads 0.857.
const SAME = 0.1;
const STEADY = 0.05;
/// The chamber toward the mouth with the volume: its luma and blue share at most these, and
/// without the volume at least LIT times as bright. Read 0.008, 0.19 and 7.8 at b4ea4ba5.
const DARK = 0.02;
const BLUE = 0.25;
const LIT = 2;

const linear = (v: number) => (v <= 0.04045 * 255 ? v / 255 / 12.92 : ((v / 255 + 0.055) / 1.055) ** 2.4);

/// A patch's mean linear luma and the blue's share of its light.
export function patch(p: Picture, [x0, y0, x1, y1]: Box): { luma: number; blue: number } {
  let [r, g, b, n] = [0, 0, 0, 0];
  for (let y = Math.floor(y0 * p.height); y < Math.floor(y1 * p.height); y++)
    for (let x = Math.floor(x0 * p.width); x < Math.floor(x1 * p.width); x++) {
      const i = (y * p.width + x) * 4;
      r += linear(p.rgba[i]);
      g += linear(p.rgba[i + 1]);
      b += linear(p.rgba[i + 2]);
      n++;
    }
  return { luma: n ? (0.2126 * r + 0.7152 * g + 0.0722 * b) / n : 0, blue: r + g + b > 0 ? b / (r + g + b) : 0 };
}

export type Reading = { on: number; off: number; ratio: number; steady: number; blue: number };

/// A patch of a view's three stills: its luma with the volume (the mean of on and back) and
/// without, the one over the other, back over on, and the blue share with the volume.
export function reading(out: string, view: string, box: Box): Reading {
  const [on, off, back] = ["on", "off", "back"].map((word) => patch(readPng(join(out, `${view}-${word}.png`)), box));
  const lit = (on.luma + back.luma) / 2;
  return { on: lit, off: off.luma, ratio: lit / Math.max(off.luma, 1e-6), steady: back.luma / Math.max(on.luma, 1e-6), blue: back.blue };
}

/// The same of a patch the sun dapples: the ratio and the steadiness are the medians of each
/// pixel's own, not the ratio of the patch's means. Boughs sway, so a sunfleck crosses the
/// bark between two stills and moves the mean by its whole light (the trunk's means read
/// 1.065 with back over on 1.037 where its pixels' median read 1.054 and 0.998, 2026-11-09);
/// the flecked pixels are few, and a median leaves them out.
export function shaded(out: string, view: string, [x0, y0, x1, y1]: Box): Reading {
  const [on, off, back] = ["on", "off", "back"].map((word) => readPng(join(out, `${view}-${word}.png`)));
  const luma = (p: Picture, i: number) => 0.2126 * linear(p.rgba[i]) + 0.7152 * linear(p.rgba[i + 1]) + 0.0722 * linear(p.rgba[i + 2]);
  const median = (all: number[]) => all.sort((a, b) => a - b)[all.length >> 1] ?? 0;
  const [ratios, steadies]: number[][] = [[], []];
  for (let y = Math.floor(y0 * on.height); y < Math.floor(y1 * on.height); y++)
    for (let x = Math.floor(x0 * on.width); x < Math.floor(x1 * on.width); x++) {
      const i = (y * on.width + x) * 4;
      const [a, f, c] = [luma(on, i), luma(off, i), luma(back, i)];
      // A black pixel has no ratio.
      if (a < 1e-4 || f < 1e-4) continue;
      ratios.push((a + c) / 2 / f);
      steadies.push(c / a);
    }
  return { ...reading(out, view, [x0, y0, x1, y1]), ratio: median(ratios), steady: median(steadies) };
}

/// The check's verdict from its readings: what fails, in words.
export function judged(trunk: Reading, chamber: Reading): string[] {
  const failed: string[] = [];
  if (Math.abs(trunk.steady - 1) > STEADY) failed.push(`the trunk with the volume put back is ${trunk.steady.toFixed(3)} of its first still (within ${STEADY} of 1): the run was not steady and measured nothing, run it again on a quieter machine`);
  if (Math.abs(trunk.ratio - 1) > SAME) failed.push(`the trunk over the den is ${trunk.ratio.toFixed(3)} of itself without the volume (within ${SAME} of 1)`);
  if (chamber.on > DARK) failed.push(`the chamber's luma is ${chamber.on.toFixed(4)} (at most ${DARK})`);
  if (chamber.blue > BLUE) failed.push(`the chamber's blue share is ${chamber.blue.toFixed(3)} (at most ${BLUE})`);
  if (1 / chamber.ratio < LIT) failed.push(`the chamber without the volume is ${(1 / chamber.ratio).toFixed(2)} times as bright (at least ${LIT}): the volume was not switched off, or lights nothing`);
  return failed;
}

const stills = (out: string, views: string[]) => views.flatMap((view) => PASSES.slice(1).map((word) => join(out, `${view}-${word}.png`)));

/// Shoots each view with the volume, without it and with it back, in one game.
async function shoot(volume: string, sandboxArgs: string[], views: (den?: Den) => View[], out: string) {
  try {
    await start([...sandboxArgs, "--offscreen"], { release: true });
    await waitUntil("the world to be played", async () => (await call("title.state").catch(() => ({ screen: "Playing" }))) as { screen: string }, (now) => now.screen === "Playing", 600);
    const { den } = (await call("den.state").catch(() => ({}))) as { den?: Den };
    const all = views(den);
    const { translation: home } = (await call("sandbox.place", { name: volume })) as { translation: Place };
    for (const word of PASSES) {
      // Far outside any world: the volume then holds nothing, and its light is still worked out.
      if (word === "off") await call("sandbox.place", { name: volume, translation: [1e7, 1e7, 1e7] });
      if (word === "back") await call("sandbox.place", { name: volume, translation: home });
      for (const view of all) {
        await call("sandbox.look", { vantage: view.vantage });
        await step(HELD_FRAMES);
        if (word !== "warm") await shot(join(out, `${view.name}-${word}.png`));
      }
    }
  } catch (error) {
    // The game's own refusal or the client's error says what to do; no stack with it.
    await stop().catch(() => undefined);
    console.log(`volumelight: ✗ the run failed: ${(error as Error).message}`);
    process.exit(1);
  }
  await stop().catch(() => undefined);
}

/// The den's views: level at the mouth from 15 m back, and from the chamber's floor toward it.
function denViews(den?: Den): View[] {
  if (!den) throw new Error("this world has no den (den.state)");
  const { mouth, floor, ahead } = den;
  const toward = (from: Place, to: Place) => {
    const [x, z] = [to[0] - from[0], to[2] - from[2]];
    const l = Math.hypot(x, z) || 1;
    return [x / l, z / l];
  };
  const [ax, az] = toward(mouth, ahead);
  const [mx, mz] = toward(floor, mouth);
  return [
    { name: "level", vantage: [mouth[0] - ax * 15, mouth[1] + 3, mouth[2] - az * 15, ...mouth] },
    { name: "chamber", vantage: [floor[0], floor[1] + 1.6, floor[2], floor[0] + mx * 4, floor[1] + 0.4, floor[2] + mz * 4] },
  ];
}

if (import.meta.main) {
  const { values, positionals } = parseArgs({
    options: { out: { type: "string" }, from: { type: "string" }, volume: { type: "string" }, view: { type: "string" }, patch: { type: "string" } },
    allowPositionals: true,
  });
  const out = resolve(values.from ?? values.out ?? join(ROOT, "target/check/volumelight"));
  const views = values.volume ? ["view"] : ["level", "chamber"];
  if (!values.from) {
    if (values.volume && !values.view) {
      console.log("volumelight: ✗ --volume needs --view x,y,z,x,y,z, an eye and what it looks at from the start (the script's header), or --from a folder already shot");
      process.exit(1);
    }
    for (const still of stills(out, views)) rmSync(still, { force: true });
    if (values.volume) await shoot(values.volume, positionals.length ? positionals : DEN, () => [{ name: "view", vantage: values.view!.split(",").map(Number) }], out);
    else await shoot(DEN_VOLUME, DEN, denViews, out);
  }
  const missing = stills(out, views).find((s) => !existsSync(s));
  if (missing) {
    console.log(`volumelight: ✗ no still ${missing}; the run's log is shots/remote.log`);
    process.exit(1);
  }
  const told = (r: Reading) => `${r.ratio.toFixed(3)} (luma ${r.on.toFixed(4)} over ${r.off.toFixed(4)}, back over on ${r.steady.toFixed(3)}, blue share ${r.blue.toFixed(3)})`;
  if (values.volume) {
    const box = (values.patch ? values.patch.split(",").map(Number) : WHOLE) as Box;
    console.log(`volumelight: "${values.volume}" on against off, patch ${box.join(",")}: ${told(reading(out, "view", box))}; stills in ${out}`);
    process.exit(0);
  }
  const [trunk, grass, chamber] = [shaded(out, "level", TRUNK), shaded(out, "level", GRASS), reading(out, "chamber", WHOLE)];
  const failed = judged(trunk, chamber);
  const fix = failed.length ? `: ${failed.join("; ")}. The den's volume holds the wrong light in those cells: see den_light.rs's \`Whose\` (open air and the rock under it hold the open sky's, D485) and \`thrown_on\`, and do not widen the bars` : "";
  console.log(`volumelight: ${failed.length ? "✗" : "✓"} the den's volume on against off: the trunk over the den ${told(trunk)}, within ${SAME} of 1; the grass by it, read only, ${told(grass)}; the chamber toward the mouth luma ${chamber.on.toFixed(4)} (at most ${DARK}), blue share ${chamber.blue.toFixed(3)} (at most ${BLUE}), ${(1 / chamber.ratio).toFixed(2)} times as bright without it (at least ${LIT})${fix}; stills in ${out}`);
  process.exit(failed.length ? 1 : 0);
}
