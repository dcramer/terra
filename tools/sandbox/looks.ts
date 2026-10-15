#!/usr/bin/env node
// The looks check (docs/art-style.md, "Places"; D356): every place's fixed views held to the
// art style's environment rules, each rule a number of its reference's. For each place it shoots
// the views its look is measured on (places.ts, as `vantage.ts` does), measures each (look.ts)
// and prints a table: each number the style holds (style.json), the views' mean, the target
// and its tolerance, whether it holds, and the view furthest out; then what is furthest out.
//
//   node tools/sandbox/looks.ts [--place lush|lake|tundra|town|all] [--time 13] [--from <dir>]
//
// --place picks one place (all of them by default); --time the hour, over the style's own for
// the place; --from measures pictures already shot, `<hour>-<view>.png` in that folder (a
// vantage run's), instead of shooting. Shots go to shots/looks/<place>/; a run shares the GPU,
// waiting only while a benchmark has it (D319). Exits 1 when a number does not hold.
// Node 22.18 or later; no dependencies.

import { spawnSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { ROOT } from "./client.ts";
import { type Bands, type Look, type Reference, bands, look, reference } from "./look.ts";
import { PLACES, place } from "./places.ts";
import { readPng } from "./png.ts";

/// How a number is held: within a tolerance either side of the reference's, at most a
/// tolerance above it, or at least a tolerance below it.
export type Hold = { rule: string; within?: number; under?: number; over?: number };
export type Style = { about: string; rules: Record<string, string>; places: Record<string, { time: string; hold: Record<string, Hold> }> };

export const STYLE_FILE = resolve(import.meta.dirname, "style.json");
export function style(): Style {
  return JSON.parse(readFileSync(STYLE_FILE, "utf8")) as Style;
}

/// The numbers a picture is judged on, by name: its look's (`mean`, `hardEdges`…) and its
/// bands' (`far.mean`, `near.contrast`, `farUnderSky`…).
export function numbers(l: Look, b?: Bands): Record<string, number> {
  const out: Record<string, number> = {};
  for (const [k, v] of Object.entries(l)) if (typeof v === "number") out[k] = v;
  if (b) {
    for (const k of ["sky", "far", "middle", "near"] as const) for (const [n, v] of Object.entries(b[k])) out[`${k}.${n}`] = v;
    out.farUnderSky = b.farUnderSky;
    out.farBluerThanMiddle = b.farBluerThanMiddle;
  }
  return out;
}

/// What each number is called in the table, and whether it is a share (printed as a percent).
export const MEASURES: Record<string, { called: string; share?: boolean }> = {
  mean: { called: "brightness" },
  p5: { called: "5th percentile" },
  p50: { called: "median" },
  p95: { called: "95th percentile" },
  dark: { called: "near black", share: true },
  bright: { called: "near white", share: true },
  redOverGreen: { called: "red/green" },
  blueOverGreen: { called: "blue/green" },
  contrast: { called: "local contrast" },
  litLitter: { called: "lit orange below", share: true },
  shadeRedOverGreen: { called: "shade red/green" },
  shadeBlueOverGreen: { called: "shade blue/green" },
  middleBlueOverGreen: { called: "middle blue/green" },
  limeTop: { called: "lime above", share: true },
  trunkMiddle: { called: "trunks across", share: true },
  tint: { called: "debug tint", share: true },
  lilac: { called: "lilac", share: true },
  hardEdges: { called: "hard shade edges", share: true },
  facets: { called: "facet edges (widths)" },
  flat: { called: "flat colour", share: true },
  farUnderSky: { called: "far under sky" },
  farBluerThanMiddle: { called: "far bluer than middle" },
  ...Object.fromEntries(
    (["sky", "far", "middle", "near"] as const).flatMap((k) => [
      [`${k}.share`, { called: `${k} share`, share: true }],
      [`${k}.mean`, { called: `${k} brightness` }],
      [`${k}.blueOverGreen`, { called: `${k} blue/green` }],
      [`${k}.saturation`, { called: `${k} saturation` }],
      [`${k}.contrast`, { called: `${k} contrast` }],
    ]),
  ),
};

/// One number judged: the views' mean against the reference's, the bounds it must keep,
/// whether it does, how far out it is in tolerances (0 inside), and the view furthest the wrong
/// way with its own value.
export type Judged = {
  rule: string;
  measure: string;
  ours: number;
  target: number;
  low: number;
  high: number;
  holds: boolean;
  out: number;
  worst: { view: string; value: number };
};

/// Every number a place's style holds, judged on its views' numbers against the reference's.
export function judge(hold: Record<string, Hold>, target: Record<string, number>, views: { view: string; numbers: Record<string, number> }[]): Judged[] {
  return Object.entries(hold).map(([measure, h]) => {
    const at = target[measure];
    if (at === undefined) throw new Error(`the reference has no ${measure}`);
    const values = views.map((v) => ({ view: v.view, value: v.numbers[measure] }));
    const ours = values.reduce((s, v) => s + v.value, 0) / values.length;
    const d = h.within ?? h.under ?? h.over ?? 0;
    const low = h.under !== undefined ? -Infinity : at - d;
    const high = h.over !== undefined ? Infinity : at + d;
    const holds = ours >= low && ours <= high;
    const out = holds ? 0 : (ours > high ? ours - high : low - ours) / Math.max(d, 1e-9);
    // The view furthest the wrong way: the highest under a ceiling, the lowest over a floor,
    // the furthest from the target held within.
    const far = (v: { value: number }) => (h.under !== undefined ? v.value : h.over !== undefined ? -v.value : Math.abs(v.value - at));
    const worst = values.reduce((a, b) => (far(b) > far(a) ? b : a));
    return { rule: h.rule, measure, ours, target: at, low, high, holds, out, worst };
  });
}

/// A number as the table prints it: a share as a percent, anything else to three places.
export function shown(measure: string, v: number): string {
  return MEASURES[measure]?.share ? `${(100 * v).toFixed(1)}%` : v.toFixed(3);
}

/// The target as the table prints it: the reference's number and its bounds.
function bounds(j: Judged): string {
  if (j.low === -Infinity) return `<= ${shown(j.measure, j.high)} (ref ${shown(j.measure, j.target)})`;
  if (j.high === Infinity) return `>= ${shown(j.measure, j.low)} (ref ${shown(j.measure, j.target)})`;
  return `${shown(j.measure, j.target)} +- ${shown(j.measure, j.high - j.target).replace(/^-/, "")}`;
}

/// A place's table: a line a number, grouped by rule, then how many hold and the furthest out.
export function table(name: string, about: string, ref: Reference, views: string[], hour: string, judged: Judged[], rules: Record<string, string>): string {
  const rows = judged.map((j) => [
    j.holds ? "ok" : "FAIL",
    rules[j.rule] ?? j.rule,
    MEASURES[j.measure]?.called ?? j.measure,
    shown(j.measure, j.ours),
    bounds(j),
    j.holds ? "" : `${j.out.toFixed(1)}x out; ${j.worst.view} ${shown(j.measure, j.worst.value)}`,
  ]);
  const head = ["", "rule", "number", "ours", "target", "worst view"];
  const widths = head.map((h, k) => Math.max(h.length, ...rows.map((r) => r[k].length)));
  const pad = (r: string[]) => `  ${r.map((c, k) => c.padEnd(widths[k])).join("  ")}`.trimEnd();
  const failed = judged.filter((j) => !j.holds).sort((a, b) => b.out - a.out);
  return [
    `looks: ${name}: ${about}`,
    `looks: ${name}: against ${ref.name} (${ref.frames} frames); views ${views.join(", ")} at ${hour}:00`,
    pad(head),
    ...rows.map(pad),
    `looks: ${name}: ${judged.length - failed.length} of ${judged.length} hold${failed.length ? `; furthest out: ${failed.slice(0, 4).map((j) => `${MEASURES[j.measure]?.called ?? j.measure} ${j.out.toFixed(1)}x (${j.worst.view})`).join(", ")}` : ""}`,
  ].join("\n");
}

if (import.meta.main) {
  const arg = (name: string) => {
    const at = process.argv.indexOf(name);
    return at > 0 ? process.argv[at + 1] : undefined;
  };
  const kept = style();
  const asked = arg("--place") ?? "all";
  const names = asked === "all" ? Object.keys(kept.places) : [asked];
  const from = arg("--from");
  for (const n of names) {
    place(n);
    if (!kept.places[n]) {
      console.error(`looks: style.json holds nothing for ${n}: ${Object.keys(kept.places).join(", ")}`);
      process.exit(2);
    }
  }
  if (from && names.length !== 1) {
    console.error("looks: --from measures one place's pictures; name it with --place");
    process.exit(2);
  }
  if (!from) {
    const build = spawnSync("cargo", ["build", "-q", "--release", "-p", "terra_sandbox"], { cwd: ROOT, stdio: "inherit" });
    if (build.status !== 0) process.exit(1);
  }
  const { shootViews } = await import("./vantage.ts");
  let failed = 0;
  const all: { place: string; j: Judged }[] = [];
  for (const n of names) {
    const where = PLACES[n];
    const hour = arg("--time") ?? kept.places[n].time;
    const ref = reference(where.look);
    let files: Map<string, string>;
    if (from) {
      files = new Map(where.measured.map((v) => [v, resolve(from, `${hour}-${v}.png`)]));
      const missing = [...files.values()].filter((f) => !existsSync(f));
      if (missing.length) {
        console.error(`looks: missing ${missing.join(", ")}`);
        process.exit(2);
      }
    } else {
      const started = Date.now();
      try {
        files = new Map(shootViews(where, hour, resolve(ROOT, "shots/looks", n)).files);
      } catch (e) {
        console.error(`looks: ${n}: ${(e as Error).message}`);
        process.exit(1);
      }
      console.log(`looks: ${n}: shot in ${((Date.now() - started) / 1000).toFixed(0)} s`);
    }
    const views = where.measured.map((view) => {
      const p = readPng(files.get(view)!);
      return { view, numbers: numbers(look(p), ref.bands ? bands(p) : undefined) };
    });
    const judged = judge(kept.places[n].hold, numbers(ref.look, ref.bands), views);
    console.log(table(n, where.about, ref, where.measured, hour, judged, kept.rules));
    console.log("");
    failed += judged.filter((j) => !j.holds).length;
    all.push(...judged.map((j) => ({ place: n, j })));
  }
  if (names.length > 1) {
    const worst = all.filter((a) => !a.j.holds).sort((a, b) => b.j.out - a.j.out).slice(0, 8);
    console.log(`looks: ${all.length - failed} of ${all.length} hold across ${names.length} places${worst.length ? `; furthest out: ${worst.map((a) => `${a.place} ${MEASURES[a.j.measure]?.called ?? a.j.measure} ${a.j.out.toFixed(1)}x (${a.j.worst.view})`).join(", ")}` : ""}`);
  }
  process.exit(failed ? 1 : 0);
}
