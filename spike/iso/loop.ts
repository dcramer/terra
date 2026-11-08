// The iso kit's judging loop (D634): shoot every biome on the render worker, measure each
// still against its own references, prepare the blind pair and the adversarial judge for
// each biome that changed, record their answers, and keep or revert the round by the
// ratchet (no biome may lose its pair). Node 22, no dependencies. It lives in the spike
// because it knows the spike's biomes (D133); the measure is tools/iso-art/stats.ts.
// The judges themselves are subagents the lead runs from the briefs this writes.
// How to use it: the iso-art skill's "The loop a job follows"; tests: loop.test.ts.

import { spawnSync } from "node:child_process";
import { appendFileSync, closeSync, copyFileSync, existsSync, mkdirSync, openSync, readdirSync, readFileSync, renameSync, statSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { basename, extname, join, resolve } from "node:path";
import { decodeAny, measure, type Rgb, type Stats } from "../../tools/iso-art/stats.ts";

const ROOT = resolve(import.meta.dirname, "../..");
const BIOMES = join(ROOT, "spike/iso/biomes");
export const KIT = process.env.ISO_KIT_HOME ?? join(homedir(), ".cache/terra-review/iso-kit");
const HISTORY = join(KIT, "history.json");
/** The worker's folder for a round's files, in its checkout (collected back). */
const OUT = "target/job/iso-loop";

/** The measures a line shows, in order, with their short names. */
export const SHOWN: [keyof Stats, string][] = [
  ["luma_p50", "p50"],
  ["luma_p95", "p95"],
  ["luma_mean", "mean"],
  ["fine_detail", "fine"],
  ["saturation", "sat"],
  ["dark_share", "dark"],
  ["mid_share", "mid"],
  ["bright_share", "bright"],
];
/** A measure moved by more than this is a change: two fixed-step shots of one ruin agree to
 * 0.0001 (iso-kit j4), real-time ones differed by 0.0025; the forest's still moves 0.0006 on
 * unchanged code (its clips start on the frame they load, iso-kit j5), so the bar sits over it. */
export const EPS = 0.001;
/** A pixel differs when a channel moved by more than this many levels (two shots of unchanged
 * code: 2.7% of pixels one level apart, the GPU's noise). */
export const LEVELS = 2;
/** A still changed when more than this share of its pixels differ (unchanged: the ruin 0.007%,
 * the forest up to 0.33% in specks on its fight's effects, iso-kit j5); a look change moves
 * several percent. */
export const PIXEL_SHARE = 0.005;
/** A biome's own noise (uc-infra): `noise` shoots it N times on one commit and keeps, in
 * spike/iso/noise.json, the most pixels any two shots differ by and each measure's spread;
 * a still changed only past this many times that, so a plain re-shoot is "same" (the forest's
 * wind and clips differ 1.2-2.4% between shots, the neon street's probe up to 9%). Shots in
 * one worker job differ less than shots of separate jobs (uc-infra, 7d65ed9c0's code: forest
 * 1.01% in one job, 2.41% across two; neon 4.24% against 8.96%), so the margin is 2.5. */
export const NOISE_MARGIN = 2.5;
const NOISE = join(ROOT, "spike/iso/noise.json");

/** What a biome's shots of one commit differ by: the most pixels of any two, each measure's
 * spread (high less low), over `shots` stills of commit `rev`. */
export type Noise = { share: number; spread: Partial<Stats>; shots: number; rev: string; at: string };
/** The bars a still must pass to have changed: a share of pixels, and a measure's move. */
export type Bar = { share: number; eps: Partial<Stats> };
export const PLAIN: Bar = { share: PIXEL_SHARE, eps: {} };

/** A biome's bars: its noise times `NOISE_MARGIN`, never under the plain ones. */
export function barOf(noise: Noise | undefined): Bar {
  if (!noise) return PLAIN;
  const eps: Partial<Stats> = {};
  for (const [k] of SHOWN) eps[k] = Math.max(EPS, Math.round((noise.spread[k] ?? 0) * NOISE_MARGIN * 1e4) / 1e4);
  return { share: Math.max(PIXEL_SHARE, noise.share * NOISE_MARGIN), eps };
}

/** The noise of N stills of one commit: the most pixels any two differ by, each measure's
 * spread. */
export function noiseOf(ims: Rgb[], rev: string, at: string): Noise {
  if (ims.length < 2) throw new Error(`noise needs two shots or more of one commit, not ${ims.length}`);
  let share = 0;
  for (let i = 0; i < ims.length; i++) for (let j = i + 1; j < ims.length; j++) share = Math.max(share, pixelShare(ims[i], ims[j]));
  const stats = ims.map(measure);
  const spread: Partial<Stats> = {};
  for (const [k] of SHOWN) {
    const v = stats.map((s) => s[k]);
    spread[k] = Math.round((Math.max(...v) - Math.min(...v)) * 1e4) / 1e4;
  }
  return { share: Math.round(share * 1e5) / 1e5, spread, shots: ims.length, rev, at };
}

function loadNoise(): Record<string, Noise> {
  return existsSync(NOISE) ? JSON.parse(readFileSync(NOISE, "utf8")) : {};
}

export type Margin = "slight" | "clear" | "strong";
export type Side = "new" | "kept";
/** `earlier`: answers a re-judge replaced (a non-blind judge's, say), oldest first. */
export type Pair = { folder: string; brief: string; a: Side; winner?: Side | "same"; margin?: Margin; earlier?: { winner: Side | "same"; margin?: Margin }[] };
export type Entry = {
  still: string;
  stats: Stats;
  /** Against the kept still: null when the biome had none. */
  changed: boolean | null;
  /** The kept still's round it was measured and paired against. */
  against?: string;
  delta?: Partial<Stats>;
  share?: number;
  pairs: Pair[];
  judge?: { folder: string; brief: string };
  /** `earlier`: scores a later judge's replaced, oldest first. */
  score?: { overall: number; gaps: string; earlier?: { overall: number; gaps: string }[] };
  film?: string;
};
export type Round = { at: string; rev: string; dirty: boolean; job: string; biomes: Record<string, Entry>; verdict?: { result: "KEEP" | "REVERT" | "DROP"; losers: string[]; at: string; rebased?: string; dropped?: string } };
export type Kept = { round: string; still: string; stats: Stats };
export type History = { kept: Record<string, Kept>; rounds: Record<string, Round> };

// ---- pure: parsing, measures, the ratchet, the blind order ----

/** A biome file's `refs` and `kind`, read past its comments (the schema: src/kit/biome.rs). */
export function parseBiome(text: string): { refs: string | null; kind: string[] } {
  const code = text.replace(/\/\/[^\n]*/g, "");
  const refs = code.match(/\brefs\s*:\s*"([^"]*)"/)?.[1] ?? null;
  const kind = code.match(/\bkind\s*:\s*\[([^\]]*)\]/)?.[1] ?? "";
  return { refs, kind: kind.split(",").map((k) => k.trim()).filter(Boolean) };
}

