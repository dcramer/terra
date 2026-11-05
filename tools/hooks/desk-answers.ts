// UserPromptSubmit hook (D422): reminds a coordinator to read the review desk's answers. Answers
// on the desk do not notify the session, and coordinators missed them ("didnt i answer them?",
// "i think i answered items in the desk", 2026-10-24 to 26). It speaks only in a session that has
// used the desk (its transcript names the desk's url) or whose prompt mentions the desk, and only
// when there may be something new (D627): whenever the prompt mentions the desk, otherwise on the
// session's first prompt and at most once an hour, since each reminder makes the turn query the desk.
// Claude Code sends {prompt, transcript_path, session_id} as JSON on stdin; what this prints in
// `additionalContext` is added to the turn's context.
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

const MENTIONS = /\b(desk|deck|answered)\b/i;
/** The least time between two reminders the prompt did not ask for. */
export const EVERY_MS = 60 * 60 * 1000;

/**
 * The line to add to the turn, or nothing when this session has no business with the desk or was
 * reminded less than an hour ago (`sinceLast`, null when never).
 */
export function reminder(url: string, prompt: string, transcript: string, sinceLast: number | null = null): string | undefined {
  if (!url) return undefined;
  const asked = MENTIONS.test(prompt);
  if (!asked && !(transcript.includes(url) && (sinceLast === null || sinceLast >= EVERY_MS))) return undefined;
  return (
    `Review desk (${url}): answers do not notify you. ArtifactData query \`items\` for status "answered" ` +
    `(the new ones only: a picked-up item is status "picked"), act on each and mark it picked ` +
    `(review-desk skill, "Read answers").`
  );
}

/** A file's text, or nothing when it cannot be read (no desk set up, no transcript yet). */
function read(path: string | number): string {
  try {
    return readFileSync(path, "utf8");
  } catch {
    return "";
  }
}

if (import.meta.main) {
  const input = JSON.parse(read(0) || "{}");
  let url = "";
  try {
    url = JSON.parse(read(join(homedir(), ".config/terra/desk.json"))).url ?? "";
  } catch {}
  const dir = join(process.env.XDG_CACHE_HOME ?? join(homedir(), ".cache"), "terra-desk-hook");
  const state = join(dir, String(input.session_id ?? "none").replace(/[^A-Za-z0-9_-]/g, "-"));
  const last = Number(read(state));
  const sinceLast = last > 0 ? Date.now() - last : null;
  // The transcript is read only when a reminder could be due.
  const due = sinceLast === null || sinceLast >= EVERY_MS;
  const line = reminder(url, input.prompt ?? "", url && due ? read(input.transcript_path ?? "") : "", sinceLast);
  if (line) {
    try {
      mkdirSync(dir, { recursive: true });
      writeFileSync(state, String(Date.now()));
    } catch {}
    console.log(
      JSON.stringify({
        hookSpecificOutput: { hookEventName: "UserPromptSubmit", additionalContext: line },
      }),
    );
  }
}
