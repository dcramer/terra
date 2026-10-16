// node --test tools/sandbox/load.test.ts
import { strict as assert } from "node:assert";
import { test } from "node:test";
import { busyDuring, middle, parseProcStat, parseTypeperf, quiet } from "./load.ts";

test("reads Windows' processor time from typeperf", () => {
  const text = '"(PDH-CSV 4.0)","\\\\BOX\\Processor(_Total)\\% Processor Time"\n"10/16/2026 11:05:03.021","93.75"\nExiting, please wait...\n';
  assert.equal(parseTypeperf(text), 93.75);
  assert.equal(parseTypeperf("The command failed"), undefined);
});

test("reads Linux's jiffies, idle and waiting on the disk not busy", () => {
  // user nice system idle iowait irq softirq steal
  assert.deepEqual(parseProcStat("cpu  60 0 40 800 100 0 0 0 0 0\ncpu0 1 2 3 4\n"), [100, 1000]);
  assert.equal(parseProcStat("nothing"), undefined);
});

test("quiet is the processors and the GPU both; busy during a run is the processors' mean", () => {
  assert.ok(quiet({ cpu: 10, gpu: 2 }));
  assert.ok(quiet({ cpu: 10 }));
  assert.ok(!quiet({ cpu: 60, gpu: 2 }));
  assert.ok(!quiet({ cpu: 10, gpu: 40 }));
  // Unknown load is not held against a run.
  assert.ok(quiet(undefined));
  assert.ok(busyDuring({ mean: 70, most: 90 }));
  assert.ok(!busyDuring({ mean: 30, most: 90 }));
  assert.ok(busyDuring({ mean: 30, most: 99 }));
});

test("the middle of runs and their spread", () => {
  const { median, low, high, spreadPct } = middle([5.5, 5, 6]);
  assert.deepEqual([median, low, high], [5.5, 5, 6]);
  assert.ok(Math.abs(spreadPct - 100 / 5.5) < 1e-9);
});
