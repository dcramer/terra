#!/usr/bin/env node
// The continuous checks on main (D37, D398): the heavy checks no hook runs (GPU renders, the
// fire sheet, look checks, benches, long sweeps), run on this GPU machine whenever origin/main
// moves, each in turn, in a worktree of its own detached at main's tip,
// `.git/terra-continuous/src`. Each check is a row of `checks.json` (data, D136): its name,
// the command, what it reads (a list of paths in `xtask/src/scope.rs`, so it runs only when one
// of them changed; none, every time main moves) and, optionally, `hours` to run it again on a
// timer although nothing changed. A check passes when its command exits 0.
//
//   cargo xtask continuous                # once: every check due at origin/main's tip
//   cargo xtask continuous --watch [min]  # the same every so many minutes (default 5)
//   cargo xtask continuous --install [min] # a systemd user timer running it every so many minutes
//
// Prints a line a check that ran; its output goes to `.git/terra-continuous/logs/<name>.log`.
// `.git/terra-continuous/status.txt` is a line a check, its state at the last commit it ran,
// with the range of commits a regression came in and those of them touching what it reads;
// `history.log` keeps every verdict. Exits 1 when a check fails. Node 22.18 or later, git and
// cargo; no dependencies.

import { spawnSync } from "node:child_process";
import { appendFileSync, closeSync, existsSync, mkdirSync, openSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join, resolve } from "node:path";

const HERE = import.meta.dirname;
const ROOT = resolve(HERE, "../..");
/// The most commits of a regression's range the status names.
const NAMED = 12;

export type Check = { name: string; does: string; run: string[]; reads?: string; hours?: number; minutes?: number };
/// What came of a check's last run: the commit and when, whether it passed, the last commit it
/// passed at, and the line it ended on.
/// `when` is an ISO time in UTC.
export type Record = { at: string; when: string; passed: boolean; good?: string; said: string; line?: string };

/// The paths of a `pub(crate) const NAME: &[&str] = &[...]` in scope.rs, read from their home
/// rather than copied.
export function scopePaths(source: string, name: string): string[] {
  const found = new RegExp(`const ${name}: &\\[&str\\] = &\\[([^\\]]*)\\]`).exec(source);
  return found ? [...found[1].matchAll(/"([^"]+)"/g)].map((m) => m[1]) : [];
}

/// Why a check runs at `tip`, or nothing when it need not: never run, main moved and touched
/// what it reads (`touched`, always true for a check that names nothing), or its timer is up.
export function due(check: Check, last: Record | undefined, tip: string, touched: boolean, now: number): string | undefined {
  if (!last) return "first run";
  if (check.hours && now - Date.parse(last.when) >= check.hours * 3600_000) return `${check.hours} h since the last`;
  if (last.at === tip) return undefined;
  return touched ? "main moved" : undefined;
}

/// The record a run leaves and the status line it says. A failure after a pass names the range
/// between the last passing commit and this one, `range` the commits in it touching what the
/// check reads (newest first, as git logs them).
export function verdict(
  check: Check,
  last: Record | undefined,
  tip: string,
  when: string,
  passed: boolean,
  said: string,
  range: string[],
): { record: Record; line: string } {
  const good = passed ? tip : last?.good;
  const record: Record = { at: tip, when, passed, good, said };
  const at = `${when.slice(0, 16).replace("T", " ")} UTC at ${tip.slice(0, 10)}`;
  let line: string;
  if (passed) line = `✓ ${check.name} ${at}${last && !last.passed ? " (passes again)" : ""}: ${said}`;
  else if (!good) line = `✗ ${check.name} ${at}: fails, no passing run to compare: ${said}`;
  else {
    const named = range.length ? `, ${range.length} commit${range.length === 1 ? "" : "s"}${check.reads ? " touching what it reads" : ""}${range.length > NAMED ? `, the newest ${NAMED}` : ""}:\n${range.slice(0, NAMED).map((c) => `    ${c}`).join("\n")}` : "";
    line = `✗ ${check.name} ${at}: ${last?.passed ? "regressed" : "still fails"} in ${good.slice(0, 10)}..${tip.slice(0, 10)}: ${said}${named}`;
  }
  return { record: { ...record, line }, line };
}

