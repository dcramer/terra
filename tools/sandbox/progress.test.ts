import assert from "node:assert/strict";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { withProgress } from "./progress.ts";

test("a run writing frames says how far it is while it runs, not only when it ends", async () => {
  const dir = mkdtempSync(join(tmpdir(), "progress-"));
  const lines: { line: string; at: number }[] = [];
  const started = Date.now();
  // Ten frames, one each 60 ms, then the run's own output.
  const script = `for i in 0 1 2 3 4 5 6 7 8 9; do : > "${dir}/f-$i.png"; sleep 0.06; done; echo SANDBOX saved`;
  const ran = await withProgress("sh", ["-c", script], {
    dir,
    frames: 10,
    name: "shot",
    every: 4,
    poll_ms: 20,
    say: (line) => lines.push({ line, at: Date.now() - started }),
  });
  assert.equal(ran.status, 0);
  assert.match(ran.stdout, /SANDBOX saved/);
  const told = lines.map((l) => Number(/(\d+) of 10 frames/.exec(l.line)?.[1]));
  assert.ok(told.length >= 3, `a line each 4 frames and the last: ${lines.map((l) => l.line).join("; ")}`);
  assert.equal(told[told.length - 1], 10);
  // The first line came well before the run ended (about 600 ms).
  assert.ok(lines[0].at < 450, `first line at ${lines[0].at} ms`);
});

test("a run that writes a frame and then nothing for a long while still says it is alive", async () => {
  const dir = mkdtempSync(join(tmpdir(), "progress-"));
  const lines: string[] = [];
  // As `--stills` does: one frame at once, then a long silence before the next.
  const script = `: > "${dir}/f-0.png"; sleep 0.6; : > "${dir}/f-1.png"`;
  const ran = await withProgress("sh", ["-c", script], {
    dir,
    frames: 3,
    name: "shot",
    every: 60,
    poll_ms: 20,
    quiet_ms: 150,
    say: (line) => lines.push(line),
  });
  assert.equal(ran.status, 0);
  const quiet = lines.filter((line) => /shot: 1 of 3 frames/.test(line));
  assert.ok(quiet.length >= 2, `a line each 150 ms of silence: ${lines.join("; ")}`);
});
