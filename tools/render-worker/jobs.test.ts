import assert from "node:assert/strict";
import { test } from "node:test";
import { benchHold, boxSettings, buildAhead, checkoutsInBuild, chooseWorker, cleanCommands, collected, commandKey, config, estimator, displayPlan, expired, freeDisplay, gpuFix, ID, incrementalClears, inWindow, type Job, jobClass, jobEnv, jobId, mayStart, needsDisplay, parseClient, parseEnvFile, parseWindow, performanceCores, plan, recentJobs, rustcIce, short, staleBuild, steps, takenDisplays, WARM, workspacePackages, xvfbArgs, xvfbBackoff } from "./jobs.ts";

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
  assert.deepEqual(a, { mode: "queue", rev: "abc", dirty: false, command: ["cargo", "xtask", "bench", "--runs", "3"], collect: ["shot.png", "~/x"], cook: false, minutes: 30, out: undefined, wait: true, worker: "box-a" });
  assert.equal((parseClient(["--", "x"], 45) as { minutes: number }).minutes, 45, ".env's limit is the default");
  assert.equal((parseClient(["--cpu", "--", "x"]) as { class?: string }).class, "cpu");
  assert.equal((parseClient(["--", "x"]) as { class?: string }).class, undefined);
  assert.ok("error" in parseClient(["--cpu", "--gpu", "--", "x"]));
  assert.deepEqual(parseClient(["--status"]), { mode: "status", worker: undefined });
  assert.deepEqual(parseClient(["--status", "--all"]), { mode: "status", worker: undefined, all: true });
  assert.ok("error" in parseClient(["--update", "--all"]));
  assert.deepEqual(parseClient(["--update", "--worker", "b"]), { mode: "update", worker: "b" });
  assert.deepEqual(parseClient(["--wait", "20261023-120000-aaaa", "--out", "d"]), { mode: "wait", id: "20261023-120000-aaaa", out: "d", worker: undefined });
  assert.deepEqual(parseClient(["--logs", "daemon"]), { mode: "logs", id: "daemon", worker: undefined });
  for (const bad of [["--rev", "abc"], ["--minutes", "9999", "--", "x"], ["--wait"], ["--cancel", "x;rm"], ["--collect", "../etc", "--", "x"], ["--frob", "--", "x"], ["--status", "--doctor"], ["--worker", "-oX", "--status"]]) {
    assert.ok("error" in parseClient(bad), bad.join(" "));
  }
  // A comma list is one path nowhere: refused, saying to repeat --collect.
  assert.deepEqual(parseClient(["--collect", "a.png,shots/b", "--", "x"]), { error: "--collect a.png,shots/b: one path a --collect; repeat it for each (--collect a.png --collect shots/b)" });
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
  // The push hook's check is a check, --cpu (as remote-check.ts asks) or not; --all's tests are not.
  assert.equal(jobClass(["cargo", "xtask", "check", "--base", "abc"]), "check");
  assert.equal(jobClass(["cargo", "xtask", "check", "--base", "abc"], "cpu"), "check");
  assert.equal(jobClass(["cargo", "xtask", "check", "--all"]), "cpu");
  assert.equal(jobClass(["cargo", "xtask", "check"], "gpu"), "gpu");
  assert.equal(jobClass(["sh", "-c", "cargo xtask check"]), "gpu", "only the command itself");
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

  // A bench builds in a free slot while the box drains for it; a job behind it of a kind never
  // run waits.
  const bench = at(2, { class: "bench" });
  const b1 = plan([at(1, { ...cpu, state: "running", slot: 3 }), bench, at(3, cpu)], s);
  assert.deepEqual(slotsOf(b1), [["02", 1]]);
  assert.equal(b1.why, "a bench waits for the box to empty");
  // Built, it holds its slot until the box is empty but for it, then is let go to its timed run,
  // and while that runs nothing else starts.
  const built = { ...bench, state: "running" as const, slot: 1, phase: "ready" as const };
  assert.deepEqual(plan([at(1, { ...cpu, state: "running", slot: 3 }), built, at(3, cpu)], s), { start: [], why: "a bench waits for the box to empty" });
  assert.deepEqual(plan([built, at(3, cpu)], s), { start: [], go: built, why: "a bench has the box" });
  assert.deepEqual(plan([{ ...built, phase: "timed" }, at(3, cpu)], s), { start: [], why: "a bench has the box" });
  // A bench started before phases is in its timed run.
  assert.deepEqual(plan([{ ...bench, state: "running", slot: 1 }, at(3, cpu)], s), { start: [], why: "a bench has the box" });
  // On an empty box it starts building, and a GPU job behind of a kind never run waits for it.
  assert.deepEqual(slotsOf(plan([bench, at(3, gpu)], s)), [["02", 1]]);
  // A warm build queued after it (an update) waits behind it too, and keeps its own slot.
  const warmAfter = at(4, { kind: "warm", slot: 1, class: "cpu" });
  assert.deepEqual(slotsOf(plan([at(1, { ...cpu, state: "running", slot: 3 }), bench, warmAfter], s)), [["02", 2]]);
});