/** The low and high of each measure over the named refs of stats.json; the names it lacks. */
export function refRanges(all: Record<string, Partial<Stats>>, names: string[]) {
  const have = names.filter((n) => all[n]);
  const missing = names.filter((n) => !all[n]);
  if (have.length === 0) return { ranges: null, have, missing };
  const ranges = {} as Record<keyof Stats, [number, number]>;
  for (const [k] of SHOWN) {
    const v = have.map((n) => all[n][k]).filter((x): x is number => typeof x === "number");
    if (v.length) ranges[k] = [Math.min(...v), Math.max(...v)];
  }
  return { ranges, have, missing };
}

/** Each measure's change from the kept still, to the fourth place. */
export function delta(now: Stats, kept: Stats): Partial<Stats> {
  const d: Partial<Stats> = {};
  for (const [k] of SHOWN) d[k] = Math.round((now[k] - kept[k]) * 1e4) / 1e4;
  return d;
}

/** The share of pixels a channel of which moved by more than `LEVELS`; 1 if the sizes differ. */
export function pixelShare(a: Rgb, b: Rgb): number {
  if (a.width !== b.width || a.height !== b.height) return 1;
  let n = 0;
  for (let p = 0; p < a.width * a.height; p++) {
    for (let c = 0; c < 3; c++) {
      if (Math.abs(a.data[p * 3 + c] - b.data[p * 3 + c]) > LEVELS) {
        n++;
        break;
      }
    }
  }
  return n / (a.width * a.height);
}

export function isChanged(d: Partial<Stats>, share: number, bar: Bar = PLAIN): boolean {
  return Object.entries(d).some(([k, v]) => Math.abs(v ?? 0) > (bar.eps[k as keyof Stats] ?? EPS)) || share > bar.share;
}

const f4 = (v: number) => v.toFixed(4);
const signed = (v: number) => (v >= 0 ? "+" : "") + v.toFixed(4);

/** One biome's line: each measure beside the refs' range (↑ above it, ↓ below), then what
 * moved from the kept still. */
export function statsLine(biome: string, s: Stats, ranges: Record<string, [number, number]> | null, kept: { round: string; delta: Partial<Stats>; share: number; bar?: Bar } | null): string {
  const parts = SHOWN.map(([k, name]) => {
    const r = ranges?.[k];
    if (!r) return `${name} ${f4(s[k])}`;
    const mark = s[k] < r[0] ? "↓" : s[k] > r[1] ? "↑" : "";
    return `${name} ${f4(s[k])}${mark} (${r[0].toFixed(3)}-${r[1].toFixed(3)})`;
  });
  let tail: string;
  if (!kept) tail = "no kept still: new";
  else {
    const bar = kept.bar ?? PLAIN;
    const moved = SHOWN.filter(([k]) => Math.abs(kept.delta[k] ?? 0) > (bar.eps[k] ?? EPS)).map(([k, n]) => `${n} ${signed(kept.delta[k] ?? 0)}`);
    const px = `${(kept.share * 100).toFixed(2)}% px`;
    const noise = bar === PLAIN ? "" : ` (its noise: ${(bar.share * 100).toFixed(2)}% px)`;
    tail = isChanged(kept.delta, kept.share, bar) ? `vs kept ${kept.round}: ${moved.join(" ") || "measures within its noise"}, ${px} differ${noise}` : `same as kept ${kept.round}${noise}`;
  }
  return `${biome}: ${parts.join(" ")}${ranges ? "" : " (no refs yet)"} | ${tail}`;
}

/** How a biome came out of its round: `tie` unchanged or even, `win`, `loss`, `new` with no
 * kept still, `pending` while a pair is unanswered (or a slight loss awaits its swap). */
export type Outcome = "tie" | "win" | "loss" | "new" | "pending";

/** The ratchet's rule for one biome (D634): a loss is a clear or strong loss, or a slight loss
 * on both of two pairs with the order swapped. */
export function outcome(e: Pick<Entry, "changed" | "pairs">): Outcome {
  if (e.changed === null) return "new";
  if (!e.changed) return "tie";
  const [p1, p2] = e.pairs;
  if (!p1?.winner) return "pending";
  if (p1.winner === "same") return "tie";
  if (p1.margin !== "slight") return p1.winner === "new" ? "win" : "loss";
  // A slight answer either way is confirmed by the swapped pair: the same side twice is a win
  // or a loss, the same letter twice (the other side) is the judge's position bias, a tie.
  if (!p2?.winner) return "pending";
  if (p2.winner === p1.winner) return p1.winner === "new" ? "win" : "loss";
  return "tie";
}

/** The round's verdict: REVERT when a biome lost; TIED when it changed biomes and none of them
 * won, so keeping its stills would make a look no judge preferred the kept one (lead 8's trap,
 * 261107-0959); KEEP otherwise. Pending biomes block it. */
export function verdict(biomes: Record<string, Pick<Entry, "changed" | "pairs">>) {
  const out = Object.fromEntries(Object.entries(biomes).map(([b, e]) => [b, outcome(e)]));
  const pending = Object.keys(out).filter((b) => out[b] === "pending");
  const losers = Object.keys(out).filter((b) => out[b] === "loss");
  const moved = Object.keys(out).filter((b) => biomes[b].changed === true);
  const tied = moved.length > 0 && moved.every((b) => out[b] === "tie");
  const result = pending.length ? null : losers.length ? ("REVERT" as const) : tied ? ("TIED" as const) : ("KEEP" as const);
  return { out, pending, losers, tied: tied ? moved : [], result };
}

