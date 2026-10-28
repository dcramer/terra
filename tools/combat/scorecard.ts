#!/usr/bin/env node
// The combat scorecard (D452, tools/combat/README.md): every criterion of criteria.json
// measured, a line each, and held to baseline.tsv, the combat ratchet.
//
//   cargo xtask combat                    the cheap criteria (the move set's timings, the move
//                                         check on every body, the fight's rule tests and what
//                                         they measure), seconds
//   cargo xtask combat --film             and the duel film's checks (the GPU, minutes)
//   cargo xtask combat --film --out DIR   the duel's film, record and checks into DIR, not
//                                         ~/.cache/terra-review/duel (two films at once share it)
//   cargo xtask combat --filmed           the film's rows from main's own duel film, not filming:
//                                         the newest clean one nothing it reads changed since
//                                         (tools/scorecard/films.ts; nightly's, so it banks them)
//   cargo xtask combat --json FILE        the results also as JSON
//   cargo xtask combat --raise "<why>"    write the rows that got worse, or are new, with why
//   cargo xtask combat --lower            write the rows that got better (nightly's alone, D414)
//
// A line a criterion: ✓ within its bar; · below its bar and no worse than its baseline (the
// task list, tools/combat's skill); ✗ worse than its baseline, or not measured; ? not
// measurable yet. It fails (exit 1) on a ✗ alone: a change may leave a criterion below its
// bar, never make it worse unless the same commit raises its baseline with --raise and a why.
// A criterion's number is how far it is outside its bar ("miss", 0 within it), so worse is
// always a larger miss. What the steps print goes to target/combat/scorecard.log.
// Node 22.18 or later; no dependencies.