test("--status lists every live job and the last day's ended, at least the last ten", () => {
  const now = new Date("2026-10-29T12:00:00.000Z");
  const old = Array.from({ length: 30 }, (_, i) => job({ id: `20261025-1000${String(i).padStart(2, "0")}-aaaa`, queued: "2026-10-25T10:00:00.000Z", state: "done" }));
  const live = job({ id: "20261025-110000-bbbb", queued: "2026-10-25T11:00:00.000Z", state: "queued" });
  const today = job({ id: "20261029-110000-cccc", queued: "2026-10-29T11:00:00.000Z", state: "failed" });
  const shown = recentJobs([...old, live, today], now);
  assert.deepEqual(shown, [...old.slice(-9), live, today]);
  assert.equal(recentJobs([...old, live, today], now, 24, 1).length, 2);
});

// ---- A bench's drain, and benches back to back (D319) ----

const T0 = Date.parse("2026-10-28T12:00:00.000Z");
const iso = (minutes: number) => new Date(T0 + minutes * 60_000).toISOString();
/// A job at minute `q` of the clock (its id and queue time), as a shell command.
const at = (q: number, command: string, over: Partial<Job> = {}): Job =>
  job({ id: `20261028-${String(1200 + q).padStart(4, "0")}00-${String(q).padStart(4, "0")}`, queued: iso(q), command: command.split(" "), ...over });
/// One that ran from minute `from` for `took` minutes.
const ran = (q: number, command: string, took: number, over: Partial<Job> = {}) => at(q, command, { state: "done", started: iso(q), ended: iso(q + took), ...over });
const s4 = { slots: 4, gpuSlots: 3 };
const ids = (r: ReturnType<typeof plan>) => r.start.map((x) => x.job.command.join(" "));

test("a job's kind blanks its numbers and revisions; its time is the longest of the last few", () => {
  assert.equal(commandKey(at(0, "sh firejob/job.sh art c3")), commandKey(at(0, "sh firejob/job.sh art c14")));
  assert.equal(commandKey(at(0, "cargo xtask check --base 0123456789abcdef")), "cargo xtask check --base <rev>");
  assert.notEqual(commandKey(at(0, "sh film.sh main")), commandKey(at(0, "sh film.sh woods")));
  const est = estimator([ran(-100, "sh film.sh 1", 30), ran(-60, "sh film.sh 2", 20), ran(-30, "sh film.sh 3", 25, { state: "failed" }), ran(-10, "cargo test -p x", 2)]);
  assert.equal(est(at(0, "sh film.sh 4"), "long"), 30 * 60_000, "the longest, a failed run too");
  assert.equal(est(at(0, "sh film.sh 4"), "short"), 20 * 60_000, "the shortest done");
  assert.equal(est(at(0, "sh film.sh 4", { minutes: 10 }), "long"), 10 * 60_000, "never past the job's own limit");
  assert.equal(est(at(0, "node new.ts", { class: "gpu" }), "long"), undefined, "a GPU job of a new kind is not guessed");
  assert.equal(est(at(0, "cargo test -p y", { class: "cpu" }), "long"), 2 * 60_000, "a CPU job of a new kind: the longest recent CPU job");
  assert.equal(est(at(0, "cargo test -p y", { class: "cpu" }), "short"), undefined);
});

test("while the box drains for a bench, only jobs that will be over before it has drained start", () => {
  const history = [ran(-200, "sh film.sh 1", 30, { class: "gpu" }), ran(-150, "cargo test -p a", 1, { class: "cpu" }), ran(-120, "sh long.sh", 40, { class: "gpu" }), ran(-90, "sh still.sh", 3, { class: "gpu" })];
  const film = at(0, "sh film.sh 2", { class: "gpu", state: "running", started: iso(0), slot: 1 });
  const bench = at(1, "cargo xtask bench crypt", { class: "bench" });
  const behind = [at(2, "cargo test -p a", { class: "cpu" }), at(3, "sh long.sh", { class: "gpu" }), at(4, "node new.ts", { class: "gpu" }), at(5, "sh still.sh", { class: "gpu" }), at(6, "cargo test -p b", { class: "cpu" })];
  // The film has 25 minutes left at the least: the bench builds in a slot, and the test, the
  // still and the new test (as long as the longest CPU job, a minute) may fill the free ones;
  // the 40-minute job and the new GPU kind wait.
  const p = plan([...history, film, bench, ...behind], s4, T0 + 5 * 60_000);
  assert.deepEqual(ids(p), ["cargo xtask bench crypt", "cargo test -p a", "sh still.sh"]);
  assert.equal(p.why, "a bench waits for the box to empty");
  const building = { ...bench, state: "running" as const, started: iso(2), slot: 2, phase: "prep" as const };
  assert.deepEqual(ids(plan([...history, film, building, ...behind], s4, T0 + 5 * 60_000)), ["cargo test -p a", "sh still.sh"]);
  // Two minutes before the film's soonest end the three-minute still no longer fits.
  assert.deepEqual(ids(plan([...history, film, building, ...behind], s4, T0 + 28 * 60_000)), ["cargo test -p a", "cargo test -p b"]);
  // Past its shortest run the film may end any moment: nothing behind the bench starts.
  assert.deepEqual(plan([...history, film, building, ...behind], s4, T0 + 31 * 60_000).start, []);
  // A running job of a kind never run gives no end to fill up to.
  assert.deepEqual(plan([...history, { ...film, command: ["node", "other.ts"] }, building, ...behind], s4, T0 + 5 * 60_000).start, []);
  // A push check starts whatever its length while the film still runs, past its shortest run
  // too, or of a kind never run; a CPU job of its length does not.
  const check = at(7, "cargo xtask check --base abc", { class: "check" });
  assert.deepEqual(ids(plan([...history, film, building, ...behind, check], s4, T0 + 31 * 60_000)), ["cargo xtask check --base abc"]);
  assert.deepEqual(ids(plan([...history, { ...film, command: ["node", "other.ts"] }, building, check], s4, T0 + 5 * 60_000)), ["cargo xtask check --base abc"]);
  // Once only checks hold the box, the built bench waits for them, and no new check starts.
  const checking = { ...check, state: "running" as const, started: iso(30), slot: 3 };
  const ready = { ...building, phase: "ready" as const, prepared: iso(4) };
  const next = at(8, "cargo xtask check --base def", { class: "check" });
  const p2 = plan([...history, checking, ready, next], s4, T0 + 31 * 60_000);
  assert.equal(p2.go, undefined);
  assert.deepEqual(p2.start, []);
  assert.equal(plan([...history, { ...checking, state: "done", ended: iso(31) }, ready, next], s4, T0 + 32 * 60_000).go, ready);
  // A job queued ahead of the bench keeps its turn, however long, and the bench builds after it.
  const ahead = at(0.5, "sh long.sh", { class: "gpu", id: "20261028-120030-0000" });
  assert.deepEqual(ids(plan([...history, film, ahead, bench], s4, T0 + 5 * 60_000)), ["sh long.sh", "cargo xtask bench crypt"]);
});

