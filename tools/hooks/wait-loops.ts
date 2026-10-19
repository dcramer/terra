// PreToolUse hook for the Bash tool (research/2026-10-19-agent-throughput.md, fix 3): refuses the
// background wait loops that hang. `until ! pgrep -f "xtask bench"; do sleep 10; done` never ends,
// because the loop's own command line holds the pattern and pgrep finds itself; and every wake-up of
// any polling loop is a turn at the agent's whole context. Waits go by the completion notice of a
// background task, or `tail --pid=<pid> -f /dev/null`.
// Claude Code sends the call as JSON on stdin; exit 2 refuses it and stderr is the reason it shows.
import { readFileSync } from "node:fs";

const USE =
  "wait on the background task's completion notice, or `tail --pid=<pid> -f /dev/null` with a pid from `$!` or `cargo xtask gpu`";

/** Blanks heredoc bodies and quoted strings, so text that only mentions a loop (a commit message,
 *  an echo) is not one; the string after `-c` (`bash -c '...'`) is code and is kept. */
export function code(command: string): string {
  const bare = command.replace(
    /<<-?\s*(["']?)(\w+)\1[^\n]*\n[\s\S]*?\n\s*\2(?=\n|$)/g,
    (m) => m.slice(0, m.indexOf("\n")),
  );
  return bare.replace(/(\s-[a-z]*c\s+)?('[^']*'|"(?:\\.|[^"\\])*")/g, (m, c, q) =>
    c ? m : `${q[0]}${q[0]}`,
  );
}

const OPEN = /(?:^|[;&|({\n]|\bdo\b|\bthen\b|\belse\b)\s*!?\s*(for|while|until|select)\b/g;
// `done` as a command word, not the middle of `out/done.txt`.
const CLOSE = /(?<=^|[;&|(){}\n]\s*)done(?=\s*(?:$|[;&|)<>\n]))/g;

interface Mark {
  at: number;
  end: number;
  kind: string;
}

/** Every `while`/`until` loop in the command: its condition and its whole text up to `done`. */
function loops(src: string): { condition: string; whole: string }[] {
  const marks: Mark[] = [];
  for (const m of src.matchAll(OPEN)) {
    marks.push({ at: m.index + m[0].lastIndexOf(m[1]), end: m.index + m[0].length, kind: m[1] });
  }
  for (const m of src.matchAll(CLOSE)) marks.push({ at: m.index, end: m.index + 4, kind: "done" });
  marks.sort((a, b) => a.at - b.at);
  const found: { condition: string; whole: string }[] = [];
  const add = (open: Mark, to: number) => {
    if (open.kind !== "while" && open.kind !== "until") return;
    const whole = src.slice(open.at, to);
    const doAt = whole.search(/(?<=^|[;&|\n])\s*do\b/);
    found.push({ condition: doAt < 0 ? whole : whole.slice(0, doAt), whole });
  };
  const stack: Mark[] = [];
  for (const mark of marks) {
    if (mark.kind !== "done") stack.push(mark);
    else {
      const open = stack.pop();
      if (open) add(open, mark.end);
    }
  }
  for (const open of stack) add(open, src.length); // no `done` yet: the rest of the command
  return found;
}

const PID_FROM_PGREP =
  "Refused: `tail --pid=$(pgrep ...)` takes the waiting shell's own pid when the pattern is in its command line, and never ends; " +
  "take the pid from `$!` when you start the job, or wait on the background task's completion notice (`cargo xtask gpu --status` lists job pids).";

/** `tail --pid=$(pgrep …)`, the backtick form, or `P=$(pgrep …); tail --pid=$P`: the pid is
 *  pgrep's, and pgrep finds the waiting shell itself. */
function pidFromPgrep(command: string): boolean {
  // `--pid="$P"` is quoted, and `code` blanks quotes: drop the quotes around the pid first.
  const text = code(command.replace(/(--pid[= ]\s*)(["'])(.*?)\2/g, "$1$3"));
  const sub = String.raw`(?:\$\(|\x60)\s*pgrep\b`;
  if (new RegExp(String.raw`\btail\b[^\n;&|]*--pid[= ]\s*${sub}`).test(text)) return true;
  for (const m of text.matchAll(new RegExp(String.raw`\b(\w+)=${sub}`, "g"))) {
    const use = String.raw`\btail\b[^\n;&|]*--pid[= ]\s*\$(?:${m[1]}\b|\{${m[1]}\b)`;
    if (new RegExp(use).test(text)) return true;
  }
  return false;
}

/** Why the command is refused, or "" when it passes. */
export function refusal(command: string): string {
  if (pidFromPgrep(command)) return PID_FROM_PGREP;
  const src = code(command);
  // `bash -c '...'` and its kin: the quoted script is a command of its own.
  const inner = [...src.matchAll(/\s-[a-z]*c\s+(?:'([^']*)'|"((?:\\.|[^"\\])*)")/g)].map(
    (m) => m[1] ?? m[2],
  );
  for (const text of [src, ...inner.map(code)]) {
    for (const { condition, whole } of loops(text)) {
      if (/\bpgrep\b/.test(condition)) {
        return `Refused: pgrep in a loop matches the loop's own command line and never ends; ${USE}.`;
      }
      if (/\bsleep\b/.test(whole)) {
        return `Refused: a sleep polling loop costs a turn at every wake-up; ${USE}.`;
      }
    }
  }
  return "";
}

if (import.meta.main) {
  let command: unknown = "";
  try {
    command = JSON.parse(readFileSync(0, "utf8")).tool_input?.command ?? "";
  } catch {
    // No readable input: no decision.
  }
  const why = typeof command === "string" ? refusal(command) : "";
  if (why !== "") {
    process.stderr.write(`${why}\n`);
    process.exit(2);
  }
}
