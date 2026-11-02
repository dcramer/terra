// The fitter's cache (D552, docs/attachment.md "Fitting a new item"): each stage keyed by the
// hash of everything it reads, as Unreal's derived-data cache keys a cooked asset by its inputs
// and its builder's version, and Unity re-imports on an importer's dependency hash. A stage whose
// key is in the cache is not run: its verdict is the one kept, and the files it wrote are put
// back from the cache by their bytes' hash if they have changed since. A key covers the stage's
// own scripts and every script of ours they import, so editing a maker or a measure runs it
// again; `fit.ts --fresh` runs every stage regardless.

import { createHash } from "node:crypto";
import { copyFileSync, existsSync, mkdirSync, readFileSync, renameSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, relative } from "node:path";

export const sha = (b: Buffer | string) => createHash("sha256").update(b).digest("hex");

const hashed = new Map<string, { at: string; sha: string }>();
/** A file's bytes' hash, "absent" for none; kept while its size and time stay. */
export function fileSha(file: string): string {
  if (!existsSync(file)) return "absent";
  const s = statSync(file);
  const at = `${s.size}:${s.mtimeMs}`;
  const was = hashed.get(file);
  if (was?.at === at) return was.sha;
  const h = sha(readFileSync(file));
  hashed.set(file, { at, sha: h });
  return h;
}

/** A script and every script of ours it reaches: a TypeScript `from "./x.ts"` or a Python or
 * shell script it names to run ("x.py", as build_steps.ts names its steps), a Python `import x`
 * or `from x import` with x.py beside it, a shell script's `$here/x` or `tools/.../x`; sorted. */
export function closure(file: string, root: string, seen = new Set<string>()): string[] {
  if (seen.has(file) || !existsSync(file)) return [...seen].sort();
  seen.add(file);
  const text = readFileSync(file, "utf8");
  const dir = dirname(file);
  const next: string[] = [];
  if (file.endsWith(".ts")) {
    for (const [, f] of text.matchAll(/from\s+"(\.{1,2}\/[^"]+\.ts)"/g)) next.push(join(dir, f));
    for (const [, f] of text.matchAll(/["`/]([\w-]+\.(?:py|sh))\b/g)) next.push(join(root, "tools", "characters", f));
  }
  if (file.endsWith(".py"))
    for (const [, a, b] of text.matchAll(/^\s*(?:import\s+([\w, ]+)|from\s+(\w+)\s+import)/gm))
      for (const m of (a ?? b).split(",").map((s) => s.trim().split(/\s+/)[0])) next.push(join(dir, `${m}.py`));
  // A shell script names its scripts however it runs them ($here/x.py, a name it picks): every
  // script's name in it that is one in tools/characters.
  if (file.endsWith(".sh")) for (const [, f] of text.replace(/^\s*#.*$/gm, "").matchAll(/\b([\w-]+\.(?:py|ts|sh))\b/g)) next.push(join(root, "tools", "characters", f));
  for (const f of next) if (existsSync(f)) closure(f, root, seen);
  return [...seen].sort();
}

/** A stage's key: its name, each input file's path and bytes, and the values it is given. */
export function key(stage: string, root: string, files: string[], values: unknown = null): string {
  const parts = [...new Set(files)].sort().map((f) => `${relative(root, f)} ${fileSha(f)}`);
  return sha(JSON.stringify([stage, parts, values])).slice(0, 32);
}

/** What a stage left: its verdict, what it measured, and the files it wrote by their hash. */
export type Entry = { ok: boolean; line: string; outputs: Record<string, string>; data?: unknown };

export class Cache {
  readonly dir: string;
  constructor(dir: string) {
    this.dir = dir;
  }
  private file = (k: string) => join(this.dir, `${k}.json`);
  private blob = (h: string) => join(this.dir, "blobs", h);

  /** The entry for a key, with its files put back as they were made; undefined if any is lost. */
  get(k: string, root: string): Entry | undefined {
    if (!existsSync(this.file(k))) return undefined;
    const e = JSON.parse(readFileSync(this.file(k), "utf8")) as Entry;
    for (const [rel, h] of Object.entries(e.outputs)) {
      const to = join(root, rel);
      if (fileSha(to) === h) continue;
      if (!existsSync(this.blob(h))) return undefined;
      mkdirSync(dirname(to), { recursive: true });
      copyFileSync(this.blob(h), to);
    }
    return e;
  }

  /** Keeps a stage's verdict and the bytes of the files it wrote (paths from the root). */
  put(k: string, root: string, e: Omit<Entry, "outputs">, outputs: string[] = []): Entry {
    mkdirSync(join(this.dir, "blobs"), { recursive: true });
    const kept: Record<string, string> = {};
    for (const f of outputs) {
      const h = fileSha(f);
      if (h === "absent") continue;
      if (!existsSync(this.blob(h))) copyFileSync(f, this.blob(h));
      kept[relative(root, f)] = h;
    }
    const entry = { ...e, outputs: kept };
    // Written whole and then named, so a run stopped half way leaves no half an entry.
    const tmp = `${this.file(k)}.${process.pid}`;
    writeFileSync(tmp, JSON.stringify(entry));
    renameSync(tmp, this.file(k));
    return entry;
  }
}