test("benches queued together run back to back on the drained box; a job between goes first only when cheaper", () => {
  const history = [ran(-200, "cargo xtask bench crypt", 5, { class: "bench" }), ran(-150, "cargo xtask bench town", 20, { class: "bench" }), ran(-100, "cargo test -p a", 1, { class: "cpu" }), ran(-90, "sh film.sh 1", 40, { class: "gpu" })];
  const first = ran(0, "cargo xtask bench crypt", 5, { class: "bench", slot: 1 });
  const second = at(1, "cargo xtask bench crypt --runs 3", { class: "bench" });
  const later = at(2, "sh film.sh 2", { class: "gpu" });
  // While the first runs, nothing starts; once it ends, the second has the box before the film.
  assert.deepEqual(plan([...history, { ...first, state: "running", ended: undefined }, second, later], s4, T0 + 3 * 60_000), { start: [], why: "a bench has the box" });
  assert.deepEqual(ids(plan([...history, first, second, later], s4, T0 + 5 * 60_000)), ["cargo xtask bench crypt --runs 3"]);
  // A 40-minute film queued between them would drain the box again for 40 minutes: the bench
  // (5 minutes) goes first.
  const between = at(0.5, "sh film.sh 3", { class: "gpu", id: "20261028-120030-0000" });
  const b5 = at(1, "cargo xtask bench crypt", { class: "bench" });
  assert.deepEqual(ids(plan([...history, between, b5], s4, T0 + 5 * 60_000)), ["cargo xtask bench crypt"]);
  // A one-minute test between them waits less than the 20-minute bench would make it: it goes
  // first, the bench building beside it.
  const test1 = at(0.5, "cargo test -p a", { class: "cpu", id: "20261028-120030-0000" });
  const b20 = at(1, "cargo xtask bench town", { class: "bench" });
  assert.deepEqual(ids(plan([...history, test1, b20], s4, T0 + 5 * 60_000)), ["cargo test -p a", "cargo xtask bench town"]);
  // Built before the test is over, the bench is let go only once it is.
  const built = { ...b20, state: "running" as const, started: iso(5), prepared: iso(6), slot: 2, phase: "ready" as const };
  assert.equal(plan([...history, { ...test1, state: "running", started: iso(5), slot: 1 }, built], s4, T0 + 6 * 60_000).go, undefined);
  assert.equal(plan([...history, built], s4, T0 + 7 * 60_000).go, built);
});

