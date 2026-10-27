// node --test tools/sandbox/load.test.ts
import { strict as assert } from "node:assert";
import { test } from "node:test";
import { busyDuring, middle, parseProcStat, parseTypeperf, quiet } from "./load.ts";

test("reads Windows' processor time from typeperf", () => {
  const text = '"(PDH-CSV 4.0)","\\\\BOX\\Processor(_Total)\\% Processor Time"\n"10/16/2026 11:05:03.021","93.75"\nExiting, please wait...\n';
  assert.deepEqual(parseTypeperf(text), [93.75]);
  assert.deepEqual(parseTypeperf("The command failed"), []);
  // With the sandbox's process beside the total.
  assert.deepEqual(parseTypeperf('"(PDH-CSV 4.0)","total","sandbox"\n"10/16/2026","40.0","320.0"\n'), [40, 320]);
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
  // Idle processors for a second, but more threads queued than processors.
  assert.ok(!quiet({ cpu: 2, gpu: 0, load: 125 }));
  assert.ok(quiet({ cpu: 2, gpu: 0, load: 10 }));
  // Unknown load is not held against a run.
  assert.ok(quiet(undefined));
  assert.ok(busyDuring({ mean: 40, most: 60 }));
  assert.ok(!busyDuring({ mean: 20, most: 60 }));
  assert.ok(busyDuring({ mean: 20, most: 80 }));
});

test("the middle of runs and their spread", () => {
  const { median, low, high, spreadPct } = middle([5.5, 5, 6]);
  assert.deepEqual([median, low, high], [5.5, 5, 6]);
  assert.ok(Math.abs(spreadPct - 100 / 5.5) < 1e-9);
});
