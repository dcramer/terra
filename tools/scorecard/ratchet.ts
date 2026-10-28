// A scorecard's ratchet (D452): criteria judged against their bars and held to a baseline,
// shared by every area's scorecard (tools/combat, tools/controller; README.md here).
//
// A criterion's number is how far it is outside its bar ("miss", 0 within it), so worse is
// always a larger miss. A line a criterion: ✓ within its bar; · below it and no worse than its
// baseline; ✗ worse than its baseline, below its bar with none, or not measured; ? not
// measurable yet; - heavy, not run. It fails (exit 1) on a ✗ alone. `--raise "<why>"` writes
// the rows that got worse with the why, in the change's commit; `--lower` the rows that got
// better, nightly's alone (D414). Either banks a row not held yet as its "first number", never
// under the raise's why; `--first ID,...` writes held rows again as their first numbers.
// Node 22.18 or later; no dependencies.

import { writeFileSync } from "node:fs";

export type Bar = { at_most: number } | { at_least: number } | { within: [number, number] } | { pass: true } | null;
export type Criterion<Check = unknown> = {
  id: string;
  noticed: number;
  notice: string;
  bar: Bar;
  unit: string;
  source: string;
  check: Check;
  cost: "cheap" | "heavy";
  measurable: boolean;
  needs?: string;
  slack?: number;
};
export type Held = { value: number; why: string };
export type Status = "pass" | "held" | "better" | "worse" | "new" | "missing" | "unmeasured" | "skipped";
export type Result = { id: string; status: Status; value?: number; miss?: number; base?: number; note?: string };

/// Two numbers this close are the same: values are kept to three decimals.
const EPS = 1e-6;

export const round = (x: number) => Math.round(x * 1000) / 1000;

/// How far a value is outside its bar, in its unit; 0 within it. A test passes as 0, fails as 1.
export function miss(value: number, bar: Bar): number {
  if (!bar) return 0;
  if ("at_most" in bar) return Math.max(0, value - bar.at_most);
  if ("at_least" in bar) return Math.max(0, bar.at_least - value);
  if ("within" in bar) return Math.max(0, bar.within[0] - value, value - bar.within[1]);
  return value;
}

export function barText(bar: Bar, unit: string): string {
  const u = unit ? ` ${unit}` : "";
  if (!bar) return "none yet";
  if ("at_most" in bar) return `at most ${bar.at_most}${u}`;
  if ("at_least" in bar) return `at least ${bar.at_least}${u}`;
  if ("within" in bar) return `${bar.within[0]}-${bar.within[1]}${u}`;
  return "passes";
}

/// A criterion against its baseline: within its bar, held, better, worse, or new (no baseline
/// and outside its bar).
export function judge(c: Criterion, value: number | undefined, base: Held | undefined): Result {
  if (!c.measurable) return { id: c.id, status: "unmeasured", note: c.needs };
  if (value === undefined) return { id: c.id, status: "missing" };
  const m = round(miss(value, c.bar));
  const b = base === undefined ? undefined : round(miss(base.value, c.bar));
  const r = { id: c.id, value, miss: m, base: base?.value };
  if (b === undefined) return { ...r, status: m === 0 ? "pass" : "new" };
  if (m > b + (c.slack ?? 0) + EPS) return { ...r, status: "worse" };
  if (m === 0) return { ...r, status: "pass", note: b > 0 ? "better than its baseline" : undefined };
  return { ...r, status: m < b - EPS ? "better" : "held" };
}

/// The baseline: a row a criterion, `id`, `value`, `why` (tab-separated, a header first).
export function readBaseline(text: string): Map<string, Held> {
  const held = new Map<string, Held>();
  for (const line of text.split("\n").slice(1)) {
    const [id, value, why] = line.split("\t");
    if (id && value !== undefined) held.set(id, { value: Number(value), why: why ?? "-" });
  }
  return held;
}