test("a bench builds ahead: what its command will build, before a timed run that has the box alone", () => {
  // What it builds is what the bench would build first, in the same profile.
  const sandbox = (profile: string) => ["cargo", "build", "-q", "--profile", profile, "-p", "terra_sandbox"];
  const xtask = ["cargo", "build", "-q", "-p", "xtask"];
  assert.deepEqual(buildAhead("cargo xtask bench crypt --runs 3".split(" ")), [xtask, sandbox("bench")]);
  assert.deepEqual(buildAhead("cargo xtask bench crypt --debug".split(" ")), [xtask, sandbox("dev")]);
  assert.deepEqual(buildAhead("cargo xtask gpu bench -- cargo xtask bench town".split(" ")), [xtask, sandbox("bench")]);
  assert.deepEqual(buildAhead("cargo xtask gpu bench -- node castlejob/bench.ts run cc5f952b".split(" ")), [xtask], "another command's builds stay in its run");
  // Every step before the run is the build ahead; the run alone is timed, on the performance cores.
  const s = steps(job({ class: "bench", command: "cargo xtask bench crypt".split(" ") }), "0-15");
  assert.deepEqual(s.map((x) => [x.what, !!x.prep]), [["check out the job's commit", true], ["clean all but the build", true], ["cook textures", true], ["build ahead of the box", true], ["build ahead of the box", true], ["run", false]]);
  assert.deepEqual(s[4].cmd, ["nice", "-n", "10", ...sandbox("bench")]);
  assert.ok(s[4].build && !s[4].run);
  assert.ok(!steps(job()).some((x) => x.prep || x.build), "only a bench builds ahead");

  // Its build and its timed run are timed apart: the build is one kind for every bench.
  const done = ran(-100, "cargo xtask bench crypt", 12, { class: "bench", phase: "timed", prepared: iso(-98), timed: iso(-95) });
  const est = estimator([done, ran(-60, "cargo xtask bench town", 30, { class: "bench", phase: "timed", prepared: iso(-57), timed: iso(-50) })]);
  assert.equal(est(at(0, "cargo xtask bench crypt", { class: "bench" }), "long"), 7 * 60_000, "the timed run, from the box empty to its end");
  assert.equal(est(at(0, "cargo xtask bench walk", { class: "bench", phase: "prep" }), "short"), 2 * 60_000, "the shortest build of any bench");
  assert.equal(est(at(0, "cargo xtask bench walk", { class: "bench", phase: "prep" }), "long"), 3 * 60_000);

  // On a box empty but for the bench building, a job behind it fills a slot only if it is over
  // by the build's soonest end; a job ahead of it goes first when that is cheaper (D319).
  const building = at(1, "cargo xtask bench crypt", { class: "bench", state: "running", started: iso(1), slot: 1, phase: "prep" });
  const quick = ran(-30, "cargo test -p a", 1, { class: "cpu" });
  const behind = at(2, "cargo test -p a", { class: "cpu" });
  assert.deepEqual(ids(plan([done, quick, building, behind], s4, T0 + 1.5 * 60_000)), ["cargo test -p a"], "a minute fits in the two the build takes");
  assert.deepEqual(plan([done, quick, building, behind], s4, T0 + 2.5 * 60_000).start, [], "not once half a minute is left");
  const older = at(0.5, "cargo test -p a", { class: "cpu", id: "20261028-120030-0000" });
  assert.deepEqual(ids(plan([done, quick, older, building], s4, T0 + 1.5 * 60_000)), ["cargo test -p a"], "a minute's test ahead goes before a 7-minute bench");
  const film = ran(-200, "sh film.sh 1", 40, { class: "gpu" });
  const olderFilm = at(0.5, "sh film.sh 2", { class: "gpu", id: "20261028-120030-0000" });
  assert.deepEqual(plan([done, film, olderFilm, building], s4, T0 + 1.5 * 60_000).start, [], "a 40-minute film ahead waits for it");

  // --status says what the bench does in its slot.
  assert.match(benchHold([done, building, behind], new Date(T0 + 2 * 60_000)).join("\n"), /building 20261028-120100-0001 ahead in slot 1 for 1m/);
  const ready = { ...building, phase: "ready" as const, prepared: iso(2) };
  assert.match(benchHold([done, ready, behind], new Date(T0 + 3 * 60_000)).join("\n"), /20261028-120100-0001 built in slot 1 1m ago, its timed run next/);
});

test("replayed on random queues, a bench never runs beside another job and every job runs", () => {
  let seed = 7;
  const rand = () => ((seed = (seed * 1103515245 + 12345) % 2 ** 31) / 2 ** 31);
  const kinds: [string, Job["class"], number][] = [["cargo test -p a", "cpu", 1], ["cargo clippy", "cpu", 4], ["sh film.sh", "gpu", 40], ["sh still.sh", "gpu", 3], ["node probe.ts", "gpu", 12], ["cargo xtask check --base x", "check", 2], ["cargo xtask bench crypt", "bench", 6], ["cargo xtask bench town", "bench", 20]];
  for (let round = 0; round < 20; round++) {
    const jobs: Job[] = [];
    const end = new Map<string, number>();
    for (let n = 0; n < 60; n++) {
      const [command, c, took] = kinds[Math.floor(rand() * kinds.length)];
      const q = Math.floor(n * 3 * rand() + n);
      jobs.push(at(q, command, { class: c, id: `20261028-${String(100000 + n).padStart(6, "0")}-${String(n).padStart(4, "0")}`, queued: iso(q), minutes: 120 }));
      end.set(jobs[n].id, Math.ceil(took * (0.5 + rand())));
    }
    // A bench builds for 1 to 2 minutes in its slot, waits built, then times its run alone.
    for (let t = 0; t < 5000 && jobs.some((j) => j.state !== "done"); t++) {
      const now = T0 + t * 60_000;
      for (const j of jobs) {
        if (j.state !== "running") continue;
        if (j.phase === "prep" && Date.parse(j.started!) + (1 + (end.get(j.id)! % 2)) * 60_000 <= now) Object.assign(j, { phase: "ready", prepared: iso(t) });
        else if (Date.parse(j.timed ?? j.started!) + end.get(j.id)! * 60_000 <= now && j.phase !== "prep" && j.phase !== "ready") Object.assign(j, { state: "done", ended: iso(t) });
      }
      const visible = jobs.filter((j) => Date.parse(j.queued) <= now);
      const p = plan(visible, s4, now);
      if (p.go) {
        assert.equal(p.start.length, 0, "nothing starts with the bench let go");
        Object.assign(p.go, { phase: "timed", timed: iso(t) });
      }
      for (const { job: j, slot } of p.start) Object.assign(j, { state: "running", started: iso(t), slot, ...(j.class === "bench" ? { phase: "prep" } : {}) });
      const running = jobs.filter((j) => j.state === "running");
      assert.ok(!(running.length > 1 && running.some((j) => j.phase === "timed")), `round ${round}, minute ${t}: a bench was timed beside ${running.length - 1} jobs`);
      assert.ok(running.filter((j) => j.class === "bench").length <= 1, `round ${round}, minute ${t}: two benches held slots`);
    }
    assert.ok(jobs.every((j) => j.state === "done"), `round ${round}: every job ran`);
  }
});

