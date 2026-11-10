// A number the game's source defines, read from it, so a tool's twin of a game system cannot
// hold another value (carry_clear.ts, wrist.ts; research/2026-11-09-holding-items-programmatically.md:
// nothing else holds a twin to the game). Node 22.18 or later; no dependencies.

import { readFileSync } from "node:fs";
import { join } from "node:path";

const ROOT = join(import.meta.dirname, "..", "..");

/** `const NAME: f32|usize|bool = value;` in `file` (a path from the repository's root), public
 * or not; a bool is 1 or 0. An error if it is not there. */
export function rustConst(file: string, name: string): number {
  const m = readFileSync(join(ROOT, file), "utf8").match(new RegExp(`^\\s*(?:pub(?:\\([a-z]+\\))? )?const ${name}: (?:f32|usize|bool) = ([0-9.e_+-]+|true|false);`, "m"));
  if (!m) throw new Error(`no const ${name} in ${file}: a tool reads this number from the game's source so the two cannot differ (D556); keep the constant there, or change the tool that asks for it with it`);
  return m[1] === "true" ? 1 : m[1] === "false" ? 0 : Number(m[1].replaceAll("_", ""));
}