/** Which side goes in a.png: random for the first pair, the other way round for the second. */
export function blindOrder(rng: () => number, first?: Side): Side {
  if (first) return first === "new" ? "kept" : "new";
  return rng() < 0.5 ? "new" : "kept";
}

/** The answer `A`, `B` or `same` as the side that won, by the pair's hidden key. */
export function winnerOf(pick: string, a: Side): Side | "same" {
  const p = pick.toUpperCase();
  if (p === "SAME") return "same";
  if (p === "A") return a;
  if (p === "B") return a === "new" ? "kept" : "new";
  throw new Error(`a pair's answer is A, B or same, not "${pick}"`);
}

/** The spike's test count from a cargo test log, and whether any failed. */
export function testResult(log: string): { passed: number; failed: boolean } {
  let passed = 0;
  let failed = /test result: FAILED|panicked|error(\[E\d+\])?:/.test(log);
  for (const m of log.matchAll(/test result: (\w+)\. (\d+) passed; (\d+) failed/g)) {
    passed += Number(m[2]);
    if (m[3] !== "0") failed = true;
  }
  if (passed === 0) failed = true;
  return { passed, failed };
}

/**
 * What a biome's run log says that a round should show (iso-kit j20): its budget lines
 * (`... budget (...)`) and every shader or pipeline error with the line under it (j19's rain
 * was never drawn, a bad shader import warned only in the run log). Colour codes stripped.
 */