test("--status says how long benches have held the queue, what runs or drains, and what waits behind", () => {
  const benchKind = ran(-100, "cargo xtask bench town", 20, { class: "bench" });
  const film = at(0, "sh film.sh 2", { class: "gpu", state: "running", started: iso(0), slot: 1 });
  const b1 = at(10, "cargo xtask bench town", { class: "bench" });
  const behind = [at(12, "sh film.sh 3", { class: "gpu" }), at(14, "cargo test -p a", { class: "cpu" })];
  const draining = benchHold([ran(-200, "sh film.sh 1", 30, { class: "gpu" }), benchKind, film, b1, ...behind], new Date(T0 + 20 * 60_000));
  assert.match(draining[0], /^bench hold: 10m, since 12:10 UTC$/);
  assert.match(draining[1], /draining: 1 job still running, the box empty in 10m at the soonest/);
  assert.match(draining[2], /benches next: 20261028-121000-0010 \(waiting 10m, its kind took up to 20m\)/);
  assert.match(draining[3], /waiting behind it: 2 jobs \(1 GPU, 1 CPU\), the oldest 20261028-121200-0012 for 8m/);
  // The first bench ran from minute 30; a second queued meanwhile keeps the hold going from minute 10.
  const done1 = { ...b1, state: "done" as const, started: iso(30), ended: iso(50) };
  const b2 = { ...at(40, "cargo xtask bench town", { class: "bench" }), state: "running" as const, started: iso(50), slot: 1 };
  const running = benchHold([benchKind, done1, b2, ...behind], new Date(T0 + 60 * 60_000));
  assert.match(running[0], /^bench hold: 50m, since 12:10 UTC$/);
  assert.match(running[1], /running 20261028-124000-0040 for 10m, its kind took up to 20m/);
  assert.deepEqual(benchHold([benchKind, film, ...behind], new Date(T0)), [], "no bench, no hold");
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
  assert.deepEqual(env, { PATH: "/bin", HOME: "/box/homes/2", CARGO_HOME: "/home/u/.cargo", RUSTUP_HOME: "/home/u/.rustup", CARGO_TARGET_DIR: "/box/slots/2/target", TERRA_RENDER_JOB: "j1", TERRA_BLENDER_GPU_BACKEND: "vulkan" });
  assert.equal(jobEnv({ HOME: "/h", TERRA_BLENDER_GPU_BACKEND: "opengl" }, "/b/h", "j", "/b/t").TERRA_BLENDER_GPU_BACKEND, "opengl", "the box's own choice stands");
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
  // A bench is never niced, and runs on the performance cores where the box has them.
  const bench = job({ command: ["cargo", "xtask", "bench", "crypt"] });
  assert.deepEqual(steps(bench, "0-15").at(-1)!.cmd, ["taskset", "-c", "0-15", "cargo", "xtask", "bench", "crypt"]);
  assert.deepEqual(steps(bench).at(-1)!.cmd, ["cargo", "xtask", "bench", "crypt"]);
  assert.deepEqual(steps(job(), "0-15").at(-1)!.cmd.slice(0, 3), ["nice", "-n", "10"]);
});

test("a hybrid processor's performance cores, from lscpu's top clocks", () => {
  // An i9-14900K: 8 P-cores with two threads each (two favoured at 6.0 GHz, six at 5.7), 16 E-cores at 4.4.
  const p = Array.from({ length: 16 }, (_, i) => `${i},${i < 4 ? "6000.0000" : "5700.0000"}`);
  const e = Array.from({ length: 16 }, (_, i) => `${16 + i},4400.0000`);
  const text = ["# The following is the parsable format", "# CPU,MAXMHZ", ...p, ...e].join("\n") + "\n";
  assert.equal(performanceCores(text), "0-15");
  // Interleaved numbering comes out as ranges.
  assert.equal(performanceCores("0,5000\n1,3000\n2,5000\n3,5000\n4,3000\n"), "0,2-3");
  // Every processor alike, or no clocks given: none.
  assert.equal(performanceCores("0,4000\n1,4000\n"), undefined);
  assert.equal(performanceCores("0,\n1,\n"), undefined);
});

test("a --dirty job says it runs a patch: its id, its line in lists and a step in its log, never the snapshot's sha", () => {
  const base = "fedcba9876543210fedcba9876543210fedcba98";
  const p = job({ id: jobId(new Date("2026-10-23T12:00:00Z"), "beef", true), patch: { base, stat: "1 file changed, 1 insertion(+)", files: 1 } });
  assert.equal(p.id, "20261023-120000-beef-patch");
  assert.ok(ID.test(p.id) && ID.test(job().id) && !ID.test("20261023-120000-beef-commit"));
  assert.match(short(p), /^fedcba987\+patch gpu: /);
  assert.ok(!short(p).includes(p.rev.slice(0, 9)), "the snapshot's own sha is never shown as what ran");
  assert.match(short(job()), /^012345678 gpu: /);
  const s = steps(p);
  assert.equal(s.findIndex((x) => x.what.startsWith("PATCH")), 2, "right after the checkout, before any build");
  assert.deepEqual(s[2].cmd, ["git", "--no-pager", "diff", "--stat", base, p.rev]);
  assert.ok(!steps(job()).some((x) => x.what.startsWith("PATCH")));
  const d = parseClient(["--dirty", "--", "x"]) as { dirty: boolean; rev?: string };
  assert.equal(d.dirty, true);
  assert.equal((parseClient(["--", "x"]) as { dirty: boolean }).dirty, false);
  assert.ok("error" in parseClient(["--dirty", "--rev", "abc", "--", "x"]), "the patch is against HEAD only");
});

