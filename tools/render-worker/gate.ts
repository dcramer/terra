// The gate between a bench's timed runs (D445): a bench job's script that runs several timed
// sides (`cargo xtask bench` once a side, lakeperf's ab.sh) stops before each at a gate, where
// the daemon lets queued push checks run (a minute or two of CPU) and opens it once none runs,
// so each timed run still has the box alone and a check never waits out an hour of them.
// Benchmark harnesses part trials so (hyperfine's --prepare between runs, JMH's forks).
//
// The gate is a `cargo` first on the timed run's PATH, written per job: `cargo xtask bench …`
// leaves `want` in the gate's folder, wakes the daemon and waits until the daemon removes it;
// every other cargo command passes straight to the real one.
import { chmodSync, mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const quote = (s: string) => `'${s.replace(/'/g, `'\\''`)}'`;

/// The gate's `cargo`, calling `real` (an absolute path) for everything.
export function shim(real: string, dir: string, wake: string): string {
  return [
    "#!/bin/sh",
    "# The render worker's gate between a bench's timed runs (tools/render-worker/gate.ts, D445).",
    `if [ "$1" = xtask ] && [ "$2" = bench ]; then`,
    `  echo $$ > ${quote(join(dir, "want.tmp"))} && mv ${quote(join(dir, "want.tmp"))} ${quote(join(dir, "want"))}`,
    `  date > ${quote(wake)}`,
    `  while [ -e ${quote(join(dir, "want"))} ]; do sleep 1; done`,
    "fi",
    `exec ${quote(real)} "$@"`,
    "",
  ].join("\n");
}

/// Writes the gate in `dir` and gives the environment its timed run takes: the gate's `cargo`
/// first on PATH.
export function writeGate(dir: string, real: string, wake: string, env: Record<string, string>): Record<string, string> {
  mkdirSync(join(dir, "bin"), { recursive: true });
  const cargo = join(dir, "bin", "cargo");
  writeFileSync(cargo, shim(real, dir, wake));
  chmodSync(cargo, 0o755);
  return { ...env, PATH: `${join(dir, "bin")}:${env.PATH ?? ""}`, TERRA_RENDER_GATE: dir };
}

/// The file a bench at its gate leaves.
export const wantFile = (dir: string) => join(dir, "want");
