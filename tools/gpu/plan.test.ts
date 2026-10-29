// The GPU wrapper's readings of the machine and its stall rule, on made-up input whose answers are known by hand.
import assert from "node:assert/strict";
import { test } from "node:test";
import { advise, byGroup, cpuGrowth, descendants, type Holding, outside, parseCard, parseCpu, parseFdinfo, parseLocks, parsePmon, parsePs, parseStat, Progress } from "./plan.ts";

test("an open file of the lock holds it, waits for it, or neither, from its fdinfo", () => {
  const held = "pos:\t0\nflags:\t0100000\nmnt_id:\t64\nino:\t28142848\nlock:\t1: FLOCK  ADVISORY  READ 2036242 00:1c:28142848 0 EOF\n";
  assert.equal(parseFdinfo(held, 28142848), "holds");
  assert.equal(parseFdinfo(held, 4), null);
  assert.equal(parseFdinfo("ino:\t28142848\nlock:\t1: -> FLOCK  ADVISORY  WRITE 9 00:1c:28142848 0 EOF\n", 28142848), "waits");
  assert.equal(parseFdinfo("pos:\t0\nino:\t28142848\n", 28142848), "open");
  assert.equal(parseFdinfo("ino:\t281428480\n", 28142848), null);
});

test("a process's group and start are read from its stat line", () => {
  const stat = "3345881 (sh) S 1 3345881 3345881 0 -1 4194560 1 2 0 0 3 4 0 0 39 19 1 0 98765432 1000 200 18446744073709551615";
  assert.deepEqual(parseStat(stat), { pgrp: 3345881, start: 98765432 });
});

test("a holder outside every live run is named with whose it was and what to do", () => {
  const loop: Holding = { pid: 3345881, pgrp: 3345881, cmd: "sh -c while :; do cargo clippy; done", cwd: "/w/slots/4", ageMs: 166 * 60000, gpuJob: 500, renderJob: "20261029-083350-269b-patch" };
  const mine: Holding = { ...loop, pid: 601, pgrp: 601, gpuJob: 600 };
  const flock: Holding = { ...loop, pid: 700, gpuJob: 600 };
  const runs = [{ pid: 600, child: 700 }];
  const live = (pid: number) => pid === 600;
  assert.deepEqual(outside([loop, mine, flock], runs, live).map((h) => h.pid), [3345881], "a live run's command and what it started are its own");
  // A run on file whose wrapper died no longer owns what it started.
  assert.deepEqual(outside([mine], runs, () => false).map((h) => h.pid), [601]);
  const clippy: Holding = { ...loop, pid: 3399999, cmd: "cargo clippy", ageMs: 60000 };
  assert.deepEqual(byGroup([clippy, loop, mine]).map((h) => [h.pid, h.others]), [[3345881, 1], [601, 0]], "a group is named by its oldest");
  const said = advise(loop, "gpu.lock");
  assert.match(said, /^gpu.lock held outside any run for 2h46m by pid 3345881 `sh -c while/);
  assert.match(said, /in \/w\/slots\/4, started by the `cargo xtask gpu` run of wrapper pid 500, which has ended, in render job 20261029-083350-269b-patch:/);
  assert.match(said, /stop it: kill -- -3345881/);
  assert.match(advise({ ...loop, pgrp: 1, gpuJob: undefined, renderJob: undefined }, "gpu.lock"), /started outside `cargo xtask gpu`.*stop it: kill 3345881; if it is a run someone started by hand, wait for it\.$/);
});

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
