// Blind visual judging by fresh subagent judges, without the position bias of side-by-side scoring (D472,
// tools/review/README.md). It writes each judge's folder and prompt and spawns nothing: the calling agent runs one
// fresh subagent a judge, then scores the folder.
//   node tools/review/judge.ts --brief <file> --set <dir> --out <dir> --judges <n> [--anchors <dir>] [--builds a,b]
//                              [--choice a:b] [--seed <n>]
//   node tools/review/judge.ts --score <out> [--compare a:b,a+b:c]
// A set is <set>/<build>/<view>.{png,jpg}, the same views in every build. Absolute (the default): <n> judges a build,
// each shown the anchors and ONE build's views, scoring each 0-10; a build's score is its judges' mean. Forced choice
// (--choice a:b): <n> judges, each shown a|b side by side per view, half with a on the left; they pick a side.
// Anchors: <anchors>/anchors.json, [{ "file": "rdr2.png", "score": 9, "what": "..." }], shown first at fixed scores.
// An absolute judge writes jNN/scores.txt, then jNN/faults.txt (its three worst faults an image), which --score
// gathers into <out>/faults.txt by build and view; the faults never enter the statistics.
import { copyFileSync, existsSync, mkdirSync, readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { basename, extname, join, resolve } from "node:path";
import { type Picture, readPng, writePng } from "../sandbox/png.ts";
import { binomialP, detectable, judgesNeeded, mean, permutationP, pooledSd, rng, sd, welch, wilson } from "./stats.ts";

export type Anchor = { file: string; score: number; what: string };
export type Judge = { id: string; build?: string; left?: string; right?: string };
export type Key = {
  mode: "absolute" | "choice";
  brief: string;
  set: string;
  builds: string[];
  views: string[];
  anchors: Anchor[];
  judges: Judge[];
};

const IMAGE = /\.(png|jpe?g)$/i;
const SCORES = "scores.txt";
/// Each absolute judge's worst faults an image, kept apart from its scores so they never enter the statistics.
const FAULTS = "faults.txt";
const FAULTS_EACH = 3;

function shuffle<T>(xs: T[], r: () => number): T[] {
  for (let k = xs.length - 1; k > 0; k--) {
    const j = Math.floor(r() * (k + 1));
    [xs[k], xs[j]] = [xs[j], xs[k]];
  }
  return xs;
}

/// The builds of a set and the views every one of them has.
export function readSet(set: string, only?: string[]): { builds: string[]; views: string[] } {
  const builds = only ?? readdirSync(set).filter((d) => statSync(join(set, d)).isDirectory()).sort();
  if (builds.length === 0) throw new Error(`${set}: no build folders`);
  const viewsOf = (b: string) => readdirSync(join(set, b)).filter((f) => IMAGE.test(f)).sort();
  const views = viewsOf(builds[0]);
  for (const b of builds)
    if (viewsOf(b).join() !== views.join()) throw new Error(`${set}/${b}: views ${viewsOf(b).join(" ")}, not ${views.join(" ")}`);
  if (views.length === 0) throw new Error(`${set}/${builds[0]}: no images`);
  return { builds, views };
}

/// Two pictures side by side, a grey bar between, the shorter padded with black at its foot.
export function sideBySide(left: Picture, right: Picture, bar = 24): Picture {
  const width = left.width + bar + right.width;
  const height = Math.max(left.height, right.height);
  const rgba = new Uint8Array(width * height * 4);
  for (let i = 0; i < width * height; i++) rgba[i * 4 + 3] = 255;
  for (let y = 0; y < height; y++)
    for (let x = left.width; x < left.width + bar; x++) rgba.fill(128, (y * width + x) * 4, (y * width + x) * 4 + 3);
  const put = (p: Picture, x0: number) => {
    for (let y = 0; y < p.height; y++)
      rgba.set(p.rgba.subarray(y * p.width * 4, (y + 1) * p.width * 4), (y * width + x0) * 4);
  };
  put(left, 0);
  put(right, left.width + bar);
  return { width, height, rgba };
}

function anchorLines(dir: string, anchors: Anchor[], fixed: boolean): string[] {
  return anchors.map((a, i) => {
    const name = `anchor-${i + 1}${extname(a.file)}`;
    return fixed ? `- \`${name}\`: ${a.what}. Its score is fixed at ${a.score}.` : `- \`${name}\`: ${a.what} (for reference only).`;
  });
}

/// The prompt a judge follows: the brief, the files in reading order and the one output it writes.
export function prompt(key: Key, judge: Judge, dir: string, brief: string): string {
  const out = join(dir, SCORES);
  const scored = key.views.map((v) => `\`${v}\``).join(", ");
  const shared = [
    "Do not open any file or folder but the ones named here, and do not list folders: other judges' files are near.",
    "Read every image once, one image per Read call, in the order given, and note each score or choice before reading the next.",
  ];
  if (key.mode === "absolute") {
    const scale = key.anchors.length
      ? "The anchors are the scale's fixed points: you do not score them; they tell you where their scores sit on the 0-10 scale."
      : "";
    return [
      "# Blind scoring, one judge",
      "",
      brief.trim(),
      "",
      "## The images",
      `All in \`${dir}\`. First the anchors, then the images you score:`,
      ...anchorLines(dir, key.anchors, true),
      `- ${scored}: the images you score.`,
      "",
      scale,
      "Score each image 0-10 with one decimal (e.g. 4.3), by the brief, on its own merits against the anchors.",
      ...shared,
      "",
      "## Output",
      `First write \`${out}\` as plain text, one line per scored image, exactly \`<image> <score>\` (e.g. \`${key.views[0]} 4.3\`), ${key.views.length} lines, nothing else: scores only, no notes.`,
      `Then write \`${join(dir, FAULTS)}\` as plain text: for each scored image, its ${FAULTS_EACH} worst faults by the brief, worst first, one line each, exactly \`<image> <fault>\` (e.g. \`${key.views[0]} the shadows are flat grey\`), ${key.views.length * FAULTS_EACH} lines, nothing else. A fault says what is wrong and where in the image, in a short sentence; it does not change a score.`,
      `Then reply with one line only: \`done ${key.views.length}\`.`,
      "",
    ].join("\n");
  }
  return [
    "# Blind forced choice, one judge",
    "",
    brief.trim(),
    "",
    "## The images",
    `All in \`${dir}\`.`,
    ...anchorLines(dir, key.anchors, false),
    `- ${scored}: each shows two versions side by side, LEFT and RIGHT of a grey bar, the same view of each.`,
    "",
    "For each image choose the side that is better by the brief. You must choose one, even where they look alike: then the one you would ship. Then make one overall choice over all the images.",
    ...shared,
    "",
    "## Output",
    `Write \`${out}\` as plain text, one line per image, exactly \`<image> L\` or \`<image> R\`, then a last line \`overall L\` or \`overall R\`, ${key.views.length + 1} lines, nothing else. Then reply with one line only: \`done ${key.views.length + 1}\`. Choices only: no notes, no review.`,
    "",
  ].join("\n");
}

export type MakeOptions = {
  brief: string;
  set: string;
  out: string;
  judges: number;
  anchors?: string;
  builds?: string[];
  choice?: [string, string];
  seed?: number;
  /// Add the judges to the panel already in `out`, with ids after its own (the same set, views and mode).
  add?: boolean;
};

/// Writes a panel: a folder and prompt a judge, and key.json (who saw what), which no judge is shown.
export function make(o: MakeOptions): Key {
  const set = resolve(o.set);
  const out = resolve(o.out);
  const old: Key | undefined = existsSync(join(out, "key.json")) ? JSON.parse(readFileSync(join(out, "key.json"), "utf8")) : undefined;
  if (old && !o.add) throw new Error(`${out}: a panel is already there; a fresh panel needs a fresh folder, or --add to add judges`);
  if (!old && o.add) throw new Error(`${out}: no panel to add judges to`);
  const { builds, views } = readSet(set, o.choice ?? o.builds ?? old?.builds);
  if (old && (old.set !== set || old.views.join() !== views.join() || (old.mode === "choice") !== Boolean(o.choice)))
    throw new Error(`${out}: a ${old.mode} panel of ${old.set}, views ${old.views.join(" ")}`);
  const anchors: Anchor[] = o.anchors ? JSON.parse(readFileSync(join(o.anchors, "anchors.json"), "utf8")) : [];
  const before = old?.judges.length ?? 0;
  const r = rng((o.seed ?? 1) + before * 7919);
  let slots: Judge[];
  if (o.choice) {
    const [a, b] = o.choice;
    slots = Array.from({ length: o.judges }, (_, i) => (i % 2 === 0 ? { id: "", left: a, right: b } : { id: "", left: b, right: a }));
  } else slots = builds.flatMap((build) => Array.from({ length: o.judges }, () => ({ id: "", build })));
  shuffle(slots, r);
  slots.forEach((s, i) => (s.id = `j${String(before + i + 1).padStart(2, "0")}`));
  const key: Key = old
    ? { ...old, builds: [...new Set([...old.builds, ...builds])], judges: [...old.judges, ...slots] }
    : { mode: o.choice ? "choice" : "absolute", brief: resolve(o.brief), set, builds, views, anchors, judges: slots };
  const brief = readFileSync(o.brief, "utf8");
  const made = new Map<string, Picture>();
  for (const j of slots) {
    const dir = join(out, j.id);
    mkdirSync(dir, { recursive: true });
    anchors.forEach((a, i) => copyFileSync(join(o.anchors!, a.file), join(dir, `anchor-${i + 1}${extname(a.file)}`)));
    for (const v of views) {
      if (j.build) copyFileSync(join(set, j.build, v), join(dir, v));
      else {
        const name = `${j.left}|${j.right}|${v}`;
        if (!made.has(name)) made.set(name, sideBySide(readPng(join(set, j.left!, v)), readPng(join(set, j.right!, v))));
        writePng(join(dir, v), made.get(name)!);
      }
    }
    writeFileSync(join(dir, "prompt.md"), prompt(key, j, dir, brief));
  }
  writeFileSync(join(out, "key.json"), JSON.stringify(key, null, 1) + "\n");
  writeFileSync(join(out, "judges.txt"), slots.map((j) => join(out, j.id, "prompt.md")).join("\n") + "\n");
  return key;
}

/// A judge's scores.txt: image -> value (a number, or L/R), or nothing when it is missing or malformed.
export function readScores(file: string, views: string[], choice: boolean): Map<string, string> | undefined {
  if (!existsSync(file)) return undefined;
  const got = new Map<string, string>();
  for (const line of readFileSync(file, "utf8").split("\n")) {
    const m = line.trim().replace(/`/g, "").match(/^(\S+)\s+(\S+)$/);
    if (!m) continue;
    // A judge may drop the extension: `dusk 4.3` for `dusk.png`.
    const view = views.find((v) => v === m[1] || basename(v, extname(v)) === m[1]) ?? m[1];
    got.set(view, m[2].toUpperCase());
  }
  const want = choice ? [...views, "overall"] : views;
  const ok = (v: string | undefined) => (choice ? v === "L" || v === "R" : v !== undefined && Number.isFinite(Number(v)));
  return want.every((w) => ok(got.get(w))) ? got : undefined;
}

/// A judge's faults.txt: image -> its faults in the judge's order (lines naming no scored image are dropped).
export function readFaults(file: string, views: string[]): Map<string, string[]> {
  const got = new Map<string, string[]>();
  if (!existsSync(file)) return got;
  for (const line of readFileSync(file, "utf8").split("\n")) {
    const m = line.trim().replace(/`/g, "").match(/^(\S+?):?\s+(.+)$/);
    if (!m) continue;
    const view = views.find((v) => v === m[1] || basename(v, extname(v)) === m[1]);
    if (view) got.set(view, [...(got.get(view) ?? []), m[2].trim()]);
  }
  return got;
}

/// Every judge's faults, by build then view, each line tagged with its judge and that judge's score of the view.
export function gatherFaults(out: string): string[] {
  const key: Key = JSON.parse(readFileSync(join(out, "key.json"), "utf8"));
  if (key.mode !== "absolute") return [];
  const lines: string[] = [];
  const none: string[] = [];
  for (const b of key.builds) {
    const judges = key.judges.filter((j) => j.build === b);
    for (const v of key.views) {
      lines.push(`${b} ${v}`);
      for (const j of judges) {
        const said = readFaults(join(out, j.id, FAULTS), key.views).get(v) ?? [];
        const s = readScores(join(out, j.id, SCORES), key.views, false)?.get(v) ?? "-";
        for (const f of said) lines.push(`  ${j.id} ${s.padEnd(4)} ${f}`);
      }
    }
    for (const j of judges) if (readFaults(join(out, j.id, FAULTS), key.views).size === 0) none.push(j.id);
  }
  if (none.length) lines.push(`no faults (${none.length}): ${none.join(" ")}`);
  return lines;
}

const f2 = (x: number) => (x >= 0 ? "+" : "") + x.toFixed(2);
const pct = (x: number) => `${Math.round(x * 100)}%`;

/// The scored panel as text lines: per build, per comparison, and what was missing.
export function score(out: string, compare?: string[][][]): string[] {
  const key: Key = JSON.parse(readFileSync(join(out, "key.json"), "utf8"));
  const lines: string[] = [];
  const missing: string[] = [];
  const read = key.judges.map((j) => {
    const s = readScores(join(out, j.id, SCORES), key.views, key.mode === "choice");
    if (!s) missing.push(j.id);
    return { j, s };
  });
  const done = read.filter((x) => x.s);
  if (key.mode === "absolute") {
    const per = new Map<string, number[]>();
    const perView = new Map<string, number[]>();
    for (const { j, s } of done) {
      const vals = key.views.map((v) => Number(s!.get(v)));
      per.set(j.build!, [...(per.get(j.build!) ?? []), mean(vals)]);
      key.views.forEach((v, i) => perView.set(`${j.build} ${v}`, [...(perView.get(`${j.build} ${v}`) ?? []), vals[i]]));
    }
    const views = key.views.map((v) => basename(v, extname(v)));
    lines.push(`build        n  mean  sd    ${views.map((v) => v.padEnd(6)).join("")}`);
    for (const b of key.builds) {
      const xs = per.get(b) ?? [];
      const vs = key.views.map((v) => (perView.get(`${b} ${v}`) ? mean(perView.get(`${b} ${v}`)!).toFixed(2).padEnd(6) : "-     "));
      lines.push(`${b.padEnd(12)} ${String(xs.length).padStart(2)}  ${xs.length ? mean(xs).toFixed(2) : "-   "}  ${sd(xs).toFixed(2)}  ${vs.join("")}`);
    }
    const pairs = compare ?? key.builds.slice(1).map((b) => [[key.builds[0]], [b]]);
    for (const [ga, gb] of pairs) {
      const a = ga.flatMap((b) => per.get(b) ?? []);
      const b = gb.flatMap((x) => per.get(x) ?? []);
      if (a.length < 2 || b.length < 2) {
        lines.push(`${gb.join("+")} - ${ga.join("+")}: too few judges`);
        continue;
      }
      const { diff, lo, hi, p } = welch(a, b);
      const perm = permutationP(a, b);
      const s = pooledSd(a, b);
      const n = Math.min(a.length, b.length);
      const verdict = lo > 0 ? `${gb.join("+")} better` : hi < 0 ? `${ga.join("+")} better` : "no difference shown";
      lines.push(
        `${gb.join("+")} - ${ga.join("+")}: ${f2(diff)} [${f2(lo)}, ${f2(hi)}] p ${p.toFixed(3)} (permutation ${perm.toFixed(3)}) -> ${verdict}; ` +
          `sd ${s.toFixed(2)}, n ${a.length}/${b.length} shows ${detectable(s, n).toFixed(2)}, 0.25 needs ${judgesNeeded(Math.max(s, 0.05), 0.25)} a build`,
      );
    }
  } else {
    const [a, b] = [key.judges[0].left!, key.judges[0].right!].sort();
    const picks = done.map(({ j, s }) => ({ j, side: s!.get("overall")!, views: key.views.map((v) => s!.get(v)!) }));
    const chose = (j: Judge, side: string) => (side === "L" ? j.left! : j.right!);
    const n = picks.length;
    const kb = picks.filter((p) => chose(p.j, p.side) === b).length;
    const kl = picks.filter((p) => p.side === "L").length;
    const bLeft = picks.filter((p) => p.j.left === b);
    const bRight = picks.filter((p) => p.j.right === b);
    const rate = (xs: typeof picks) => (xs.length ? xs.filter((p) => chose(p.j, p.side) === b).length / xs.length : NaN);
    const [lo, hi] = wilson(kb, n);
    const [llo, lhi] = wilson(kl, n);
    const views = picks.flatMap((p) => p.views.map((side) => ({ j: p.j, side })));
    const vb = views.filter((v) => chose(v.j, v.side) === b).length;
    const vl = views.filter((v) => v.side === "L").length;
    lines.push(`${n} judges, ${bLeft.length} with ${b} on the left`);
    lines.push(`${b} chosen ${kb}/${n} = ${pct(kb / n)} [${pct(lo)}, ${pct(hi)}] p ${binomialP(kb, n).toFixed(3)} (over ${a})`);
    lines.push(`left chosen ${kl}/${n} = ${pct(kl / n)} [${pct(llo)}, ${pct(lhi)}] p ${binomialP(kl, n).toFixed(3)} (the side bias)`);
    lines.push(`${b} chosen on the left ${pct(rate(bLeft))}, on the right ${pct(rate(bRight))}; order-free ${pct((rate(bLeft) + rate(bRight)) / 2)}`);
    lines.push(`per view: ${b} ${vb}/${views.length}, left ${vl}/${views.length}`);
  }
  if (missing.length) lines.push(`missing or malformed (${missing.length}): ${missing.join(" ")}`);
  return lines;
}

const FLAGS = new Set(["add"]);

function args(argv: string[]): Map<string, string> {
  const m = new Map<string, string>();
  for (let i = 0; i < argv.length; i++) {
    if (!argv[i].startsWith("--")) throw new Error(`unexpected ${argv[i]}`);
    const name = argv[i].slice(2);
    m.set(name, FLAGS.has(name) ? "" : (argv[++i] ?? ""));
  }
  return m;
}

function main() {
  const a = args(process.argv.slice(2));
  if (a.has("score")) {
    const out = a.get("score")!;
    const compare = a.get("compare")?.split(",").map((p) => p.split(":").map((g) => g.split("+")));
    const lines = score(out, compare);
    writeFileSync(join(out, "score.txt"), lines.join("\n") + "\n");
    console.log(lines.join("\n"));
    const faults = gatherFaults(out);
    if (faults.length) {
      writeFileSync(join(out, FAULTS), faults.join("\n") + "\n");
      console.log(`faults by build and view: ${join(out, FAULTS)}`);
    }
    return;
  }
  for (const k of ["brief", "set", "out", "judges"]) if (!a.get(k)) throw new Error(`--${k} is needed (see the head of tools/review/judge.ts)`);
  const choice = a.get("choice")?.split(":") as [string, string] | undefined;
  if (choice && choice.length !== 2) throw new Error("--choice a:b names two builds");
  const key = make({
    brief: a.get("brief")!,
    set: a.get("set")!,
    out: a.get("out")!,
    judges: Number(a.get("judges")),
    anchors: a.get("anchors"),
    builds: a.get("builds")?.split(","),
    choice,
    seed: a.has("seed") ? Number(a.get("seed")) : undefined,
    add: a.has("add"),
  });
  const out = resolve(a.get("out")!);
  const what = key.mode === "choice" ? `forced choice ${choice!.join(" vs ")}` : `absolute, ${key.builds.length} builds x ${a.get("judges")}`;
  console.log(`✓ ${key.judges.length} judges (${what}) in ${out}; prompts listed in ${join(out, "judges.txt")}`);
  console.log(`  run each as a fresh subagent: "Read <prompt.md> and follow it exactly."; then --score ${out}`);
}

if (import.meta.main) {
  try {
    main();
  } catch (e) {
    console.error(`✗ ${(e as Error).message}`);
    process.exit(1);
  }
}