test("a card the driver cannot reach gets the commands that fix it", () => {
  assert.match(gpuFix("6.8.0-146-generic", "535", false).join("\n"), /linux-modules-nvidia-535-generic[\s\S]*reboot/);
  assert.match(gpuFix("k", "535", true).join("\n"), /modprobe nvidia/);
  assert.match(gpuFix("k", undefined, false).join("\n"), /ubuntu-drivers/);
});

// ---- Stale builds ----

const WORKSPACE = ["terra_build", "terra_build_layout", "terra_sandbox", "terra_world", "xtask"];
const T = "/home/u/terra-render/slots/2/target";

test("a link error naming a workspace crate is a stale build: its crates and the failing package are cleaned, in the profile its paths show", () => {
  // rustc's link failure with lld's report, the symbol a newer terra_world has and the stale rlib lacks.
  const lld = [
    "   Compiling terra_sandbox v0.1.0 (/home/u/terra-render/slots/2/crates/terra_sandbox)",
    "error: linking with `cc` failed: exit status: 1",
    `  = note: LC_ALL="C" "cc" "${T}/release/deps/terra_sandbox-1a2b.terra_sandbox.3c4d-cgu.07.rcgu.o" "${T}/release/deps/libterra_build_layout-9f.rlib" "${T}/release/deps/libbevy_ecs-77.rlib"`,
    "  = note: rust-lld: error: undefined symbol: <terra_world::fire::FireGlows as bevy_ecs::component::Component>::register_required_components",
    "          >>> referenced by terra_sandbox.3c4d-cgu.07",
    'error: could not compile `terra_sandbox` (bin "terra_sandbox") due to 1 previous error',
  ].join("\n");
  const s = staleBuild(lld, WORKSPACE);
  assert.deepEqual(s?.packages, ["terra_sandbox", "terra_world"], "named in the symbol and failing, not every rlib on the command line");
  assert.deepEqual(s?.profiles, ["release"]);
  assert.match(s?.why ?? "", /undefined symbol: <terra_world::fire::FireGlows/);
  assert.deepEqual(cleanCommands(s!), [["cargo", "clean", "--release", "-p", "terra_sandbox", "-p", "terra_world"]]);

  // GNU ld with mangled names, in a dev build: the crate is found inside the mangling.
  const ld = [
    "error: linking with `cc` failed: exit status: 1",
    `  = note: /usr/bin/ld: ${T}/debug/deps/xtask-5e.o: in function \`main':`,
    "          undefined reference to `_ZN11terra_build4kits4load17h0123456789abcdefE'",
    'error: could not compile `xtask` (bin "xtask")',
  ].join("\n");
  assert.deepEqual(staleBuild(ld, WORKSPACE), { why: "undefined reference to `_ZN11terra_build4kits4load17h0123456789abcdefE'", packages: ["terra_build", "xtask"], profiles: ["dev"] });

  // rustc finding a crate's metadata from an older build; no path says the profile, so both.
  const meta = "error[E0460]: found possibly newer version of crate `terra_world` which `terra_sandbox` depends on\nerror: could not compile `terra_sandbox`";
  const m = staleBuild(meta, WORKSPACE);
  assert.deepEqual(m?.packages, ["terra_sandbox", "terra_world"]);
  assert.deepEqual(cleanCommands(m!), [
    ["cargo", "clean", "--release", "-p", "terra_sandbox", "-p", "terra_world"],
    ["cargo", "clean", "-p", "terra_sandbox", "-p", "terra_world"],
  ]);
});

