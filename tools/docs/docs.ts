// Checks the repository's docs, run by `cargo xtask check`: links and anchors resolve, every
// doc, note, spike and tool script is in its folder's index, the decisions table is well
// formed, with every decision a doc cites in it, and every test a contract page cites as its
// proof exists, and a page's part rendered from a data file matches it. Prints each fault;
// exits 1 if there are any.
//
//   node tools/docs/docs.ts
//   node tools/docs/docs.ts --next    the next free decision number, here and on origin/main (`cargo xtask decision`)
//   node tools/docs/docs.ts --render  writes the parts of pages rendered from data files (the voxel contract's matrix)

import { execFileSync } from "node:child_process";
import { existsSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { basename, dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

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

/** The decisions table's rows that are malformed, and the decision numbers they hold. */
export function decisionTable(markdown: string): { faults: string[]; ids: Set<string> } {
  const faults: string[] = [];
  const topics = new Map<string, string>();
  const twice: string[] = [];
  for (const line of markdown.split("\n")) {
    if (!/^\| D\d+ \|/.test(line)) continue;
    const cells = line.slice(1, -1).split(" | ").map((c) => c.trim());
    const [id, topic, owner, by, status] = cells;
    const fault = (what: string) => faults.push(`docs/decisions.md: ${id} ${what}`);
    if (!line.endsWith("|") || cells.length !== 7) fault(`has ${cells.length} cells, not 7 (a | in its text?)`);
    // Two agents took the same number at once; the second to push renumbers its own.
    const first = topics.get(id);
    if (first !== undefined) twice.push(`${id} is in the table twice, "${first}" and "${topic}"`);
    else topics.set(id, topic);
    if (owner !== "Design" && owner !== "Tech") fault(`has Owner "${owner}", not Design or Tech`);
    if (!STATUS.test(status ?? "")) fault(`has Status "${status}"`);
    const nobody = /^(Open|Rejected|Superseded)/.test(status ?? "");
    if (nobody ? by !== "—" : by !== "Developers" && by !== "Agent")
      fault(`has By "${by}": Developers or Agent, or — when Open, Rejected or Superseded`);
  }
  const ids = new Set(topics.keys());
  for (const what of twice)
    faults.push(`docs/decisions.md: ${what}: renumber the one you added to D${nextDecision(ids)}, the next free number`);
  return { faults, ids };
}

/** The next free decision number: one past the highest in use. */
export function nextDecision(ids: Iterable<string>): number {
  let highest = 0;
  for (const id of ids) highest = Math.max(highest, Number(id.slice(1)));
  return highest + 1;
}

/** Decision numbers a text cites that the table does not have. */
export function unknownDecisions(file: string, text: string, ids: Set<string>): string[] {
  const missing = new Set([...text.matchAll(/\bD(\d{1,3})\b/g)].map((m) => `D${m[1]}`).filter((d) => !ids.has(d)));
  return [...missing].map((d) => `${file}: cites ${d}, which is not in docs/decisions.md`);
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

/** Pages whose tables are written from a data file (D348): the page, and the file. */
const RENDERED = [{ page: "docs/building-voxels.md", data: "docs/building-voxels.json" }];

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
function renderPages(read: (path: string) => string | undefined, write?: (path: string, text: string) => void): string[] {
  const faults: string[] = [];
  for (const { page, data } of RENDERED) {
    const text = read(page) ?? "";
    let again: string | undefined;
    try {
      again = rendered(text, () => renderMatrix(JSON.parse(read(data) ?? "")));
    } catch (why) {
      faults.push(`${data}: does not read: ${why}`);
      continue;
    }
    if (again === undefined) faults.push(`${page}: has no part rendered from ${data}`);
    else if (again !== text) {
      if (write) write(page, again);
      else faults.push(`${page}: differs from ${data}; edit the data, then run node tools/docs/docs.ts --render`);
    }
  }
  return faults;
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

  const table = decisionTable(read("docs/decisions.md")!);
  faults.push(...table.faults);
  for (const file of docs) if (file !== "docs/decisions.md") faults.push(...unknownDecisions(file, read(file)!, table.ids));

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

/** The next free decision number here and on origin/main, fetched first, for `cargo xtask decision`. */
function next(): number {
  const git = (...args: string[]) => execFileSync("git", args, { cwd: ROOT, encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] });
  const tables = [readFileSync(join(ROOT, "docs/decisions.md"), "utf8")];
  try {
    git("fetch", "-q", "origin", "main");
  } catch {
    console.error("docs: could not fetch origin; going by the origin/main this repository has");
  }
  try {
    tables.push(git("show", "origin/main:docs/decisions.md"));
  } catch {
    console.error("docs: there is no origin/main to compare with");
  }
  return Math.max(...tables.map((t) => nextDecision(decisionTable(t).ids)));
}

if (import.meta.main && process.argv.includes("--next")) {
  console.log(`D${next()}`);
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
