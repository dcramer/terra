import assert from "node:assert/strict";
import { test } from "node:test";
import { abandoned, collected, decode, encode, expired, inWindow, type Job, jobEnv, jobId, mayStart, parseClient, parseWindow, pick, type Rec, steps } from "./jobs.ts";

const job = (over: Partial<Job> = {}): Job => ({
  id: "20261023-120000-aaaa",
  rev: "0123456789abcdef0123456789abcdef01234567",
  command: ["cargo", "xtask", "trailer", "--cut", "x.json"],
  collect: [],
  cook: true,
  minutes: 60,
  by: "agent@box",
  queued: "2026-10-23T12:00:00.000Z",
  state: "queued",
  ...over,
});
const rec = (j: Job): Rec => ({ ref: j.id, job: j, files: [] });

test("a job survives its release notes, and other notes are not jobs", () => {
  const j = job({ note: "a ``` b" });
  assert.deepEqual(decode(encode(j)), j);
  assert.equal(decode("```json\n{}\n```"), undefined);
  assert.equal(decode("Release v1"), undefined);
});

test("job ids sort by when they were queued and are valid tags", () => {
  const a = jobId(new Date("2026-10-23T09:05:01Z"), "beef");
  const b = jobId(new Date("2026-10-23T10:00:00Z"), "0000");
  assert.equal(a, "20261023-090501-beef");
  assert.ok(a < b);
  assert.match(a, /^[0-9a-z-]+$/);
});

test("the client's arguments: the command after --, options before it", () => {
  const a = parseClient(["--rev", "abc", "--collect", "shot.png", "--collect", "~/x", "--minutes", "30", "--no-cook", "--", "cargo", "xtask", "bench", "--runs", "3"]);
  assert.deepEqual(a, { mode: "queue", rev: "abc", command: ["cargo", "xtask", "bench", "--runs", "3"], collect: ["shot.png", "~/x"], cook: false, minutes: 30, out: undefined, wait: true, local: false });
  assert.deepEqual(parseClient(["--status"]), { mode: "status", local: false });
  assert.deepEqual(parseClient(["--wait", "j1", "--out", "d"]), { mode: "wait", id: "j1", out: "d", local: false });
  assert.ok("error" in parseClient(["--rev", "abc"]));
  assert.ok("error" in parseClient(["--minutes", "9999", "--", "x"]));
  assert.ok("error" in parseClient(["--wait"]));
  assert.ok("error" in parseClient(["--collect", "../etc", "--", "x"]));
  assert.ok("error" in parseClient(["--frob", "--", "x"]));
});

test("a window wraps past midnight, and the policy puts CI first", () => {
  const night = parseWindow("22:00-08:00");
  assert.deepEqual(night, { from: 1320, to: 480 });
  assert.equal(parseWindow(""), undefined);
  assert.ok(parseWindow("22-8") && "error" in parseWindow("22-8")!);
  const w = night as { from: number; to: number };
  assert.ok(inWindow(w, 23 * 60) && inWindow(w, 7 * 60 + 59) && !inWindow(w, 8 * 60) && !inWindow(w, 12 * 60));
  assert.ok(inWindow({ from: 600, to: 720 }, 650) && !inWindow({ from: 600, to: 720 }, 720));
  assert.deepEqual(mayStart({ afterCi: true }, 600, true), { ok: false, why: "nightly is running" });
  assert.deepEqual(mayStart({ afterCi: false }, 600, true), { ok: true });
  assert.deepEqual(mayStart({ afterCi: true, window: w }, 600, false), { ok: false, why: "outside the window" });
  assert.deepEqual(mayStart({ afterCi: true, window: w }, 30, false), { ok: true });
});

test("one job at a time, the oldest first; old jobs expire, a dead worker's job is abandoned", () => {
  const a = rec(job({ id: "20261023-100000-a" }));
  const b = rec(job({ id: "20261023-090000-b" }));
  assert.equal(pick([a, b])?.job.id, b.job.id);
  assert.equal(pick([a, rec(job({ state: "running" }))]), undefined);
  assert.equal(pick([rec(job({ state: "done" }))]), undefined);
  const now = new Date("2026-10-31T12:00:00Z");
  const old = rec(job({ state: "done", ended: "2026-10-23T13:00:00Z" }));
  const recent = rec(job({ state: "done", ended: "2026-10-30T13:00:00Z" }));
  assert.deepEqual(expired([old, recent, rec(job({ state: "running" }))], now, 7), [old]);
  const started = "2026-10-31T10:00:00Z";
  assert.ok(abandoned(rec(job({ state: "running", started, minutes: 30 })), now));
  assert.ok(!abandoned(rec(job({ state: "running", started, minutes: 90 })), now));
});

test("the job's environment holds no token, and cargo keeps its homes", () => {
  const env = jobEnv({ HOME: "/root", PATH: "/bin", GH_TOKEN: "t", GITHUB_TOKEN: "t", TRIPO_STUDIO_COOKIE: "c", CARGO_HOME: "/usr/local/cargo" }, "/root", "j1");
  assert.deepEqual(env, { PATH: "/bin", HOME: "/root", CARGO_HOME: "/usr/local/cargo", RUSTUP_HOME: "/root/.rustup", TERRA_RENDER_JOB: "j1" });
});

test("only what the command wrote is collected, never a film's frames", () => {
  assert.ok(collected("trailer/cut.mp4", 2000, 1500));
  assert.ok(!collected("trailer/cut.mp4", 100, 1500));
  assert.ok(!collected("trailer/frames/lake/f-001.png", 2000, 1500));
});

test("the worker's steps: only origin's branches, then cook unless told not, then the command", () => {
  const s = steps(job());
  assert.deepEqual(s.map((x) => x.what), ["fetch origin's branches", "check the commit is on a branch of origin", "check it out", "clean all but the build", "cook textures", "run"]);
  assert.deepEqual(s[0].cmd.at(-1), "+refs/heads/*:refs/remotes/origin/*");
  assert.deepEqual(s.at(-1)!.cmd, ["nice", "-n", "10", "cargo", "xtask", "trailer", "--cut", "x.json"]);
  assert.ok(!steps(job({ cook: false })).some((x) => x.what === "cook textures"));
});
