import assert from "node:assert/strict";
import { test } from "node:test";
import { boxSettings, chooseWorker, cleanCommands, collected, config, displayPlan, expired, freeDisplay, gpuFix, ID, inWindow, type Job, jobClass, jobEnv, jobId, mayStart, needsDisplay, parseClient, parseEnvFile, parseWindow, performanceCores, plan, short, staleBuild, steps, takenDisplays, WARM, workspacePackages, xvfbArgs, xvfbBackoff } from "./jobs.ts";

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
  // A warm build queued after it (an update) waits behind it too; one queued before goes first.
  const warmAfter = at(4, { kind: "warm", slot: 2, class: "cpu" });
  assert.deepEqual(plan([at(1, { ...cpu, state: "running", slot: 3 }), bench, warmAfter], s), { start: [], why: "a bench waits for the box to empty" });
  assert.deepEqual(slotsOf(plan([bench, warmAfter], s)), [["02", 1]]);
  assert.deepEqual(slotsOf(plan([at(1, { kind: "warm", slot: 2, class: "cpu" }), bench], s)), [["01", 2]]);
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

test("an error in the code, or one naming nothing of the workspace, is not a stale build", () => {
  const typo = "error[E0308]: mismatched types\n --> crates/terra_world/src/fire.rs:10:5\nerror: could not compile `terra_world` (lib) due to 1 previous error";
  assert.equal(staleBuild(typo, WORKSPACE), undefined, "a real error is not run twice");
  assert.equal(staleBuild("thread 'main' panicked at crates/terra_world/src/cine.rs:4:1", WORKSPACE), undefined);
  assert.equal(staleBuild("rust-lld: error: undefined symbol: vkCreateInstance\nerror: could not compile `ash`", WORKSPACE), undefined, "a registry crate is never cleaned");
  assert.equal(staleBuild("rust-lld: error: undefined symbol: terra_buildings::x\n", WORKSPACE), undefined, "a longer name is not its prefix");
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
