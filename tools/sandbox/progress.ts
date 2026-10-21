// A long run that writes frames says how far it is as it goes (D388), so a scheduler that
// stops a silent run (`cargo xtask gpu film`'s stall, D319) sees that a long shot is alive.
// trailer.ts's shots took 23 minutes in silence and were stopped by the film class's 10.

import { spawn } from "node:child_process";
import { existsSync, readdirSync } from "node:fs";

export type Ran = { status: number | null; stdout: string; stderr: string };

export type Progress = {
  /// Where the run writes its frames, one `.png` each.
  dir: string;
  /// How many frames it will write.
  frames: number;
  /// What each line names.
  name: string;
  /// A line each this many frames written.
  every?: number;
  /// How often the directory is looked at, in ms.
  poll_ms?: number;
  /// Before its first frame, a line each this many ms.
  loading_ms?: number;
  cwd?: string;
  env?: NodeJS.ProcessEnv;
  say?: (line: string) => void;
};

/// Runs `command` as spawnSync would (its whole output kept), saying `<name>: N of M frames`
/// each `every` frames written into `dir`, and `<name>: loading` while none are.
export async function withProgress(command: string, options: string[], p: Progress): Promise<Ran> {
  const every = p.every ?? 60;
  const say = p.say ?? ((line: string) => console.log(line));
  const child = spawn(command, options, { cwd: p.cwd, env: p.env });
  let stdout = "";
  let stderr = "";
  child.stdout.setEncoding("utf8").on("data", (part: string) => (stdout += part));
  child.stderr.setEncoding("utf8").on("data", (part: string) => (stderr += part));
  const started = Date.now();
  let told = 0;
  let loading = 0;
  const look = () => {
    const done = existsSync(p.dir) ? readdirSync(p.dir).filter((file) => file.endsWith(".png")).length : 0;
    const seconds = ((Date.now() - started) / 1000).toFixed(0);
    if (done >= told + every || (done >= p.frames && done > told)) {
      told = done;
      say(`trailer: ${p.name}: ${done} of ${p.frames} frames, ${seconds} s`);
    } else if (done === 0 && Date.now() - started >= (loading + 1) * (p.loading_ms ?? 60_000)) {
      loading++;
      say(`trailer: ${p.name}: loading, ${seconds} s`);
    }
  };
  const tick = setInterval(look, p.poll_ms ?? 5000);
  const status = await new Promise<number | null>((end) => child.on("close", end));
  clearInterval(tick);
  look();
  return { status, stdout, stderr };
}
