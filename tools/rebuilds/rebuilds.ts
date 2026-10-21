// How long the sandbox takes to rebuild after a file changes (D407): for each probe, one
// file in each part of the code that is or will be a crate of its own, the file is touched
// and the sandbox rebuilt, in dev and in release, and cargo's own timings say which crates
// were compiled again and for how long. A line a probe; the per-crate times go to
// target/rebuilds/last.json, against which the next run is compared.
// Node 22.18 or later; no dependencies. `cargo xtask rebuilds [--dev|--release] [probe…]`.

import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readdirSync, readFileSync, statSync, utimesSync, writeFileSync } from "node:fs";
import { cpus, loadavg } from "node:os";
import { join, resolve } from "node:path";

const ROOT = resolve(import.meta.dirname, "../..");
const OUT = join(ROOT, "target/rebuilds");
const LAST = join(OUT, "last.json");
const LOG = join(OUT, "cargo.log");

/// One file in each part of the code a crate is, or is to be (research/2026-10-18-crate-layout.md).
export const PROBES: Record<string, string> = {
  core: "crates/terra_core/src/lib.rs",
  "build-rules": "crates/terra_build/src/cubes.rs",
  "build-layout": "crates/terra_build_layout/src/layout/runs.rs",
  "build-mesh": "crates/terra_build_mesh/src/sections.rs",
  town: "crates/terra_town/src/town/streets.rs",
  world: "crates/terra_world/src/flora.rs",
  bases: "crates/terra_bases/src/hlod.rs",
  walker: "crates/terra_game/src/play/movement.rs",
  creatures: "crates/terra_game/src/play/creatures.rs",
};

type Unit = { name: string; target: string; mode: string; duration: number };
type Result = { total: number; crates: Record<string, number> };

/// The units cargo's timings report lists, from the report it wrote last.
export function units(html: string): Unit[] {
  const start = html.indexOf("const UNIT_DATA = ");
  if (start < 0) return [];
  const from = html.indexOf("[", start);
  const to = html.indexOf("\n];", from);
  return JSON.parse(html.slice(from, to + 2));
}

/// The workspace's crates compiled, by name, and their seconds.
export function ours(list: Unit[]): Record<string, number> {
  const out: Record<string, number> = {};
  for (const u of list) if (u.name.startsWith("terra_") && !u.target.includes("build-script")) out[u.name] = (out[u.name] ?? 0) + u.duration;
  return out;
}

function latestReport(): string {
  const dir = join(ROOT, "target/cargo-timings");
  const files = readdirSync(dir).filter((f) => f.startsWith("cargo-timing-") && f.endsWith(".html"));
  files.sort((a, b) => statSync(join(dir, b)).mtimeMs - statSync(join(dir, a)).mtimeMs);
  return readFileSync(join(dir, files[0]), "utf8");
}

function build(release: boolean, timings: boolean): Result | null {
  const args = ["build", "-p", "terra_sandbox", ...(release ? ["--release"] : []), ...(timings ? ["--timings"] : [])];
  const begun = performance.now();
  const run = spawnSync("cargo", args, { cwd: ROOT, encoding: "utf8", env: { ...process.env, CARGO_TERM_COLOR: "never" } });
  writeFileSync(LOG, `${run.stdout}${run.stderr}`, { flag: "a" });
  if (run.status !== 0) return null;
  const total = (performance.now() - begun) / 1000;
  return { total, crates: timings ? ours(units(latestReport())) : {} };
}

const load = () => `load ${loadavg()[0].toFixed(0)}/${cpus().length}`;
const secs = (s: number) => `${s.toFixed(0)} s`;

function main(): number {
  const args = process.argv.slice(2);
  const modes = args.includes("--dev") ? [false] : args.includes("--release") ? [true] : [false, true];
  const names = args.filter((a) => !a.startsWith("--"));
  const unknown = names.filter((n) => !PROBES[n]);
  if (unknown.length) {
    console.error(`✗ no probe ${unknown.join(", ")}; the probes: ${Object.keys(PROBES).join(", ")}`);
    return 1;
  }
  const probes = names.length ? names : Object.keys(PROBES);
  mkdirSync(OUT, { recursive: true });
  writeFileSync(LOG, "");
  const last: Record<string, Result> = existsSync(LAST) ? JSON.parse(readFileSync(LAST, "utf8")) : {};
  const now: Record<string, Result> = { ...last };
  for (const release of modes) {
    const mode = release ? "release" : "dev";
    if (!build(release, false)) {
      console.error(`✗ ${mode}: the sandbox does not build; see ${LOG}`);
      return 1;
    }
    for (const probe of probes) {
      const file = join(ROOT, PROBES[probe]);
      const at = new Date();
      utimesSync(file, at, at);
      const result = build(release, true);
      if (!result) {
        console.error(`✗ ${mode} ${probe}: the build failed; see ${LOG}`);
        return 1;
      }
      const key = `${mode} ${probe}`;
      const before = last[key];
      const was = before ? ` (was ${secs(before.total)})` : "";
      const crates = Object.entries(result.crates)
        .sort((a, b) => b[1] - a[1])
        .map(([c, s]) => `${c.replace("terra_", "")} ${secs(s)}`)
        .join(", ");
      console.log(`${key}: ${secs(result.total)}${was}, ${load()}: ${crates}`);
      now[key] = result;
    }
  }
  writeFileSync(LAST, JSON.stringify(now, null, 1));
  console.log(`✓ per-crate seconds in ${LAST}`);
  return 0;
}

if (import.meta.main) process.exit(main());
