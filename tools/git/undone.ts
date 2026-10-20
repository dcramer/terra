// Commits about to be pushed that silently undo another's recent work (D431): a file written
// over a fresh main from a stale copy takes out every line the commits since added and puts
// back what they replaced. b743f619 did so to five commits on 2026-10-20.
//
//   node tools/git/undone.ts [range]      (default origin/main..HEAD; the pre-push hook runs it)
//
// For each commit C in the range and each file it changes that its message does not name
// (by file name), it blames the lines C deletes. A commit O from the last 48 hours, outside
// the range, is undone in that file when C deletes every line O added there (at least
// three) and either puts back a line O removed or O only added; deleted lines C adds again
// elsewhere (a move) and `spike/` don't count, and naming O's short hash in C's message
// says it was meant. It warns, never fails: one line a commit undone, nothing when clean.
// Node 22.18+, no dependencies; `node --test tools/git/undone.test.ts`.
import { execFileSync } from "node:child_process";
import { basename } from "node:path";

/// How far back a commit's work counts as fresh, in seconds.
const WINDOW = 48 * 3600;
/// The fewest of O's lines in a file that make an undo worth a warning.
const LEAST = 3;

export type Undone = { commit: string; file: string; undid: string; lines: number; restored: number };

/// Lines worth comparing: trimmed, and not a lone bracket or blank.
const meaningful = (line: string) => line.trim().length >= 4;

export function undone(range: string, cwd = process.cwd()): Undone[] {
  const git = (...args: string[]) =>
    execFileSync("git", args, { cwd, encoding: "utf8", maxBuffer: 1 << 28, stdio: ["ignore", "pipe", "ignore"] });
  const commits = git("rev-list", "--no-merges", range).split("\n").filter(Boolean);
  const pushed = new Set(commits);
  const found: Undone[] = [];
  for (const c of commits) {
    let diff: string;
    try {
      diff = git("diff", "-U0", "--no-renames", `${c}~1`, c);
    } catch {
      continue; // a root commit
    }
    const time = Number(git("log", "-1", "--format=%ct", c).trim());
    const message = git("log", "-1", "--format=%B", c);
    // The line ranges C deletes in each file, as they stood before it, and every line it adds.
    const deleted = new Map<string, string[]>();
    const added = new Set<string>();
    let file = "";
    for (const line of diff.split("\n")) {
      if (line.startsWith("--- ")) {
        file = line === "--- /dev/null" ? "" : line.slice(6);
        continue;
      }
      if (line.startsWith("+++ ")) continue;
      const hunk = /^@@ -(\d+)(?:,(\d+))? \+/.exec(line);
      if (hunk && file) {
        const start = Number(hunk[1]);
        const count = hunk[2] === undefined ? 1 : Number(hunk[2]);
        if (count > 0) (deleted.get(file) ?? deleted.set(file, []).get(file)!).push("-L", `${start},${start + count - 1}`);
      } else if (line.startsWith("+") && meaningful(line.slice(1))) {
        added.add(line.slice(1).trim());
      }
    }
    for (const [f, ranges] of deleted) {
      if (f.startsWith("spike/") || message.includes(basename(f))) continue;
      // Who wrote each deleted line, and the lines themselves.
      const blame = git("blame", "--porcelain", "-w", ...ranges, `${c}~1`, "--", f);
      const times = new Map<string, number>();
      const lines = new Map<string, string[]>();
      let author = "";
      for (const line of blame.split("\n")) {
        const head = /^([0-9a-f]{40}) \d+ \d+/.exec(line);
        if (head) author = head[1];
        else if (line.startsWith("committer-time ")) times.set(author, Number(line.slice(15)));
        else if (line.startsWith("\t")) (lines.get(author) ?? lines.set(author, []).get(author)!).push(line.slice(1).trim());
      }
      for (const [o, gone] of lines) {
        const age = time - (times.get(o) ?? 0);
        if (pushed.has(o) || age < 0 || age > WINDOW || message.includes(o.slice(0, 8))) continue;
        const kept = gone.filter(meaningful);
        if (kept.length < LEAST) continue;
        // A move: most of what C deletes of O's it writes again somewhere.
        if (kept.filter((l) => added.has(l)).length * 2 > kept.length) continue;
        const own = git("show", "-U0", "--no-renames", "--format=", o, "--", f).split("\n");
        const theirs = own.filter((l) => l.startsWith("+") && !l.startsWith("+++") && meaningful(l.slice(1)));
        const replaced = new Set(
          own.filter((l) => l.startsWith("-") && !l.startsWith("---") && meaningful(l.slice(1))).map((l) => l.slice(1).trim()),
        );
        if (kept.length < theirs.length) continue; // O's work in f partly survives: an edit, not an undo
        const restored = [...added].filter((l) => replaced.has(l)).length;
        if (restored === 0 && replaced.size > 0) continue;
        found.push({ commit: c, file: f, undid: o, lines: kept.length, restored });
      }
    }
  }
  return found;
}

if (import.meta.main) {
  const range = process.argv[2] ?? "origin/main..HEAD";
  let found: Undone[] = [];
  try {
    found = undone(range);
  } catch {
    process.exit(0); // no such range (a fresh clone, no origin): nothing to warn of
  }
  const subject = (hash: string) =>
    execFileSync("git", ["log", "-1", "--format=%s", hash], { encoding: "utf8" }).trim().slice(0, 50);
  for (const u of found) {
    const back = u.restored ? `, puts back ${u.restored} it replaced` : "";
    console.error(
      `⚠ ${u.commit.slice(0, 8)} undoes ${u.undid.slice(0, 8)} in ${u.file}: deletes all ${u.lines} lines it added${back} ("${subject(u.undid)}…")`,
    );
  }
  if (found.length) {
    console.error(
      "  a stale copy? rebase onto origin/main, never copy files over it (docs/development.md, D431); if meant, name the file or the commit in the message",
    );
  }
}
