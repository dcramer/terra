import assert from "node:assert/strict";
import { test } from "node:test";
import { boxSettings, chooseWorker, collected, config, displayPlan, expired, freeDisplay, gpuFix, inWindow, type Job, jobClass, jobEnv, jobId, mayStart, needsDisplay, parseClient, parseEnvFile, parseWindow, plan, steps, takenDisplays, WARM, xvfbArgs, xvfbBackoff } from "./jobs.ts";

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
  assert.equal((parseClient(["--cpu", "--", "x"]) as { class?: string }).class, "cpu");
  assert.equal((parseClient(["--", "x"]) as { class?: string }).class, undefined);
  assert.ok("error" in parseClient(["--cpu", "--gpu", "--", "x"]));
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
  assert.deepEqual(boxSettings({ TERRA_RENDER_WINDOW: "22:00-08:00", TERRA_RENDER_KEEP_DAYS: "3" }), { policy: { window: night, afterCi: true }, ciLock: undefined, keepDays: 3, slots: { slots: 4, gpuSlots: 3 } });
  assert.ok("error" in boxSettings({ TERRA_RENDER_KEEP_DAYS: "0" }));
  assert.deepEqual((boxSettings({ TERRA_RENDER_SLOTS: "2" }) as { slots: object }).slots, { slots: 2, gpuSlots: 2 }, "never more GPU slots than slots");
  assert.deepEqual((boxSettings({ TERRA_RENDER_SLOTS: "8", TERRA_RENDER_GPU_SLOTS: "1" }) as { slots: object }).slots, { slots: 8, gpuSlots: 1 });
  assert.ok("error" in boxSettings({ TERRA_RENDER_SLOTS: "0" }));
  assert.ok("error" in boxSettings({ TERRA_RENDER_GPU_SLOTS: "x" }));
});

test("a job's class: from its command unless --gpu or --cpu says; a bench always has the box", () => {
  assert.equal(jobClass(["cargo", "xtask", "trailer", "--cut", "x.json"]), "gpu");
  assert.equal(jobClass(["cargo", "xtask", "sandbox", "--offscreen"]), "gpu");
  assert.equal(jobClass(["cargo", "xtask", "bench"]), "bench");
  assert.equal(jobClass(["cargo", "xtask", "gpu", "bench", "--", "x"]), "bench");
  assert.equal(jobClass(["cargo", "xtask", "bench"], "cpu"), "bench");
  assert.equal(jobClass(["cargo", "test", "-p", "terra_gen"]), "cpu");
  assert.equal(jobClass(["cargo", "+nightly", "clippy", "--workspace"]), "cpu");
  assert.equal(jobClass(["cargo", "-q", "check"]), "cpu");
  assert.equal(jobClass(["cargo", "xtask", "check", "--base", "abc"]), "cpu");
  assert.equal(jobClass(["node", "tools/sandbox/film.ts"]), "gpu", "anything else may draw");
  assert.equal(jobClass(["node", "x.ts"], "cpu"), "cpu");
  assert.equal(jobClass(["cargo", "test"], "gpu"), "gpu");
});