export function writeBaseline(held: Map<string, Held>, order: string[]): string {
  const ids = [...order.filter((id) => held.has(id)), ...[...held.keys()].filter((id) => !order.includes(id)).sort()];
  return ["id\tvalue\twhy", ...ids.map((id) => `${id}\t${held.get(id)!.value}\t${held.get(id)!.why}`)].join("\n") + "\n";
}

/// The why of a row's first number: a row not held yet is nobody's raise or lowering.
export const firstNumber = (day: string) => `first number ${day}`;

/// The baseline after a raise (rows worse take their value with the why) or a lowering (rows
/// better, or within their bar now and outside it before, take theirs). Either banks a row not
/// held yet (within its bar or not) as its first number, with `first`'s why, never the raise's.
/// A raise never lowers a held row and a lowering never raises one (D414).
export function rebaseline(held: Map<string, Held>, results: Result[], mode: "raise" | "lower", why: string, first = firstNumber(today())): Map<string, Held> {
  const next = new Map(held);
  for (const r of results) {
    if (r.value === undefined) continue;
    if (r.base === undefined) {
      next.set(r.id, { value: r.value, why: first });
      continue;
    }
    const raise = r.status === "worse";
    // Within its bar now: banked when it was outside it (`note`).
    const lower = r.status === "better" || (r.status === "pass" && r.note !== undefined);
    if ((mode === "raise" && raise) || (mode === "lower" && lower)) next.set(r.id, { value: r.value, why });
  }
  return next;
}

/// The baseline with the rows `ids` written again as their first numbers: for a row banked under
/// another row's why. Only a row measured and no worse than held; each refused one with why.
export function rebank(held: Map<string, Held>, results: Result[], ids: string[], first = firstNumber(today())): { next: Map<string, Held>; refused: string[] } {
  const next = new Map(held);
  const refused: string[] = [];
  for (const id of ids) {
    const r = results.find((each) => each.id === id);
    if (!r || r.value === undefined) refused.push(`${id}: not measured in this run`);
    else if (!held.has(id)) refused.push(`${id}: not in the baseline (a raise or nightly banks it)`);
    else if (r.status === "worse") refused.push(`${id}: worse than its baseline, raise it with its why`);
    else next.set(id, { value: held.get(id)!.value, why: first });
  }
  return { next, refused };
}

const today = () => new Date().toISOString().slice(0, 10);

/// Each named test's verdict from cargo's output: `test <path> ... ok`, 0 passed, 1 failed.
export function testVerdicts(out: string): Map<string, number> {
  const verdicts = new Map<string, number>();
  // A name that ran in two crates' tests passes only if both passed.
  for (const m of out.matchAll(/^test (\S+) \.\.\. (ok|FAILED)/gm)) verdicts.set(m[1], Math.max(verdicts.get(m[1]) ?? 0, m[2] === "ok" ? 0 : 1));
  return verdicts;
}

/// How the scorecards build and run the Rust tests their criteria name: one cargo run over every
/// member's lib tests, names after `--`, whatever crate a test is in. Cargo resolves the
/// dependencies' features for the packages chosen, so `-p <crate>` for each crate built Bevy
/// and our crates again for every selection; over the workspace, the combat and controller
/// scorecards and `cargo test --workspace` share one build, the one the shared seed holds
/// (D183), and a fresh worktree's first run compiles each of our crates once.
export const TEST_BUILD = ["test", "-q", "--workspace", "--lib"];

const mark: Record<Status, string> = { pass: "✓", held: "·", better: "·", worse: "✗", new: "✗", missing: "✗", unmeasured: "?", skipped: "-" };
const fmt = (x: number) => String(round(x));

