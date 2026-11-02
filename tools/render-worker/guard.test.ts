// The disk guard (guard.ts, daemon.ts `Keeper`, box.ts `waitFor`): what is pruned and in which
// order, a full disk (a real ENOSPC, from /dev/full) caught rather than crashing the daemon, and
// a wait that gives up on a box that cannot end its job. No build, no GPU; under a second.

import assert from "node:assert/strict";
import { existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, statSync, symlinkSync, utimesSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, test } from "node:test";
import { box, jobFile, readJob, slotDir, slotHome, writeJob } from "./box.ts";
import { Keeper, writeStatus } from "./daemon.ts";
import { DISK, diskSettings, GB, health, level, lowMark, type Prune, prune, pruneOrder, type SlotUse } from "./guard.ts";
import type { Job } from "./jobs.ts";

// The scratch boxes sit on /tmp, a RAM disk of a few GB here: marks that never call it full.
process.env.TERRA_RENDER_DISK_LOW_GB = "2";
process.env.TERRA_RENDER_DISK_FULL_GB = "1";
const tmp = mkdtempSync(join(tmpdir(), "render-guard-"));
after(() => rmSync(tmp, { recursive: true, force: true }));
const now = new Date("2026-11-02T04:00:00Z");
const daysAgo = (d: number) => new Date(now.getTime() - d * 86400_000).toISOString();
const job = (over: Partial<Job> = {}): Job => ({ id: "20261101-120000-aaaa", kind: "run", rev: "0".repeat(40), command: ["true"], collect: [], cook: false, minutes: 1, by: "t", queued: daysAgo(5), state: "done", ...over });

test("settings: the marks' defaults, and a full mark under the low one", () => {
  assert.deepEqual(diskSettings({}), DISK);
  assert.ok("error" in diskSettings({ TERRA_RENDER_DISK_FULL_GB: "200" }), "a full mark over the low mark is refused");
  assert.ok("error" in diskSettings({ TERRA_RENDER_MEDIA_DAYS: "0" }));
  // 15% of 1.8 TB is over 150 GB, so it is the low mark there; on a 500 GB disk 150 GB is.
  assert.equal(Math.round(lowMark(DISK, 1800 * GB) / GB), 270);
  assert.equal(lowMark(DISK, 500 * GB), 150 * GB);
  assert.equal(level(DISK, { free: 360 * GB, total: 1800 * GB }), "ok");
  assert.equal(level(DISK, { free: 200 * GB, total: 1800 * GB }), "low");
  assert.equal(level(DISK, { free: 10 * GB, total: 1800 * GB }), "full");
});

test("prune order: idle slots' debug incremental, then old jobs' files, then release incremental; busy slots and live jobs never", () => {
  const slot = (n: number, busy: boolean, review = true): SlotUse => ({ slot: n, busy, target: `/s/${n}/target`, profiles: ["debug", "release"], ...(review ? { review: `/h/${n}/r` } : {}) });
  const jobs = [
    { ...job({ id: "20261025-000000-0001", ended: daysAgo(8) }), out: "/j/1/out" },
    { ...job({ id: "20261101-000000-0002", ended: daysAgo(1) }), out: "/j/2/out" },
    { ...job({ id: "20261020-000000-0003", state: "queued", ended: undefined, queued: daysAgo(12) }), out: "/j/3/out" },
    { ...job({ id: "20261020-000000-0004", state: "running", ended: undefined, queued: daysAgo(12) }), out: "/j/4/out" },
  ];
  const order = pruneOrder([slot(2, false), slot(1, true), slot(3, false, false)], jobs, now, 3);
  assert.deepEqual(
    order.map((p) => `${p.stage} ${p.kind} ${p.path}`),
    ["1 dir /s/2/target/debug/incremental", "1 dir /s/3/target/debug/incremental", "2 media /j/1/out", "2 review /h/2/r", "3 dir /s/2/target/release/incremental", "3 dir /s/3/target/release/incremental"],
  );
  assert.equal(order.find((p) => p.kind === "review")?.before, now.getTime() - 3 * 86400_000);
});

