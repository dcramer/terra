#!/usr/bin/env node
// Every experiment runs on its own (a continuous check, tools/continuous/checks.json; D623):
// each the sandbox lists, but the icon baker (it writes assets/icons), is started alone,
// offscreen, and must enter its world and draw frame 20 with nothing panicking. A world
// system that reads a world resource and is not gated on `instance::in_world` panics while
// the land is generated; one experiment that reads what only another brings (lush, once,
// the day's clock it took from play) panics on entering. Neither shows in a layer's test,
// whose app has no renderer, so this runs the real one.
//
//   node tools/sandbox/alone.ts [--out target/check/alone] [experiment…]
//
// Builds the release sandbox, then runs each through the GPU scheduler (`cargo xtask gpu
// still`). A line an experiment; exits 1 when one fails, naming its log and its panic.
// Node 22.18 or later; no dependencies.

import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";

const ROOT = resolve(import.meta.dirname, "../..");
const SANDBOX = resolve(ROOT, "target/release/terra_sandbox");
/// What is not run: the icon baker writes the icons into the repository.
const NOT = ["icons"];
const FRAME = 20;

function args(): { out: string; only: string[] } {
  const argv = process.argv.slice(2);
  const at = argv.indexOf("--out");
  const out = at >= 0 ? resolve(argv[at + 1]) : resolve(ROOT, "target/check/alone");
  const only = argv.filter((_, k) => at < 0 || (k !== at && k !== at + 1));
  return { out, only };
}

/// The experiments the sandbox lists, by name.
function listed(): string[] {
  const list = spawnSync(SANDBOX, ["--list"], { cwd: ROOT, encoding: "utf8" });
  return list.stdout
    .split("\n")
    .map((line) => line.split(/\s+/)[0])
    .filter((name) => name && !NOT.includes(name));
}

/// The first panic in a run's log, its message line included.
function panic(log: string): string | undefined {
  const lines = log.replace(/\x1b\[[0-9;]*m/g, "").split("\n");
  const at = lines.findIndex((line) => line.includes("panicked"));
  return at < 0 ? undefined : lines.slice(at, at + 2).join(" ").slice(0, 400);
}

function main(): number {
  const { out, only } = args();
  const build = spawnSync("cargo", ["build", "-q", "--release", "-p", "terra_sandbox"], { cwd: ROOT, encoding: "utf8" });
  if (build.status !== 0) {
    console.log(`alone: ✗ build\n${build.stderr.split("\n").slice(-20).join("\n")}`);
    return 1;
  }
  console.log("alone: built");
  mkdirSync(out, { recursive: true });
  const names = only.length > 0 ? only : listed();
  let failed = 0;
  for (const name of names) {
    const shot = join(out, name);
    const sandbox = [SANDBOX, "--offscreen", "--mute", "--experiment", name, "--shots", `${shot}@${FRAME}`];
    const done = spawnSync("cargo", ["xtask", "gpu", "still", "--", "timeout", "600", ...sandbox], { cwd: ROOT, encoding: "utf8", env: { ...process.env, BEVY_ASSET_ROOT: ROOT }, maxBuffer: 1 << 28 });
    const log = `${done.stdout}${done.stderr}`;
    const logFile = join(out, `${name}.log`);
    writeFileSync(logFile, log);
    const panicked = panic(log);
    const drawn = existsSync(`${shot}-${String(FRAME).padStart(5, "0")}.png`);
    if (done.status === 0 && !panicked && drawn) {
      console.log(`alone: ✓ ${name}`);
      continue;
    }
    failed += 1;
    const why = panicked ?? (drawn ? `exit ${done.status}` : `exit ${done.status}, frame ${FRAME} not drawn`);
    console.log(`alone: ✗ ${name}: ${why}\n  log ${logFile}; a panic's summary.txt is named on its last line (D399). A world system missing a world resource while the land is made wants .run_if(terra_world::instance::in_world) on its plugin's set (D623); an experiment reading what another brings brings it (experiments::ON_THE_WORLD).`);
  }
  console.log(failed === 0 ? `alone: ✓ ${names.length} experiments each ran alone` : `alone: ✗ ${failed} of ${names.length} experiments failed alone`);
  return failed === 0 ? 0 : 1;
}

process.exit(main());