export function runNotes(log: string): string[] {
  const lines = log.split("\n").map((l) => l.replace(/\x1b\[[0-9;]*m/g, "").trimEnd());
  const notes: string[] = [];
  lines.forEach((l, i) => {
    if (/ budget \(/.test(l)) notes.push(l);
    else if (/shader_cache|wgpu.*(error|ERROR)|\bERROR\b/.test(l)) {
      const s = `${l.replace(/^\S+Z\s+/, "")}${lines[i + 1]?.startsWith("error:") ? ` ${lines[i + 1]}` : ""}`;
      if (!notes.includes(s)) notes.push(s);
    }
  });
  return notes;
}

/** A round's name: the day and minute it was shot, `yymmdd-hhmm`. */
export function stamp(d: Date): string {
  const p = (n: number) => String(n).padStart(2, "0");
  return `${p(d.getFullYear() % 100)}${p(d.getMonth() + 1)}${p(d.getDate())}-${p(d.getHours())}${p(d.getMinutes())}`;
}

// ---- files ----

export function biomeNames(): string[] {
  return readdirSync(BIOMES).filter((f) => f.endsWith(".ron")).map((f) => basename(f, ".ron")).sort();
}

function loadHistory(say: (l: string) => void): History {
  if (existsSync(HISTORY)) return JSON.parse(readFileSync(HISTORY, "utf8"));
  const h: History = { kept: {}, rounds: {} };
  mkdirSync(join(KIT, "kept"), { recursive: true });
  for (const b of biomeNames()) {
    const seed = join(KIT, "j1", `kit-${b}.png`);
    if (!existsSync(seed)) continue;
    const still = join(KIT, "kept", `${b}.png`);
    copyFileSync(seed, still);
    h.kept[b] = { round: "j1", still, stats: measure(decodeAny(still)) };
  }
  say(`seeded the kept stills from iso-kit j1 (${join(KIT, "j1")}/kit-*.png): ${Object.keys(h.kept).join(", ") || "none"}`);
  saveHistory(h);
  return h;
}

function saveHistory(h: History) {
  mkdirSync(KIT, { recursive: true });
  writeHistory(HISTORY, h);
}

/** Writes the history whole through a temporary file and a rename, so a reader never sees half. */
function writeHistory(file: string, h: History) {
  const tmp = `${file}.${process.pid}.tmp`;
  writeFileSync(tmp, JSON.stringify(h, null, 1));
  renameSync(tmp, file);
}

/**
 * Reserves a round's folder `<kit>/rounds/<base>`, or `<base>-2`, `-3` when that is taken in the
 * history or on disk. The mkdir is not recursive, so of two shoots started in the same minute
 * only one gets each name (iso-kit j35: both took 261107-0250 when it was chosen from the history).
 */
export function reserveRound(kit: string, base: string, taken: (name: string) => boolean = () => false): string {
  mkdirSync(join(kit, "rounds"), { recursive: true });
  for (let i = 1; ; i++) {
    const name = i === 1 ? base : `${base}-${i}`;
    if (taken(name)) continue;
    try {
      mkdirSync(join(kit, "rounds", name));
      return name;
    } catch (err) {
      if ((err as NodeJS.ErrnoException).code !== "EEXIST") throw err;
    }
  }
}

/**
 * Applies a change to the history as it is on disk now, not as it was read: a shoot writes some
 * minutes after it read, and a concurrent shoot's round or a record in between must survive, so
 * each writer merges only its own round's entry and the kept stills it changed (iso-kit j35).
 */
export function updateHistory(file: string, change: (h: History) => void) {
  const h: History = existsSync(file) ? JSON.parse(readFileSync(file, "utf8")) : { kept: {}, rounds: {} };
  change(h);
  writeHistory(file, h);
}

/** Copies a still at a width (ffmpeg's Lanczos), or as it is when it already has that width. */
function scaled(src: string, dst: string, width: number) {
  if (decodeAny(src).width === width) return copyFileSync(src, dst);
  const r = spawnSync("ffmpeg", ["-loglevel", "error", "-y", "-i", src, "-vf", `scale=${width}:-1:flags=lanczos`, dst], { encoding: "utf8" });
  if (r.status !== 0) throw new Error(`ffmpeg could not scale ${src} to ${width} px: ${r.stderr.trim()}; is ffmpeg on PATH?`);
}

/** The refs of a biome: the images in refs/<refs>/ and their range in refs/stats.json. */
function refsOf(biome: string) {
  const { refs, kind } = parseBiome(readFileSync(join(BIOMES, `${biome}.ron`), "utf8"));
  const dir = join(KIT, "refs", refs ?? biome);
  const names = existsSync(dir) ? readdirSync(dir).filter((f) => /\.(jpe?g|png|webp)$/i.test(f)).map((f) => basename(f, extname(f))) : [];
  const statsFile = join(KIT, "refs", "stats.json");
  const all = existsSync(statsFile) ? JSON.parse(readFileSync(statsFile, "utf8")) : {};
  return { dir, kind, ...refRanges(all, names) };
}

const JUDGE_MD = join(KIT, "judge.md");

function pairBrief(round: string, biome: string, n: number, folder: string, refs: string): string {
  return `# Blind pair: ${biome}, round ${round}, pair ${n}

Read ${JUDGE_MD}, part (b), the blind-pair brief, and follow it.
The two stills: ${folder}/a.png and ${folder}/b.png (960 px), the same biome at the same camera; one is the kit's kept still, which is which you are not told and must not try to learn.
The biome's references: ${refs}/960/.
Say which reads as the better, more realistic game frame against the references: A, B or same, and by how much: slight, clear or strong, with one line of why.
The lead records it: node spike/iso/loop.ts record ${round} ${biome} pair <A|B|same> <slight|clear|strong>
`;
}

function judgeBrief(round: string, biome: string, folder: string, refs: string, line: string): string {
  return `# Adversarial judge: ${biome}, round ${round}

Read ${JUDGE_MD}, part (a), the adversarial scene rubric, and follow it: find its faults.
The still: ${folder}/still-960.png for the whole frame, ${folder}/still-1920.png for 1:1 crops.
Its measures beside the references' ranges: ${folder}/stats.txt
  ${line}
The biome's references: ${refs}/960/.
Give an overall score of 10 and its gaps, the worst first.
The lead records it: node spike/iso/loop.ts record ${round} ${biome} score <overall> "<gaps, the worst first>"
`;
}

/** The blind folder of a biome's pair n: a.png and b.png only; the key goes in the history. */
function preparePair(round: string, biome: string, e: Entry, kept: Kept, refsDir: string): Pair {
  const n = e.pairs.length + 1;
  const a = blindOrder(Math.random, e.pairs[0]?.a);
  const folder = join(KIT, "blind", `${round}-${biome}-${n}`);
  mkdirSync(folder, { recursive: true });
  const [sa, sb] = a === "new" ? [e.still, kept.still] : [kept.still, e.still];
  scaled(sa, join(folder, "a.png"), 960);
  scaled(sb, join(folder, "b.png"), 960);
  const brief = join(KIT, "rounds", round, "briefs", `${biome}-pair${n}.md`);
  mkdirSync(join(KIT, "rounds", round, "briefs"), { recursive: true });
  writeFileSync(brief, pairBrief(round, biome, n, folder, refsDir));
  return { folder, brief, a };
}

// ---- the commands ----

class Stop extends Error {}
const fail = (msg: string): never => {
  throw new Stop(msg);
};

/** The worker's script: the spike's tests, then each biome's still (and film); with `shots`
 * over 1, that many stills a biome, `<biome>-<n>.png`, and no film. */
export function script(biomes: string[], film: boolean, shots = 1): string {
  const run = "cargo run --release --manifest-path spike/iso/Cargo.toml --";
  const lines = [`d=${OUT}; rm -rf $d; mkdir -p $d`, `cargo test --release --manifest-path spike/iso/Cargo.toml > $d/test.txt 2>&1`];
  for (const b of biomes) {
    if (shots > 1) {
      for (let n = 1; n <= shots; n++) lines.push(`${run} --biome ${b} --shot $d/${b}-${n}.png > $d/run-${b}-${n}.txt 2>&1`);
      continue;
    }
    lines.push(`${run} --biome ${b} --shot $d/${b}.png > $d/run-${b}.txt 2>&1`);
    // scene.md's film: 480 frames at a fixed 30 a game second from frame 300, 16 s, at the
    // still's 1920x1080 so the desk's 1600-wide encode scales down, never up.
    if (film) lines.push(`mkdir -p $d/film-${b}; ${run} --biome ${b} --frames 300 --film 480 --shot $d/film-${b}/f.jpg > $d/film-${b}.txt 2>&1`);
  }
  return lines.join("; ") + "; true";
}

/** The biomes `--biomes a,b` names, all of them when it is left out. */
function biomesArg(argv: string[]): string[] {
  const all = biomeNames();
  const bi = argv.indexOf("--biomes");
  const biomes = bi >= 0 ? (argv[bi + 1] ?? "").split(",").filter(Boolean) : all;
  const unknown = biomes.filter((b) => !all.includes(b));
  if (unknown.length || biomes.length === 0) fail(`no biome file for ${unknown.join(", ") || "--biomes (empty)"}: the biomes are spike/iso/biomes/*.ron (${all.join(", ")}); name them with --biomes a,b or leave it out for all`);
  return biomes;
}

/** Runs a script on the worker (its GPU), its output in `log`; the collected folder, the job's
 * name and the spike's test count, or a stop naming why. */
function onWorker(sh: string, dirty: boolean, minutes: number, log: string, say: (l: string) => void) {
  const fd = openSync(log, "w");
  const args = ["xtask", "remote-render", "--gpu", ...(dirty ? ["--dirty"] : []), "--minutes", String(minutes), "--collect", OUT, "--", "sh", "-c", sh];
  const r = spawnSync("cargo", args, { cwd: ROOT, stdio: ["ignore", fd, fd] });
  closeSync(fd);
  const text = readFileSync(log, "utf8");
  const job = text.match(/render (\S+): queued/)?.[1] ?? "";
  const done = text.match(/✓ render \S+: done .*files in (\S+)\s*$/m);
  if (r.status !== 0 || !done) fail(`the worker job ${job || "(none queued)"} did not finish: ${text.trim().split("\n").filter((l) => l.includes("✗")).pop() ?? "see the log"}; read ${log}, fix what it names and shoot again (nothing was filed)`);
  const out = join(done![1], OUT);
  const tests = testResult(existsSync(join(out, "test.txt")) ? readFileSync(join(out, "test.txt"), "utf8") : "");
  if (tests.failed) fail(`the spike's tests failed on the worker (${tests.passed} passed): read ${join(out, "test.txt")}; a round is shot on passing tests, fix them first`);
  say(`✓ worker job ${job}: spike tests ${tests.passed} pass`);
  const rev = spawnSync("git", ["rev-parse", "--short=9", "HEAD"], { cwd: ROOT, encoding: "utf8" }).stdout.trim();
  return { out, job, rev };
}

/** Shoots each biome N times on one commit and keeps its noise in spike/iso/noise.json. */
function noise(argv: string[], say: (l: string) => void) {
  const dirty = argv.includes("--dirty");
  const biomes = biomesArg(argv);
  const si = argv.indexOf("--shots");
  const shots = si >= 0 ? Number(argv[si + 1]) : 3;
  if (!(Number.isInteger(shots) && shots >= 2)) fail(`--shots is a whole number of two or more, not "${argv[si + 1]}": noise --shots 3`);
  const dir = join(KIT, "noise", stamp(new Date()));
  mkdirSync(dir, { recursive: true });
  const log = join(dir, "shoot.log");
  say(`noise: ${shots} shots each of ${biomes.join(", ")} on the worker${dirty ? " (uncommitted changes as a patch)" : ""}; log ${log}`);
  const { out, rev } = onWorker(script(biomes, false, shots), dirty, 30 + 10 * shots, log, say);
  const all = loadNoise();
  for (const b of biomes) {
    const files = Array.from({ length: shots }, (_, i) => join(out, `${b}-${i + 1}.png`)).filter(existsSync);
    if (files.length < 2) {
      say(`✗ ${b}: ${files.length} of its ${shots} stills came back; its run logs ${join(out, `run-${b}-*.txt`)} say why (grep panicked|ERROR)`);
      continue;
    }
    for (const f of files) copyFileSync(f, join(dir, basename(f)));
    const n = noiseOf(files.map(decodeAny), `${rev}${dirty ? "+dirty" : ""}`, new Date().toISOString());
    all[b] = n;
    const moved = SHOWN.filter(([k]) => (n.spread[k] ?? 0) > EPS).map(([k, s]) => `${s} ${f4(n.spread[k] ?? 0)}`);
    say(`${b}: ${(n.share * 100).toFixed(2)}% px between two shots at most, measures ${moved.join(" ") || `within ${EPS}`}; its bar ${(barOf(n).share * 100).toFixed(2)}% px`);
  }
  writeFileSync(NOISE, JSON.stringify(all, null, 1) + "\n");
  say(`wrote ${NOISE}: land it so the next shoot uses it${dirty ? " (measured on uncommitted changes: land them first)" : ""}`);
}

function shoot(argv: string[], say: (l: string) => void) {
  const film = argv.includes("--film");
  const dirty = argv.includes("--dirty");
  const biomes = biomesArg(argv);
  const h = loadHistory(say);
  const round = reserveRound(KIT, stamp(new Date()), (n) => !!h.rounds[n]);
  const dir = join(KIT, "rounds", round);
  const log = join(dir, "shoot.log");
  say(`round ${round}: shooting ${biomes.join(", ")}${film ? " with films" : ""} on the worker${dirty ? " (uncommitted changes as a patch)" : ""}; log ${log}`);
  const { out, job, rev } = onWorker(script(biomes, film), dirty, film ? 90 : 45, log, say);
  const rnd: Round = { at: new Date().toISOString(), rev, dirty, job, biomes: {} };
  let judges = 0;
  for (const b of biomes) {
    // Its run log's budgets and shader errors, in the round's log and on screen.
    const run = join(out, `run-${b}.txt`);
    for (const n of existsSync(run) ? runNotes(readFileSync(run, "utf8")) : []) {
      say(`  ${b}: ${n}`);
      appendFileSync(log, `${b}: ${n}\n`);
    }
    // Each biome's line also goes in the round's stats.txt, where its judges read it.
    const e = fileBiome(round, b, out, film, h.kept[b], (l) => {
      say(l);
      appendFileSync(join(dir, "stats.txt"), `${l}\n`);
    });
    if (!e) continue;
    rnd.biomes[b] = e;
    if (e.judge) judges++;
  }
  updateHistory(HISTORY, (now) => {
    now.rounds[round] = rnd;
  });
  const ties = Object.values(rnd.biomes).filter((e) => e.changed === false).length;
  if (judges) say(`next: run the briefs of the ${judges} changed biome${judges > 1 ? "s" : ""} in ${join(dir, "briefs")} (the lead, as subagents), record each answer, then: node spike/iso/loop.ts verdict ${round}`);
  else say(`every biome ties its kept still (${ties}): nothing to judge; node spike/iso/loop.ts verdict ${round} keeps it`);
}

/** Files a biome's still (and film), measures it, and prepares its judges if it changed. */
function fileBiome(round: string, b: string, out: string, film: boolean, kept: Kept | undefined, say: (l: string) => void): Entry | null {
  const dir = join(KIT, "rounds", round);
  const src = join(out, `${b}.png`);
  if (!existsSync(src)) {
    say(`✗ ${b}: no still came back; its run log ${join(out, `run-${b}.txt`)} says why (grep panicked|ERROR)`);
    return null;
  }
  const still = join(dir, `${b}.png`);
  copyFileSync(src, still);
  const im = decodeAny(still);
  const stats = measure(im);
  const refs = refsOf(b);
  let e: Entry = { still, stats, changed: null, pairs: [] };
  let keptInfo = null;
  if (kept) {
    const d = delta(stats, kept.stats);
    const share = pixelShare(im, decodeAny(kept.still));
    const bar = barOf(loadNoise()[b]);
    e = { ...e, delta: d, share, changed: isChanged(d, share, bar), against: kept.round };
    keptInfo = { round: kept.round, delta: d, share, bar };
  }
  const line = statsLine(b, stats, refs.ranges, keptInfo);
  say(line + (refs.missing.length ? ` [${refs.missing.length} refs not in stats.json]` : ""));
  if (film) e.film = makeFilm(b, out, dir, say);
  if (e.changed === false) return e;
  const jf = join(dir, "judge", b);
  mkdirSync(jf, { recursive: true });
  scaled(still, join(jf, "still-960.png"), 960);
  scaled(still, join(jf, "still-1920.png"), 1920);
  writeFileSync(join(jf, "stats.txt"), line + "\n");
  mkdirSync(join(dir, "briefs"), { recursive: true });
  const brief = join(dir, "briefs", `${b}-judge.md`);
  writeFileSync(brief, judgeBrief(round, b, jf, refs.dir, line));
  e.judge = { folder: jf, brief };
  if (kept) e.pairs.push(preparePair(round, b, e, kept, refs.dir));
  return e;
}

/** The film's encode for the developer's desk (under 15 MB for 16 s), in the round and as
 * the biome's latest film in KIT/films/, where the desk links it. */
function makeFilm(b: string, out: string, dir: string, say: (l: string) => void): string | undefined {
  const mp4 = join(dir, `${b}.mp4`);
  const enc = ["-c:v", "libx264", "-crf", "28", "-preset", "slow", "-g", "480", "-vf", "scale=1600:-2", "-pix_fmt", "yuv420p", "-movflags", "+faststart"];
  const r = spawnSync("ffmpeg", ["-loglevel", "error", "-y", "-framerate", "30", "-i", join(out, `film-${b}`, "f-%04d.jpg"), "-an", ...enc, mp4], { encoding: "utf8" });
  if (r.status !== 0) {
    say(`✗ ${b}: its film's frames did not make a film (${r.stderr.trim().split("\n").pop()}); its run log ${join(out, `film-${b}.txt`)}`);
    return;
  }
  mkdirSync(join(KIT, "films"), { recursive: true });
  copyFileSync(mp4, join(KIT, "films", `${b}.mp4`));
  say(`  ${b}: film ${mp4} (${(statSync(mp4).size / 1e6).toFixed(1)} MB), the desk's copy ${join(KIT, "films", `${b}.mp4`)}`);
  return mp4;
}

function record(argv: string[], say: (l: string) => void) {
  const [round, biome, what, ...rest] = argv;
  const h = loadHistory(say);
  const e = h.rounds[round]?.biomes[biome];
  if (!e) fail(`no biome ${biome} in round ${round}: \`node spike/iso/loop.ts history\` lists the rounds and their biomes`);
  if (what === "score") {
    const overall = Number(rest[0]);
    if (!(overall >= 0 && overall <= 10)) fail(`a score is a number of 10, not "${rest[0]}": record ${round} ${biome} score 5.5 "<gaps>"`);
    // A second judge's score keeps the first's, oldest first, as a re-judged pair does.
    const was = e!.score;
    const earlier = was ? [...(was.earlier ?? []), { overall: was.overall, gaps: was.gaps }] : undefined;
    e!.score = { overall, gaps: rest.slice(1).join(" "), ...(earlier ? { earlier } : {}) };
    say(`${biome} ${round}: score ${overall} recorded${was ? ` (was ${was.overall}, kept in its "earlier")` : ""}`);
  } else if (what === "pair") {
    const r = rest.indexOf("--rejudge");
    const which = r >= 0 && /^\d+$/.test(rest[r + 1] ?? "") ? Number(rest[r + 1]) : 1;
    const args = r < 0 ? rest : rest.filter((_, i) => i !== r && !(i === r + 1 && /^\d+$/.test(rest[i])));
    const p = r >= 0 ? e!.pairs[which - 1] : e!.pairs.find((x) => !x.winner);
    if (r >= 0 && !p?.winner) fail(`${biome} in ${round} has no answered pair ${which} to re-judge (${e!.pairs.length} pair${e!.pairs.length === 1 ? "" : "s"}); an unanswered pair is recorded without --rejudge`);
    if (!p) fail(`${biome} in ${round} has no pair waiting for an answer (${e!.changed === false ? "it tied its kept still unchanged" : "each is answered"}); a blind judge's answer over an earlier one: record ${round} ${biome} pair <A|B|same> <margin> --rejudge <n>`);
    const margin = args[1] as Margin | undefined;
    if (args[0]?.toUpperCase() !== "SAME" && !["slight", "clear", "strong"].includes(margin ?? "")) fail(`a pair's answer needs its margin: record ${round} ${biome} pair <A|B|same> <slight|clear|strong>`);
    if (r >= 0) p!.earlier = [...(p!.earlier ?? []), { winner: p!.winner!, margin: p!.margin }];
    p!.winner = winnerOf(args[0] ?? "", p!.a);
    p!.margin = margin;
    const n = r >= 0 ? which : e!.pairs.length;
    say(`${biome} ${round}: pair ${n} ${r >= 0 ? `re-judged (was ${p!.earlier!.at(-1)!.winner} ${p!.earlier!.at(-1)!.margin ?? ""})` : "recorded"}`);
    const said = h.rounds[round].verdict;
    const now = verdict({ ...h.rounds[round].biomes, [biome]: e! }).result;
    if (said && !said.rebased && now && (now === "KEEP") !== (said.result === "KEEP"))
      say(`warning: ${round}'s verdict was ${said.result}, its pairs now say ${now}: undo or redo what it did (\`git revert\` the change, then \`node spike/iso/loop.ts verdict ${round} --revert\`; or land it again and judge a new round)`);
    if (r >= 0 && n === 1 && e!.pairs[1]?.winner && p!.winner !== "same" && margin === "slight")
      say(`pair 2 keeps its earlier answer (${e!.pairs[1].winner} ${e!.pairs[1].margin ?? ""}): re-judge it too, \`record ${round} ${biome} pair <A|B|same> <margin> --rejudge 2\` (its brief ${e!.pairs[1].brief})`);
    if (n === 1 && e!.pairs.length === 1 && p!.winner !== "same" && margin === "slight") {
      const kept = h.kept[biome];
      const q = preparePair(round, biome, e!, kept, refsOf(biome).dir);
      e!.pairs.push(q);
      say(`a slight answer ${p!.winner === "new" ? "for" : "against"} the change: a second pair, the order swapped, confirms it (the same letter twice is a tie); brief ${q.brief}`);
    }
  } else fail(`record takes "pair" or "score": record <round> <biome> pair <A|B|same> <slight|clear|strong>, or record <round> <biome> score <overall> "<gaps>"`);
  updateHistory(HISTORY, (now) => {
    now.rounds[round].biomes[biome] = e!;
  });
}

/** The biomes of `round` whose kept still is from a later round: a KEEP of `round` would put
 * its older stills back over them and drop the newer win (lead 5 and 6 lost kept stills so). */
export function newerKept(h: Pick<History, "kept" | "rounds">, round: string): string[] {
  const at = h.rounds[round]?.at ?? "";
  return Object.keys(h.rounds[round]?.biomes ?? {}).filter((b) => {
    const k = h.kept[b]?.round;
    return k !== undefined && k !== round && (h.rounds[k]?.at ?? "") > at;
  });
}

/** The kept round a biome of `round` was measured against: its `against`, or for a round shot
 * before that was recorded, the last round kept for the biome before `round` was shot. */
export function shotAgainst(h: Pick<History, "rounds">, round: string, b: string): string | undefined {
  const r = h.rounds[round];
  if (r?.biomes[b]?.against) return r.biomes[b].against;
  let best: [string, string] | undefined;
  for (const [name, o] of Object.entries(h.rounds)) {
    const v = o.verdict;
    if (name === round || !o.biomes[b] || v?.result !== "KEEP" || !r || v.at >= r.at) continue;
    if (!best || v.at > best[1]) best = [name, v.at];
  }
  return best?.[0];
}

/** The round whose look a kept round's still shows for a biome: past every round that only
 * tied the one before it (a KEEP copies its ties' stills, the same look). */
function lookOf(h: Pick<History, "rounds">, round: string | undefined, b: string): string | undefined {
  for (let r = round, n = 0; r && n < 100; n++) {
    if (h.rounds[r]?.biomes[b]?.changed !== false) return r;
    const prev = shotAgainst(h, r, b);
    if (!prev) return r;
    r = prev;
  }
  return round;
}

/** The biomes of `round` measured against a kept still that has since been replaced: a tie
 * there only matched the older still (keeping it would undo the newer win), a change was
 * judged against the wrong one. */
export function staleAgainst(h: Pick<History, "kept" | "rounds">, round: string): { ties: string[]; changed: string[] } {
  const out = { ties: [] as string[], changed: [] as string[] };
  for (const [b, e] of Object.entries(h.rounds[round]?.biomes ?? {})) {
    const was = lookOf(h, shotAgainst(h, round, b), b);
    const now = lookOf(h, h.kept[b]?.round, b);
    if (!was || !now || was === now || now === round) continue;
    (e.changed ? out.changed : out.ties).push(b);
  }
  return out;
}

function judgeVerdict(argv: string[], say: (l: string) => void) {
  const round = argv.find((a) => !a.startsWith("--"))!;
  const revert = argv.includes("--revert");
  const h = loadHistory(say);
  const rnd = h.rounds[round];
  if (!rnd) fail(`no round ${round}: \`node spike/iso/loop.ts history\` lists them`);
  // A revert copies no stills, so a round older than a kept one (or judged against a replaced
  // one) may still be recorded as reverted.
  const later = revert ? [] : newerKept(h, round);
  if (later.length) fail(`${round} is older than the kept still of ${later.map((b) => `${b} (${h.kept[b].round})`).join(", ")}: a verdict on it would put its older stills back over the newer kept ones (D634's ratchet only moves forward). Judge the newest round; to restore an older round's stills on purpose, \`node spike/iso/loop.ts rebase ${round} "restore: <why>"\``);
  const stale = staleAgainst(h, round);
  if (stale.changed.length && !revert) fail(`${stale.changed.map((b) => `${b} (judged against kept ${shotAgainst(h, round, b)}, kept now ${h.kept[b].round})`).join(", ")}: ${round}'s pair was judged against a kept still since replaced, so its answer says nothing against the new one. Shoot the change again on top of the newer kept look (rebase the change onto main) and judge that round`);
  const v = verdict(rnd!.biomes);
  for (const [b, o] of Object.entries(v.out)) {
    const s = rnd!.biomes[b].score;
    say(`${b}: ${o}${s ? `, score ${s.overall}` : ""}`);
  }
  // A revert copies no stills, so it closes a round still waiting on a pair (its change undone,
  // or unable to win: a slight answer against it, 261107-1721-2); a keep waits for every answer.
  if (!v.result && !revert) {
    const briefs = v.pending.map((b) => rnd!.biomes[b].pairs.find((p) => !p.winner)?.brief).filter(Boolean);
    fail(`no verdict yet: ${v.pending.join(", ")} wait for a pair's answer; run ${briefs.join(", ") || "a second pair's brief"} and record each, or record the change undone: verdict ${round} --revert`);
  }
  if (!v.result) say(`${v.pending.join(", ")}: closed unanswered by the revert`);
  if (v.result === "TIED" && !revert)
    fail(`${round}: ${v.tied.join(", ")} changed and only tied, so no judge preferred the change; keeping it would make its stills the kept ones while the change is reverted (D634's ratchet moves only on a gain). Revert the change and record it: \`node spike/iso/loop.ts verdict ${round} --revert\` (the kept stills stay); to keep the look anyway, land it and judge a new round`);
  const result = revert || v.result !== "KEEP" ? ("REVERT" as const) : ("KEEP" as const);
  if (result === "KEEP") {
    mkdirSync(join(KIT, "kept"), { recursive: true });
    for (const [b, e] of Object.entries(rnd!.biomes)) {
      if (stale.ties.includes(b)) continue;
      const still = join(KIT, "kept", `${b}.png`);
      copyFileSync(e.still, still);
      h.kept[b] = { round, still, stats: e.stats };
    }
    if (stale.ties.length) say(`${stale.ties.join(", ")}: tied only an older kept still, so the newer kept one stays`);
    say(`KEEP ${round}: no biome lost; its stills are the kept ones now (${join(KIT, "kept")})`);
  } else say(`REVERT ${round}: ${v.losers.length ? `${v.losers.join(", ")} lost` : v.tied.length ? `${v.tied.join(", ")} only tied` : "reverted by hand"}; undo the change shot in it (${rnd!.rev}${rnd!.dirty ? " with uncommitted changes" : ""}: git revert, never a force-push); the kept stills stay`);
  const said = { result, losers: v.losers, at: new Date().toISOString() };
  updateHistory(HISTORY, (now) => {
    now.rounds[round].verdict = said;
    if (result === "KEEP") for (const b of Object.keys(rnd!.biomes)) now.kept[b] = h.kept[b];
  });
}

/** Makes a round's stills the kept ones unjudged, when how the stills are shot changed (the
 * fixed step, the size, the camera), never the look: the ratchet's baseline moves, said why. */
function rebase(argv: string[], say: (l: string) => void) {
  const [round, ...words] = argv;
  const why = words.join(" ").trim();
  const h = loadHistory(say);
  const rnd = h.rounds[round];
  if (!rnd) fail(`no round ${round}: \`node spike/iso/loop.ts history\` lists them`);
  if (!why) fail(`rebase needs its why, a change of how stills are shot and never of the look (that is judged): rebase ${round} "<why>"`);
  mkdirSync(join(KIT, "kept"), { recursive: true });
  for (const [b, e] of Object.entries(rnd!.biomes)) {
    const still = join(KIT, "kept", `${b}.png`);
    copyFileSync(e.still, still);
    h.kept[b] = { round, still, stats: e.stats };
  }
  const said = { result: "KEEP" as const, losers: [], at: new Date().toISOString(), rebased: why };
  updateHistory(HISTORY, (now) => {
    now.rounds[round].verdict = said;
    for (const b of Object.keys(rnd!.biomes)) now.kept[b] = h.kept[b];
  });
  say(`KEEP ${round} unjudged as the kept stills of ${Object.keys(rnd!.biomes).join(", ")}: ${why}`);
}

/** The biomes of `round` with an answered pair that still says something: one judged against
 * a kept still since replaced says nothing of the new one (staleAgainst), so it may be dropped. */
export function liveAnswers(h: Pick<History, "kept" | "rounds">, round: string): string[] {
  const stale = staleAgainst(h, round).changed;
  return Object.entries(h.rounds[round]?.biomes ?? {}).filter(([b, e]) => e.pairs.some((p) => p.winner) && !stale.includes(b)).map(([b]) => b);
}

/** Closes a round no judge will answer (a plain re-shoot, a withdrawn or superseded change, a
 * film-only round) as DROP, said why: its stills are never kept and nothing measures against it
 * (shotAgainst reads only KEEPs). A round a judge answered closes by its pairs instead. */
function drop(argv: string[], say: (l: string) => void) {
  const [round, ...words] = argv;
  const why = words.join(" ").trim();
  const h = loadHistory(say);
  const rnd = h.rounds[round];
  if (!rnd) fail(`no round ${round}: \`node spike/iso/loop.ts history\` lists them`);
  if (!why) fail(`drop needs its why (a plain re-shoot, withdrawn, superseded by a later round): drop ${round} "<why>"`);
  if (rnd!.verdict) fail(`${round} is closed already (${rnd!.verdict.result}${rnd!.verdict.dropped ? `: ${rnd!.verdict.dropped}` : ""}); a verdict is changed by \`verdict ${round} [--revert]\`, never dropped over`);
  const live = liveAnswers(h, round);
  if (live.length) fail(`${round}'s ${live.join(", ")} ${live.length > 1 ? "have" : "has"} a judge's answer, so the round closes by its pairs (D634): \`node spike/iso/loop.ts verdict ${round}\`, or \`verdict ${round} --revert\` when the change was undone or cannot win; drop is for rounds no judge answered`);
  const said = { result: "DROP" as const, losers: [], at: new Date().toISOString(), dropped: why };
  updateHistory(HISTORY, (now) => {
    now.rounds[round].verdict = said;
  });
  say(`DROP ${round} unjudged, its stills not kept: ${why}`);
}

function history(say: (l: string) => void) {
  const h = loadHistory(say);
  const names = [...new Set([...Object.keys(h.kept), ...Object.values(h.rounds).flatMap((r) => Object.keys(r.biomes))])].sort();
  for (const b of names) {
    const cells = Object.entries(h.rounds).filter(([, r]) => r.biomes[b]).map(([name, r]) => {
      const e = r.biomes[b];
      const o = r.verdict?.rebased || r.verdict?.dropped ? "unjudged" : outcome(e);
      const p = e.pairs.filter((x) => x.winner).map((x) => `${x.winner}${x.margin ? " " + x.margin : ""}`).join("/");
      const v = r.verdict ? ` ${r.verdict.result}${r.verdict.rebased ? " (rebased)" : ""}` : "";
      return `${name} ${o}${p ? ` (${p})` : ""}${e.score ? ` ${e.score.overall}` : ""}${v}`;
    });
    say(`${b}: kept ${h.kept[b]?.round ?? "none"} · ${cells.join(" · ") || "no rounds"}`);
  }
}

const USAGE = `usage: node spike/iso/loop.ts
  shoot [--biomes a,b] [--film] [--dirty]   every biome's still on the worker, measured and judged against its kept one
  noise [--biomes a,b] [--shots 3] [--dirty] each biome shot N times on one commit: its noise in spike/iso/noise.json, the bar a shoot's "same" passes
  record <round> <biome> pair <A|B|same> <slight|clear|strong> [--rejudge [n]]   --rejudge: a blind answer over pair n's earlier one (kept in its "earlier")
  record <round> <biome> score <overall> "<gaps>"
  verdict <round> [--revert]                KEEP if no biome lost and a changed one won, REVERT if one lost; a round whose changes only tied is refused unless --revert
  rebase <round> "<why>"                    its stills kept unjudged, when how stills are shot changed, never the look
  drop <round> "<why>"                      closes a round no judge answered (a plain re-shoot, withdrawn, superseded) as DROP; its stills never kept
  history                                  each biome's rounds, pairs, scores and kept still`;

export function main(argv: string[], say = (l: string) => console.log(l)): number {
  const [cmd, ...rest] = argv;
  try {
    if (cmd === "shoot") shoot(rest, say);
    else if (cmd === "noise") noise(rest, say);
    else if (cmd === "record") record(rest, say);
    else if (cmd === "verdict") judgeVerdict(rest, say);
    else if (cmd === "rebase") rebase(rest, say);
    else if (cmd === "drop") drop(rest, say);
    else if (cmd === "history") history(say);
    else return say(USAGE), 2;
    return 0;
  } catch (err) {
    if (!(err instanceof Stop)) throw err;
    say(`✗ ${err.message}`);
    return 1;
  }
}

if (import.meta.main) process.exitCode = main(process.argv.slice(2));