import { spawnSync } from "node:child_process";
import { appendFileSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { join, resolve } from "node:path";
import { parseArgs } from "node:util";
import { COMBAT_FILMS, filmedThrough, keep, mainFilm, stamp } from "../scorecard/films.ts";
import { type Criterion as Row, finish, type Held, judge, readBaseline, round, type Status, TEST_BUILD, testVerdicts } from "../scorecard/ratchet.ts";

// The ratchet is shared (tools/scorecard); its parts are named here too, for this scorecard's tests.
export { type Bar, barText, type Held, judge, miss, readBaseline, rebaseline, type Result, round, type Status, testVerdicts, writeBaseline } from "../scorecard/ratchet.ts";

const ROOT = resolve(import.meta.dirname, "../..");
export const CRITERIA = resolve(import.meta.dirname, "criteria.json");
export const BASELINE = resolve(import.meta.dirname, "baseline.tsv");
const SET = resolve(ROOT, "assets/data/moves/sword.shielded.ron");
/// The player's body, feet to crown (the walker's figure), for spacing in body heights.
const BODY_M = 1.8;
export type CheckOf =
  | { by: "timing"; measure: string; move?: string; next?: string }
  | { by: "moves"; row: string; measure: string; hold?: string[] }
  | { by: "test"; crate: string; test: string }
  | { by: "measured"; crate: string; test: string }
  | { by: "film"; name: string }
  | null;
export type Criterion = Row<CheckOf>;

/// A move's numbers in the move set, read without a RON parser: each `"name": (` and its
/// fields to the next.
export type Move = { from: number; speed: number; active?: [number, number]; done: number; cancel?: number; next?: string; hit_stop: number; stop_at?: number };
export function readSet(text: string): Map<string, Move> {
  const body = text.replace(/\/\/.*$/gm, "");
  const start = body.indexOf("moves:");
  const names = [...body.slice(start).matchAll(/"([\w-]+)":\s*\(/g)];
  const set = new Map<string, Move>();
  names.forEach((name, i) => {
    const chunk = body.slice(start).slice(name.index, names[i + 1]?.index);
    const num = (key: string) => {
      const m = new RegExp(`\\b${key}:\\s*([\\d.]+)`).exec(chunk);
      return m ? Number(m[1]) : undefined;
    };
    const active = /\bactive:\s*\(\s*([\d.]+)\s*,\s*([\d.]+)\s*\)/.exec(chunk);
    const next = /\bnext:\s*"([\w-]+)"/.exec(chunk);
    set.set(name[1], {
      from: num("from") ?? 0,
      speed: num("speed") ?? 1,
      active: active ? [Number(active[1]), Number(active[2])] : undefined,
      done: num("done") ?? 0,
      cancel: num("cancel"),
      next: next?.[1],
      hit_stop: num("hit_stop") ?? 0,
      stop_at: num("stop_at"),
    });
  });
  return set;
}

/// A timing in real seconds ((clip time - from) / speed), as the Enshrouded comparison reads
/// it (research/2026-10-24-enshrouded-combat-measured.md): the hit is the active span's start;
/// a chained blow starts at its predecessor's cancel.
export function timing(set: Map<string, Move>, check: { measure: string; move?: string; next?: string }): number | undefined {
  const m = check.move ? set.get(check.move) : undefined;
  const real = (mv: Move, t: number) => (t - mv.from) / mv.speed;
  switch (check.measure) {
    case "idle_to_hit":
      return m?.active && real(m, m.active[0]);
    case "hit_to_guard":
      return m?.active && (m.done - m.active[0]) / m.speed;
    case "total":
      return m && real(m, m.done);
    case "cancel":
      return m?.cancel !== undefined ? real(m, m.cancel) : undefined;
    case "hit_stop":
      return m?.hit_stop;
    case "hit_to_hit": {
      const n = check.next ? set.get(check.next) : undefined;
      if (!m?.active || m.cancel === undefined || !n?.active) return undefined;
      return (m.cancel - m.active[0]) / m.speed + real(n, n.active[0]);
    }
    case "chain_blows": {
      let blows = 0;
      const seen = new Set<string>();
      for (let name = check.move; name && set.has(name) && !seen.has(name); name = set.get(name)!.next) seen.add(name), blows++;
      return blows || undefined;
    }
    case "spacing": {
      const stops = [...set.entries()].filter(([name, mv]) => !name.startsWith("partner") && mv.active && mv.stop_at !== undefined).map(([, mv]) => mv.stop_at!);
      return stops.length ? Math.max(...stops) / BODY_M : undefined;
    }
  }
  return undefined;
}

/// The numbers a passing test printed of what it measured, `measured <criterion id> <value>`
/// (a test that fails prints none, so its criteria have no number).
export function measuredValues(out: string): Map<string, number> {
  const values = new Map<string, number>();
  for (const m of out.matchAll(/measured ([\w.]+) (-?\d+(?:\.\d+)?(?:e-?\d+)?)/g)) values.set(m[1], Number(m[2]));
  return values;
}

/// The move check's rows (moves.ts --json): a row named by its move, or a clip with no move.
type MovesRow = { move?: string; clip: string; measured?: Record<string, number> };
export function movesValue(rows: MovesRow[], check: { row: string; measure: string }): number | undefined {
  const row = rows.find((r) => r.move === check.row) ?? rows.find((r) => !r.move && r.clip === check.row);
  return row?.measured?.[check.measure];
}

function main() {
  const { values: opt } = parseArgs({
    options: {
      film: { type: "boolean", default: false },
      filmed: { type: "boolean", default: false },
      out: { type: "string" },
      json: { type: "string" },
      raise: { type: "string" },
      lower: { type: "boolean", default: false },
    },
  });
  if (opt.raise !== undefined && !opt.raise.trim()) throw new Error("--raise needs its why");
  const criteria: Criterion[] = JSON.parse(readFileSync(CRITERIA, "utf8")).criteria;
  const held = existsSync(BASELINE) ? readBaseline(readFileSync(BASELINE, "utf8")) : new Map<string, Held>();
  const logDir = resolve(ROOT, "target/combat");
  mkdirSync(logDir, { recursive: true });
  const LOG = join(logDir, "scorecard.log");
  writeFileSync(LOG, `scorecard.ts ${process.argv.slice(2).join(" ")}\n`);
  const run = (command: string, args: string[]) => {
    appendFileSync(LOG, `\n$ ${command} ${args.join(" ")}\n`);
    const r = spawnSync(command, args, { cwd: ROOT, encoding: "utf8", maxBuffer: 1 << 28 });
    const out = (r.stdout ?? "") + (r.stderr ?? "");
    appendFileSync(LOG, out);
    return out;
  };
  // Main's duel film without filming (nightly's): the newest clean film of this commit's
  // history that nothing the film reads changed since (tools/scorecard/films.ts).
  let filmedAt: string | undefined;
  if (opt.filmed && !opt.film) {
    const found = mainFilm(COMBAT_FILMS);
    if ("why" in found) console.log(`  film rows not judged: ${found.why}`);
    else {
      filmedAt = found.dir;
      console.log(`  film rows from main's film at ${found.commit.slice(0, 10)}, ${found.behind} commits back, nothing the film reads changed since`);
    }
  }
  const film = opt.film || filmedAt !== undefined;
  const wanted = criteria.filter((c) => c.measurable && (c.cost === "cheap" || film));
  const values = new Map<string, number>();
  const filmValues = (json: string) => {
    const checks: { name: string; faults: number; value?: number }[] = existsSync(json) ? JSON.parse(readFileSync(json, "utf8")) : [];
    for (const c of wanted) if (c.check?.by === "film") {
      const found = checks.find((each) => each.name === (c.check as { name: string }).name);
      if (found) values.set(c.id, found.value ?? found.faults);
    }
  };

  // The move set's timings.
  const set = readSet(readFileSync(SET, "utf8"));
  for (const c of wanted) if (c.check?.by === "timing") {
    const v = timing(set, c.check);
    if (v !== undefined) values.set(c.id, round(v));
  }
  // The move check, on every body template: a run for each set of held looks the rows name
  // (`hold`, moves.ts's --hold; the prototype's sword and shield when none), so other items in
  // hand (a torch carried in the off hand, holding.md) are measured over the same clips.
  const holds = new Map<string, Criterion[]>();
  for (const c of wanted) if (c.check?.by === "moves") {
    const key = JSON.stringify(c.check.hold ?? []);
    holds.set(key, [...(holds.get(key) ?? []), c]);
  }
  for (const [index, [key, cs]] of [...holds].entries()) {
    const hold: string[] = JSON.parse(key);
    const json = join(logDir, hold.length ? `moves.${index}.json` : "moves.json");
    run(process.execPath, [resolve(ROOT, "tools/characters/moves.ts"), "--json", json, ...hold.flatMap((h) => ["--hold", h])]);
    const rows: MovesRow[] = existsSync(json) ? JSON.parse(readFileSync(json, "utf8")) : [];
    for (const c of cs) {
      const v = c.check?.by === "moves" ? movesValue(rows, c.check) : undefined;
      if (v !== undefined) values.set(c.id, round(v));
    }
  }
  // The rule tests, and the numbers rule tests measure (printed as they pass): a cargo run each,
  // over the workspace's lib tests (TEST_BUILD), whatever crates they are in.
  const testing = wanted.filter((c) => c.check?.by === "test");
  if (testing.length) {
    const names = [...new Set(testing.map((c) => (c.check as { test: string }).test))];
    const verdicts = testVerdicts(run("cargo", [...TEST_BUILD, "--", "--exact", "--format=pretty", ...names]));
    for (const c of testing) {
      const v = verdicts.get((c.check as { test: string }).test);
      if (v !== undefined) values.set(c.id, v);
    }
  }
  const measuring = wanted.filter((c) => c.check?.by === "measured");
  if (measuring.length) {
    const names = [...new Set(measuring.map((c) => (c.check as { test: string }).test))];
    const measured = measuredValues(run("cargo", [...TEST_BUILD, "--", "--exact", "--nocapture", "--test-threads=1", ...names]));
    for (const c of measuring) {
      const v = measured.get(c.id);
      if (v !== undefined) values.set(c.id, round(v));
    }
  }
  // The duel film's checks (tools/sandbox/duel.ts writes checks.json beside its film); a clean
  // checkout's are kept by its commit, for nightly to bank them.
  if (opt.film) {
    const out = opt.out ? resolve(opt.out) : join(homedir(), ".cache/terra-review/duel");
    const json = join(out, "checks.json");
    rmSync(json, { force: true });
    const before = stamp();
    run(process.execPath, [resolve(ROOT, "tools/sandbox/duel.ts"), "--out", out]);
    if (existsSync(json)) console.log(`  duel film ${keep(out, filmedThrough(before, stamp()), COMBAT_FILMS)}`);
    filmValues(json);
  } else if (filmedAt) filmValues(join(filmedAt, "checks.json"));

  const results = criteria.map((c) => (c.measurable && c.cost === "heavy" && !film ? { id: c.id, status: "skipped" as Status } : judge(c, values.get(c.id), held.get(c.id))));
  finish({
    name: "combat",
    criteria,
    results,
    held,
    baseline: BASELINE,
    baselineName: "tools/combat/baseline.tsv",
    log: "target/combat/scorecard.log",
    heavy: "run with --film",
    heavyRun: film,
    json: opt.json,
    raise: opt.raise,
    lower: opt.lower,
  });
}

if (import.meta.main) main();
