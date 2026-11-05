// The guard on Quaternius's parts (D599: "their parts stay as made"), three of wardrobe.ts's
// stages, a line each (D388):
//
// - **vendored**: every file under art/characters/quaternius/ is listed in its SHA256SUMS and
//   matches it byte for byte, and every listed file is there; only the CC0 packs are vendored
//   (the developers, 2026-11-05: "Yes, CC0 packs only"; the Bestiary never, D599's licences).
// - **private**: each vendored file is, byte for byte, a file of the same name in the same pack
//   of the private repository's manifest (~/src/terra-private/SHA256SUMS, or $TERRA_PRIVATE),
//   so SHA256SUMS cannot be rewritten to bless a changed file. Said, not failed, on a machine
//   without the private repository.
// - **reshape**: no step can reshape their files. tools/blender/run.sh asks theirs.py before
//   Blender starts, and theirs.py refuses any script that reshapes meshes (the stretch,
//   decimation, a shrinkwrap, the clipping fixer's pushes) given one of their files; its spared
//   list names each script that carries a mark yet moves only ours, and why. The build's steps
//   that read their files run none of the reshaping scripts.
//
// Node 22.18 or later; no dependencies. Its test: vendored.test.ts.

import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { homedir } from "node:os";
import { basename, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
export const VENDORED = "art/characters/quaternius";
/** Our pack folders under VENDORED and theirs in the private repository: the CC0 packs alone. */
export const PACKS: Record<string, string> = {
  "modular-outfits-fantasy": "modular-character-outfits-fantasy",
  "universal-base-characters": "universal-base-characters",
};
const OURS = new Set(["README.md", "SHA256SUMS", ".gitattributes"]);

export type Stage = { name: string; faults: string[]; said: string };


/** `hash  path` lines of a sha256sum manifest, path to hash. */
export function manifest(text: string): Map<string, string> {
  const m = new Map<string, string>();
  for (const line of text.split("\n")) {
    const hit = /^([0-9a-f]{64}) [ *](.+)$/.exec(line.trimEnd());
    if (hit) m.set(hit[2], hit[1]);
  }
  return m;
}

function walk(dir: string): string[] {
  return readdirSync(dir).flatMap((n) => (statSync(join(dir, n)).isDirectory() ? walk(join(dir, n)) : [join(dir, n)]));
}

/** Every vendored file against SHA256SUMS, both ways. `root` the repository's. */
export function vendored(root = ROOT): Stage {
  const dir = join(root, VENDORED);
  const faults: string[] = [];
  const sums = manifest(readFileSync(join(dir, "SHA256SUMS"), "utf8"));
  const there = walk(dir).map((f) => relative(root, f).replaceAll("\\", "/"));
  const attrs = join(dir, ".gitattributes");
  if (there.length && !(existsSync(attrs) && /^\*\s+-text\b/m.test(readFileSync(attrs, "utf8")))) faults.push(`${VENDORED}/.gitattributes lost its "* -text": git would convert their files' line endings on a checkout, so they would no longer be as shipped; put it back`);
  let bytes = 0;
  for (const f of there) {
    if (OURS.has(basename(f)) && relative(VENDORED, f) === basename(f)) continue;
    const pack = relative(VENDORED, f).split("/")[0];
    if (!PACKS[pack]) faults.push(`${f}: ${pack} is not a CC0 pack; only CC0 packs are vendored (the developers, 2026-11-05), the Bestiary never (D599): remove it`);
    const want = sums.get(f);
    const data = readFileSync(join(root, f));
    bytes += data.length;
    if (!want) faults.push(`${f} is not in ${VENDORED}/SHA256SUMS: a file of theirs goes in byte for byte from the private repository with its line (sha256sum), a file of ours goes elsewhere`);
    else if (data.subarray(0, 40).toString().startsWith("version https://git-lfs")) faults.push(`${f} is a Git LFS pointer, not the file: git lfs pull`);
    else if (createHash("sha256").update(data).digest("hex") !== want) faults.push(`${f} is not as shipped (its SHA-256 is not SHA256SUMS's): their files are never changed (D599); restore it, git checkout -- ${f}`);
  }
  const have = new Set(there);
  for (const f of sums.keys()) if (!have.has(f)) faults.push(`${f} is in SHA256SUMS but not there: restore it (git lfs pull) or take its line out with the file`);
  return { name: "vendored", faults, said: `${sums.size} files of theirs (${(bytes / 1e6).toFixed(0)} MB) as SHA256SUMS lists, CC0 packs only` };
}

/** The private repository's root, if this machine has it. */
export function privateRoot(): string | undefined {
  const at = process.env.TERRA_PRIVATE ?? join(homedir(), "src", "terra-private");
  return existsSync(join(at, "SHA256SUMS")) ? at : undefined;
}

/** Each line of SHA256SUMS against the private manifest: a file of the same name in the same
 *  pack with the same hash. */
export function matchesPrivate(sums: Map<string, string>, priv: Map<string, string>): string[] {
  const byPack = new Map<string, Map<string, Set<string>>>();
  for (const [path, hash] of priv) {
    const [, pack] = path.split("/");
    const names = byPack.get(pack) ?? new Map<string, Set<string>>();
    byPack.set(pack, names);
    const n = basename(path);
    names.set(n, (names.get(n) ?? new Set()).add(hash));
  }
  const faults: string[] = [];
  for (const [path, hash] of sums) {
    const pack = PACKS[relative(VENDORED, path).split("/")[0]];
    const hashes = byPack.get(pack ?? "")?.get(basename(path));
    if (!hashes) faults.push(`${path}: no ${basename(path)} in the private repository's ${pack ?? "packs"}: vendor only files of theirs, as shipped`);
    else if (!hashes.has(hash)) faults.push(`${path}: SHA256SUMS's hash is no ${basename(path)} the private repository has: copy it again from there, byte for byte, and its line with it`);
  }
  return faults;
}

export function privately(root = ROOT, at = privateRoot()): Stage {
  if (!at) return { name: "private", faults: [], said: "no private repository on this machine (~/src/terra-private or TERRA_PRIVATE): SHA256SUMS alone vouches" };
  const sums = manifest(readFileSync(join(root, VENDORED, "SHA256SUMS"), "utf8"));
  const faults = matchesPrivate(sums, manifest(readFileSync(join(at, "SHA256SUMS"), "utf8")));
  return { name: "private", faults, said: `all ${sums.size} the private repository's own bytes (${at})` };
}

/** The reshaping scripts theirs.py finds and spares (`python3 theirs.py --list`). */
export function reshapers(root = ROOT): { reshapes: string[]; spared: string[]; stale: string[] } {
  const out = execFileSync("python3", [join(root, "tools/characters/theirs.py"), "--list"], { encoding: "utf8" });
  const of = (kind: string) => out.split("\n").filter((l) => l.startsWith(`${kind} `)).map((l) => l.slice(kind.length + 1).split("\t")[0]);
  return { reshapes: of("RESHAPES"), spared: of("SPARED"), stale: of("STALE") };
}

/** Steps (name, the files they read, their commands) that read their files and run a reshaper. */
export function reshapingSteps(steps: { name: string; inputs: string[]; run: string }[], reshapes: string[], theirs: (f: string) => boolean): string[] {
  const names = reshapes.map((r) => basename(r));
  return steps.flatMap((s) => {
    if (!s.inputs.some(theirs)) return [];
    const hit = names.filter((n) => new RegExp(`\\b${n.replace(".", "\\.")}\\b`).test(s.run));
    return hit.length ? [`build step ${s.name} reads their files and runs ${hit.join(", ")}, which reshapes meshes: their parts stay as made (D599); give it our pieces, or take the script out of the step`] : [];
  });
}

export function reshape(steps: { name: string; inputs: string[]; run: string }[], pieceFolders: string[], root = ROOT): Stage {
  const faults: string[] = [];
  if (!readFileSync(join(root, "tools/blender/run.sh"), "utf8").includes("characters/theirs.py")) faults.push("tools/blender/run.sh no longer asks theirs.py before Blender starts: put the check back (D599)");
  let found: ReturnType<typeof reshapers>;
  try {
    found = reshapers(root);
  } catch (e) {
    return { name: "reshape", faults: [`python3 tools/characters/theirs.py --list failed (${(e as Error).message.split("\n")[0]}): python3 is needed to guard their files`], said: "" };
  }
  for (const s of found.stale) faults.push(`theirs.py spares ${s}, which is not there: take it out of SPARED`);
  const theirs = (f: string) => f.startsWith(`${VENDORED}/`) || f.includes("/quaternius-") || pieceFolders.some((p) => f.startsWith(`assets/${p}/`));
  faults.push(...reshapingSteps(steps, found.reshapes, theirs));
  return { name: "reshape", faults, said: `${found.reshapes.length} scripts that reshape meshes refused their files by run.sh, ${found.spared.length} spared as moving only ours; no build step runs one on theirs` };
}

