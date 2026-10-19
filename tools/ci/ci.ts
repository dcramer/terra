// The CI agent (docs/ci-agent.md): runs the full run on main (`cargo xtask nightly`, D411) and
// turns its report into GitHub issues, labelled `ci-agent`: one a failing stage, commented when
// its lines change and closed when it passes again, and one status issue edited every run, so a
// box that stopped shows. Its state, lock and log are in the git folder, `.git/terra-ci/`.
// `cargo xtask ci [--dry-run] [--no-run]` runs once; `--watch [minutes]` runs continuously, as
// soon as origin/main moves, and is what the container in this folder runs (docs/ci-agent.md).
// Node 22.18 or later; no dependencies; git, cargo and gh.

import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { closeSync, existsSync, mkdirSync, openSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { arch, cpus, release, totalmem, type } from "node:os";
import { join, resolve } from "node:path";

const ROOT = resolve(import.meta.dirname, "../..");
const LABEL = "ci-agent";
const STATUS = "<!-- ci-agent:status -->";
/// The most lines of a stage an issue quotes; the report keeps 20 (nightly.rs, REPORT_LINES).
const QUOTED = 50;

export type Failure = { stage: string; found?: string; lines: string[] };
export type Report = {
  passed: boolean;
  commit: string;
  when: string;
  summary: string;
  failures: Failure[];
  stages: { name: string; ok: boolean }[];
};
export type Issue = { number: number; title: string; body: string };
export type Action =
  | { kind: "create"; stage: string; title: string; body: string }
  | { kind: "comment"; number: number; stage: string; body: string }
  | { kind: "close"; number: number; stage: string; body: string };

export const marker = (stage: string) => `<!-- ci-agent:nightly/${stage} -->`;

/// The report `cargo xtask nightly` writes (`passed at <commit> (<when>): …` or `failed at …`,
/// nightly.rs's `failure`), or nothing when its first line is neither.
export function parse(text: string): Report | undefined {
  const lines = text.split("\n");
  const head = /^(passed|failed) at ([0-9a-f]{7,}) \(([^)]*)\): (.*)$/.exec(lines[0] ?? "");
  if (!head) return undefined;
  const [, verdict, commit, when, summary] = head;
  const report: Report = { passed: verdict === "passed", commit, when, summary, failures: [], stages: [] };
  if (report.passed) return report;
  const names = summary.split(", ");
  let current: Failure | undefined;
  for (const line of lines.slice(1)) {
    if (line.startsWith("stages: ")) {
      report.stages = line
        .slice("stages: ".length)
        .split(", ")
        .map((s) => ({ name: s.slice(2), ok: s.startsWith("✓") }));
    } else if (line.startsWith("    ") && current) {
      current.lines.push(line.slice(4));
    } else if (!current && line.startsWith(`  ${names[0]} `)) {
      current = { stage: names[0], found: line.slice(names[0].length + 3), lines: [] };
      report.failures.push(current);
    } else if (line.endsWith(":") && names.includes(line.slice(0, -1))) {
      current = { stage: line.slice(0, -1), lines: [] };
      report.failures.push(current);
    }
  }
  return report;
}

const fingerprint = (f: Failure) => createHash("sha1").update(f.lines.join("\n")).digest("hex");

function details(report: Report, f: Failure, machine: string): string {
  const quoted = f.lines.slice(0, QUOTED).join("\n");
  return [
    `At \`${report.commit}\` (${report.when}).${f.found ? ` ${f.found}.` : ""}`,
    "",
    "```",
    quoted || "(no lines: see the stage's log)",
    "```",
    "",
    `Reproduce: \`cargo xtask nightly\` (or the stage alone, as \`.git/terra-nightly/logs/${f.stage}.log\` names it). Full logs on the CI box in \`.git/terra-nightly/logs/\`. Machine: ${machine}.`,
  ].join("\n");
}

