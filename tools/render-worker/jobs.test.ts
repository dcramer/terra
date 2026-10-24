import assert from "node:assert/strict";
import { test } from "node:test";
import { boxSettings, chooseWorker, collected, config, expired, gpuFix, inWindow, type Job, jobEnv, jobId, mayStart, parseClient, parseEnvFile, parseWindow, pick, steps, WARM } from "./jobs.ts";

const job = (over: Partial<Job> = {}): Job => ({
  id: "20261023-120000-aaaa",
  kind: "run",
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

test(".env files: comments, export and quotes", () => {
  const text = "# the workers\nexport TERRA_WORKERS=box-a, box-b\nTERRA_WORKER_DIR='renders'\nTERRA_RENDER_WINDOW=\"22:00-08:00\"\nX=1 # trailing\n\nnot a line\n";
  assert.deepEqual(parseEnvFile(text), { TERRA_WORKERS: "box-a, box-b", TERRA_WORKER_DIR: "renders", TERRA_RENDER_WINDOW: "22:00-08:00", X: "1" });
});

test("the client's settings: the environment over .env, every one checked", () => {
  const file = { TERRA_WORKERS: "box-a", TERRA_WORKER_DIR: "renders", TERRA_RENDER_MINUTES: "30" };
  assert.deepEqual(config({}, file), { workers: ["box-a"], dir: "renders", minutes: 30, connectTimeout: 10 });
  assert.deepEqual(config({ TERRA_WORKERS: " box-b , user@box-c ", TERRA_RENDER_MINUTES: "" }, file), { workers: ["box-b", "user@box-c"], dir: "renders", minutes: 30, connectTimeout: 10 });
  assert.deepEqual(config({ TERRA_WORKERS: "box-a" }), { workers: ["box-a"], dir: "terra-render", minutes: 120, connectTimeout: 10 });
  assert.match((config({}) as { error: string }).error, /TERRA_WORKERS/);
  assert.ok("error" in config({ TERRA_WORKERS: "-oProxyCommand=x" }), "a host is never an ssh option");
  assert.ok("error" in config({ TERRA_WORKERS: "box a" }));
  assert.ok("error" in config({ TERRA_WORKERS: "b", TERRA_WORKER_DIR: "../x" }));
  assert.ok("error" in config({ TERRA_WORKERS: "b", TERRA_WORKER_DIR: "a;rm" }));
  assert.ok("error" in config({ TERRA_WORKERS: "b", TERRA_RENDER_MINUTES: "9999" }));
});

test("job ids sort by when they were queued and are valid refs", () => {
  const a = jobId(new Date("2026-10-23T09:05:01Z"), "beef");
  assert.equal(a, "20261023-090501-beef");
  assert.ok(a < jobId(new Date("2026-10-23T10:00:00Z"), "0000"));
});

test("the client's arguments: the command after --, options before it, one mode", () => {
  const a = parseClient(["--rev", "abc", "--collect", "shot.png", "--collect", "~/x", "--minutes", "30", "--no-cook", "--worker", "box-a", "--", "cargo", "xtask", "bench", "--runs", "3"]);
  assert.deepEqual(a, { mode: "queue", rev: "abc", command: ["cargo", "xtask", "bench", "--runs", "3"], collect: ["shot.png", "~/x"], cook: false, minutes: 30, out: undefined, wait: true, worker: "box-a" });
  assert.equal((parseClient(["--", "x"], 45) as { minutes: number }).minutes, 45, ".env's limit is the default");
  assert.deepEqual(parseClient(["--status"]), { mode: "status", worker: undefined });
  assert.deepEqual(parseClient(["--update", "--worker", "b"]), { mode: "update", worker: "b" });
  assert.deepEqual(parseClient(["--wait", "20261023-120000-aaaa", "--out", "d"]), { mode: "wait", id: "20261023-120000-aaaa", out: "d", worker: undefined });
  assert.deepEqual(parseClient(["--logs", "daemon"]), { mode: "logs", id: "daemon", worker: undefined });
  for (const bad of [["--rev", "abc"], ["--minutes", "9999", "--", "x"], ["--wait"], ["--cancel", "x;rm"], ["--collect", "../etc", "--", "x"], ["--frob", "--", "x"], ["--status", "--doctor"], ["--worker", "-oX", "--status"]]) {
    assert.ok("error" in parseClient(bad), bad.join(" "));
  }
});

test("a job goes to a running daemon with the shortest queue, else waits on a box that answers", () => {
  const s = (host: string, reachable: boolean, alive: boolean, waiting: number) => ({ host, reachable, alive, waiting });
  assert.deepEqual(chooseWorker([s("a", true, true, 2), s("b", true, true, 0)]), { host: "b" });
  assert.deepEqual(chooseWorker([s("a", true, true, 1), s("b", true, true, 1)]), { host: "a" });
  assert.deepEqual(chooseWorker([s("a", false, false, 0), s("b", true, true, 5)]), { host: "b" });
  const down = chooseWorker([s("a", false, false, 0), s("b", true, false, 0)]);
  assert.equal("host" in down && down.host, "b");
  assert.ok("host" in down && down.note);
  assert.ok("error" in chooseWorker([s("a", false, false, 0)]));
});

test("a window wraps past midnight, and a CI lock holds jobs back", () => {
  const night = parseWindow("22:00-08:00") as { from: number; to: number };
  assert.deepEqual(night, { from: 1320, to: 480 });
  assert.equal(parseWindow(""), undefined);
  assert.ok("error" in (parseWindow("22-8") as object));
  assert.ok(inWindow(night, 23 * 60) && inWindow(night, 7 * 60 + 59) && !inWindow(night, 8 * 60) && !inWindow(night, 12 * 60));
  assert.ok(!mayStart({ afterCi: true }, 600, true).ok);
  assert.ok(mayStart({ afterCi: false }, 600, true).ok);
  assert.deepEqual(mayStart({ afterCi: true, window: night }, 600, false), { ok: false, why: "outside the window" });
  assert.ok(mayStart({ afterCi: true, window: night }, 30, false).ok);
  assert.deepEqual(boxSettings({ TERRA_RENDER_WINDOW: "22:00-08:00", TERRA_RENDER_KEEP_DAYS: "3" }), { policy: { window: night, afterCi: true }, ciLock: undefined, keepDays: 3 });
  assert.ok("error" in boxSettings({ TERRA_RENDER_KEEP_DAYS: "0" }));
});

test("one job at a time: a warm build first, then the oldest; old jobs expire", () => {
  const a = job({ id: "20261023-100000-aaaa" });
  const b = job({ id: "20261023-090000-bbbb" });
  const warm = job({ id: "20261023-110000-cccc", kind: "warm" });
  assert.equal(pick([a, b])?.id, b.id);
  assert.equal(pick([a, b, warm])?.id, warm.id);
  assert.equal(pick([a, job({ state: "running" })]), undefined);
  assert.equal(pick([job({ state: "cancelled" })]), undefined);
  const now = new Date("2026-10-31T12:00:00Z");
  const old = job({ state: "done", ended: "2026-10-23T13:00:00Z" });
  assert.deepEqual(expired([old, job({ state: "done", ended: "2026-10-30T13:00:00Z" }), job({ state: "running" })], now, 7), [old]);
});

test("the job's environment holds no token or agent socket, and cargo keeps its homes", () => {
  const env = jobEnv({ HOME: "/home/u", PATH: "/bin", GH_TOKEN: "t", TRIPO_STUDIO_COOKIE: "c", SSH_AUTH_SOCK: "/s" }, "/box/home", "j1");
  assert.deepEqual(env, { PATH: "/bin", HOME: "/box/home", CARGO_HOME: "/home/u/.cargo", RUSTUP_HOME: "/home/u/.rustup", TERRA_RENDER_JOB: "j1" });
});

test("only what the command wrote is collected, never a film's frames", () => {
  assert.ok(collected("trailer/cut.mp4", 2000, 1500));
  assert.ok(!collected("trailer/cut.mp4", 100, 1500));
  assert.ok(!collected("trailer/frames/lake/f-001.png", 2000, 1500));
});

test("a job's steps: its pushed ref, a clean checkout, cook unless told not, the command niced", () => {
  const s = steps(job(), "/box/repo");
  assert.deepEqual(s.map((x) => x.what), ["fetch the job's commit", "check it out", "clean all but the build", "cook textures", "run"]);
  assert.deepEqual(s[0].cmd.slice(-2), ["/box/repo", "+refs/jobs/20261023-120000-aaaa:refs/jobs/20261023-120000-aaaa"]);
  assert.ok(s[2].cmd.includes("target/"), "the build is kept");
  assert.deepEqual(s.at(-1)!.cmd, ["nice", "-n", "10", "cargo", "xtask", "trailer", "--cut", "x.json"]);
  assert.ok(!steps(job({ cook: false }), "r").some((x) => x.what === "cook textures"));
  const warm = steps(job({ kind: "warm" }), "r");
  assert.equal(warm[0].cmd.at(-1), "+refs/worker/main:refs/worker/main");
  assert.deepEqual(warm.filter((x) => x.run).map((x) => x.cmd.slice(3)), WARM);
});

test("a card the driver cannot reach gets the commands that fix it", () => {
  assert.match(gpuFix("6.8.0-146-generic", "535", false).join("\n"), /linux-modules-nvidia-535-generic[\s\S]*reboot/);
  assert.match(gpuFix("k", "535", true).join("\n"), /modprobe nvidia/);
  assert.match(gpuFix("k", undefined, false).join("\n"), /ubuntu-drivers/);
});
