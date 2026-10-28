// A program run by a script that films several side by side (tools/controller/yard.ts,
// tools/sandbox/duel.ts), its output written to its log as it comes. Under WSL `cargo xtask gpu`
// sees a Windows program's work only as output (its CPU and GPU use do not show there), so a
// script that held its runs' output until they ended read as stalled while they worked, and the
// longest run was cut off about half the time (handoff yard-stall, 2026-10-28). So:
//   - each run's output goes to its log as it comes: a run stopped part way leaves what it said;
//   - the newest line of any run passes on, on stderr, at most every PASS_ON_MS: progress for
//     `cargo xtask gpu`, and silence when every run is silent;
//   - a run silent for SILENT_MS is stopped, in its own process group so the Windows program's
//     WSL proxy goes too, and the script judges the others.
// Node 22.18 or later; no dependencies.

import { spawn } from "node:child_process";
import { appendFileSync, writeFileSync } from "node:fs";

/// How often a run's newest line is passed on, on stderr.
export const PASS_ON_MS = 30000;
/// A run that says nothing for this long is stopped: under `cargo xtask gpu`'s film limit
/// (10 min), so the film is not stopped first, and over the play world's silent first frame.
export const SILENT_MS = 8 * 60000;
/// When a run's newest line was last passed on, across the runs of one script.
let passedOn = 0;

export type Streamed = {
  /// The script's word for itself, at the start of each line it passes on ("yard", "duel").
  tag: string;
  /// The run's name in those lines.
  name: string;
  command: string;
  words: string[];
  log: string;
  cwd?: string;
  env?: NodeJS.ProcessEnv;
  silentMs?: number;
  passOnMs?: number;
};

/// A run to its end, its output written to `log` as it comes (the command line first); stopped
/// once silent for `silentMs`; tried again while Windows will not replace a program still closing.
/// What it said, with a line saying so when it was stopped.
export async function streamed(run: Streamed): Promise<string> {
  const silentMs = run.silentMs ?? SILENT_MS;
  const passOnMs = run.passOnMs ?? PASS_ON_MS;
  for (let attempt = 0; ; attempt++) {
    writeFileSync(run.log, `${run.command} ${run.words.join(" ")}\n\n`);
    const said = await new Promise<string>((done, fail) => {
      // Its own process group, so stopping it stops the Windows program's WSL proxy too.
      const child = spawn(run.command, run.words, { cwd: run.cwd, env: run.env, detached: true });
      const chunks: Buffer[] = [];
      let heard = Date.now();
      const silent = setInterval(
        () => {
          if (Date.now() - heard < silentMs) return;
          heard = Infinity;
          const note = `\n${run.tag}: ${run.name} said nothing for ${minutes(silentMs)}; stopped\n`;
          (chunks.push(Buffer.from(note)), appendFileSync(run.log, note), process.stderr.write(note.trimStart()));
          try {
            process.kill(-(child.pid ?? 0), "SIGKILL");
          } catch {}
        },
        Math.min(5000, silentMs / 4),
      );
      const take = (chunk: Buffer) => {
        (chunks.push(chunk), appendFileSync(run.log, chunk));
        heard = Date.now();
        if (heard - passedOn < passOnMs) return;
        passedOn = heard;
        const last = chunk.toString("utf8").replace(/\x1b\[[0-9;]*m/g, "").trim().split("\n").pop() ?? "";
        process.stderr.write(`${run.tag}: ${run.name}: ${last.slice(0, 120)}\n`);
      };
      child.stdout.on("data", take);
      child.stderr.on("data", take);
      child.on("error", (error) => (clearInterval(silent), fail(error)));
      child.on("close", () => (clearInterval(silent), done(Buffer.concat(chunks).toString("utf8"))));
    });
    if (attempt >= 30 || !said.includes("Could not copy the program")) return said;
    await new Promise((done) => setTimeout(done, 2000));
  }
}

function minutes(ms: number): string {
  return ms >= 60000 ? `${ms / 60000} min` : `${ms / 1000} s`;
}