test("slots: four jobs at once, the GPU cap holds and CPU jobs pass, a bench waits for an empty box", () => {
  const at = (n: number, over: Partial<Job> = {}) => job({ id: `20261023-1000${String(n).padStart(2, "0")}-aaaa`, ...over });
  const cpu = { class: "cpu" as const };
  const gpu = { class: "gpu" as const };
  const s = { slots: 4, gpuSlots: 3 };
  const slotsOf = (r: ReturnType<typeof plan>) => r.start.map((x) => [x.job.id.slice(13, 15), x.slot]);

  // Five CPU jobs: four start at once, one in each slot, the oldest first; the fifth waits.
  const five = [5, 1, 3, 2, 4].map((n) => at(n, cpu));
  const p = plan(five, s);
  assert.deepEqual(slotsOf(p), [["01", 1], ["02", 2], ["03", 3], ["04", 4]]);
  assert.equal(p.why, "all 4 slots are taken");

  // Four GPU jobs: three take the card; a CPU job behind them takes the fourth slot.
  const g = plan([at(1, gpu), at(2, gpu), at(3, gpu), at(4, gpu), at(5, cpu)], s);
  assert.deepEqual(slotsOf(g), [["01", 1], ["02", 2], ["03", 3], ["05", 4]]);
  assert.equal(g.why, "the 3 GPU slots are taken");
  // With three GPU jobs running, no GPU job starts, whatever slots are free.
  const running = [1, 2, 3].map((n) => at(n, { ...gpu, state: "running", slot: n + 1 }));
  assert.deepEqual(slotsOf(plan([...running, at(4, gpu), at(5, cpu)], s)), [["05", 1]]);
  // A job queued before classes is classed by its command (a trailer: GPU).
  assert.deepEqual(plan([...running, at(4)], s).start, []);

  // A bench waits for the box to empty, and nothing queued behind it starts meanwhile.
  const bench = at(2, { class: "bench" });
  const b1 = plan([at(1, { ...cpu, state: "running", slot: 3 }), bench, at(3, cpu)], s);
  assert.deepEqual(b1, { start: [], why: "a bench waits for the box to empty" });
  // Jobs ahead of it still start; then it waits for them.
  assert.deepEqual(slotsOf(plan([at(1, cpu), bench, at(3, cpu)], s)), [["01", 1]]);
  // On an empty box it starts alone, and while it runs nothing else does.
  assert.deepEqual(slotsOf(plan([bench, at(3, cpu)], s)), [["02", 1]]);
  assert.deepEqual(plan([{ ...bench, state: "running", slot: 1 }, at(3, cpu)], s), { start: [], why: "a bench has the box" });
});

test("warm builds go first, each in the slot it warms, and other jobs keep off those slots", () => {
  const warm = (slot: number) => job({ id: `20261023-110000-ccc${slot}`, kind: "warm", slot, class: "cpu" });
  const older = job({ id: "20261023-090000-bbbb", class: "cpu" });
  const p = plan([older, warm(1), warm(2), warm(3), warm(4)], { slots: 4, gpuSlots: 3 });
  assert.deepEqual(p.start.map((x) => [x.job.id, x.slot]), [1, 2, 3, 4].map((n) => [`20261023-110000-ccc${n}`, n]));
  // Slot 2 busy: its warm build waits for it, and the run job does not take slot 2's turn.
  const busy = job({ id: "20261023-080000-dddd", class: "cpu", state: "running", slot: 2 });
  const q = plan([busy, older, warm(1), warm(2), warm(3), warm(4)], { slots: 4, gpuSlots: 3 });
  assert.deepEqual(q.start.map((x) => x.slot), [1, 3, 4]);
  assert.ok(!q.start.some((x) => x.job.id === older.id));
  // A warm build for a slot the box no longer has takes any free one.
  assert.deepEqual(plan([warm(4)], { slots: 2, gpuSlots: 2 }).start.map((x) => x.slot), [1]);
});

test("old jobs expire", () => {
  const now = new Date("2026-10-31T12:00:00Z");
  const old = job({ state: "done", ended: "2026-10-23T13:00:00Z" });
  assert.deepEqual(expired([old, job({ state: "done", ended: "2026-10-30T13:00:00Z" }), job({ state: "running" })], now, 7), [old]);
});

test("the job's environment holds no token or agent socket, cargo keeps its homes, and the build is the slot's", () => {
  const env = jobEnv({ HOME: "/home/u", PATH: "/bin", GH_TOKEN: "t", TRIPO_STUDIO_COOKIE: "c", SSH_AUTH_SOCK: "/s", CARGO_TARGET_DIR: "/shared" }, "/box/homes/2", "j1", "/box/slots/2/target");
  assert.deepEqual(env, { PATH: "/bin", HOME: "/box/homes/2", CARGO_HOME: "/home/u/.cargo", RUSTUP_HOME: "/home/u/.rustup", CARGO_TARGET_DIR: "/box/slots/2/target", TERRA_RENDER_JOB: "j1" });
});

