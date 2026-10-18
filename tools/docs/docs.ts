// Checks the repository's docs, run by `cargo xtask check`: links and anchors resolve, every
// doc, note, spike and tool script is in its folder's index, every decision file
// (docs/decisions/Dnnn.md) is well formed, with every decision a tracked file cites among them,
// every test a contract page cites as its proof exists, a page's part rendered from data
// matches it, and the guides name only real `cargo xtask` tasks and skills only real paths (D396).
// Prints each fault; exits 1 if there are any.
//
//   node tools/docs/docs.ts
//   node tools/docs/docs.ts --next         the next free decision number, here and on origin/main (`cargo xtask decision`)
//   node tools/docs/docs.ts --show D28 …   those decisions in full, with their log lines (`cargo xtask decision D28 …`)
//   node tools/docs/docs.ts --render       writes the parts of pages rendered from data (the voxel matrix, the decisions index)

import { execFileSync } from "node:child_process";
import { existsSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { basename, dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { ORDER } from "./merge.ts";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..", "..");

/** A heading's anchor as GitHub makes it, numbered when a heading repeats. */
export function anchors(markdown: string): Set<string> {
  const seen = new Map<string, number>();
  const out = new Set<string>();
  for (const [, text] of withoutFences(markdown).matchAll(/^#{1,6} +(.+?) *#*$/gm)) {
    const plain = text.replace(/\[([^\]]*)\]\([^)]*\)/g, "$1").replace(/[`*]/g, "");
    const base = plain.toLowerCase().replace(/[^\p{L}\p{N}\- _]/gu, "").replace(/ /g, "-");
    const n = seen.get(base) ?? 0;
    seen.set(base, n + 1);
    out.add(n ? `${base}-${n}` : base);
  }
  return out;
}

function withoutFences(markdown: string): string {
  return markdown.replace(/^```[\s\S]*?^```/gm, "");
}

function withoutCode(markdown: string): string {
  return withoutFences(markdown).replace(/`[^`\n]*`/g, "");
}

/** Relative links in a file that lead nowhere, or to a heading that isn't there. */
export function brokenLinks(file: string, markdown: string, read: (path: string) => string | undefined): string[] {
  const faults: string[] = [];
  for (const [, target] of withoutCode(markdown).matchAll(/\]\(<?([^)\s>]+)>?(?: "[^"]*")?\)/g)) {
    if (/^[a-z]+:/i.test(target)) continue;
    const [path, anchor] = target.split("#");
    const resolved = path ? join(dirname(file), decodeURIComponent(path)) : file;
    const text = read(resolved);
    if (text === undefined) faults.push(`${file}: links to ${target}, which does not exist`);
    else if (anchor && resolved.endsWith(".md") && !anchors(text).has(anchor.toLowerCase()))
      faults.push(`${file}: links to ${target}, which has no such heading`);
  }
  return faults;
}

const STATUS = /^(Decided|Leaning|Open|Rejected|Superseded by D\d+(, D\d+)*)$/;

/**
 * Design calls an agent marked Decided before D412 made that a fault, most with the developers'
 * leave quoted in their Why. They are the developers' to confirm (By Developers) or reopen
 * (Leaning); this list only shrinks.
 */
const SETTLED_BY_AGENTS = ["D171", "D178", "D194", "D229", "D233", "D252", "D253", "D298", "D313", "D364"];

/** The index's areas, in order, each a heading of docs/decisions.md and a decision's Area. */
export const AREAS = [
  { title: "The game as a whole", intro: "What we are building, its tenets, scope and players." },
  { title: "Worlds and realms", intro: "Ground, water, weather, seasons, life and how realms join." },
  { title: "The player and survival", intro: "Needs, death, the body, the map and options." },
  { title: "Items and crafting", intro: "Equipment, packs, tools, stations and icons." },
  { title: "Lore, magic, combat and growth", intro: "Essences, ranks, skills, fighting, creatures and threats." },
  { title: "People and the tribe", intro: "NPCs, factions, claims, recruits and outposts." },
  { title: "Building", intro: "How structures are planned, built, drawn and coded." },
  { title: "Art, characters and sound", intro: "The look, models, bodies and gear, textures and audio." },
  { title: "Engine and code", intro: "Rust and Bevy, terrain, shared state, networking and the game's model." },
  { title: "Tools and workflow", intro: "Checks, developer tools, languages and the repository." },
];
const GONE = { title: "Superseded and rejected", intro: "Kept so their numbers still resolve. They no longer bind anything." };

/** One decision, as its file docs/decisions/Dnnn.md holds it. */
export interface Decision {
  id: string;
  topic: string;
  area: string;
  owner: string;
  by: string;
  status: string;
  decision: string;
  why: string;
}

/**
 * A decision file read: line 1 `# Dnnn: Topic`, then `Area: … · Owner: … · By: … · Status: …`,
 * then the decision, then optionally `## Why` and the reason. Its faults, if it isn't one.
 */
export function parseDecision(file: string, text: string): { decision?: Decision; faults: string[] } {
  const faults: string[] = [];
  const fault = (what: string) => faults.push(`${file}: ${what}`);
  const shape = text.match(/^# (D\d+): (.+)\n\nArea: (.+) · Owner: (.+) · By: (.+) · Status: (.+)\n\n([\s\S]+?)\n(?:\n## Why\n\n([\s\S]+?)\n)?$/);
  if (!shape) {
    fault("is not `# Dnnn: Topic`, a blank line, `Area: … · Owner: … · By: … · Status: …`, a blank line, the decision, then optionally `## Why` and the reason");
    return { faults };
  }
  const [, id, topic, area, owner, by, status, decision, why = ""] = shape;
  if (basename(file) !== `${id}.md`) fault(`is headed ${id}; a decision's file is named for its number`);
  if (owner !== "Design" && owner !== "Tech") fault(`has Owner "${owner}", not Design or Tech`);
  if (!STATUS.test(status)) fault(`has Status "${status}": Decided, Leaning, Open, Rejected or Superseded by Dn`);
  const nobody = /^(Open|Rejected|Superseded)/.test(status);
  if (nobody ? by !== "—" : by !== "Developers" && by !== "Agent")
    fault(`has By "${by}": Developers or Agent, or — when Open, Rejected or Superseded`);
  // A design call is the developers' (D412): an agent's stays Leaning until they settle it.
  if (owner === "Design" && status === "Decided" && by !== "Developers" && !SETTLED_BY_AGENTS.includes(id))
    fault(`is a design call Decided By ${by}: only the developers settle one; make it Leaning and ask them (D412)`);
  const gone = /^(Rejected|Superseded)/.test(status);
  if (!AREAS.some((a) => a.title === area) && !(gone && area === "—"))
    fault(`has Area "${area}": one of docs/decisions.md's headings, or — when Rejected or Superseded`);
  return { decision: { id, topic, area, owner, by, status, decision, why }, faults };
}

/** The decision files, from their names and texts: the decisions, and the faults of those that aren't. */
export function readDecisions(files: { name: string; text: string }[]): { decisions: Decision[]; faults: string[] } {
  const decisions: Decision[] = [];
  const faults: string[] = [];
  for (const { name, text } of files) {
    const file = `docs/decisions/${name}`;
    if (!/^D\d+\.md$/.test(name)) {
      faults.push(`${file}: is not a decision; the folder holds only Dnnn.md files`);
      continue;
    }
    const read = parseDecision(file, text);
    faults.push(...read.faults);
    if (read.decision) decisions.push(read.decision);
  }
  return { decisions, faults };
}

/**
 * A decision's gist for the index: its first sentence (after one that only points at other
 * decisions, as "Amends D270."), or when that is long, as much of it as ends at a clause (a
 * comma, colon, semicolon, dash or bracket outside code) or else a word, links as their text.
 */
export function gist(decision: string, limit = GIST): string {
  let text = decision.split("\n\n")[0].replace(/\[([^\]]*)\]\([^)]*\)/g, "$1");
  const first = /^.+?[.!?](?=\s+[A-Z*`"(]|$)/;
  const pointer = text.match(first)?.[0] ?? "";
  if (pointer.length < 40 && /\bD\d+\b/.test(pointer) && pointer.length < text.length) text = text.slice(pointer.length).trimStart();
  let sentence = text.match(first)?.[0] ?? text;
  // A sentence of a word or two ("None.") takes the next with it when both fit.
  while (sentence.length < 24 && sentence.length < text.length) {
    const more = text.slice(sentence.length).match(/^\s*.+?[.!?](?=\s+[A-Z*`"(]|$)/)?.[0];
    if (!more || sentence.length + more.length > limit) break;
    sentence += more;
  }
  if (sentence.length <= limit) return sentence;
  let depth = 0;
  let code = false;
  let cut = -1;
  for (let i = 0; i < limit; i++) {
    const c = sentence[i];
    if (c === "`") code = !code;
    else if (code) continue;
    else if (c === "(") {
      if (depth === 0 && i > 0) cut = i;
      depth++;
    } else if (c === ")") depth--;
    else if (depth === 0 && (/[,;:]/.test(c) || sentence.startsWith(" —", i)) && sentence[i + 1] === " ") cut = i;
  }
  if (cut < limit * 0.6) cut = sentence.lastIndexOf(" ", limit);
  let short = sentence.slice(0, cut).replace(/[\s,;:—]+$/, "");
  if ((short.match(/`/g) ?? []).length % 2) short += "`";
  if ((short.match(/(?<!\*)\*(?!\*)/g) ?? []).length % 2) short += "*";
  if ((short.match(/\*\*/g) ?? []).length % 2) short += "**";
  return `${short}…`;
}

/** How long a gist may be, in characters: short enough to keep the index near 60 KB (D389). */
const GIST = 56;

/** A cell's text with its pipes escaped, so the row keeps its cells. */
const cell = (text: string) => text.replace(/\|/g, "\\|");

/** The index of decisions.md: per area its heading and a row per decision, the gone ones last. */
export function renderIndex(decisions: Decision[]): string {
  const out: string[] = [];
  const number = (d: Decision) => Number(d.id.slice(1));
  const sorted = [...decisions].sort((a, b) => number(a) - number(b));
  const gone = (d: Decision) => /^(Rejected|Superseded)/.test(d.status);
  for (const area of [...AREAS, GONE]) {
    const rows = sorted.filter((d) => (area === GONE ? gone(d) : !gone(d) && d.area === area.title));
    out.push(`## ${area.title}`, "", area.intro, "", "| # | Topic | Owner | By | Status | Gist |", "|---|---|---|---|---|---|");
    for (const d of rows)
      out.push(`| [${d.id}](decisions/${d.id}.md) | ${cell(d.topic)} | ${d.owner} | ${d.by} | ${d.status} | ${cell(gist(d.decision))} |`);
    out.push("");
  }
  return out.join("\n");
}

/** What a decision fault or a stale index tells someone who edited docs/decisions.md by hand. */
const EDIT_THE_FILE = "decisions live in docs/decisions/Dnnn.md; edit that and run `node tools/docs/docs.ts --render`";

/** Table rows in docs/decisions.md outside its rendered index: someone wrote a decision into the page. */
export function strayRows(markdown: string): string[] {
  const outside = markdown.replace(/<!-- rendered from [\s\S]*?<!-- end of rendered -->/, "");
  return outside.split("\n").some((line) => line.startsWith("|")) ? [`docs/decisions.md: has a table outside its index; ${EDIT_THE_FILE}`] : [];
}

/** The next free decision number: one past the highest in use. */
export function nextDecision(ids: Iterable<string>): number {
  let highest = 0;
  for (const id of ids) highest = Math.max(highest, Number(id.slice(1)));
  return highest + 1;
}

/** The tracked files whose citations of decisions are checked: docs, code, data and scripts. */
const CITES = /(\.(md|rs|ts|mjs|toml|json|ron|wgsl|wesl|py|sh|yml|yaml|txt)|Makefile)$/;

/** Decision numbers a text cites that have no file. */
export function unknownDecisions(file: string, text: string, ids: Set<string>): string[] {
  const missing = new Set([...text.matchAll(/\bD(\d{1,3})\b/g)].map((m) => `D${m[1]}`).filter((d) => !ids.has(d)));
  return [...missing].map((d) => `${file}: cites ${d}, which has no docs/decisions/${d}.md`);
}

/** Pages whose rows name the tests that prove them (D348): each test they name must exist. */
const PROOF_PAGES = ["docs/building-voxels.md"];

/**
 * The tests a page names that no Rust source defines: every name in backticks of four words
 * or more in snake case, as a test's is (a module's or a function's is shorter), and any cut
 * short with an ellipsis, which names nothing.
 */
export function missingTests(file: string, markdown: string, sources: string): string[] {
  const faults: string[] = [];
  const seen = new Set<string>();
  for (const [, name] of withoutFences(markdown).matchAll(/`([a-z][a-z0-9_]*…?)`/g)) {
    if (seen.has(name)) continue;
    seen.add(name);
    if (name.endsWith("…")) faults.push(`${file}: names a test cut short, ${name}`);
    else if (/^[a-z][a-z0-9]*(_[a-z0-9]+){3,}$/.test(name) && !new RegExp(`\\bfn ${name}\\s*\\(`).test(sources))
      faults.push(`${file}: names the test ${name}, which no crate defines`);
  }
  return faults;
}

/**
 * Pages whose tables are written from data (D348, D389): the page, how its part is written from
 * what `read` gives, and what to tell someone whose page differs from it.
 */
const RENDERED: { page: string; render: (read: (path: string) => string | undefined) => string; stale: string }[] = [
  {
    page: "docs/building-voxels.md",
    render: (read) => renderMatrix(JSON.parse(read("docs/building-voxels.json") ?? "")),
    stale: "differs from docs/building-voxels.json; edit the data, then run node tools/docs/docs.ts --render",
  },
  { page: "docs/decisions.md", render: () => renderIndex(decisionFiles().decisions), stale: EDIT_THE_FILE },
  { page: "research/README.md", render: () => renderNotes(noteFiles()), stale: "differs from the notes; run node tools/docs/docs.ts --render (D409)" },
];

/** The research notes on disk, read. */
function noteFiles(): { name: string; text: string }[] {
  const dir = join(ROOT, "research");
  return readdirSync(dir)
    .filter((name) => name.endsWith(".md") && name !== "README.md")
    .map((name) => ({ name, text: readFileSync(join(dir, name), "utf8") }));
}

const DATE = /\d{4}-\d{2}-\d{2}/;

/**
 * The research index (D409): a row a note, its title and the date it states (in its name, or
 * at its top), newest first. Rendered from the notes, so adding one never edits a shared row.
 */
export function renderNotes(notes: { name: string; text: string }[]): string {
  const rows = notes.map(({ name, text }) => {
    const title = text.match(/^# (.+)$/m)?.[1];
    const date = name.match(DATE)?.[0] ?? text.slice(0, 1000).match(DATE)?.[0];
    if (!title || !date) throw new Error(`research/${name} needs a \`# Title\` and its date at the top`);
    return `| [${cell(title)}](${name}) | ${date} |`;
  });
  return ["| Note | Dated |", "|---|---|", ...rows.sort(ORDER["research/README.md"]), ""].join("\n");
}

/** The decision files on disk, read. */
function decisionFiles(): { decisions: Decision[]; faults: string[] } {
  const dir = join(ROOT, "docs/decisions");
  const names = existsSync(dir) ? readdirSync(dir).sort() : [];
  return readDecisions(names.map((name) => ({ name, text: readFileSync(join(dir, name), "utf8") })));
}

/** Where a rendered part of a page begins and ends. */
const BEGIN = /^<!-- rendered from (\S+) by `node tools\/docs\/docs\.ts --render`: edit that, not this -->$/m;
const END = "<!-- end of rendered -->";

/** A contract's row, as its data file holds it. */
interface Row {
  junction: string;
  rule: string;
  code: string[];
  tests: string[];
  cases: string[];
  also?: string;
  status: string;
}

/**
 * A contract's matrix as markdown: a heading a section, its rows' junction, the rule that
 * closes it, the code that implements the rule, what proves it (its tests by module, then its
 * registered cases, then anything else) and its status.
 */
export function renderMatrix(data: { sections: { title: string; intro?: string; rows: Row[] }[] }): string {
  const out: string[] = [];
  for (const section of data.sections) {
    out.push(`### ${section.title}`, "");
    if (section.intro) out.push(section.intro, "");
    out.push("| Junction | What closes it | Code | Proved by | Status |", "|---|---|---|---|---|");
    for (const row of section.rows) {
      const modules = new Map<string, string[]>();
      for (const test of row.tests) {
        const at = test.lastIndexOf("::");
        const module = test.slice(0, at);
        modules.set(module, [...(modules.get(module) ?? []), test.slice(at + 2)]);
      }
      const proofs = [...modules].map(([module, names]) => `\`${module}\`: ${names.map((n) => `\`${n}\``).join(", ")}`);
      if (row.cases.length) proofs.push(`the checker's ${row.cases.map((c) => `*${c}*`).join(", ")}`);
      if (row.also) proofs.push(row.also);
      const code = row.code.map((c) => `\`${c}\``).join(", ");
      out.push(`| ${row.junction} | ${row.rule} | ${code} | ${proofs.join("; ") || "—"} | ${row.status} |`);
    }
    out.push("");
  }
  return out.join("\n");
}

/** A page with its rendered part written again from `render`, or undefined if it has none. */
export function rendered(markdown: string, render: (data: string) => string): string | undefined {
  const begin = markdown.match(BEGIN);
  const end = markdown.indexOf(END);
  if (!begin || begin.index === undefined || end < begin.index) return undefined;
  const from = begin.index + begin[0].length;
  return `${markdown.slice(0, from)}\n\n${render(begin[1])}\n${markdown.slice(end)}`;
}

/** Each rendered page written again from its data file; the faults, or the pages written. */
export function renderPages(read: (path: string) => string | undefined, write?: (path: string, text: string) => void): string[] {
  const faults: string[] = [];
  for (const { page, render, stale } of RENDERED) {
    const text = read(page) ?? "";
    let again: string | undefined;
    try {
      again = rendered(text, () => render(read));
    } catch (why) {
      faults.push(`${page}: its data does not read: ${why}`);
      continue;
    }
    if (again === undefined) faults.push(`${page}: has no rendered part (its markers are gone); ${stale}`);
    else if (again !== text) {
      if (write) write(page, again);
      else faults.push(`${page}: ${stale}`);
    }
  }
  return faults;
}

/** The tasks `cargo xtask` runs: the names in xtask/src/main.rs's `TASKS`, and `help`. */
export function xtaskTasks(main: string): Set<string> {
  const table = main.slice(main.indexOf("const TASKS"));
  return new Set(["help", ...[...table.matchAll(/^\s*name: "([^"]+)",$/gm)].map((m) => m[1])]);
}

/** Pages read by agents and developers for how to work, whose commands and paths must be real (D396). */
export const GUIDES = (file: string) =>
  (file.startsWith("docs/") || file.startsWith(".agents/skills/") || file === "AGENTS.md" || /^tools\/.+\/README\.md$/.test(file)) &&
  file.endsWith(".md");

/** Each `cargo xtask <task>` a page names that is no task, with the tasks there are. */
export function unknownTasks(file: string, markdown: string, tasks: Set<string>): string[] {
  const named = new Set([...markdown.matchAll(/\bcargo xtask ([a-z][a-z0-9-]*)/g)].map((m) => m[1]));
  return [...named]
    .filter((task) => !tasks.has(task))
    .map((task) => `${file}: names \`cargo xtask ${task}\`, which is no task; \`cargo xtask tools\` lists them (xtask/src/main.rs)`);
}

/**
 * The repository paths a page names in backticks that do not exist: each word in a code span
 * that begins with one of the repository's top folders (`crates/…`, `node tools/x.ts --y`),
 * less a trailing `:line`, `#anchor` or punctuation. A word with a placeholder or a pattern in it
 * (`<name>`, `NAME`, `*`, `{a,b}`, `…`, `$X`) names no one path and is skipped. `exists` says
 * whether a path is in the repository or is somewhere git ignores, where a tool writes.
 */
export function missingPaths(file: string, markdown: string, exists: (path: string) => boolean, tops: Set<string>): string[] {
  const faults = new Set<string>();
  for (const [, span] of withoutFences(markdown).matchAll(/`([^`\n]+)`/g)) {
    for (const word of span.split(/[\s=]+/)) {
      const path = word.replace(/^["'(]+/, "").replace(/(:\d+(-\d+)?|#[\w-]*)?["'),.;:]*$/, "").replace(/\/$/, "");
      const top = path.split("/")[0];
      const placeholder = /[<>*{}…$\[\]|]/.test(path) || path.split("/").some((part) => /^[A-Z]+$|nnn|NNN/.test(part));
      if (!path.includes("/") || !tops.has(top) || placeholder) continue;
      if (!exists(path)) faults.add(`${file}: names \`${path}\`, which is not in the repository`);
    }
  }
  return [...faults];
}

/** Entries of a folder that its index does not link or name. */
export function unindexed(index: string, indexText: string, entries: string[]): string[] {
  return entries.filter((e) => !indexText.includes(e)).map((e) => `${index}: does not list ${e}`);
}

function check(): string[] {
  const tracked = execFileSync("git", ["ls-files"], { cwd: ROOT, encoding: "utf8" }).trim().split("\n");
  const read = (path: string) => {
    const full = join(ROOT, path);
    if (!existsSync(full)) return undefined;
    return statSync(full).isDirectory() ? "" : readFileSync(full, "utf8");
  };
  const faults: string[] = [];
  const docs = tracked.filter((f) => f.endsWith(".md"));
  for (const file of docs) faults.push(...brokenLinks(file, read(file)!, read));

  const sources = tracked
    .filter((f) => f.startsWith("crates/") && f.endsWith(".rs"))
    .map((f) => read(f) ?? "")
    .join("\n");
  for (const page of PROOF_PAGES) faults.push(...missingTests(page, read(page) ?? "", sources));
  faults.push(...renderPages(read));

  const tasks = xtaskTasks(read("xtask/src/main.rs") ?? "");
  const files = new Set(tracked);
  const folders = new Set(tracked.flatMap((f) => f.split("/").slice(0, -1).map((_, i, parts) => parts.slice(0, i + 1).join("/"))));
  const tops = new Set([...folders].filter((f) => !f.includes("/")));
  // Ignored as it stands, or as a folder (`out/`), which git can tell only of one that exists.
  const ignored = (path: string) => {
    try {
      execFileSync("git", ["check-ignore", "--no-index", path, `${path}/`], { cwd: ROOT, stdio: "ignore" });
      return true;
    } catch {
      return false;
    }
  };
  const exists = (path: string) => files.has(path) || folders.has(path) || ignored(path);
  for (const file of docs.filter(GUIDES)) {
    faults.push(...unknownTasks(file, read(file)!, tasks));
    if (file.startsWith(".agents/skills/")) faults.push(...missingPaths(file, read(file)!, exists, tops));
  }

  const decisions = decisionFiles();
  faults.push(...decisions.faults, ...strayRows(read("docs/decisions.md") ?? ""));
  const ids = new Set(decisions.decisions.map((d) => d.id));
  for (const file of tracked.filter((f) => CITES.test(f))) faults.push(...unknownDecisions(file, read(file) ?? "", ids));

  const inFolder = (folder: string, pattern: RegExp) =>
    tracked.filter((f) => dirname(f) === folder && pattern.test(basename(f)) && basename(f) !== "README.md").map((f) => basename(f));
  const index = (readme: string, entries: string[]) => faults.push(...unindexed(readme, read(readme) ?? "", entries));
  index("docs/README.md", inFolder("docs", /\.md$/));
  index("docs/proposals/README.md", inFolder("docs/proposals", /\.md$/));
  index("research/README.md", inFolder("research", /\.(md|csv)$/));
  const spikes = readdirSync(join(ROOT, "spike")).filter((s) => tracked.some((f) => f.startsWith(`spike/${s}/`)));
  index("spike/README.md", spikes.map((s) => `${s}/`));
  for (const tool of new Set(tracked.filter((f) => f.startsWith("tools/")).map((f) => f.split("/")[1]))) {
    const readme = `tools/${tool}/README.md`;
    if (read(readme) === undefined) faults.push(`tools/${tool}: has no README.md`);
    else
      index(
        readme,
        tracked
          .filter((f) => f.startsWith(`tools/${tool}/`) && /\.(ts|mjs|py|sh)$/.test(f) && !/\.test\./.test(f))
          .map((f) => basename(f)),
      );
  }
  return faults;
}

/** The decision numbers a listing of docs/decisions/ or an index's rows hold. */
export function decisionIds(text: string): string[] {
  return [...text.matchAll(/^(?:\| \[?)?(D\d+)(?:\.md|\b)/gm)].map((m) => m[1]);
}

/** The next free decision number here and on origin/main, fetched first, for `cargo xtask decision`. */
function next(): number {
  const git = (...args: string[]) => execFileSync("git", args, { cwd: ROOT, encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] });
  const ids = readdirSync(join(ROOT, "docs/decisions")).flatMap(decisionIds);
  try {
    git("fetch", "-q", "origin", "main");
  } catch {
    console.error("docs: could not fetch origin; going by the origin/main this repository has");
  }
  try {
    ids.push(...decisionIds(git("ls-tree", "--name-only", "origin/main", "docs/decisions/").replaceAll("docs/decisions/", "")));
    ids.push(...decisionIds(git("show", "origin/main:docs/decisions.md")));
  } catch {
    console.error("docs: there is no origin/main to compare with");
  }
  return nextDecision(ids);
}

/**
 * Decisions in full, each followed by the log's lines that name it (`D12`, or a range
 * `D9 to D18` it falls in), for `cargo xtask decision D28 D198`.
 */
export function show(ids: string[], read: (id: string) => string | undefined, log: string): string {
  const lines = log.split("\n").filter((line) => line.startsWith("- "));
  const out: string[] = [];
  for (const raw of ids) {
    const id = raw.toUpperCase().replace(/^(\d)/, "D$1");
    const n = Number(id.slice(1));
    const text = read(id);
    if (text === undefined) {
      out.push(`${id}: there is no docs/decisions/${id}.md`, "");
      continue;
    }
    const named = lines.filter(
      (line) =>
        new RegExp(`\\b${id}\\b`).test(line) ||
        [...line.matchAll(/\bD(\d+) to D(\d+)\b/g)].some(([, a, b]) => Number(a) <= n && n <= Number(b)),
    );
    out.push(text.trimEnd(), "", named.length ? `## Log\n\n${named.join("\n")}` : "## Log\n\nNo lines.", "");
  }
  return out.join("\n");
}

if (import.meta.main && process.argv.includes("--next")) {
  console.log(`D${next()}`);
} else if (import.meta.main && process.argv.includes("--show")) {
  const ids = process.argv.slice(process.argv.indexOf("--show") + 1);
  const read = (id: string) => {
    const file = join(ROOT, "docs/decisions", `${id}.md`);
    return existsSync(file) ? readFileSync(file, "utf8") : undefined;
  };
  console.log(show(ids, read, readFileSync(join(ROOT, "docs/decisions-log.md"), "utf8")));
} else if (import.meta.main && process.argv.includes("--render")) {
  const read = (path: string) => (existsSync(join(ROOT, path)) ? readFileSync(join(ROOT, path), "utf8") : undefined);
  const faults = renderPages(read, (path, text) => {
    writeFileSync(join(ROOT, path), text);
    console.log(`docs: wrote ${path}`);
  });
  for (const fault of faults) console.log(`  ${fault}`);
  process.exit(faults.length ? 1 : 0);
} else if (import.meta.main) {
  const faults = check();
  for (const fault of faults) console.log(`  ${fault}`);
  console.log(faults.length ? `docs: ${faults.length} faults` : "docs: ok");
  process.exit(faults.length ? 1 : 0);
}