test("prune stops once free space reaches its mark, and says what each stage freed", async () => {
  const items: Prune[] = [
    { stage: 1, kind: "dir", path: "a", what: "slot 1's debug/incremental" },
    { stage: 1, kind: "dir", path: "b", what: "slot 2's debug/incremental" },
    { stage: 2, kind: "media", path: "c", what: "job x's files" },
  ];
  let free = 100;
  const gone: string[] = [];
  const lines: string[] = [];
  const freed = await prune(items, 150 * GB, () => free * GB, async (p) => (gone.push(p.path), (free += 40)), (l) => lines.push(l));
  assert.deepEqual(gone, ["a", "b"], "the media stay: two incremental folders were enough");
  assert.equal(freed, 80 * GB);
  assert.deepEqual(lines, ["disk: pruned the debug incremental of idle slots (slot 1's debug/incremental, slot 2's debug/incremental): freed 80 GB"]);
});

/// A box folder for a test, and a disk of 1000 GB for it whose free space is `start` less what
/// the box's files have grown by since, a byte of them standing for 20 MB, so pruning a few kB
/// shows as tens of GB freed.
function scratchBox(name: string) {
  const b = box(join(tmp, name));
  mkdirSync(b.jobs, { recursive: true });
  mkdirSync(b.repo, { recursive: true });
  const size = (p: string): number => {
    const st = statSync(p, { throwIfNoEntry: false });
    if (!st) return 0;
    return st.isDirectory() ? readdirSync(p).reduce((n, f) => n + size(join(p, f)), 0) : st.size;
  };
  const measure = (start: number) => {
    const at = size(b.dir);
    return () => ({ free: start - (size(b.dir) - at) * 0.02 * GB, total: 1000 * GB });
  };
  return { b, measure };
}

