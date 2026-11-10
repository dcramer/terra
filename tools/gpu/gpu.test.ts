// The wrapper on real processes (D319): shared runs start at once, a bench waits for them and
// holds back new ones while it waits, and a job that makes no progress is stopped at its limit.
// Each test has its own state directory and lock, and a stand-in nvidia-smi that fails, so the
// live runs, their lock and the card are never touched.
import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";

const GPU = path.join(import.meta.dirname, "gpu.ts");

/// A private machine: state, lock and a failing nvidia-smi in a temporary folder.
function machine() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "terra-gpu-test-"));
  const bin = path.join(dir, "bin");
  fs.mkdirSync(bin);
  fs.writeFileSync(path.join(bin, "nvidia-smi"), "#!/bin/sh\nexit 1\n", { mode: 0o755 });
  const env: NodeJS.ProcessEnv = {
    ...process.env,
    PATH: `${bin}:${process.env.PATH}`,
    TERRA_GPU_DIR: path.join(dir, "gpu"),
    TERRA_GPU_LOCK: path.join(dir, "gpu.lock"),
    // The game's one-copy slots (terra_world::single_instance), which a game run waits for.
    TERRA_INSTANCE_DIR: path.join(dir, "instance"),
  };
  delete env.TERRA_GPU_JOB;
  const log = path.join(dir, "order.log");
  const run = (args: string[]): Promise<{ code: number | null; err: string; seconds: number }> => {
    const started = Date.now();
    const child = spawn(process.execPath, [GPU, ...args], { env, stdio: ["ignore", "pipe", "pipe"] });
    let err = "";
    child.stderr.on("data", (chunk) => (err += chunk));
    return new Promise((done) => child.on("close", (code) => done({ code, err, seconds: (Date.now() - started) / 1000 })));
  };
  /// A command that logs its name and the time when it starts and ends, sleeping between (the
  /// time from Node, as macOS's `date` has no %N).
  const now = `$("${process.execPath}" -p 'Date.now()')`;
  const step = (name: string, seconds: number) => ["sh", "-c", `echo "${name} start ${now}" >> ${log}; sleep ${seconds}; echo "${name} end ${now}" >> ${log}`];
  const lines = () =>
    fs.readFileSync(log, "utf8").trim().split("\n").map((line) => {
      const [name, at, ms] = line.split(" ");
      return { name, at, ms: Number(ms) };
    });
  return { dir, env, run, step, lines, done: () => fs.rmSync(dir, { recursive: true, force: true }) };
}

const pause = (ms: number) => new Promise((done) => setTimeout(done, ms));

test("two stills that start no game start at once, side by side", async () => {
  const m = machine();
  const launched = Date.now();
  const [a, b] = await Promise.all([m.run(["still", "--no-game", "--", ...m.step("a", 2)]), m.run(["still", "--no-game", "--", ...m.step("b", 2)])]);
  assert.equal(a.code, 0, a.err);
  assert.equal(b.code, 0, b.err);
  const starts = m.lines().filter((l) => l.at === "start");
  assert.equal(starts.length, 2);
  for (const s of starts) assert.ok(s.ms - launched < 1500, `${s.name} started ${s.ms - launched} ms after launch`);
  const ends = m.lines().filter((l) => l.at === "end");
  assert.ok(Math.max(...starts.map((s) => s.ms)) < Math.min(...ends.map((e) => e.ms)), "both ran at the same time");
  m.done();
});

// Without flock (macOS) the wrapper takes no lock and the game takes its own turn (gpu_turn).
const noFlock = spawnSync("flock", ["--version"]).status !== 0 && "no flock here: the wrapper runs its command without the lock";