function git(args: string[], cwd = ROOT): { ok: boolean; out: string } {
  const r = spawnSync("git", args, { cwd, encoding: "utf8" });
  return { ok: r.status === 0, out: (r.stdout ?? "").trim() };
}

/// The main checkout: the folder the git folder is in, wherever this runs from.
function mainCheckout(): string {
  return dirname(git(["rev-parse", "--path-format=absolute", "--git-common-dir"]).out);
}

/// Holds `<state>/lock` (a folder with the holder's pid) or says who does.
function lock(state: string): (() => void) | undefined {
  const dir = join(state, "lock");
  try {
    mkdirSync(dir);
  } catch {
    const pid = Number(readFileSync(join(dir, "pid"), "utf8").trim() || 0);
    let alive = false;
    try {
      process.kill(pid, 0);
      alive = pid > 0;
    } catch {}
    if (alive) {
      console.log(`continuous: already running (pid ${pid})`);
      return undefined;
    }
    rmSync(dir, { recursive: true, force: true });
    mkdirSync(dir);
  }
  writeFileSync(join(dir, "pid"), `${process.pid}\n`);
  return () => rmSync(dir, { recursive: true, force: true });
}

/// The worktree detached at `tip`, made and seeded (D183) the first time, cleared of what the
/// last run left but its build.
function prepare(src: string, tip: string, log: string): string | undefined {
  if (!existsSync(join(src, ".git"))) {
    git(["worktree", "prune"]);
    if (!git(["worktree", "add", "-q", "--detach", src, tip]).ok) return `cannot make its worktree at ${src}`;
  }
  for (const args of [["reset", "-q", "--hard"], ["clean", "-fdq"], ["checkout", "-q", "--detach", tip]]) {
    if (!git(args, src).ok) return `git ${args.join(" ")} failed in ${src}`;
  }
  if (!existsSync(join(src, "target"))) {
    const fd = openSync(log, "a");
    spawnSync("cargo", ["xtask", "seed"], { cwd: src, stdio: ["ignore", fd, fd] });
    closeSync(fd);
  }
  return undefined;
}

