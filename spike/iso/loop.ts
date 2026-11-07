// The iso kit's judging loop (D634): shoot every biome on the render worker, measure each
// still against its own references, prepare the blind pair and the adversarial judge for
// each biome that changed, record their answers, and keep or revert the round by the
// ratchet (no biome may lose its pair). Node 22, no dependencies. It lives in the spike
// because it knows the spike's biomes (D133); the measure is tools/iso-art/stats.ts.
// The judges themselves are subagents the lead runs from the briefs this writes.
// How to use it: the iso-art skill's "The loop a job follows"; tests: loop.test.ts.

import { spawnSync } from "node:child_process";
import { appendFileSync, closeSync, copyFileSync, existsSync, mkdirSync, openSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
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

export type Margin = "slight" | "clear" | "strong";
export type Side = "new" | "kept";
export type Pair = { folder: string; brief: string; a: Side; winner?: Side | "same"; margin?: Margin };
export type Entry = {
  still: string;
  stats: Stats;
  /** Against the kept still: null when the biome had none. */
  changed: boolean | null;
  delta?: Partial<Stats>;
  share?: number;
  pairs: Pair[];
  judge?: { folder: string; brief: string };
  score?: { overall: number; gaps: string };
  film?: string;
};
export type Round = { at: string; rev: string; dirty: boolean; job: string; biomes: Record<string, Entry>; verdict?: { result: "KEEP" | "REVERT"; losers: string[]; at: string; rebased?: string } };
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

export function isChanged(d: Partial<Stats>, share: number): boolean {
  return Object.values(d).some((v) => Math.abs(v ?? 0) > EPS) || share > PIXEL_SHARE;
}

const f4 = (v: number) => v.toFixed(4);
const signed = (v: number) => (v >= 0 ? "+" : "") + v.toFixed(4);

/** One biome's line: each measure beside the refs' range (↑ above it, ↓ below), then what
 * moved from the kept still. */
export function statsLine(biome: string, s: Stats, ranges: Record<string, [number, number]> | null, kept: { round: string; delta: Partial<Stats>; share: number } | null): string {
  const parts = SHOWN.map(([k, name]) => {
    const r = ranges?.[k];
    if (!r) return `${name} ${f4(s[k])}`;
    const mark = s[k] < r[0] ? "↓" : s[k] > r[1] ? "↑" : "";
    return `${name} ${f4(s[k])}${mark} (${r[0].toFixed(3)}-${r[1].toFixed(3)})`;
  });
  let tail: string;
  if (!kept) tail = "no kept still: new";
  else {
    const moved = SHOWN.filter(([k]) => Math.abs(kept.delta[k] ?? 0) > EPS).map(([k, n]) => `${n} ${signed(kept.delta[k] ?? 0)}`);
    const px = `${(kept.share * 100).toFixed(2)}% px`;
    tail = isChanged(kept.delta, kept.share) ? `vs kept ${kept.round}: ${moved.join(" ") || "measures within " + EPS}, ${px} differ` : `same as kept ${kept.round}`;
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
  if (p1.winner === "new") return "win";
  if (p1.margin !== "slight") return "loss";
  if (!p2?.winner) return "pending";
  if (p2.winner === "kept") return "loss"; // slight twice, or worse the second time
  return "tie";
}

/** The round's verdict: KEEP when no biome lost; pending biomes block it. */
export function verdict(biomes: Record<string, Pick<Entry, "changed" | "pairs">>) {
  const out = Object.fromEntries(Object.entries(biomes).map(([b, e]) => [b, outcome(e)]));
  const pending = Object.keys(out).filter((b) => out[b] === "pending");
  const losers = Object.keys(out).filter((b) => out[b] === "loss");
  return { out, pending, losers, result: pending.length ? null : losers.length ? ("REVERT" as const) : ("KEEP" as const) };
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
  writeFileSync(HISTORY, JSON.stringify(h, null, 1));
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

/** The worker's script: the spike's tests, then each biome's still (and film). */
function script(biomes: string[], film: boolean): string {
  const run = "cargo run --release --manifest-path spike/iso/Cargo.toml --";
  const lines = [`d=${OUT}; rm -rf $d; mkdir -p $d`, `cargo test --release --manifest-path spike/iso/Cargo.toml > $d/test.txt 2>&1`];
  for (const b of biomes) {
    lines.push(`${run} --biome ${b} --shot $d/${b}.png > $d/run-${b}.txt 2>&1`);
    // scene.md's film: 480 frames at a fixed 30 a game second from frame 300, 16 s.
    if (film) lines.push(`mkdir -p $d/film-${b}; ${run} --biome ${b} --size 1280x720 --frames 300 --film 480 --shot $d/film-${b}/f.jpg > $d/film-${b}.txt 2>&1`);
  }
  return lines.join("; ") + "; true";
}

function shoot(argv: string[], say: (l: string) => void) {
  const film = argv.includes("--film");
  const dirty = argv.includes("--dirty");
  const all = biomeNames();
  const bi = argv.indexOf("--biomes");
  const biomes = bi >= 0 ? (argv[bi + 1] ?? "").split(",").filter(Boolean) : all;
  const unknown = biomes.filter((b) => !all.includes(b));
  if (unknown.length || biomes.length === 0) fail(`no biome file for ${unknown.join(", ") || "--biomes (empty)"}: the biomes are spike/iso/biomes/*.ron (${all.join(", ")}); name them with --biomes a,b or leave it out for all`);
  const h = loadHistory(say);
  let round = stamp(new Date());
  for (let i = 2; h.rounds[round]; i++) round = `${stamp(new Date())}-${i}`;
  const dir = join(KIT, "rounds", round);
  mkdirSync(dir, { recursive: true });
  const log = join(dir, "shoot.log");
  say(`round ${round}: shooting ${biomes.join(", ")}${film ? " with films" : ""} on the worker${dirty ? " (uncommitted changes as a patch)" : ""}; log ${log}`);
  const fd = openSync(log, "w");
  const args = ["xtask", "remote-render", "--gpu", ...(dirty ? ["--dirty"] : []), "--minutes", film ? "90" : "45", "--collect", OUT, "--", "sh", "-c", script(biomes, film)];
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
  h.rounds[round] = rnd;
  saveHistory(h);
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
    e = { ...e, delta: d, share, changed: isChanged(d, share) };
    keptInfo = { round: kept.round, delta: d, share };
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

function makeFilm(b: string, out: string, dir: string, say: (l: string) => void): string | undefined {
  const mp4 = join(dir, `${b}.mp4`);
  const r = spawnSync("ffmpeg", ["-loglevel", "error", "-y", "-framerate", "30", "-i", join(out, `film-${b}`, "f-%04d.jpg"), "-c:v", "libx264", "-crf", "18", "-pix_fmt", "yuv420p", mp4], { encoding: "utf8" });
  if (r.status === 0) return mp4;
  say(`✗ ${b}: its film's frames did not make a film (${r.stderr.trim().split("\n").pop()}); its run log ${join(out, `film-${b}.txt`)}`);
}

function record(argv: string[], say: (l: string) => void) {
  const [round, biome, what, ...rest] = argv;
  const h = loadHistory(say);
  const e = h.rounds[round]?.biomes[biome];
  if (!e) fail(`no biome ${biome} in round ${round}: \`node spike/iso/loop.ts history\` lists the rounds and their biomes`);
  if (what === "score") {
    const overall = Number(rest[0]);
    if (!(overall >= 0 && overall <= 10)) fail(`a score is a number of 10, not "${rest[0]}": record ${round} ${biome} score 5.5 "<gaps>"`);
    e!.score = { overall, gaps: rest.slice(1).join(" ") };
    say(`${biome} ${round}: score ${overall} recorded`);
  } else if (what === "pair") {
    const p = e!.pairs.find((x) => !x.winner);
    if (!p) fail(`${biome} in ${round} has no pair waiting for an answer (${e!.changed === false ? "it tied its kept still unchanged" : "each is answered"})`);
    const margin = rest[1] as Margin | undefined;
    if (rest[0]?.toUpperCase() !== "SAME" && !["slight", "clear", "strong"].includes(margin ?? "")) fail(`a pair's answer needs its margin: record ${round} ${biome} pair <A|B|same> <slight|clear|strong>`);
    p!.winner = winnerOf(rest[0] ?? "", p!.a);
    p!.margin = margin;
    const n = e!.pairs.length;
    say(`${biome} ${round}: pair ${n} recorded`);
    if (n === 1 && p!.winner === "kept" && margin === "slight") {
      const kept = h.kept[biome];
      const q = preparePair(round, biome, e!, kept, refsOf(biome).dir);
      e!.pairs.push(q);
      say(`a slight answer against the change: a second pair, the order swapped, confirms it; brief ${q.brief}`);
    }
  } else fail(`record takes "pair" or "score": record <round> <biome> pair <A|B|same> <slight|clear|strong>, or record <round> <biome> score <overall> "<gaps>"`);
  saveHistory(h);
}

function judgeVerdict(argv: string[], say: (l: string) => void) {
  const round = argv[0];
  const h = loadHistory(say);
  const rnd = h.rounds[round];
  if (!rnd) fail(`no round ${round}: \`node spike/iso/loop.ts history\` lists them`);
  const v = verdict(rnd!.biomes);
  for (const [b, o] of Object.entries(v.out)) {
    const s = rnd!.biomes[b].score;
    say(`${b}: ${o}${s ? `, score ${s.overall}` : ""}`);
  }
  if (!v.result) {
    const briefs = v.pending.map((b) => rnd!.biomes[b].pairs.find((p) => !p.winner)?.brief).filter(Boolean);
    fail(`no verdict yet: ${v.pending.join(", ")} wait for a pair's answer; run ${briefs.join(", ")} and record each`);
  }
  if (v.result === "KEEP") {
    mkdirSync(join(KIT, "kept"), { recursive: true });
    for (const [b, e] of Object.entries(rnd!.biomes)) {
      const still = join(KIT, "kept", `${b}.png`);
      copyFileSync(e.still, still);
      h.kept[b] = { round, still, stats: e.stats };
    }
    say(`KEEP ${round}: no biome lost; its stills are the kept ones now (${join(KIT, "kept")})`);
  } else say(`REVERT ${round}: ${v.losers.join(", ")} lost; undo the change shot in it (${rnd!.rev}${rnd!.dirty ? " with uncommitted changes" : ""}: git revert, never a force-push); the kept stills stay`);
  rnd!.verdict = { result: v.result!, losers: v.losers, at: new Date().toISOString() };
  saveHistory(h);
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
  rnd!.verdict = { result: "KEEP", losers: [], at: new Date().toISOString(), rebased: why };
  saveHistory(h);
  say(`KEEP ${round} unjudged as the kept stills of ${Object.keys(rnd!.biomes).join(", ")}: ${why}`);
}

function history(say: (l: string) => void) {
  const h = loadHistory(say);
  const names = [...new Set([...Object.keys(h.kept), ...Object.values(h.rounds).flatMap((r) => Object.keys(r.biomes))])].sort();
  for (const b of names) {
    const cells = Object.entries(h.rounds).filter(([, r]) => r.biomes[b]).map(([name, r]) => {
      const e = r.biomes[b];
      const o = r.verdict?.rebased ? "unjudged" : outcome(e);
      const p = e.pairs.filter((x) => x.winner).map((x) => `${x.winner}${x.margin ? " " + x.margin : ""}`).join("/");
      const v = r.verdict ? ` ${r.verdict.result}${r.verdict.rebased ? " (rebased)" : ""}` : "";
      return `${name} ${o}${p ? ` (${p})` : ""}${e.score ? ` ${e.score.overall}` : ""}${v}`;
    });
    say(`${b}: kept ${h.kept[b]?.round ?? "none"} · ${cells.join(" · ") || "no rounds"}`);
  }
}

const USAGE = `usage: node spike/iso/loop.ts
  shoot [--biomes a,b] [--film] [--dirty]   every biome's still on the worker, measured and judged against its kept one
  record <round> <biome> pair <A|B|same> <slight|clear|strong>
  record <round> <biome> score <overall> "<gaps>"
  verdict <round>                           KEEP if no biome lost, else REVERT
  rebase <round> "<why>"                    its stills kept unjudged, when how stills are shot changed, never the look
  history                                   each biome's rounds, pairs, scores and kept still`;

export function main(argv: string[], say = (l: string) => console.log(l)): number {
  const [cmd, ...rest] = argv;
  try {
    if (cmd === "shoot") shoot(rest, say);
    else if (cmd === "record") record(rest, say);
    else if (cmd === "verdict") judgeVerdict(rest, say);
    else if (cmd === "rebase") rebase(rest, say);
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