test("the keeper prunes a real box in order: an idle slot's debug incremental first, a busy slot's never", async () => {
  const { b, measure } = scratchBox("prune");
  const put = (p: string, bytes: number) => (mkdirSync(join(p, ".."), { recursive: true }), writeFileSync(p, Buffer.alloc(bytes)));
  for (const n of [1, 2]) {
    put(join(slotDir(b, n), "target/debug/incremental/s/a.bin"), 3000);
    put(join(slotDir(b, n), "target/release/incremental/s/a.bin"), 3000);
  }
  put(join(b.jobs, "20261025-000000-0001/out/film.mp4"), 1000);
  put(join(b.jobs, "20261025-000000-0001/out/job.log"), 10);
  writeJob(b, job({ id: "20261025-000000-0001", ended: daysAgo(8) }));
  const old = join(slotHome(b, 2), ".cache/terra-review/old.png");
  put(old, 500);
  utimesSync(old, new Date(daysAgo(9)), new Date(daysAgo(9)));
  // 140 GB free, under the low mark of 150 GB: slot 2's debug incremental (3 kB, "60 GB")
  // takes it past the 180 GB pruning stops at.
  const lines: string[] = [];
  const k = new Keeper(b, DISK, (l) => lines.push(l), measure(140 * GB));
  const running = new Map([["20261102-000000-0009", { job: job({ id: "20261102-000000-0009", state: "running", slot: 1 }), cancelled: false }]]);
  assert.equal(await k.pass(30, running, 2, now), "", "low, not full: jobs still start");
  assert.ok(!existsSync(join(slotDir(b, 2), "target/debug/incremental")), "the idle slot's debug incremental went");
  assert.ok(existsSync(join(slotDir(b, 1), "target/debug/incremental/s/a.bin")), "the busy slot is untouched");
  assert.ok(existsSync(join(slotDir(b, 2), "target/release/incremental/s/a.bin")), "release incremental stays: the first stage was enough");
  assert.ok(existsSync(join(b.jobs, "20261025-000000-0001/out/film.mp4")), "and so do the job's files");
  assert.match(lines.join("\n"), /pruning to 180 GB[\s\S]*debug incremental of idle slots \(slot 2's debug\/incremental\)/);

  // Lower still: the old job's files go but its log, then the old HOME file, then release incremental.
  const k2 = new Keeper(b, DISK, (l) => lines.push(l), measure(50 * GB));
  assert.equal(await k2.pass(30, running, 2, now), "");
  assert.deepEqual(readdirSync(join(b.jobs, "20261025-000000-0001/out")), ["job.log"]);
  assert.match(readFileSync(join(b.jobs, "20261025-000000-0001/out/job.log"), "utf8"), /collected files were deleted .* to free the box's disk/);
  assert.ok(!existsSync(old), "the HOME review cache's old file went");
  assert.ok(!existsSync(join(slotDir(b, 2), "target/release/incremental")));
  assert.ok(existsSync(join(slotDir(b, 1), "target/release/incremental/s/a.bin")), "the busy slot is untouched still");
});

test("ENOSPC: the keeper says the disk is full and keeps the record it could not write, and takes jobs again once it can", async () => {
  const { b, measure } = scratchBox("full");
  // A job a dead daemon left running, whose record cannot be written: /dev/full fails every write with ENOSPC.
  const stale = job({ id: "20261102-000000-00aa", state: "running", queued: daysAgo(0) });
  writeJob(b, stale);
  symlinkSync("/dev/full", `${jobFile(b, stale.id)}.tmp`);
  const lines: string[] = [];
  const k = new Keeper(b, DISK, (l) => lines.push(l), measure(500 * GB));
  const full = await k.pass(7, new Map(), 2, now);
  assert.match(full, /a write failed: ENOSPC/, "the pass returns why, it does not throw");
  assert.ok(k.disk?.full, "daemon.json says so");
  assert.equal(k.unsaved.get(stale.id)?.state, "failed", "the record is kept to write again");
  assert.equal(readJob(b, stale.id)?.state, "running");
  assert.match(lines.join("\n"), /✗ disk full .*no job starts until space is freed/);
  // Space again: the record is written, and the keeper takes jobs.
  rmSync(`${jobFile(b, stale.id)}.tmp`);
  assert.equal(await k.pass(7, new Map(), 2, now), "");
  assert.equal(readJob(b, stale.id)?.state, "failed");
  assert.equal(k.unsaved.size, 0);
  assert.match(lines.at(-1)!, /✓ disk: .* free again; taking jobs/);
  // Free space under the full mark after pruning is full too, with nothing written failing.
  const k2 = new Keeper(b, DISK, () => {}, () => ({ free: 10 * GB, total: 1000 * GB }));
  assert.match(await k2.pass(7, new Map(), 2, now), /10 GB free, under the 30 GB mark/);
});

test("daemon.json is written over its own blocks when no new one can be had", () => {
  const file = join(tmp, "daemon.json");
  writeStatus(file, JSON.stringify({ status: "idle" }));
  assert.equal(statSync(file).size, 4096, "padded to a block");
  symlinkSync("/dev/full", `${file}.tmp`);
  writeStatus(file, JSON.stringify({ status: "disk full" }));
  assert.equal(JSON.parse(readFileSync(file, "utf8")).status, "disk full");
});

test("health: a daemon silent for minutes, and a full disk, each say what is wrong and the command that fixes it", () => {
  const sp = { free: 360 * GB, total: 1800 * GB };
  const d = { pid: 1, mode: "systemd", rev: "x", since: daysAgo(3), beat: daysAgo(63 / 24), status: "idle", alive: false };
  const [down] = health(d, DISK, sp, "/b", now);
  assert.match(down.what, /has not answered for 63h00/);
  assert.match(down.fix, /cargo xtask remote-render --update/);
  assert.equal(down.stops, "all");
  const full = health({ ...d, alive: true, beat: now.toISOString(), disk: { ...sp, at: "", full: "a write failed: ENOSPC" } }, DISK, sp, "/b", now);
  assert.equal(full.length, 1);
  assert.match(full[0].what, /disk is full \(a write failed: ENOSPC\)/);
  assert.match(full[0].fix, /du -xsh \/b\/slots\/\*\/target/);
  assert.match(health(undefined, DISK, { free: 5 * GB, total: 1800 * GB }, "/b", now)[1].what, /5 GB free, under the 30 GB mark/);
  assert.deepEqual(health({ ...d, alive: true }, DISK, sp, "/b", now), []);
});