/// The last line a command said that is not empty, without its colours.
function lastLine(text: string): string {
  const lines = text.replace(/\x1b\[[0-9;]*m/g, "").split("\n").map((l) => l.trim()).filter(Boolean);
  return lines.at(-1) ?? "said nothing";
}

function once(): number {
  const home = mainCheckout();
  const state = join(home, ".git", "terra-continuous");
  mkdirSync(join(state, "logs"), { recursive: true });
  const unlock = lock(state);
  if (!unlock) return 0;
  try {
    if (!git(["fetch", "-q", "origin", "main"]).ok) {
      console.log("✗ continuous: cannot fetch origin/main");
      return 1;
    }
    const tip = git(["rev-parse", "origin/main"]).out;
    const checks: Check[] = JSON.parse(readFileSync(join(HERE, "checks.json"), "utf8"));
    const recordsFile = join(state, "records.json");
    const records: { [name: string]: Record } = existsSync(recordsFile) ? JSON.parse(readFileSync(recordsFile, "utf8")) : {};
    const scope = readFileSync(join(ROOT, "xtask/src/scope.rs"), "utf8");
    const src = join(state, "src");
    let failed = false;
    for (const check of checks) {
      const paths = check.reads ? scopePaths(scope, check.reads) : [];
      if (check.reads && !paths.length) {
        console.log(`✗ continuous: ${check.name} reads ${check.reads}, which xtask/src/scope.rs does not list`);
        failed = true;
        continue;
      }
      const last = records[check.name];
      const touched = !last || !paths.length || !git(["diff", "--quiet", last.at, tip, "--", ...paths]).ok;
      const why = due(check, last, tip, touched, Date.now());
      if (!why) {
        if (last && last.at !== tip) records[check.name] = { ...last, at: tip };
        failed ||= last ? !last.passed : false;
        continue;
      }
      const log = join(state, "logs", `${check.name}.log`);
      writeFileSync(log, `${check.run.join(" ")} at ${tip} (${why})\n`);
      const broken = prepare(src, tip, log);
      const started = Date.now();
      let passed = false;
      let said = broken ?? "";
      if (!broken) {
        const fd = openSync(log, "a");
        const r = spawnSync("nice", ["-n", "10", ...check.run], {
          cwd: src,
          stdio: ["ignore", fd, fd],
          timeout: (check.minutes ?? 60) * 60_000,
        });
        closeSync(fd);
        passed = r.status === 0;
        said = r.error || r.signal ? `stopped after ${check.minutes ?? 60} min` : lastLine(readFileSync(log, "utf8"));
      }
      const good = last?.good;
      const range = !passed && good ? git(["log", "--format=%h %s", `${good}..${tip}`, "--", ...paths]).out.split("\n").filter(Boolean) : [];
      const when = new Date().toISOString();
      const { record, line } = verdict(check, last, tip, when, passed, said.slice(0, 300), range);
      records[check.name] = record;
      failed ||= !passed;
      console.log(`${line.split("\n")[0]} (${Math.round((Date.now() - started) / 60000)} min, ${why}; log ${log})`);
      appendFileSync(join(state, "history.log"), `${line}\n`);
      writeFileSync(recordsFile, `${JSON.stringify(records, null, 1)}\n`);
    }
    writeFileSync(recordsFile, `${JSON.stringify(records, null, 1)}\n`);
    const status = checks.map((check) => records[check.name]?.line ?? `· ${check.name}: not run yet`);
    writeFileSync(join(state, "status.txt"), `origin/main ${tip.slice(0, 10)}, looked ${new Date().toISOString().slice(0, 16).replace("T", " ")}\n${status.join("\n")}\n`);
    return failed ? 1 : 0;
  } finally {
    unlock();
  }
}

function sleep(ms: number): void {
  Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, ms);
}

/// A systemd user timer running `cargo xtask continuous` from the main checkout every so many
/// minutes; a run while one is going does nothing.
function install(every: number): number {
  const units = join(homedir(), ".config/systemd/user");
  mkdirSync(units, { recursive: true });
  const home = mainCheckout();
  writeFileSync(
    join(units, "terra-continuous.service"),
    `[Unit]\nDescription=Terra's continuous checks on main (tools/continuous)\n\n[Service]\nType=oneshot\nWorkingDirectory=${home}\nExecStart=/bin/sh -lc 'exec cargo xtask continuous'\n`,
  );
  writeFileSync(
    join(units, "terra-continuous.timer"),
    `[Unit]\nDescription=Terra's continuous checks on main every ${every} minutes\n\n[Timer]\nOnBootSec=5min\nOnUnitInactiveSec=${every}min\n\n[Install]\nWantedBy=timers.target\n`,
  );
  for (const args of [["--user", "daemon-reload"], ["--user", "enable", "--now", "terra-continuous.timer"]]) {
    if (spawnSync("systemctl", args, { stdio: "inherit" }).status !== 0) return 1;
  }
  console.log(`✓ continuous: terra-continuous.timer every ${every} min from ${home}; status ${join(home, ".git/terra-continuous/status.txt")}`);
  return 0;
}

function main(): number {
  const [flag, value] = process.argv.slice(2);
  const minutes = Number(value ?? 5) || 5;
  if (flag === undefined) return once();
  if (flag === "--install") return install(minutes);
  if (flag === "--watch") {
    for (;;) {
      once();
      sleep(minutes * 60_000);
    }
  }
  console.log("usage: cargo xtask continuous [--watch [minutes] | --install [minutes]]");
  return 2;
}

if (import.meta.main) process.exit(main());
