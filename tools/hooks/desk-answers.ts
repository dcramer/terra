// UserPromptSubmit hook (D422): reminds a coordinator to read the review desk's answers before it
// replies. Answers on the desk do not notify the session, and coordinators missed them ("didnt i
// answer them?", "i think i answered items in the desk", 2026-10-24 to 26). It speaks only in a
// session that has used the desk (its transcript names the desk's url) or whose prompt mentions
// the desk, so other sessions pay nothing.
// Claude Code sends {prompt, transcript_path} as JSON on stdin; what this prints in
// `additionalContext` is added to the turn's context.
import { readFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

const MENTIONS = /\b(desk|deck|answered)\b/i;

/** The line to add to the turn, or nothing when this session has no business with the desk. */
export function reminder(url: string, prompt: string, transcript: string): string | undefined {
  if (!url || !(MENTIONS.test(prompt) || transcript.includes(url))) return undefined;
  return (
    `Review desk (${url}): answers do not notify you. Before replying, ArtifactData query \`items\` ` +
    `for status "answered" whose note lacks "Picked up:", act on each and mark it ` +
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
  const line = reminder(url, input.prompt ?? "", url ? read(input.transcript_path ?? "") : "");
  if (line) {
    console.log(
      JSON.stringify({
        hookSpecificOutput: { hookEventName: "UserPromptSubmit", additionalContext: line },
      }),
    );
  }
}