test("a bench waits for the running still, and a still asked for meanwhile waits for the bench", { skip: noFlock }, async () => {
  const m = machine();
  // The bench's stall limit (3 s) is shorter than its wait: waiting for the lock is not a stall.
  const first = m.run(["still", "--", ...m.step("first", 4)]);
  await pause(500);
  const bench = m.run(["bench", "--stall", "0.05", "--", ...m.step("bench", 1)]);
  await pause(1500);
  const later = m.run(["still", "--", ...m.step("later", 1)]);
  const [f, b, l] = await Promise.all([first, bench, later]);
  for (const r of [f, b, l]) assert.equal(r.code, 0, r.err);
  assert.deepEqual(
    m.lines().map((x) => `${x.name} ${x.at}`),
    ["first start", "first end", "bench start", "bench end", "later start", "later end"],
  );
  assert.match(l.err, /waits for a benchmark/);
  m.done();
});

// A run's background process that outlives it keeps the lock's file open, so a bench waits on it:
// as a cancelled render job's setsid clippy loop held slot 4's lock for 2 h 46 min on 2026-10-29.
const noProc = (noFlock || (!fs.existsSync("/proc/self/fdinfo") && "no /proc here")) as string | false;

test("a holder left by an ended run is named, with its folder and what to do, by the status and the bench waiting on it", { skip: noProc }, async () => {
  const m = machine();
  m.env.TERRA_GPU_OUTSIDE_S = "1";
  const pidFile = path.join(m.dir, "stray.pid");
  const left = await m.run(["still", "--", "sh", "-c", `setsid sh -c 'echo $$ > ${pidFile}; exec sleep 30' >/dev/null 2>&1 & while [ ! -s ${pidFile} ]; do sleep 0.05; done`]);
  assert.equal(left.code, 0, left.err);
  const stray = Number(fs.readFileSync(pidFile, "utf8"));
  try {
    const bench = m.run(["bench", "--", ...m.step("bench", 0)]);
    await pause(2500);
    const status = spawnSync(process.execPath, [GPU, "--status"], { env: m.env, encoding: "utf8" });
    assert.match(status.stdout, new RegExp(`✗ outside: .*gpu\\.lock held outside any run for \\d+s by pid ${stray} \`sleep 30\` in ${process.cwd()}, started by the \`cargo xtask gpu\` run of wrapper pid \\d+, which has ended`));
    assert.match(status.stdout, new RegExp(`stop it: kill -- -${stray} \\(it and what it started\\)`));
    process.kill(stray, "SIGTERM");
    const b = await bench;
    assert.equal(b.code, 0, b.err);
    assert.match(b.err, new RegExp(`gpu: ✗ bench waits \\d+s: .*by pid ${stray} \`sleep 30\``), "the bench's log names the holder");
  } finally {
    try {
      process.kill(stray, "SIGKILL");
    } catch {}
    m.done();
  }
});

test("a silent job that burns CPU runs to its end; a silent sleep is stopped at the limit", async () => {
  // A 3 s limit: the busy loop is silent for twice that, the sleep would last far longer.
  const m = machine();
  const busy = m.run(["still", "--no-game", "--stall", "0.05", "--", process.execPath, "-e", "const end = Date.now() + 6000; while (Date.now() < end);"]);
  const idle = m.run(["still", "--no-game", "--stall", "0.05", "--", "sleep", "60"]);
  const [b, i] = await Promise.all([busy, idle]);
  assert.equal(b.code, 0, b.err);
  assert.ok(b.seconds >= 6, `the busy job ran ${b.seconds} s`);
  assert.equal(i.code, 124, i.err);
  assert.match(i.err, /no progress for 0.05 min/);
  assert.ok(i.seconds < 20, `the sleep was stopped after ${i.seconds} s`);
  m.done();
});

// One game copy a machine (D319): the stand-ins below are commands the wrapper takes for the game
// (any command but a Blender run, a --multirun copy or one under --no-game); none starts it.