/// What to do on GitHub for a report, given the open `ci-agent` issues and the fingerprint of
/// the lines last posted for each stage: open an issue for a stage newly failing, comment when
/// its lines changed, close one whose stage passed. `run` is the run itself failing to start.
export function plan(
  report: Report,
  open: Issue[],
  posted: Record<string, string>,
  machine: string,
): { actions: Action[]; posted: Record<string, string> } {
  const actions: Action[] = [];
  const next = { ...posted };
  const issueOf = (stage: string) => open.find((i) => i.body.includes(marker(stage)));
  for (const f of report.failures) {
    const issue = issueOf(f.stage);
    const print = fingerprint(f);
    if (!issue) {
      const title = `nightly: ${f.stage} fails${f.stage === "run" ? " to run" : ""}`;
      actions.push({ kind: "create", stage: f.stage, title, body: `${marker(f.stage)}\n${details(report, f, machine)}` });
    } else if (posted[f.stage] !== print) {
      actions.push({ kind: "comment", number: issue.number, stage: f.stage, body: `Still failing, with other lines.\n\n${details(report, f, machine)}` });
    }
    next[f.stage] = print;
  }
  const failing = new Set(report.failures.map((f) => f.stage));
  const ran = report.failures.every((f) => f.stage !== "run");
  const passed = new Set(report.stages.filter((s) => s.ok).map((s) => s.name));
  for (const issue of open) {
    const stage = /<!-- ci-agent:nightly\/([^ ]+) -->/.exec(issue.body)?.[1];
    if (!stage || failing.has(stage)) continue;
    if (report.passed || passed.has(stage) || (stage === "run" && ran)) {
      actions.push({ kind: "close", number: issue.number, stage, body: `Passes at \`${report.commit}\` (${report.when}).` });
      delete next[stage];
    }
  }
  return { actions, posted: next };
}

function machine(): string {
  const cpu = cpus();
  return `${type()} ${release()} ${arch()}, ${cpu.length} threads (${cpu[0]?.model.trim() ?? "?"}), ${Math.round(totalmem() / 2 ** 30)} GB`;
}

function run(cmd: string, args: string[], input?: string): { ok: boolean; out: string } {
  const r = spawnSync(cmd, args, { cwd: ROOT, encoding: "utf8", input, stdio: [input === undefined ? "ignore" : "pipe", "pipe", "pipe"] });
  return { ok: r.status === 0, out: `${r.stdout ?? ""}${r.status === 0 ? "" : (r.stderr ?? r.error?.message ?? "")}` };
}

function gitCommon(): string {
  return run("git", ["rev-parse", "--path-format=absolute", "--git-common-dir"]).out.trim();
}

function minutes(ms: number): string {
  return `${Math.round(ms / 60000)}m`;
}

/// Holds `.git/terra-ci/lock` (a folder, with the holder's pid) or says who does.
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
      console.log(`ci: already running (pid ${pid})`);
      return undefined;
    }
    rmSync(dir, { recursive: true, force: true });
    mkdirSync(dir);
  }
  writeFileSync(join(dir, "pid"), `${process.pid}\n`);
  return () => rmSync(dir, { recursive: true, force: true });
}

type Seen = { last?: string; posted: Record<string, string>; labelled?: boolean };

