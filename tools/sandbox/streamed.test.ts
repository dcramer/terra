import assert from "node:assert/strict";
import { mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { streamed } from "./streamed.ts";

const dir = mkdtempSync(join(tmpdir(), "streamed-"));
/// A node program that says `first`, then nothing for `quiet` ms, then `last`.
const program = (quiet: number) => ["-e", `console.log("first"); setTimeout(() => console.log("last"), ${quiet})`];

test("a run's output is in its log while it runs, and a run that finishes says all of it", async () => {
  const log = join(dir, "talks.log");
  const going = streamed({ tag: "t", name: "talks", command: process.execPath, words: program(1500), log, silentMs: 60000, passOnMs: 0 });
  await new Promise((done) => setTimeout(done, 1000));
  // Before the run ends, its log holds what it said so far (held in memory until the end, it
  // read as stalled to `cargo xtask gpu` under WSL).
  assert.match(readFileSync(log, "utf8"), /^first$/m);
  assert.doesNotMatch(readFileSync(log, "utf8"), /^last$/m);
  const said = await going;
  assert.match(said, /^first\n[^]*^last$/m);
  assert.match(readFileSync(log, "utf8"), /^last$/m);
});

test("a run silent past its limit is stopped, its log saying so after what it said", async () => {
  const log = join(dir, "stalls.log");
  const started = Date.now();
  const said = await streamed({ tag: "t", name: "stalls", command: process.execPath, words: program(60000), log, silentMs: 800, passOnMs: 0 });
  assert.ok(Date.now() - started < 10000, "stopped long before it would have spoken again");
  assert.doesNotMatch(said, /^last$/m);
  const written = readFileSync(log, "utf8");
  assert.match(written, /^first\n[^]*t: stalls said nothing for 0.8 s; stopped/m);
});