/// A criterion's line: its mark, id, number and bar, and what its verdict asks for. `heavy`
/// says how a heavy criterion is run; a `test` check reads passes or fails.
export function line(c: Criterion, r: Result, scorecard: string, heavy: string): string {
  const test = (c.check as { by?: string } | null)?.by === "test";
  const v = r.value === undefined ? "" : test ? (r.value === 0 ? ": passes" : ": fails") : `: ${fmt(r.value)}${c.unit ? ` ${c.unit}` : ""}`;
  const bar = test || r.value === undefined ? "" : ` (bar ${barText(c.bar, c.unit)})`;
  const said: Record<Status, string> = {
    pass: "",
    held: "",
    better: `, better than its baseline ${fmt(r.base!)}: nightly banks it`,
    worse: `, WORSE than its baseline ${fmt(r.base!)}: undo, or ${scorecard} --raise "<why>" in this commit`,
    new: `, below its bar with no baseline: --raise "<why>" banks its first number, or nightly does`,
    missing: ": not measured (its check gave no number; the log says why)",
    unmeasured: `: not measurable yet, needs ${r.note}`,
    skipped: `: heavy, ${heavy}`,
  };
  const note = r.status === "pass" && r.note ? `, ${r.note}: nightly banks it` : "";
  return `${mark[r.status]} ${c.id}${v}${bar}${said[r.status]}${note}`;
}

/// What a scorecard's run asked for, and where its files are.
export type Finish = {
  name: string;
  criteria: Criterion[];
  results: Result[];
  held: Map<string, Held>;
  baseline: string;
  baselineName: string;
  log: string;
  heavy: string;
  heavyRun: boolean;
  json?: string;
  raise?: string;
  lower: boolean;
  /** `--first ID,...`: write these held rows again as their first numbers (`rebank`). */
  first?: string[];
};

/// Prints a line a criterion, writes the JSON and the raised or lowered baseline, and the
/// summary; sets the exit code to 1 when a criterion is worse or without its number.
export function finish(f: Finish): void {
  const { criteria, results } = f;
  for (const [i, c] of criteria.entries()) console.log(line(c, results[i], `cargo xtask ${f.name}`, f.heavy));
  const count = (s: Status[]) => results.filter((r) => s.includes(r.status)).length;
  const bad = results.filter((r) => ["worse", "new", "missing"].includes(r.status));
  if (f.json) writeFileSync(f.json, JSON.stringify(criteria.map((c, i) => ({ ...c, ...results[i] })), null, 1) + "\n");
  if (f.raise || f.lower) {
    const next = rebaseline(f.held, results, f.raise ? "raise" : "lower", f.raise ?? `lowered by nightly ${new Date().toISOString().slice(0, 10)}`);
    const changed = [...next.entries()].filter(([id, h]) => f.held.get(id)?.value !== h.value).length;
    writeFileSync(f.baseline, writeBaseline(next, criteria.map((c) => c.id)));
    const firsts = [...next.keys()].filter((id) => !f.held.has(id)).length;
    console.log(`${f.name}: ${f.raise ? "raised" : "lowered"} ${changed - firsts} rows of ${f.baselineName}, ${firsts} first numbers`);
    for (const r of bad) if (r.status !== "missing" && (f.raise || r.status === "new")) r.status = "held";
  } else if (f.first?.length) {
    const { next, refused } = rebank(f.held, results, f.first);
    for (const why of refused) console.log(`✗ ${f.name} --first ${why}`);
    if (refused.length) process.exitCode = 1;
    else {
      writeFileSync(f.baseline, writeBaseline(next, criteria.map((c) => c.id)));
      console.log(`${f.name}: ${f.first.length} rows of ${f.baselineName} written again as their first numbers`);
    }
  }
  const still = results.filter((r) => ["worse", "new", "missing"].includes(r.status));
  const heavy = f.heavyRun ? "" : `, ${count(["skipped"])} heavy (${f.heavy})`;
  const summary = `${count(["pass"])} pass, ${count(["held", "better"])} below their bar and held, ${still.length} worse than their baseline or without their number, ${count(["unmeasured"])} not measurable yet${heavy}`;
  if (still.length) {
    console.log(`✗ ${f.name}: ${summary} (log ${f.log}):`);
    for (const r of still) console.log(`  ${line(criteria.find((c) => c.id === r.id)!, r, `cargo xtask ${f.name}`, f.heavy)}`);
    process.exitCode = 1;
  } else console.log(`✓ ${f.name}: ${summary}; held to ${f.baselineName}`);
}
