// Git's merge driver for the pages whose index is rendered from files of their own (D409):
// docs/decisions.md from docs/decisions/, research/README.md from the notes. Each side's index
// is a function of its files, and those files merge on their own, so the index merges row by
// row: a row is keyed by what it links to, each side's added, changed and removed rows are
// kept, and the rows are put back in the order the page renders them. Two agents adding a
// decision or a note at once never conflict. As npm's lockfile merge driver does, it works on
// the generated text alone; anything it cannot place (the page around the index changed on both
// sides, one row changed two ways) it hands back to git as an ordinary conflict, which
// `node tools/docs/docs.ts --render` mends.
//
//   node tools/docs/merge.ts <base> <ours> <theirs> <path>   (git runs it: .gitattributes, `merge.rendered`)
//
// `cargo xtask` sets `merge.rendered.driver` for a clone, as it sets the hooks.

import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";

/** A page's rendered index as rows by key, and the rest of it with a place for each table's rows. */
export interface Index {
  skeleton: string[];
  rows: Map<string, { section: string; line: string }>;
}

const SEPARATOR = /^\|[-| :]+\|$/;
const BEGIN = /^<!-- rendered from \S+ by `node tools\/docs\/docs\.ts --render`: edit that, not this -->$/;
const END = "<!-- end of rendered -->";
const PLACE = "\u0000rows:";

/** What a row is known by: the first link in it, else its first cell. */
export function key(row: string): string {
  return row.match(/\]\(([^)]+)\)/)?.[1] ?? row.split("|")[1].trim();
}

/** A page's index read, or undefined when it has no rendered part. */
export function read(text: string): Index | undefined {
  const lines = text.split("\n");
  const begin = lines.findIndex((line) => BEGIN.test(line));
  const end = lines.indexOf(END);
  if (begin < 0 || end < begin) return undefined;
  const skeleton: string[] = [];
  const rows: Index["rows"] = new Map();
  let section = "";
  let inTable = false;
  for (const [i, line] of lines.entries()) {
    const inside = i > begin && i < end;
    if (inside && line.startsWith("#")) section = line;
    if (inside && inTable && line.startsWith("|")) {
      rows.set(key(line), { section, line });
      continue;
    }
    inTable = inside && SEPARATOR.test(line);
    skeleton.push(line);
    if (inTable) skeleton.push(PLACE + section);
  }
  return { skeleton, rows };
}

/** The side that changed of three, or undefined when both changed it differently. */
function pick<T>(base: T, ours: T, theirs: T, same: (a: T, b: T) => boolean): { value: T } | undefined {
  if (same(ours, theirs) || same(theirs, base)) return { value: ours };
  if (same(ours, base)) return { value: theirs };
  return undefined;
}

/** How each page orders its rows: decisions by number, notes newest first. */
export const ORDER: Record<string, (a: string, b: string) => number> = {
  "docs/decisions.md": (a, b) => Number(key(a).match(/D(\d+)/)?.[1]) - Number(key(b).match(/D(\d+)/)?.[1]),
  "research/README.md": (a, b) => {
    const date = (row: string) => row.split("|").at(-2)?.trim() ?? "";
    return date(b).localeCompare(date(a)) || key(a).localeCompare(key(b));
  },
};

/** Three versions of a page merged row by row, or undefined when they cannot be. */
export function merge(base: string, ours: string, theirs: string, order: (a: string, b: string) => number): string | undefined {
  const [o, a, b] = [base, ours, theirs].map(read);
  if (!o || !a || !b) return undefined;
  const skeleton = pick(o.skeleton, a.skeleton, b.skeleton, (x, y) => x.join("\n") === y.join("\n"));
  if (!skeleton) return undefined;
  const sections = new Map<string, string[]>();
  for (const name of new Set([...o.rows.keys(), ...a.rows.keys(), ...b.rows.keys()])) {
    const row = pick(o.rows.get(name), a.rows.get(name), b.rows.get(name), (x, y) => x?.section === y?.section && x?.line === y?.line);
    if (!row) return undefined;
    if (row.value) sections.set(row.value.section, [...(sections.get(row.value.section) ?? []), row.value.line]);
  }
  const out: string[] = [];
  for (const line of skeleton.value) {
    if (!line.startsWith(PLACE)) {
      out.push(line);
      continue;
    }
    const section = line.slice(PLACE.length);
    out.push(...(sections.get(section) ?? []).sort(order));
    sections.delete(section);
  }
  // A row whose section the page no longer has cannot be placed.
  return sections.size ? undefined : out.join("\n");
}

if (import.meta.main) {
  const [base, ours, theirs, path] = process.argv.slice(2);
  const text = (file: string) => readFileSync(file, "utf8");
  const merged = ORDER[path] && merge(text(base), text(ours), text(theirs), ORDER[path]);
  if (merged !== undefined) {
    writeFileSync(ours, merged);
    process.exit(0);
  }
  // git's own three-way merge, conflicts marked, for whoever runs --render after it.
  try {
    execFileSync("git", ["merge-file", "-L", "ours", "-L", "base", "-L", "theirs", ours, base, theirs]);
  } catch {
    // It exits with the number of conflicts.
  }
  console.error(`docs: ${path} could not be merged row by row; run node tools/docs/docs.ts --render`);
  process.exit(1);
}