test("a job that draws gets a display: TERRA_RENDER_DISPLAY, else the daemon's own, else an Xvfb", () => {
  assert.deepEqual(displayPlan({}), { how: "xvfb" }, "a systemd user service has none");
  assert.deepEqual(displayPlan({ TERRA_RENDER_DISPLAY: "" }), { how: "xvfb" }, "empty is auto");
  assert.deepEqual(displayPlan({ DISPLAY: ":0" }), { how: "inherit", said: "DISPLAY=:0" });
  assert.deepEqual(displayPlan({ WAYLAND_DISPLAY: "wayland-1", DISPLAY: ":0" }), { how: "inherit", said: "WAYLAND_DISPLAY=wayland-1" });
  assert.deepEqual(displayPlan({ TERRA_RENDER_DISPLAY: "1", WAYLAND_DISPLAY: "wayland-1" }), { how: "fixed", display: ":1" }, "the setting wins");
  assert.deepEqual(displayPlan({ TERRA_RENDER_DISPLAY: " :7.0 " }), { how: "fixed", display: ":7.0" });
  assert.ok("error" in displayPlan({ TERRA_RENDER_DISPLAY: "host:0" }), "never a remote display");
  assert.ok("error" in displayPlan({ TERRA_RENDER_DISPLAY: ":0; rm -rf ~" }));
});

test("the Xvfb's number is the first free one from 99, by sockets and lock files", () => {
  const taken = takenDisplays(["X0", "X0_", "X99", ".X100-lock", ".X11-unix", "Xauthority", "X1x"]);
  assert.deepEqual([...taken].sort((a, b) => a - b), [0, 99, 100]);
  assert.equal(freeDisplay(taken), 101);
  assert.equal(freeDisplay(new Set()), 99);
  assert.deepEqual(xvfbArgs(101).slice(0, 4), [":101", "-screen", "0", "1920x1080x24"]);
  assert.ok(xvfbArgs(101).includes("-nolisten"), "no TCP");
  assert.deepEqual([1, 2, 3, 10].map(xvfbBackoff), [2, 4, 8, 60], "an Xvfb that keeps dying is started less often");
});

test("renders, benches and sandbox runs need a display; builds and warm jobs don't; the job's DISPLAY replaces Wayland", () => {
  assert.ok(needsDisplay(job()));
  assert.ok(needsDisplay(job({ command: ["cargo", "xtask", "bench"] })));
  assert.ok(needsDisplay(job({ class: "cpu", command: ["cargo", "xtask", "sandbox", "--offscreen", "--experiment", "play"] })), "a sandbox run under --cpu still opens winit");
  assert.ok(!needsDisplay(job({ command: ["cargo", "test", "-p", "terra_gen"] })));
  assert.ok(!needsDisplay(job({ kind: "warm", command: [] })));
  const env = jobEnv({ HOME: "/h", WAYLAND_DISPLAY: "wayland-1" }, "/b/h", "j", "/b/t", ":99");
  assert.equal(env.DISPLAY, ":99");
  assert.equal(env.WAYLAND_DISPLAY, undefined);
  assert.equal(jobEnv({ HOME: "/h", DISPLAY: ":0" }, "/b/h", "j", "/b/t").DISPLAY, ":0", "no display given keeps the daemon's");
});

test("only what the command wrote is collected, never a film's frames", () => {
  assert.ok(collected("trailer/cut.mp4", 2000, 1500));
  assert.ok(!collected("trailer/cut.mp4", 100, 1500));
  assert.ok(!collected("trailer/frames/lake/f-001.png", 2000, 1500));
});

test("a job's steps: a clean checkout of its commit, cook unless told not, the command niced", () => {
  const s = steps(job());
  assert.deepEqual(s.map((x) => x.what), ["check out the job's commit", "clean all but the build", "cook textures", "run"]);
  assert.equal(s[0].cmd.at(-1), job().rev);
  assert.ok(s[1].cmd.includes("target/"), "the build is kept");
  assert.deepEqual(s.at(-1)!.cmd, ["nice", "-n", "10", "cargo", "xtask", "trailer", "--cut", "x.json"]);
  assert.ok(!steps(job({ cook: false })).some((x) => x.what === "cook textures"));
  const warm = steps(job({ kind: "warm" }));
  assert.deepEqual(warm.filter((x) => x.run).map((x) => x.cmd.slice(3)), WARM);
});

test("a card the driver cannot reach gets the commands that fix it", () => {
  assert.match(gpuFix("6.8.0-146-generic", "535", false).join("\n"), /linux-modules-nvidia-535-generic[\s\S]*reboot/);
  assert.match(gpuFix("k", "535", true).join("\n"), /modprobe nvidia/);
  assert.match(gpuFix("k", undefined, false).join("\n"), /ubuntu-drivers/);
});