function once(dry: boolean, noRun: boolean): number {
  const common = gitCommon();
  const state = join(common, "terra-ci");
  mkdirSync(state, { recursive: true });
  const unlock = lock(state);
  if (!unlock) return 0;
  try {
    const seenFile = join(state, "seen.json");
    const seen: Seen = existsSync(seenFile) ? JSON.parse(readFileSync(seenFile, "utf8")) : { posted: {} };
    const reportFile = join(common, "terra-nightly", "report.txt");
    const before = existsSync(reportFile) ? readFileSync(reportFile, "utf8") : "";
    const log = join(state, "run.log");
    let startFailure: string[] | undefined;
    if (!noRun) {
      const started = Date.now();
      const fd = openSync(log, "w");
      const r = spawnSync("cargo", ["xtask", "nightly"], { cwd: ROOT, stdio: ["ignore", fd, fd] });
      closeSync(fd);
      const said = readFileSync(log, "utf8").split("\n");
      const verdict = said.findLast((l) => /^[✓✗] nightly|^nightly:/.test(l)) ?? "nightly: said nothing";
      console.log(`${verdict} (${minutes(Date.now() - started)}, ${log})`);
      const after = existsSync(reportFile) ? readFileSync(reportFile, "utf8") : "";
      if (r.status !== 0 && after === before) startFailure = said.filter((l) => l.trim()).slice(-QUOTED);
    }
    const text = existsSync(reportFile) ? readFileSync(reportFile, "utf8") : "";
    const head = (text.split("\n")[0] ?? "").trim();
    let report = parse(text);
    if (startFailure) {
      const tip = run("git", ["rev-parse", "origin/main"]).out.trim();
      const when = new Date().toISOString().slice(0, 16).replace("T", " ") + " UTC";
      report = { passed: false, commit: tip, when, summary: "run", failures: [{ stage: "run", lines: startFailure }], stages: [] };
    } else if (!report || head === seen.last) {
      report = undefined;
    }
    const repo = run("gh", ["repo", "view", "--json", "nameWithOwner", "-q", ".nameWithOwner"]);
    if (!repo.ok) {
      console.log(`✗ ci: gh cannot see the repository; run \`gh auth login\` (${repo.out.trim().split("\n")[0]})`);
      return 1;
    }
    const R = ["-R", repo.out.trim()];
    if (!seen.labelled && !dry) {
      run("gh", ["label", "create", LABEL, ...R, "--force", "--color", "b60205", "--description", "Filed by the CI agent (docs/ci-agent.md)"]);
      seen.labelled = true;
    }
    const listed = run("gh", ["issue", "list", ...R, "--label", LABEL, "--state", "open", "--json", "number,title,body", "-L", "200"]);
    const open: Issue[] = listed.ok ? JSON.parse(listed.out) : [];
    const box = machine();
    let counts = { create: 0, comment: 0, close: 0 };
    if (report) {
      const { actions, posted } = plan(report, open, seen.posted, box);
      for (const a of actions) {
        counts[a.kind]++;
        if (dry) {
          console.log(`  would ${a.kind} ${a.kind === "create" ? `"${a.title}"` : `#${a.number}`} (${a.stage})`);
          continue;
        }
        const done =
          a.kind === "create"
            ? run("gh", ["issue", "create", ...R, "--title", a.title, "--label", LABEL, "--body-file", "-"], a.body)
            : a.kind === "comment"
              ? run("gh", ["issue", "comment", String(a.number), ...R, "--body-file", "-"], a.body)
              : run("gh", ["issue", "close", String(a.number), ...R, "--comment", a.body]);
        if (!done.ok) console.log(`✗ ci: gh issue ${a.kind} (${a.stage}): ${done.out.trim().split("\n")[0]}`);
      }
      seen.posted = posted;
      if (!startFailure) seen.last = head;
    }
    const status = [
      STATUS,
      `Last checked ${new Date().toISOString().slice(0, 16).replace("T", " ")} UTC on ${box}.`,
      "",
      startFailure ? `The run failed to start: see the \`nightly: run fails to run\` issue.` : `Last nightly: \`${head || "none yet"}\``,
      "",
      "Run by `cargo xtask ci` (docs/ci-agent.md). If this stops changing, the CI box is down.",
    ].join("\n");
    const statusIssue = open.find((i) => i.body.includes(STATUS));
    if (!dry) {
      if (statusIssue) run("gh", ["issue", "edit", String(statusIssue.number), ...R, "--body-file", "-"], status);
      else run("gh", ["issue", "create", ...R, "--title", "CI agent status", "--label", LABEL, "--body-file", "-"], status);
      writeFileSync(seenFile, `${JSON.stringify(seen, null, 2)}\n`);
    }
    const verb = dry ? "would be " : "";
    console.log(
      report
        ? `${dry ? "~" : "✓"} issues: ${counts.create} ${verb}opened, ${counts.comment} ${verb}commented, ${counts.close} ${verb}closed`
        : `✓ issues: nothing new in the report (${head || "no report yet"})`,
    );
    return 0;
  } finally {
    unlock();
  }
}

/// How often an unchanged main still updates the status issue, so a box that stopped shows.
const HEARTBEAT_MS = 60 * 60 * 1000;

function sleep(ms: number): void {
  Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, ms);
}

/// Runs whenever origin/main moves, checking every so many minutes: a continuous build, as
/// build bots run one, each run taking the commits landed during the last (nightly bisects).
/// The checkout is fast-forwarded first when it is on main and clean; when that changes this
/// folder, it exits, for whatever started it (the container's restart policy) to start the new.
function watch(every: number): number {
  console.log(`ci: watching origin/main every ${every} min`);
  let ran = "";
  let beat = 0;
  for (;;) {
    run("git", ["fetch", "-q", "origin", "main"]);
    const tip = run("git", ["rev-parse", "origin/main"]).out.trim();
    const head = run("git", ["rev-parse", "HEAD"]).out.trim();
    const onMain = run("git", ["symbolic-ref", "-q", "--short", "HEAD"]).out.trim() === "main";
    const clean = run("git", ["status", "--porcelain", "--untracked-files=no"]).out.trim() === "";
    if (onMain && clean && head !== tip && run("git", ["merge", "-q", "--ff-only", "origin/main"]).ok) {
      if (!run("git", ["diff", "--quiet", head, tip, "--", "tools/ci"]).ok) {
        console.log("ci: tools/ci changed; exiting to be started again");
        return 0;
      }
    }
    if (tip !== ran || Date.now() - beat > HEARTBEAT_MS) {
      once(false, false);
      ran = tip;
      beat = Date.now();
    }
    sleep(every * 60 * 1000);
  }
}

function main(): number {
  const args = process.argv.slice(2);
  if (args[0] === "--watch") return watch(Number(args[1] ?? 2) || 2);
  const unknown = args.filter((a) => a !== "--dry-run" && a !== "--no-run");
  if (unknown.length) {
    console.log("usage: cargo xtask ci [--dry-run] [--no-run] | cargo xtask ci --watch [minutes]");
    return 2;
  }
  return once(args.includes("--dry-run"), args.includes("--no-run"));
}

if (import.meta.main) process.exit(main());