test("a registry crate's rlib wanting a symbol it defines itself is corrupt, and cleaned by its package name", () => {
  // Worker slot 4 on 2026-10-27: one code unit of bevy_reflect's rlib wants an instantiation
  // another of its own units lacks; cleaning terra_sandbox alone links the same rlib again.
  const R = `${T}/release/deps`;
  const corrupt = [
    "error: linking with `cc` failed: exit status: 1",
    `  = note: "cc" "-m64" "${R}/rustcPIhiF9/symbols.o" "${R}/{libterra_game-d674d78cd12cdca2,libbevy_reflect-f3789ba75e255a89}.rlib"`,
    "  = note: rust-lld: error: undefined hidden symbol: <erased_serde::de::erase::Visitor<<f32 as serde_core::de::Deserialize>::deserialize::PrimitiveVisitor> as erased_serde::de::Visitor>::erased_visit_u64",
    "          >>> referenced by bevy_reflect.9e1bd75dc8004647-cgu.01",
    `          >>>               bevy_reflect-f3789ba75e255a89.bevy_reflect.9e1bd75dc8004647-cgu.01.rcgu.o:(anon.5f9e.38.llvm.3444) in archive ${R}/libbevy_reflect-f3789ba75e255a89.rlib`,
    "          >>> did you mean: <erased_serde::de::erase::Visitor<<f32 as serde_core::de::Deserialize>::deserialize::PrimitiveVisitor> as erased_serde::de::Visitor>::erased_visit_f64",
    `          >>> defined in: ${R}/libbevy_reflect-f3789ba75e255a89.rlib(bevy_reflect-f3789ba75e255a89.bevy_reflect.9e1bd75dc8004647-cgu.00.rcgu.o)`,
    "          rust-lld: error: undefined hidden symbol: <erased_serde::de::erase::Visitor<serde_core::de::impls::BoolVisitor> as erased_serde::de::Visitor>::erased_visit_u64",
    "          >>> referenced by bevy_reflect.9e1bd75dc8004647-cgu.01",
    `          >>>               bevy_reflect-f3789ba75e255a89.bevy_reflect.9e1bd75dc8004647-cgu.01.rcgu.o:(anon.5f9e.65.llvm.3444) in archive ${R}/libbevy_reflect-f3789ba75e255a89.rlib`,
    "          collect2: error: ld returned 1 exit status",
    'error: could not compile `terra_sandbox` (bin "terra_sandbox") due to 1 previous error',
  ].join("\n");
  const s = staleBuild(corrupt, WORKSPACE, ["bevy_reflect", "erased-serde", "serde_core"]);
  assert.deepEqual(s?.packages, ["bevy_reflect", "terra_sandbox"], "the corrupt rlib's crate, not erased_serde the symbol names");
  assert.deepEqual(cleanCommands(s!), [["cargo", "clean", "--release", "-p", "bevy_reflect", "-p", "terra_sandbox"]]);

  // The rlib is named by its crate; the clean names Cargo.lock's package.
  const hyphen = corrupt.replace(/bevy_reflect/g, "erased_serde");
  assert.deepEqual(staleBuild(hyphen, WORKSPACE, ["erased-serde"])?.packages, ["erased-serde", "terra_sandbox"]);

  // A symbol one rlib wants and another defines is the code's or a version's, not corruption.
  const other = corrupt.replace(`defined in: ${R}/libbevy_reflect-f3789ba75e255a89.rlib`, `defined in: ${R}/liberased_serde-0123456789abcdef.rlib`);
  assert.deepEqual(staleBuild(other, WORKSPACE, ["bevy_reflect", "erased-serde"])?.packages, ["terra_sandbox"]);
});

test("an error in the code, or one naming nothing of the workspace, is not a stale build", () => {
  const typo = "error[E0308]: mismatched types\n --> crates/terra_world/src/fire.rs:10:5\nerror: could not compile `terra_world` (lib) due to 1 previous error";
  assert.equal(staleBuild(typo, WORKSPACE), undefined, "a real error is not run twice");
  assert.equal(staleBuild("thread 'main' panicked at crates/terra_world/src/cine.rs:4:1", WORKSPACE), undefined);
  assert.equal(staleBuild("rust-lld: error: undefined symbol: vkCreateInstance\nerror: could not compile `ash`", WORKSPACE), undefined, "a registry crate is never cleaned");
  assert.equal(staleBuild("rust-lld: error: undefined symbol: terra_buildings::x\n", WORKSPACE), undefined, "a longer name is not its prefix");
});

// Slot 3's crash on 2026-10-28 (job 20261028-235127-6698), cut to its telling lines.
const ICE_LOG = (target: string) => `   9:     0x71f3e7267f2d - __rustc[6ef78fba13ac776c]::rust_begin_unwind
  11:     0x71f3e3e39a99 - core[37f591cfbe66b0b1]::panicking::panic_bounds_check
  12:     0x71f3e91105d1 - <rustc_middle[8a940fbd8c5d50e6]::dep_graph::serialized::SerializedDepGraph>::decode
  13:     0x71f3e8c430ca - rustc_incremental[42cfc642e867122b]::persist::load::setup_dep_graph

error: the compiler unexpectedly panicked. This is a bug

note: compiler flags: --crate-type lib -C opt-level=3 -C embed-bitcode=no -C incremental=[REDACTED] -C strip=debuginfo
error: could not compile \`terra_folk\` (lib)

Caused by:
  process didn't exit successfully: \`rustc --crate-name terra_folk --edition=2024 crates/terra_folk/src/lib.rs -C incremental=${target}/release/incremental -C strip=debuginfo\` (exit status: 101)`;

test("rustc crashing in its incremental cache names the crate and the folder its rustc used", () => {
  const ice = rustcIce(ICE_LOG("/box/slots/3/target"), "/box/slots/3/target");
  assert.deepEqual(ice, {
    why: "rustc panicked in <rustc_middle::dep_graph::serialized::SerializedDepGraph>::decode",
    cache: true,
    crates: ["terra_folk"],
    dirs: ["/box/slots/3/target/release/incremental"],
  });
  const list = () => ["terra_folk-1br3qhgka38wn", "terra_folk-1j6p413myu8dy", "terra_folk_x-02a72n5hq7z0c", "terra_world-0hom6yfkb85ag"];
  assert.deepEqual(incrementalClears(ice!, list), ["/box/slots/3/target/release/incremental/terra_folk-1br3qhgka38wn", "/box/slots/3/target/release/incremental/terra_folk-1j6p413myu8dy"], "the crate's sessions, not a longer name's");
  // A folder outside the slot's target is never named; with none left, release's and dev's.
  const elsewhere = rustcIce(ICE_LOG("/other/target"), "/box/slots/3/target");
  assert.deepEqual(elsewhere?.dirs, ["/box/slots/3/target/debug/incremental", "/box/slots/3/target/release/incremental"]);
  // No crate named: the folders whole.
  const unnamed = rustcIce(ICE_LOG("/t").replace(/error: could not compile.*/, ""), "/t");
  assert.deepEqual(incrementalClears(unnamed!, list), ["/t/release/incremental"]);
});

