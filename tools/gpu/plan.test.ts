// The GPU wrapper's readings of the machine and its stall rule, on made-up input whose answers are known by hand.
import assert from "node:assert/strict";
import { test } from "node:test";
import { cpuGrowth, descendants, parseCard, parseCpu, parseLocks, parsePmon, parsePs, Progress } from "./plan.ts";

test("nvidia-smi's card line is read", () => {
  assert.deepEqual(parseCard("10240, 944\n"), { totalMB: 10240, usedMB: 944 });
  assert.equal(parseCard("No devices were found"), null);
});


test("the holders and waiters of a lock are read from /proc/locks", () => {
  const locks = "33: FLOCK  ADVISORY  READ  704977 00:1c:7468718 0 EOF\n" +
    "33: -> FLOCK  ADVISORY  WRITE 766125 00:1c:7468718 0 EOF\n" +
    "34: POSIX  ADVISORY  WRITE 1200 103:02:555 0 EOF\n";
  assert.deepEqual(parseLocks(locks, 7468718), [
    { pid: 704977, write: false, waiting: false },
    { pid: 766125, write: true, waiting: true },
  ]);
});

test("a run's processes are found under it", () => {
  const parents = new Map([[10, 1], [11, 10], [12, 11], [20, 1]]);
  assert.deepEqual([...descendants(10, parents)].sort(), [10, 11, 12]);
});

test("a process's CPU time is read from its stat line, its reaped children's included", () => {
  // pid (name) state ppid pgrp session tty tpgid flags minflt cminflt majflt cmajflt utime stime cutime cstime ...
  const stat = "4242 (my game (x)) R 1 4242 4242 0 -1 4194304 10 0 0 0 300 50 7 3 20 0 1 0 999";
  assert.equal(parseCpu(stat), 360);
});

test("without /proc, each process's parent and CPU time are read from ps, hundredths of a second", () => {
  // macOS ps: minutes past an hour, hours, days; a blank line and a garbled one skipped.
  const text = "    1     0  25:59.39\n  338     1  88:21.15\n  400   338 1:02:03.45\n  401   400 2-00:00:00.01\n\n  junk\n";
  const { parents, cpu } = parsePs(text);
  assert.deepEqual([...parents], [[1, 0], [338, 1], [400, 338], [401, 400]]);
  assert.deepEqual([...cpu], [[1, 155939], [338, 530115], [400, 372345], [401, 17280001]]);
});

test("the processes busy on the card are read from pmon, idle and header lines skipped", () => {
  const text = "# gpu         pid   type     sm    mem\n# Idx           #    C/G      %      %\n" +
    "    0       1376     G      -      -    Hyprland\n    0       5000   C+G     37     12    terra_sandbox\n    0       5001     C      0      0    idle\n";
  assert.deepEqual([...parsePmon(text)], [5000]);
});

test("a job is stalled only when it has neither written, used a core nor the card for its limit", () => {
  const stallMs = 4 * 60000;
  const at = (s: number) => s * 1000;
  // Silent, at half a core: never stalled.
  const busy = new Progress(0, stallMs);
  for (let s = 60; s <= 600; s += 60) busy.read(at(s), new Map([[7, s * 50]]), false);
  assert.equal(busy.stalled(at(600), stallMs), false);
  // Silent, a tick a minute as a waiting process uses: stalled after the limit.
  const idle = new Progress(0, stallMs);
  for (let s = 60; s <= 600; s += 60) idle.read(at(s), new Map([[7, s / 60]]), false);
  assert.equal(idle.stalled(at(600), stallMs), true);
  // Busy on the card alone, its CPU waiting on the GPU: not stalled.
  const gpu = new Progress(0, stallMs);
  for (let s = 60; s <= 600; s += 60) gpu.read(at(s), new Map([[7, 0]]), true);
  assert.equal(gpu.stalled(at(600), stallMs), false);
  // Output alone keeps an idle job alive.
  idle.output(at(590));
  assert.equal(idle.stalled(at(600), stallMs), false);
});

test("a child's work counts once it has ended and its time is in its parent's", () => {
  // The child (8) used 500 ticks and was reaped: the parent's (7) count grows by them.
  assert.equal(cpuGrowth(new Map([[7, 10], [8, 500]]), new Map([[7, 510]])), 500);
  // A new process counts all its ticks; a gone one takes nothing away.
  assert.equal(cpuGrowth(new Map([[7, 10]]), new Map([[7, 10], [9, 40]])), 40);
});