test("two game runs asked for together both finish, the second after the first, and a run that starts no game does not wait", { skip: noFlock }, async () => {
  const m = machine();
  m.env.TERRA_GPU_SAY_S = "1";
  const launched = Date.now();
  const first = m.run(["still", "--", ...m.step("first", 3)]);
  await pause(300);
  const second = m.run(["film", "--", ...m.step("second", 1)]);
  const other = m.run(["still", "--no-game", "--", ...m.step("other", 1)]);
  await pause(1500);
  const status = spawnSync(process.execPath, [GPU, "--status"], { env: m.env, encoding: "utf8" });
  const [f, s, o] = await Promise.all([first, second, other]);
  for (const r of [f, s, o]) assert.equal(r.code, 0, r.err);
  const at = (name: string, when: string) => m.lines().find((l) => l.name === name && l.at === when)!.ms;
  assert.ok(at("second", "start") >= at("first", "end"), "the second game run started after the first ended");
  assert.ok(at("other", "start") - launched < 1800, `the run that starts no game started ${at("other", "start") - launched} ms after launch`);
  assert.ok(at("other", "end") < at("first", "end"), "and ran beside the first game run");
  assert.match(s.err, /gpu: film waits for the game's one copy a machine \(D319\), held by the still of wrapper pid \d+ in .*running \d+s/);
  assert.ok((s.err.match(/waits for the game's one copy/g) ?? []).length >= 2, "it says so again while it waits, so its silence is not taken for a hang");
  assert.doesNotMatch(o.err, /waits/);
  assert.match(status.stdout, /1 waiting/);
  assert.match(status.stdout, /waiting +\d+s film .*for the game's one copy, held by the still of wrapper pid \d+/);
  m.done();
});

test("a game run waits for a copy started outside the wrapper, named from its record, and for no dead copy's record", { skip: noFlock }, async () => {
  const m = machine();
  const folder = m.env.TERRA_INSTANCE_DIR!;
  fs.mkdirSync(folder);
  // A copy that died left its record, unchanged; one that runs rewrites its own every beat.
  fs.writeFileSync(path.join(folder, "instance-1.txt"), "pid=7\nos=linux\nworktree=/dead\ncommand=terra_sandbox\nsince=1\nbeat=9\n");
  const record = path.join(folder, "instance-0.txt");
  const copy = spawn(process.execPath, ["-e", `
    const fs = require("node:fs");
    let beat = 0;
    const write = () => fs.writeFileSync(${JSON.stringify(record)}, "pid=" + process.pid + "\\nos=windows\\nworktree=C:/played\\ncommand=terra_sandbox --experiment play\\nsince=1\\nbeat=" + beat++ + "\\n");
    write();
    const beating = setInterval(write, 250);
    setTimeout(() => { clearInterval(beating); fs.rmSync(${JSON.stringify(record)}); }, 4000);
  `]);
  const closed = new Promise<number>((done) => copy.on("close", () => done(Date.now())));
  await pause(500);
  const run = await m.run(["still", "--", ...m.step("run", 0)]);
  assert.equal(run.code, 0, run.err);
  assert.ok(m.lines()[0].ms >= (await closed) - 300, "the run started once the copy had closed");
  assert.match(run.err, new RegExp(`held by the game copy pid ${copy.pid} \\(windows\\) of C:/played, started outside this queue \\(\`terra_sandbox --experiment play\`\\)`));
  assert.ok(run.seconds < 12, `the dead copy's record held it ${run.seconds} s`);
  m.done();
});

test("a game run the game refused all the same is started again", { skip: noFlock }, async () => {
  const m = machine();
  const once = path.join(m.dir, "refused-once");
  const game = `if [ ! -e ${once} ]; then touch ${once}; echo 'SANDBOX ✗ refused to start: another copy runs' >&2; exit 75; fi; echo ran > ${path.join(m.dir, "ran")}`;
  const run = await m.run(["still", "--", "sh", "-c", game]);
  assert.equal(run.code, 0, run.err);
  assert.match(run.err, /the game refused to start, .* then starting again/);
  assert.ok(fs.existsSync(path.join(m.dir, "ran")));
  const never = await m.run(["still", "--", "sh", "-c", "echo 'SANDBOX ✗ refused to start' >&2; exit 75"]);
  assert.equal(never.code, 75, "a start refused every time fails as it did");
  assert.match(never.err, /✗ still exit 75 .*one copy a machine/);
  m.done();
});