// Slot 1's crash on 2026-10-29 (job 20261029-023426-a1cb-patch, a script's cargo build, so no
// rustc command line), cut to its telling lines; CARGO_INCREMENTAL=0 built it.
const QUERY_ICE_LOG = `thread 'rustc' panicked at compiler/rustc_middle/src/query/plumbing.rs:331:9:
  18:     0x772aa7d7ac30 - rustc_query_impl[bdb42d22f02e134a]::query_impl::needs_drop_raw::execute_query_incr::__rust_end_short_backtrace
  19:     0x772aa896b825 - rustc_codegen_ssa[20a34f9232644acf]::mir::codegen_mir::<rustc_codegen_llvm[49149b7e7b12cc7]::builder::GenericBuilder>

error: the compiler unexpectedly panicked. This is a bug

note: rustc 1.97.1 (8bab26f4f 2026-07-14) running on x86_64-unknown-linux-gnu

note: compiler flags: --crate-type lib -C opt-level=3 -C embed-bitcode=no -C incremental=[REDACTED] -C strip=debuginfo

query stack during panic:
#0 [is_copy_raw] computing whether \`bevy_ptr::MovingPtr<'_, core::mem::maybe_uninit::MaybeUninit<pooling::Mote>>\` is \`Copy\`
#1 [needs_drop_raw] computing whether \`bevy_ptr::MovingPtr<'_, core::mem::maybe_uninit::MaybeUninit<pooling::Mote>>\` needs drop
end of query stack
error: could not compile \`terra_folk\` (lib) due to 1 previous error`;

test("any ICE is rustc's crash, named by its query, by where it panicked, or plainly", () => {
  assert.deepEqual(rustcIce(QUERY_ICE_LOG, "/box/slots/1/target"), {
    why: "rustc panicked in its is_copy_raw query, computing whether `bevy_ptr::MovingPtr<'_, core::mem::maybe_uninit::MaybeUninit<pooling::Mote>>` is `Copy`",
    cache: false,
    crates: ["terra_folk"],
    dirs: ["/box/slots/1/target/debug/incremental", "/box/slots/1/target/release/incremental"],
  });
  const typeck = "thread 'rustc' panicked at compiler/rustc_hir_typeck/src/lib.rs:12:5:\n  12:  0x1 - rustc_hir_typeck[ab]::check\nerror: the compiler unexpectedly panicked. This is a bug\nerror: could not compile `terra_world`";
  assert.deepEqual(rustcIce(typeck, "/t"), { why: "rustc panicked at compiler/rustc_hir_typeck/src/lib.rs:12:5", cache: false, crates: ["terra_world"], dirs: ["/t/debug/incremental", "/t/release/incremental"] });
  assert.equal(rustcIce("error: internal compiler error: no type for node\nerror: could not compile `terra_world`", "/t")?.why, "rustc panicked");
});

test("a failure that is no ICE is not rustc's crash", () => {
  assert.equal(rustcIce("error[E0308]: mismatched types\nerror: could not compile `terra_world`", "/t"), undefined);
  assert.equal(rustcIce("note: rustc_incremental was mentioned\n#0 [is_copy_raw] computing\nerror: could not compile `terra_world`", "/t"), undefined, "no ICE, no retry");
  assert.equal(rustcIce("thread 'main' panicked at crates/terra_folk/src/lib.rs:3:1:\nerror: test failed", "/t"), undefined, "the game panicking is no ICE");
});

test("the workspace's packages come from its members' manifests", () => {
  const files: Record<string, string> = {
    "crates/terra_world/Cargo.toml": '[package]\nname = "terra_world"\nversion.workspace = true\n\n[dependencies]\nname = "not this"\n',
    "crates/terra_gen/Cargo.toml": '[package]\nversion = "0.1.0"\nname = "terra_gen"\n',
    "xtask/Cargo.toml": '[package]\nname = "xtask"\n',
  };
  const root = '[workspace]\nresolver = "3"\nmembers = ["crates/*", "xtask"]\nexclude = ["spike"]\n';
  const list = (d: string) => (d === "crates" ? ["terra_world", "terra_gen", "README.md"] : []);
  assert.deepEqual(workspacePackages(root, (p) => files[p], list), ["terra_gen", "terra_world", "xtask"]);
  assert.deepEqual(workspacePackages("", (p) => files[p], list), []);
});

test("a checkout inside the slot's build is found, one beside it or the slot itself is not", () => {
  const list = [
    ...["worktree /box/repo", "HEAD 3740b8755", "detached", ""],
    ...["worktree /box/slots/3", "HEAD 7dc14197", "detached", ""],
    ...["worktree /box/slots/3/target/gate/base", "HEAD 799be03c", "detached", ""],
    ...["worktree /box/slots/3/targets/x", "", "worktree /box/slots/4/target/gate/base", ""],
  ].join("\n");
  assert.deepEqual(checkoutsInBuild(list, "/box/slots/3/target"), ["/box/slots/3/target/gate/base"]);
  assert.deepEqual(checkoutsInBuild(list, "/box/slots/3/target/"), ["/box/slots/3/target/gate/base"]);
  assert.deepEqual(checkoutsInBuild(list, "/box/slots/1/target"), []);
});
