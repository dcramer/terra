// Git LFS without git-lfs (D525): what a commit's LFS pointers name, and whether the files
// .gitattributes puts in LFS are committed as pointers. An agent on a machine without git-lfs
// would otherwise commit whole binaries, which then sit in the history for good.
//
//   node tools/git/lfs.ts --staged     the files about to be committed (the pre-commit hook)
//   node tools/git/lfs.ts --push       the commits a push sends, on stdin as git gives a pre-push
//                                      hook: each commit not yet on origin (the pre-push hook)
//
// A line on failure naming the files and the fix, nothing when clean. The render worker's client
// reads `objects` to send a job's commit the LFS objects it needs (tools/render-worker/client.ts).
// Node 22.18+, no dependencies; `node --test tools/git/lfs.test.ts`.
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";

/// The largest a pointer file can be; git-lfs's own limit for reading one.
const POINTER_MAX = 1024;
const VERSION = "version https://git-lfs.github.com/spec/v1\n";

/// The object a Git LFS pointer names (its sha256), or undefined when the text is not a pointer.
export function pointerOid(text: string): string | undefined {
  if (!text.startsWith(VERSION) || text.length > POINTER_MAX) return undefined;
  return /^oid sha256:([0-9a-f]{64})$/m.exec(text)?.[1];
}

function git(cwd: string, args: string[], input?: string): string {
  const r = spawnSync("git", args, { cwd, input, encoding: "utf8", maxBuffer: 1 << 28 });
  if (r.status !== 0) throw new Error(`git ${args[0]}: ${(r.stderr ?? "").trim().split("\n").at(-1)}`);
  return r.stdout;
}

/// Each object's size, by `git cat-file --batch-check`; undefined for one git does not have.
function sizes(cwd: string, specs: string[]): (number | undefined)[] {
  const lines = git(cwd, ["cat-file", "--batch-check=%(objectsize)"], `${specs.join("\n")}\n`).split("\n");
  return specs.map((_, i) => (/^\d+$/.test(lines[i]) ? Number(lines[i]) : undefined));
}

/// Each object's content, by `git cat-file --batch`: only small ones are asked for.
function contents(cwd: string, specs: string[]): (string | undefined)[] {
  if (!specs.length) return [];
  const r = spawnSync("git", ["cat-file", "--batch"], { cwd, input: `${specs.join("\n")}\n`, maxBuffer: 1 << 28 });
  const buf = r.stdout;
  const out: (string | undefined)[] = [];
  let i = 0;
  for (const _ of specs) {
    const nl = buf.indexOf(10, i);
    const size = /^\S+ \S+ (\d+)$/.exec(buf.subarray(i, nl).toString())?.[1];
    i = nl + 1;
    if (size === undefined) {
      out.push(undefined);
      continue;
    }
    out.push(buf.subarray(i, i + Number(size)).toString("latin1"));
    i += Number(size) + 1;
  }
  return out;
}

/// The LFS objects (sha256) the pointers in a commit's tree name, sorted and without repeats.
export function objects(cwd: string, rev: string): string[] {
  const small = git(cwd, ["ls-tree", "-r", "-l", "-z", rev])
    .split("\0")
    .map((e) => /^\d+ blob ([0-9a-f]+) +(\d+)\t/.exec(e))
    .filter((m): m is RegExpExecArray => m !== null && Number(m[2]) <= POINTER_MAX)
    .map((m) => m[1]);
  const oids = contents(cwd, small).map((t) => (t === undefined ? undefined : pointerOid(t)));
  return [...new Set(oids.filter((o): o is string => o !== undefined))].sort();
}

/// Of these paths, those .gitattributes puts in LFS: as staged (`source` undefined) or at a commit.
function inLfs(cwd: string, paths: string[], source?: string): string[] {
  if (!paths.length) return [];
  const out = git(cwd, ["check-attr", "-z", "--stdin", source ? `--source=${source}` : "--cached", "filter"], `${paths.join("\0")}\0`).split("\0");
  const found: string[] = [];
  for (let i = 0; i + 2 < out.length; i += 3) if (out[i + 2] === "lfs") found.push(out[i]);
  return found;
}

/// The LFS paths among those written whose blobs are whole files rather than pointers.
function whole(cwd: string, paths: string[], at: string): string[] {
  const specs = paths.map((p) => `${at}:${p}`);
  const size = sizes(cwd, specs);
  const small = specs.filter((_, i) => (size[i] ?? Infinity) <= POINTER_MAX);
  const read = contents(cwd, small);
  const text = new Map(small.map((s, i) => [s, read[i]]));
  return paths.filter((_, i) => {
    const t = text.get(specs[i]);
    return size[i] !== undefined && (t === undefined || pointerOid(t) === undefined);
  });
}

/// The files about to be committed that should be LFS pointers and are not.
export function stagedWhole(cwd: string): string[] {
  const paths = git(cwd, ["diff", "--cached", "--name-only", "-z", "--diff-filter=AMCRT"]).split("\0").filter(Boolean);
  return whole(cwd, inLfs(cwd, paths), "");
}

/// The files a commit writes that should be LFS pointers and are not, as `<commit> <path>`.
export function commitWhole(cwd: string, commit: string): string[] {
  const paths = git(cwd, ["diff-tree", "-r", "-z", "--root", "--no-commit-id", "--name-only", "--diff-filter=AMCRT", commit]).split("\0").filter(Boolean);
  return whole(cwd, inLfs(cwd, paths, commit), commit).map((p) => `${commit.slice(0, 9)} ${p}`);
}

/// The commits a push sends that origin does not have yet, from the pre-push hook's stdin.
export function pushedCommits(cwd: string, stdin: string): string[] {
  const tips = stdin
    .split("\n")
    .map((l) => l.trim().split(/\s+/)[1])
    .filter((sha) => sha && !/^0+$/.test(sha));
  if (!tips.length) return [];
  return git(cwd, ["rev-list", "--no-merges", ...tips, "--not", "--remotes=origin"]).split("\n").filter(Boolean);
}

export function fix(found: string[]): string {
  const shown = found.slice(0, 5).join(", ") + (found.length > 5 ? `, and ${found.length - 5} more` : "");
  return [
    `✗ ${found.length} file(s) .gitattributes puts in Git LFS are committed whole, not as LFS pointers (D525): ${shown}`,
    "    install git-lfs (docs/development.md: `mise use -g git-lfs@latest`, then `git lfs install --skip-repo`),",
    "    then `git add --renormalize <those files>` and commit again (amend a commit not yet pushed)",
  ].join("\n");
}

function main(args: string[]): number {
  const cwd = process.cwd();
  let found: string[];
  if (args[0] === "--staged") found = stagedWhole(cwd);
  else if (args[0] === "--push") {
    found = pushedCommits(cwd, readFileSync(0, "utf8")).flatMap((c) => commitWhole(cwd, c));
  } else {
    console.log("usage: node tools/git/lfs.ts --staged | --push  (the hooks run it)");
    return 2;
  }
  if (found.length) console.log(fix(found));
  return found.length ? 1 : 0;
}

if (import.meta.main) process.exit(main(process.argv.slice(2)));
